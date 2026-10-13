//! The engineer: a procedural human figure and its poses.
//!
//! The figure is a man of 1.78 m in a work jacket, trousers and boots. He faces +Z, and his
//! left side is +X. The root node is on the ground between the feet. Group nodes make the
//! skeleton, see [`Joint`]. Smooth lofts make the clothes and the skin. Skin weights bend
//! the jacket, the sleeves, the trousers, the neck and the boots at the joints, so the
//! outline stays smooth. The head, the hands and the finger segments are rigid.
//!
//! A [`Pose`] holds the joint rotations. The pose generators put the feet in place with
//! inverse kinematics. Thus the soles stay on the ground (y = 0), and a planted foot does
//! not slide.
//!
//! ```rust,ignore
//! use the_why::figure::{Figure, FigureStyle, Pose};
//!
//! // once
//! let figure = Figure::build(&mut scene, None, &FigureStyle::default());
//!
//! // in render_frame: walk at 1 m/s from `start`, in the direction `heading`
//! let phase = t * 1.0 / Pose::stride(1.0);
//! let mut frame_scene = scene.clone();
//! figure.apply(&mut frame_scene, &Pose::walk(phase, 1.0).placed(start, heading));
//! ```

// Colors are hex numbers as in three.js.
#![allow(clippy::unreadable_literal)]

use std::f32::consts::{FRAC_PI_2, PI, TAU};
use std::ops::{Index, IndexMut};
use std::sync::Arc;

use fframes_3d::math::{noise1, progress, smoothstep};
use fframes_3d::*;

use Joint::*;

/// The joints of the skeleton. Each joint is a group node. The fingers have their own
/// nodes, see [`Pose::fingers`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Joint {
    Pelvis,
    Spine,
    Chest,
    Neck,
    Head,
    ClavicleL,
    ShoulderL,
    ElbowL,
    WristL,
    ClavicleR,
    ShoulderR,
    ElbowR,
    WristR,
    HipL,
    KneeL,
    AnkleL,
    ToeL,
    HipR,
    KneeR,
    AnkleR,
    ToeR,
}

/// The number of joints.
pub const JOINTS: usize = 21;

/// The index of the left hand, arm and leg in the arrays of this module.
pub const LEFT: usize = 0;
/// The index of the right hand, arm and leg.
pub const RIGHT: usize = 1;

/// The start of the line that [`Pose::draw`] draws, in root space, in meters.
pub const LINE_START: Vec3 = Vec3::new(-0.2, 0.0, 0.95);
/// The end of the line. The line is 0.6 m long.
pub const LINE_END: Vec3 = Vec3::new(-0.24, 0.0, 0.35);

/// The sign of X on each side: the left side is +X.
const SIDE: [f32; 2] = [1.0, -1.0];
const ARMS: [[Joint; 4]; 2] = [
    [ClavicleL, ShoulderL, ElbowL, WristL],
    [ClavicleR, ShoulderR, ElbowR, WristR],
];
const LEGS: [[Joint; 4]; 2] = [[HipL, KneeL, AnkleL, ToeL], [HipR, KneeR, AnkleR, ToeR]];
/// The joints above the pelvis.
const UPPER_BODY: [Joint; 12] = [
    Spine, Chest, Neck, Head, ClavicleL, ShoulderL, ElbowL, WristL, ClavicleR, ShoulderR, ElbowR,
    WristR,
];

/// From the hip joint to the knee joint, in meters.
const THIGH: f32 = 0.42;
/// From the knee joint to the ankle joint.
const SHIN: f32 = 0.405;
/// From the shoulder joint to the elbow joint.
const UPPER_ARM: f32 = 0.30;
/// From the elbow joint to the wrist joint.
const FOREARM: f32 = 0.255;

/// The toe joint (the ball of the foot) in the space of the ankle.
const TOE_JOINT: Vec3 = Vec3::new(0.0, -0.07, 0.13);
/// The back of the heel on the sole, in the space of the ankle.
const HEEL: Vec3 = Vec3::new(0.0, -0.095, -0.065);
/// The sole below the toe joint, in the space of the toe joint.
const BALL: Vec3 = Vec3::new(0.0, -0.025, 0.0);
/// The front of the sole, in the space of the toe joint.
const TOE_TIP: Vec3 = Vec3::new(0.0, -0.025, 0.09);
/// The height of the knee joint when the knee is on the ground.
const KNEE_RADIUS: f32 = 0.055;

/// The part of a gait cycle where a foot carries weight.
const STANCE: f32 = 0.62;
/// The end of the roll on the heel after the heel strike, as a part of the gait cycle.
const FOOT_FLAT: f32 = 0.1;
/// The start of the heel lift.
const HEEL_OFF: f32 = 0.36;
/// The angle of the foot, toes up, at the heel strike, in radians.
const STRIKE: f32 = 0.25;
/// The angle of the foot, heel up, at the toe off, in radians.
const TOE_OFF: f32 = 0.75;

/// The heel points and the turns of the feet (radians) when he stands.
const STAND_FEET: [(Vec3, f32); 2] = [
    (Vec3::new(0.11, 0.0, -0.065), 0.14),
    (Vec3::new(-0.11, 0.0, -0.045), -0.1),
];
/// The directions of the knees when he stands, in root space.
const KNEE_POLES: [Vec3; 2] = [Vec3::new(0.25, 0.0, 1.0), Vec3::new(-0.25, 0.0, 1.0)];
/// The right knee joint when he kneels. It is below the right hip joint.
const KNEE_DOWN: Vec3 = Vec3::new(-0.09, KNEE_RADIUS, 0.0);
/// The heel point and the turn (radians) of the left foot when he kneels.
const KNEEL_LEFT: (Vec3, f32) = (Vec3::new(0.17, 0.0, 0.52), 0.15);
/// The turn of the right foot when he kneels, in radians.
const KNEEL_RIGHT_YAW: f32 = -0.06;
/// The angle of the right foot, heel up, when he kneels on its toes.
const TUCK: f32 = 1.2;

const THUMB: usize = 0;
const INDEX: usize = 1;

/// The bend of the fingers of a relaxed hand: thumb, index, middle, ring, little finger.
const RELAXED: [f32; 5] = [0.2, 0.25, 0.32, 0.38, 0.45];

/// A finger of the left hand. The right hand mirrors X.
struct Finger {
    /// The base joint in the space of the wrist. The fingers hang along -Y, the palm faces
    /// -X and the thumb side is +Z.
    base: Vec3,
    /// The lengths of the three segments, in meters.
    lengths: [f32; 3],
    /// The radius at the base and at the tip, in meters.
    radius: [f32; 2],
    /// The turn of the open finger away from the middle finger, in radians.
    splay: f32,
}

/// Thumb, index, middle, ring and little finger.
const FINGERS: [Finger; 5] = [
    Finger {
        base: Vec3::new(-0.01, -0.024, 0.02),
        lengths: [0.045, 0.032, 0.027],
        radius: [0.0135, 0.0095],
        splay: 0.0,
    },
    Finger {
        base: Vec3::new(0.0, -0.09, 0.029),
        lengths: [0.042, 0.025, 0.021],
        radius: [0.0098, 0.0083],
        splay: -0.06,
    },
    Finger {
        base: Vec3::new(0.0, -0.094, 0.0095),
        lengths: [0.046, 0.029, 0.022],
        radius: [0.0102, 0.0086],
        splay: 0.0,
    },
    Finger {
        base: Vec3::new(0.0, -0.09, -0.0095),
        lengths: [0.043, 0.027, 0.021],
        radius: [0.0097, 0.0082],
        splay: 0.06,
    },
    Finger {
        base: Vec3::new(0.0, -0.083, -0.028),
        lengths: [0.034, 0.02, 0.018],
        radius: [0.0086, 0.0073],
        splay: 0.13,
    },
];

