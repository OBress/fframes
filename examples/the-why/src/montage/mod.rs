//! Shot 8, "The why" (67 to 82 s of the film). After the engineer draws his line, the
//! machines build what people need. Each part starts from a warm line:
//!
//! - 0 to 5 s, [`bridge`]: the line is the pilot line of the Golden Gate Bridge at blue hour.
//!   Machines build the bridge in the real strait, and San Francisco and Sausalito light
//!   up.
//! - 5 to 10 s, [`greenhouse`]: the headlights of the first car become a point of light that
//!   draws a ring on the sand. Drones build a glass dome on it, and the dome turns green.
//! - 10 to 15 s, [`rocket`]: a whip pan lands on a launch tower at dawn. Warm lights run up
//!   the tower, the rocket lifts off, and its flare fills the frame for the cut to shot 9.
//!
//! The machines carry cold blue-white light. What they build carries warm light.

mod bay;
mod bridge;
mod golden_gate;
mod greenhouse;
mod rocket;

use fframes::{Svgr, svgr};
use fframes_3d::math::hash;
use fframes_3d::*;

use crate::WIDTH;
use crate::assets::Assets;
use crate::look::{BAR, PICTURE_HEIGHT};

/// The length of the shot in seconds.
pub const DURATION: f32 = 15.0;
/// The start of the greenhouse part in seconds.
pub const GREENHOUSE: f32 = 5.0;
/// The start of the rocket part in seconds.
pub const ROCKET: f32 = 10.0;

/// The warm core of human intent.
const AMBER: u32 = 0xffb35c;
/// The edges of the warm line.
const AMBER_EDGE: u32 = 0xff8a2a;
/// The cold light of the machines.
const COLD: u32 = 0xcfe4ff;
/// The cold blue of the machine LEDs.
const COLD_BLUE: u32 = 0x7fb4ff;

/// The shot: the templates of the three parts.
pub struct MontageSet {
    bridge: bridge::Bridge,
    greenhouse: greenhouse::Greenhouse,
    rocket: rocket::Rocket,
}

/// Distant versions of the finished builds for shot 9. Each scene holds only nodes in its
/// own space: meters, +Y up, the origin on the ground at the center of the build. Add one to
/// a scene with `scene.append(Some(anchor), &landmarks.bridge)` below a group node that
/// places and turns it.
pub struct Landmarks {
    /// The Golden Gate Bridge, finished and lit, at a fifth of its size, see
    /// [`golden_gate_landmark`].
    pub bridge: Scene,
    /// The glass dome, 40 m wide, with the plants and the warm light inside.
    pub greenhouse: Scene,
    /// The launch tower, and the trail of the rocket that rises to 2.5 km and bends to +X.
    pub contrail: Scene,
}

impl MontageSet {
    /// Builds the three sets. It takes about a second.
    pub fn new(assets: &Assets) -> Self {
        Self {
            bridge: bridge::Bridge::new(assets),
            greenhouse: greenhouse::Greenhouse::new(assets),
            rocket: rocket::Rocket::new(assets),
        }
    }

    /// The renderer for pictures of `width` x `height` pixels: ACES and a wide bloom, so the
    /// warm lights and the final flare glow.
    pub fn renderer(width: u32, height: u32) -> Renderer {
        Renderer::new(width, height).bloom(Bloom {
            strength: 0.55,
            radius: 0.75,
            threshold: 1.0,
        })
    }

    /// The scene and the camera at `t` seconds into the shot.
    pub fn moment(&self, t: f32) -> (Scene, Camera) {
        if t < GREENHOUSE {
            self.bridge.moment(t)
        } else if t < ROCKET {
            self.greenhouse.moment(t - GREENHOUSE)
        } else {
            self.rocket.moment(t - ROCKET)
        }
    }

    /// The motion blur samples at `t`: more on the fast camera moves and the transitions.
    pub fn shutter(&self, t: f32) -> usize {
        match t {
            // the pull back from the cable
            t if t < 1.8 => 3,
            // the car on the bridge: its glare is wider than the gap between two samples
            t if (4.4..GREENHOUSE).contains(&t) => 2,
            // the pen at the start of the ring
            t if (GREENHOUSE..5.6).contains(&t) => 3,
            // the whip pan: the most samples at the cut, where it is fastest. The dome costs
            // more per sample than the pad, so its side gets fewer.
            t if (9.55..ROCKET).contains(&t) => 4,
            t if (ROCKET..10.15).contains(&t) => 6,
            t if (10.15..10.5).contains(&t) => 5,
            // the lift off
            t if t > 12.4 => 3,
            _ => 1,
        }
    }

