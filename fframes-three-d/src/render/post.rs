//! Post processing of the HDR frame: bloom, depth of field, tone mapping and the conversion
//! to premultiplied sRGB bytes.

use std::sync::LazyLock;

use glam::{Mat3, Vec2, Vec3, Vec4};
use rayon::prelude::*;

use super::reflection::{MAX_LEVELS, level_count};
use crate::color::linear_to_srgb;

/// Glow around bright pixels, like the three.js `UnrealBloomPass`.
///
/// Give it to [`Renderer::bloom`](super::Renderer::bloom). Only pixels brighter than
/// `threshold` glow. Thus the parts that must glow need an emissive intensity above it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bloom {
    /// How much glow is added.
    pub strength: f32,
    /// How far the glow spreads, `0..1`.
    pub radius: f32,
    /// The linear brightness above which pixels glow. Lit surfaces are seldom brighter
    /// than 1. Emissive parts and sparks must be brighter.
    pub threshold: f32,
}

impl Default for Bloom {
    fn default() -> Self {
        Self {
            strength: 0.8,
            radius: 0.6,
            threshold: 1.0,
        }
    }
}

/// The weight of each bloom level after the first. `radius` moves weight to the wide levels.
pub(crate) const BLOOM_FACTORS: [f32; MAX_LEVELS - 1] = [0.8, 0.6, 0.4, 0.2, 0.1];

/// The weight of the upsampled level `level` (1 for the second level) for `radius`, as the
/// three.js `lerpBloomFactor`.
pub(crate) fn bloom_weight(level: usize, radius: f32) -> f32 {
    let f = BLOOM_FACTORS[level - 1];
    (f + (1.2 - f - f) * radius).max(0.0)
}

/// What the glow of `levels` levels sums to over an even frame of brightness 1: each level
/// adds the level below it times its weight. Dividing by it makes the mixed glow keep the
/// light of the frame.
pub(crate) fn bloom_total(levels: usize, radius: f32) -> f32 {
    let mut total = 1.0;
    for level in (1..levels).rev() {
        total = 1.0 + bloom_weight(level, radius) * total;
    }
    total
}

/// Ambient occlusion from the depth of the frame (screen space): creases, corners and the
/// ground under things darken, as the light of the room reaches them less.
///
/// Give it to [`Renderer::ambient_occlusion`](super::Renderer::ambient_occlusion). It
/// darkens the whole light of a pixel, after the lighting and before the depth of field and
/// the bloom. Orthographic cameras get none.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AmbientOcclusion {
    /// How much creases darken: 1 is a typical amount, 2 is heavy.
    pub strength: f32,
    /// How far around a point other surfaces occlude it, in meters.
    pub radius: f32,
}

impl Default for AmbientOcclusion {
    fn default() -> Self {
        Self {
            strength: 0.8,
            radius: 0.5,
        }
    }
}

/// The samples of the ambient occlusion of a pixel.
pub(crate) const AO_SAMPLES: usize = 12;
/// The largest sample radius in pixels of a 1080 pixel tall frame.
pub(crate) const AO_MAX_RADIUS: f32 = 90.0;
/// Samples nearer to the tangent plane than this cosine do not occlude: the normals come
/// from 16 bit depths on the GPU and tilt a little.
pub(crate) const AO_BIAS: f32 = 0.25;

/// How HDR colors map to the display, as in three.js.
///
/// Set it with [`Renderer::tone_mapping`](super::Renderer::tone_mapping).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ToneMapping {
    /// Clamps each channel to `0..1`.
    Linear,
    /// three.js `ACESFilmicToneMapping`. It gives high contrast, and bright colors become
    /// white.
    #[default]
    Aces,
    /// three.js `NeutralToneMapping` (Khronos PBR Neutral). It keeps base colors accurate.
    Neutral,
    /// AgX (Blender, three.js `AgXToneMapping`). Bright lights roll off to white smoothly
    /// and keep their hue longer than with ACES; the picture is flatter and less saturated,
    /// a good start for a grade. Mid grey comes out darker than with ACES (about a stop),
    /// so it wants more exposure.
    Agx,
}

/// An image of linear premultiplied colors, row major. The bloom and the reflection filter
/// it.
pub(crate) struct Image {
    pub width: usize,
    pub height: usize,
    pub data: Vec<Vec4>,
}