impl Joint {
    /// All joints. A parent comes before its children.
    pub const ALL: [Joint; JOINTS] = [
        Pelvis, Spine, Chest, Neck, Head, ClavicleL, ShoulderL, ElbowL, WristL, ClavicleR,
        ShoulderR, ElbowR, WristR, HipL, KneeL, AnkleL, ToeL, HipR, KneeR, AnkleR, ToeR,
    ];

    /// The parent joint and the offset from it in the rest pose, in meters. In the rest
    /// pose all joint rotations are zero: he stands straight with the arms down.
    fn layout(self) -> (Option<Joint>, Vec3) {
        match self {
            Pelvis => (None, vec3(0.0, 0.98, 0.0)),
            Spine => (Some(Pelvis), vec3(0.0, 0.1, -0.015)),
            Chest => (Some(Spine), vec3(0.0, 0.2, -0.005)),
            Neck => (Some(Chest), vec3(0.0, 0.2, 0.0)),
            Head => (Some(Neck), vec3(0.0, 0.12, 0.025)),
            ClavicleL => (Some(Chest), vec3(0.02, 0.14, 0.02)),
            ShoulderL => (Some(ClavicleL), vec3(0.165, -0.02, -0.01)),
            ElbowL => (Some(ShoulderL), vec3(0.0, -UPPER_ARM, 0.0)),
            WristL => (Some(ElbowL), vec3(0.0, -FOREARM, 0.0)),
            ClavicleR => (Some(Chest), vec3(-0.02, 0.14, 0.02)),
            ShoulderR => (Some(ClavicleR), vec3(-0.165, -0.02, -0.01)),
            ElbowR => (Some(ShoulderR), vec3(0.0, -UPPER_ARM, 0.0)),
            WristR => (Some(ElbowR), vec3(0.0, -FOREARM, 0.0)),
            HipL => (Some(Pelvis), vec3(0.09, -0.06, 0.0)),
            KneeL => (Some(HipL), vec3(0.0, -THIGH, 0.0)),
            AnkleL => (Some(KneeL), vec3(0.0, -SHIN, 0.0)),
            ToeL => (Some(AnkleL), TOE_JOINT),
            HipR => (Some(Pelvis), vec3(-0.09, -0.06, 0.0)),
            KneeR => (Some(HipR), vec3(0.0, -THIGH, 0.0)),
            AnkleR => (Some(KneeR), vec3(0.0, -SHIN, 0.0)),
            ToeR => (Some(AnkleR), TOE_JOINT),
        }
    }
}

/// The position of a joint in root space in the rest pose.
fn rest(joint: Joint) -> Vec3 {
    let (parent, offset) = joint.layout();
    parent.map_or(offset, |p| rest(p) + offset)
}

/// The materials of the figure.
#[derive(Debug, Clone)]
pub struct FigureStyle {
    pub skin: Material,
    pub jacket: Material,
    pub trousers: Material,
    pub boots: Material,
    pub hair: Material,
}

impl Default for FigureStyle {
    /// Dark cloth and skin with a thin cool rim, for a silhouette against light.
    fn default() -> Self {
        let rim = Rgb::hex(0x9fb4d8) * 0.3;
        let dark = |color: u32, roughness: f32| {
            Material::standard(color).roughness(roughness).rim(rim, 4.0)
        };
        Self {
            skin: dark(0x2a1d17, 0.55),
            jacket: dark(0x1b1e23, 0.85),
            trousers: dark(0x15171a, 0.9),
            boots: dark(0x1c1612, 0.6),
            hair: dark(0x0d0b0a, 0.75),
        }
    }
}

/// The scene nodes of the figure.
#[derive(Debug, Clone)]
pub struct Figure {
    /// The root node: on the ground between the feet. [`Figure::apply`] sets its position
    /// and its turn.
    pub root: NodeId,
    joints: [NodeId; JOINTS],
    /// The three joints of each finger, see [`Pose::fingers`].
    fingers: [[[NodeId; 3]; 5]; 2],
}

impl Figure {
    /// Adds the figure in the rest pose below `parent`, or at the root of the scene for
    /// `None`.
    pub fn build(scene: &mut Scene, parent: Option<NodeId>, style: &FigureStyle) -> Figure {
        let node = Node::group().named("figure");
        let root = match parent {
            Some(parent) => scene.add_to(parent, node),
            None => scene.add(node),
        };
        let mut joints = [root; JOINTS];
        for joint in Joint::ALL {
            let (parent, offset) = joint.layout();
            let parent = parent.map_or(root, |p| joints[p as usize]);
            joints[joint as usize] = scene.add_to(parent, Node::group().with_position(offset));
        }

        // The skinned parts are in root space in the rest pose. Each joint of the skin
        // moves them from its rest position.
        let skin = Skin {
            joints: joints.to_vec(),
            inverse_bind_matrices: Arc::new(
                Joint::ALL
                    .iter()
                    .map(|&j| Mat4::from_translation(-rest(j)))
                    .collect(),
            ),
        };
        let skinned = |scene: &mut Scene, mut geometry: Geometry, weights, material: &Material| {
            geometry.skin = Some(weights);
            let mut node = Node::mesh(geometry, material.clone());
            if let Some(mesh) = node.mesh_mut() {
                mesh.skin = Some(skin.clone());
            }
            scene.add_to(root, node);
        };
        // A rigid part, given in root space in the rest pose, moves with its joint.
        let rigid = |scene: &mut Scene, joint: Joint, geometry: Geometry, material: &Material| {
            let at = rest(joint);
            let geometry = geometry.translate(-at.x, -at.y, -at.z);
            scene.add_to(
                joints[joint as usize],
                Node::mesh(geometry, material.clone()),
            );
        };

        let jacket = loft(&JACKET, 40);
        let weights = chain_weights(
            &jacket,
            Vec3::Y,
            &[Pelvis, Spine, Chest],
            &[(rest(Spine), 0.08), (rest(Chest), 0.08)],
        );
        skinned(scene, jacket, weights, &style.jacket);
        rigid(scene, Chest, collar(), &style.jacket);

        let neck = loft(&NECK, 24);
        let weights = chain_weights(
            &neck,
            Vec3::Y,
            &[Chest, Neck, Head],
            &[(rest(Neck), 0.03), (rest(Head), 0.03)],
        );
        skinned(scene, neck, weights, &style.skin);
        rigid(scene, Head, head(), &style.skin);
        rigid(scene, Head, hair(), &style.hair);
        rigid(scene, Pelvis, loft(&SEAT, 40), &style.trousers);

        for i in [LEFT, RIGHT] {
            let [clavicle, shoulder, elbow, wrist] = ARMS[i];
            let at = rest(shoulder);
            let sleeve = loft(&SLEEVE, 24).translate(at.x, 0.0, at.z);
            let weights = chain_weights(
                &sleeve,
                Vec3::NEG_Y,
                &[clavicle, shoulder, elbow],
                &[(at + Vec3::Y * 0.03, 0.035), (rest(elbow), 0.045)],
            );
            skinned(scene, sleeve, weights, &style.jacket);
            // The sphere fills the outside of the elbow when the arm bends.
            let ball = Geometry::sphere(0.046, 16, 10).translate(at.x, rest(elbow).y, at.z);
            rigid(scene, elbow, ball, &style.jacket);

            let [hip, knee, ankle, toe] = LEGS[i];
            let at = rest(hip);
            let leg = loft(&LEG, 28).translate(at.x, 0.0, at.z);
            let weights = chain_weights(
                &leg,
                Vec3::NEG_Y,
                &[Pelvis, hip, knee],
                &[(at + Vec3::Y * 0.03, 0.06), (rest(knee), 0.05)],
            );
            skinned(scene, leg, weights, &style.trousers);
            let cap = Geometry::sphere(0.056, 20, 12).translate(at.x, rest(knee).y, 0.006);
            rigid(scene, knee, cap, &style.trousers);

            let at = rest(ankle);
            let boot = boot().translate(at.x, at.y, at.z);
            let weights = chain_weights(&boot, Vec3::Z, &[ankle, toe], &[(rest(toe), 0.025)]);
            skinned(scene, boot, weights, &style.boots);
            // The top of the shaft follows the shin, so it stays inside the trousers.
            let shaft = loft(&BOOT_SHAFT, 24).translate(at.x, at.y, at.z);
            let weights = chain_weights(
                &shaft,
                Vec3::NEG_Y,
                &[knee, ankle],
                &[(at + Vec3::Y * 0.01, 0.03)],
            );
            skinned(scene, shaft, weights, &style.boots);

            scene.add_to(
                joints[wrist as usize],
                Node::mesh(loft(&PALM, 20), style.skin.clone()),
            );
        }

        let mut fingers = [[[root; 3]; 5]; 2];
        for (i, side) in SIDE.into_iter().enumerate() {
            let wrist = joints[ARMS[i][3] as usize];
            for (f, finger) in FINGERS.iter().enumerate() {
                let rotations = finger_rotations(side, f, RELAXED[f]);
                let mut parent = wrist;
                let mut offset = finger.base * vec3(side, 1.0, 1.0);
                for (k, rotation) in rotations.into_iter().enumerate() {
                    let node = Node::group().with_position(offset).with_rotation(rotation);
                    parent = scene.add_to(parent, node);
                    fingers[i][f][k] = parent;
                    let [r0, r1] = finger.radius;
                    let segment = finger_segment(
                        finger.lengths[k],
                        r0 + (r1 - r0) * k as f32 / 3.0,
                        r0 + (r1 - r0) * (k + 1) as f32 / 3.0,
                    );
                    scene.add_to(parent, Node::mesh(segment, style.skin.clone()));
                    offset = vec3(0.0, -finger.lengths[k], 0.0);
                }
            }
        }

        Figure {
            root,
            joints,
            fingers,
        }
    }

