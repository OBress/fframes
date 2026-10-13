//! The light that scatters in the air, see [`crate::Atmosphere`]. A march along the view rays
//! at half resolution finds the light that the air adds and the light that gets through. A
//! depth aware filter then applies both to the frame. `atmosphere.wgsl` does the same on the
//! GPU.

use std::f32::consts::PI;
use std::sync::LazyLock;

use glam::{Mat4, Vec3, Vec4};

use super::par;
use super::prepare::{LightKind, Lights};
use super::shade::attenuation;
use super::shadow::ShadowMap;
use crate::Atmosphere;
use crate::math::{hash, smoothstep};

/// The haze sees each lamp as a ball of this radius in meters. Samples nearer to a point
/// light than this get no more light, so they do not show as bright specks.
const LAMP_RADIUS: f32 = 0.2;

/// The noise repeats after this number of cells on each axis.
pub(crate) const NOISE_CELLS: usize = 32;

/// The value of each noise cell corner, `NOISE_CELLS`³ bytes. The GPU uses the same values as
/// a 3D texture.
pub(crate) static NOISE: LazyLock<Vec<u8>> = LazyLock::new(|| {
    (0..NOISE_CELLS.pow(3) as u32)
        .map(|i| (hash(i) * 255.0).round() as u8)
        .collect()
});

/// The air of one view.
pub(crate) struct Air<'a> {
    pub atmosphere: &'a Atmosphere,
    pub lights: &'a Lights,
    pub shadows: &'a [Option<ShadowMap>],
    pub camera: Vec3,
    pub forward: Vec3,
    pub inverse_view_proj: Mat4,
    /// The moment of the frame, `0` for a frame without motion blur. Each moment starts
    /// its steps elsewhere, so that the motion blur averages the starts.
    pub moment: usize,
}

