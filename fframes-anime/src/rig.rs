//! A humanoid skeleton with editable proportions, poses as world rotations, forward kinematics,
//! two-bone IK and retargeting of motion capture onto the skeleton.
//!
//! Conventions: metres, Y up, the character faces +Z in the rest pose, its left hand is at +X.
//! The rest pose is a T-pose and a joint rotation of identity means "as in the rest pose", so a
//! pose is the world rotation of every joint plus the hips position.

use glam::{Mat3, Quat, Vec3};

use crate::bvh::{Bvh, JointWorld};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Bone {
    Hips,
    Spine,
    Chest,
    Neck,
    Head,
    ShoulderL,
    UpperArmL,
    LowerArmL,
    HandL,
    ShoulderR,
    UpperArmR,
    LowerArmR,
    HandR,
    UpperLegL,
    LowerLegL,
    FootL,
    ToesL,
    UpperLegR,
    LowerLegR,
    FootR,
    ToesR,
}

pub const BONE_COUNT: usize = 21;

pub const BONES: [Bone; BONE_COUNT] = [
    Bone::Hips,
    Bone::Spine,
    Bone::Chest,
    Bone::Neck,
    Bone::Head,
    Bone::ShoulderL,
    Bone::UpperArmL,
    Bone::LowerArmL,
    Bone::HandL,
    Bone::ShoulderR,
    Bone::UpperArmR,
    Bone::LowerArmR,
    Bone::HandR,
    Bone::UpperLegL,
    Bone::LowerLegL,
    Bone::FootL,
    Bone::ToesL,
    Bone::UpperLegR,
    Bone::LowerLegR,
    Bone::FootR,
    Bone::ToesR,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Left,
    Right,
}

impl Side {
    pub fn sign(self) -> f32 {
        match self {
            Side::Left => 1.0,
            Side::Right => -1.0,
        }
    }
}

impl Bone {
    pub fn index(self) -> usize {
        self as usize
    }

    pub fn parent(self) -> Option<Bone> {
        use Bone::*;
        Some(match self {
            Hips => return None,
            Spine | UpperLegL | UpperLegR => Hips,
            Chest => Spine,
            Neck | ShoulderL | ShoulderR => Chest,
            Head => Neck,
            UpperArmL => ShoulderL,
            LowerArmL => UpperArmL,
            HandL => LowerArmL,
            UpperArmR => ShoulderR,
            LowerArmR => UpperArmR,
            HandR => LowerArmR,
            LowerLegL => UpperLegL,
            FootL => LowerLegL,
            ToesL => FootL,
            LowerLegR => UpperLegR,
            FootR => LowerLegR,
            ToesR => FootR,
        })
    }

    /// The child that defines the direction of this bone, `None` for the ends of chains.
    pub fn tip_child(self) -> Option<Bone> {
        use Bone::*;
        Some(match self {
            Hips => Spine,
            Spine => Chest,
            Chest => Neck,
            Neck => Head,
            ShoulderL => UpperArmL,
            UpperArmL => LowerArmL,
            LowerArmL => HandL,
            ShoulderR => UpperArmR,
            UpperArmR => LowerArmR,
            LowerArmR => HandR,
            UpperLegL => LowerLegL,
            LowerLegL => FootL,
            FootL => ToesL,
            UpperLegR => LowerLegR,
            LowerLegR => FootR,
            FootR => ToesR,
            Head | HandL | HandR | ToesL | ToesR => return None,
        })
    }

    pub fn side(self) -> Option<Side> {
        use Bone::*;
        match self {
            ShoulderL | UpperArmL | LowerArmL | HandL | UpperLegL | LowerLegL | FootL | ToesL => {
                Some(Side::Left)
            }
            ShoulderR | UpperArmR | LowerArmR | HandR | UpperLegR | LowerLegR | FootR | ToesR => {
                Some(Side::Right)
            }
            _ => None,
        }
    }

