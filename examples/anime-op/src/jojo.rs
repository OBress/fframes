//! ジョジョ立ち (the JoJo stand): rivals snap between poses the way the manga's characters strike
//! them, hips thrown to one side, the chest twisted the other way, leaning back, a hand framing
//! the face. Keyed like the dance: poses land on their frames through an in-between or with an
//! overshoot, and hold as moving holds ([`fframes_anime::koma::Sheet`]).

use std::sync::Arc;

use fframes_anime::glam::Vec3;
use fframes_anime::koma::{Key, Sheet};
use fframes_anime::rig::{Bone, Pose, Side, Skeleton, two_bone_ik};

use crate::procedural::{Body, Dance, chest_point, hand_facing, plant_feet, ypr};

/// One drawing of a pose. Angles in degrees, distances in metres; hand targets and directions
/// are in the chest frame (x towards the character's left, y up, z forward).
#[derive(Debug, Clone, Copy)]
pub struct Strike {
    /// Hips thrown sideways (+ = to the character's left).
    pub sway: f32,
    /// Hips turn, and the chest's twist against them.
    pub hips: f32,
    pub twist: f32,
    /// Leaning back (+) and to the right (+).
    pub lean: f32,
    pub tilt: f32,
    /// Head turn, nod (+ = chin down) and tilt (+ = to the right).
    pub head: Vec3,
    /// Wrists.
    pub left: Vec3,
    pub right: Vec3,
    /// Fingers and palm of each hand.
    pub left_hand: (Vec3, Vec3),
    pub right_hand: (Vec3, Vec3),
    /// How far the elbows point out.
    pub elbows: [f32; 2],
    /// Half the stance, the left foot's step forward, how deep the knees bend.
    pub stance: f32,
    pub step: f32,
    pub crouch: f32,
}

/// Standing tall, arms hanging: the start of every pose.
pub const STAND: Strike = Strike {
    sway: 0.0,
    hips: 0.0,
    twist: 0.0,
    lean: 0.0,
    tilt: 0.0,
    head: Vec3::new(0.0, 6.0, 0.0),
    left: Vec3::new(0.2, -0.36, 0.04),
    right: Vec3::new(-0.2, -0.36, 0.04),
    left_hand: (Vec3::new(0.05, -1.0, 0.1), Vec3::new(-1.0, 0.0, 0.0)),
    right_hand: (Vec3::new(-0.05, -1.0, 0.1), Vec3::new(1.0, 0.0, 0.0)),
    elbows: [0.6, 0.6],
    stance: 0.14,
    step: 0.0,
    crouch: 0.0,
};

impl Strike {
    pub fn lerp(&self, o: &Strike, t: f32) -> Strike {
        let f = |a: f32, b: f32| a + (b - a) * t;
        let h = |a: (Vec3, Vec3), b: (Vec3, Vec3)| {
            (
                a.0.lerp(b.0, t).normalize_or(b.0),
                a.1.lerp(b.1, t).normalize_or(b.1),
            )
        };
        Strike {
            sway: f(self.sway, o.sway),
            hips: f(self.hips, o.hips),
            twist: f(self.twist, o.twist),
            lean: f(self.lean, o.lean),
            tilt: f(self.tilt, o.tilt),
            head: self.head.lerp(o.head, t),
            left: self.left.lerp(o.left, t),
            right: self.right.lerp(o.right, t),
            left_hand: h(self.left_hand, o.left_hand),
            right_hand: h(self.right_hand, o.right_hand),
            elbows: [
                f(self.elbows[0], o.elbows[0]),
                f(self.elbows[1], o.elbows[1]),
            ],
            stance: f(self.stance, o.stance),
            step: f(self.step, o.step),
            crouch: f(self.crouch, o.crouch),
        }
    }

    /// The `n`th drawing of a moving hold: the pose keeps sinking into itself.
    fn settle(&self, n: u32) -> Strike {
        let k = n.min(4) as f32;
        Strike {
            lean: self.lean + 0.8 * k,
            tilt: self.tilt * (1.0 + 0.03 * k),
            twist: self.twist * (1.0 + 0.02 * k),
            head: self.head + Vec3::new(0.0, -0.8 * k, 0.6 * k * self.tilt.signum()),
            crouch: self.crouch + 0.004 * k,
            ..*self
        }
    }

