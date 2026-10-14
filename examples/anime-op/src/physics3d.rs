//! Rigid bodies in 3D with Rapier (rapier3d), baked per frame: a cream puff dropping into a pair
//! of hands, resting in a palm while the arm moves. The hands are kinematic bodies whose
//! colliders are the hand meshes that get drawn, so a prop lands on the fingers you see and
//! moves with them, instead of being pasted near a hand and drifting off it.

use fframes_anime::figure::Figure;
use fframes_anime::glam::{Affine3A, Quat, Vec3};
use fframes_anime::rig::{Bone, Posed, Side};
use fframes_anime::toon::HandShape;
use rapier3d::prelude::*;

/// A prop let go at `release` seconds: before that it is held at `pos`.
#[derive(Debug, Clone)]
pub struct Toss {
    /// Points of the prop (its own frame, unscaled) for a convex collider.
    pub hull: Vec<Vec3>,
    pub scale: f32,
    pub pos: Vec3,
    pub rot: Quat,
    pub vel: Vec3,
    /// Angular velocity (radians per second around each axis).
    pub spin: Vec3,
    pub restitution: f32,
    pub friction: f32,
    pub release: f32,
}

/// The prop's placement at every simulated frame.
#[derive(Debug, Clone)]
pub struct Baked3 {
    pub start: f32,
    pub fps: f32,
    pub frames: Vec<(Vec3, Quat)>,
    /// When it first touched a hand.
    pub landed: Option<f32>,
}

impl Baked3 {
    /// Position and orientation at `t` (between frames interpolated, so a prop sampled at the
    /// time of a held drawing matches the hand of that drawing).
    pub fn at(&self, t: f32) -> (Vec3, Quat) {
        let f = ((t - self.start) * self.fps).max(0.0);
        let i = (f.floor() as usize).min(self.frames.len() - 1);
        let j = (i + 1).min(self.frames.len() - 1);
        let k = (f - i as f32).clamp(0.0, 1.0);
        let (a, b) = (self.frames[i], self.frames[j]);
        (a.0.lerp(b.0, k), a.1.slerp(b.1, k))
    }

    /// The prop's placement at `t` with its scale.
    pub fn affine(&self, t: f32, scale: f32) -> Affine3A {
        let (p, r) = self.at(t);
        Affine3A::from_scale_rotation_translation(Vec3::splat(scale), r, p)
    }
}

fn v(p: Vec3) -> Vector {
    Vector::new(p.x, p.y, p.z)
}

fn from_v(p: Vector) -> Vec3 {
    Vec3::new(p.x, p.y, p.z)
}

fn q(r: Quat) -> Rotation {
    Rotation::from_xyzw(r.x, r.y, r.z, r.w)
}

fn from_q(r: Rotation) -> Quat {
    Quat::from_xyzw(r.x, r.y, r.z, r.w).normalize()
}

/// The rigid placement of a hand mesh (modelled in the rest pose) for a posed skeleton.
fn hand_pose(figure: &Figure, posed: &Posed, side: Side) -> Pose {
    let bone = Bone::arm(side)[3];
    let rot = posed.rot(bone);
    let t = posed.pos(bone) - rot * figure.rest.pos(bone);
    Pose::from_parts(v(t), q(rot))
}

