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

    /// The distance from `p` to the path, and the distance along the path of the point of
    /// the path nearest to `p`.
    pub fn project(&self, p: Vec2) -> (f32, f32) {
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
    /// For a lane through Times Square, where the cars stop at the red lights: the
    /// distance along the path of each car, every [`STEP`] seconds from [`SIM_START`].
    /// Empty for the other lanes, where the platoons always get green.
    sim: Vec<Vec<f32>>,
}

/// The time step of the traffic simulation in seconds.
const STEP: f32 = 1.0 / 30.0;
/// The first time of the recorded simulation, in seconds of the shot.
const SIM_START: f32 = -1.0;
/// The length of a car in meters.
const CAR: f32 = 4.6;

/// The lanes of the avenues near the flight. Broadway stays empty: in Midtown it is a
/// street for walkers. All lanes of an avenue share one phase, so its platoons drive side
/// by side. The cars of the lanes through Times Square stop at its red lights.
pub fn lanes(routes: &[Street], near: impl Fn(Vec2) -> bool, signals: &[Signal]) -> Vec<Lane> {
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
            let path = Path::new(points);
            let phase = phase + if back { PERIOD * 0.5 } else { 0.0 };
            // The stop lines of the lights of Times Square on this lane, 12 m before the
            // crossings.
            let mut stops: Vec<f32> = signals
                .iter()
                .filter(|s| s.key && s.avenue)
                .filter_map(|s| {
                    let (off, along) = path.project(s.crossing);
                    (off < 20.0).then_some(along - 12.0)
                })
                .collect();
            stops.sort_by(f32::total_cmp);
            stops.dedup_by(|a, b| (*a - *b).abs() < 5.0);
            let sim = if stops.is_empty() {
                Vec::new()
            } else {
                simulate(&path, phase, &stops)
            };
            out.push(Lane { path, phase, sim });
        }
    }
    out
}

/// The cars of a lane through Times Square, from the platoons 20 s before the shot. Each
/// car keeps a safe distance to the car ahead (Gipps' model with the short reaction time of
/// a machine) and stops at the stop lines while the light of the avenue is amber or red.
fn simulate(path: &Path, phase: f32, stops: &[f32]) -> Vec<Vec<f32>> {
    let start = -20.0;
    let end = 14.0;
    let length = path.length();
    // The platoons at the start, also on the part of the pattern before the lane, from
    // where cars drive in during the simulation.
    let before = SPEED * (end - start) + PERIOD;
    let shift = (phase + start * SPEED).rem_euclid(PERIOD);
    let mut cars: Vec<(f32, f32)> = Vec::new();
    let mut m = -((before / PERIOD).ceil() as i32);
    while (m as f32) * PERIOD + shift <= length {
        for k in 0..PLATOON {
            cars.push((shift + m as f32 * PERIOD + k as f32 * GAP, SPEED));
        }
        m += 1;
    }
    // The leader first.
    cars.sort_by(|a, b| b.0.total_cmp(&a.0));
    let mut out = vec![Vec::new(); cars.len()];
    let (brake, reaction) = (6.0, 0.3);
    let mut t = start;
    while t <= end + STEP {
        if t >= SIM_START - STEP * 0.5 {
            for (o, c) in out.iter_mut().zip(&cars) {
                o.push(c.0);
            }
        }
        let red = avenue_stops(t);
        for i in 0..cars.len() {
            let (s, v) = cars[i];
            // The obstacles: the car ahead, and the next stop line when the light says stop
            // and the car can still stop before it.
            let mut gap = f32::MAX;
            let mut lead = SPEED;
            if i > 0 {
                gap = cars[i - 1].0 - s - CAR - 2.0;
                lead = cars[i - 1].1;
            }
            if red && let Some(&stop) = stops.iter().find(|&&p| p > s) {
                let room = stop - s;
                if room > v * v / (2.0 * brake) * 0.8 && room - 1.0 < gap {
                    gap = room - 1.0;
                    lead = 0.0;
                }
            }
            let safe = -brake * reaction
                + (brake * brake * reaction * reaction
                    + brake * (2.0 * gap.max(0.0) + lead * lead / brake))
                    .max(0.0)
                    .sqrt();
            let target = safe.min(SPEED);
            let v_new = if target > v {
                (v + 2.5 * STEP).min(target)
            } else {
                (v - 9.0 * STEP).max(target)
            };
            cars[i] = (s + f32::midpoint(v, v_new) * STEP, v_new.max(0.0));
        }
        t += STEP;
    }
    out
}