impl Air<'_> {
    /// Adds the haze to `hdr`. `key` holds the depth key of each pixel: 1 / view depth, 0
    /// where there is no surface.
    pub(crate) fn apply(&self, hdr: &mut [Vec4], key: &[f32], width: usize, height: usize) {
        // Each half resolution pixel marches the ray of the full resolution pixel at twice
        // its coordinates. The filter then knows the depth of each sample.
        let (hw, hh) = (width.div_ceil(2), height.div_ceil(2));
        let mut haze = vec![Vec4::ZERO; hw * hh];
        par::chunks_mut(&mut haze, hw, |y, row| {
            let mut suns = Vec::new();
            for (x, out) in row.iter_mut().enumerate() {
                let (fx, fy) = (2 * x, 2 * y);
                let ndc = |z| {
                    Vec3::new(
                        (fx as f32 + 0.5) / width as f32 * 2.0 - 1.0,
                        1.0 - (fy as f32 + 0.5) / height as f32 * 2.0,
                        z,
                    )
                };
                let near = self.inverse_view_proj.project_point3(ndc(0.0));
                let far = self.inverse_view_proj.project_point3(ndc(1.0));
                let ray = (far - near).normalize_or(self.forward);
                let start = jitter(x, y, self.moment);
                *out = self.march(ray, key[fy * width + fx], start, &mut suns);
            }
        });

        par::chunks_mut(hdr, width, |y, row| {
            for (x, pixel) in row.iter_mut().enumerate() {
                let h = filter(&haze, key, width, hw, hh, x, y);
                *pixel = (pixel.truncate() * h.w + h.truncate()).extend(pixel.w);
            }
        });
    }

    /// The light that the air adds along `ray` up to the surface at depth key `key`, and the
    /// part of the light of the surface that gets through (in `w`). `suns` is space for the
    /// directional lights with a shadow.
    fn march<'s>(
        &'s self,
        ray: Vec3,
        key: f32,
        jitter: f32,
        suns: &mut Vec<(Vec3, &'s ShadowMap)>,
    ) -> Vec4 {
        let a = self.atmosphere;
        // A directional light scatters the same part of its light at each step. Only its
        // shadow changes.
        let mut constant = a.ambient.0;
        suns.clear();
        for l in &self.lights.direct {
            if let LightKind::Directional { direction, shadow } = &l.kind {
                let light = l.color * phase(direction.dot(-ray), a.anisotropy);
                match shadow.and_then(|i| self.shadows.get(i)?.as_ref()) {
                    Some(map) => suns.push((light, map)),
                    None => constant += light,
                }
            }
        }

        let length = if key > 0.0 {
            1.0 / (key * ray.dot(self.forward).max(1e-4))
        } else {
            f32::INFINITY
        };
        let end = length.min(a.distance);
        // The steps get longer with the distance. Near the camera the haze shows the most
        // detail.
        let steps = a.steps.max(1);
        let n = steps as f32;
        let mut scatter = Vec3::ZERO;
        let mut through = 1.0;
        for i in 0..steps {
            let i = i as f32;
            let u = (i + jitter) / n;
            let t = end * u * u;
            let dt = end * (2.0 * i + 1.0) / (n * n);
            let p = self.camera + ray * t;
            let density = self.density(p, dt);
            if density <= 0.0 {
                continue;
            }
            let mut light = constant + self.lamps(p, ray);
            // A long step crosses many shafts of light. The shadow is the mean of taps
            // spread over the step, so the shafts do not alias into ripples.
            let start = end * (i / n) * (i / n);
            for (sun, map) in suns.iter() {
                let lit: f32 = (0..SHADOW_TAPS)
                    .map(|k| {
                        let s = start + dt * (k as f32 + jitter) / SHADOW_TAPS as f32;
                        map.lit(self.camera + ray * s)
                    })
                    .sum();
                light += *sun * (lit / SHADOW_TAPS as f32);
            }
            // The light scattered in the step, without the part that the air absorbs
            // again on its way to the camera (Hillaire 2015).
            let absorbed = (-density * dt).exp();
            scatter += light * (through * (1.0 - absorbed));
            through *= absorbed;
        }
        (scatter * a.color.0).extend(through)
    }

    /// The density of the air at `p`, for a step of length `dt`. A step longer than the
    /// noise crosses many of its cells, and one sample of the noise does not tell their mean.
    /// When the camera moves, such samples change at random and the haze shimmers. So the
    /// noise fades out on long steps, to its mean.
    fn density(&self, p: Vec3, dt: f32) -> f32 {
        let a = self.atmosphere;
        let mut density = a.density * (-a.falloff * (p.y - a.base).max(0.0)).exp();
        let scale = 1.0 / a.noise_size.max(1e-3);
        let amount = a.noise * (1.0 - smoothstep(0.5, 2.0, dt * scale));
        if amount > 0.0 {
            density *= 1.0 + amount * (2.0 * fractal_noise(p * scale + a.offset * scale) - 1.0);
        }
        density
    }

    /// The light of the point and spot lights that the air at `p` scatters back along `ray`.
    fn lamps(&self, p: Vec3, ray: Vec3) -> Vec3 {
        let mut light = Vec3::ZERO;
        for l in &self.lights.direct {
            let (position, distance, decay, cone) = match l.kind {
                LightKind::Directional { .. } => continue,
                LightKind::Point {
                    position,
                    distance,
                    decay,
                } => (position, distance, decay, None),
                LightKind::Spot {
                    position,
                    direction,
                    distance,
                    decay,
                    cos_outer,
                    cos_inner,
                } => (
                    position,
                    distance,
                    decay,
                    Some((direction, cos_outer, cos_inner)),
                ),
            };
            let d = p - position;
            let dist = d.length().max(LAMP_RADIUS);
            let travel = d / dist;
            let mut radiance = l.color * attenuation(dist, distance, decay);
            if let Some((direction, cos_outer, cos_inner)) = cone {
                radiance *= smoothstep(cos_outer, cos_inner, travel.dot(direction));
            }
            light += radiance * phase(travel.dot(-ray), self.atmosphere.anisotropy);
        }
        light
    }
}

