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

use std::collections::HashMap;
use std::f32::consts::{FRAC_PI_2, PI, TAU};
use std::ops::{Index, IndexMut};
use std::sync::{Arc, OnceLock};

use fframes_3d::math::{hash, noise1, progress, smoothstep};
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

/// The parts of [`Pose::kneel`] at which the left foot, the right foot and the right knee
/// touch the ground.
pub const KNEEL_CONTACTS: [f32; 3] = [0.35, 0.7, 1.0];

/// The start of the line that [`Pose::draw`] draws, in root space, in meters.
pub const LINE_START: Vec3 = Vec3::new(-0.218, 0.0, 0.55);
/// The end of the line, beside the right knee. The line is 0.43 m long.
pub const LINE_END: Vec3 = Vec3::new(-0.247, 0.0, 0.12);

/// The point at the part `u` (`0..1`) of the line that [`Pose::draw`] draws, in root
/// space. A hand does not draw a straight line: the path bows 12 mm to the side and
/// wavers a little.
pub fn line_point(u: f32) -> Vec3 {
    let along = LINE_END - LINE_START;
    let side = along.cross(Vec3::Y).normalize();
    let bend = 0.012 * (PI * u).sin() + 0.003 * u * (3.1 * PI * u + 0.5).sin();
    LINE_START + along * u + side * bend
}

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

/// The size of a tile of the cloth textures, in meters. The UVs of a [`loft`] are in meters.
const TILE: f32 = 0.04;
/// The distance between two folds of a [`Fold`] band, in meters.
const FOLD_PERIOD: f32 = 0.024;

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

/// The hip to ankle distance of a standing leg, as a part of the leg length. The knees
/// then bend about 5 degrees.
const STAND_EXTENSION: f32 = 0.999;
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
const KNEEL_LEFT: (Vec3, f32) = (Vec3::new(0.17, 0.0, 0.36), 0.15);
/// Where he stands when the kneel begins, along Z from the root, in meters. The body goes
/// forward and down onto the knee, and the right foot steps back only a little.
const KNEEL_START: f32 = -0.25;
/// The turn of the right foot when he kneels, in radians.
const KNEEL_RIGHT_YAW: f32 = -0.06;
/// The angle of the right foot, heel up, when he kneels on its toes.
const TUCK: f32 = 1.2;
/// How far the ankles stand behind a railing, in meters. Then the arms on the rail are
/// almost straight.
const RAIL_DISTANCE: f32 = 0.58;
/// The distance of each hand on the rail from the middle, in meters.
const RAIL_HANDS: f32 = 0.22;
/// The middle of the palm surface in the space of the left wrist. The right hand mirrors X.
const PALM_CENTER: Vec3 = Vec3::new(-0.017, -0.055, 0.0);

const THUMB: usize = 0;
const INDEX: usize = 1;