    pub fn mirror(self) -> Bone {
        use Bone::*;
        match self {
            ShoulderL => ShoulderR,
            UpperArmL => UpperArmR,
            LowerArmL => LowerArmR,
            HandL => HandR,
            UpperLegL => UpperLegR,
            LowerLegL => LowerLegR,
            FootL => FootR,
            ToesL => ToesR,
            ShoulderR => ShoulderL,
            UpperArmR => UpperArmL,
            LowerArmR => LowerArmL,
            HandR => HandL,
            UpperLegR => UpperLegL,
            LowerLegR => LowerLegL,
            FootR => FootL,
            ToesR => ToesL,
            other => other,
        }
    }

    /// Joint names of the Bandai Namco Research motion datasets (and many Maya rigs).
    pub fn bandai_namco_name(self) -> &'static str {
        use Bone::*;
        match self {
            Hips => "Hips",
            Spine => "Spine",
            Chest => "Chest",
            Neck => "Neck",
            Head => "Head",
            ShoulderL => "Shoulder_L",
            UpperArmL => "UpperArm_L",
            LowerArmL => "LowerArm_L",
            HandL => "Hand_L",
            ShoulderR => "Shoulder_R",
            UpperArmR => "UpperArm_R",
            LowerArmR => "LowerArm_R",
            HandR => "Hand_R",
            UpperLegL => "UpperLeg_L",
            LowerLegL => "LowerLeg_L",
            FootL => "Foot_L",
            ToesL => "Toes_L",
            UpperLegR => "UpperLeg_R",
            LowerLegR => "LowerLeg_R",
            FootR => "Foot_R",
            ToesR => "Toes_R",
        }
    }

    pub fn arm(side: Side) -> [Bone; 4] {
        match side {
            Side::Left => [
                Bone::ShoulderL,
                Bone::UpperArmL,
                Bone::LowerArmL,
                Bone::HandL,
            ],
            Side::Right => [
                Bone::ShoulderR,
                Bone::UpperArmR,
                Bone::LowerArmR,
                Bone::HandR,
            ],
        }
    }

    pub fn leg(side: Side) -> [Bone; 4] {
        match side {
            Side::Left => [Bone::UpperLegL, Bone::LowerLegL, Bone::FootL, Bone::ToesL],
            Side::Right => [Bone::UpperLegR, Bone::LowerLegR, Bone::FootR, Bone::ToesR],
        }
    }
}

/// Lengths of the skeleton in metres.
#[derive(Debug, Clone, Copy)]
pub struct Proportions {
    pub hip_height: f32,
    pub hip_width: f32,
    pub spine: f32,
    pub chest: f32,
    pub neck: f32,
    /// From the neck joint to the head joint (base of the skull).
    pub head_offset: f32,
    /// Height of the head from the head joint to the top of the skull.
    pub head_height: f32,
    pub clavicle: f32,
    pub shoulder_width: f32,
    pub upper_arm: f32,
    pub forearm: f32,
    pub hand: f32,
    pub thigh: f32,
    pub shin: f32,
    pub ankle_height: f32,
    pub foot: f32,
}

impl Proportions {
    /// A tall anime adult, about 7.5 heads: long legs, narrow hips.
    pub fn anime_adult() -> Self {
        Proportions {
            hip_height: 1.0,
            hip_width: 0.085,
            spine: 0.11,
            chest: 0.16,
            neck: 0.2,
            head_offset: 0.065,
            head_height: 0.26,
            clavicle: 0.02,
            shoulder_width: 0.12,
            upper_arm: 0.29,
            forearm: 0.27,
            hand: 0.165,
            thigh: 0.45,
            shin: 0.45,
            ankle_height: 0.075,
            foot: 0.13,
        }
    }

    pub fn scaled(self, s: f32) -> Self {
        Proportions {
            hip_height: self.hip_height * s,
            hip_width: self.hip_width * s,
            spine: self.spine * s,
            chest: self.chest * s,
            neck: self.neck * s,
            head_offset: self.head_offset * s,
            head_height: self.head_height * s,
            clavicle: self.clavicle * s,
            shoulder_width: self.shoulder_width * s,
            upper_arm: self.upper_arm * s,
            forearm: self.forearm * s,
            hand: self.hand * s,
            thigh: self.thigh * s,
            shin: self.shin * s,
            ankle_height: self.ankle_height * s,
            foot: self.foot * s,
        }
    }
}

