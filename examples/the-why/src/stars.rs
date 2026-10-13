//! The night sky on a sphere around the camera: stars of all magnitudes in the colors of
//! their temperatures, and the Milky Way with its dark lanes of dust. Shots 4 and 5 show the
//! same sky, so the cut between them matches.

use std::f32::consts::PI;

use fframes_3d::math::{hash, noise1, random_direction, smoothstep};
use fframes_3d::*;

/// The distance of the sky from the camera in meters. It is less than the far plane of the
/// default camera.
const RADIUS: f32 = 400.0;

/// The pole of the Milky Way. The band is the great circle 90 degrees from it. It crosses
/// the last view of shot 4, (0.145, 0.906, 0.397), at 35 degrees and rises to the right.
const POLE: Vec3 = vec3(-0.2841, -0.3463, 0.8941);
/// The core of the Milky Way: on the band, 18 degrees down and left of that view.
const CORE: Vec3 = vec3(0.4308, 0.787, 0.4417);

/// The number of stars that the eye resolves, on the whole sphere.
const STARS: u32 = 48000;
/// The magnitude of the faintest stars.
const FAINTEST: f32 = 8.0;
/// The radius of a star in meters, about 0.3 pixels at 1920 pixels with a 35 mm lens.
const STAR_SIZE: f32 = 0.06;
/// The light of a star of the faintest magnitude.
const STAR_LIGHT: f32 = 0.15;

/// The spacing of the soft glow sprites of the Milky Way, in radians.
const GLOW_STEP: f32 = 0.0065;
/// The radius of a glow sprite in radians.
const GLOW_RADIUS: f32 = 0.014;
/// The light of a glow sprite in the brightest part of the band.
const GLOW_LIGHT: f32 = 0.007;
/// The attempts to place a faint star of the band. The light of the band decides if it
/// stays.
const GRAINS: u32 = 300_000;

/// Star colors from hot to cool (sRGB), and the part of the stars that has each color.
const COLORS: [(u32, f32); 6] = [
    (0x9bb0ff, 0.07),
    (0xcad7ff, 0.18),
    (0xf8f7ff, 0.2),
    (0xfff4ea, 0.18),
    (0xffd2a1, 0.27),
    (0xffb56c, 0.1),
];

/// The sky. Set the position of the node to the camera position in each frame, so the sky
/// stays infinitely far away.
pub fn stars() -> Node {
    let mut items: Vec<Particle> = field().collect();
    items.extend(milky_way());
    Node::particles(items, Blending::Additive).named("stars")
}

/// The color of a star for the random value `pick` in `0..1`.
fn star_color(pick: f32) -> Rgb {
    let mut sum = 0.0;
    for (hex, part) in COLORS {
        sum += part;
        if pick < sum {
            return Rgb::hex(hex);
        }
    }
    Rgb::hex(COLORS[COLORS.len() - 1].0)
}

/// The resolved stars: few bright ones and many faint ones. The number of stars brighter
/// than a magnitude grows about 2.8 times with each magnitude.
fn field() -> impl Iterator<Item = Particle> {
    (0..STARS).filter_map(|i| {
        let dir = random_direction(i * 3 + 1);
        if dir.y < -0.2 {
            return None;
        }
        let magnitude = (FAINTEST + hash(i * 3 + 2).max(1e-9).log10() / 0.45).max(-1.0);
        // A bright star blooms. The power keeps the brightest from a flat white disc.
        let flux = 10f32.powf(-0.4 * (magnitude - FAINTEST)).powf(0.75);
        let color = star_color(hash(i * 5 + 7)) * (STAR_LIGHT * flux);
        Some(Particle::new(dir * RADIUS, STAR_SIZE, color))
    })
}

/// The direction at `longitude` along the Milky Way from its core and `latitude` across it,
/// in radians.
fn on_band(longitude: f32, latitude: f32) -> Vec3 {
    let east = POLE.cross(CORE);
    (CORE * longitude.cos() + east * longitude.sin()) * latitude.cos() + POLE * latitude.sin()
}

