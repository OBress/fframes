//! Small 2D rigid body simulations baked per frame: bouncing balls with squash and spin.

use fframes_anime::glam::Vec2;

#[derive(Debug, Clone, Copy)]
pub struct BallState {
    pub pos: Vec2,
    pub rot: f32,
    /// > 1 right after an impact (flattened along the impact normal).
    pub squash: f32,
    /// Direction of the last impact normal.
    pub normal: Vec2,
}

#[derive(Debug, Clone, Copy)]
pub struct BallSetup {
    pub pos: Vec2,
    pub vel: Vec2,
    pub radius: f32,
    pub spin: f32,
    pub gravity: f32,
    pub restitution: f32,
    /// Left, top, right, bottom walls (`f32::INFINITY` to disable one).
    pub bounds: [f32; 4],
}

/// Axis-aligned boxes the ball bounces off (letters, plates, floors).
pub type Rect = [f32; 4];

/// Simulates `seconds` at `fps` output frames (with substeps) and returns one state per frame.
pub fn bake_ball(setup: BallSetup, rects: &[Rect], seconds: f32, fps: f32) -> Vec<BallState> {
    let substeps = 16;
    let dt = 1.0 / (fps * substeps as f32);
    let mut pos = setup.pos;
    let mut vel = setup.vel;
    let mut rot = 0.0_f32;
    let mut spin = setup.spin;
    let mut squash_v = 0.0_f32; // squash spring velocity
    let mut squash = 0.0_f32; // offset from 1
    let mut normal = Vec2::Y;
    let r = setup.radius;
    let frames = (seconds * fps).ceil() as usize + 1;
    let mut out = Vec::with_capacity(frames);
    for _ in 0..frames {
        out.push(BallState {
            pos,
            rot,
            squash: 1.0 + squash,
            normal,
        });
        for _ in 0..substeps {
            vel.y += setup.gravity * dt;
            pos += vel * dt;
            rot += spin * dt;
            let mut hit = |n: Vec2, depth: f32, pos: &mut Vec2, vel: &mut Vec2| {
                *pos += n * depth;
                let vn = vel.dot(n);
                if vn < 0.0 {
                    let impact = -vn;
                    *vel -= n * vn * (1.0 + setup.restitution);
                    // friction turns sliding into spin
                    let tangent = Vec2::new(-n.y, n.x);
                    let vt = vel.dot(tangent);
                    spin = vt / r;
                    *vel -= tangent * vt * 0.15;
                    squash_v += impact * 0.0012;
                    normal = n;
                }
            };
            let [l, t, rr, b] = setup.bounds;
            if pos.x - r < l {
                hit(Vec2::X, l - (pos.x - r), &mut pos, &mut vel);
            }
            if pos.x + r > rr {
                hit(Vec2::NEG_X, pos.x + r - rr, &mut pos, &mut vel);
            }
            if pos.y - r < t {
                hit(Vec2::Y, t - (pos.y - r), &mut pos, &mut vel);
            }
            if pos.y + r > b {
                hit(Vec2::NEG_Y, pos.y + r - b, &mut pos, &mut vel);
            }
            for rect in rects {
                let closest =
                    Vec2::new(pos.x.clamp(rect[0], rect[2]), pos.y.clamp(rect[1], rect[3]));
                let d = pos - closest;
                let len = d.length();
                if len < r && len > 1e-4 {
                    hit(d / len, r - len, &mut pos, &mut vel);
                }
            }
            // squash spring
            let k = 900.0;
            let c = 14.0;
            squash_v += (-k * squash - c * squash_v) * dt;
            squash += squash_v * dt;
            squash = squash.clamp(-0.35, 0.6);
        }
    }
    out
}

impl BallState {
    pub fn at(states: &[BallState], t: f32, fps: f32) -> BallState {
        let i = ((t * fps).floor().max(0.0) as usize).min(states.len().saturating_sub(1));
        states[i]
    }
}