/// Simulates `toss` from `start` to `end` seconds with the hands `(side, shape)` of `posed(t)`
/// as kinematic colliders, plus static boxes `(centre, half extents)` (a table, the floor).
pub fn bake(
    toss: &Toss,
    figure: &Figure,
    hands: &[(Side, HandShape)],
    posed: &dyn Fn(f32) -> Posed,
    statics: &[(Vec3, Vec3)],
    start: f32,
    end: f32,
) -> Baked3 {
    let fps = 24.0;
    let substeps = 12;
    let mut world = PhysicsWorld {
        gravity: Vector::new(0.0, -9.81, 0.0),
        ..Default::default()
    };
    world.integration_parameters.dt = 1.0 / (fps * substeps as f32);
    for (c, half) in statics {
        world.insert(
            RigidBodyBuilder::fixed().translation(v(*c)),
            ColliderBuilder::cuboid(half.x, half.y, half.z).friction(0.9),
        );
    }
    let first = posed(start);
    let hand_bodies: Vec<(RigidBodyHandle, ColliderHandle, Side)> = hands
        .iter()
        .filter_map(|(side, shape)| {
            let mesh = figure.hand(*side, *shape);
            let collider = ColliderBuilder::trimesh(
                mesh.rest.iter().map(|p| v(*p)).collect(),
                mesh.tris.clone(),
            )
            .ok()?
            .friction(1.2)
            .restitution(0.1);
            let (b, c) = world.insert(
                RigidBodyBuilder::kinematic_position_based().pose(hand_pose(figure, &first, *side)),
                collider,
            );
            Some((b, c, *side))
        })
        .collect();
    let points: Vec<Vector> = toss.hull.iter().map(|p| v(*p * toss.scale)).collect();
    let shape = ColliderBuilder::convex_hull(&points)
        .unwrap_or_else(|| ColliderBuilder::ball(0.05 * toss.scale));
    let (prop, prop_collider) = world.insert(
        RigidBodyBuilder::kinematic_position_based()
            .pose(Pose::from_parts(v(toss.pos), q(toss.rot)))
            .ccd_enabled(true)
            .linear_damping(0.3)
            .angular_damping(1.2),
        shape
            .restitution(toss.restitution)
            .friction(toss.friction)
            .density(400.0),
    );
    let mut released = false;
    let frame_count = ((end - start) * fps).ceil() as usize + 1;
    let mut frames = Vec::with_capacity(frame_count);
    let mut landed = None;
    let snapshot = |world: &PhysicsWorld| {
        let b = &world.bodies[prop];
        (from_v(b.translation()), from_q(*b.rotation()))
    };
    frames.push(snapshot(&world));
    for f in 1..frame_count {
        for s in 0..substeps {
            let t = start + ((f - 1) as f32 + (s + 1) as f32 / substeps as f32) / fps;
            let p = posed(t);
            for (b, _, side) in &hand_bodies {
                if let Some(body) = world.bodies.get_mut(*b) {
                    body.set_next_kinematic_position(hand_pose(figure, &p, *side));
                }
            }
            if !released && t >= toss.release {
                released = true;
                if let Some(body) = world.bodies.get_mut(prop) {
                    body.set_body_type(RigidBodyType::Dynamic, true);
                    body.set_linvel(v(toss.vel), true);
                    body.set_angvel(v(toss.spin), true);
                }
            }
            world.step();
            if landed.is_none()
                && hand_bodies.iter().any(|(_, c, _)| {
                    world
                        .narrow_phase
                        .contact_pair(prop_collider, *c)
                        .is_some_and(|pair| pair.has_any_active_contact())
                })
            {
                landed = Some(t);
            }
        }
        frames.push(snapshot(&world));
    }
    Baked3 {
        start,
        fps,
        frames,
        landed,
    }
}

/// A prop laid in the palm of `side` a moment before the shot (`preroll` seconds) and carried
/// there from then on: it settles into the hand and rides its moves, swaying and slipping a
/// little when the hand moves fast.
#[allow(clippy::too_many_arguments)]
pub fn in_palm(
    figure: &Figure,
    posed: &dyn Fn(f32) -> Posed,
    side: Side,
    shape: HandShape,
    hull: Vec<Vec3>,
    scale: f32,
    preroll: f32,
    end: f32,
) -> Baked3 {
    let start = -preroll;
    let p = posed(start);
    let hand = Bone::arm(side)[3];
    let rot = p.rot(hand);
    let up = rot * Vec3::NEG_Y;
    let toss = Toss {
        hull,
        scale,
        pos: p.pos(hand) + rot * figure.palm(side) + up * 0.004,
        rot: Quat::from_rotation_arc(Vec3::Y, up),
        vel: Vec3::ZERO,
        spin: Vec3::ZERO,
        restitution: 0.05,
        friction: 1.4,
        release: start,
    };
    bake(&toss, figure, &[(side, shape)], posed, &[], start, end)
}