    /// The node of a joint.
    pub fn joint(&self, joint: Joint) -> NodeId {
        self.joints[joint as usize]
    }

    /// Sets the root and the joints to `pose`. Only rotations and positions change.
    pub fn apply(&self, scene: &mut Scene, pose: &Pose) {
        let root = &mut scene[self.root];
        root.position = pose.position;
        root.rotation = Quat::from_rotation_y(pose.heading);
        for joint in Joint::ALL {
            scene[self.joint(joint)].rotation = pose[joint];
        }
        scene[self.joint(Pelvis)].position = rest(Pelvis) + pose.pelvis;
        for (i, side) in SIDE.into_iter().enumerate() {
            for (f, (nodes, curl)) in self.fingers[i].iter().zip(pose.fingers[i]).enumerate() {
                for (node, rotation) in nodes.iter().zip(finger_rotations(side, f, curl)) {
                    scene[*node].rotation = rotation;
                }
            }
        }
    }

    /// The tip of the right index finger in world space. [`Pose::draw`] moves it along the
    /// line from [`LINE_START`] to [`LINE_END`].
    pub fn fingertip(&self, scene: &Scene) -> Vec3 {
        scene.local_to_world(self.fingers[RIGHT][INDEX][2], index_tip())
    }

    /// The heel, the ball and the toe tip of the left and the right sole, in world space.
    /// Use them for footprints and dust.
    pub fn feet(&self, scene: &Scene) -> [[Vec3; 3]; 2] {
        LEGS.map(|[_, _, ankle, toe]| {
            [
                scene.local_to_world(self.joint(ankle), HEEL),
                scene.local_to_world(self.joint(toe), BALL),
                scene.local_to_world(self.joint(toe), TOE_TIP),
            ]
        })
    }
}

/// The joint rotations and the place of the figure at one moment.
#[derive(Debug, Clone, PartialEq)]
pub struct Pose {
    /// The root position in the space of the parent node, in meters. The ground is y = 0.
    pub position: Vec3,
    /// The turn of the root around Y, in radians. At 0 the figure faces +Z.
    pub heading: f32,
    /// The offset of the pelvis from its rest position, in root space, in meters.
    pub pelvis: Vec3,
    /// The rotation of each joint relative to its parent, in [`Joint`] order. Index a pose
    /// with a [`Joint`] to read or set one.
    pub joints: [Quat; JOINTS],
    /// The bend of each finger from straight (0) to a fist (1): thumb, index, middle, ring
    /// and little finger. The left hand comes first.
    pub fingers: [[f32; 5]; 2],
}

impl Default for Pose {
    /// The rest pose: straight, with the arms down and relaxed hands.
    fn default() -> Self {
        Self {
            position: Vec3::ZERO,
            heading: 0.0,
            pelvis: Vec3::ZERO,
            joints: [Quat::IDENTITY; JOINTS],
            fingers: [RELAXED; 2],
        }
    }
}

impl Index<Joint> for Pose {
    type Output = Quat;

    fn index(&self, joint: Joint) -> &Quat {
        &self.joints[joint as usize]
    }
}

impl IndexMut<Joint> for Pose {
    fn index_mut(&mut self, joint: Joint) -> &mut Quat {
        &mut self.joints[joint as usize]
    }
}

impl Pose {
    /// He stands and breathes, and his weight moves slowly from one leg to the other.
    /// `time` is in seconds.
    pub fn stand(time: f32) -> Pose {
        Self::standing(time, 0.0)
    }

    /// He stands and lifts the chest and the head to the sky. `t` goes from 0 (the stand at
    /// time 0) to 1 (the head up).
    pub fn look_up(t: f32) -> Pose {
        Self::standing(0.0, smoothstep(0.0, 1.0, t))
    }

    /// The distance of one gait cycle (two steps) at `speed` meters per second, in meters.
    pub fn stride(speed: f32) -> f32 {
        2.0 * (0.42 + 0.21 * speed.clamp(0.3, 2.0))
    }

