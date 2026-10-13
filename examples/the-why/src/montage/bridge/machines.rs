//! The machines that build the Golden Gate in the time-lapse: creeper derricks and
//! scaffolds on the towers, lifting gantries on the main cables and robotic barges on the
//! water, with their work lights, welding arcs and sparks. The frames pose them in the
//! space of the bridge.

use std::f32::consts::TAU;

use fframes_3d::math::{hash, random_direction};
use fframes_3d::*;

use super::super::golden_gate::{CABLE_Z, ORANGE, SEGMENT};
use super::super::{COLD, COLD_BLUE, mix, span};

/// The yellow paint of the cranes and the gantries.
const CRANE: u32 = 0xd8a62c;
/// Galvanized steel of the scaffolds.
const GALVANIZED: u32 = 0x9a9c9e;
/// The dark steel of the platforms.
const DARK: u32 = 0x34363a;
/// The red light on the top of a crane.
const RED: u32 = 0xff2010;
/// The orange-white of hot metal.
const HOT: u32 = 0xffb060;

/// The machines of one frame: their steel as instances of a unit box that stands on
/// Y = 0..1 (see [`span`]), each with its paint, and their lights as additive particles.
pub struct Work {
    pub steel: Vec<Mat4>,
    pub paint: Vec<Rgb>,
    pub glows: Vec<Particle>,
    /// The camera in the space of the bridge.
    eye: Vec3,
    /// The size of one pixel 1 m in front of the camera, in meters.
    pixel: f32,
}

impl Work {
    /// An empty frame for a camera at `eye` with pixels of `pixel` meters at 1 m.
    pub fn new(eye: Vec3, pixel: f32) -> Self {
        Self {
            steel: Vec::new(),
            paint: Vec::new(),
            glows: Vec::new(),
            eye,
            pixel,
        }
    }

    /// The size of one pixel at `p`, in meters.
    pub fn pixel_at(&self, p: Vec3) -> f32 {
        self.eye.distance(p) * self.pixel
    }

    /// A member from `a` to `b` with the `thickness` in meters. It is at least half a pixel
    /// thick, so it does not flicker far away.
    pub fn member(&mut self, a: Vec3, b: Vec3, thickness: f32, paint: u32) {
        let thickness = thickness.max(0.5 * self.pixel_at((a + b) * 0.5));
        self.steel.push(span(a, b, thickness));
        self.paint.push(Rgb::hex(paint));
    }

    /// A box with the `size` and its bottom center at `bottom`.
    pub fn block(&mut self, bottom: Vec3, size: Vec3, paint: u32) {
        self.steel.push(Mat4::from_scale_rotation_translation(
            size,
            Quat::IDENTITY,
            bottom,
        ));
        self.paint.push(Rgb::hex(paint));
    }

    /// A light of `size` meters, at least `pixels` pixels wide.
    pub fn glow(&mut self, p: Vec3, size: f32, pixels: f32, color: Rgb) {
        let size = size.max(pixels * self.pixel_at(p));
        self.glows.push(Particle::new(p, size, color));
    }

    /// A lattice boom from `a` to `b`: four chords, a square frame at each bay and a
    /// diagonal on each face. `side` turns the faces around the axis. The width goes from
    /// `width.0` at `a` to `width.1` at `b`.
    pub fn lattice(
        &mut self,
        a: Vec3,
        b: Vec3,
        side: Vec3,
        width: (f32, f32),
        bays: usize,
        paint: u32,
    ) {
        let d = (b - a).normalize_or(Vec3::Y);
        let u = (side - d * side.dot(d)).normalize_or(d.any_orthonormal_vector());
        let v = d.cross(u);
        let corners = [(1.0, 1.0), (-1.0, 1.0), (-1.0, -1.0), (1.0, -1.0)];
        let corner = |s: f32, k: usize| {
            let w = mix(width.0, width.1, s) * 0.5;
            let (cu, cv) = corners[k % 4];
            a.lerp(b, s) + (u * cu + v * cv) * w
        };
        let chord = width.0 * 0.1;
        for k in 0..4 {
            self.member(corner(0.0, k), corner(1.0, k), chord, paint);
        }
        for i in 0..bays {
            let (s0, s1) = (i as f32 / bays as f32, (i + 1) as f32 / bays as f32);
            for k in 0..4 {
                self.member(corner(s1, k), corner(s1, k + 1), chord * 0.6, paint);
                self.member(corner(s0, k), corner(s1, k + 1), chord * 0.5, paint);
            }
        }
    }

