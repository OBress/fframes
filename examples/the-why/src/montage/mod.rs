//! Shot 8, "The why" (67 to 82 s of the film). After the engineer draws his line, the
//! machines build what people need, in six parts. Each part starts from a warm line, and
//! the light goes from night to dawn. The machines carry cold blue-white light. What they
//! build carries warm light.
//!
//! The times are seconds of the shot. The parts play faster than their own time where the
//! shot needs it (see [`PARTS`]).
//!
//! - 0.0 to 3.75, [`bridge`]: blue hour. The line is the pilot line of the Golden Gate
//!   Bridge. Machines build the bridge in the real strait, the lights of San Francisco and
//!   Sausalito come on, and the headlights of the first car flare up.
//! - 3.75 to 4.99, [`wind`]: night. The flare sits on the crest of a ridge and splits
//!   into the warm line along it. Turbines rise, their blades turn, and the lights of the
//!   village below come on. A tree wipes the picture.
//! - 4.99 to 6.08, [`school`]: night. The wipe goes on past a tree of the school yard. A
//!   gantry prints the walls of a school along the warm footprint, the windows light up,
//!   and a flag runs up. The camera tilts up after the flag into the stars.
//! - 6.08 to 7.24, [`water`]: before the dawn. The tilt comes down from the same stars to
//!   a warm line on a dry plain. A paver builds a canal on it, the water runs down the
//!   canal behind a warm head, and the field turns green in rows.
//! - 7.24 to 10.89, [`greenhouse`]: first light. The head of the water is the pen that
//!   draws a ring on the sand. Machines build a glass dome on it, and the dome turns
//!   green. A spark of the ring flies away, and the camera whips after it.
//! - 10.89 to 15.0, [`rocket`]: dawn. The whip pan lands on a launch pad, and the spark
//!   runs up the tower. The rocket lifts off, and its flare fills the frame for the cut to
//!   the sunrise of shot 9.

mod bay;
mod bridge;
mod golden_gate;
mod greenhouse;
mod rocket;
pub mod school;
pub mod water;
pub mod wind;

use fframes::{Svgr, svgr};
use fframes_3d::math::hash;
use fframes_3d::*;

use crate::WIDTH;
use crate::assets::Assets;
use crate::look::{BAR, PICTURE_HEIGHT};

/// The length of the shot in seconds.
pub const DURATION: f32 = 15.0;
/// The cuts between the six parts, in seconds of the shot. Each cut lies between two
/// frames at 60 fps, so the motion blur of a frame never mixes two parts.
pub const CUTS: [f32; 5] = [
    224.5 / 60.0,
    294.5 / 60.0,
    364.5 / 60.0,
    434.5 / 60.0,
    653.5 / 60.0,
];
/// The start of the greenhouse part in seconds of the shot.
pub const GREENHOUSE: f32 = CUTS[3];
/// The start of the rocket part in seconds of the shot.
pub const ROCKET: f32 = CUTS[4];
/// The rate of the time of the quick builds: they play a little faster than their own
/// time.
const BUILD_RATE: f32 = wind::LENGTH / (CUTS[1] - CUTS[0]);

/// The parts of the shot, in their order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Part {
    Bridge,
    Wind,
    School,
    Water,
    Greenhouse,
    Rocket,
}

/// A key of the time of a part: the second of the shot, the second of the part, and the
/// seconds of the part per second of the shot there.
type Key = (f32, f32, f32);

/// The time of each part, as keys. Between two keys the time of the part is a cubic curve
/// with the rates of the keys, so its speed changes smoothly. The bridge, the
/// greenhouse and the rocket have 5 s each, and play faster in the middle. They play at
/// their own speed where their transitions need it: the flare of the car, and both sides
/// of the whip pan.
const PARTS: [(Part, &[Key]); 6] = [
    (Part::Bridge, &[(0.0, 0.0, 0.9), (CUTS[0], 5.0, 1.0)]),
    (
        Part::Wind,
        &[
            (CUTS[0], 0.0, BUILD_RATE),
            (CUTS[1], wind::LENGTH, BUILD_RATE),
        ],
    ),
    (
        Part::School,
        &[
            (CUTS[1], 0.0, BUILD_RATE),
            (CUTS[2], school::LENGTH, BUILD_RATE),
        ],
    ),
    (
        Part::Water,
        &[
            (CUTS[2], 0.0, BUILD_RATE),
            (CUTS[3], water::LENGTH, BUILD_RATE),
        ],
    ),
    // the ring and the climb of the light (0 to 2.6) stay readable, the dome turns green
    // at about 3.0
    (
        Part::Greenhouse,
        &[
            (CUTS[3], 0.0, 1.1),
            (CUTS[3] + 2.3, 2.6, 1.2),
            (CUTS[4], 5.0, 1.0),
        ],
    ),
    // the build of the pad plays fast, the launch near its own speed
    (
        Part::Rocket,
        &[
            (CUTS[4], 0.0, 1.0),
            (CUTS[4] + 1.35, 2.0, 1.05),
            (DURATION, 5.0, 1.1),
        ],
    ),
];