/// The Milky Way: a soft glow, grainy with faint stars, cut by lanes of dust.
fn milky_way() -> Vec<Particle> {
    let mut items = Vec::new();
    // The glow sprites stand on a jittered grid, so the glow is smooth.
    let (columns, rows) = ((2.0 * PI / GLOW_STEP) as u32, (0.6 / GLOW_STEP) as u32);
    for i in 0..columns * rows {
        let h = |n: u32| hash(i * 5 + n + 90_001);
        let longitude = ((i % columns) as f32 + 0.5 * h(1)) * GLOW_STEP - PI;
        let latitude = ((i / columns) as f32 + 0.5 * h(2)) * GLOW_STEP - 0.3;
        let light = brightness(longitude, latitude);
        let dir = on_band(longitude, latitude);
        if light < 0.02 || dir.y < -0.2 {
            continue;
        }
        // The core is warm, the arms are cooler.
        let core = (-(longitude / 0.7).powi(2)).exp();
        let tint = Rgb::hex(0xc8d4ff).lerp(Rgb::hex(0xfff0e0), core);
        items.push(Particle::new(
            dir * RADIUS,
            GLOW_RADIUS * RADIUS,
            tint * (GLOW_LIGHT * light),
        ));
    }
    // Faint stars, dense where the band is bright.
    for i in 0..GRAINS {
        let h = |n: u32| hash(i * 7 + n + 400_009);
        let longitude = (h(1) * 2.0 - 1.0) * PI;
        let latitude = (h(2) * 2.0 - 1.0) * 0.3;
        let light = brightness(longitude, latitude);
        let dir = on_band(longitude, latitude);
        if h(3) > light || dir.y < -0.2 {
            continue;
        }
        let color = star_color(h(4)) * (STAR_LIGHT * (0.3 + 0.9 * h(5).powi(2)));
        items.push(Particle::new(dir * RADIUS, STAR_SIZE, color));
    }
    items
}

/// The light of the Milky Way at a place on the band, about 1 in the core and 0 far from
/// the band and in the dust. The angles are in radians, see [`on_band`].
fn brightness(longitude: f32, latitude: f32) -> f32 {
    let core = (-(longitude / 0.6).powi(2)).exp();
    // The band is wider at the core, and its edges wave.
    let width = 0.08 + 0.08 * core + 0.025 * fbm(11, longitude * 2.0, 0.3, 3);
    let middle = 0.02 * fbm(12, longitude * 1.5, 0.7, 2);
    let across = (-((latitude - middle) / width).powi(2)).exp();
    // Clouds of stars along the band.
    let clouds = 0.6 + 0.4 * fbm(13, longitude * 5.0, latitude * 12.0, 4);
    (0.35 + 0.65 * core) * across * clouds * (1.0 - dust(longitude, latitude))
}

/// The dust in front of the band, `0..1`: a long rift with broken edges near the middle of
/// the band through the core, and dark clouds that stretch along the band.
fn dust(longitude: f32, latitude: f32) -> f32 {
    let center = 0.01 + 0.03 * fbm(21, longitude * 1.5, 0.5, 3);
    let width = 0.028 * (1.0 + 0.6 * fbm(22, longitude * 3.0, 0.5, 3));
    let length = smoothstep(-1.1, -0.5, longitude) * (1.0 - smoothstep(1.0, 1.8, longitude));
    let edge = 0.3 * fbm(23, longitude * 8.0, latitude * 20.0, 3);
    let lane = (-((latitude - center) / width).powi(2)).exp() + edge;
    let rift = 0.92 * length * smoothstep(0.0, 0.6, lane);
    let clouds = 0.8 * smoothstep(0.05, 0.45, fbm(24, longitude * 4.0, latitude * 11.0, 5));
    1.0 - (1.0 - rift) * (1.0 - clouds)
}

/// Fractal noise in about `-1..1`: `octaves` layers of [`noise2`], each with twice the
/// frequency and half the amplitude of the layer before.
fn fbm(seed: u32, x: f32, y: f32, octaves: u32) -> f32 {
    let (mut sum, mut amplitude, mut total) = (0.0, 1.0, 0.0);
    let (mut x, mut y) = (x, y);
    for i in 0..octaves {
        sum += amplitude * noise2(seed + i * 31, x, y);
        total += amplitude;
        amplitude *= 0.5;
        (x, y) = (x * 2.03 + 1.7, y * 2.03 - 0.6);
    }
    sum / total
}

/// Smooth 2D value noise in `-1..1`: rows of [`noise1`] that blend into each other.
fn noise2(seed: u32, x: f32, y: f32) -> f32 {
    let row = y.floor();
    let f = y - row;
    let u = f * f * (3.0 - 2.0 * f);
    let r = row as i32 as u32;
    let a = noise1(seed.wrapping_add(r.wrapping_mul(7919)), x);
    let b = noise1(seed.wrapping_add(r.wrapping_add(1).wrapping_mul(7919)), x);
    a + (b - a) * u
}
