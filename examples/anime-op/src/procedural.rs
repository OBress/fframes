//! Hand-keyed moves on the cast's skeletons: poses are built from torso angles, the arms and
//! legs reach their targets with two-bone IK. Used for everything the motion capture does not
//! have, starting with the Bling-Bang-Bang-Born dance.

use std::sync::Arc;

use fframes_anime::glam::{EulerRot, Quat, Vec3};
use fframes_anime::rig::{Bone, Pose, Side, Skeleton, two_bone_ik};

use crate::motion::MotionFn;

/// The song's beat grid (157 BPM): beat `n` sounds at `PHASE + n * PERIOD` seconds.
pub const PERIOD: f32 = 0.38216;
pub const PHASE: f32 = 0.164;

/// Beats since the start of the song at `t` seconds (song time).
pub fn beat(t: f32) -> f32 {
    (t - PHASE) / PERIOD
}

pub fn beat_time(n: f32) -> f32 {
    PHASE + n * PERIOD
}

/// Yaw, pitch and roll in degrees (pitch > 0 bends forward, roll > 0 leans to the right).
pub fn ypr(yaw: f32, pitch: f32, roll: f32) -> Quat {
    Quat::from_euler(
        EulerRot::YXZ,
        yaw.to_radians(),
        pitch.to_radians(),
        roll.to_radians(),
    )
}

/// Torso angles of a pose; arms and legs are solved separately.
#[derive(Debug, Clone, Copy, Default)]
pub struct Body {
    pub root: Vec3,
    pub yaw: f32,
    pub hips: Vec3,
    pub spine: Vec3,
    pub chest: Vec3,
    pub neck: Vec3,
    pub head: Vec3,
}

fn e(v: Vec3) -> Quat {
    ypr(v.x, v.y, v.z)
}

impl Body {
    pub fn lerp(&self, o: &Body, t: f32) -> Body {
        Body {
            root: self.root.lerp(o.root, t),
            yaw: self.yaw + (o.yaw - self.yaw) * t,
            hips: self.hips.lerp(o.hips, t),
            spine: self.spine.lerp(o.spine, t),
            chest: self.chest.lerp(o.chest, t),
            neck: self.neck.lerp(o.neck, t),
            head: self.head.lerp(o.head, t),
        }
    }

    /// Builds the torso of a pose; limbs hang straight down from their parents.
    pub fn pose(&self, sk: &Skeleton) -> Pose {
        let mut pose = sk.rest_pose();
        pose.root = self.root;
        let hips = ypr(self.yaw, 0.0, 0.0) * e(self.hips);
        let spine = hips * e(self.spine);
        let chest = spine * e(self.chest);
        let neck = chest * e(self.neck);
        let head = neck * e(self.head);
        pose.rot[Bone::Hips.index()] = hips;
        pose.rot[Bone::Spine.index()] = spine;
        pose.rot[Bone::Chest.index()] = chest;
        pose.rot[Bone::Neck.index()] = neck;
        pose.rot[Bone::Head.index()] = head;
        for side in [Side::Left, Side::Right] {
            let [shoulder, upper, lower, hand] = Bone::arm(side);
            pose.rot[shoulder.index()] = chest;
            // arms down: rotate the T-pose arm 80 degrees down around the forward axis
            let down = chest * Quat::from_rotation_z(-80_f32.to_radians() * side.sign());
            pose.rot[upper.index()] = down;
            pose.rot[lower.index()] = down;
            pose.rot[hand.index()] = down;
            for b in Bone::leg(side) {
                pose.rot[b.index()] = hips;
            }
        }
        pose
    }
}

/// Points relative to the chest joint, in the chest's frame.
pub fn chest_point(sk: &Skeleton, pose: &Pose, local: Vec3) -> Vec3 {
    let solved = sk.solve(pose);
    solved.pos(Bone::Chest) + pose.get(Bone::Chest) * local
}