    pub fn build(&self, sk: &Skeleton, at: Vec3, facing: f32) -> Pose {
        let face = ypr(facing, 0.0, 0.0);
        let body = Body {
            root: at
                + face * Vec3::new(self.sway, 0.0, 0.0)
                + Vec3::Y * (sk.proportions.hip_height - 0.015 - self.crouch),
            yaw: facing + self.hips,
            hips: Vec3::new(
                0.0,
                -self.lean * 0.2 + self.crouch * 40.0,
                self.tilt * 0.25 - self.sway * 90.0,
            ),
            spine: Vec3::new(
                self.twist * 0.35,
                -self.lean * 0.35,
                self.tilt * 0.35 + self.sway * 60.0,
            ),
            chest: Vec3::new(
                self.twist * 0.65,
                -self.lean * 0.45,
                self.tilt * 0.4 + self.sway * 40.0,
            ),
            neck: Vec3::new(self.head.x * 0.3, self.head.y * 0.3, self.head.z * 0.3),
            head: Vec3::new(self.head.x * 0.7, self.head.y * 0.7, self.head.z * 0.7),
        };
        let mut pose = body.pose(sk);
        plant_feet(
            sk,
            &mut pose,
            at,
            facing + self.hips * 0.6,
            self.stance,
            self.step,
        );
        let chest = pose.get(Bone::Chest);
        for (side, target, (fingers, palm), elbow) in [
            (Side::Left, self.left, self.left_hand, self.elbows[0]),
            (Side::Right, self.right, self.right_hand, self.elbows[1]),
        ] {
            let goal = chest_point(sk, &pose, target);
            let shoulder = sk.solve(&pose).pos(Bone::arm(side)[1]);
            let pole = shoulder + chest * Vec3::new(side.sign() * elbow, -0.6, -0.15);
            two_bone_ik(sk, &mut pose, Bone::arm(side)[1], goal, pole, None);
            pose.rot[Bone::arm(side)[3].index()] = hand_facing(side, chest * fingers, chest * palm);
        }
        pose
    }
}

/// A pose landing at `time` (shot seconds). `snap` hits it with an overshoot; otherwise it is
/// reached through one in-between with most of the move done (ツメ).
#[derive(Debug, Clone, Copy)]
pub struct StrikeKey {
    pub time: f32,
    pub pose: Strike,
    pub snap: bool,
}

/// Times poses as key animation: the motion of the drawings, the timesheet, and the eased
/// motion between drawings for hair and cloth.
pub fn strike_sheet(sk: &Skeleton, keys: &[StrikeKey], at: Vec3, facing: f32) -> Dance {
    let fps = crate::actor::FPS;
    let sheet_keys: Vec<Key> = keys
        .iter()
        .map(|k| Key {
            frame: (k.time * fps).round() as i32,
            chart: if k.snap { vec![] } else { vec![0.7] },
            overshoot: if k.snap { 0.16 } else { 0.0 },
        })
        .collect();
    let sheet = Arc::new(Sheet::new(&sheet_keys, 2, 3));
    let poses: Arc<Vec<Strike>> = Arc::new(keys.iter().map(|k| k.pose).collect());
    let drawing = {
        let poses = Arc::clone(&poses);
        move |c: &fframes_anime::koma::Cel| poses[c.from].lerp(&poses[c.to], c.t).settle(c.hold)
    };
    let index = {
        let sheet = Arc::clone(&sheet);
        move |t: f32| {
            let frame = (t * fps + 1e-3).floor() as i32;
            sheet.cels().partition_point(|c| c.frame <= frame).max(1) - 1
        }
    };
    let motion = {
        let (sheet, sk, index, drawing) = (
            Arc::clone(&sheet),
            sk.clone(),
            index.clone(),
            drawing.clone(),
        );
        Arc::new(move |t: f32| drawing(&sheet.cels()[index(t)]).build(&sk, at, facing))
    };
    let between = {
        let (sheet, index, sk) = (Arc::clone(&sheet), index.clone(), sk.clone());
        Arc::new(move |t: f32| {
            let cels = sheet.cels();
            let i = index(t);
            let c = &cels[i];
            let pose = match cels.get(i + 1) {
                Some(n) => {
                    let u =
                        ((t * fps - c.frame as f32) / (n.frame - c.frame) as f32).clamp(0.0, 1.0);
                    drawing(c).lerp(&drawing(n), u * u * (3.0 - 2.0 * u))
                }
                None => drawing(c),
            };
            pose.build(&sk, at, facing)
        })
    };
    Dance {
        motion,
        sheet: Arc::new(move |t| sheet.cels()[index(t)].frame as f32 / fps),
        between,
    }
}