/// The part at `t` seconds of the shot, and the time in it.
fn part_at(t: f32) -> (Part, f32) {
    let (part, keys) = PARTS[CUTS.partition_point(|&cut| cut <= t)];
    (part, warp(keys, t))
}

/// The time of a part at `t` seconds of the shot, from its keys, see [`PARTS`]. Outside the
/// keys the time goes on at the rate of the nearest key, for the motion blur samples.
fn warp(keys: &[Key], t: f32) -> f32 {
    let i = keys.partition_point(|k| k.0 <= t).clamp(1, keys.len() - 1);
    let ((t0, p0, v0), (t1, p1, v1)) = (keys[i - 1], keys[i]);
    if t < t0 {
        return p0 + v0 * (t - t0);
    }
    if t > t1 {
        return p1 + v1 * (t - t1);
    }
    // the cubic Hermite curve from (t0, p0) with the slope v0 to (t1, p1) with the slope v1
    let h = t1 - t0;
    let s = (t - t0) / h;
    let (s2, s3) = (s * s, s * s * s);
    (2.0 * s3 - 3.0 * s2 + 1.0) * p0
        + (s3 - 2.0 * s2 + s) * h * v0
        + (3.0 * s2 - 2.0 * s3) * p1
        + (s3 - s2) * h * v1
}

/// The warm core of human intent.
const AMBER: u32 = 0xffb35c;
/// The edges of the warm line.
const AMBER_EDGE: u32 = 0xff8a2a;
/// The cold light of the machines.
const COLD: u32 = 0xcfe4ff;
/// The cold blue of the machine LEDs.
const COLD_BLUE: u32 = 0x7fb4ff;
/// Half the angle of the whip pan from the greenhouse to the rocket, in radians. The camera
/// turns this much to the left on each side of the cut.
const WHIP_HALF: f32 = 0.72;
/// The pitch of the view in radians at the cut from the school to the water: up in the
/// stars. The school tilts up to it, and the water tilts down from it.
const SKY_TILT: f32 = 0.5;
/// The rate of that tilt at the cut in rad/s.
const SKY_RATE: f32 = 2.6;
/// The rate of the whip pan in 1/s. At the cut the camera turns at `WHIP_HALF * WHIP_RATE`
/// rad/s.
const WHIP_RATE: f32 = 12.0;