/// Puts a hand at `target` (world), elbow bending out and back.
pub fn reach(sk: &Skeleton, pose: &mut Pose, side: Side, target: Vec3, elbow_out: f32) {
    let chest = pose.get(Bone::Chest);
    let solved = sk.solve(pose);
    let shoulder = solved.pos(Bone::arm(side)[1]);
    let pole = shoulder + chest * Vec3::new(side.sign() * elbow_out, -0.35, -0.45);
    two_bone_ik(sk, pose, Bone::arm(side)[1], target, pole, None);
}

/// Hand target given in the chest frame.
pub fn reach_local(sk: &Skeleton, pose: &mut Pose, side: Side, local: Vec3, elbow_out: f32) {
    let target = chest_point(sk, pose, local);
    reach(sk, pose, side, target, elbow_out);
}

/// Plants both feet: `stance` is half the distance between the feet, `step` moves the left foot
/// forward and the right one back (metres).
pub fn plant_feet(sk: &Skeleton, pose: &mut Pose, ground: Vec3, yaw: f32, stance: f32, step: f32) {
    let rot = ypr(yaw, 0.0, 0.0);
    for side in [Side::Left, Side::Right] {
        let s = side.sign();
        let ankle =
            ground + rot * Vec3::new(stance * s, sk.proportions.ankle_height, step * s + 0.02);
        let solved = sk.solve(pose);
        let hip = solved.pos(Bone::leg(side)[0]);
        let pole = hip + rot * Vec3::new(0.1 * s, -0.4, 1.0);
        let foot_rot = ypr(yaw + 8.0 * s, 0.0, 0.0);
        two_bone_ik(sk, pose, Bone::leg(side)[0], ankle, pole, Some(foot_rot));
        pose.rot[Bone::leg(side)[3].index()] = foot_rot;
    }
}

/// One drawing of the Bling-Bang-Bang-Born dance. Fists are wrist targets in the chest frame
/// (x towards the dancer's left, z forward), angles in degrees, distances in metres.
#[derive(Debug, Clone, Copy)]
pub struct DancePose {
    pub left: Vec3,
    pub right: Vec3,
    /// Chest turn, positive towards the dancer's left.
    pub yaw: f32,
    /// Lean, positive towards the dancer's right.
    pub roll: f32,
    /// Bend forward.
    pub bend: f32,
    /// How far the hips drop (bent knees).
    pub crouch: f32,
    /// Hips sideways, positive towards the dancer's left.
    pub sway: f32,
    pub head_tilt: f32,
    /// Chin down.
    pub nod: f32,
    pub elbow_out: f32,
    /// Left and right shoulder raised (degrees).
    pub shrug: [f32; 2],
}

impl DancePose {
    pub fn lerp(&self, o: &DancePose, t: f32) -> DancePose {
        let f = |a: f32, b: f32| a + (b - a) * t;
        DancePose {
            left: self.left.lerp(o.left, t),
            right: self.right.lerp(o.right, t),
            yaw: f(self.yaw, o.yaw),
            roll: f(self.roll, o.roll),
            bend: f(self.bend, o.bend),
            crouch: f(self.crouch, o.crouch),
            sway: f(self.sway, o.sway),
            head_tilt: f(self.head_tilt, o.head_tilt),
            nod: f(self.nod, o.nod),
            elbow_out: f(self.elbow_out, o.elbow_out),
            shrug: [f(self.shrug[0], o.shrug[0]), f(self.shrug[1], o.shrug[1])],
        }
    }

    /// The same pose to the other side.
    pub fn mirror(&self) -> DancePose {
        let m = |v: Vec3| Vec3::new(-v.x, v.y, v.z);
        DancePose {
            left: m(self.right),
            right: m(self.left),
            yaw: -self.yaw,
            roll: -self.roll,
            sway: -self.sway,
            head_tilt: -self.head_tilt,
            shrug: [self.shrug[1], self.shrug[0]],
            ..*self
        }
    }