impl Image {
    /// The bilinear sample at `u`, `v` (`0..1` over the image). Outside the image it
    /// clamps to the edge, like the GPU sampler.
    pub fn sample(&self, u: f32, v: f32) -> Vec4 {
        let x = (u * self.width as f32 - 0.5).clamp(0.0, self.width as f32 - 1.0);
        let y = (v * self.height as f32 - 0.5).clamp(0.0, self.height as f32 - 1.0);
        let (x0, y0) = (x.floor() as usize, y.floor() as usize);
        let (x1, y1) = ((x0 + 1).min(self.width - 1), (y0 + 1).min(self.height - 1));
        let (fx, fy) = (x - x0 as f32, y - y0 as f32);
        let at = |x: usize, y: usize| self.data[y * self.width + x];
        at(x0, y0)
            .lerp(at(x1, y0), fx)
            .lerp(at(x0, y1).lerp(at(x1, y1), fx), fy)
    }

    /// The image and its [`level_count`] levels. Each level has half the size of the
    /// level before it.
    pub fn mip_chain(self) -> Vec<Image> {
        let count = level_count(self.width, self.height);
        let mut levels = vec![self];
        while levels.len() < count {
            levels.push(levels[levels.len() - 1].downsample());
        }
        levels
    }

    /// Makes a copy at half the size from four bilinear taps one texel out (a 4x4 tent
    /// filter). The GPU does the same in `fs_down` and `fs_down_rgba`.
    fn downsample(&self) -> Image {
        let width = (self.width / 2).max(1);
        let height = (self.height / 2).max(1);
        let (du, dv) = (1.0 / self.width as f32, 1.0 / self.height as f32);
        let mut data = vec![Vec4::ZERO; width * height];
        data.par_chunks_mut(width).enumerate().for_each(|(y, row)| {
            let v = (y as f32 + 0.5) / height as f32;
            for (x, out) in row.iter_mut().enumerate() {
                let u = (x as f32 + 0.5) / width as f32;
                *out = (self.sample(u - du, v - dv)
                    + self.sample(u + du, v - dv)
                    + self.sample(u - du, v + dv)
                    + self.sample(u + du, v + dv))
                    * 0.25;
            }
        });
        Image {
            width,
            height,
            data,
        }
    }

    /// Adds `coarse`, upsampled with a 3x3 tent filter, times `weight`.
    fn add_upsampled(&mut self, coarse: &Image, weight: f32) {
        let (width, height) = (self.width, self.height);
        let (du, dv) = (1.0 / coarse.width as f32, 1.0 / coarse.height as f32);
        self.data
            .par_chunks_mut(width)
            .enumerate()
            .for_each(|(y, row)| {
                let v = (y as f32 + 0.5) / height as f32;
                for (x, out) in row.iter_mut().enumerate() {
                    let u = (x as f32 + 0.5) / width as f32;
                    let mut sum = coarse.sample(u, v) * 4.0;
                    sum += (coarse.sample(u - du, v)
                        + coarse.sample(u + du, v)
                        + coarse.sample(u, v - dv)
                        + coarse.sample(u, v + dv))
                        * 2.0;
                    sum += coarse.sample(u - du, v - dv)
                        + coarse.sample(u + du, v - dv)
                        + coarse.sample(u - du, v + dv)
                        + coarse.sample(u + du, v + dv);
                    *out += sum * (weight / 16.0);
                }
            });
    }
}

