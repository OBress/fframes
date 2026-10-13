//! The autonomous traffic on the avenues and the traffic lights that change for nobody.

use std::collections::HashMap;

use fframes_3d::math::hash;
use fframes_3d::{Mat4, Particle, Quat, Rgb, Vec2, Vec3, vec2, vec3};

use super::data::{Class, Street};

/// The speed of all cars in meters per second. The machines drive in perfect platoons.
const SPEED: f32 = 15.0;
/// The distance between two cars of a platoon, in meters.
const GAP: f32 = 15.0;
/// The cars in one platoon.
const PLATOON: u32 = 6;
/// The length of the repeating pattern of a lane: one platoon and the space after it.
const PERIOD: f32 = 330.0;
/// The width of one lane in meters.
const LANE: f32 = 3.2;
/// The seconds between two platoons, and the cycle of the traffic lights.
const CYCLE: f32 = PERIOD / SPEED;

/// A polyline with the distance along it at each point.
pub struct Path {
    points: Vec<Vec2>,
    along: Vec<f32>,
}

impl Path {
    pub fn new(points: Vec<Vec2>) -> Self {
        let mut along = Vec::with_capacity(points.len());
        let mut s = 0.0;
        for (i, p) in points.iter().enumerate() {
            if i > 0 {
                s += p.distance(points[i - 1]);
            }
            along.push(s);
        }
        Self { points, along }
    }

    pub fn length(&self) -> f32 {
        self.along.last().copied().unwrap_or(0.0)
    }

    /// The point and the unit direction at the distance `s` along the path.
    pub fn at(&self, s: f32) -> (Vec2, Vec2) {
        let i = self
            .along
            .partition_point(|&a| a <= s)
            .clamp(1, self.points.len().max(2) - 1);
        let (a, b) = (self.points[i - 1], self.points[i]);
        let span = (self.along[i] - self.along[i - 1]).max(1e-3);
        let k = ((s - self.along[i - 1]) / span).clamp(0.0, 1.0);
        (a.lerp(b, k), (b - a).normalize_or(Vec2::Y))
    }

    /// The distance along the path of the point of the path nearest to `p`.
    fn project(&self, p: Vec2) -> (f32, f32) {
        let mut best = (f32::MAX, 0.0);
        for (i, w) in self.points.windows(2).enumerate() {
            let d = w[1] - w[0];
            let k = ((p - w[0]).dot(d) / d.length_squared().max(1e-6)).clamp(0.0, 1.0);
            let dist = p.distance(w[0] + d * k);
            if dist < best.0 {
                best = (dist, self.along[i] + d.length() * k);
            }
        }
        best
    }
}

/// One lane of an avenue. The cars drive along the path.
pub struct Lane {
    path: Path,
    phase: f32,
}

/// The lanes of the avenues near the flight. Broadway stays empty: in Midtown it is a
/// street for walkers. All lanes of an avenue share one phase, so its platoons drive side
/// by side.
pub fn lanes(routes: &[Street], near: impl Fn(Vec2) -> bool) -> Vec<Lane> {
    let mut out = Vec::new();
    for (i, route) in routes.iter().enumerate() {
        if route.class != Class::Avenue || !route.points.iter().any(|&p| near(p)) {
            continue;
        }
        let count = (route.lanes.clamp(2, 5)) as usize;
        let phase = hash(i as u32 * 31 + 7) * PERIOD;
        for l in 0..count {
            // One way avenues use all lanes. On two way avenues the second half goes back.
            let back = !route.one_way && l >= count / 2;
            let offset = (l as f32 + 0.5) * LANE - count as f32 * LANE * 0.5;
            let mut points = offset_polyline(&route.points, offset);
            if back {
                points.reverse();
            }
            out.push(Lane {
                path: Path::new(points),
                phase: phase + if back { PERIOD * 0.5 } else { 0.0 },
            });
        }
    }
    out
}

/// The polyline moved `offset` meters to its right.
fn offset_polyline(points: &[Vec2], offset: f32) -> Vec<Vec2> {
    (0..points.len())
        .map(|i| {
            let prev = points[i.saturating_sub(1)];
            let next = points[(i + 1).min(points.len() - 1)];
            let d = (next - prev).normalize_or(Vec2::Y);
            // The right side of a direction along +Z is -X.
            points[i] + vec2(-d.y, d.x) * offset
        })
        .collect()
}