    /// Sparks that fall from `p`: `count` streaks with a life of `life` seconds, thrown
    /// out at `speed` m/s. The gravity is that of the time-lapse, so they fall fast.
    pub fn sparks(&mut self, p: Vec3, t: f32, seed: u32, count: u32, speed: f32, life: f32) {
        let size = (0.8 * self.pixel_at(p)).max(0.15);
        for i in 0..count {
            let s = seed.wrapping_mul(131) + i * 7;
            let age = ((t / life + hash(s)).fract()) * life;
            let mut dir = random_direction(s + 3);
            dir.y = dir.y.abs() * 0.4;
            let velocity = dir * speed * (0.5 + 0.5 * hash(s + 5));
            let gravity = 180.0;
            let at = p + velocity * age - Vec3::Y * (0.5 * gravity * age * age);
            let now = velocity - Vec3::Y * (gravity * age);
            let fade = 1.0 - age / life;
            self.glows
                .push(Particle::new(at, size, Rgb::hex(HOT) * (80.0 * fade)).tail(-now * 0.02));
        }
    }

    /// An arc of a welder at `p`: a hot blue-white point that flickers each frame, and
    /// sparks that fall from it. `age` is the time since it started, `frame` the number of
    /// the frame.
    pub fn weld(&mut self, p: Vec3, age: f32, seed: u32, frame: u32) {
        let flicker = 0.4 + 0.6 * hash(seed.wrapping_mul(977) + frame);
        self.glow(p, 0.5, 0.7, Rgb::hex(0xe8f0ff) * (60.0 * flicker));
        self.glow(p, 2.5, 1.5, Rgb::hex(COLD_BLUE) * (1.5 * flicker));
        self.sparks(p, age, seed, 16, 12.0, 0.4);
    }
}