/// Adds the glow of `bloom` to `hdr`, a frame of `width` x `height` pixels.
/// With `mix` the glow replaces `strength` of the frame instead of adding to it, see
/// [`Renderer::soft_bloom`](super::Renderer::soft_bloom).
pub(crate) fn bloom(hdr: &mut [Vec4], width: usize, height: usize, bloom: &Bloom, mix: bool) {
    if bloom.strength <= 0.0 {
        return;
    }
    // The bright pass, at half resolution and with a soft knee.
    let (hw, hh) = ((width / 2).max(1), (height / 2).max(1));
    let threshold = bloom.threshold.max(0.0);
    let knee = threshold * 0.5 + 1e-4;
    let mut bright = vec![Vec4::ZERO; hw * hh];
    let source = &*hdr;
    bright.par_chunks_mut(hw).enumerate().for_each(|(y, row)| {
        for (x, out) in row.iter_mut().enumerate() {
            let mut sum = Vec3::ZERO;
            for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                let sx = (x * 2 + dx).min(width - 1);
                let sy = (y * 2 + dy).min(height - 1);
                sum += source[sy * width + sx].truncate();
            }
            let c = sum * 0.25;
            let brightness = c.max_element();
            let soft = (brightness - threshold + knee).clamp(0.0, 2.0 * knee);
            let soft = soft * soft / (4.0 * knee);
            let contribution = soft.max(brightness - threshold) / brightness.max(1e-4);
            *out = (c * contribution.max(0.0)).extend(0.0);
        }
    });

    // Each level glows wider than the level before it.
    let mut levels = Image {
        width: hw,
        height: hh,
        data: bright,
    }
    .mip_chain();
    let total = bloom_total(levels.len(), bloom.radius);
    let Some(mut glow) = levels.pop() else {
        return;
    };
    while let Some(mut finer) = levels.pop() {
        finer.add_upsampled(&glow, bloom_weight(levels.len() + 1, bloom.radius));
        glow = finer;
    }
    let strength = bloom.strength;
    hdr.par_chunks_mut(width).enumerate().for_each(|(y, row)| {
        let v = (y as f32 + 0.5) / height as f32;
        for (x, px) in row.iter_mut().enumerate() {
            let u = (x as f32 + 0.5) / width as f32;
            let g = glow.sample(u, v);
            if mix {
                let mixed = px.truncate().lerp(g.truncate() / total, strength.min(1.0));
                *px = mixed.extend(px.w);
            } else {
                *px += g * strength;
            }
        }
    });
}

/// Darkens `hdr` by the ambient occlusion of its depth keys (`1 / view depth`, 0 for the
/// background). `focal` is the focal length in pixels (pixels per meter at 1 m). `fs_ao`
/// and `fs_ao_apply` in `post.wgsl` are the same.
pub(crate) fn ambient_occlusion(
    hdr: &mut [Vec4],
    depth: &[f32],
    width: usize,
    height: usize,
    focal: f32,
    ao: &AmbientOcclusion,
) {
    if ao.strength <= 0.0 || ao.radius <= 0.0 {
        return;
    }
    let max_radius = AO_MAX_RADIUS * height as f32 / 1080.0;
    let half = Vec2::new(width as f32, height as f32) * 0.5;
    // The position of a pixel in view space (x right, y down, z forward).
    let position = |x: i64, y: i64| -> Option<Vec3> {
        let x = x.clamp(0, width as i64 - 1) as usize;
        let y = y.clamp(0, height as i64 - 1) as usize;
        let key = depth[y * width + x];
        (key > 0.0).then(|| {
            let z = 1.0 / key;
            ((Vec2::new(x as f32, y as f32) + 0.5 - half) * (z / focal)).extend(z)
        })
    };
    let mut occlusion = vec![1.0f32; width * height];
    occlusion.par_chunks_mut(width).enumerate().for_each(|(y, row)| {
        for (x, out) in row.iter_mut().enumerate() {
            *out = ao_at(&position, x as i64, y as i64, focal, max_radius, ao);
        }
    });
    // A depth aware 4x4 blur: it spans the 4x4 pattern of the sample rotations.
    hdr.par_chunks_mut(width).enumerate().for_each(|(y, row)| {
        for (x, px) in row.iter_mut().enumerate() {
            let key = depth[y * width + x];
            if key <= 0.0 {
                continue;
            }
            let (mut sum, mut count) = (0.0, 0.0);
            for dy in -2..2i64 {
                for dx in -2..2i64 {
                    let sx = (x as i64 + dx).clamp(0, width as i64 - 1) as usize;
                    let sy = (y as i64 + dy).clamp(0, height as i64 - 1) as usize;
                    let k = depth[sy * width + sx];
                    if (k - key).abs() <= 0.1 * key {
                        sum += occlusion[sy * width + sx];
                        count += 1.0;
                    }
                }
            }
            let a = sum / f32::max(count, 1.0);
            *px = (px.truncate() * a).extend(px.w);
        }
    });
}