/// The shot: the templates of the six parts.
pub struct MontageSet {
    bridge: bridge::Bridge,
    wind: wind::Wind,
    school: school::School,
    water: water::Water,
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
    /// Builds the six sets. It takes about a second.
    pub fn new(assets: &Assets) -> Self {
        // The glow of the greenhouse sky is where the glow of the rocket sky is at the cut,
        // so the whip pan crosses one sky.
        let heading = |camera: Camera| camera.forward().z.atan2(camera.forward().x);
        let glow =
            rocket::GLOW + heading(greenhouse::camera_at(5.0)) - heading(rocket::camera_at(0.0));
        Self {
            bridge: bridge::Bridge::new(assets),
            wind: wind::Wind::new(assets),
            school: school::School::new(assets),
            water: water::Water::new(assets, school::heading()),
            greenhouse: greenhouse::Greenhouse::new(assets, glow),
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
        let (part, x) = part_at(t);
        match part {
            Part::Bridge => self.bridge.moment(x),
            Part::Wind => self.wind.moment(x),
            Part::School => self.school.moment(x),
            Part::Water => self.water.moment(x),
            Part::Greenhouse => self.greenhouse.moment(x),
            Part::Rocket => self.rocket.moment(x),
        }
    }

    /// The motion blur samples at `t`: more on the fast camera moves and the transitions.
    pub fn shutter(&self, t: f32) -> usize {
        // The whip pan: about one sample for each 20 pixels of blur at 1920 pixels, and 6 at
        // most, so the frame stays within its time. Both sides of the whip play at their
        // own speed.
        let whip = |x: f32| {
            let rate = WHIP_HALF * WHIP_RATE / (WHIP_RATE * x).cosh().powi(2);
            (rate * 0.9).ceil() as usize
        };
        let (part, x) = part_at(t);
        match part {
            Part::Bridge => match x {
                // the pull back from the cable
                x if x < 1.8 => 3,
                // the car on the bridge: its glare is wider than the gap between two samples
                x if x > 4.4 => 2,
                _ => 1,
            },
            Part::Wind => self.wind.shutter(x),
            Part::School => self.school.shutter(x),
            Part::Water => self.water.shutter(x),
            // the pen is fastest at the start of the ring, and a hundred drones fly in
            // with the glass from 1.2 to 3.8
            Part::Greenhouse => whip(x - 5.0).clamp(
                match x {
                    x if x < 1.4 => 3,
                    x if x < 3.9 => 2,
                    _ => 1,
                },
                6,
            ),
            // the shake of the launch and the climb
            Part::Rocket => whip(x).clamp(if x > rocket::LIFTOFF { 2 } else { 1 }, 6),
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
    match part_at(t) {
        (Part::Rocket, x) => rocket::final_white(x),
        _ => 0.0,
    }
}

/// The turn of the whip pan to the left at `x` seconds from the cut, in radians. It goes
/// from `-WHIP_HALF` long before the cut to `WHIP_HALF` long after it, fastest at the cut.
fn whip(x: f32) -> f32 {
    WHIP_HALF * (WHIP_RATE * x).tanh()
}

/// The samples along each view ray of the haze at `x` seconds from the cut of the whip pan.
/// The pan smears the haze, so its fastest frames take fewer, and the motion blur can take
/// more samples in the same time.
fn haze_steps(x: f32) -> u32 {
    if x.abs() < 0.12 { 12 } else { 32 }
}

/// The spark that leads the whip pan: a hot head at `at(t)`, a warm glow of `glow`
/// strength, and a trail of light that fades over the last 0.07 s of its path.
fn spark(at: impl Fn(f32) -> Vec3, t: f32, glow: f32) -> Vec<Particle> {
    let head = at(t);
    let mut items = vec![
        Particle::new(head, 0.25, Rgb::hex(0xfff0d8) * 60.0),
        Particle::new(head, 0.6 + 1.1 * glow, Rgb::hex(AMBER) * (2.0 + 4.0 * glow)),
    ];
    let segments = 6;
    for k in 0..segments {
        let fade = 1.0 - k as f32 / segments as f32;
        let (a, b) = (at(t - k as f32 * 0.012), at(t - (k + 1) as f32 * 0.012));
        items.push(
            Particle::new(
                a,
                0.18 * fade + 0.05,
                Rgb::hex(AMBER) * (30.0 * fade * fade),
            )
            .tail(b - a),
        );
    }
    items
}

/// The point `distance` meters from `eye` in the direction `forward`, turned by `yaw` to
/// the left and raised by `pitch` above the horizontal plane, in radians.
fn polar(eye: Vec3, forward: Vec3, yaw: f32, pitch: f32, distance: f32) -> Vec3 {
    let flat = (Quat::from_rotation_y(yaw) * forward.with_y(0.0)).normalize_or(Vec3::NEG_Z);
    eye + (flat * pitch.cos() + Vec3::Y * pitch.sin()) * distance
}

/// The turn to the left from `forward` to `to`, and the pitch of `to` above the horizontal
/// plane, in radians. It is the inverse of [`polar`] for directions.
fn yaw_pitch(forward: Vec3, to: Vec3) -> (f32, f32) {
    let (f, d) = (forward.with_y(0.0), to.with_y(0.0));
    let yaw = f.cross(d).y.atan2(f.dot(d));
    (yaw, to.y.atan2(d.length()))
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

/// A point in the unit ball for the seed `i`, denser at the middle.
fn random_in_ball(i: u32) -> Vec3 {
    math::random_direction(i) * hash(i + 1).cbrt()
}

/// The stars of the night sky on a sphere 4500 m around `center`: few bright ones and many
/// faint ones, in the colors of their temperatures. `light` scales them, so they can fade
/// at dawn.
fn stars(center: Vec3, light: f32) -> Vec<Particle> {
    let colors = [0xa8b8ff, 0xd8e0ff, 0xfff8f0, 0xffe0c0, 0xffc890];
    (0..5000u32)
        .filter_map(|i| {
            let dir = math::random_direction(i * 3 + 77);
            if dir.y < 0.02 {
                return None;
            }
            let bright = hash(i * 3 + 78).powi(6);
            let color = Rgb::hex(colors[(hash(i * 3 + 79) * 5.0) as usize % 5]);
            Some(Particle::new(
                center + dir * 4500.0,
                2.0 + 1.5 * bright,
                color * (light * (0.6 + 9.0 * bright)),
            ))
        })
        .collect()
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