    /// A walk along +Z at `speed` meters per second. `phase` counts gait cycles: the right
    /// heel strikes at each whole number, the left heel half a cycle later. The root moves
    /// `phase * Pose::stride(speed)` meters, so a planted foot stays in place.
    pub fn walk(phase: f32, speed: f32) -> Pose {
        let stride = Self::stride(speed);
        let travel = phase * stride;
        let amount = (speed / 1.3).clamp(0.3, 1.4);
        let angle = TAU * phase;
        let mut pose = Pose {
            position: vec3(0.0, 0.0, travel),
            ..Pose::default()
        };

        // The pelvis turns forward on the side of the swing leg, drops on that side and
        // moves over the standing foot.
        let yaw = 0.08 * amount * angle.cos();
        let list = -0.045 * angle.sin();
        let lean = 0.02 + 0.04 * amount;
        pose[Pelvis] = Quat::from_rotation_y(yaw) * Quat::from_rotation_z(list);
        pose.pelvis.x = -0.02 * angle.sin();
        let feet = [phase - 0.5, phase].map(|cycle| {
            let side = if cycle < phase { 1.0 } else { -1.0 };
            let mut foot = walk_foot(cycle, stride, side);
            foot.ankle.z -= cycle * stride;
            foot
        });
        pose.pelvis.y = pose.pelvis_reach(&feet, 0.997);
        pose.solve_legs(&feet, [vec3(0.1, 0.0, 1.0), vec3(-0.1, 0.0, 1.0)]);

        // The spine turns against the pelvis, so the shoulders stay calm and the head
        // looks ahead.
        pose[Spine] = euler(0.4 * lean, -0.7 * yaw, -0.6 * list);
        pose[Chest] = euler(0.6 * lean, -0.6 * yaw, -0.3 * list);
        pose[Neck] = euler(0.03 - 0.4 * lean, 0.15 * yaw, 0.0);
        pose[Head] = euler(0.03 - 0.6 * lean, 0.15 * yaw, -0.1 * list);

        // The arms swing against the legs: the left arm is forward when the right heel
        // strikes. The elbow bends more when the arm is forward.
        let swing = 0.24 * amount;
        let forward = (angle - 0.25).cos();
        for i in [LEFT, RIGHT] {
            let side = SIDE[i];
            let [_, shoulder, elbow, _] = ARMS[i];
            let f = side * forward;
            pose[shoulder] = euler(0.03 - swing * f, 0.0, side * 0.1);
            pose[elbow] = euler(-0.22 - 0.2 * (0.5 + 0.5 * f), 0.0, 0.0);
        }
        pose
    }

    /// He kneels on the right knee. The left foot steps forward, the right foot steps back
    /// onto its toes, and the body goes down. `t` goes from 0 ([`Pose::stand`] at time 0)
    /// to 1 (on the knee). The film plays it over about 1.5 s.
    pub fn kneel(t: f32) -> Pose {
        let t = t.clamp(0.0, 1.0);
        let start = Pose::stand(0.0);
        let end = kneeling(0.1, 0.12);
        // One foot steps at a time, so the other foot carries the weight.
        let left_step = smoothstep(0.0, 0.35, t);
        let right_step = smoothstep(0.35, 0.7, t);
        let down = smoothstep(0.3, 1.0, t);
        let upper = smoothstep(0.2, 1.0, t);

        let mut pose = start.clone();
        for joint in UPPER_BODY {
            pose[joint] = start[joint].slerp(end[joint], upper);
        }
        for (hand, end_hand) in pose.fingers.iter_mut().zip(end.fingers) {
            for (curl, end_curl) in hand.iter_mut().zip(end_hand) {
                *curl += (end_curl - *curl) * upper;
            }
        }
        pose[Pelvis] = start[Pelvis].slerp(end[Pelvis], down);
        pose.pelvis = start.pelvis.lerp(end.pelvis, down);

        let (left_heel, left_yaw) = STAND_FEET[LEFT];
        let (right_heel, right_yaw) = STAND_FEET[RIGHT];
        let mut left = planted(
            left_heel.lerp(KNEEL_LEFT.0, left_step),
            left_yaw + (KNEEL_LEFT.1 - left_yaw) * left_step,
            0.0,
        );
        left.ankle.y += 0.08 * (PI * left_step).sin();
        let mut right = planted(
            right_heel.lerp(kneeling_heel(), right_step),
            right_yaw + (KNEEL_RIGHT_YAW - right_yaw) * right_step,
            -TUCK * right_step,
        );
        right.ankle.y += 0.04 * (PI * right_step).sin();
        let feet = [left, right];

        let height = start.pelvis.y + (end.pelvis.y - start.pelvis.y) * down;
        pose.pelvis.y = height.min(pose.pelvis_reach(&feet, 0.993));
        let hip = pose.hip(RIGHT);
        let knee = KNEE_POLES[RIGHT]
            .normalize()
            .lerp((KNEE_DOWN - hip).normalize(), down);
        pose.solve_legs(&feet, [KNEE_POLES[LEFT], knee]);
        pose
    }

    /// He kneels, leans forward and draws a line in the dust with the right index finger.
    /// The tip of the finger is at `LINE_START.lerp(LINE_END, t)`, see
    /// [`Figure::fingertip`]. The other fingers curl, and the left hand rests on the left
    /// thigh.
    pub fn draw(t: f32) -> Pose {
        let t = t.clamp(0.0, 1.0);
        let tip = LINE_START.lerp(LINE_END, t) + Vec3::Y * 0.004;
        let fingers = [1.0, 0.0, 0.92, 1.0, 1.0];
        // The finger points down and forward along the line, so it drags in the dust.
        let ahead = (LINE_START - LINE_END).normalize();
        let hand = hand_rotation(ahead * 0.7 + Vec3::NEG_Y, Vec3::X);
        let wrist = tip - hand * index_tip_in_wrist(SIDE[RIGHT], fingers[INDEX]);

        // He bends from the hips, with a little bend of the back to the right. The hips
        // start forward over the right knee and rock back as the finger comes closer. He
        // bends only as far as the arm needs to reach the wrist. The right shoulder comes
        // forward and down with the bend.
        let forward = 0.85 - 1.05 * t;
        let bent = |bend: f32| {
            let mut pose = kneeling(forward, 0.12 + 0.7 * bend);
            pose[Spine] = euler(0.15 * bend, -0.05, 0.06 * bend);
            pose[Chest] = euler(0.15 * bend, -0.1, 0.06 * bend);
            pose[ClavicleR] = euler(0.0, 0.2 * bend, 0.12 * bend);
            pose
        };
        let reach = 0.92 * (UPPER_ARM + FOREARM);
        let (mut low, mut high) = (0.0, 1.4);
        for _ in 0..20 {
            let mid = f32::midpoint(low, high);
            let shoulder = bent(mid).joint_matrices()[ShoulderR as usize].w_axis;
            if shoulder.truncate().distance(wrist) > reach {
                low = mid;
            } else {
                high = mid;
            }
        }
        let mut pose = bent(high);
        pose.fingers[RIGHT] = fingers;
        pose.reach(RIGHT, wrist, hand, vec3(-0.8, 0.3, -0.5));
        pose.rest_hand_on_thigh(1.2);
        pose.look_at(tip);
        pose
    }

    /// Interpolates the place, the joints and the fingers, then puts the lowest point of
    /// the feet and the knees on the ground.
    pub fn lerp(&self, other: &Pose, t: f32) -> Pose {
        let turn = (other.heading - self.heading + PI).rem_euclid(TAU) - PI;
        let pose = Pose {
            position: self.position.lerp(other.position, t),
            heading: self.heading + turn * t,
            pelvis: self.pelvis.lerp(other.pelvis, t),
            joints: std::array::from_fn(|i| self.joints[i].slerp(other.joints[i], t)),
            fingers: std::array::from_fn(|h| {
                std::array::from_fn(|f| {
                    self.fingers[h][f] + (other.fingers[h][f] - self.fingers[h][f]) * t
                })
            }),
        };
        pose.grounded()
    }

    /// Moves the pelvis up or down so that the lowest point of the soles and the knees
    /// touches the ground.
    pub fn grounded(mut self) -> Pose {
        let m = self.joint_matrices();
        let mut lowest = f32::INFINITY;
        for [_, knee, ankle, toe] in LEGS {
            let points = [
                m[knee as usize].w_axis.y - KNEE_RADIUS,
                m[ankle as usize].transform_point3(HEEL).y,
                m[toe as usize].transform_point3(BALL).y,
                m[toe as usize].transform_point3(TOE_TIP).y,
            ];
            lowest = points.into_iter().fold(lowest, f32::min);
        }
        self.pelvis.y -= lowest;
        self
    }