/// The unblurred ambient occlusion of one pixel, `0..1` (1 is open).
fn ao_at(
    position: &impl Fn(i64, i64) -> Option<Vec3>,
    x: i64,
    y: i64,
    focal: f32,
    max_radius: f32,
    ao: &AmbientOcclusion,
) -> f32 {
    let Some(c) = position(x, y) else {
        return 1.0;
    };
    // The normal from the neighbors two pixels away, on each axis the side nearer in depth
    // (the other may be across an edge).
    let side = |a: Option<Vec3>, b: Option<Vec3>| match (a, b) {
        (Some(a), Some(b)) => {
            if (c.z - a.z).abs() < (b.z - c.z).abs() {
                c - a
            } else {
                b - c
            }
        }
        (Some(a), None) => c - a,
        (None, Some(b)) => b - c,
        (None, None) => Vec3::ZERO,
    };
    let dx = side(position(x - 2, y), position(x + 2, y));
    let dy = side(position(x, y - 2), position(x, y + 2));
    let mut n = dy.cross(dx).normalize_or_zero();
    if n.dot(c) > 0.0 {
        n = -n;
    }
    let radius = (ao.radius * focal / c.z).min(max_radius);
    if radius < 1.0 || n == Vec3::ZERO {
        return 1.0;
    }
    let r2 = ao.radius * ao.radius;
    let angle = bayer(x as usize, y as usize) * std::f32::consts::TAU;
    let mut sum = 0.0;
    for i in 0..AO_SAMPLES {
        let t = (i as f32 + 0.5) / AO_SAMPLES as f32;
        let a = angle + i as f32 * 2.399_963;
        let offset = Vec2::new(a.cos(), a.sin()) * (radius * t);
        let Some(q) = position(x + offset.x.round() as i64, y + offset.y.round() as i64) else {
            continue;
        };
        let v = q - c;
        let vv = v.length_squared();
        if vv < 1e-12 {
            continue;
        }
        // A surface above the tangent plane occludes, less the farther it is.
        let cos = v.dot(n) / vv.sqrt();
        sum += (cos - AO_BIAS).max(0.0) * (1.0 - vv / r2).max(0.0);
    }
    (1.0 - ao.strength * 2.5 * sum / AO_SAMPLES as f32).clamp(0.0, 1.0)
}

fn aces(color: Vec3) -> Vec3 {
    const INPUT: Mat3 = Mat3::from_cols(
        Vec3::new(0.59719, 0.07600, 0.02840),
        Vec3::new(0.35458, 0.90834, 0.13383),
        Vec3::new(0.04823, 0.01566, 0.83777),
    );
    const OUTPUT: Mat3 = Mat3::from_cols(
        Vec3::new(1.60475, -0.10208, -0.00327),
        Vec3::new(-0.53108, 1.10813, -0.07276),
        Vec3::new(-0.07367, -0.00605, 1.07602),
    );
    let v = INPUT * (color / 0.6);
    let a = v * (v + 0.024_578_6) - 0.000_090_537;
    let b = v * (0.983_729 * v + 0.432_951) + 0.238_081;
    (OUTPUT * (a / b)).clamp(Vec3::ZERO, Vec3::ONE)
}

fn neutral(mut color: Vec3) -> Vec3 {
    const START: f32 = 0.8 - 0.04;
    const DESATURATION: f32 = 0.15;
    let x = color.min_element();
    let offset = if x < 0.08 { x - 6.25 * x * x } else { 0.04 };
    color -= offset;
    let peak = color.max_element();
    if peak < START {
        return color.max(Vec3::ZERO);
    }
    let d = 1.0 - START;
    let new_peak = 1.0 - d * d / (peak + d - START);
    color *= new_peak / peak;
    let g = 1.0 - 1.0 / (DESATURATION * (peak - new_peak) + 1.0);
    color.lerp(Vec3::splat(new_peak), g).max(Vec3::ZERO)
}

/// AgX as in three.js (after Filament and Blender): into a log encoding of the Rec. 2020
/// primaries pulled in toward white, a sigmoid, back out.
fn agx(color: Vec3) -> Vec3 {
    const SRGB_TO_2020: Mat3 = Mat3::from_cols(
        Vec3::new(0.6274, 0.0691, 0.0164),
        Vec3::new(0.3293, 0.9195, 0.0880),
        Vec3::new(0.0433, 0.0113, 0.8956),
    );
    const REC2020_TO_SRGB: Mat3 = Mat3::from_cols(
        Vec3::new(1.6605, -0.1246, -0.0182),
        Vec3::new(-0.5876, 1.1329, -0.1006),
        Vec3::new(-0.0728, -0.0083, 1.1187),
    );
    const INSET: Mat3 = Mat3::from_cols(
        Vec3::new(0.856_627_15, 0.137_318_97, 0.111_898_21),
        Vec3::new(0.095_121_24, 0.761_241_99, 0.076_799_42),
        Vec3::new(0.048_251_61, 0.101_439_04, 0.811_302_37),
    );
    const OUTSET: Mat3 = Mat3::from_cols(
        Vec3::new(1.127_100_6, -0.141_329_76, -0.141_329_76),
        Vec3::new(-0.110_606_64, 1.157_823_7, -0.110_606_64),
        Vec3::new(-0.016_493_94, -0.016_493_94, 1.251_936_4),
    );
    const MIN_EV: f32 = -12.473_93;
    const MAX_EV: f32 = 4.026_069;
    let c = (INSET * (SRGB_TO_2020 * color)).max(Vec3::splat(1e-10));
    let x = ((Vec3::new(c.x.log2(), c.y.log2(), c.z.log2()) - MIN_EV) / (MAX_EV - MIN_EV))
        .clamp(Vec3::ZERO, Vec3::ONE);
    let x2 = x * x;
    let x4 = x2 * x2;
    let curve = 15.5 * x4 * x2 - 40.14 * x4 * x + 31.96 * x4 - 6.868 * x2 * x + 0.4298 * x2
        + 0.1191 * x
        - 0.00232;
    let out = (OUTSET * curve).max(Vec3::ZERO);
    let out = Vec3::new(out.x.powf(2.2), out.y.powf(2.2), out.z.powf(2.2));
    (REC2020_TO_SRGB * out).clamp(Vec3::ZERO, Vec3::ONE)
}