/// The cars at `t` seconds near the camera: the light particles, and the transforms of
/// the car bodies that are close enough to see. `fade` (`0..1`) dims the lights and grows
/// the bodies out of the street.
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
    let reach = 1800.0 + camera.y;
    let head = Rgb::hex(0xdfeaff) * (3.2 * fade);
    let tail = Rgb::hex(0xff2414) * (2.2 * fade);
    for lane in lanes {
        let length = lane.path.length();
        let shift = (lane.phase + t * SPEED).rem_euclid(PERIOD);
        for m in 0..=(length / PERIOD) as i32 {
            for k in 0..PLATOON {
                let s = shift + m as f32 * PERIOD + k as f32 * GAP;
                if s > length {
                    continue;
                }
                let (p, d) = lane.path.at(s);
                let center = vec3(p.x, 0.0, p.y);
                let to = center - camera;
                if to.length_squared() > reach * reach || to.dot(forward) < -30.0 {
                    continue;
                }
                let dir = vec3(d.x, 0.0, d.y);
                let side = vec3(-d.y, 0.0, d.x);
                let streak = -dir * (SPEED * 0.1);
                for x in [-0.72, 0.72] {
                    let front = center + dir * 2.25 + side * x + Vec3::Y * 0.7;
                    let back = center - dir * 2.25 + side * x + Vec3::Y * 0.8;
                    lights.push(Particle::new(front, 0.2, head).tail(streak));
                    lights.push(Particle::new(back, 0.16, tail).tail(streak));
                }
                if to.length_squared() < 500.0 * 500.0 {
                    bodies.push(Mat4::from_scale_rotation_translation(
                        vec3(1.0, fade.max(0.01), 1.0),
                        Quat::from_rotation_y(d.x.atan2(d.y)),
                        center,
                    ));
                }
            }
        }
    }
    (lights, bodies)
}

/// One traffic light.
#[derive(Debug, Clone, Copy)]
pub struct Signal {
    /// The lit lens.
    pub head: Vec3,
    /// The center of its glow on the street.
    pub pool: Vec2,
    /// Whether it controls the avenue or the cross street.
    pub avenue: bool,
    /// The offset of its cycle in seconds.
    pub phase: f32,
    /// Whether it stands at the key crossing in view.
    pub key: bool,
}

/// The traffic lights at the crossings of the avenues near the flight. Their cycle follows
/// the platoons: green when a platoon comes, red when the avenue is empty. At the key
/// crossing (Broadway and 42nd Street, the south end of Times Square) nobody comes from
/// either side.
pub fn signals(
    streets: &[Street],
    routes: &[Street],
    near: impl Fn(Vec2) -> bool,
    key: Vec2,
) -> Vec<Signal> {
    // The crossings: the ends of three or more segments.
    let mut degree: HashMap<(i32, i32), u32> = HashMap::new();
    let cell = |p: Vec2| ((p.x * 0.5).round() as i32, (p.y * 0.5).round() as i32);
    for s in streets {
        for p in [s.points[0], s.points[s.points.len() - 1]] {
            *degree.entry(cell(p)).or_default() += 1;
        }
    }
    let mut out = Vec::new();
    let mut done = std::collections::HashSet::new();
    for (i, route) in routes.iter().enumerate() {
        if route.class == Class::Highway {
            continue;
        }
        let path = Path::new(route.points.clone());
        let phase = hash(i as u32 * 31 + 7) * PERIOD;
        for &p in &route.points {
            let is_key = p.distance(key) < 3.0;
            if !is_key
                && (degree.get(&cell(p)).copied().unwrap_or(0) < 3
                    || !near(p)
                    || !done.insert(cell(p)))
            {
                continue;
            }
            let (_, s) = path.project(p);
            // The first car of a platoon reaches the crossing at `arrival`. The avenue gets
            // green 1.5 s before it.
            let arrival = (s - phase - (PLATOON - 1) as f32 * GAP) / SPEED;
            let phase = if is_key {
                // Green for the avenue until 6 s of the shot, red from 7.5 s.
                3.0
            } else if route.class == Class::Avenue {
                1.5 - arrival
            } else {
                hash(i as u32 * 13 + out.len() as u32) * CYCLE
            };
            let (_, d) = path.at(s);
            let side = vec2(-d.y, d.x);
            let half = route.width * 0.5 + 2.0;
            for (n, corner) in [d + side, side - d, -d - side, d - side]
                .into_iter()
                .enumerate()
            {
                let at = p + corner * half * 0.7;
                out.push(Signal {
                    head: vec3(at.x, 6.0, at.y),
                    pool: p + corner * half * 0.45,
                    avenue: n % 2 == 0,
                    phase,
                    key: is_key,
                });
            }
        }
    }
    out
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