/// Rest offsets of every joint from its parent (in the parent's rest frame, which is the world
/// frame in the T-pose).
#[derive(Debug, Clone)]
pub struct Skeleton {
    pub proportions: Proportions,
    pub offsets: [Vec3; BONE_COUNT],
}

impl Skeleton {
    pub fn new(p: Proportions) -> Self {
        use Bone::*;
        let mut offsets = [Vec3::ZERO; BONE_COUNT];
        let mut set = |b: Bone, v: Vec3| offsets[b.index()] = v;
        set(Hips, Vec3::new(0.0, p.hip_height, 0.0));
        set(Spine, Vec3::new(0.0, p.spine, 0.0));
        set(Chest, Vec3::new(0.0, p.chest, 0.0));
        set(Neck, Vec3::new(0.0, p.neck, 0.0));
        set(Head, Vec3::new(0.0, p.head_offset, 0.0));
        for side in [Side::Left, Side::Right] {
            let s = side.sign();
            let [shoulder, upper, lower, hand] = Bone::arm(side);
            set(shoulder, Vec3::new(p.clavicle * s, p.neck * 0.8, 0.0));
            set(upper, Vec3::new(p.shoulder_width * s, 0.0, 0.0));
            set(lower, Vec3::new(p.upper_arm * s, 0.0, 0.0));
            set(hand, Vec3::new(p.forearm * s, 0.0, 0.0));
            let [thigh, shin, foot, toes] = Bone::leg(side);
            set(thigh, Vec3::new(p.hip_width * s, -0.06, 0.0));
            set(shin, Vec3::new(0.0, -p.thigh, 0.0));
            set(foot, Vec3::new(0.0, -p.shin, 0.0));
            set(toes, Vec3::new(0.0, -p.ankle_height, p.foot));
        }
        Skeleton {
            proportions: p,
            offsets,
        }
    }

    pub fn offset(&self, bone: Bone) -> Vec3 {
        self.offsets[bone.index()]
    }

    /// Direction of a bone in the rest pose.
    pub fn rest_direction(&self, bone: Bone) -> Vec3 {
        match bone.tip_child() {
            Some(child) => self.offset(child).normalize(),
            None => match bone {
                Bone::Head => Vec3::Y,
                Bone::HandL => Vec3::X,
                Bone::HandR => Vec3::NEG_X,
                _ => Vec3::Z,
            },
        }
    }

    /// The rest pose: every rotation identity.
    pub fn rest_pose(&self) -> Pose {
        Pose {
            root: self.offset(Bone::Hips),
            rot: [Quat::IDENTITY; BONE_COUNT],
        }
    }

    /// World positions of every joint for a pose.
    pub fn solve(&self, pose: &Pose) -> Posed {
        let mut p = [Vec3::ZERO; BONE_COUNT];
        for bone in BONES {
            p[bone.index()] = match bone.parent() {
                None => pose.root,
                Some(parent) => p[parent.index()] + pose.rot[parent.index()] * self.offset(bone),
            };
        }
        Posed {
            p,
            r: pose.rot,
            proportions: self.proportions,
        }
    }
}

/// World rotation of every joint and the hips position.
#[derive(Debug, Clone, Copy)]
pub struct Pose {
    pub root: Vec3,
    pub rot: [Quat; BONE_COUNT],
}

impl Pose {
    pub fn get(&self, bone: Bone) -> Quat {
        self.rot[bone.index()]
    }

    /// Spherical blend of two poses.
    pub fn blend(&self, other: &Pose, t: f32) -> Pose {
        let mut rot = self.rot;
        for (r, o) in rot.iter_mut().zip(other.rot.iter()) {
            *r = r.slerp(*o, t);
        }
        Pose {
            root: self.root.lerp(other.root, t),
            rot,
        }
    }

    /// Rotates `bone` and everything below it by `delta` (applied in world space around the joint).
    pub fn rotate_subtree(&mut self, bone: Bone, delta: Quat) {
        for b in BONES {
            if b == bone || is_descendant(b, bone) {
                self.rot[b.index()] = delta * self.rot[b.index()];
            }
        }
    }