    /// The flare of the last frames turns the picture to warm white for the cut to the sun of
    /// shot 9. The 3D flare does most of it. This adds the last part, which bloom cannot
    /// give.
    pub fn overlay(&self, t: f32) -> Svgr<'static> {
        let white = final_white(t);
        if white <= 0.0 {
            return Svgr::empty();
        }
        svgr!(
            <rect x="0" y={BAR} width={WIDTH} height={PICTURE_HEIGHT} fill="#fff3dc" opacity={white} />
        )
    }

    /// The finished builds, seen from far away in shot 9.
    pub fn landmarks(&self) -> Landmarks {
        Landmarks {
            bridge: self.bridge.landmark(),
            greenhouse: self.greenhouse.landmark(),
            contrail: self.rocket.landmark(),
        }
    }
}

/// The Golden Gate Bridge for the distance of shot 9: finished, with its lamps and red
/// lights, without terrain and without lights. It is a fifth of the real size: the main
/// span is 256 m, the towers are 45 m high, and the road is 15 m above the feet of the
/// towers. X runs along the bridge, Z across it, and the origin is the middle of the main
/// span at the level of the feet of the towers. The towers stand at X = ±128 m, the
/// anchorages at ±197 m, and the approaches end at ±276 m on the ground (Y = 0).
pub fn golden_gate_landmark() -> Scene {
    golden_gate::bridge(
        &golden_gate::Paints::plain(),
        bridge::LANDMARK_SCALE,
        &|_, _| 0.0,
    )
}

/// The opacity of the warm white that [`MontageSet::overlay`] draws at `t`, `0..1`.
pub fn final_white(t: f32) -> f32 {
    rocket::final_white(t - ROCKET)
}

/// Cubic ease in and out, `0..1` to `0..1`.
fn ease(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    if x < 0.5 {
        4.0 * x * x * x
    } else {
        1.0 - (-2.0 * x + 2.0).powi(3) / 2.0
    }
}

/// Cubic ease out, `0..1` to `0..1`.
fn ease_out(x: f32) -> f32 {
    1.0 - (1.0 - x.clamp(0.0, 1.0)).powi(3)
}

/// Cubic ease in, `0..1` to `0..1`.
fn ease_in(x: f32) -> f32 {
    x.clamp(0.0, 1.0).powi(3)
}

