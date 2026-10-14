//! Rigid body piles and floods simulated with Rapier (rapier2d), baked per frame.
//! Units are screen pixels divided by 100 (1 unit = 100 px), y down.

use fframes_anime::glam::Vec2;
use rapier2d::prelude::*;

#[derive(Debug, Clone, Copy)]
pub enum Shape2 {
    Ball(f32),
    /// Half extents.
    Box(f32, f32),
    /// Half height (of the straight part) and radius, vertical.
    Capsule(f32, f32),
}

#[derive(Debug, Clone, Copy)]
pub struct Body2 {
    pub shape: Shape2,
    /// Pixels.
    pub pos: Vec2,
    /// Pixels per second.
    pub vel: Vec2,
    pub angle: f32,
    pub spin: f32,
    pub restitution: f32,
    pub friction: f32,
}

impl Body2 {
    pub fn ball(pos: Vec2, r: f32) -> Self {
        Body2 {
            shape: Shape2::Ball(r),
            pos,
            vel: Vec2::ZERO,
            angle: 0.0,
            spin: 0.0,
            restitution: 0.3,
            friction: 0.6,
        }
    }
}

/// A body moved by animation (it pushes the simulated ones but is not pushed back).
pub struct Kinematic {
    pub shape: Shape2,
    pub path: Box<dyn Fn(f32) -> Vec2>,
}

#[derive(Debug, Clone, Copy)]
pub struct Pose2 {
    pub pos: Vec2,
    pub angle: f32,
}

/// Positions of every body at every frame.
pub struct Baked2 {
    pub fps: f32,
    pub frames: Vec<Vec<Pose2>>,
}

impl Baked2 {
    pub fn at(&self, t: f32) -> &[Pose2] {
        let i = ((t * self.fps).floor().max(0.0) as usize).min(self.frames.len().saturating_sub(1));
        &self.frames[i]
    }
}

const SCALE: f32 = 0.01;

fn collider(shape: Shape2) -> ColliderBuilder {
    match shape {
        Shape2::Ball(r) => ColliderBuilder::ball(r * SCALE),
        Shape2::Box(hx, hy) => ColliderBuilder::cuboid(hx * SCALE, hy * SCALE),
        Shape2::Capsule(h, r) => ColliderBuilder::capsule_y(h * SCALE, r * SCALE),
    }
}

/// Simulates `bodies` falling with `gravity` (px/s², y down) inside the static boxes `walls`
/// (centre and half extents in pixels), with optional kinematic pushers.
pub fn bake(
    bodies: &[Body2],
    walls: &[(Vec2, Vec2)],
    kinematics: &[Kinematic],
    gravity: f32,
    seconds: f32,
    fps: f32,
) -> Baked2 {
    let mut world = PhysicsWorld {
        gravity: Vector::new(0.0, gravity * SCALE),
        ..Default::default()
    };
    let substeps = 4;
    world.integration_parameters.dt = 1.0 / (fps * substeps as f32);
    for (c, half) in walls {
        world.insert(
            RigidBodyBuilder::fixed().translation(Vector::new(c.x * SCALE, c.y * SCALE)),
            ColliderBuilder::cuboid(half.x * SCALE, half.y * SCALE).friction(0.8),
        );
    }
    let kin: Vec<RigidBodyHandle> = kinematics
        .iter()
        .map(|k| {
            let p = (k.path)(0.0);
            world
                .insert(
                    RigidBodyBuilder::kinematic_position_based()
                        .translation(Vector::new(p.x * SCALE, p.y * SCALE)),
                    collider(k.shape),
                )
                .0
        })
        .collect();
    let handles: Vec<RigidBodyHandle> = bodies
        .iter()
        .map(|b| {
            world
                .insert(
                    RigidBodyBuilder::dynamic()
                        .translation(Vector::new(b.pos.x * SCALE, b.pos.y * SCALE))
                        .rotation(b.angle)
                        .linvel(Vector::new(b.vel.x * SCALE, b.vel.y * SCALE))
                        .angvel(b.spin)
                        .ccd_enabled(true),
                    collider(b.shape)
                        .restitution(b.restitution)
                        .friction(b.friction),
                )
                .0
        })
        .collect();
    let frames_n = (seconds * fps).ceil() as usize + 1;
    let mut frames = Vec::with_capacity(frames_n);
    let snapshot = |world: &PhysicsWorld| -> Vec<Pose2> {
        handles
            .iter()
            .map(|h| {
                let b = &world.bodies[*h];
                let t = b.translation();
                Pose2 {
                    pos: Vec2::new(t.x / SCALE, t.y / SCALE),
                    angle: b.rotation().angle(),
                }
            })
            .collect()
    };
    frames.push(snapshot(&world));
    for f in 1..frames_n {
        for s in 0..substeps {
            let t = ((f - 1) as f32 + (s + 1) as f32 / substeps as f32) / fps;
            for (k, h) in kinematics.iter().zip(&kin) {
                let p = (k.path)(t);
                if let Some(body) = world.bodies.get_mut(*h) {
                    body.set_next_kinematic_translation(Vector::new(p.x * SCALE, p.y * SCALE));
                }
            }
            world.step();
        }
        frames.push(snapshot(&world));
    }
    Baked2 { fps, frames }
}