    /// Moves the pose to `origin` and turns it by `heading` radians around Y. A walk that
    /// went along +Z then goes in the direction `heading`.
    pub fn placed(mut self, origin: Vec3, heading: f32) -> Pose {
        self.position = origin + Quat::from_rotation_y(heading) * self.position;
        self.heading += heading;
        self
    }

    /// The idle stance at `time` seconds. `look` lifts the chest and the head, `0..1`.
    fn standing(time: f32, look: f32) -> Pose {
        let breath = (TAU * time / 4.5).sin();
        // Positive puts the weight on the left leg.
        let weight = 0.7 * (TAU * time / 11.0).sin() + 0.3 * noise1(3, 0.2 * time);
        let mut pose = Pose {
            pelvis: vec3(0.02 * weight, 0.0, 0.02 * look),
            ..Pose::default()
        };
        pose[Pelvis] = euler(-0.03 * look, 0.0, 0.03 * weight);
        let feet = STAND_FEET.map(|(heel, yaw)| planted(heel, yaw, 0.0));
        pose.pelvis.y = pose.pelvis_reach(&feet, 0.993);
        pose.solve_legs(&feet, KNEE_POLES);

        // The spine keeps the shoulders level, and the chest rises when he breathes in.
        pose[Spine] = euler(-0.02 - 0.05 * look, 0.0, -0.02 * weight);
        pose[Chest] = euler(0.02 - 0.012 * breath - 0.1 * look, 0.0, -0.012 * weight);
        pose[Neck] = euler(0.06 - 0.25 * look, 0.0, 0.0);
        pose[Head] = euler(
            0.03 + 0.02 * noise1(6, 0.2 * time) - 0.4 * look,
            0.06 * noise1(5, 0.25 * time),
            0.0,
        );
        for i in [LEFT, RIGHT] {
            let side = SIDE[i];
            let [clavicle, shoulder, elbow, _] = ARMS[i];
            pose[clavicle] = euler(0.0, side * 0.06 * look, side * 0.01 * breath);
            pose[shoulder] = euler(0.03, 0.0, side * 0.11);
            pose[elbow] = euler(-0.16, 0.0, 0.0);
        }
        pose
    }

    /// The transform of each joint in root space, in [`Joint`] order.
    fn joint_matrices(&self) -> [Mat4; JOINTS] {
        let mut out = [Mat4::IDENTITY; JOINTS];
        for joint in Joint::ALL {
            let (parent, mut offset) = joint.layout();
            if joint == Pelvis {
                offset += self.pelvis;
            }
            let local = Mat4::from_rotation_translation(self[joint], offset);
            out[joint as usize] = parent.map_or(local, |p| out[p as usize] * local);
        }
        out
    }

    /// The hip joint of leg `i` in root space.
    fn hip(&self, i: usize) -> Vec3 {
        rest(Pelvis) + self.pelvis + self[Pelvis] * LEGS[i][0].layout().1
    }

    /// The highest pelvis offset (y) at which each leg reaches its ankle with the hip to
    /// ankle distance at `extension` times the leg length. The knees then bend a little.
    fn pelvis_reach(&self, feet: &[Foot; 2], extension: f32) -> f32 {
        let reach = (THIGH + SHIN) * extension;
        let [left, right] = [LEFT, RIGHT].map(|i| {
            let hip = self.hip(i) - Vec3::Y * self.pelvis.y;
            let flat = (feet[i].ankle - hip).with_y(0.0).length().min(reach);
            feet[i].ankle.y + (reach * reach - flat * flat).sqrt() - hip.y
        });
        smooth_min(left, right, 0.04)
    }

    /// Bends the legs so that the ankles reach `feet`. `poles` are the directions that the
    /// knees point to, in root space.
    fn solve_legs(&mut self, feet: &[Foot; 2], poles: [Vec3; 2]) {
        let m = self.joint_matrices();
        let pelvis = Quat::from_mat4(&m[Pelvis as usize]);
        for (i, foot) in feet.iter().enumerate() {
            let [hip, knee, ankle, toe] = LEGS[i];
            let start = m[hip as usize].w_axis.truncate();
            let (upper, lower) = two_bone(pelvis, start, foot.ankle, THIGH, SHIN, poles[i], 1.0);
            self[hip] = upper;
            self[knee] = lower;
            self[ankle] = (pelvis * upper * lower).inverse() * foot.rotation;
            self[toe] = Quat::from_rotation_x(foot.toes);
        }
    }

    /// Bends arm `i` so that the wrist reaches `wrist` with the hand rotation `hand`, in root
    /// space. `pole` is the direction that the elbow points to.
    fn reach(&mut self, i: usize, wrist: Vec3, hand: Quat, pole: Vec3) {
        let [clavicle, shoulder, elbow, wrist_joint] = ARMS[i];
        let m = self.joint_matrices();
        let parent = Quat::from_mat4(&m[clavicle as usize]);
        let start = m[shoulder as usize].w_axis.truncate();
        let (upper, lower) = two_bone(parent, start, wrist, UPPER_ARM, FOREARM, pole, -1.0);
        self[shoulder] = upper;
        self[elbow] = lower;
        self[wrist_joint] = (parent * upper * lower).inverse() * hand;
    }

    /// Rests the left hand on the left thigh, with the fingers along the thigh and down.
    /// `along` puts the wrist from the hip (0) to the knee (1). Above 1 the forearm lies
    /// on the knee and the hand hangs in front of it.
    fn rest_hand_on_thigh(&mut self, along: f32) {
        let m = self.joint_matrices();
        let hip = m[HipL as usize].w_axis.truncate();
        let knee = m[KneeL as usize].w_axis.truncate();
        let wrist = hip.lerp(knee, along) + Vec3::Y * 0.08;
        let hand = hand_rotation(knee - hip + Vec3::NEG_Y * 0.4, Vec3::NEG_X);
        self.reach(LEFT, wrist, hand, vec3(1.0, -0.3, 0.0));
    }

    /// Turns the neck and the head toward `target`, in root space. The eyes do the rest,
    /// so the head turns 80 % of the way.
    fn look_at(&mut self, target: Vec3) {
        let m = self.joint_matrices();
        let chest = Quat::from_mat4(&m[Chest as usize]);
        let eyes = m[Head as usize].transform_point3(vec3(0.0, 0.06, 0.08));
        let d = chest.inverse() * (target - eyes).normalize();
        let pitch = -0.8 * d.y.asin();
        let yaw = 0.8 * d.x.atan2(d.z);
        self[Neck] = euler(0.4 * pitch, 0.4 * yaw, 0.0);
        self[Head] = euler(0.6 * pitch, 0.6 * yaw, 0.0);
    }
}

/// He kneels on the right knee. `forward` turns the right thigh forward from vertical, in
/// radians: the hips go forward and down. `tilt` bends the pelvis forward, in radians.
fn kneeling(forward: f32, tilt: f32) -> Pose {
    let mut pose = Pose::default();
    pose[Pelvis] = euler(tilt, 0.0, 0.0);
    // The right hip is one thigh length from the knee on the ground.
    let hip = KNEE_DOWN + THIGH * vec3(0.0, forward.cos(), forward.sin());
    pose.pelvis = hip - rest(Pelvis) - pose[Pelvis] * HipR.layout().1;
    let feet = [
        planted(KNEEL_LEFT.0, KNEEL_LEFT.1, 0.0),
        planted(kneeling_heel(), KNEEL_RIGHT_YAW, -TUCK),
    ];
    pose.solve_legs(&feet, [KNEE_POLES[LEFT], KNEE_DOWN - hip]);

    pose[Spine] = euler(0.06, 0.0, 0.0);
    pose[Chest] = euler(0.1, 0.0, 0.0);
    pose[Neck] = euler(0.12, 0.0, 0.0);
    pose[Head] = euler(0.3, 0.0, 0.0);
    pose[ShoulderR] = euler(-0.2, 0.0, -0.08);
    pose[ElbowR] = euler(-0.35, 0.0, 0.0);
    pose.rest_hand_on_thigh(0.93);
    pose
}