/// The bend of the fingers of a relaxed hand: thumb, index, middle, ring, little finger.
const RELAXED: [f32; 5] = [0.2, 0.25, 0.32, 0.38, 0.45];
/// The bend of the fingers of a hand that hangs loose: the fingers curl more from the index
/// to the little finger.
const LOOSE: [f32; 5] = [0.3, 0.32, 0.42, 0.5, 0.6];

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
///
/// [`Figure::build`] adds surface detail to them. The jacket gets a canvas weave and the
/// trousers a denim weave: a normal map with occlusion and roughness maps. The hair gets a
/// normal map of strands. A material that has its own normal map keeps it. The sole is a
/// darker copy of `boots`, and the nails are a lighter and smoother copy of `skin`.
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
            // A warm rim on the skin, as light that goes through the edge of an ear or a
            // finger.
            skin: dark(0x2a1d17, 0.55).rim(Rgb::hex(0xc0705a) * 0.3, 2.5),
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
    /// The meshes of the skin and the nails.
    bare: Vec<NodeId>,
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
            scene.add_to(root, node)
        };
        // A rigid part, given in root space in the rest pose, moves with its joint.
        let rigid = |scene: &mut Scene, joint: Joint, geometry: Geometry, material: &Material| {
            let at = rest(joint);
            let geometry = geometry.translate(-at.x, -at.y, -at.z);
            scene.add_to(
                joints[joint as usize],
                Node::mesh(geometry, material.clone()),
            )
        };

        // Woven cloth, hair strands, a rubber sole and nails. A material that has its own
        // normal map keeps it.
        let detail = detail();
        let cloth = |material: &Material, maps: &[Arc<Texture>; 2], scale: f32| {
            let mut material = material.clone();
            if material.normal_map.is_none() {
                material.normal_map = Some(maps[0].clone());
                material.normal_scale = scale;
                material.metal_rough_map = Some(maps[1].clone());
                material.ao_map = Some(maps[1].clone());
            }
            material
        };
        let jacket_cloth = cloth(&style.jacket, &detail.canvas, 0.9);
        let denim = cloth(&style.trousers, &detail.denim, 0.8);
        let skin_material = cloth(&style.skin, &detail.skin, 1.0);
        let leather = cloth(&style.boots, &detail.leather, 0.9);
        let mut hair_material = style.hair.clone();
        if hair_material.normal_map.is_none() {
            hair_material = hair_material
                .normal_map(detail.strands.clone())
                .normal_scale(1.0);
        }
        let rubber = style
            .boots
            .clone()
            .color(style.boots.color * 0.45)
            .roughness(0.85);
        let nail = style
            .skin
            .clone()
            .color(style.skin.color * 1.35)
            .roughness(0.42);

        // The seams and the zip are dark lines on the jacket, as a shadow in a groove. They
        // are unlit, because a thin lit ridge flickers and escapes the shadow of the body.
        let seam = Material::unlit(style.jacket.color * 0.12);
        let jacket = crumpled(loft(&JACKET, 72, 0.01), Vec2::ZERO, 0.0018, 1);
        for (part, material) in [(jacket, &jacket_cloth), (seams(), &seam)] {
            let weights = chain_weights(
                &part,
                Vec3::Y,
                &[Pelvis, Spine, Chest],
                &[(rest(Spine), 0.08), (rest(Chest), 0.08)],
            );
            let weights = hem_follows_legs(&part, weights);
            skinned(scene, part, weights, material);
        }
        rigid(scene, Chest, collar(), &jacket_cloth);

        let neck = loft(&NECK, 24, 0.02);
        let weights = chain_weights(
            &neck,
            Vec3::Y,
            &[Chest, Neck, Head],
            &[(rest(Neck), 0.03), (rest(Head), 0.03)],
        );
        // The skin: the rim of [`Figure::set_rim`] does not change it.
        let mut bare = vec![
            skinned(scene, neck, weights, &skin_material),
            rigid(scene, Head, head(), &skin_material),
        ];
        rigid(scene, Head, hair(), &hair_material);
        rigid(scene, Pelvis, loft(&SEAT, 40, 0.02), &denim);

        for i in [LEFT, RIGHT] {
            let [clavicle, shoulder, elbow, _] = ARMS[i];
            let at = rest(shoulder);
            // Folds at the inside of the elbow.
            let elbow_folds = Fold {
                center: vec3(at.x, rest(elbow).y, at.z),
                side: Vec3::Z,
                width: 0.07,
                depth: 0.004,
            };
            let mut sleeve = crumpled(
                folded(
                    loft(&SLEEVE, 28, FOLD_PERIOD / 4.0).translate(at.x, 0.0, at.z),
                    &[elbow_folds],
                ),
                vec2(at.x, at.z),
                0.0022,
                3 + i as u32,
            );
            // The opening of the cuff around the wrist is in shadow.
            for (p, color) in sleeve.positions.iter().zip(&mut sleeve.colors) {
                let r = vec2(p.x - at.x, p.z - at.z).length();
                if p.y < CUFF + 0.0015 && r < 0.04 {
                    *color = (color.truncate() * 0.12).extend(1.0);
                }
            }
            let weights = chain_weights(
                &sleeve,
                Vec3::NEG_Y,
                &[clavicle, shoulder, elbow],
                &[(at + Vec3::Y * 0.03, 0.035), (rest(elbow), 0.045)],
            );
            skinned(scene, sleeve, weights, &jacket_cloth);
            // The sphere fills the outside of the elbow when the arm bends.
            let ball = Geometry::sphere(0.046, 16, 10).translate(at.x, rest(elbow).y, at.z);
            rigid(scene, elbow, ball, &jacket_cloth);

            let [hip, knee, ankle, toe] = LEGS[i];
            let at = rest(hip);
            // Folds behind the knee, above and below the knee in front, and where the
            // trousers break on the boot.
            let leg_folds = [
                Fold {
                    center: vec3(at.x, rest(knee).y + 0.02, at.z),
                    side: Vec3::NEG_Z,
                    width: 0.11,
                    depth: 0.009,
                },
                Fold {
                    center: vec3(at.x, rest(knee).y + 0.09, at.z),
                    side: Vec3::Z,
                    width: 0.04,
                    depth: 0.004,
                },
                Fold {
                    center: vec3(at.x, rest(knee).y - 0.08, at.z),
                    side: Vec3::Z,
                    width: 0.035,
                    depth: 0.003,
                },
                Fold {
                    center: vec3(at.x, 0.17, at.z),
                    side: Vec3::Z,
                    width: 0.05,
                    depth: 0.003,
                },
            ];
            let leg = worn(
                crumpled(
                    folded(
                        loft(&LEG, 28, FOLD_PERIOD / 4.0).translate(at.x, 0.0, at.z),
                        &leg_folds,
                    ),
                    vec2(at.x, at.z),
                    0.003,
                    5 + i as u32,
                ),
                vec2(at.x, at.z),
                i as u32,
            );
            let weights = chain_weights(
                &leg,
                Vec3::NEG_Y,
                &[Pelvis, hip, knee],
                &[(at + Vec3::Y * 0.03, 0.06), (rest(knee), 0.05)],
            );
            skinned(scene, leg, weights, &denim);
            let cap = knee_cap().translate(at.x, rest(knee).y, 0.006);
            rigid(scene, knee, cap, &denim);

            let at = rest(ankle);
            let boot = boot().translate(at.x, at.y, at.z);
            let weights = chain_weights(&boot, Vec3::Z, &[ankle, toe], &[(rest(toe), 0.025)]);
            skinned(scene, boot, weights, &leather);
            let sole = sole().translate(at.x, at.y, at.z);
            let weights = chain_weights(&sole, Vec3::Z, &[ankle, toe], &[(rest(toe), 0.025)]);
            skinned(scene, sole, weights, &rubber);
            // The top of the shaft follows the shin, so it stays inside the trousers.
            let shaft = loft(&BOOT_SHAFT, 24, 0.02).translate(at.x, at.y, at.z);
            let weights = chain_weights(
                &shaft,
                Vec3::NEG_Y,
                &[knee, ankle],
                &[(at + Vec3::Y * 0.01, 0.03)],
            );
            skinned(scene, shaft, weights, &leather);
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
                    offset = vec3(0.0, -finger.lengths[k], 0.0);
                }
            }
            // The palm and the fingers are one skinned mesh, so the skin bends smoothly at
            // the knuckles. The nails are a second mesh on the same joints.
            let wrist_rest = rest(ARMS[i][3]);
            let binds = finger_binds(side, wrist_rest);
            let mut hand_joints = vec![wrist];
            let mut inverse = vec![Mat4::from_translation(-wrist_rest)];
            for (f, chain) in binds.iter().enumerate() {
                for (k, bind) in chain.iter().enumerate() {
                    hand_joints.push(fingers[i][f][k]);
                    inverse.push(bind.inverse());
                }
            }
            let hand_skin = Skin {
                joints: hand_joints,
                inverse_bind_matrices: Arc::new(inverse),
            };
            let (skin_part, nails) = hand(side, wrist_rest, &binds);
            for (geometry, material) in [(skin_part, &skin_material), (nails, &nail)] {
                let mut node = Node::mesh(geometry, material.clone());
                if let Some(mesh) = node.mesh_mut() {
                    mesh.skin = Some(hand_skin.clone());
                }
                bare.push(scene.add_to(root, node));
            }
        }

        Figure {
            root,
            joints,
            fingers,
            bare,
        }
    }

    /// Sets the rim light of the clothes and the hair that have one in the [`FigureStyle`]:
    /// `color` and the falloff `power`. A shot uses it when the light behind the figure
    /// changes. The skin keeps its rim: on the nose and the chin a strong rim draws lines.
    pub fn set_rim(&self, scene: &mut Scene, color: Rgb, power: f32) {
        for id in scene.descendants(self.root) {
            if self.bare.contains(&id) {
                continue;
            }
            if let Some(material) = scene[id].material_mut()
                && material.rim != Rgb::BLACK
            {
                material.rim = color;
                material.rim_power = power;
            }
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

    /// The tip of the right index finger in world space. [`Pose::draw`] moves it along
    /// [`line_point`].
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

    /// The middle of the left and the right palm surface, in world space. [`Pose::at_rail`]
    /// puts them on the rail.
    pub fn palms(&self, scene: &Scene) -> [Vec3; 2] {
        [LEFT, RIGHT].map(|i| {
            let palm = PALM_CENTER * vec3(SIDE[i], 1.0, 1.0);
            scene.local_to_world(self.joint(ARMS[i][3]), palm)
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
    /// At 1 m/s a step is 0.6 m long, and he takes 100 steps a minute.
    pub fn stride(speed: f32) -> f32 {
        2.0 * (0.36 + 0.24 * speed.clamp(0.3, 2.0))
    }

    /// A walk along +Z at `speed` meters per second. `phase` counts gait cycles: the right
    /// heel strikes at each whole number, the left heel half a cycle later. The root moves
    /// `phase * Pose::stride(speed)` meters, so a planted foot stays in place.
    ///
    /// The motion follows the joint curves of a slow, tired walk. The pelvis turns, drops
    /// on the side of the swing leg and moves over the standing foot. It is lowest just
    /// after a heel strike, when the knee of the new standing leg bends to take the weight.
    /// The chest turns against the pelvis. The arms swing late and loose, and the head is
    /// steadier than the body. Each step is a little different.
    pub fn walk(phase: f32, speed: f32) -> Pose {
        let stride = Self::stride(speed);
        let amount = (speed / 1.3).clamp(0.3, 1.4);
        let a = TAU * phase;
        // A value in -1..1 that changes from step to step.
        let vary = |seed: u32| noise1(seed, 2.0 * phase);
        let mut pose = Pose {
            position: vec3(0.0, 0.0, phase * stride),
            fingers: [LEFT, RIGHT].map(|i| {
                std::array::from_fn(|f| {
                    let seed = 40 + 5 * i as u32 + f as u32;
                    LOOSE[f] + 0.06 * noise1(seed, 1.3 * phase)
                })
            }),
            ..Pose::default()
        };

        // The pelvis turns forward on the side of the swing leg, drops on that side early in
        // the stance and moves over the standing foot. It tips forward a little after each
        // heel strike. The body is slowest over the standing foot.
        let yaw = 0.075 * amount * (a - 0.15).cos() * (1.0 + 0.15 * vary(11));
        let list = -0.06 * (a + 0.35).sin() * (1.0 + 0.2 * vary(12));
        let tilt = 0.03 + 0.02 * (2.0 * a - 0.6).cos();
        pose[Pelvis] =
            Quat::from_rotation_y(yaw) * Quat::from_rotation_x(tilt) * Quat::from_rotation_z(list);
        pose.pelvis.x = -0.026 * (a - 0.3).sin() * (1.0 + 0.15 * vary(13));
        pose.pelvis.z = 0.01 * amount * (2.0 * a).sin();
        let feet = [phase - 0.5, phase].map(|cycle| {
            let side = if cycle < phase { 1.0 } else { -1.0 };
            let mut foot = walk_foot(cycle, stride, side);
            foot.ankle.z -= cycle * stride;
            foot
        });
        // The pelvis is highest over the standing foot and lowest just after each heel
        // strike. It never goes higher than the legs reach, so the knees do not lock.
        let bob = 0.022 * amount * (1.0 + 0.2 * vary(14));
        let height = -0.014 + bob * (2.0 * a - TAU * 0.56).cos();
        pose.pelvis.y = smooth_min(height, pose.pelvis_reach(&feet, 0.999, 0.0), 0.02);
        pose.solve_legs(&feet, [vec3(0.1, 0.0, 1.0), vec3(-0.1, 0.0, 1.0)]);

        // The chest turns against the pelvis, a little later, with the arms. The spine bends
        // against the drop of the pelvis and leans over the standing foot. He walks a little
        // bent forward: he is tired.
        let turn = -0.06 * amount * (a - 0.4).cos() - yaw;
        let lean = 0.02 * (a - 0.35).sin();
        let bend = 0.015 * (2.0 * a - 0.9).cos();
        pose[Spine] = euler(0.05 + 0.4 * bend, 0.5 * turn, -0.6 * list + lean);
        pose[Chest] = euler(0.09 + 0.6 * bend, 0.5 * turn, -0.3 * list + 0.5 * lean);

        // The arms swing against the legs. Each arm is most forward a little after the heel
        // strike of the other foot. The elbow bends later than the shoulder swings, and the
        // hand trails the forearm. The shoulders hang forward, and the right arm swings a
        // little more than the left.
        for i in [LEFT, RIGHT] {
            let side = SIDE[i];
            let [clavicle, shoulder, elbow, wrist] = ARMS[i];
            let center = 0.09 + 0.5 * i as f32;
            let wave = |lag: f32| {
                let x = TAU * (phase - center - lag);
                x.cos() + 0.12 * (2.0 * x - 0.8).cos()
            };
            let swing = 0.27 * amount * (1.0 + 0.15 * vary(20 + i as u32)) * (1.0 - 0.06 * side);
            pose[clavicle] = euler(0.0, -side * (0.1 + 0.05 * wave(0.0)), -side * 0.03);
            pose[shoulder] = euler(
                0.05 - swing * wave(0.0),
                -side * 0.25,
                side * (0.11 - 0.03 * wave(0.0)),
            );
            pose[elbow] = euler(-(0.25 + 0.35 * amount * (0.5 + 0.5 * wave(0.07))), 0.0, 0.0);
            let trail = (TAU * (phase - center - 0.1)).sin();
            pose[wrist] = euler(-0.15 * trail, 0.0, -side * 0.15);
        }

        // The head looks a little down at the ground ahead. It turns only a part of the
        // way with the chest, and slowly looks around.
        let look = euler(
            0.14 + 0.04 * noise1(51, 0.5 * phase),
            0.06 * noise1(52, 0.35 * phase),
            0.0,
        );
        pose.steady_head(look, 0.3);
        pose
    }

    /// He kneels on the right knee. `t` goes from 0 ([`Pose::stand`] at time 0) to 1, when
    /// the knee touches the ground, see [`KNEEL_CONTACTS`]. The film plays it over about 1.5
    /// s.
    ///
    /// First his weight goes onto the right foot, and the left foot steps forward. Then the
    /// weight goes forward onto the left foot, the right foot goes back onto its toes and
    /// the body goes down. He leans forward to keep his balance: the left hand reaches for
    /// the left knee, and the right arm comes out a little. The knee touches the ground
    /// softly. From 1 to about 1.5 the body settles: it sinks back a little and comes
    /// forward again, and he breathes out.
    pub fn kneel(t: f32) -> Pose {
        let settle = settle(t - 1.0);
        let t = t.clamp(0.0, 1.0);
        let back = vec3(0.0, 0.0, KNEEL_START);
        let mut start = Pose::stand(0.0);
        start.pelvis += back;
        let end = kneeling(0.1 - 0.06 * settle, 0.12 + 0.05 * settle);
        // One foot steps at a time, so the other foot carries the weight.
        let [left_down, right_down, knee_down] = KNEEL_CONTACTS;
        let left_step = smoothstep(0.0, left_down, t);
        let right_step = smoothstep(left_down, right_down, t);
        // The body goes down slowly at the end, so the knee touches the ground softly.
        let down = smoothstep(0.35, knee_down, t);
        let ahead = smoothstep(0.05, 0.5, t);
        let upper = smoothstep(0.15, 0.9, t);

        let mut pose = start.clone();
        for joint in UPPER_BODY {
            pose[joint] = start[joint].slerp(end[joint], upper);
        }
        for (hand, end_hand) in pose.fingers.iter_mut().zip(end.fingers) {
            for (curl, end_curl) in hand.iter_mut().zip(end_hand) {
                *curl += (end_curl - *curl) * upper;
            }
        }
        // He leans forward while he goes down, and a little again when he settles.
        let lean = 0.16 * bump(t, 0.35, 0.75, 1.0) + 0.06 * settle;
        pose[Spine] *= Quat::from_rotation_x(0.4 * lean);
        pose[Chest] *= Quat::from_rotation_x(0.6 * lean + 0.03 * settle);
        // The right arm comes forward and out for balance.
        let balance = bump(t, 0.3, 0.65, 1.0);
        pose[ShoulderR] *= euler(-0.2 * balance, 0.0, -0.22 * balance);
        pose[ElbowR] *= Quat::from_rotation_x(-0.3 * balance);
        pose[Pelvis] = start[Pelvis].slerp(end[Pelvis], down);
        pose.pelvis = start.pelvis.lerp(end.pelvis, ahead);
        // The weight goes onto the right foot while the left foot steps, then onto the left
        // foot while the body goes down.
        pose.pelvis.x += -0.03 * bump(t, 0.0, 0.18, left_down) + 0.035 * bump(t, 0.3, 0.6, 1.0);

        let (left_heel, left_yaw) = STAND_FEET[LEFT];
        let (right_heel, right_yaw) = STAND_FEET[RIGHT];
        // A foot that steps lifts first and stops before it touches down.
        let reach = |step: f32| smoothstep(0.0, 0.8, step);
        let mut left = planted(
            (left_heel + back).lerp(KNEEL_LEFT.0, reach(left_step)),
            left_yaw + (KNEEL_LEFT.1 - left_yaw) * reach(left_step),
            0.0,
        );
        left.ankle.y += 0.08 * (PI * left_step).sin();
        let mut right = planted(
            (right_heel + back).lerp(kneeling_heel(), reach(right_step)),
            right_yaw + (KNEEL_RIGHT_YAW - right_yaw) * reach(right_step),
            -TUCK * right_step,
        );
        right.ankle.y += 0.04 * (PI * right_step).sin();
        let feet = [left, right];

        let height = start.pelvis.y + (end.pelvis.y - start.pelvis.y) * down;
        pose.pelvis.y = height.min(pose.pelvis_reach(&feet, STAND_EXTENSION, 0.0));
        let hip = pose.hip(RIGHT);
        let knee = KNEE_POLES[RIGHT]
            .normalize()
            .lerp((KNEE_DOWN - hip).normalize(), right_step);
        pose.solve_legs(&feet, [KNEE_POLES[LEFT], knee]);
        pose
    }

    /// He kneels, leans forward and draws a line in the dust with the right index finger.
    /// The tip of the finger is `lift` meters above [`line_point`]`(t)`, see
    /// [`Figure::fingertip`]. `time` in seconds moves the breath and the small corrections
    /// of the hand.
    ///
    /// The index finger points, and the other fingers curl loosely. The finger trails the
    /// wrist, most in the middle of the stroke. The eyes go ahead of the hand, and the hips
    /// and the shoulder follow it late. The left hand rests on the left knee.
    pub fn draw(t: f32, lift: f32, time: f32) -> Pose {
        let t = t.clamp(0.0, 1.0);
        let tip = line_point(t) + Vec3::Y * (0.004 + lift);
        // The fingers curl more when the hand lifts from the dust.
        let relax = (lift / 0.03).clamp(0.0, 1.0);
        let fingers = [
            0.55 + 0.1 * relax,
            0.1 + 0.3 * relax,
            0.62 + 0.1 * relax,
            0.72 + 0.08 * relax,
            0.8 + 0.06 * relax,
        ];
        // The finger points down and back along the line, so it drags in the dust. Small
        // corrections turn the hand.
        let back = (LINE_START - LINE_END).normalize();
        let across = back.cross(Vec3::Y);
        let drag = 0.9 + 0.3 * (PI * t).sin();
        let hand = hand_rotation(
            back * drag + across * (0.08 * noise1(61, 2.2 * time)) + Vec3::NEG_Y,
            Vec3::X + back * (0.15 * noise1(62, 1.7 * time)),
        );
        let wrist = tip - hand * index_tip_in_wrist(SIDE[RIGHT], fingers[INDEX]);

        // He bends from the hips, with a little bend of the back to the right. The hips
        // start forward over the right knee and rock back as the finger comes closer, a
        // little later than the hand. He bends only as far as the arm needs to reach the
        // wrist. The right shoulder comes forward and down with the bend.
        let body = t - 0.1 * (PI * t).sin();
        let forward = 0.65 - 0.9 * body;
        let breath = breath(time);
        let bent = |bend: f32| {
            let mut pose = kneeling(forward, 0.12 + 0.7 * bend);
            pose[Spine] = euler(0.15 * bend, -0.05, 0.06 * bend);
            pose[Chest] = euler(0.15 * bend - 0.015 * breath, -0.1, 0.06 * bend);
            pose[ClavicleR] = euler(0.0, 0.2 * bend, 0.12 * bend);
            pose[ClavicleL] = Quat::from_rotation_z(0.012 * breath);
            pose
        };
        let bend = least(1.4, |bend| {
            let shoulder = bent(bend).joint_matrices()[ShoulderR as usize].w_axis;
            shoulder.truncate().distance(wrist) <= 0.95 * (UPPER_ARM + FOREARM)
        });
        let mut pose = bent(bend);
        pose.fingers[RIGHT] = fingers;
        pose.reach(RIGHT, wrist, hand, vec3(-0.8, 0.3, -0.5));
        pose.rest_hand_on_thigh(1.2);
        // The eyes go ahead of the finger along the line.
        pose.look_at(line_point((t + 0.12).min(1.0)) + back * (0.02 * noise1(63, 0.9 * time)));
        pose
    }

    /// He stands at a railing with both hands on its top and looks down over it.
    ///
    /// `rail` is the middle of the top of the railing in root space. The railing runs along X
    /// in front of him. He stands back from the railing, so the arms are almost straight.
    /// Thus the feet can be behind the root. He leans from the hips only as far as the arms
    /// need. The shoulders rise a little, and the head looks about 20 degrees down. He
    /// breathes and moves his weight as in [`Pose::stand`]. `time` is in seconds. The palms
    /// stay on the rail, see [`Figure::palms`].
    pub fn at_rail(time: f32, rail: Vec3) -> Pose {
        let stand = Pose::stand(time);
        let back = vec3(0.0, 0.0, rail.z - RAIL_DISTANCE);
        let feet = STAND_FEET.map(|(heel, yaw)| planted(heel + back, yaw, 0.0));
        // The palms are on the top of the rail. The fingers point forward and a little in,
        // and they hang over the front edge.
        let hands = [LEFT, RIGHT].map(|i| {
            let side = SIDE[i];
            let hand = hand_rotation(vec3(-side * 0.25, 0.0, 1.0), Vec3::X * -side);
            let palm = vec3(side * RAIL_HANDS, rail.y, rail.z);
            (palm - hand * (PALM_CENTER * vec3(side, 1.0, 1.0)), hand)
        });
        // The shoulders rise and come forward a little.
        let leaning = |lean: f32| {
            let mut pose = stand.clone();
            pose.pelvis += back;
            pose[Pelvis] = euler(lean, 0.0, 0.0) * stand[Pelvis];
            pose.pelvis.y = pose.pelvis_reach(&feet, STAND_EXTENSION, 0.0);
            for i in [LEFT, RIGHT] {
                pose[ARMS[i][0]] *= euler(0.0, -SIDE[i] * 0.05, SIDE[i] * 0.08);
            }
            pose
        };
        let lean = least(0.6, |lean| {
            let m = leaning(lean).joint_matrices();
            [LEFT, RIGHT].into_iter().all(|i| {
                let shoulder = m[ARMS[i][1] as usize].w_axis.truncate();
                shoulder.distance(hands[i].0) <= 0.985 * (UPPER_ARM + FOREARM)
            })
        });
        let mut pose = leaning(lean);
        pose.solve_legs(&feet, KNEE_POLES);
        for (i, (wrist, hand)) in hands.into_iter().enumerate() {
            pose.reach(i, wrist, hand, vec3(SIDE[i] * 0.6, 0.0, -1.0));
        }
        // The head looks 20 degrees down, at the floor far below. The neck does 40 % of the
        // turn.
        let forward = pose.joint_matrices()[Head as usize].transform_vector3(Vec3::Z);
        let turn = deg(20.0) + forward.y.asin();
        pose[Neck] *= Quat::from_rotation_x(0.4 * turn);
        pose[Head] *= Quat::from_rotation_x(0.6 * turn);
        pose
    }

    /// Interpolates the place, the joints and the fingers, then puts the lowest point of
    /// the feet and the knees on the ground.
    pub fn lerp(&self, other: &Pose, t: f32) -> Pose {
        let turn = (other.heading - self.heading + PI).rem_euclid(TAU) - PI;
        let mut pose = Pose {
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
        // Each foot goes straight from its place in one pose to its place in the other, and
        // turns around its heel. Thus a foot whose heel stands in the same place in both
        // poses does not slide. The knees keep the side to which they point in the blend.
        let world = |p: &Pose, foot: Foot| {
            let turn = Quat::from_rotation_y(p.heading);
            let rotation = turn * foot.rotation;
            (p.position + turn * foot.ankle + rotation * HEEL, rotation)
        };
        let to_root = Quat::from_rotation_y(pose.heading).inverse();
        let (from, to) = (self.feet(), other.feet());
        let feet = [LEFT, RIGHT].map(|i| {
            let (a, ra) = world(self, from[i]);
            let (b, rb) = world(other, to[i]);
            let rotation = ra.slerp(rb, t);
            Foot {
                ankle: to_root * (a.lerp(b, t) - rotation * HEEL - pose.position),
                rotation: to_root * rotation,
                toes: from[i].toes + (to[i].toes - from[i].toes) * t,
            }
        });
        let m = pose.joint_matrices();
        let poles = LEGS.map(|[hip, knee, _, _]| {
            m[knee as usize].w_axis.truncate() - m[hip as usize].w_axis.truncate() + Vec3::Z * 0.02
        });
        pose.solve_legs(&feet, poles);
        pose.grounded()
    }

    /// The places of the feet in root space.
    fn feet(&self) -> [Foot; 2] {
        let m = self.joint_matrices();
        LEGS.map(|[_, _, ankle, toe]| {
            let toes = self[toe];
            Foot {
                ankle: m[ankle as usize].w_axis.truncate(),
                rotation: Quat::from_mat4(&m[ankle as usize]),
                toes: 2.0 * toes.x.atan2(toes.w),
            }
        })
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

    /// The back of the heel of leg `i` ([`LEFT`] or [`RIGHT`]) on the sole, in root space
    /// before [`Pose::placed`]: relative to `position`, not turned by `heading`.
    pub fn heel(&self, i: usize) -> Vec3 {
        self.joint_matrices()[LEGS[i][2] as usize].transform_point3(HEEL)
    }

    /// The idle stance at `time` seconds. `look` lifts the chest and the head, `0..1`. He
    /// breathes, his weight moves slowly from one leg to the other, he sways a little over
    /// the ankles, and the arms and the fingers are never quite still.
    fn standing(time: f32, look: f32) -> Pose {
        let breath = breath(time);
        // Positive puts the weight on the left leg.
        let weight = 0.7 * (TAU * time / 11.0).sin() + 0.3 * noise1(3, 0.2 * time);
        let sway = 0.008 * noise1(4, 0.35 * time);
        let mut pose = Pose {
            pelvis: vec3(0.02 * weight, 0.0, 0.02 * look + sway),
            fingers: [LEFT, RIGHT].map(|i| {
                std::array::from_fn(|f| {
                    let seed = 80 + 5 * i as u32 + f as u32;
                    LOOSE[f] + 0.05 * noise1(seed, 0.3 * time)
                })
            }),
            ..Pose::default()
        };
        pose[Pelvis] = euler(-0.03 * look, 0.03 * noise1(13, 0.1 * time), 0.03 * weight);
        let feet = STAND_FEET.map(|(heel, yaw)| planted(heel, yaw, 0.0));
        pose.pelvis.y = pose.pelvis_reach(&feet, STAND_EXTENSION, 0.0);
        pose.solve_legs(&feet, KNEE_POLES);

        // The spine keeps the shoulders level, and the chest rises when he breathes in.
        pose[Spine] = euler(
            -0.02 - 0.05 * look,
            -0.02 * noise1(13, 0.1 * time),
            -0.02 * weight,
        );
        pose[Chest] = euler(0.02 - 0.015 * breath - 0.1 * look, 0.0, -0.012 * weight);
        pose[Neck] = euler(0.06 - 0.25 * look, 0.0, 0.0);
        pose[Head] = euler(
            0.03 + 0.03 * noise1(6, 0.2 * time) - 0.4 * look,
            0.08 * noise1(5, 0.25 * time),
            0.02 * noise1(7, 0.15 * time),
        );
        for i in [LEFT, RIGHT] {
            let side = SIDE[i];
            let [clavicle, shoulder, elbow, wrist] = ARMS[i];
            let drift = |seed: u32, rate: f32| noise1(seed + 10 * i as u32, rate * time);
            pose[clavicle] = euler(0.0, side * 0.06 * look, side * 0.012 * breath);
            pose[shoulder] = euler(
                0.03 + 0.02 * drift(20, 0.3) + 0.005 * breath,
                -side * 0.2,
                side * (0.11 + 0.01 * drift(21, 0.2)),
            );
            pose[elbow] = euler(-0.2 - 0.05 * drift(22, 0.25), 0.0, 0.0);
            pose[wrist] = euler(0.04 * drift(23, 0.3), 0.0, -side * 0.12);
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
    /// `blend` is the height range in meters over which the two legs share the limit. It
    /// keeps the pelvis smooth in a walk, but it also lowers the pelvis by up to `blend / 4`.
    fn pelvis_reach(&self, feet: &[Foot; 2], extension: f32, blend: f32) -> f32 {
        let reach = (THIGH + SHIN) * extension;
        let [left, right] = [LEFT, RIGHT].map(|i| {
            let hip = self.hip(i) - Vec3::Y * self.pelvis.y;
            let flat = (feet[i].ankle - hip).with_y(0.0).length().min(reach);
            feet[i].ankle.y + (reach * reach - flat * flat).sqrt() - hip.y
        });
        smooth_min(left, right, blend)
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

    /// Turns the neck and the head so that the head has the rotation `level` in root
    /// space, and follows the turn of the chest only for the part `follow`. The neck does 40
    /// % of the turn. A head that does not follow the chest is steadier than the body.
    fn steady_head(&mut self, level: Quat, follow: f32) {
        let chest = Quat::from_mat4(&self.joint_matrices()[Chest as usize]);
        let turn = chest.inverse() * level.slerp(chest, follow);
        self[Neck] = Quat::IDENTITY.slerp(turn, 0.4);
        self[Head] = self[Neck].inverse() * turn;
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
    // Each step lands a little differently: a value in -1..1 for each step and foot.
    let vary = |step: f32, seed: u32| {
        let n = (step as i32 as u32).wrapping_mul(2) + u32::from(side > 0.0);
        hash(n.wrapping_mul(7919) ^ seed.wrapping_mul(104_729)) * 2.0 - 1.0
    };
    let yaw = |step: f32| side * 0.08 + 0.035 * vary(step, 1);
    // The ankle is below the hip in the middle of the stance.
    let heel = |step: f32| {
        vec3(
            side * (0.075 + 0.01 * vary(step, 2)),
            0.0,
            (step + 0.25) * stride + HEEL.z,
        )
    };
    // Heel strike, then the foot rolls flat, then the heel lifts until the toes leave.
    let stance = |f: f32, step: f32| {
        let roll = if f < FOOT_FLAT {
            STRIKE * (1.0 - smoothstep(0.0, FOOT_FLAT, f))
        } else {
            -TOE_OFF * progress(f, HEEL_OFF, STANCE).powi(2)
        };
        planted(heel(step), yaw(step), roll)
    };
    let step = cycle.floor();
    let f = cycle - step;
    if f < STANCE {
        return stance(f, step);
    }

    // The swing: the foot lifts, goes forward and turns from toes down to toes up. A tired
    // foot lifts little.
    let s = (f - STANCE) / (1.0 - STANCE);
    let from = stance(STANCE, step);
    let to = stance(0.0, step + 1.0);
    let lift = 0.06 + 0.01 * vary(step, 3);
    let mut ankle = from.ankle.lerp(to.ankle, smoothstep(0.0, 1.0, s));
    ankle.y = from.ankle.y + (to.ankle.y - from.ankle.y) * s + lift * (PI * s.sqrt()).sin().powi(2);
    let pitch = -TOE_OFF + (STRIKE + TOE_OFF) * (1.0 - (1.0 - s).powi(2));
    let mut foot = Foot {
        ankle,
        rotation: Quat::from_rotation_y(yaw(step) + (yaw(step + 1.0) - yaw(step)) * s)
            * Quat::from_rotation_x(-pitch),
        toes: -TOE_OFF * (1.0 - smoothstep(0.0, 0.4, s)),
    };
    // The sole keeps a small distance from the ground.
    let clearance = 0.008 * (PI * s).sin();
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

/// The smallest value in `0..=max` for which `enough` is true, found by bisection.
/// `enough` must be false below a value and true above it. Returns `max` when `enough(max)`
/// is false.
fn least(max: f32, enough: impl Fn(f32) -> bool) -> f32 {
    let (mut low, mut high) = (0.0, max);
    for _ in 0..20 {
        let mid = f32::midpoint(low, high);
        if enough(mid) {
            high = mid;
        } else {
            low = mid;
        }
    }
    high
}

/// A smooth minimum of `a` and `b` that is never above `min(a, b)`. `k` is the width of
/// the blend. With `k` = 0 it is `min(a, b)`.
fn smooth_min(a: f32, b: f32, k: f32) -> f32 {
    if k <= 0.0 {
        return a.min(b);
    }
    let h = (0.5 + 0.5 * (b - a) / k).clamp(0.0, 1.0);
    b + (a - b) * h - k * h * (1.0 - h)
}

/// The breath at `time` seconds, from -1 (breathed out) to 1 (breathed in). He breathes
/// about 14 times a minute, a little irregularly. He breathes in for 40 % of a breath and
/// out for the rest.
fn breath(time: f32) -> f32 {
    let x = (time / 4.3 + 0.08 * noise1(71, 0.15 * time)).fract();
    if x < 0.4 {
        -(PI * x / 0.4).cos()
    } else {
        (PI * (x - 0.4) / 0.6).cos()
    }
}

/// A smooth bump: 0 before `start`, 1 at `peak` and 0 again after `end`.
fn bump(t: f32, start: f32, peak: f32, end: f32) -> f32 {
    smoothstep(start, peak, t) * (1.0 - smoothstep(peak, end, t))
}

/// The settle after a contact, `s` seconds of the motion after it: 0 before the contact,
/// up to 1 after 0.12, and back to 0 within about 0.5.
fn settle(s: f32) -> f32 {
    let k = (s / 0.12).max(0.0);
    k * (1.0 - k).exp()
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

/// Lets the hem of the jacket follow the thighs a little: the lowest 10 cm in front and
/// on the sides move up to 22 % with the hip joint of their side. So the hem swings and
/// lifts with each step, and the jacket does not move as one rigid block.
fn hem_follows_legs(geometry: &Geometry, mut weights: SkinWeights) -> SkinWeights {
    for ((p, joints), w) in geometry
        .positions
        .iter()
        .zip(&mut weights.joints)
        .zip(&mut weights.weights)
    {
        let low = smoothstep(0.98, 0.87, p.y);
        let side = (p.x.abs() / 0.12).min(1.0) * smoothstep(-0.06, 0.04, p.z);
        let hip = 0.22 * low * side;
        if hip <= 0.0 {
            continue;
        }
        for v in &mut w[..2] {
            *v *= 1.0 - hip;
        }
        joints[2] = if p.x > 0.0 { HipL } else { HipR } as u16;
        w[2] = hip;
    }
    weights
}

/// A smooth closed tube through elliptic cross-sections stacked along Y.
///
/// Each ring is `[y, half width along X, half depth along Z, center on Z]`. A ring of zero
/// size is one point that closes the tube. A Catmull-Rom spline through the rings adds
/// rings: at least one between two rings, and at most `spacing` meters apart along Y.
///
/// The UVs are in meters: U goes around the tube from the back (-Z), and V goes along it.
/// Each ring has a whole number of texture tiles ([`TILE`]) around, so the texture has no
/// seam.
fn loft(rings: &[[f32; 4]], segments: u32, spacing: f32) -> Geometry {
    let last = rings.len() - 1;
    let ring = |i: usize| Vec4::from_array(rings[i.min(last)]);
    let mut smooth = Vec::new();
    for i in 0..last {
        let (p0, p1, p2, p3) = (ring(i.saturating_sub(1)), ring(i), ring(i + 1), ring(i + 2));
        let steps = (((p2.x - p1.x).abs() / spacing).ceil() as usize).max(2);
        for k in 0..steps {
            let t = k as f32 / steps as f32;
            let (t2, t3) = (t * t, t * t * t);
            let mut r = 0.5
                * (2.0 * p1
                    + (p2 - p0) * t
                    + (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * t2
                    + (3.0 * p1 - p0 - 3.0 * p2 + p3) * t3);
            // Y is linear: the spline can overshoot in Y where the rings are not evenly
            // spaced, and then the tube folds over itself.
            r.x = p1.x + (p2.x - p1.x) * t;
            smooth.push(r);
        }
    }
    smooth.push(ring(last));

    // The first vertex of each ring, and whether the ring is a single point. A full ring
    // has `segments + 1` vertices: the last one is a copy of the first with a larger U.
    let mut starts = Vec::with_capacity(smooth.len());
    let mut positions = Vec::new();
    let mut uvs = Vec::new();
    let mut along = 0.0;
    for (j, r) in smooth.iter().enumerate() {
        let [y, rx, rz, cz] = r.to_array();
        if j > 0 {
            let [py, prx, prz, pcz] = smooth[j - 1].to_array();
            along += vec3(y - py, 0.5 * (rx + rz - prx - prz), cz - pcz).length();
        }
        let point = rx <= 1e-5 && rz <= 1e-5;
        starts.push((positions.len() as u32, point));
        if point {
            positions.push(vec3(0.0, y, cz));
            uvs.push(vec2(0.0, along));
            continue;
        }
        let mut ring: Vec<Vec3> = (0..segments)
            .map(|s| {
                let a = PI + s as f32 / segments as f32 * TAU;
                vec3(rx.max(0.0) * a.sin(), y, cz + rz.max(0.0) * a.cos())
            })
            .collect();
        ring.push(ring[0]);
        let mut around = vec![0.0];
        for pair in ring.windows(2) {
            around.push(around[around.len() - 1] + pair[0].distance(pair[1]));
        }
        let perimeter = around[around.len() - 1].max(1e-6);
        let tiles = (perimeter / TILE).round().max(1.0) * TILE;
        for (p, a) in ring.into_iter().zip(around) {
            positions.push(p);
            uvs.push(vec2(a / perimeter * tiles, along));
        }
    }
    let vertex = |j: usize, s: u32| {
        let (start, point) = starts[j];
        if point { start } else { start + s }
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
    let normals = smooth_normals(&positions, &indices);
    Geometry::new(positions, normals, uvs, indices)
}

/// Smooth vertex normals. Vertices at the same position share a normal, so a UV seam does
/// not show in the shading.
fn smooth_normals(positions: &[Vec3], indices: &[u32]) -> Vec<Vec3> {
    let mut first = HashMap::new();
    let same: Vec<u32> = positions
        .iter()
        .enumerate()
        .map(|(i, p)| {
            *first
                .entry(p.to_array().map(f32::to_bits))
                .or_insert(i as u32)
        })
        .collect();
    let welded = indices.iter().map(|&i| same[i as usize]).collect();
    let normals = Geometry::new(positions.to_vec(), Vec::new(), Vec::new(), welded).normals;
    same.iter().map(|&i| normals[i as usize]).collect()
}

/// The ring of a ring table (see [`loft`]) at `y`, linear between the rows.
fn ring_at(rings: &[[f32; 4]], y: f32) -> [f32; 4] {
    let i = rings
        .iter()
        .position(|r| r[0] > y)
        .unwrap_or(rings.len() - 1)
        .max(1);
    let (a, b) = (Vec4::from_array(rings[i - 1]), Vec4::from_array(rings[i]));
    a.lerp(b, ((y - a.x) / (b.x - a.x)).clamp(0.0, 1.0))
        .to_array()
}

/// A band of soft folds on a limb, as in a sleeve at the inside of the elbow.
struct Fold {
    /// The middle of the band on the axis of the limb, in root space.
    center: Vec3,
    /// The side of the limb where the folds are deepest: a unit vector across the limb.
    side: Vec3,
    /// The half length of the band along Y, in meters.
    width: f32,
    /// The depth of the folds, in meters.
    depth: f32,
}

/// Presses soft folds into a loft along Y and darkens them with vertex colors. The folds
/// only go in, so the outline stays the same.
fn folded(geometry: Geometry, folds: &[Fold]) -> Geometry {
    let mut positions = geometry.positions;
    let mut colors = vec![Vec4::ONE; positions.len()];
    for (p, color) in positions.iter_mut().zip(&mut colors) {
        for fold in folds {
            let d = *p - fold.center;
            let out = d.with_y(0.0).normalize_or_zero();
            let facing = out.dot(fold.side);
            let band = (1.0 - (d.y / fold.width).powi(2)).max(0.0).powi(2);
            let around = (0.5 + 0.5 * facing).powi(2);
            // The fold lines curve down toward the sides of the limb and wave a little
            // around it, so they do not look like rings.
            let angle = out.x.atan2(out.z);
            let phase = d.y / FOLD_PERIOD + 0.3 * facing + 0.12 * (3.0 * angle).sin();
            let wave = 0.5 + 0.5 * (TAU * phase).cos();
            let groove = band * around * wave;
            *p -= out * fold.depth * groove;
            let shade = 1.0 - 0.45 * groove - 0.15 * band * around;
            *color = (color.truncate() * shade).extend(1.0);
        }
    }
    let normals = smooth_normals(&positions, &geometry.indices);
    let mut out = Geometry::new(positions, normals, geometry.uvs, geometry.indices);
    out.colors = colors;
    out
}

/// Smooth value noise in `0..1` on a grid of cells of size 1.
fn noise2(p: Vec2, seed: u32) -> f32 {
    let cell = p.floor();
    let f = p - cell;
    let u = f * f * (3.0 - 2.0 * f);
    let at = |i: f32, j: f32| {
        let (x, y) = ((cell.x + i) as i32 as u32, (cell.y + j) as i32 as u32);
        hash(x.wrapping_mul(7919) ^ y.wrapping_mul(104_729) ^ seed.wrapping_mul(0x9e37_79b9))
    };
    let a = at(0.0, 0.0) + (at(1.0, 0.0) - at(0.0, 0.0)) * u.x;
    let b = at(0.0, 1.0) + (at(1.0, 1.0) - at(0.0, 1.0)) * u.x;
    a + (b - a) * u.y
}

/// Presses soft wrinkles into cloth around the vertical axis `axis` (x, z): long diagonal
/// creases 5 to 12 cm long, at most `depth` meters deep. Cloth is never a smooth tube. The
/// wrinkles only go in, and the creases are a little darker in the vertex colors.
fn crumpled(geometry: Geometry, axis: Vec2, depth: f32, seed: u32) -> Geometry {
    let mut positions = geometry.positions;
    let mut colors = if geometry.colors.is_empty() {
        vec![Vec4::ONE; positions.len()]
    } else {
        geometry.colors
    };
    for ((p, n), color) in positions.iter_mut().zip(&geometry.normals).zip(&mut colors) {
        let arc = 0.1 * (p.x - axis.x).atan2(p.z - axis.y);
        let q = vec2(arc + 0.5 * p.y, p.y - 0.4 * arc) * vec2(1.0 / 0.05, 1.0 / 0.11);
        // a ridge where the noise crosses its middle is a crease
        let crease = |q: Vec2, k: u32| 1.0 - (2.0 * noise2(q, seed * 13 + k) - 1.0).abs();
        let c = 0.65 * crease(q, 0).powi(3) + 0.35 * crease(q * 2.1 + 7.0, 1).powi(3);
        *p -= *n * depth * c;
        *color = (color.truncate() * (1.0 - 0.25 * c)).extend(1.0);
    }
    let normals = smooth_normals(&positions, &geometry.indices);
    let mut out = Geometry::new(positions, normals, geometry.uvs, geometry.indices);
    out.colors = colors;
    out
}

/// Wear on a trouser leg around the axis `axis` (x, z): long streaks where the cloth is
/// lighter or darker, and a darker hem with dust. It multiplies the vertex colors.
fn worn(mut geometry: Geometry, axis: Vec2, seed: u32) -> Geometry {
    for (p, color) in geometry.positions.iter().zip(&mut geometry.colors) {
        let angle = (p.x - axis.x).atan2(p.z - axis.y);
        let streak =
            noise1(80 + seed, 5.0 * angle + 2.0 * p.y) + 0.5 * noise1(90 + seed, 13.0 * angle);
        let hem = 1.0 - 0.25 * (1.0 - smoothstep(0.1, 0.3, p.y));
        let tone = (1.0 + 0.09 * streak) * hem;
        *color = (color.truncate() * tone).extend(1.0);
    }
    geometry
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
    [1.38, 0.212, 0.116, 0.0],
    [1.405, 0.2, 0.106, -0.005],
    [1.43, 0.165, 0.094, -0.01],
    [1.46, 0.115, 0.08, -0.015],
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
const SLEEVE: [[f32; 4]; 15] = [
    [1.4, 0.0, 0.0, 0.0],
    [1.395, 0.032, 0.036, 0.0],
    [1.382, 0.05, 0.054, 0.0],
    [1.357, 0.058, 0.061, 0.0],
    [1.32, 0.058, 0.06, 0.0],
    [1.26, 0.054, 0.056, 0.0],
    [1.14, 0.05, 0.052, 0.0],
    [1.03, 0.049, 0.05, 0.0],
    [0.93, 0.045, 0.046, 0.0],
    [0.895, 0.0445, 0.0455, 0.0],
    [0.889, 0.0465, 0.0475, 0.0],
    [0.869, 0.0465, 0.0475, 0.0],
    [0.863, 0.0445, 0.0455, 0.0],
    [0.861, 0.035, 0.036, 0.0],
    [0.86, 0.0, 0.0, 0.0],
];

/// The height of the end of a sleeve in root space, in the rest pose.
const CUFF: f32 = 0.86;

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
const PALM: [[f32; 4]; 11] = [
    [0.045, 0.0, 0.0, 0.0],
    [0.04, 0.019, 0.026, 0.0],
    [0.012, 0.018, 0.027, 0.0],
    [-0.012, 0.0185, 0.032, 0.0],
    [-0.035, 0.0185, 0.038, 0.0],
    [-0.06, 0.017, 0.041, 0.0],
    [-0.08, 0.0145, 0.042, 0.0],
    [-0.09, 0.0125, 0.04, 0.0],
    [-0.097, 0.0095, 0.035, 0.0],
    [-0.102, 0.0055, 0.026, 0.0],
    [-0.104, 0.0, 0.0, 0.0],
];

/// The cloth over the knee, centered on the knee joint. The denim is pulled tight over the
/// knee cap, so it is worn lighter there. Soft folds run out from it to the sides.
fn knee_cap() -> Geometry {
    let mut cap = Geometry::sphere(0.056, 40, 24);
    let mut colors = Vec::with_capacity(cap.positions.len());
    for p in &mut cap.positions {
        let d = *p / 0.056;
        // the front of the knee is +Z
        let front = d.z.max(0.0);
        let angle = d.y.atan2(d.x);
        let fold = (0.5 + 0.5 * (5.0 * angle + 1.3 * (3.0 * angle).sin()).cos()).powi(3);
        let ring = smoothstep(0.55, 0.85, 1.0 - front) * (1.0 - smoothstep(0.9, 1.0, 1.0 - front));
        *p *= 1.0 - 0.03 * fold * ring;
        let worn = smoothstep(0.6, 0.95, front);
        let shade = (1.0 + 0.35 * worn) * (1.0 - 0.3 * fold * ring);
        colors.push(vec4(shade, shade, shade * (1.0 + 0.05 * worn), 1.0));
    }
    let normals = smooth_normals(&cap.positions, &cap.indices);
    let mut geometry = Geometry::new(cap.positions, normals, cap.uvs, cap.indices);
    geometry.colors = colors;
    geometry
}

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

/// A boot without the shaft and the sole in the space of the ankle: the upper and the
/// laces.
fn boot() -> Geometry {
    // The loft turns from +Y to +Z, and its Z (the height) to -Y.
    let mut upper = loft(&BOOT, 48, 0.004).rotate_x(FRAC_PI_2);
    // Dark seams of the toe cap and the heel, a row of stitches above the sole, and
    // lighter scuffs on the toe.
    upper.colors = upper
        .positions
        .iter()
        .map(|p| {
            let across = p.x / 0.05;
            let toe_seam = 0.125 + 0.025 * across * across;
            let heel_seam = -0.045 - 0.02 * (p.y + 0.03) / 0.06;
            let line = |d: f32| (-(d / 0.0012).powi(2)).exp();
            let seams = line(p.z - toe_seam).max(line(p.z - heel_seam));
            let stitch = line(p.y + 0.08) * (0.5 + 0.5 * (TAU * p.z / 0.004).cos()).powi(4);
            let scuff =
                smoothstep(0.14, 0.21, p.z) * (0.5 + 0.5 * noise1(95, 160.0 * p.x + 90.0 * p.y));
            let shade = (1.0 - 0.55 * seams - 0.4 * stitch) * (1.0 + 0.45 * scuff);
            vec4(shade, shade, shade, 1.0)
        })
        .collect();
    // The laces cross between eyelets on the instep.
    let eyelet = |s: f32, side: f32| {
        let [_, half_width, half_height, depth] = ring_at(&BOOT, s);
        vec3(
            side * 0.5 * half_width,
            0.87 * half_height - depth + 0.001,
            s,
        )
    };
    let rows = [0.035, 0.06, 0.085, 0.11];
    let mut laces = Vec::new();
    for pair in rows.windows(2) {
        for side in [1.0, -1.0] {
            let (a, b) = (eyelet(pair[0], side), eyelet(pair[1], -side));
            let over = if side > 0.0 { 0.0045 } else { 0.003 };
            let path = [a, (a + b) * 0.5 + Vec3::Y * over, b];
            laces.push(Geometry::tube(&path, 0.0018, 6));
        }
    }
    Geometry::merge([upper, Geometry::merge(laces)])
}

/// A rubber sole in the space of the ankle: a heel, a thin part under the arch and the front
/// part. It is 6 mm wider than the upper, and its bottom is the ground of a flat foot.
fn sole() -> Geometry {
    let part = |from: f32, to: f32, bottom: f32, height: f32| {
        let at = |k: usize| from + (to - from) * k as f32 / 12.0;
        let width = |k: usize| ring_at(&BOOT, at(k))[1] + 0.006;
        let right = (0..=12).map(|k| vec2(width(k), at(k)));
        let left = (0..=12).rev().map(|k| vec2(-width(k), at(k)));
        let outline: Vec<Vec2> = right.chain(left).collect();
        // The extrusion turns from Z to Y.
        Geometry::extrude(&outline, height, 0.003)
            .rotate_x(FRAC_PI_2)
            .translate(0.0, bottom + 0.5 * height, 0.0)
    };
    Geometry::merge([
        part(-0.094, -0.012, -0.095, 0.03),
        part(-0.02, 0.06, -0.084, 0.013),
        part(0.045, 0.234, -0.095, 0.02),
    ])
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
        [skull, loft(&FACE, 32, 0.01), loft(&NOSE, 12, 0.01)]
            .into_iter()
            .chain(ears),
    )
}

/// Short hair: a shell over the skull. The hairline is high on the forehead and low on the
/// neck.
fn hair() -> Geometry {
    let mut cap = Geometry::sphere_partial(1.0, 64, 32, 0.0, 1.85);
    // The hair is thick on top and thin at the hairline, so the edge does not look like a
    // cap. Short tufts make the outline uneven, so it catches the backlight in small
    // points. The positions are on the unit sphere, so `y` is the cosine of the polar angle.
    for p in &mut cap.positions {
        let polar = p.y.acos();
        let around = p.z.atan2(p.x);
        let tufts = noise2(vec2(around * 4.0, polar * 7.0), 61) * 0.7
            + noise2(vec2(around * 11.0, polar * 17.0), 62) * 0.3;
        let thick = smoothstep(1.85, 1.0, polar);
        *p *= 1.015 + 0.07 * thick + 0.12 * thick * tufts;
    }
    let normals = smooth_normals(&cap.positions, &cap.indices);
    Geometry::new(cap.positions, normals, cap.uvs, cap.indices)
        .rotate_x(-0.4)
        .scale(0.076, 0.098, 0.099)
        .translate(0.0, 1.685, -0.008)
}

/// The joints of the fingers of the hand of `side` in root space, with straight fingers:
/// the bind pose of the skinned hand. `wrist` is the wrist joint in the rest pose.
fn finger_binds(side: f32, wrist: Vec3) -> [[Mat4; 3]; 5] {
    std::array::from_fn(|f| {
        let finger = &FINGERS[f];
        let rotations = finger_rotations(side, f, 0.0);
        let mut m = Mat4::from_translation(wrist);
        let mut offset = finger.base * vec3(side, 1.0, 1.0);
        std::array::from_fn(|k| {
            m *= Mat4::from_rotation_translation(rotations[k], offset);
            offset = vec3(0.0, -finger.lengths[k], 0.0);
            m
        })
    })
}

/// Skin weights between two joints: `a` before `at`, `b` after it, blended over `width`
/// on each side. `s` is the place of the vertex on the same axis.
fn blend(s: f32, at: f32, width: f32, a: u16, b: u16) -> ([u16; 4], [f32; 4]) {
    let w = smoothstep(at - width, at + width, s);
    ([a, b, 0, 0], [1.0 - w, w, 0.0, 0.0])
}

/// The hand of `side` in root space with straight fingers, see [`finger_binds`]: the skin
/// of the palm and the fingers, and the nails. Joint 0 of the skin weights is the wrist,
/// joint `1 + 3 * f + k` is the joint `k` of finger `f`.
fn hand(side: f32, wrist: Vec3, binds: &[[Mat4; 3]; 5]) -> (Geometry, Geometry) {
    // `Geometry::merge` drops the skin weights, so they are joined here in the same order.
    let palm = palm(side).translate(wrist.x, wrist.y, wrist.z);
    let mut skin = SkinWeights {
        joints: vec![[0; 4]; palm.positions.len()],
        weights: vec![[1.0, 0.0, 0.0, 0.0]; palm.positions.len()],
    };
    let mut nail_skin = SkinWeights::default();
    let mut parts = vec![palm];
    let mut nails = Vec::new();
    for (f, bind) in binds.iter().enumerate() {
        let (finger, nail) = finger(f, side);
        let first = 1 + 3 * f as u16;
        let [l0, l1, _] = FINGERS[f].lengths;
        let width = 0.35 * FINGERS[f].radius[0];
        for (geometry, out, weights) in [
            (finger, &mut parts, &mut skin),
            (nail, &mut nails, &mut nail_skin),
        ] {
            for p in &geometry.positions {
                let s = -p.y;
                let (joints, w) = if s < 0.5 * l0 {
                    blend(s, 0.0, 1.8 * width, 0, first)
                } else if s < l0 + 0.5 * l1 {
                    blend(s, l0, width, first, first + 1)
                } else {
                    blend(s, l0 + l1, width, first + 1, first + 2)
                };
                weights.joints.push(joints);
                weights.weights.push(w);
            }
            out.push(geometry.transform(bind[0]));
        }
    }
    let mut skin_part = Geometry::merge(parts);
    skin_part.skin = Some(skin);
    let mut nails = Geometry::merge(nails);
    nails.skin = Some(nail_skin);
    (skin_part, nails)
}

/// The half thickness (palm to back) and the half width of finger `f` at `s` meters from
/// its base joint along the straight finger. The joints are a little thicker than the
/// shafts, and the tip is round.
fn finger_radius(f: usize, s: f32) -> (f32, f32) {
    let finger = &FINGERS[f];
    let [l0, l1, l2] = finger.lengths;
    let length = l0 + l1 + l2;
    let [r0, r1] = finger.radius;
    let mut r = r0 + (r1 - r0) * (s / length).clamp(0.0, 1.0);
    for joint in [0.0, l0, l0 + l1] {
        r *= 1.0 + 0.07 * (-((s - joint) / (0.5 * r)).powi(2)).exp();
    }
    let e = ((s - (length - 0.2 * r1)) / (1.1 * r1)).clamp(0.0, 1.0);
    r *= (1.0 - e * e).sqrt();
    (0.88 * r, r)
}

/// Finger `f` of the hand of `side`, straight along -Y in the space of its base joint, and
/// its nail. The palm side has soft pads and two creases at each joint. The back has fine
/// wrinkles over the knuckles, which are a little red. The back of the last segment is
/// flatter under the nail.
fn finger(f: usize, side: f32) -> (Geometry, Geometry) {
    let finger = &FINGERS[f];
    let [l0, l1, l2] = finger.lengths;
    let length = l0 + l1 + l2;
    let (r0, tip) = (finger.radius[0], finger.radius[1]);
    let top = 0.6 * r0;
    let end = length + 0.9 * tip;
    // Rings every 1.6 mm; the loft adds one ring between two.
    let count = ((top + end) / 0.0016).ceil() as usize;
    let mut rings: Vec<[f32; 4]> = vec![[top + 0.3 * r0, 0.0, 0.0, 0.0]];
    for k in 0..=count {
        let s = -top + (top + end) * k as f32 / count as f32;
        let (rx, rz) = finger_radius(f, s.max(0.0));
        if rx > 1e-5 {
            rings.push([-s, rx, rz, 0.0]);
        }
    }
    rings.push([-end, 0.0, 0.0, 0.0]);
    let tube = loft(&rings, 24, 1.0);
    let back = vec3(side, 0.0, 0.0);
    let joints = [l0, l0 + l1];
    let mut positions = Vec::with_capacity(tube.positions.len());
    let mut colors = Vec::with_capacity(tube.positions.len());
    for (&p, &n) in tube.positions.iter().zip(&tube.normals) {
        let s = -p.y;
        let facing = n.dot(back);
        let (r, _) = finger_radius(f, s.clamp(0.0, length));
        let mut out = 0.0;
        let mut shade = 1.0f32;
        let mut red = 0.0;
        // Pads on the palm side, the largest at the tip.
        let palm = (-facing).max(0.0).powf(1.5);
        for (k, (from, len)) in [(0.0, l0), (l0, l1), (l0 + l1, l2)].into_iter().enumerate() {
            let center = from + if k == 2 { 0.7 } else { 0.55 } * len;
            let pad = (-((s - center) / (0.35 * len)).powi(2)).exp();
            out += palm * pad * r * if k == 2 { 0.16 } else { 0.09 };
        }
        // Under the nail the back is flatter.
        let bed = smoothstep(length - 0.65 * l2, length - 0.4 * l2, s) * facing.max(0.0).powi(2);
        out -= 0.08 * r * bed;
        for joint in joints {
            let d = s - joint;
            // Fine wrinkles over the knuckle on the back: arcs around the middle of the
            // knuckle, deepest on top.
            let across = p.z / r;
            let arc = (d * d + (0.45 * r * across).powi(2)).sqrt();
            let window = (-(arc / (0.42 * r)).powi(2)).exp() * smoothstep(0.35, 0.8, facing);
            let wave = 0.5 + 0.5 * (TAU * arc / 0.0011).cos();
            out -= 0.0001 * window * wave;
            shade *= 1.0 - 0.18 * window * wave;
            red += 0.6 * window;
            // two creases on the palm side
            for c in [-0.18 * r, 0.22 * r] {
                let crease = (-((d - c) / 0.0006).powi(2)).exp() * smoothstep(0.2, 0.6, -facing);
                out -= 0.0003 * crease;
                shade *= 1.0 - 0.35 * crease;
            }
        }
        // the base knuckle is red too, and the skin at the tip
        red += 0.4 * (-(s / (0.8 * r)).powi(2)).exp() * facing.max(0.0);
        red += 0.5 * smoothstep(length - 0.3 * l2, length + 0.3 * tip, s);
        positions.push(p + n * out);
        let red = red.min(1.0);
        colors.push(vec4(
            shade * (1.0 + 0.08 * red),
            shade * (1.0 - 0.06 * red),
            shade * (1.0 - 0.08 * red),
            1.0,
        ));
    }
    let normals = smooth_normals(&positions, &tube.indices);
    let mut geometry = Geometry::new(positions, normals, tube.uvs, tube.indices);
    geometry.colors = colors;
    (geometry, nail(f, side))
}

/// The nail of finger `f` on the hand of `side`, in the space of the base joint of the
/// straight finger. It is a thin shell 0.3 mm over the skin. Its edges sink into the skin,
/// except the free edge at the tip. The half moon at its root is paler, and the free edge is
/// white.
fn nail(f: usize, side: f32) -> Geometry {
    let [_, _, l2] = FINGERS[f].lengths;
    let length = FINGERS[f].lengths.iter().sum::<f32>();
    let tip = FINGERS[f].radius[1];
    let (from, to) = (length - 0.62 * l2, length + 0.3 * tip);
    let (rows, columns) = (14, 12);
    let mut positions = Vec::new();
    let mut colors = Vec::new();
    let mut uvs = Vec::new();
    for i in 0..=rows {
        let v = i as f32 / rows as f32;
        let s = from + (to - from) * v;
        for j in 0..=columns {
            let u = j as f32 / columns as f32 * 2.0 - 1.0;
            // The root of the nail is an arch: the sides start nearer the tip.
            let s = s + 0.12 * l2 * u * u * (1.0 - v);
            let (rx, rz) = finger_radius(f, s.min(length - 0.1 * tip));
            let angle = 0.85 * u;
            let edge = u.abs().powi(4).max((1.0 - v / 0.12).max(0.0));
            let lift = 0.0003 * (1.0 - edge) - 0.0001;
            // the flat bed under the nail
            let rx = rx * (1.0 - 0.08 * smoothstep(0.0, 0.3, v));
            positions.push(vec3(
                side * ((rx + lift) * angle.cos()),
                -s,
                (rz + lift) * angle.sin(),
            ));
            let moon = 1.0 - smoothstep(0.1, 0.3, v);
            let free = smoothstep(0.86, 0.95, v);
            let c = Vec3::splat(0.8)
                .lerp(vec3(0.95, 0.9, 0.9), moon)
                .lerp(vec3(1.2, 1.15, 1.08), free);
            colors.push(c.extend(1.0));
            uvs.push(vec2(u, v));
        }
    }
    let mut indices = Vec::new();
    let w = columns + 1;
    for i in 0..rows {
        for j in 0..columns {
            let a = i * w + j;
            let quad = [a, a + w, a + 1, a + 1, a + w, a + w + 1];
            // the shell faces out on both hands
            if side < 0.0 {
                indices.extend(quad);
            } else {
                indices.extend([quad[0], quad[2], quad[1], quad[3], quad[5], quad[4]]);
            }
        }
    }
    let normals = smooth_normals(&positions, &indices);
    let mut geometry = Geometry::new(positions, normals, uvs, indices);
    geometry.colors = colors;
    geometry
}

/// Turns the elliptic rings of a [`loft`] with `segments` vertices around into rounded
/// rectangles. `power` is 2 for an ellipse and larger for squarer corners.
fn squared(geometry: Geometry, segments: u32, power: f32) -> Geometry {
    let mut positions = geometry.positions;
    let mut start = 0;
    while start < positions.len() {
        let y = positions[start].y;
        let count = positions[start..].iter().take_while(|p| p.y == y).count();
        if count == segments as usize + 1 {
            let ring = &mut positions[start..start + count];
            let center =
                ring[..segments as usize].iter().map(|p| p.z).sum::<f32>() / segments as f32;
            for (k, p) in ring.iter_mut().enumerate() {
                let a = PI + k as f32 / segments as f32 * TAU;
                let k = |c: f32| {
                    if c.abs() < 1e-6 {
                        0.0
                    } else {
                        c.abs().powf(2.0 / power - 1.0)
                    }
                };
                p.x *= k(a.sin());
                p.z = center + (p.z - center) * k(a.cos());
            }
        }
        start += count;
    }
    let normals = smooth_normals(&positions, &geometry.indices);
    Geometry::new(positions, normals, geometry.uvs, geometry.indices)
}

/// The veins on the back of the left hand, in the space of the wrist: points of (y, z).
/// They run from the wrist to the gaps between the knuckles, and an arch joins them.
const VEINS: [&[[f32; 2]]; 4] = [
    &[
        [0.04, 0.024],
        [0.0, 0.026],
        [-0.035, 0.022],
        [-0.068, 0.019],
    ],
    &[
        [0.04, -0.02],
        [0.005, -0.024],
        [-0.03, -0.022],
        [-0.07, -0.019],
    ],
    &[
        [0.04, 0.002],
        [0.01, -0.004],
        [-0.025, 0.004],
        [-0.05, -0.001],
        [-0.074, 0.0],
    ],
    &[
        [-0.034, 0.022],
        [-0.046, 0.01],
        [-0.05, -0.001],
        [-0.044, -0.012],
        [-0.03, -0.022],
    ],
];

/// The distance from (y, z) to the polyline `path`.
fn path_distance(q: Vec2, path: &[[f32; 2]]) -> f32 {
    path.windows(2)
        .map(|pair| {
            let (a, b) = (Vec2::from(pair[0]), Vec2::from(pair[1]));
            let t = ((q - a).dot(b - a) / (b - a).length_squared()).clamp(0.0, 1.0);
            q.distance(a + (b - a) * t)
        })
        .fold(f32::INFINITY, f32::min)
}

/// The palm and the back of the hand in the space of the wrist of `side`, from inside the
/// cuff to the knuckles. The cross-section is a rounded rectangle. The back is a little
/// arched, with four knuckles, the tendons that run to them and the veins. The palm has the
/// pads at the thumb and at the little finger, and it is lighter. The bone of the wrist
/// shows on the side of the little finger.
fn palm(side: f32) -> Geometry {
    let hand = squared(loft(&PALM, 160, 0.0025), 160, 2.6);
    // A round bump of `height` meters at `center`, `radius` meters wide.
    let bump = |p: Vec3, center: Vec3, radius: f32, height: f32| {
        height * (-p.distance_squared(center * vec3(side, 1.0, 1.0)) / (radius * radius)).exp()
    };
    let mut positions = Vec::with_capacity(hand.positions.len());
    let mut colors = Vec::with_capacity(hand.positions.len());
    for (&p, &n) in hand.positions.iter().zip(&hand.normals) {
        let facing = n.x * side;
        let back = facing > 0.0;
        let mut out = bump(p, vec3(-0.018, -0.04, 0.026), 0.016, 0.005)
            + bump(p, vec3(-0.016, -0.05, -0.032), 0.015, 0.003)
            + bump(p, vec3(0.008, 0.014, -0.026), 0.007, 0.0025);
        let mut knuckle = 0.0;
        let mut vein = 0.0;
        if back {
            let on_back = smoothstep(0.3, 0.8, facing);
            for finger in &FINGERS[INDEX..] {
                let top = vec3(0.012, finger.base.y + 0.003, finger.base.z);
                knuckle += bump(p, top, 0.008, 1.0);
                // the tendon from the wrist to the knuckle
                let along = (p.y / (finger.base.y + 0.008)).clamp(0.0, 1.0);
                let z = finger.base.z * (0.35 + 0.65 * along);
                let ridge = smoothstep(0.0, 0.3, along) * (1.0 - smoothstep(0.85, 1.0, along));
                out += 0.0009 * ridge * on_back * (-((p.z - z) / 0.0022).powi(2)).exp();
            }
            out += 0.0035 * knuckle;
            // The veins are soft ridges. They fade before the knuckles.
            let q = vec2(p.y, p.z);
            for path in VEINS {
                let d = path_distance(q, path);
                vein = f32::max(vein, (-(d / 0.0016).powi(2)).exp());
            }
            vein *= on_back * (1.0 - smoothstep(-0.07, -0.08, p.y));
            out += 0.0011 * vein;
            // the back is a little arched
            out += 0.0015 * on_back * (1.0 - (p.z / 0.04).powi(2)).max(0.0);
        }
        positions.push(p + n * out);
        // The palm is lighter, the knuckles a little red, the veins a little blue.
        let tone = if back { 1.0 } else { 1.18 };
        colors.push(vec4(
            tone * (1.0 - 0.1 * vein),
            tone * (1.0 - 0.08 * knuckle - 0.04 * vein),
            tone * (1.0 - 0.1 * knuckle + 0.02 * vein),
            1.0,
        ));
    }
    let normals = smooth_normals(&positions, &hand.indices);
    let mut geometry = Geometry::new(positions, normals, hand.uvs, hand.indices);
    geometry.colors = colors;
    geometry
}

/// A point on the jacket at the height `y` and the angle `angle` around from the front,
/// `out` meters outside the surface, in root space.
fn on_jacket(y: f32, angle: f32, out: f32) -> Vec3 {
    let [_, half_width, half_depth, center] = ring_at(&JACKET, y);
    let (sin, cos) = angle.sin_cos();
    vec3(
        (half_width + out) * sin,
        y,
        center + (half_depth + out) * cos,
    )
}

/// The zip line down the front of the jacket and the seams of two chest pockets with flaps,
/// in root space. The lines are half in the cloth, so only a small ridge shows.
fn seams() -> Geometry {
    let zip: Vec<Vec3> = (0..=56)
        .map(|k| on_jacket(0.89 + 0.01 * k as f32, 0.0, 0.0012))
        .collect();
    let mut seams = vec![Geometry::tube(&zip, 0.0024, 8)];
    for side in [1.0f32, -1.0] {
        // The pocket under the flap: angles around the jacket and heights.
        let (left, right, bottom, top) = (0.34, 0.8, 1.18, 1.255);
        // The sides and the bottom of the pocket, with round corners.
        let mut outline = vec![vec2(left, top)];
        for k in 0..=8 {
            let a = PI + FRAC_PI_2 * k as f32 / 8.0;
            outline.push(vec2(
                left + 0.06 + 0.06 * a.cos(),
                bottom + 0.012 + 0.012 * a.sin(),
            ));
        }
        for k in 0..=8 {
            let a = 1.5 * PI + FRAC_PI_2 * k as f32 / 8.0;
            outline.push(vec2(
                right - 0.06 + 0.06 * a.cos(),
                bottom + 0.012 + 0.012 * a.sin(),
            ));
        }
        outline.push(vec2(right, top));
        // The flap over the opening.
        let (flap_left, flap_right) = (left - 0.02, right + 0.02);
        let flap = [
            vec2(flap_left, top + 0.035),
            vec2(flap_left, top),
            vec2(flap_right, top),
            vec2(flap_right, top + 0.035),
            vec2(flap_left, top + 0.035),
        ];
        for line in [&outline[..], &flap[..]] {
            // A point is an angle around the jacket (x) and a height (y). The points are
            // close, so the seam follows the curve of the chest.
            let mut points: Vec<Vec2> = line
                .windows(2)
                .flat_map(|pair| {
                    let steps = ((pair[1] - pair[0]).length() / 0.01).ceil().max(1.0);
                    (0..steps as usize).map(move |k| pair[0].lerp(pair[1], k as f32 / steps))
                })
                .collect();
            points.extend(line.last());
            let path: Vec<Vec3> = points
                .iter()
                .map(|p| on_jacket(p.y, side * p.x, 0.0008))
                .collect();
            seams.push(Geometry::tube(&path, 0.0014, 6));
        }
    }
    Geometry::merge(seams)
}

/// The detail textures of all figures. They are built once and shared, so the GPU loads
/// them once.
struct Detail {
    /// The canvas of the jacket: a normal map, and occlusion (R) with roughness (G).
    canvas: [Arc<Texture>; 2],
    /// The denim of the trousers, in the same two maps.
    denim: [Arc<Texture>; 2],
    /// The strands of the hair: a normal map for the UVs of a sphere.
    strands: Arc<Texture>,
    /// Pores and fine lines of the skin, in the two maps of the cloth.
    skin: [Arc<Texture>; 2],
    /// The grain and the creases of the leather of the boots.
    leather: [Arc<Texture>; 2],
}

fn detail() -> &'static Detail {
    static DETAIL: OnceLock<Detail> = OnceLock::new();
    DETAIL.get_or_init(|| Detail {
        canvas: twill(60, 2),
        denim: twill(80, 3),
        strands: Arc::new(strands()),
        skin: skin_lines(),
        leather: grain(40.0, 0.12e-3, 0.8, 5),
    })
}

/// A value of `0..1` as a texture byte.
fn byte(value: f32) -> u8 {
    (value.clamp(0.0, 1.0) * 255.0).round() as u8
}

/// A tangent space normal as texture bytes.
fn normal_bytes(n: Vec3) -> [u8; 4] {
    let n = n.normalize() * 0.5 + 0.5;
    [byte(n.x), byte(n.y), byte(n.z), 255]
}

/// The maps of a twill weave, one tile of [`TILE`] meters with `threads` threads each way.
///
/// Each thread goes over `float` threads and under one, and each row starts one thread
/// later. This makes the diagonal ribs of canvas and denim. `threads` must be a multiple of
/// `float + 1`, so the tile repeats without a seam. The threads are 0.12 mm high.
fn twill(threads: u32, float: u32) -> [Arc<Texture>; 2] {
    let count = threads as f32;
    let period = (float + 1) as f32;
    // Each thread has its own thickness.
    surface(0.12e-3, |u, v| {
        let (x, y) = (u * count, v * count);
        let (column, row) = (x.floor(), y.floor());
        let rise = |t: f32| 0.55 + 0.45 * (TAU * t / period).cos();
        let thickness = |i: f32, seed: u32| 0.8 + 0.4 * hash((i as u32) % threads + seed);
        let warp = (PI * x).sin().powi(2) * rise(y - column) * thickness(column, 0);
        let weft = (PI * y).sin().powi(2) * rise(x - row + 0.5 * period) * thickness(row, 7919);
        warp.max(weft)
    })
}

/// The maps of skin or leather, one tile of [`TILE`] meters: soft bumps, `cells` across
/// the tile, and thin creases that cross them. `depth` is the height in meters.
fn grain(cells: f32, depth: f32, creases: f32, seed: u32) -> [Arc<Texture>; 2] {
    // Smooth value noise in 0..1 that repeats `n` times across the tile.
    let noise = |u: f32, v: f32, n: f32, seed: u32| {
        let (x, y) = (u * n, v * n);
        let (i, j) = (x.floor(), y.floor());
        let (fx, fy) = (x - i, y - j);
        let (sx, sy) = (fx * fx * (3.0 - 2.0 * fx), fy * fy * (3.0 - 2.0 * fy));
        let at = |a: f32, b: f32| {
            hash(a.rem_euclid(n) as u32 + b.rem_euclid(n) as u32 * 4099 + seed * 7919)
        };
        let top = at(i, j) + (at(i + 1.0, j) - at(i, j)) * sx;
        let bottom = at(i, j + 1.0) + (at(i + 1.0, j + 1.0) - at(i, j + 1.0)) * sx;
        top + (bottom - top) * sy
    };
    surface(depth, |u, v| {
        let bumps = 0.65 * noise(u, v, cells, seed) + 0.35 * noise(u, v, cells * 2.0, seed + 1);
        // A crease is a thin valley where a coarse noise crosses 0.5.
        let crease = (1.0 - 10.0 * (noise(u, v, cells / 6.0, seed + 2) - 0.5).abs()).max(0.0);
        bumps - creases * crease * crease
    })
}

/// The maps of the skin, one tile of [`TILE`] meters: two sets of fine lines that cross
/// in a diamond pattern 0.9 mm wide, small pores, and soft bumps. The lines wave a little,
/// so the pattern does not look like a grid.
fn skin_lines() -> [Arc<Texture>; 2] {
    // Smooth value noise in 0..1 that repeats `n` times across the tile.
    let noise = |u: f32, v: f32, n: f32, seed: u32| {
        let (x, y) = (u * n, v * n);
        let (i, j) = (x.floor(), y.floor());
        let (fx, fy) = (x - i, y - j);
        let (sx, sy) = (fx * fx * (3.0 - 2.0 * fx), fy * fy * (3.0 - 2.0 * fy));
        let at = |a: f32, b: f32| {
            hash(a.rem_euclid(n) as u32 + b.rem_euclid(n) as u32 * 4099 + seed * 7919)
        };
        let top = at(i, j) + (at(i + 1.0, j) - at(i, j)) * sx;
        let bottom = at(i, j + 1.0) + (at(i + 1.0, j + 1.0) - at(i, j + 1.0)) * sx;
        top + (bottom - top) * sy
    };
    surface(0.025e-3, |u, v| {
        // A whole number of lines crosses the tile in each direction, so it has no seam.
        // The lines fade in and out, as on real skin.
        let wobble = 0.6 * (noise(u, v, 12.0, 1) - 0.5);
        let fade = smoothstep(0.25, 0.75, noise(u, v, 6.0, 4));
        let line = |a: f32, width: f32| (-((a.fract() - 0.5) / width).powi(2)).exp();
        let first = line(u * 40.0 + v * 23.0 + wobble, 0.09);
        let second = line(v * 38.0 - u * 25.0 - wobble, 0.07);
        let pores = smoothstep(0.72, 0.9, noise(u, v, 110.0, 2));
        let bumps = noise(u, v, 30.0, 3);
        1.0 - (0.45 * first + 0.3 * second) * fade - 0.7 * pores + 0.45 * bumps
    })
}

/// The normal map and the occlusion (R) with roughness (G) map of a surface with the
/// height `height(u, v)` in `0..1` over one tile of [`TILE`] meters. `depth` is the height
/// of 1 in meters. The low parts are darker and a little rougher.
fn surface(depth: f32, height: impl Fn(f32, f32) -> f32 + Sync) -> [Arc<Texture>; 2] {
    const SIZE: u32 = 512;
    // The slope is in tile units.
    let depth = depth / TILE;
    let texel = 1.0 / SIZE as f32;
    let normal = Texture::from_fn_data(SIZE, SIZE, false, |u, v| {
        let du = (height(u + texel, v) - height(u - texel, v)) / (2.0 * texel);
        let dv = (height(u, v + texel) - height(u, v - texel)) / (2.0 * texel);
        normal_bytes(vec3(-du * depth, dv * depth, 1.0))
    });
    let occlusion_roughness = Texture::from_fn_data(SIZE, SIZE, false, |u, v| {
        let h = height(u, v);
        [byte(0.55 + 0.45 * h), byte(1.0 - 0.2 * h), 0, 255]
    });
    [normal, occlusion_roughness].map(|t| Arc::new(t.repeat(1.0 / TILE, 1.0 / TILE)))
}

/// A normal map of short hair for the UVs of a sphere: fine strands from the crown down,
/// with a slow wave.
fn strands() -> Texture {
    const WIDTH: u32 = 1024;
    const HEIGHT: u32 = 256;
    // The hair shell is about 0.5 m around and 0.2 m from the crown to the hairline.
    const AROUND: f32 = 0.5;
    const DOWN: f32 = 0.2;
    // Smooth noise across the strands that repeats `count` times around.
    let across = |x: f32, count: f32, seed: u32| {
        let i = x.floor();
        let f = x - i;
        let at = |k: f32| hash(k.rem_euclid(count) as u32 + seed);
        at(i) + (at(i + 1.0) - at(i)) * f * f * (3.0 - 2.0 * f)
    };
    let height = |u: f32, v: f32| {
        let wave = 0.8 * (TAU * 2.0 * v).sin();
        across(u * 360.0 + wave, 360.0, 0) + 0.5 * across(u * 720.0 + 2.0 * wave, 720.0, 997)
    };
    // The strands are 0.25 mm high.
    let depth = 0.25e-3;
    let (du, dv) = (1.0 / WIDTH as f32, 1.0 / HEIGHT as f32);
    Texture::from_fn_data(WIDTH, HEIGHT, false, |u, v| {
        let su = (height(u + du, v) - height(u - du, v)) / (2.0 * du * AROUND);
        let sv = (height(u, v + dv) - height(u, v - dv)) / (2.0 * dv * DOWN);
        normal_bytes(vec3(-su * depth, sv * depth, 1.0))
    })
}
