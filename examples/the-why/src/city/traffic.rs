//! The autonomous traffic on the arterials and the traffic lights at the empty crossings.

use fframes_3d::math::hash;
use fframes_3d::{Mat4, Particle, Quat, Rgb, Vec2, Vec3, vec3};

use super::build::Signal;
use super::layout::Plan;

/// The speed of all cars in meters per second. The machines drive in perfect platoons.
const SPEED: f32 = 15.0;
/// The distance between two cars of a platoon, in meters.
const GAP: f32 = 15.0;
/// The cars in one platoon.
const PLATOON: u32 = 6;
/// The length of the repeating pattern of a lane: one platoon and the space after it.
const PERIOD: f32 = 330.0;
/// The width of one lane in meters.
pub const LANE: f32 = 3.4;
/// The seconds between two platoons, and the cycle of the traffic lights.
pub const CYCLE: f32 = PERIOD / SPEED;
/// The second at which the first car of a platoon reaches the key crossing of the
/// boulevard. The lights there turn green for the boulevard 1.5 s before.
const KEY_ARRIVAL: f32 = -1.5;

/// One lane of an arterial. The cars drive from `from` along `dir`.
pub struct Lane {
    from: Vec2,
    dir: Vec2,
    length: f32,
    phase: f32,
}

/// The lanes of all arterials. The traffic keeps to the right. The platoons of the lanes
/// through the key crossing reach it together, when their light is green.
pub fn lanes(plan: &Plan) -> Vec<Lane> {
    let key = plan.key_crossing;
    let mut out = Vec::new();
    for (i, s) in plan.streets.iter().enumerate() {
        // Only the arterials along Z carry cars. They never cross each other, like the
        // lines of a data bus.
        let dir = s.direction();
        if !s.arterial || dir.y.abs() < 0.99 {
            continue;
        }
        // The right side of a car that drives along `dir`.
        let right = Vec2::new(-dir.y, dir.x);
        let count = ((s.width * 0.5 - s.median * 0.5 - 1.0) / LANE).floor() as i32;
        let length = s.from.distance(s.to);
        let through_key = (key.x - s.from.x).abs() < 1.0;
        for l in 0..count {
            let offset = s.median * 0.5 + 0.5 + LANE * (l as f32 + 0.5);
            let seed = i as u32 * 101 + l as u32 * 7;
            for (from, dir, seed) in [
                (s.from + right * offset, dir, seed),
                (s.to - right * offset, -dir, seed + 3),
            ] {
                // The distance to the near edge of the key crossing.
                let to_key = (key - from).dot(dir) - 12.0;
                let phase = if through_key {
                    // The first car of a platoon is at the crossing at `KEY_ARRIVAL`.
                    (to_key - (PLATOON - 1) as f32 * GAP - SPEED * KEY_ARRIVAL + 3.0 * l as f32)
                        .rem_euclid(PERIOD)
                } else {
                    hash(seed) * PERIOD
                };
                out.push(Lane {
                    from,
                    dir,
                    length,
                    phase,
                });
            }
        }
    }
    out
}

/// The cars at `t` seconds near the camera: the light particles, and the transforms of
/// the car bodies that are close enough to see. `fade` (`0..1`) dims the lights and
/// grows the bodies out of the street.
pub fn cars(
    lanes: &[Lane],
    t: f32,
    camera: Vec3,
    forward: Vec3,
    fade: f32,
) -> (Vec<Particle>, Vec<Mat4>) {
    let mut lights = Vec::new();
    let mut bodies = Vec::new();
    if fade <= 0.0 {
        return (lights, bodies);
    }
    let reach = 1700.0 + camera.y;
    let ground = Vec2::new(camera.x, camera.z);
    let head = Rgb::hex(0xdfeaff) * (3.2 * fade);
    let tail = Rgb::hex(0xff2414) * (2.2 * fade);
    for lane in lanes {
        let along = (ground - lane.from).dot(lane.dir);
        let off = (ground - lane.from - lane.dir * along).length();
        if off > reach {
            continue;
        }
        let chord = (reach * reach - off * off).sqrt();
        let (start, end) = ((along - chord).max(0.0), (along + chord).min(lane.length));
        if start >= end {
            continue;
        }
        let shift = (lane.phase + t * SPEED).rem_euclid(PERIOD);
        let first = ((start - shift) / PERIOD).floor() as i32;
        let last = ((end - shift) / PERIOD).ceil() as i32;
        let dir = vec3(lane.dir.x, 0.0, lane.dir.y);
        let side = vec3(-lane.dir.y, 0.0, lane.dir.x);
        let streak = -dir * (SPEED * 0.1);
        for m in first..=last {
            for k in 0..PLATOON {
                let s = shift + m as f32 * PERIOD + k as f32 * GAP;
                if s < start || s > end {
                    continue;
                }
                let p = lane.from + lane.dir * s;
                let center = vec3(p.x, 0.0, p.y);
                let to = center - camera;
                if to.dot(forward) < -30.0 {
                    continue;
                }
                for x in [-0.72, 0.72] {
                    let front = center + dir * 2.25 + side * x + Vec3::Y * 0.7;
                    let back = center - dir * 2.25 + side * x + Vec3::Y * 0.8;
                    lights.push(Particle::new(front, 0.2, head).tail(streak));
                    lights.push(Particle::new(back, 0.16, tail).tail(streak));
                }
                if to.length_squared() < 500.0 * 500.0 {
                    let angle = lane.dir.x.atan2(lane.dir.y);
                    // The cars grow out of the street as the traffic comes on.
                    bodies.push(Mat4::from_scale_rotation_translation(
                        vec3(1.0, fade.max(0.01), 1.0),
                        Quat::from_rotation_y(angle),
                        center,
                    ));
                }
            }
        }
    }
    (lights, bodies)
}

/// The color of a traffic light at `t` seconds. The avenue and the street get green in
/// turn, with a short time of red on both. The cycle is as long as the time between two
/// platoons.
pub fn signal_color(signal: &Signal, t: f32) -> Rgb {
    let c = (t + signal.phase).rem_euclid(CYCLE);
    let green = Rgb::hex(0x2dffb4);
    let amber = Rgb::hex(0xffc23a);
    let red = Rgb::hex(0xff1e1e);
    if signal.avenue {
        match c {
            c if c < 9.0 => green,
            c if c < 10.5 => amber,
            _ => red,
        }
    } else {
        match c {
            c if c < 11.0 => red,
            c if c < 19.5 => green,
            c if c < 21.0 => amber,
            _ => red,
        }
    }
}

/// The offset of the cycle of the lights at the key crossing: green for the boulevard
/// 1.5 s before the platoons come.
pub fn key_phase() -> f32 {
    1.5 - KEY_ARRIVAL
}