/// The heel point of the right foot when he kneels: the heel point of the flat foot that,
/// on its toes, puts the ankle one shin length behind the knee on the ground.
fn kneeling_heel() -> Vec3 {
    let tucked = planted(Vec3::ZERO, KNEEL_RIGHT_YAW, -TUCK).ankle;
    let rise = tucked.y - KNEE_DOWN.y;
    let back = (SHIN * SHIN - rise * rise).sqrt();
    vec3(KNEE_DOWN.x - tucked.x, 0.0, KNEE_DOWN.z - back - tucked.z)
}

/// The place of a foot in root space.
#[derive(Debug, Clone, Copy)]
struct Foot {
    ankle: Vec3,
    rotation: Quat,
    /// The rotation of the toes around X, in radians. A negative angle bends them up.
    toes: f32,
}

impl Foot {
    /// The height of the lowest point of the sole.
    fn lowest(&self) -> f32 {
        let toe = self.ankle + self.rotation * TOE_JOINT;
        let toes = self.rotation * Quat::from_rotation_x(self.toes);
        let heel = (self.ankle + self.rotation * HEEL).y;
        let ball = (toe + toes * BALL).y;
        let tip = (toe + toes * TOE_TIP).y;
        heel.min(ball).min(tip)
    }
}

/// A foot on the ground. `heel` is the heel point of the flat foot and `yaw` its turn
/// around Y. A positive `roll` lifts the toes, and the foot turns on the heel. A negative
/// `roll` lifts the heel, and the foot turns on the toe joint while the toes stay flat.
fn planted(heel: Vec3, yaw: f32, roll: f32) -> Foot {
    let turn = Quat::from_rotation_y(yaw);
    let rotation = turn * Quat::from_rotation_x(-roll);
    let ankle = if roll >= 0.0 {
        heel - rotation * HEEL
    } else {
        heel + turn * (TOE_JOINT - HEEL) - rotation * TOE_JOINT
    };
    Foot {
        ankle,
        rotation,
        toes: roll.min(0.0),
    }
}

/// The foot of `side` in a walk. `cycle` counts the gait cycles of this foot from a heel
/// strike. Z is relative to the root at cycle 0, and the root moves one stride in each
/// cycle.
fn walk_foot(cycle: f32, stride: f32, side: f32) -> Foot {
    let yaw = side * 0.08;
    // The ankle is below the hip in the middle of the stance.
    let heel = |step: f32| vec3(side * 0.075, 0.0, (step + 0.25) * stride + HEEL.z);
    // Heel strike, then the foot rolls flat, then the heel lifts until the toes leave.
    let stance = |f: f32, step: f32| {
        let roll = if f < FOOT_FLAT {
            STRIKE * (1.0 - smoothstep(0.0, FOOT_FLAT, f))
        } else {
            -TOE_OFF * progress(f, HEEL_OFF, STANCE).powi(2)
        };
        planted(heel(step), yaw, roll)
    };
    let step = cycle.floor();
    let f = cycle - step;
    if f < STANCE {
        return stance(f, step);
    }

    // The swing: the foot lifts, goes forward and turns from toes down to toes up.
    let s = (f - STANCE) / (1.0 - STANCE);
    let from = stance(STANCE, step);
    let to = stance(0.0, step + 1.0);
    let mut ankle = from.ankle.lerp(to.ankle, smoothstep(0.0, 1.0, s));
    ankle.y = from.ankle.y + (to.ankle.y - from.ankle.y) * s + 0.07 * (PI * s.sqrt()).sin().powi(2);
    let pitch = -TOE_OFF + (STRIKE + TOE_OFF) * (1.0 - (1.0 - s).powi(2));
    let mut foot = Foot {
        ankle,
        rotation: Quat::from_rotation_y(yaw) * Quat::from_rotation_x(-pitch),
        toes: -TOE_OFF * (1.0 - smoothstep(0.0, 0.4, s)),
    };
    // The sole keeps a small distance from the ground.
    let clearance = 0.012 * (PI * s).sin();
    foot.ankle.y += (clearance - foot.lowest()).max(0.0);
    foot
}

/// Bends a limb of two bones so that its end reaches `target`, in root space.
///
/// `start` is the upper joint and `parent` is the rotation of its parent. `pole` is the
/// direction that the middle joint (knee or elbow) points to. In the rest pose the limb
/// hangs along -Y and the middle joint points to `+Z * bend`: `bend` is 1 for a leg and -1
/// for an arm. Returns the local rotations of the upper and the middle joint.
fn two_bone(
    parent: Quat,
    start: Vec3,
    target: Vec3,
    upper_length: f32,
    lower_length: f32,
    pole: Vec3,
    bend: f32,
) -> (Quat, Quat) {
    let (a, b) = (upper_length, lower_length);
    let to_target = target - start;
    let d = to_target
        .length()
        .clamp((a - b).abs() + 1e-4, (a + b) * 0.9999);
    let dir = to_target.normalize_or(Vec3::NEG_Y);
    let side = (pole - dir * pole.dot(dir)).normalize_or(dir.any_orthonormal_vector());
    let angle = ((a * a + d * d - b * b) / (2.0 * a * d))
        .clamp(-1.0, 1.0)
        .acos();
    let upper = dir * angle.cos() + side * angle.sin();
    let front = side * angle.cos() - dir * angle.sin();
    let y = -upper;
    let z = front * bend;
    let world = Quat::from_mat3(&Mat3::from_cols(y.cross(z), y, z));
    let inner = ((a * a + b * b - d * d) / (2.0 * a * b))
        .clamp(-1.0, 1.0)
        .acos();
    (
        parent.inverse() * world,
        Quat::from_rotation_x(bend * (PI - inner)),
    )
}

/// The rotation of a hand whose fingers point along `fingers` and whose thumb side faces
/// `thumb`, in root space.
fn hand_rotation(fingers: Vec3, thumb: Vec3) -> Quat {
    let y = -fingers.normalize();
    let z = (thumb - y * thumb.dot(y)).normalize();
    Quat::from_mat3(&Mat3::from_cols(y.cross(z), y, z))
}

/// The local rotations of the three joints of finger `f` on the hand of `side`. `curl`
/// bends the finger from straight (0) to a fist (1).
fn finger_rotations(side: f32, f: usize, curl: f32) -> [Quat; 3] {
    // A positive angle turns the finger toward the palm.
    let bend = |angle: f32| Quat::from_rotation_z(-side * angle);
    if f == THUMB {
        return [
            thumb_rest(side) * bend(0.35 * curl),
            bend(0.6 * curl),
            bend(0.9 * curl),
        ];
    }
    let splay = Quat::from_rotation_x(FINGERS[f].splay * (1.0 - curl));
    [
        splay * bend(1.5 * curl),
        bend(1.75 * curl),
        bend(1.2 * curl),
    ]
}