/// A creeper derrick on the top of a tower leg, as the real towers had in 1935: a
/// platform that grips the leg, a mast, and a boom that slews and luffs. `top` is the top
/// of the leg, `leg` its size, `out` the side of the water (`-1` or `1` along Z). `slew`
/// turns the boom away from the water, `luff` raises it (radians). `hook` is the length of
/// the hoist line, and `load` hangs a piece of the tower on it. `t` makes the lights blink.
#[allow(clippy::too_many_arguments)]
pub fn creeper(
    work: &mut Work,
    top: Vec3,
    leg: Vec2,
    out: f32,
    slew: f32,
    luff: f32,
    hook: f32,
    load: bool,
    t: f32,
) {
    let (hx, hz) = (leg.x * 0.5 + 1.0, leg.y * 0.5 + 1.0);
    // the platform: two frames around the leg and the posts between them
    let ring = [(1.0, 1.0), (-1.0, 1.0), (-1.0, -1.0), (1.0, -1.0)]
        .map(|(x, z)| top + vec3(x * hx, 0.0, z * hz));
    for k in 0..4 {
        let (a, b) = (ring[k], ring[(k + 1) % 4]);
        work.member(a, b, 0.5, DARK);
        work.member(a - Vec3::Y * 3.5, b - Vec3::Y * 3.5, 0.4, DARK);
        work.member(a - Vec3::Y * 3.5, a, 0.4, DARK);
    }
    work.block(top, vec3(2.0 * hx, 0.3, 2.0 * hz), DARK);
    // the mast on the side of the water, and its back stays
    let foot = top + vec3(0.0, 0.3, out * (hz - 0.9));
    let head = foot + Vec3::Y * 15.0;
    work.lattice(foot, head, Vec3::X, (1.4, 1.0), 5, CRANE);
    for x in [-hx, hx] {
        work.member(head, top + vec3(x, 0.3, -out * hz), 0.12, DARK);
    }
    // the boom
    let base = foot + Vec3::Y * 1.5;
    let level = vec3(slew.sin(), 0.0, out * slew.cos());
    let tip = base + (level * luff.cos() + Vec3::Y * luff.sin()) * 38.0;
    work.lattice(base, tip, Vec3::Y, (1.6, 0.6), 8, CRANE);
    let across = level.cross(Vec3::Y) * 0.4;
    for s in [-1.0, 1.0] {
        work.member(head + across * s, tip, 0.08, DARK);
    }
    // the hoist line, the hook block and the load
    let hook_at = tip - Vec3::Y * hook;
    work.member(hook_at, tip, 0.07, DARK);
    work.block(hook_at - Vec3::Y * 0.8, Vec3::splat(0.8), DARK);
    if load {
        work.member(
            hook_at - Vec3::Y * 7.8,
            hook_at - Vec3::Y * 1.0,
            3.2,
            ORANGE,
        );
    }
    // the lights: a work light on the boom tip, two floods on the platform, and the red
    // light on the mast
    work.glow(tip, 0.6, 1.5, Rgb::hex(COLD) * 30.0);
    for x in [-hx, hx] {
        work.glow(
            top + vec3(x, 1.2, out * hz),
            0.7,
            1.8,
            Rgb::hex(COLD) * 35.0,
        );
    }
    let blink = if (t * 1.3 + out).fract() < 0.5 {
        40.0
    } else {
        6.0
    };
    work.glow(head + Vec3::Y * 0.8, 0.5, 1.4, Rgb::hex(RED) * blink);
}

/// A scaffold around a tower leg below its `top`: posts, a deck at each of the `levels`
/// lifts of 3 m, braces on each face, and work lamps on the top deck.
pub fn scaffold(work: &mut Work, top: Vec3, leg: Vec2, levels: usize) {
    if levels == 0 {
        return;
    }
    let (hx, hz) = (leg.x * 0.5 + 1.0, leg.y * 0.5 + 1.0);
    let corners = [(1.0, 1.0), (-1.0, 1.0), (-1.0, -1.0), (1.0, -1.0)];
    let at = |k: usize, y: f32| {
        let (x, z) = corners[k % 4];
        top + vec3(x * hx, y, z * hz)
    };
    let bottom = -3.0 * levels as f32;
    for k in 0..4 {
        work.member(at(k, bottom), at(k, 1.1), 0.18, GALVANIZED);
        let middle = (at(k, bottom) + at(k + 1, bottom)) * 0.5;
        work.member(middle, middle + Vec3::Y * (1.1 - bottom), 0.15, GALVANIZED);
        for i in 0..levels {
            let y = -3.0 * i as f32;
            work.member(at(k, y), at(k + 1, y), 0.15, GALVANIZED);
            work.member(at(k, y - 3.0), at(k + 1, y), 0.1, GALVANIZED);
        }
        work.member(at(k, bottom), at(k + 1, bottom), 0.15, GALVANIZED);
        work.member(at(k, 1.1), at(k + 1, 1.1), 0.1, GALVANIZED);
    }
    for k in 0..4 {
        let p = at(k, 1.4);
        work.glow(p, 0.35, 1.0, Rgb::hex(COLD) * 25.0);
    }
}

