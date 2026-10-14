//! Secondary motion with spring bones, the technique VRM avatars (and most anime CG rigs) use
//! for hair, ties, ribbons and tails: a chain hangs off an animated joint, every link is pulled
//! back to its rest direction by a spring, keeps its momentum, falls with gravity, keeps its
//! length and is pushed out of the body's colliders.
//!
//! Frames of a video render in parallel and in any order, so a simulation is never stepped in
//! `render_frame`. [`bake`] runs it once at a fixed time step over a time range and stores the
//! state at every output frame; [`Baked::at`] reads (and interpolates) it later.

use glam::{Quat, Vec3};

/// A sphere or a capsule (a sphere swept from `a` to `b`) the simulated points stay out of.
#[derive(Debug, Clone, Copy)]
pub struct Collider {
    pub a: Vec3,
    pub b: Vec3,
    pub radius: f32,
}

impl Collider {
    pub fn sphere(center: Vec3, radius: f32) -> Self {
        Collider {
            a: center,
            b: center,
            radius,
        }
    }

    pub fn capsule(a: Vec3, b: Vec3, radius: f32) -> Self {
        Collider { a, b, radius }
    }

    /// Moves `p` out of the collider (`margin` is the point's own radius).
    pub fn push_out(&self, p: Vec3, margin: f32) -> Vec3 {
        let ab = self.b - self.a;
        let t = if ab.length_squared() < 1e-10 {
            0.0
        } else {
            ((p - self.a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0)
        };
        let c = self.a + ab * t;
        let d = p - c;
        let r = self.radius + margin;
        let len = d.length();
        if len < r {
            if len < 1e-6 {
                return c + Vec3::Y * r;
            }
            c + d / len * r
        } else {
            p
        }
    }
}

/// An animated joint the chains and cloth hang from.
#[derive(Debug, Clone, Copy)]
pub struct Anchor {
    pub position: Vec3,
    pub rotation: Quat,
}

impl Anchor {
    pub fn point(&self, local: Vec3) -> Vec3 {
        self.position + self.rotation * local
    }
}

/// What the simulated objects need from the animated body at a moment.
#[derive(Debug, Clone, Default)]
pub struct BodyState {
    pub anchors: Vec<Anchor>,
    pub colliders: Vec<Collider>,
    /// Velocity of the air (m/s). Simulated points are dragged towards it at their damping
    /// rate, so still air (zero) slows them down and a moving body makes its own wind.
    pub wind: Vec3,
    /// Air flowing outwards from the body's vertical axis through the first anchor of each
    /// cloth (m/s): a robe billows open like wings.
    pub billow: f32,
    /// Height of the ground: hair and cloth rest on it instead of sinking through (a robe
    /// spreading on the floor under a push-up).
    pub floor: Option<f32>,
}

/// Keeps `p` above the floor; touching it, the point sticks (friction) instead of sliding.
fn rest_on_floor(p: &mut Vec3, prev: &mut Vec3, margin: f32, floor: Option<f32>) {
    let Some(f) = floor else { return };
    let y = f + margin;
    if p.y < y {
        p.y = y;
        prev.x = prev.x + (p.x - prev.x) * 0.6;
        prev.z = prev.z + (p.z - prev.z) * 0.6;
        prev.y = prev.y.max(y);
    }
}

#[derive(Debug, Clone, Copy)]
pub struct SpringParams {
    /// Pull towards the rest direction in m/s² per radian of bend, independent of the link
    /// length: under gravity a link sags about `9.8 / stiffness` radians. 20 = soft hair,
    /// 300 = gelled.
    pub stiffness: f32,
    /// Air drag: how fast the velocity follows the air's velocity (1/s; 1 is gentle, 6 is syrup).
    pub damping: f32,
    pub gravity: Vec3,
    /// Radius of the links when colliding.
    pub radius: f32,
    /// Damping of each link's motion relative to where its parent wants it (1/s). Without it
    /// links of equal stiffness resonate down the chain and the tip whips out of control.
    pub joint_damping: f32,
}

impl Default for SpringParams {
    fn default() -> Self {
        SpringParams {
            stiffness: 40.0,
            damping: 2.5,
            gravity: Vec3::new(0.0, -9.8, 0.0),
            radius: 0.01,
            joint_damping: 12.0,
        }
    }
}

/// A chain of points hanging from `anchor`. `rest` are the points in the anchor's frame; the
/// first point is attached to the anchor, the others are simulated.
#[derive(Debug, Clone)]
pub struct SpringChain {
    pub anchor: usize,
    pub rest: Vec<Vec3>,
    pub params: SpringParams,
    /// Indices into [`BodyState::colliders`] this chain collides with; `None` = all of them.
    pub colliders: Option<Vec<usize>>,
}

#[derive(Debug, Clone)]
struct ChainState {
    cur: Vec<Vec3>,
    prev: Vec<Vec3>,
    /// Where each link's spring pulled it on the previous step.
    targets: Vec<Option<Vec3>>,
}

/// Particles connected by distance constraints, pinned to anchors: capes, robes, scarves,
/// ribbons, flags. Solved with position based dynamics.
#[derive(Debug, Clone)]
pub struct Cloth {
    /// Rest positions in the frame of the anchor of the first pinned particle.
    pub rest: Vec<Vec3>,
    /// `Some(anchor)` keeps the particle attached to that anchor at its rest offset.
    pub pins: Vec<Option<usize>>,
    /// `(a, b, rest length, tension only)`: tension-only links (shear and bend) resist
    /// stretching but let the cloth fold, like fabric does.
    pub constraints: Vec<(usize, usize, f32, bool)>,
    pub damping: f32,
    pub gravity: Vec3,
    pub radius: f32,
    pub iterations: usize,
    /// Extra drag of the cloth relative to `damping` (it catches more air than hair).
    pub wind_factor: f32,
    pub colliders: Option<Vec<usize>>,
    /// How fast (1/s) the cloth is pulled back to its rest shape around the anchor: anime CG
    /// keeps cloth on model this way, it swings and settles back into the designed drape
    /// instead of sliding open or crumpling. 0 = free cloth.
    pub shape: f32,
}

impl Cloth {
    /// A grid of `rows` x `cols` particles. `point(row, col)` returns the rest position (in the
    /// anchor's frame for pinned rows, world space for the others when `anchor` is `None`).
    /// Rows listed in `pinned_rows` stay attached to `anchor`.
    pub fn grid(
        rows: usize,
        cols: usize,
        anchor: usize,
        pinned_rows: &[usize],
        point: impl Fn(usize, usize) -> Vec3,
    ) -> Self {
        let mut rest = Vec::with_capacity(rows * cols);
        let mut pins = Vec::with_capacity(rows * cols);
        for r in 0..rows {
            for c in 0..cols {
                rest.push(point(r, c));
                pins.push(pinned_rows.contains(&r).then_some(anchor));
            }
        }
        let idx = |r: usize, c: usize| r * cols + c;
        let mut constraints = Vec::new();
        let mut link = |a: usize, b: usize, rest: &[Vec3], tension_only: bool| {
            constraints.push((a, b, (rest[a] - rest[b]).length(), tension_only));
        };
        for r in 0..rows {
            for c in 0..cols {
                if c + 1 < cols {
                    link(idx(r, c), idx(r, c + 1), &rest, false);
                }
                if r + 1 < rows {
                    link(idx(r, c), idx(r + 1, c), &rest, false);
                }
                if r + 1 < rows && c + 1 < cols {
                    link(idx(r, c), idx(r + 1, c + 1), &rest, true);
                    link(idx(r, c + 1), idx(r + 1, c), &rest, true);
                }
                if r + 2 < rows {
                    link(idx(r, c), idx(r + 2, c), &rest, true);
                }
                if c + 2 < cols {
                    link(idx(r, c), idx(r, c + 2), &rest, true);
                }
            }
        }
        Cloth {
            rest,
            pins,
            constraints,
            damping: 1.5,
            gravity: Vec3::new(0.0, -9.8, 0.0),
            radius: 0.02,
            iterations: 8,
            wind_factor: 1.0,
            shape: 0.0,
            colliders: None,
        }
    }
}

#[derive(Debug, Clone)]
struct ClothState {
    cur: Vec<Vec3>,
    prev: Vec<Vec3>,
}

/// Everything simulated for one character or one shot.
#[derive(Debug, Clone, Default)]
pub struct Rig {
    pub chains: Vec<SpringChain>,
    pub cloths: Vec<Cloth>,
}

/// Snapshot of all chains and cloths at one output frame.
#[derive(Debug, Clone)]
pub struct Snapshot {
    pub chains: Vec<Vec<Vec3>>,
    pub cloths: Vec<Vec<Vec3>>,
}

/// The simulation sampled at every output frame.
#[derive(Debug, Clone)]
pub struct Baked {
    pub start: f32,
    pub fps: f32,
    pub frames: Vec<Snapshot>,
}

impl Baked {
    /// State at `t` seconds (same clock as [`bake`]'s `start`), linearly interpolated.
    pub fn at(&self, t: f32) -> Snapshot {
        if self.frames.is_empty() {
            return Snapshot {
                chains: Vec::new(),
                cloths: Vec::new(),
            };
        }
        let f = ((t - self.start) * self.fps).max(0.0);
        let i = (f.floor() as usize).min(self.frames.len() - 1);
        let j = (i + 1).min(self.frames.len() - 1);
        let k = (f - i as f32).clamp(0.0, 1.0);
        if i == j || k < 1e-4 {
            return self.frames[i].clone();
        }
        let lerp = |a: &Vec<Vec<Vec3>>, b: &Vec<Vec<Vec3>>| {
            a.iter()
                .zip(b)
                .map(|(x, y)| x.iter().zip(y).map(|(p, q)| p.lerp(*q, k)).collect())
                .collect()
        };
        Snapshot {
            chains: lerp(&self.frames[i].chains, &self.frames[j].chains),
            cloths: lerp(&self.frames[i].cloths, &self.frames[j].cloths),
        }
    }
}

fn collide(p: Vec3, margin: f32, state: &BodyState, only: &Option<Vec<usize>>) -> Vec3 {
    let mut p = p;
    match only {
        Some(list) => {
            for &i in list {
                if let Some(c) = state.colliders.get(i) {
                    p = c.push_out(p, margin);
                }
            }
        }
        None => {
            for c in &state.colliders {
                p = c.push_out(p, margin);
            }
        }
    }
    p
}

/// Runs the simulation from `start` to `end` seconds with `substeps` steps per output frame and
/// records `fps` snapshots per second. `body(t)` returns the animated anchors and colliders.
/// The rig first settles in the pose at `start - preroll`, then plays the motion from there up
/// to `start`, so at the first recorded frame hair and cloth already move with the body (a
/// character flying into the shot has a streaming robe, not one that starts to react).
pub fn bake(
    rig: &Rig,
    start: f32,
    end: f32,
    fps: f32,
    substeps: usize,
    preroll: f32,
    body: impl Fn(f32) -> BodyState,
) -> Baked {
    let dt = 1.0 / (fps * substeps as f32);
    let initial = body(start - preroll);

    let mut chains: Vec<ChainState> = rig
        .chains
        .iter()
        .map(|chain| {
            let anchor = initial.anchors[chain.anchor];
            let pts: Vec<Vec3> = chain.rest.iter().map(|p| anchor.point(*p)).collect();
            ChainState {
                targets: vec![None; pts.len()],
                prev: pts.clone(),
                cur: pts,
            }
        })
        .collect();
    let mut cloths: Vec<ClothState> = rig
        .cloths
        .iter()
        .map(|cloth| {
            let pts: Vec<Vec3> = cloth
                .rest
                .iter()
                .zip(&cloth.pins)
                .map(|(p, pin)| match pin {
                    Some(a) => initial.anchors[*a].point(*p),
                    None => match cloth.pins.iter().flatten().next() {
                        Some(a) => initial.anchors[*a].point(*p),
                        None => *p,
                    },
                })
                .collect();
            ClothState {
                prev: pts.clone(),
                cur: pts,
            }
        })
        .collect();

    let step =
        |state: &BodyState, chains: &mut Vec<ChainState>, cloths: &mut Vec<ClothState>, dt: f32| {
            for (chain, s) in rig.chains.iter().zip(chains.iter_mut()) {
                let anchor = state.anchors[chain.anchor];
                let damp = (1.0 - chain.params.damping * dt).clamp(0.0, 1.0);
                s.cur[0] = anchor.point(chain.rest[0]);
                s.prev[0] = s.cur[0];
                let mut parent_rot = anchor.rotation;
                for i in 1..chain.rest.len() {
                    let rest_local = chain.rest[i] - chain.rest[i - 1];
                    let len = rest_local.length();
                    let rest_dir = parent_rot * (rest_local / len.max(1e-6));
                    let parent = s.cur[i - 1];
                    let target = parent + rest_dir * len;
                    let cur = s.cur[i];
                    let target_vel = s.targets[i].map_or(Vec3::ZERO, |t| (target - t) / dt);
                    s.targets[i] = Some(target);
                    let vel = (cur - s.prev[i]) / dt;
                    let accel = (target - cur) * (chain.params.stiffness / len.max(1e-3))
                        + chain.params.gravity
                        + (target_vel - vel) * chain.params.joint_damping;
                    let drag = (chain.params.damping * dt).min(1.0);
                    let mut next =
                        cur + (cur - s.prev[i]) * damp + state.wind * drag * dt + accel * dt * dt;
                    next = parent + (next - parent).normalize_or(rest_dir) * len;
                    next = collide(next, chain.params.radius, state, &chain.colliders);
                    next = parent + (next - parent).normalize_or(rest_dir) * len;
                    let mut prev = cur;
                    rest_on_floor(&mut next, &mut prev, chain.params.radius, state.floor);
                    s.prev[i] = prev;
                    s.cur[i] = next;
                    let bent =
                        Quat::from_rotation_arc(rest_dir, (next - parent).normalize_or(rest_dir));
                    parent_rot = bent * parent_rot;
                }
            }
            for (cloth, s) in rig.cloths.iter().zip(cloths.iter_mut()) {
                let rate = (cloth.damping * cloth.wind_factor * dt).min(1.0);
                let damp = 1.0 - rate;
                let accel = cloth.gravity;
                let home = cloth
                    .pins
                    .iter()
                    .flatten()
                    .next()
                    .map(|a| &state.anchors[*a]);
                let axis = home.map_or(Vec3::ZERO, |a| a.position);
                let keep = (cloth.shape * dt).min(1.0);
                for i in 0..s.cur.len() {
                    if let Some(a) = cloth.pins[i] {
                        let p = state.anchors[a].point(cloth.rest[i]);
                        s.prev[i] = s.cur[i];
                        s.cur[i] = p;
                        continue;
                    }
                    let cur = s.cur[i];
                    let out = (cur - axis) * Vec3::new(1.0, 0.0, 1.0);
                    let air = state.wind + out.normalize_or_zero() * state.billow;
                    let mut next =
                        cur + (cur - s.prev[i]) * damp + air * rate * dt + accel * dt * dt;
                    if let (Some(home), true) = (home, keep > 0.0) {
                        next = next.lerp(home.point(cloth.rest[i]), keep);
                    }
                    s.prev[i] = cur;
                    s.cur[i] = next;
                }
                for _ in 0..cloth.iterations {
                    for &(a, b, rest, tension_only) in &cloth.constraints {
                        let wa = if cloth.pins[a].is_some() { 0.0 } else { 1.0 };
                        let wb = if cloth.pins[b].is_some() { 0.0 } else { 1.0 };
                        let w = wa + wb;
                        if w == 0.0 {
                            continue;
                        }
                        let d = s.cur[b] - s.cur[a];
                        let len = d.length();
                        if len < 1e-7 || (tension_only && len <= rest) {
                            continue;
                        }
                        let corr = d * ((len - rest) / (len * w));
                        s.cur[a] += corr * wa;
                        s.cur[b] -= corr * wb;
                    }
                    for i in 0..s.cur.len() {
                        if cloth.pins[i].is_none() {
                            s.cur[i] = collide(s.cur[i], cloth.radius, state, &cloth.colliders);
                            rest_on_floor(&mut s.cur[i], &mut s.prev[i], cloth.radius, state.floor);
                        }
                    }
                }
            }
        };

    // Settle in the pose before the run-up, then play the run-up.
    for _ in 0..(0.5 / dt).ceil() as usize {
        step(&initial, &mut chains, &mut cloths, dt);
    }
    let run_up = (preroll / dt).ceil() as usize;
    for k in 0..run_up {
        let t = start - preroll + (k + 1) as f32 * dt;
        step(&body(t.min(start)), &mut chains, &mut cloths, dt);
    }

    let frame_count = ((end - start) * fps).ceil().max(0.0) as usize + 1;
    let mut frames = Vec::with_capacity(frame_count);
    let snapshot = |chains: &Vec<ChainState>, cloths: &Vec<ClothState>| Snapshot {
        chains: chains.iter().map(|c| c.cur.clone()).collect(),
        cloths: cloths.iter().map(|c| c.cur.clone()).collect(),
    };
    frames.push(snapshot(&chains, &cloths));
    for f in 1..frame_count {
        for s in 0..substeps {
            let t = start + ((f - 1) as f32 + (s + 1) as f32 / substeps as f32) / fps;
            let state = body(t);
            step(&state, &mut chains, &mut cloths, dt);
        }
        frames.push(snapshot(&chains, &cloths));
    }
    Baked { start, fps, frames }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn smoothstep(x: f32) -> f32 {
        x * x * (3.0 - 2.0 * x)
    }

    #[test]
    fn chain_swings_back_when_anchor_moves() {
        let rig = Rig {
            chains: vec![SpringChain {
                anchor: 0,
                rest: vec![
                    Vec3::ZERO,
                    Vec3::new(0.0, -0.1, 0.0),
                    Vec3::new(0.0, -0.2, 0.0),
                ],
                params: SpringParams {
                    gravity: Vec3::ZERO,
                    stiffness: 12.0,
                    ..Default::default()
                },
                colliders: None,
            }],
            cloths: vec![],
        };
        // The anchor slides 30 cm right between 0.5 and 1.0 s: the tip lags behind, then returns
        // under it.
        let baked = bake(&rig, 0.0, 6.0, 30.0, 8, 0.2, |t| BodyState {
            anchors: vec![Anchor {
                position: Vec3::new(
                    0.3 * smoothstep(((t - 0.5) / 0.5).clamp(0.0, 1.0)),
                    0.0,
                    0.0,
                ),
                rotation: Quat::IDENTITY,
            }],
            ..Default::default()
        });
        let anchor_x = 0.3 * smoothstep(0.5);
        let lag = baked.at(0.75).chains[0][2];
        assert!(lag.x < anchor_x - 0.01 && lag.y < -0.1, "{lag:?}");
        let settled = baked.at(6.0).chains[0][2];
        assert!(
            (settled - Vec3::new(0.3, -0.2, 0.0)).length() < 0.02,
            "{settled:?}"
        );
    }

    #[test]
    fn cloth_hangs_and_keeps_length() {
        let cloth = Cloth::grid(6, 4, 0, &[0], |r, c| {
            Vec3::new(c as f32 * 0.1, -(r as f32) * 0.1, 0.0)
        });
        let rig = Rig {
            chains: vec![],
            cloths: vec![cloth],
        };
        let baked = bake(&rig, 0.0, 1.0, 30.0, 8, 0.5, |_| BodyState {
            anchors: vec![Anchor {
                position: Vec3::ZERO,
                rotation: Quat::IDENTITY,
            }],
            ..Default::default()
        });
        let last = &baked.at(1.0).cloths[0];
        let bottom = last[5 * 4];
        assert!((bottom.y + 0.5).abs() < 0.05, "{bottom:?}");
    }
}

#[cfg(test)]
mod hang_tests {
    use super::*;