/// The value from `a` to `b` at `t`, `0..1`.
fn mix(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// Smooth 2D value noise in `0..1`, with cells of size 1. A different `seed` gives a
/// different pattern.
fn noise2(seed: u32, x: f32, y: f32) -> f32 {
    let (ix, iy) = (x.floor(), y.floor());
    let (fx, fy) = (x - ix, y - iy);
    let (ux, uy) = (fx * fx * (3.0 - 2.0 * fx), fy * fy * (3.0 - 2.0 * fy));
    let at = |dx: i32, dy: i32| {
        let cx = (ix as i32 + dx) as u32;
        let cy = (iy as i32 + dy) as u32;
        hash(
            cx.wrapping_mul(73_856_093)
                ^ cy.wrapping_mul(19_349_663)
                ^ seed.wrapping_mul(83_492_791),
        )
    };
    let top = mix(at(0, 0), at(1, 0), ux);
    let bottom = mix(at(0, 1), at(1, 1), ux);
    mix(top, bottom, uy)
}

/// Fractal 2D noise in about `-1..1`: `octaves` layers of [`noise2`], each two times finer
/// and half as strong.
fn fbm(seed: u32, x: f32, y: f32, octaves: u32) -> f32 {
    let (mut sum, mut amplitude, mut frequency, mut total) = (0.0, 1.0, 1.0, 0.0);
    for octave in 0..octaves {
        sum += amplitude * (noise2(seed + octave * 7, x * frequency, y * frequency) * 2.0 - 1.0);
        total += amplitude;
        amplitude *= 0.5;
        frequency *= 2.03;
    }
    sum / total
}

/// The transform that turns +Y into the direction from `a` to `b`, with the length of that
/// direction as the Y scale and `thickness` as the X and Z scale. It places a unit cylinder
/// that stands on Y = 0..1 between two points.
fn span(a: Vec3, b: Vec3, thickness: f32) -> Mat4 {
    let d = b - a;
    let rotation = Quat::from_rotation_arc(Vec3::Y, d.normalize_or(Vec3::Y));
    Mat4::from_scale_rotation_translation(vec3(thickness, d.length(), thickness), rotation, a)
}

/// A sky panorama: `zenith` straight up, `horizon` at the horizon, and the warm `glow` of
/// the sun below the horizon around the direction `azimuth` (radians, `atan2(z, x)`). Thin
/// clouds near the horizon are dark against the sky and warm in the glow.
fn sky(zenith: Rgb, horizon: Rgb, glow: Rgb, azimuth: f32) -> std::sync::Arc<EnvironmentMap> {
    use std::f32::consts::{PI, TAU};
    let (width, height) = (1024, 512);
    let ground = horizon.0 * 0.08;
    let data = (0..width * height)
        .map(|i| {
            let (x, y) = (i % width, i / width);
            // the inverse of the panorama mapping of the engine
            let around = ((x as f32 + 0.5) / width as f32 - 0.5) * TAU;
            let elevation = (0.5 - (y as f32 + 0.5) / height as f32) * PI;
            if elevation < 0.0 {
                return horizon
                    .0
                    .lerp(ground, math::smoothstep(0.0, -0.08, elevation));
            }
            let mut color = horizon.0.lerp(zenith.0, elevation.sin().powf(0.45));
            let away = (around - azimuth).rem_euclid(TAU) - PI;
            let band = (-(away / 0.9).powi(2)).exp();
            color += glow.0 * band * ((-elevation / 0.06).exp() + 0.08 * (-elevation / 0.3).exp());
            let streak = fbm(71, around * 7.0, elevation * 55.0, 4);
            let cloud = math::smoothstep(0.1, 0.45, streak)
                * math::smoothstep(0.02, 0.05, elevation)
                * math::smoothstep(0.2, 0.06, elevation);
            let lit = glow.0 * (0.08 + 0.8 * band);
            color.lerp(lit.max(zenith.0 * 0.6), cloud * 0.7)
        })
        .collect();
    std::sync::Arc::new(EnvironmentMap::from_linear(width, height, data))
}

/// A ring of desert mountains around the origin, `radius` meters away, from `low` to `high`
/// meters tall. The two desert parts share it, so the skyline matches over the whip pan.
fn ridges(radius: f32, low: f32, high: f32) -> Geometry {
    use std::f32::consts::TAU;
    let steps = 240;
    let mut positions = Vec::new();
    let mut uvs = Vec::new();
    for i in 0..=steps {
        let a = i as f32 / steps as f32 * TAU;
        let n = fbm(21, a.cos() * 3.0 + 9.0, a.sin() * 3.0 + 9.0, 4) * 0.5 + 0.5;
        let r = radius + 500.0 * fbm(23, a.cos() * 2.0, a.sin() * 2.0, 2);
        let h = low + (high - low) * n.powf(1.3);
        let dir = vec3(a.sin(), 0.0, a.cos());
        for (d, y) in [(-900.0, -20.0), (0.0, h), (700.0, h * 0.5)] {
            positions.push(dir * (r + d) + vec3(0.0, y, 0.0));
            uvs.push(vec2(i as f32 * 40.0, y));
        }
    }
    let mut indices = Vec::new();
    for i in 0..steps as u32 {
        for j in 0..2u32 {
            let (a, b) = (i * 3 + j, (i + 1) * 3 + j);
            indices.extend([a, a + 1, b, b, a + 1, b + 1]);
        }
    }
    Geometry::new(positions, Vec::new(), uvs, indices)
}

/// A camera with a lens of `mm` millimeters from `position` to `target`, with the clip
/// planes for scenes of kilometers. The near plane grows with the distance to the subject,
/// so the depth buffer keeps its precision.
fn camera(mm: f32, position: Vec3, target: Vec3) -> Camera {
    let near = (position.distance(target) * 0.05).clamp(0.05, 4.0);
    crate::look::lens(mm)
        .position(position)
        .look_at(target)
        .clip(near, 6000.0)
}