    /// Moves and turns the whole pose: `transform` is applied around the world origin.
    pub fn transformed(&self, rotation: Quat, translation: Vec3) -> Pose {
        let mut rot = self.rot;
        for r in &mut rot {
            *r = rotation * *r;
        }
        Pose {
            root: rotation * self.root + translation,
            rot,
        }
    }

    /// Sets a joint from a rotation local to its parent.
    pub fn set_local(&mut self, bone: Bone, local: Quat) {
        let parent = bone.parent().map_or(Quat::IDENTITY, |p| self.get(p));
        self.rot[bone.index()] = parent * local;
    }

    /// Rotation of a joint relative to its parent.
    pub fn local(&self, bone: Bone) -> Quat {
        let parent = bone.parent().map_or(Quat::IDENTITY, |p| self.get(p));
        parent.inverse() * self.get(bone)
    }

    /// Replaces the local rotation of `bone` and carries its children along.
    pub fn set_local_keep_children(&mut self, bone: Bone, local: Quat) {
        let before = self.get(bone);
        self.set_local(bone, local);
        let delta = self.get(bone) * before.inverse();
        for b in BONES {
            if is_descendant(b, bone) {
                self.rot[b.index()] = delta * self.rot[b.index()];
            }
        }
    }
}

pub fn is_descendant(bone: Bone, ancestor: Bone) -> bool {
    let mut cur = bone.parent();
    while let Some(b) = cur {
        if b == ancestor {
            return true;
        }
        cur = b.parent();
    }
    false
}

/// Joint positions and frames of a solved pose.
#[derive(Debug, Clone, Copy)]
pub struct Posed {
    pub p: [Vec3; BONE_COUNT],
    pub r: [Quat; BONE_COUNT],
    pub proportions: Proportions,
}

impl Posed {
    pub fn pos(&self, bone: Bone) -> Vec3 {
        self.p[bone.index()]
    }

    pub fn rot(&self, bone: Bone) -> Quat {
        self.r[bone.index()]
    }

    /// Point `local` (in the bone's rest frame) attached to `bone`.
    pub fn attach(&self, bone: Bone, local: Vec3) -> Vec3 {
        self.pos(bone) + self.rot(bone) * local
    }

    /// End of a hand (finger tips) in world space.
    pub fn hand_tip(&self, side: Side) -> Vec3 {
        let hand = Bone::arm(side)[3];
        self.attach(
            hand,
            Vec3::new(self.proportions.hand * side.sign(), 0.0, 0.0),
        )
    }

    /// Top of the skull.
    pub fn head_top(&self) -> Vec3 {
        self.attach(
            Bone::Head,
            Vec3::new(0.0, self.proportions.head_height, 0.0),
        )
    }

    /// Facing direction of the chest.
    pub fn chest_forward(&self) -> Vec3 {
        self.rot(Bone::Chest) * Vec3::Z
    }
}

/// Rotation that turns `from` into `to` with the least twist.
pub fn arc(from: Vec3, to: Vec3) -> Quat {
    Quat::from_rotation_arc(from.normalize_or_zero(), to.normalize_or_zero())
}

/// Rotation mapping the rest frame (`rest_dir`, `rest_up`) onto (`dir`, `up`).
pub fn frame_rotation(rest_dir: Vec3, rest_up: Vec3, dir: Vec3, up: Vec3) -> Quat {
    let basis = |d: Vec3, u: Vec3| {
        let d = d.normalize_or_zero();
        let u = (u - d * u.dot(d)).normalize_or_zero();
        Mat3::from_cols(d, u, d.cross(u))
    };
    let a = basis(rest_dir, rest_up);
    let b = basis(dir, up);
    Quat::from_mat3(&(b * a.transpose())).normalize()
}