    #[test]
    fn tilted_hanging_chain_stays_calm() {
        let rest: Vec<Vec3> = (0..6)
            .map(|k| Vec3::new(0.0, -0.07 * k as f32, 0.012 * k as f32))
            .collect();
        let rig = Rig {
            chains: vec![SpringChain {
                anchor: 0,
                rest,
                params: SpringParams {
                    stiffness: 25.0,
                    damping: 1.6,
                    ..Default::default()
                },
                colliders: None,
            }],
            cloths: vec![],
        };
        let baked = bake(&rig, 0.0, 2.0, 24.0, 10, 0.8, |_| BodyState {
            anchors: vec![Anchor {
                position: Vec3::ZERO,
                rotation: Quat::IDENTITY,
            }],
            ..Default::default()
        });
        for t in [0.0, 0.5, 1.0, 2.0] {
            let pts = &baked.at(t).chains[0];
            let tip = pts[5];
            assert!(tip.y < -0.33, "{tip:?}");
        }
    }
}

#[cfg(test)]
mod tie_tests {
    use super::*;

    /// A tie hanging from a chest that turns in steps (drawings on twos) settles instead of
    /// whipping about: its bending must be critically damped (the tie's parameters).
    #[test]
    fn stepped_turn_does_not_whip_a_hanging_chain() {
        let top = Vec3::new(0.0, 0.232, 0.096);
        let rig = Rig {
            chains: vec![SpringChain {
                anchor: 0,
                rest: (0..6)
                    .map(|i| top + Vec3::new(0.0, -0.07 * i as f32, 0.003 * i as f32))
                    .collect(),
                params: SpringParams {
                    stiffness: 120.0,
                    damping: 5.0,
                    gravity: Vec3::new(0.0, -9.8, 0.0),
                    radius: 0.01,
                    joint_damping: 75.0,
                },
                colliders: Some(vec![]),
            }],
            cloths: vec![],
        };
        let held = |t: f32| (t * 12.0).floor() / 12.0;
        let yaw = |t: f32| {
            let t = held(t) - 0.3;
            if t <= 0.0 {
                -0.18
            } else {
                -0.18 * (-6.0 * t).exp() * (1.2 * std::f32::consts::TAU * t).cos()
            }
        };
        let baked = bake(&rig, 0.0, 1.5, 24.0, 10, 0.8, |t| BodyState {
            anchors: vec![Anchor {
                position: Vec3::new(0.0, 1.27, 0.0),
                rotation: Quat::from_rotation_y(yaw(t)),
            }],
            ..Default::default()
        });
        let mut worst = 0.0_f32;
        for f in 0..36 {
            let t = f as f32 / 24.0;
            let snap = baked.at(t);
            let tip = snap.chains[0][5];
            let rot = Quat::from_rotation_y(yaw(t));
            let local = rot.inverse() * (tip - Vec3::new(0.0, 1.27, 0.0));
            worst = worst.max(local.x.abs()).max((local.y + 0.118).abs());
        }
        assert!(worst < 0.03, "the tip swings {worst} m");
    }
}