    /// The `n`th drawing of a moving hold: the weight settles, fists and chin sink.
    fn settle(&self, n: u32) -> DancePose {
        let k = n.min(3) as f32;
        DancePose {
            left: self.left - Vec3::Y * 0.012 * k,
            right: self.right - Vec3::Y * 0.012 * k,
            crouch: self.crouch + 0.006 * k,
            nod: self.nod + 1.5 * k,
            ..*self
        }
    }

    pub fn build(&self, sk: &Skeleton, at: Vec3, facing: f32) -> Pose {
        let face = ypr(facing, 0.0, 0.0);
        let body = Body {
            root: at
                + face * Vec3::new(self.sway, 0.0, 0.0)
                + Vec3::Y * (sk.proportions.hip_height - 0.03 - self.crouch),
            yaw: facing + self.yaw * 0.25,
            hips: Vec3::new(-self.yaw * 0.2, 3.0 + self.crouch * 60.0, -self.roll * 0.4),
            spine: Vec3::new(self.yaw * 0.3, 2.0 + self.bend * 0.4, self.roll * 0.6),
            chest: Vec3::new(self.yaw * 0.45, 3.0 + self.bend * 0.6, self.roll * 0.8),
            neck: Vec3::new(0.0, -4.0 - self.bend * 0.5, 0.0),
            head: Vec3::new(-self.yaw * 0.45, self.nod, self.head_tilt - self.roll * 0.4),
        };
        let mut pose = body.pose(sk);
        let chest = pose.get(Bone::Chest);
        for (side, shrug) in [(Side::Left, self.shrug[0]), (Side::Right, self.shrug[1])] {
            let raise = chest * Quat::from_rotation_z(shrug.to_radians() * side.sign());
            let [shoulder, upper, lower, hand] = Bone::arm(side);
            pose.rot[shoulder.index()] = raise;
            for b in [upper, lower, hand] {
                pose.rot[b.index()] =
                    raise * Quat::from_rotation_z(-80_f32.to_radians() * side.sign());
            }
        }
        plant_feet(sk, &mut pose, at, facing, 0.16, 0.03);
        for (side, target) in [(Side::Left, self.left), (Side::Right, self.right)] {
            let goal = chest_point(sk, &pose, target);
            let shoulder = sk.solve(&pose).pos(Bone::arm(side)[1]);
            let pole = shoulder + chest * Vec3::new(side.sign() * self.elbow_out, -0.8, -0.12);
            two_bone_ik(sk, &mut pose, Bone::arm(side)[1], goal, pole, None);
            // the fists stand up from the forearms, knuckles up, curled fingers to the front and
            // a little inwards (held up like the reference's, never pointing at each other)
            let lower = pose.get(Bone::arm(side)[2]);
            let along = (lower * Vec3::X * side.sign() + chest * Vec3::Y * 1.3).normalize();
            let palm = chest * Vec3::new(-0.45 * side.sign(), 0.0, 1.0);
            pose.rot[Bone::arm(side)[3].index()] = hand_facing(side, along, palm);
        }
        pose
    }
}

const TALL: DancePose = DancePose {
    left: Vec3::ZERO,
    right: Vec3::ZERO,
    yaw: 0.0,
    roll: 0.0,
    bend: 0.0,
    crouch: 0.0,
    sway: 0.0,
    head_tilt: 0.0,
    nod: 4.0,
    elbow_out: 0.9,
    shrug: [0.0, 0.0],
};

// The poses below are read off the reference at 12 fps. Heights are wrist targets in the chest
// frame (the shoulders are 0.16 above the chest joint, the chin about 0.3): the fists ride at
// chest to chin height the whole time, side by side, elbows bent down; the crouches drop the
// body, not the fists.

/// Fists side by side at the chin, standing tall (the pass through the middle).
const CHIN: DancePose = DancePose {
    left: Vec3::new(0.11, 0.2, 0.22),
    right: Vec3::new(-0.1, 0.22, 0.23),
    crouch: -0.01,
    nod: 2.0,
    elbow_out: 0.55,
    ..TALL
};