/// Two-bone IK for an arm or a leg: turns the upper and lower bones so the end reaches `target`,
/// bending towards `pole`. The end joint (hand, foot) follows the lower bone unless
/// `end_rotation` sets its world rotation.
pub fn two_bone_ik(
    skeleton: &Skeleton,
    pose: &mut Pose,
    upper: Bone,
    target: Vec3,
    pole: Vec3,
    end_rotation: Option<Quat>,
) {
    let lower = upper
        .tip_child()
        .expect("two bone IK needs a bone with a child");
    let end = lower.tip_child().expect("two bone IK needs two bones");
    let solved = skeleton.solve(pose);
    let root = solved.pos(upper);
    let a = skeleton.offset(lower).length();
    let b = skeleton.offset(end).length();
    let to_target = target - root;
    let c = to_target.length().clamp(1e-4, (a + b) * 0.9995);
    let dir = to_target.normalize_or(Vec3::NEG_Y);
    let cos_alpha = ((a * a + c * c - b * b) / (2.0 * a * c)).clamp(-1.0, 1.0);
    let sin_alpha = (1.0 - cos_alpha * cos_alpha).sqrt();
    let pole_dir = {
        let p = pole - root;
        let ortho = p - dir * p.dot(dir);
        if ortho.length_squared() < 1e-8 {
            dir.any_orthonormal_vector()
        } else {
            ortho.normalize()
        }
    };
    let elbow = root + dir * (a * cos_alpha) + pole_dir * (a * sin_alpha);
    let hand = root + dir * c;

    let upper_rest = skeleton.offset(lower).normalize();
    let lower_rest = skeleton.offset(end).normalize();
    // Keep the bend plane: the rest "up" of the bones is the pole direction after rotation.
    let rest_up = rest_bend_axis(upper);
    let upper_rot = frame_rotation(upper_rest, rest_up, elbow - root, pole_dir);
    let lower_rot = frame_rotation(lower_rest, rest_up, hand - elbow, pole_dir);
    let end_before = pose.get(end);
    pose.rot[upper.index()] = upper_rot;
    pose.rot[lower.index()] = lower_rot;
    pose.rot[end.index()] = end_rotation.unwrap_or(lower_rot);
    // Fingers and toes follow the end joint.
    if let Some(after) = end.tip_child() {
        let delta = pose.get(end) * end_before.inverse();
        pose.rot[after.index()] = delta * pose.get(after);
    }
}

/// The direction a limb bends towards in the rest pose (elbows bend back, knees forward).
fn rest_bend_axis(upper: Bone) -> Vec3 {
    match upper {
        Bone::UpperLegL | Bone::UpperLegR => Vec3::Z,
        _ => Vec3::NEG_Z,
    }
}

/// Motion capture mapped onto a [`Skeleton`]: every joint's world rotation is the BVH joint's
/// rotation times a constant correction found once from a calibration frame, positions are
/// rebuilt with the target skeleton's lengths so the proportions change but the motion keeps
/// its directions.
#[derive(Debug, Clone)]
pub struct Retarget {
    joint_index: [usize; BONE_COUNT],
    correction: [Quat; BONE_COUNT],
    /// BVH world -> our world (turns the clip to face +Z).
    facing: Quat,
    origin: Vec3,
    root_scale: f32,
}