/// The Henyey-Greenstein phase function: the part of the light that turns by the angle with
/// cosine `cos`.
fn phase(cos: f32, g: f32) -> f32 {
    let x = (1.0 + g * g - 2.0 * g * cos).max(1e-4);
    (1.0 - g * g) / (4.0 * PI * x * x.sqrt())
}

/// The shadow taps of a step in the haze.
const SHADOW_TAPS: usize = 4;

/// The start of the steps of a half resolution pixel, `0..1`: interleaved gradient noise
/// (Jimenez 2014). Its values in a small block are spread over `0..1`, and the filter
/// averages them, without the regular pattern that a fixed matrix shows when the camera
/// moves. Each moment of a motion blurred frame moves the starts by the golden ratio.
fn jitter(x: usize, y: usize, moment: usize) -> f32 {
    let fract = |v: f32| v - v.floor();
    let noise = fract(52.982_918 * fract(0.067_110_56 * x as f32 + 0.005_837_15 * y as f32));
    fract(noise + 0.618_034 * moment as f32)
}

/// Two octaves of smooth 3D value noise, `0..1`.
fn fractal_noise(p: Vec3) -> f32 {
    (value_noise(p) * 2.0 + value_noise(p * 2.03 + 17.0)) / 3.0
}

/// Value noise with smoothstep weights between the values of [`NOISE`].
fn value_noise(p: Vec3) -> f32 {
    let cell = p.floor();
    let f = p - cell;
    let u = f * f * (3.0 - 2.0 * f);
    let c = cell.as_ivec3();
    let mask = NOISE_CELLS as i32 - 1;
    let wrap = |v: i32| (v & mask) as usize;
    let at = |x: i32, y: i32, z: i32| {
        let i = wrap(c.x + x) + (wrap(c.y + y) + wrap(c.z + z) * NOISE_CELLS) * NOISE_CELLS;
        f32::from(NOISE[i]) / 255.0
    };
    let lerp = |a: f32, b: f32, t: f32| a + (b - a) * t;
    let plane = |z| {
        lerp(
            lerp(at(0, 0, z), at(1, 0, z), u.x),
            lerp(at(0, 1, z), at(1, 1, z), u.x),
            u.y,
        )
    };
    lerp(plane(0), plane(1), u.z)
}

/// The haze at the full resolution pixel `x`, `y`: a box filter, 4 half resolution samples
/// wide, around it. The box averages the jitter starts of 16 samples. Samples at
/// another depth get less weight, so the haze of the sky does not leak onto the edges of
/// objects.
fn filter(
    haze: &[Vec4],
    key: &[f32],
    width: usize,
    hw: usize,
    hh: usize,
    x: usize,
    y: usize,
) -> Vec4 {
    let own = key[y * width + x];
    let (bx, by) = ((x / 2) as isize, (y / 2) as isize);
    let (fx, fy) = ((x % 2) as f32 * 0.5, (y % 2) as f32 * 0.5);
    // the part of the sample at offset `i` that the box covers
    let cover = |i: isize, f: f32| (2.5 - (i as f32 - f).abs()).clamp(0.0, 1.0);
    let mut sum = Vec4::ZERO;
    let mut total = 0.0;
    for j in -2..=2isize {
        let wy = cover(j, fy);
        let qy = (by + j).clamp(0, hh as isize - 1) as usize;
        for i in -2..=2isize {
            let w = cover(i, fx) * wy;
            if w == 0.0 {
                continue;
            }
            let qx = (bx + i).clamp(0, hw as isize - 1) as usize;
            let k = key[2 * qy * width + 2 * qx];
            let difference = (k - own).abs() / k.max(own).max(1e-6);
            let w = w / (0.02 + difference);
            sum += haze[qy * hw + qx] * w;
            total += w;
        }
    }
    sum / total
}