/// Fists side by side in front of the chest.
const CHEST: DancePose = DancePose {
    left: Vec3::new(0.11, 0.08, 0.26),
    right: Vec3::new(-0.1, 0.1, 0.26),
    crouch: 0.01,
    elbow_out: 0.55,
    ..TALL
};

/// Fists together low in front of the belly, about to go down.
const BELLY: DancePose = DancePose {
    left: Vec3::new(0.1, -0.06, 0.26),
    right: Vec3::new(-0.09, -0.04, 0.26),
    crouch: 0.02,
    bend: 4.0,
    elbow_out: 0.5,
    ..TALL
};

/// Both fists out by the dancer's left shoulder, the right one crossing the chest to sit beside
/// the left: the "shelf" of the BBBB dance.
const SIDE: DancePose = DancePose {
    left: Vec3::new(0.3, 0.22, 0.12),
    right: Vec3::new(0.13, 0.22, 0.22),
    yaw: 12.0,
    roll: -4.0,
    bend: 2.0,
    crouch: 0.02,
    sway: -0.015,
    head_tilt: -4.0,
    nod: 4.0,
    elbow_out: 0.95,
    shrug: [12.0, 4.0],
};

/// The fists thrown up by the head on the dancer's left, leaning into them.
const HIGH_SIDE: DancePose = DancePose {
    left: Vec3::new(0.3, 0.34, 0.1),
    right: Vec3::new(0.14, 0.32, 0.18),
    yaw: 10.0,
    roll: -6.0,
    bend: 0.0,
    crouch: 0.0,
    sway: -0.02,
    head_tilt: 6.0,
    nod: 2.0,
    elbow_out: 1.05,
    shrug: [18.0, 6.0],
};

/// Dropped into the knees, the fists still at chest height out to the dancer's left.
const CROUCH_SIDE: DancePose = DancePose {
    left: Vec3::new(0.3, 0.18, 0.15),
    right: Vec3::new(0.13, 0.17, 0.24),
    yaw: 8.0,
    roll: -1.0,
    bend: 4.0,
    crouch: 0.08,
    sway: 0.0,
    head_tilt: -2.0,
    nod: 5.0,
    elbow_out: 0.9,
    shrug: [8.0, 2.0],
};

/// The deepest drop, the fists low out to the dancer's left.
const LOW_SIDE: DancePose = DancePose {
    left: Vec3::new(0.3, 0.1, 0.18),
    right: Vec3::new(0.13, 0.09, 0.26),
    yaw: 10.0,
    roll: 0.0,
    bend: 8.0,
    crouch: 0.13,
    sway: 0.0,
    head_tilt: -1.0,
    nod: 7.0,
    elbow_out: 0.85,
    shrug: [4.0, 0.0],
};

/// The six-beat phrase of the dance, read off the reference at 12 fps: a side on every beat, the
/// fists passing through the chin, chest or belly on the way across, the body dropping into the
/// knees on beats 2 to 4. `(beat, pose, the move snaps in)`.
fn phrase() -> [(f32, DancePose, bool); 12] {
    [
        (0.0, HIGH_SIDE.mirror(), true),
        (0.65, CHIN, false),
        (1.0, SIDE, true),
        (1.8, CHEST, false),
        (2.05, CROUCH_SIDE.mirror(), true),
        (2.7, BELLY, false),
        (3.0, LOW_SIDE, true),
        (3.75, BELLY, false),
        (
            4.05,
            DancePose {
                crouch: 0.05,
                bend: 5.0,
                ..CROUCH_SIDE
            }
            .mirror(),
            true,
        ),
        (4.65, CHEST, false),
        (5.0, SIDE, true),
        (5.65, CHIN, false),
    ]
}

/// The song beat the phrase starts on (the reference's dance starts its phrase here).
const PHRASE_START: f32 = 164.55;
const PHRASE_BEATS: f32 = 6.0;

/// A dance as a timesheet: the keys of the phrase on the beat grid, in-betweens and holds from
/// [`fframes_anime::koma::Sheet`].
pub struct Dance {
    pub motion: MotionFn,
    pub sheet: crate::actor::SheetFn,
    /// Eased from drawing to drawing, for the hair and cloth.
    pub between: MotionFn,
}