/// A lifting gantry that stands on the two main cables at `cables` and holds a deck
/// segment on four strands. `hang` is the height of the top of the segment. `t` makes its
/// red light blink.
pub fn gantry(work: &mut Work, cables: [Vec3; 2], hang: f32, t: f32) {
    let mut tops = [Vec3::ZERO; 2];
    for (i, c) in cables.into_iter().enumerate() {
        let top = c + Vec3::Y * 5.0;
        for dx in [-3.5, 3.5] {
            for dz in [-1.2, 1.2] {
                work.member(c + vec3(dx, 0.4, dz), top, 0.35, CRANE);
            }
        }
        // the carriage with its wheels on the cable
        work.block(c + vec3(0.0, 0.1, 0.0), vec3(8.0, 0.8, 1.6), DARK);
        tops[i] = top;
    }
    work.lattice(tops[0], tops[1], Vec3::Y, (1.8, 1.8), 6, CRANE);
    // the strands down to the corners of the segment
    let x = tops[0].x;
    for dx in [-SEGMENT * 0.35, SEGMENT * 0.35] {
        for z in [-CABLE_Z + 2.5, CABLE_Z - 2.5] {
            let at = tops[0].lerp(tops[1], (z + CABLE_Z) / (2.0 * CABLE_Z));
            let from = vec3(x + dx, at.y, z);
            work.member(from, vec3(x + dx, hang, z), 0.1, DARK);
        }
    }
    for p in tops {
        work.glow(p + Vec3::Y * 1.2, 0.6, 1.5, Rgb::hex(COLD) * 30.0);
    }
    if (t * 1.7 + x * 0.01).fract() < 0.5 {
        let middle = (tops[0] + tops[1]) * 0.5 + Vec3::Y * 1.4;
        work.glow(middle, 0.4, 1.2, Rgb::hex(RED) * 30.0);
    }
}

/// A robotic barge with a cradle for a deck segment, a sensor mast and thrusters, 28 m
/// long, so that barges under two segments side by side do not touch. Its deck is at
/// Y = 2.5. Returns the steel and the LED strips along its deck,
/// which glow in the fog.
pub fn barge() -> (Geometry, Geometry) {
    let hull = Geometry::merge([
        Geometry::rounded_box(28.0, 3.6, 22.0, 1.2, 2).translate(0.0, 0.7, 0.0),
        // the cradle that holds the segment
        Geometry::cuboid(1.4, 1.2, 20.0).translate(-7.0, 3.1, 0.0),
        Geometry::cuboid(1.4, 1.2, 20.0).translate(7.0, 3.1, 0.0),
        // the sensor mast with its radar, and the housings of the thrusters
        Geometry::cylinder(0.25, 0.35, 9.0, 6).translate(-11.5, 7.0, 9.0),
        Geometry::cylinder(1.2, 1.2, 0.5, 12).translate(-11.5, 11.6, 9.0),
        Geometry::cuboid(3.0, 2.4, 3.0).translate(-11.5, 3.7, -8.5),
        Geometry::cuboid(3.0, 2.4, 3.0).translate(11.5, 3.7, 8.5),
    ]);
    let mut strips = Vec::new();
    for z in [-11.05, 11.05] {
        strips.push(Geometry::cuboid(25.0, 0.25, 0.12).translate(0.0, 2.2, z));
    }
    for x in [-14.05, 14.05] {
        strips.push(Geometry::cuboid(0.12, 0.25, 19.0).translate(x, 2.2, 0.0));
    }
    strips.push(Geometry::sphere(0.45, 8, 6).translate(-11.5, 12.2, 9.0));
    (hull, Geometry::merge(strips))
}

/// The lights of a barge at `p`, turned by `turn`: the red and the green side lights, and
/// the blue light of its sensor mast, which pulses.
pub fn barge_lights(work: &mut Work, p: Vec3, turn: Quat, t: f32) {
    let at = |x: f32, y: f32, z: f32| p + turn * vec3(x, y, z);
    work.glow(at(14.5, 3.0, -10.5), 0.4, 1.2, Rgb::hex(RED) * 25.0);
    work.glow(at(14.5, 3.0, 10.5), 0.4, 1.2, Rgb::hex(0x30ff70) * 25.0);
    let pulse = 0.5 + 0.5 * (t * TAU * 1.5).sin();
    work.glow(
        at(-11.5, 12.4, 9.0),
        0.6,
        1.4,
        Rgb::hex(COLD_BLUE) * (30.0 * pulse),
    );
}