const LUT_SIZE: usize = 16384;

/// The sRGB bytes of `LUT_SIZE` linear values in `0..1`.
static SRGB: LazyLock<Vec<u8>> = LazyLock::new(|| {
    (0..LUT_SIZE)
        .map(|i| {
            (linear_to_srgb(i as f32 / (LUT_SIZE - 1) as f32) * 255.0)
                .round()
                .clamp(0.0, 255.0) as u8
        })
        .collect()
});

/// Tone maps `hdr` and encodes it as premultiplied sRGB RGBA8, the format of `usvgr`
/// images.
pub(crate) fn encode(
    hdr: &[Vec4],
    width: usize,
    height: usize,
    exposure: f32,
    tone_mapping: ToneMapping,
) -> Vec<u8> {
    let lut: &[u8] = &SRGB;
    let mut out = vec![0u8; width * height * 4];
    out.par_chunks_mut(width * 4)
        .enumerate()
        .for_each(|(y, row)| {
            for (x, out) in row.as_chunks_mut::<4>().0.iter_mut().enumerate() {
                let px = hdr[y * width + x];
                let rgb = px.truncate().max(Vec3::ZERO) * exposure;
                // Light without coverage (glows, additive blending) becomes partly opaque light.
                let alpha = px.w.max(rgb.max_element().min(1.0)).clamp(0.0, 1.0);
                if alpha <= 0.0 {
                    continue;
                }
                let straight = rgb / alpha;
                let mapped = match tone_mapping {
                    ToneMapping::Linear => straight.clamp(Vec3::ZERO, Vec3::ONE),
                    ToneMapping::Aces => aces(straight),
                    ToneMapping::Neutral => neutral(straight).clamp(Vec3::ZERO, Vec3::ONE),
                    ToneMapping::Agx => agx(straight),
                };
                let a8 = (alpha * 255.0 + 0.5) as u8;
                let alpha = f32::from(a8) / 255.0;
                // An ordered dither against banding in dark gradients.
                let dither = (bayer(x, y) - 0.5) / 255.0;
                let channel = |c: f32| {
                    let i = ((c * (LUT_SIZE - 1) as f32) as usize).min(LUT_SIZE - 1);
                    let srgb = f32::from(lut[i]) / 255.0;
                    (((srgb + dither) * alpha).clamp(0.0, 1.0) * 255.0 + 0.5) as u8
                };
                *out = [channel(mapped.x), channel(mapped.y), channel(mapped.z), a8];
            }
        });
    out
}

/// The 4x4 Bayer threshold of a pixel, in `0..1`.
#[inline]
fn bayer(x: usize, y: usize) -> f32 {
    const M: [[u8; 4]; 4] = [[0, 8, 2, 10], [12, 4, 14, 6], [3, 11, 1, 9], [15, 7, 13, 5]];
    (f32::from(M[y & 3][x & 3]) + 0.5) / 16.0
}