/// The Bling-Bang-Bang-Born dance for a shot whose time 0 is `song_offset` in the song and
/// lasts `len` seconds. `canon` delays the dancer by that many beats (a line in canon).
pub fn bbbb(sk: &Skeleton, song_offset: f32, len: f32, at: Vec3, facing: f32, canon: f32) -> Dance {
    use fframes_anime::koma::{Key, Sheet};
    let fps = crate::actor::FPS;
    let first = ((beat(song_offset) - canon - PHRASE_START) / PHRASE_BEATS).floor() - 1.0;
    let last = ((beat(song_offset + len) - canon - PHRASE_START) / PHRASE_BEATS).ceil() + 1.0;
    let mut keys = Vec::new();
    let mut poses = Vec::new();
    let mut n = first;
    while n <= last {
        for (b, pose, snap) in phrase() {
            let song = beat_time(PHRASE_START + canon + n * PHRASE_BEATS + b);
            keys.push(Key {
                frame: ((song - song_offset) * fps).round() as i32,
                // a side snaps in with an overshoot, the centre is reached through one
                // in-between with most of the way already done (ツメ)
                chart: if snap { vec![] } else { vec![0.6] },
                overshoot: if snap { 0.14 } else { 0.0 },
            });
            poses.push(pose);
        }
        n += 1.0;
    }
    let sheet = Arc::new(Sheet::new(&keys, 2, 3));
    let poses = Arc::new(poses);
    let sk = sk.clone();
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
        let (sheet, index) = (Arc::clone(&sheet), index.clone());
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

/// World rotation of a hand pointing `along` with the palm facing `palm`.
pub fn hand_facing(side: Side, along: Vec3, palm: Vec3) -> Quat {
    let rest_along = Vec3::X * side.sign();
    fframes_anime::rig::frame_rotation(rest_along, Vec3::NEG_Y, along, palm)
}

/// A pose with both hands placed in the chest frame (`None` keeps the arm hanging), feet planted
/// at `ground` when `stance` is set.
#[derive(Clone, Copy)]
pub struct Arms {
    pub left: Option<Vec3>,
    pub right: Option<Vec3>,
    pub elbow_out: f32,
    /// Palm direction in world space (`None` follows the forearm).
    pub left_palm: Option<Vec3>,
    pub right_palm: Option<Vec3>,
    /// Finger direction in world space when a palm is set (`None` = along the forearm).
    pub left_fingers: Option<Vec3>,
    pub right_fingers: Option<Vec3>,
}

impl Default for Arms {
    fn default() -> Self {
        Arms {
            left: None,
            right: None,
            elbow_out: 0.8,
            left_palm: None,
            right_palm: None,
            left_fingers: None,
            right_fingers: None,
        }
    }
}

pub fn build(sk: &Skeleton, body: Body, arms: Arms, feet: Option<(Vec3, f32, f32, f32)>) -> Pose {
    let mut pose = body.pose(sk);
    if let Some((ground, yaw, stance, step)) = feet {
        plant_feet(sk, &mut pose, ground, yaw, stance, step);
    }
    for (side, target, palm, fingers) in [
        (Side::Left, arms.left, arms.left_palm, arms.left_fingers),
        (Side::Right, arms.right, arms.right_palm, arms.right_fingers),
    ] {
        if let Some(local) = target {
            reach_local(sk, &mut pose, side, local, arms.elbow_out);
            if let Some(palm) = palm {
                let lower = pose.get(Bone::arm(side)[2]);
                let along = fingers.unwrap_or(lower * Vec3::X * side.sign());
                pose.rot[Bone::arm(side)[3].index()] = hand_facing(side, along, palm);
            }
        }
    }
    pose
}

/// Wraps a pose function into a motion.
pub fn motion(f: impl Fn(f32) -> Pose + Send + Sync + 'static) -> MotionFn {
    Arc::new(f)
}