/// The rest rotation of the thumb base: the thumb points down and forward, and it bends
/// across the palm toward the little finger.
fn thumb_rest(side: f32) -> Quat {
    let along = vec3(-side * 0.35, -0.75, 0.56).normalize();
    let across = vec3(-side * 0.7, 0.0, -0.7);
    let across = (across - along * across.dot(along)).normalize();
    let x = -side * across;
    let y = -along;
    Quat::from_mat3(&Mat3::from_cols(x, y, x.cross(y)))
}

/// The tip of the index finger in the space of its last joint.
fn index_tip() -> Vec3 {
    let finger = &FINGERS[INDEX];
    vec3(0.0, -finger.lengths[2] - 0.9 * finger.radius[1], 0.0)
}

/// The tip of the index finger in the space of the wrist of `side`, with the bend `curl`.
fn index_tip_in_wrist(side: f32, curl: f32) -> Vec3 {
    let finger = &FINGERS[INDEX];
    let mut m = Mat4::IDENTITY;
    let mut offset = finger.base * vec3(side, 1.0, 1.0);
    for (k, rotation) in finger_rotations(side, INDEX, curl).into_iter().enumerate() {
        m *= Mat4::from_rotation_translation(rotation, offset);
        offset = vec3(0.0, -finger.lengths[k], 0.0);
    }
    m.transform_point3(index_tip())
}

/// A smooth minimum of `a` and `b` that is never above `min(a, b)`. `k` is the width of
/// the blend.
fn smooth_min(a: f32, b: f32, k: f32) -> f32 {
    let h = (0.5 + 0.5 * (b - a) / k).clamp(0.0, 1.0);
    b + (a - b) * h - k * h * (1.0 - h)
}

/// Skin weights for a part that runs along `axis` in the rest pose. The part belongs to
/// `bones[0]` up to the first cut, then to `bones[1]` up to the next cut, and so on. Each
/// cut is a point and the half width of the blend between the two bones, in meters.
fn chain_weights(
    geometry: &Geometry,
    axis: Vec3,
    bones: &[Joint],
    cuts: &[(Vec3, f32)],
) -> SkinWeights {
    let mut weights = SkinWeights::default();
    for p in &geometry.positions {
        let s = p.dot(axis);
        let k = cuts
            .iter()
            .take_while(|(at, blend)| s > at.dot(axis) - blend)
            .count();
        // The point is past `k` cuts. Near the last of them it blends two bones.
        let (mut pair, mut w) = ([bones[0], bones[0]], 0.0);
        if k > 0 {
            let (at, blend) = cuts[k - 1];
            let c = at.dot(axis);
            pair = [bones[k - 1], bones[k]];
            w = smoothstep(c - blend, c + blend, s);
        }
        weights.joints.push([pair[0] as u16, pair[1] as u16, 0, 0]);
        weights.weights.push([1.0 - w, w, 0.0, 0.0]);
    }
    weights
}