/// Blurs `hdr` by the distance from the focus, like the depth of field of a lens.
///
/// It gathers, as in the single pass bokeh of Gustafsson. Each pixel averages samples on a
/// golden angle spiral, out to the largest blur near it. A sample counts where its own blur
/// reaches back to the pixel. Samples behind a sharp pixel blur only a little into it. Thus
/// sharp edges stay sharp in front of a blurred background.
///
/// `depth` holds the depth key of each pixel: `1 / distance`, or 0 for the background.
/// `focus` is the focus distance. `blur` is the blur radius in pixels of objects very far
/// behind the focus.
pub(crate) fn depth_of_field(
    hdr: &mut [Vec4],
    depth: &[f32],
    width: usize,
    height: usize,
    focus: f32,
    blur: f32,
) {
    // Samples nearer than INNER pixels read the frame. Samples farther away read a copy at
    // half resolution: the blur is wide there, and the smaller copy stays in the cache.
    const INNER: f32 = 6.0;
    const TILE: usize = 16;
    let max_blur = (blur * 1.6).min(40.0);
    // `focus * key` is `focus / distance`.
    let coc_of = |key: f32| (blur * (1.0 - focus * key).abs()).min(max_blur);
    let coc: Vec<f32> = depth.iter().map(|&key| coc_of(key)).collect();

    // The largest blur that reaches into each tile. Pixels of tiles without blur are
    // skipped.
    let (tw, th) = (width.div_ceil(TILE), height.div_ceil(TILE));
    let mut tile_max = vec![0.0f32; tw * th];
    for y in 0..height {
        for x in 0..width {
            let t = (y / TILE) * tw + x / TILE;
            tile_max[t] = tile_max[t].max(coc[y * width + x]);
        }
    }
    let reach = (max_blur / TILE as f32).ceil() as usize;
    let mut near_max = vec![0.0f32; tw * th];
    for ty in 0..th {
        for tx in 0..tw {
            let mut m = 0.0f32;
            for ny in ty.saturating_sub(reach)..(ty + reach + 1).min(th) {
                for nx in tx.saturating_sub(reach)..(tx + reach + 1).min(tw) {
                    m = m.max(tile_max[ny * tw + nx]);
                }
            }
            near_max[ty * tw + tx] = m;
        }
    }

    let source = hdr.to_vec();
    let (hw, hh) = (width.div_ceil(2), height.div_ceil(2));
    let mut half = vec![(Vec4::ZERO, 0.0f32); hw * hh];
    half.par_chunks_mut(hw).enumerate().for_each(|(hy, row)| {
        for (hx, out) in row.iter_mut().enumerate() {
            let mut color = Vec4::ZERO;
            let mut key = 0.0;
            for dy in 0..2 {
                for dx in 0..2 {
                    let x = (hx * 2 + dx).min(width - 1);
                    let y = (hy * 2 + dy).min(height - 1);
                    color += source[y * width + x];
                    key += depth[y * width + x];
                }
            }
            *out = (color * 0.25, key * 0.25);
        }
    });
    // The sample direction turns by the golden angle from sample to sample.
    let turn = Vec2::from_angle(2.399_963);
    hdr.par_chunks_mut(width).enumerate().for_each(|(y, row)| {
        for (x, out) in row.iter_mut().enumerate() {
            let radius_max = near_max[(y / TILE) * tw + x / TILE];
            if radius_max < 0.5 {
                continue;
            }
            let i = y * width + x;
            let center_coc = coc[i];
            let center_key = depth[i];
            let mut sum = source[i];
            let mut count = 1.0;
            let mut radius = 0.8f32;
            let mut dir = Vec2::X;
            while radius < radius_max {
                let offset = dir * radius;
                // `as usize` saturates at 0, so samples past the edge read the edge pixel.
                let (color, key) = if radius < INNER {
                    let sx = ((x as f32 + offset.x).round() as usize).min(width - 1);
                    let sy = ((y as f32 + offset.y).round() as usize).min(height - 1);
                    (source[sy * width + sx], depth[sy * width + sx])
                } else {
                    // the 2x2 block that the sample point is in
                    let sx = (((x as f32 + 0.5 + offset.x) * 0.5) as usize).min(hw - 1);
                    let sy = (((y as f32 + 0.5 + offset.y) * 0.5) as usize).min(hh - 1);
                    half[sy * hw + sx]
                };
                let mut sample_coc = coc_of(key);
                // A sample farther away than the pixel (a smaller key) blurs only a little
                // over it.
                if key < center_key {
                    sample_coc = sample_coc.min(center_coc * 2.0);
                }
                let m = (sample_coc - radius + 0.5).clamp(0.0, 1.0);
                sum += (sum / count).lerp(color, m);
                count += 1.0;
                // The far samples are 2x2 averages, so half the density is enough.
                radius += if radius < INNER { 1.6 } else { 3.2 } / radius;
                dir = turn.rotate(dir);
            }
            *out = sum / count;
        }
    });
}