/// A unit vector, for writing hand directions by eye.
pub fn dir(x: f32, y: f32, z: f32) -> Vec3 {
    Vec3::new(x, y, z).normalize()
}

/// The same pose to the other side.
pub fn mirror(s: &Strike) -> Strike {
    let m = |v: Vec3| Vec3::new(-v.x, v.y, v.z);
    let mh = |h: (Vec3, Vec3)| (m(h.0), m(h.1));
    Strike {
        sway: -s.sway,
        hips: -s.hips,
        twist: -s.twist,
        tilt: -s.tilt,
        head: Vec3::new(-s.head.x, s.head.y, -s.head.z),
        left: m(s.right),
        right: m(s.left),
        left_hand: mh(s.right_hand),
        right_hand: mh(s.left_hand),
        elbows: [s.elbows[1], s.elbows[0]],
        step: -s.step,
        ..*s
    }
}

/// Leaning back, a spread hand over the face, the other arm flung out and back (DIO).
pub const LEAN: Strike = Strike {
    sway: -0.06,
    hips: -20.0,
    twist: 38.0,
    lean: 22.0,
    tilt: -9.0,
    head: Vec3::new(-14.0, -14.0, 13.0),
    left: Vec3::new(0.48, -0.12, -0.12),
    right: Vec3::new(-0.05, 0.28, 0.25),
    left_hand: (Vec3::new(0.92, -0.32, -0.18), Vec3::new(0.0, 0.2, 0.98)),
    right_hand: (Vec3::new(0.3, 0.95, -0.08), Vec3::new(0.0, 0.08, -1.0)),
    elbows: [0.4, 1.1],
    stance: 0.2,
    step: 0.14,
    crouch: 0.02,
};

/// Pointing at the viewer across a twisted chest, the other fist on the hip (Jotaro).
pub const POINT: Strike = Strike {
    sway: 0.05,
    hips: 22.0,
    twist: -28.0,
    lean: 7.0,
    tilt: 6.0,
    head: Vec3::new(18.0, -6.0, -8.0),
    left: Vec3::new(0.21, -0.3, -0.02),
    right: Vec3::new(-0.1, 0.16, 0.6),
    left_hand: (Vec3::new(-0.3, -0.5, -0.81), Vec3::new(-0.98, 0.0, 0.2)),
    right_hand: (Vec3::new(-0.05, 0.05, 1.0), Vec3::new(0.29, -0.96, 0.0)),
    elbows: [1.4, 0.3],
    stance: 0.22,
    step: -0.1,
    crouch: 0.03,
};

/// Arms crossed in front of the face, glaring over them (ザ・ワールド).
pub const CROSS: Strike = Strike {
    sway: 0.04,
    hips: 25.0,
    twist: 30.0,
    lean: 12.0,
    tilt: 10.0,
    head: Vec3::new(10.0, 10.0, 8.0),
    left: Vec3::new(-0.08, 0.3, 0.24),
    right: Vec3::new(0.08, 0.24, 0.22),
    left_hand: (Vec3::new(-0.44, 0.88, 0.18), Vec3::new(0.0, 0.0, 1.0)),
    right_hand: (Vec3::new(0.44, 0.88, 0.18), Vec3::new(0.0, 0.0, 1.0)),
    elbows: [0.9, 0.9],
    stance: 0.2,
    step: 0.1,
    crouch: 0.02,
};

/// A hand flat on the chest, the other arm raised behind the head, hips thrown out (Giorno).
pub const CHEST: Strike = Strike {
    sway: -0.07,
    hips: -20.0,
    twist: -38.0,
    lean: 5.0,
    tilt: -12.0,
    head: Vec3::new(-22.0, 4.0, -14.0),
    left: Vec3::new(0.02, 0.1, 0.16),
    right: Vec3::new(-0.1, 0.5, -0.04),
    left_hand: (Vec3::new(-0.41, 0.91, 0.0), Vec3::new(0.0, 0.0, -1.0)),
    right_hand: (Vec3::new(0.78, 0.22, -0.45), Vec3::new(0.0, -0.29, -0.96)),
    elbows: [0.9, 1.8],
    stance: 0.18,
    step: -0.12,
    crouch: 0.03,
};