impl Retarget {
    /// Calibrates against `frame` of `bvh` (the actor should stand roughly upright with the arms
    /// down, the first frame of most clips does). `skeleton` provides the target lengths.
    pub fn new(bvh: &Bvh, skeleton: &Skeleton, frame: usize) -> Option<Self> {
        let mut joint_index = [0; BONE_COUNT];
        for bone in BONES {
            joint_index[bone.index()] = bvh.joint(bone.bandai_namco_name())?;
        }
        let world = bvh.world_at_frame(frame);
        let at = |b: Bone| world[joint_index[b.index()]];

        let up = Vec3::Y;
        let left = (at(Bone::UpperLegL).position - at(Bone::UpperLegR).position)
            .reject_from(up)
            .normalize();
        // X (left) x Y (up) = Z (forward).
        let forward = left.cross(up).normalize();

        let mut correction = [Quat::IDENTITY; BONE_COUNT];
        for bone in BONES {
            let w: JointWorld = at(bone);
            let world_dir = match bone.tip_child() {
                Some(child) => (at(child).position - w.position).normalize(),
                None => match bone.parent().and_then(|p| p.tip_child().map(|_| p)) {
                    // Ends of chains continue their parent's direction (head up the neck,
                    // hands along the forearm, toes along the foot).
                    Some(parent) => (w.position - at(parent).position).normalize(),
                    None => up,
                },
            };
            let world_dir = if bone == Bone::Head { up } else { world_dir };
            let (rest_dir, rest_secondary, world_secondary) = match bone {
                Bone::FootL | Bone::FootR | Bone::ToesL | Bone::ToesR => {
                    (skeleton.rest_direction(bone), Vec3::Y, up)
                }
                _ => (skeleton.rest_direction(bone), Vec3::Z, forward),
            };
            // Which local axis of the BVH joint carries the bone and which the secondary
            // direction.
            let axes = [
                Vec3::X,
                Vec3::NEG_X,
                Vec3::Y,
                Vec3::NEG_Y,
                Vec3::Z,
                Vec3::NEG_Z,
            ];
            let primary = *axes
                .iter()
                .max_by(|a, b| {
                    (w.rotation * **a)
                        .dot(world_dir)
                        .total_cmp(&(w.rotation * **b).dot(world_dir))
                })
                .unwrap();
            let secondary_target = world_secondary.reject_from(world_dir).normalize_or_zero();
            let secondary = *axes
                .iter()
                .filter(|a| a.dot(primary).abs() < 0.5)
                .max_by(|a, b| {
                    (w.rotation * **a)
                        .dot(secondary_target)
                        .total_cmp(&(w.rotation * **b).dot(secondary_target))
                })
                .unwrap();
            // C maps our rest frame onto the BVH joint's local frame.
            correction[bone.index()] = frame_rotation(rest_dir, rest_secondary, primary, secondary);
        }

        let facing = arc(forward.reject_from(up), Vec3::Z);
        let origin = Vec3::new(at(Bone::Hips).position.x, 0.0, at(Bone::Hips).position.z);
        let bvh_leg = (at(Bone::UpperLegL).position - at(Bone::LowerLegL).position).length()
            + (at(Bone::LowerLegL).position - at(Bone::FootL).position).length();
        let our_leg = skeleton.proportions.thigh + skeleton.proportions.shin;
        Some(Retarget {
            joint_index,
            correction,
            facing,
            origin,
            root_scale: our_leg / bvh_leg,
        })
    }

    /// The pose at `seconds` into the clip.
    pub fn pose_at(&self, bvh: &Bvh, seconds: f32) -> Pose {
        let world = bvh.world_at(seconds);
        self.pose_from_world(&world)
    }

    pub fn pose_from_world(&self, world: &[JointWorld]) -> Pose {
        let mut rot = [Quat::IDENTITY; BONE_COUNT];
        for bone in BONES {
            let w = world[self.joint_index[bone.index()]];
            rot[bone.index()] =
                (self.facing * w.rotation * self.correction[bone.index()]).normalize();
        }
        let hips = world[self.joint_index[Bone::Hips.index()]].position;
        let root = self.facing * (hips - self.origin) * self.root_scale;
        Pose { root, rot }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rest_pose_has_expected_height() {
        let sk = Skeleton::new(Proportions::anime_adult());
        let posed = sk.solve(&sk.rest_pose());
        let top = posed.head_top().y;
        assert!(top > 1.7 && top < 1.9, "{top}");
        assert!(posed.pos(Bone::ToesL).y.abs() < 0.05);
        assert!(posed.pos(Bone::HandL).x > 0.5);
    }

    #[test]
    fn ik_reaches_target() {
        let sk = Skeleton::new(Proportions::anime_adult());
        let mut pose = sk.rest_pose();
        let target = Vec3::new(0.25, 1.3, 0.3);
        two_bone_ik(
            &sk,
            &mut pose,
            Bone::UpperArmL,
            target,
            Vec3::new(0.6, 1.0, -0.5),
            None,
        );
        let posed = sk.solve(&pose);
        assert!((posed.pos(Bone::HandL) - target).length() < 1e-3);
    }
}