/// Whether the cars on the avenue must stop at the lights of Times Square at `t`: from
/// amber to the next green. See [`signal_color`].
fn avenue_stops(t: f32) -> bool {
    (SQUARE_AMBER..SQUARE_GREEN).contains(&t)
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
    // The positions along the lane and the speeds of its cars at `t`.
    let positions = |lane: &Lane| -> Vec<(f32, f32, f32)> {
        if lane.sim.is_empty() {
            let shift = (lane.phase + t * SPEED).rem_euclid(PERIOD);
            let count = (lane.path.length() / PERIOD) as i32;
            return (0..=count)
                .flat_map(|m| {
                    (0..PLATOON)
                        .map(move |k| (shift + m as f32 * PERIOD + k as f32 * GAP, SPEED, 0.0))
                })
                .collect();
        }
        let k = ((t - SIM_START) / STEP).max(0.0);
        let i = k as usize;
        let f = k - i as f32;
        lane.sim
            .iter()
            .map(|car| {
                let at = |j: usize| car.get(j.min(car.len() - 1)).copied().unwrap_or(0.0);
                let s = at(i) + (at(i + 1) - at(i)) * f;
                let v = (at(i + 1) - at(i)) / STEP;
                let v_before = (at(i) - at(i.saturating_sub(1))) / STEP;
                (s, v, (v - v_before) / STEP)
            })
            .collect()
    };
    for lane in lanes {
        let length = lane.path.length();
        for (s, v, a) in positions(lane) {
            if !(0.0..=length).contains(&s) {
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
            // The light trails: the path of the last quarter second.
            let streak = -dir * (v * 0.25);
            // The brake lights.
            let stop = if a < -1.0 || v < 0.5 { 2.6 } else { 1.0 };
            for x in [-0.72, 0.72] {
                let front = center + dir * 2.25 + side * x + Vec3::Y * 0.7;
                let back = center - dir * 2.25 + side * x + Vec3::Y * 0.8;
                lights.push(Particle::new(front, 0.2, head).tail(streak));
                lights.push(Particle::new(back, 0.16, tail * stop).tail(streak));
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
    (lights, bodies)
}

/// One traffic light.
#[derive(Debug, Clone, Copy)]
pub struct Signal {
    /// The crossing.
    pub crossing: Vec2,
    /// The lit lens.
    pub head: Vec3,
    /// The center of its glow on the street.
    pub pool: Vec2,
    /// Whether it controls the avenue or the cross street.
    pub avenue: bool,
    /// The offset of its cycle in seconds.
    pub phase: f32,
    /// Whether it stands in Times Square, where all lights change together for the camera.
    pub key: bool,
}

/// The lights of Times Square: the avenue gets amber, the avenue gets red, the cross
/// streets get green, amber, red, and the avenue gets green again, in seconds of the shot.
pub const SQUARE_AMBER: f32 = 7.6;
pub const SQUARE_RED: f32 = 8.6;
pub const STREET_GREEN: f32 = 9.0;
pub const STREET_AMBER: f32 = 10.4;
pub const STREET_RED: f32 = 10.9;
pub const SQUARE_GREEN: f32 = 11.2;

/// The traffic lights at the crossings of the avenues near the flight. Their cycle follows
/// the platoons: green when a platoon comes, red when the avenue is empty. In Times Square
/// (`square`) all lights change together, and nobody comes from the cross streets.
pub fn signals(
    streets: &[Street],
    routes: &[Street],
    near: impl Fn(Vec2) -> bool,
    square: (Vec2, Vec2),
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
            let is_key = p.cmpge(square.0).all() && p.cmple(square.1).all();
            if degree.get(&cell(p)).copied().unwrap_or(0) < 3
                || !(near(p) || is_key)
                || !done.insert(cell(p))
            {
                continue;
            }
            let (_, s) = path.project(p);
            // The first car of a platoon reaches the crossing at `arrival`. The avenue gets
            // green 1.5 s before it.
            let arrival = (s - phase - (PLATOON - 1) as f32 * GAP) / SPEED;
            let phase = if is_key {
                0.0
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
                    crossing: p,
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
    if signal.key {
        let (go, wait) = if signal.avenue {
            (!(SQUARE_AMBER..SQUARE_GREEN).contains(&t), t < SQUARE_RED)
        } else {
            (
                (STREET_GREEN..STREET_AMBER).contains(&t),
                (STREET_AMBER..STREET_RED).contains(&t),
            )
        };
        return if go {
            green
        } else if wait {
            amber
        } else {
            red
        };
    }
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