/// A smooth closed tube through elliptic cross-sections stacked along Y.
///
/// Each ring is `[y, half width along X, half depth along Z, center on Z]`. A ring of zero
/// size is one point that closes the tube. A Catmull-Rom spline through the rings adds
/// three rings between two rings, so a few rings give a smooth shape.
fn loft(rings: &[[f32; 4]], segments: u32) -> Geometry {
    const STEPS: usize = 4;
    let last = rings.len() - 1;
    let ring = |i: usize| Vec4::from_array(rings[i.min(last)]);
    let mut smooth = Vec::with_capacity(last * STEPS + 1);
    for i in 0..last {
        let (p0, p1, p2, p3) = (ring(i.saturating_sub(1)), ring(i), ring(i + 1), ring(i + 2));
        for k in 0..STEPS {
            let t = k as f32 / STEPS as f32;
            let (t2, t3) = (t * t, t * t * t);
            smooth.push(
                0.5 * (2.0 * p1
                    + (p2 - p0) * t
                    + (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * t2
                    + (3.0 * p1 - p0 - 3.0 * p2 + p3) * t3),
            );
        }
    }
    smooth.push(ring(last));

    // The first vertex of each ring, and whether the ring is a single point.
    let mut starts = Vec::with_capacity(smooth.len());
    let mut positions = Vec::new();
    for r in &smooth {
        let [y, rx, rz, cz] = r.to_array();
        let point = rx <= 1e-5 && rz <= 1e-5;
        starts.push((positions.len() as u32, point));
        if point {
            positions.push(vec3(0.0, y, cz));
            continue;
        }
        for s in 0..segments {
            let a = s as f32 / segments as f32 * TAU;
            positions.push(vec3(rx.max(0.0) * a.sin(), y, cz + rz.max(0.0) * a.cos()));
        }
    }
    let vertex = |j: usize, s: u32| {
        let (start, point) = starts[j];
        if point { start } else { start + s % segments }
    };
    let up = smooth[smooth.len() - 1].x > smooth[0].x;
    let mut indices = Vec::new();
    for j in 0..smooth.len() - 1 {
        for s in 0..segments {
            let (a, b) = (vertex(j, s), vertex(j, s + 1));
            let (c, d) = (vertex(j + 1, s), vertex(j + 1, s + 1));
            for [p, q, r] in [[a, b, c], [d, c, b]] {
                if p != q && q != r && p != r {
                    indices.extend(if up { [p, q, r] } else { [p, r, q] });
                }
            }
        }
    }
    Geometry::new(positions, Vec::new(), Vec::new(), indices)
}

/// The jacket from the hem to the collar, in root space.
const JACKET: [[f32; 4]; 14] = [
    [0.86, 0.0, 0.0, -0.01],
    [0.875, 0.15, 0.1, -0.01],
    [0.885, 0.186, 0.13, -0.012],
    [0.93, 0.186, 0.13, -0.012],
    [1.0, 0.172, 0.118, -0.004],
    [1.08, 0.163, 0.114, 0.002],
    [1.16, 0.168, 0.12, 0.008],
    [1.25, 0.18, 0.126, 0.012],
    [1.33, 0.19, 0.124, 0.008],
    [1.39, 0.198, 0.112, -0.002],
    [1.43, 0.172, 0.095, -0.01],
    [1.46, 0.12, 0.08, -0.015],
    [1.48, 0.07, 0.066, -0.018],
    [1.49, 0.0, 0.0, -0.018],
];

/// The seat of the trousers, from the crotch to the belt.
const SEAT: [[f32; 4]; 8] = [
    [0.805, 0.0, 0.0, -0.005],
    [0.815, 0.07, 0.06, -0.005],
    [0.84, 0.13, 0.1, -0.012],
    [0.89, 0.162, 0.11, -0.012],
    [0.95, 0.166, 0.112, -0.01],
    [1.0, 0.16, 0.11, -0.006],
    [1.04, 0.15, 0.104, 0.0],
    [1.055, 0.0, 0.0, 0.0],
];

/// A trouser leg from inside the seat to the hem over the boot. X and Z are relative to the
/// hip joint.
const LEG: [[f32; 4]; 13] = [
    [1.0, 0.0, 0.0, 0.0],
    [0.98, 0.07, 0.075, 0.0],
    [0.93, 0.082, 0.09, -0.005],
    [0.84, 0.084, 0.09, 0.0],
    [0.72, 0.078, 0.08, 0.006],
    [0.6, 0.067, 0.07, 0.008],
    [0.5, 0.061, 0.065, 0.008],
    [0.4, 0.061, 0.066, -0.004],
    [0.28, 0.057, 0.061, -0.002],
    [0.17, 0.056, 0.06, 0.0],
    [0.115, 0.058, 0.062, 0.002],
    [0.105, 0.052, 0.056, 0.002],
    [0.1, 0.0, 0.0, 0.0],
];

/// A sleeve from the top of the shoulder to the cuff. X and Z are relative to the shoulder
/// joint.
const SLEEVE: [[f32; 4]; 12] = [
    [1.432, 0.0, 0.0, 0.0],
    [1.428, 0.035, 0.038, 0.0],
    [1.415, 0.054, 0.057, 0.0],
    [1.39, 0.061, 0.063, 0.0],
    [1.35, 0.06, 0.062, 0.0],
    [1.26, 0.054, 0.056, 0.0],
    [1.14, 0.05, 0.052, 0.0],
    [1.03, 0.049, 0.05, 0.0],
    [0.93, 0.045, 0.046, 0.0],
    [0.875, 0.044, 0.045, 0.0],
    [0.865, 0.037, 0.038, 0.0],
    [0.862, 0.0, 0.0, 0.0],
];

/// The neck from inside the jacket to inside the head.
const NECK: [[f32; 4]; 6] = [
    [1.4, 0.0, 0.0, -0.02],
    [1.42, 0.06, 0.06, -0.02],
    [1.5, 0.058, 0.06, -0.016],
    [1.57, 0.055, 0.057, -0.006],
    [1.62, 0.05, 0.052, -0.002],
    [1.64, 0.0, 0.0, 0.0],
];

/// The face and the jaw, from below the chin to the forehead.
const FACE: [[f32; 4]; 9] = [
    [1.534, 0.0, 0.0, 0.062],
    [1.54, 0.02, 0.02, 0.068],
    [1.552, 0.04, 0.038, 0.058],
    [1.572, 0.058, 0.062, 0.034],
    [1.6, 0.068, 0.076, 0.02],
    [1.63, 0.073, 0.084, 0.01],
    [1.655, 0.07, 0.078, 0.008],
    [1.67, 0.06, 0.06, 0.008],
    [1.68, 0.0, 0.0, 0.006],
];

/// The nose, a small wedge on the face.
const NOSE: [[f32; 4]; 6] = [
    [1.603, 0.0, 0.0, 0.098],
    [1.607, 0.015, 0.009, 0.1],
    [1.616, 0.014, 0.012, 0.103],
    [1.635, 0.009, 0.01, 0.095],
    [1.655, 0.006, 0.006, 0.09],
    [1.66, 0.0, 0.0, 0.088],
];

/// The palm in the space of the wrist, from inside the cuff to the knuckles.
const PALM: [[f32; 4]; 8] = [
    [0.045, 0.0, 0.0, 0.0],
    [0.035, 0.022, 0.026, 0.0],
    [0.0, 0.021, 0.028, 0.0],
    [-0.03, 0.018, 0.035, 0.0],
    [-0.06, 0.016, 0.04, 0.0],
    [-0.083, 0.014, 0.041, 0.0],
    [-0.094, 0.01, 0.036, 0.0],
    [-0.099, 0.0, 0.0, 0.0],
];

/// The upper of a boot along the foot, in the space of the ankle: `[distance along +Z, half
/// width, half height, depth of the center below the ankle]`. The rows are rings of a
/// [`loft`] that turns from Y to Z.
const BOOT: [[f32; 4]; 12] = [
    [-0.09, 0.0, 0.0, 0.04],
    [-0.084, 0.028, 0.032, 0.043],
    [-0.068, 0.036, 0.046, 0.029],
    [-0.035, 0.04, 0.06, 0.015],
    [0.0, 0.042, 0.064, 0.011],
    [0.04, 0.046, 0.056, 0.019],
    [0.085, 0.05, 0.042, 0.033],
    [0.13, 0.052, 0.033, 0.042],
    [0.17, 0.049, 0.028, 0.047],
    [0.2, 0.044, 0.024, 0.051],
    [0.217, 0.032, 0.018, 0.057],
    [0.226, 0.0, 0.0, 0.06],
];

/// The shaft of a boot around the ankle, in the space of the ankle.
const BOOT_SHAFT: [[f32; 4]; 6] = [
    [-0.04, 0.0, 0.0, -0.015],
    [-0.03, 0.044, 0.05, -0.015],
    [0.03, 0.046, 0.052, -0.012],
    [0.085, 0.048, 0.054, -0.01],
    [0.092, 0.04, 0.046, -0.01],
    [0.095, 0.0, 0.0, -0.01],
];

/// A boot without the shaft in the space of the ankle: the upper and a sole with a heel.
fn boot() -> Geometry {
    // The loft turns from +Y to +Z, and its Z (the height) to -Y.
    let upper = loft(&BOOT, 24).rotate_x(FRAC_PI_2);
    // The outline of the sole follows the upper with a small margin.
    let outline: Vec<Vec2> = BOOT
        .iter()
        .map(|r| vec2(r[1] + 0.004, r[0]))
        .chain(BOOT.iter().rev().map(|r| vec2(-r[1] - 0.004, r[0])))
        .collect();
    let sole = Geometry::extrude(&outline, 0.022, 0.004)
        .rotate_x(FRAC_PI_2)
        .translate(0.0, -0.084, 0.0);
    Geometry::merge([upper, sole])
}

/// The collar of the jacket, in root space. It is lower in front.
fn collar() -> Geometry {
    let profile = [
        vec2(0.086, 0.0),
        vec2(0.098, 0.006),
        vec2(0.078, 0.055),
        vec2(0.068, 0.058),
        vec2(0.072, 0.03),
        vec2(0.086, 0.0),
    ];
    Geometry::lathe(&profile, 32)
        .scale(1.0, 1.0, 1.12)
        .rotate_x(0.3)
        .translate(0.0, 1.455, -0.012)
}

/// The head without hair, in root space: the skull, the face, the nose and the ears.
fn head() -> Geometry {
    let skull = Geometry::sphere(1.0, 40, 24)
        .scale(0.076, 0.098, 0.099)
        .translate(0.0, 1.685, -0.008);
    let ear = Geometry::sphere(1.0, 16, 10)
        .scale(0.009, 0.026, 0.016)
        .rotate_x(-0.2);
    let ears = SIDE.map(|side| ear.clone().translate(side * 0.073, 1.63, -0.016));
    Geometry::merge(
        [skull, loft(&FACE, 32), loft(&NOSE, 12)]
            .into_iter()
            .chain(ears),
    )
}

/// Short hair: a shell over the skull. The hairline is high on the forehead and low on the
/// neck.
fn hair() -> Geometry {
    let mut cap = Geometry::sphere_partial(1.0, 40, 20, 0.0, 1.85);
    // The hair is thick on top and thin at the hairline, so the edge does not look like a
    // cap. The positions are on the unit sphere, so `y` is the cosine of the polar angle.
    for p in &mut cap.positions {
        *p *= 1.012 + 0.05 * smoothstep(1.85, 1.0, p.y.acos());
    }
    Geometry::new(cap.positions, Vec::new(), cap.uvs, cap.indices)
        .rotate_x(-0.4)
        .scale(0.076, 0.098, 0.099)
        .translate(0.0, 1.685, -0.008)
}

/// One segment of a finger along -Y, from its joint to the next joint. The round ends
/// overlap the neighbors, so the knuckles look closed.
fn finger_segment(length: f32, base: f32, tip: f32) -> Geometry {
    loft(
        &[
            [0.9 * base, 0.0, 0.0, 0.0],
            [0.55 * base, 0.8 * base, 0.85 * base, 0.0],
            [0.0, 0.95 * base, base, 0.0],
            [-length, 0.95 * tip, tip, 0.0],
            [-length - 0.55 * tip, 0.8 * tip, 0.85 * tip, 0.0],
            [-length - 0.9 * tip, 0.0, 0.0, 0.0],
        ],
        12,
    )
}
