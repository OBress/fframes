//! Character design: what a toon character is made of (hair locks, glasses, beard, robe, tie,
//! build), its expression and hand shapes per frame ([`Look`]), and the spring bones and cloth
//! that give it secondary motion. [`crate::figure`] builds the meshes from a [`Design`].

use glam::{Quat, Vec3};

use crate::rig::{Bone, Posed, Proportions};
use crate::spring::{Anchor, BodyState, Cloth, Collider, Rig, SpringChain, SpringParams};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Material {
    /// Jacket, trousers, shoes.
    Suit,
    Robe,
    /// The inside of the robe.
    RobeLining,
    Shirt,
    Tie,
    Skin,
    Hair,
    Beard,
    Scarf,
    /// Dark details (face features, the inside of a mouth).
    Ink,
    /// Light details (eye whites, teeth, highlights).
    Paper,
    /// Glowing eyes, emblems.
    Accent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandShape {
    Fist,
    Open,
    Point,
    Peace,
    /// Relaxed, fingers slightly curled.
    Relaxed,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Mouth {
    Line,
    Smile,
    Grin,
    /// Open mouth, `0..1`.
    Open(f32),
    Frown,
    /// A roaring mouth with teeth and fangs, `0..1`.
    Roar(f32),
    /// A talking or singing mouth: `open` 0..1, `wide` 0 (a round "o"/"u") to 1 (a flat
    /// "i"/"e" showing the upper teeth). See [`Mouth::vowel`].
    Talk {
        open: f32,
        wide: f32,
    },
    /// A lopsided smile, the corner on the dancer's left (1) or right (-1) side pulled up.
    Smirk(f32),
    /// Clenched teeth, `0..1` how far the lips pull back.
    Teeth(f32),
    /// A nervous wobbly line.
    Wavy,
    /// The cat mouth (ω): smug or cute.
    Cat,
    /// A small round "o", `0..1`.
    O(f32),
    /// Lips pushed out, sulking.
    Pout,
}

impl Mouth {
    /// The mouth shape of a sung or spoken vowel (`a i u e o`, `n`/`m` closed).
    pub fn vowel(v: char) -> Mouth {
        let (open, wide) = match v {
            'a' => (0.95, 0.6),
            'i' => (0.28, 1.0),
            'u' => (0.32, 0.05),
            'e' => (0.55, 0.85),
            'o' => (0.72, 0.25),
            _ => (0.0, 0.5),
        };
        Mouth::Talk { open, wide }
    }

    /// How far the jaw is open, for blending.
    fn openness(&self) -> Option<(f32, f32)> {
        match *self {
            Mouth::Talk { open, wide } => Some((open, wide)),
            Mouth::Open(o) => Some((o, 0.5)),
            _ => None,
        }
    }
}

/// Expression and hands for one frame.
#[derive(Debug, Clone, Copy)]
pub struct Look {
    /// 1 = open, 0 = closed.
    pub eyes: f32,
    /// Pupil size, 1 = normal, 0.4 = shocked.
    pub pupils: f32,
    /// Both brows up (1) or down (-1).
    pub brows: f32,
    /// Brow angle: 1 = angry (inner ends down), -1 = worried (inner ends up). The upper lids
    /// follow.
    pub brow_angle: f32,
    /// One brow up and the other down (sceptical): positive raises the left brow.
    pub brow_tilt: f32,
    pub mouth: Mouth,
    pub hand_l: HandShape,
    pub hand_r: HandShape,
    /// Draw face features (off for pure silhouettes and far shots).
    pub face: bool,
    /// Lens glint on glasses, 0..1.
    pub glint: f32,
    /// Closed eyes curve up (laughing) instead of down (asleep).
    pub happy: bool,
    /// Irises in the accent colour (glowing eyes).
    pub accent_eyes: bool,
    /// Where the eyes look, in the head's frame: x towards the character's left, y up, about
    /// -1..1 (the iris at the corner of the eye).
    pub gaze: glam::Vec2,
    /// Heavy upper lids (bored, smug, sleepy), 0..1.
    pub lids: f32,
    /// Lower lids pushed up (a smile in the eyes, or focus), 0..1.
    pub squint: f32,
    /// Closes one eye: positive the left, negative the right.
    pub wink: f32,
    /// Hatched blush lines on the cheeks, 0..1.
    pub blush: f32,
    /// A sweat drop at the temple, 0..1 (its size).
    pub sweat: f32,
    /// Vertical gloom lines and shadow over the upper face (shock, dread), 0..1.
    pub gloom: f32,
    /// An anger mark on the forehead, 0..1.
    pub vein: f32,
    /// Star highlights in the eyes, 0..1.
    pub sparkle: f32,
    /// Teary eyes and tear streams, 0..1.
    pub tears: f32,
    /// Size of the hands (1 = as modelled): anime draws fists thrown at the camera bigger.
    pub fists: f32,
    /// The whites of the eyes turn the accent colour and the irises light up in the paper
    /// colour with a pin of a pupil (a demonic red eye).
    pub bloodshot: f32,
}

impl Default for Look {
    fn default() -> Self {
        Look {
            eyes: 1.0,
            pupils: 1.0,
            brows: 0.0,
            brow_angle: 0.0,
            brow_tilt: 0.0,
            mouth: Mouth::Line,
            hand_l: HandShape::Relaxed,
            hand_r: HandShape::Relaxed,
            face: true,
            glint: 0.0,
            happy: false,
            accent_eyes: false,
            gaze: glam::Vec2::ZERO,
            lids: 0.0,
            squint: 0.0,
            wink: 0.0,
            blush: 0.0,
            sweat: 0.0,
            gloom: 0.0,
            vein: 0.0,
            sparkle: 0.0,
            tears: 0.0,
            fists: 1.0,
            bloodshot: 0.0,
        }
    }
}

impl Look {
    /// How open the eye on `side` is (1 = the left eye, -1 = the right).
    pub fn eye_open(&self, side: f32) -> f32 {
        let wink = if side > 0.0 {
            self.wink.max(0.0)
        } else {
            (-self.wink).max(0.0)
        };
        (self.eyes * (1.0 - wink)).clamp(0.0, 1.0)
    }

    /// An in-between of two expressions: the continuous parts blend, the mouth and the
    /// switches change half way (a jaw opening and closing blends too).
    pub fn lerp(&self, o: &Look, t: f32) -> Look {
        fn step<T>(t: f32, a: T, b: T) -> T {
            if t < 0.5 { a } else { b }
        }
        let f = |a: f32, b: f32| a + (b - a) * t;
        let mouth = match (self.mouth.openness(), o.mouth.openness()) {
            (Some((a0, w0)), Some((a1, w1))) => Mouth::Talk {
                open: f(a0, a1),
                wide: f(w0, w1),
            },
            _ => step(t, self.mouth, o.mouth),
        };
        Look {
            eyes: f(self.eyes, o.eyes),
            pupils: f(self.pupils, o.pupils),
            brows: f(self.brows, o.brows),
            brow_angle: f(self.brow_angle, o.brow_angle),
            brow_tilt: f(self.brow_tilt, o.brow_tilt),
            mouth,
            hand_l: step(t, self.hand_l, o.hand_l),
            hand_r: step(t, self.hand_r, o.hand_r),
            face: step(t, self.face, o.face),
            glint: f(self.glint, o.glint),
            happy: step(t, self.happy, o.happy),
            accent_eyes: step(t, self.accent_eyes, o.accent_eyes),
            gaze: self.gaze.lerp(o.gaze, t),
            lids: f(self.lids, o.lids),
            squint: f(self.squint, o.squint),
            wink: f(self.wink, o.wink),
            blush: f(self.blush, o.blush),
            sweat: f(self.sweat, o.sweat),
            gloom: f(self.gloom, o.gloom),
            vein: f(self.vein, o.vein),
            sparkle: f(self.sparkle, o.sparkle),
            tears: f(self.tears, o.tears),
            fists: f(self.fists, o.fists),
            bloodshot: f(self.bloodshot, o.bloodshot),
        }
    }
}

/// A clump of hair: a spring chain rooted on the scalp.
#[derive(Debug, Clone, Copy)]
pub struct Clump {
    /// Root on the skull, in the head frame.
    pub root: Vec3,
    /// Rest direction (head frame).
    pub dir: Vec3,
    pub length: f32,
    pub width: f32,
    /// Bend applied to every next link (radians around the head's X axis), curls the clump.
    pub curl: f32,
    pub stiffness: f32,
}

impl Clump {
    /// The chain in the rest pose (head frame): `segments + 1` points from the root. Every
    /// link bends by `curl` towards the skull (negative: away from it) and slides over the head
    /// instead of entering it, so locks lie on the hair and hang over the forehead.
    pub fn rest_points(&self, segments: usize) -> Vec<Vec3> {
        let mut rest = vec![self.root];
        let mut dir = self.dir.normalize();
        let step = self.length / segments as f32;
        for k in 0..segments {
            let p = *rest.last().unwrap();
            let mut next = p + dir * step;
            // the shell the lock lies on: the skull (deeper in front for the face), lifted a
            // little more towards the tip
            let lift = 1.0 + 0.012 + 0.03 * (k + 1) as f32 / segments as f32;
            let q = (next - HAIR_SHELL_CENTER) / (HAIR_SHELL * lift);
            if q.length() < 1.0 {
                next = HAIR_SHELL_CENTER + q.normalize() * HAIR_SHELL * lift;
                let len = (next - p).length();
                next = p + (next - p) * (step / len.max(1e-6));
            }
            rest.push(next);
            dir = (next - p).normalize_or(dir);
            let radial = (next - SKULL_CENTER).normalize_or(Vec3::Y);
            let axis = dir.cross(radial).normalize_or(Vec3::X);
            dir = (Quat::from_axis_angle(axis, -self.curl) * dir).normalize();
        }
        rest
    }
}

const HAIR_SHELL_CENTER: Vec3 = Vec3::new(0.0, 0.127, -0.012);
const HAIR_SHELL: Vec3 = Vec3::new(0.087, 0.097, 0.108);

#[derive(Debug, Clone)]
pub struct Hair {
    /// Skull cap: hair covers the sphere above the plane `normal · (p - center) > offset`.
    pub cap_normal: Vec3,
    pub cap_offset: f32,
    /// How much bigger than the skull the hair volume is.
    pub volume: f32,
    /// Length of the jagged tufts along the front hairline (0 = a clean edge).
    pub fringe: f32,
    pub clumps: Vec<Clump>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Glasses {
    Round,
    Rect,
}

#[derive(Debug, Clone)]
pub struct Design {
    pub hair: Hair,
    pub glasses: Option<Glasses>,
    /// Beard coverage 0 (none) .. 1 (full).
    pub beard: f32,
    pub robe: bool,
    pub scarf: bool,
    pub tie: bool,
    /// Robe length from the shoulders (m).
    pub robe_length: f32,
    /// Overall girth multiplier.
    pub build: f32,
    /// Bulging muscles with ink detail (giants, demons).
    pub muscles: bool,
    /// The hair is a hood: the face inside is in shadow, eyes glow.
    pub hood: bool,
    /// A creature of glossy goo (with `muscles`): lumpy dripping body, too many eyes, too many
    /// teeth, six fingers.
    pub slime: bool,
}

/// Size of a head relative to the default design (heads, hair and faces are modelled for a head
/// 0.235 m tall and scaled to the skeleton's).
pub fn head_scale(p: &Proportions) -> f32 {
    p.head_height / 0.235
}

/// Links of every hair chain.
pub const HAIR_SEGMENTS: usize = 4;

/// Anchors used by the physics rig.
pub const ANCHOR_HEAD: usize = 0;
pub const ANCHOR_CHEST: usize = 1;
pub const ANCHOR_HIPS: usize = 2;
pub const ANCHOR_NECK: usize = 3;

/// Indices of the generated chains and cloths, so drawing can find them.
#[derive(Debug, Clone, Default)]
pub struct RigLayout {
    pub hair: std::ops::Range<usize>,
    pub tie: Option<usize>,
    pub scarf: Option<(usize, usize)>,
    pub robe: Option<usize>,
    pub robe_rows: usize,
    pub robe_cols: usize,
}

const SKULL_CENTER: Vec3 = Vec3::new(0.0, 0.12, -0.005);

impl Design {
    /// Builds the springs and cloth of this design for a skeleton with these proportions (the
    /// hair scales with the head).
    pub fn physics(&self, proportions: &Proportions) -> (Rig, RigLayout) {
        let hs = head_scale(proportions);
        let mut rig = Rig::default();
        let mut layout = RigLayout::default();
        let hair_start = rig.chains.len();
        for clump in &self.hair.clumps {
            let rest: Vec<Vec3> = clump
                .rest_points(HAIR_SEGMENTS)
                .into_iter()
                .map(|p| p * hs)
                .collect();
            rig.chains.push(SpringChain {
                anchor: ANCHOR_HEAD,
                rest,
                params: SpringParams {
                    stiffness: clump.stiffness,
                    damping: 3.0,
                    gravity: Vec3::new(0.0, -9.8, 0.0),
                    radius: 0.006,
                    joint_damping: 10.0,
                },
                colliders: Some(vec![0, 8]),
            });
        }
        layout.hair = hair_start..rig.chains.len();

        if self.tie {
            // Knot at the collar, hanging down the shirt (chest frame).
            let top = Vec3::new(0.0, 0.232, 0.096);
            let rest: Vec<Vec3> = (0..6)
                .map(|i| top + Vec3::new(0.0, -0.07 * i as f32, 0.003 * i as f32))
                .collect();
            layout.tie = Some(rig.chains.len());
            rig.chains.push(SpringChain {
                anchor: ANCHOR_CHEST,
                rest,
                // The bending is critically damped (joint damping about 2·√(stiffness / link
                // length)): an underdamped tie rings at 6-7 Hz, which on twos reads as the tie
                // thrashing about.
                params: SpringParams {
                    stiffness: 120.0,
                    damping: 5.0,
                    gravity: Vec3::new(0.0, -9.8, 0.0),
                    radius: 0.01,
                    joint_damping: 75.0,
                },
                colliders: Some(vec![1, 2]),
            });
        }

        if self.scarf {
            let mut ends = [0; 2];
            for (i, x) in [0.035_f32, -0.03].into_iter().enumerate() {
                let top = Vec3::new(x, 0.02, 0.12);
                let rest: Vec<Vec3> = (0..6)
                    .map(|k| top + Vec3::new(x * 0.3, -0.07 * k as f32, 0.05 + 0.006 * k as f32))
                    .collect();
                ends[i] = rig.chains.len();
                rig.chains.push(SpringChain {
                    anchor: ANCHOR_NECK,
                    rest,
                    params: SpringParams {
                        stiffness: 25.0,
                        damping: 1.6,
                        gravity: Vec3::new(0.0, -9.8, 0.0),
                        radius: 0.012,
                        joint_damping: 8.0,
                    },
                    colliders: Some(vec![1, 2]),
                });
            }
            layout.scarf = Some((ends[0], ends[1]));
        }

        if self.robe {
            let rows = 12;
            let cols = 19;
            let length = self.robe_length;
            let b = self.build;
            let cloth = Cloth::grid(rows, cols, ANCHOR_CHEST, &[0, 1], |r, c| {
                // Open at the front: from the left front edge around the back to the right one,
                // wide enough that the shirt and tie show down the chest
                let a0 = 21_f32.to_radians();
                let a1 = (360.0 - 21.0_f32).to_radians();
                let a = a0 + (a1 - a0) * c as f32 / (cols - 1) as f32;
                let side = a.sin().abs();
                if r == 0 {
                    // the collar, close around the base of the neck
                    return Vec3::new(a.sin() * 0.07 * b, 0.215, a.cos() * 0.066 * b - 0.008);
                }
                let t = (r - 1) as f32 / (rows - 2) as f32;
                // the yoke over the shoulders (lower at the sides), then an A-line to the hem
                let slope = 0.03 * side * side * (1.0 - t * 4.0).max(0.0);
                let flare = 1.0 + t * t * 1.4 + t * 0.3;
                let rx = (0.178 - 0.02 * (t * 5.0).min(1.0)) * flare * b;
                let rz = 0.114 * flare * b;
                Vec3::new(
                    a.sin() * rx,
                    0.165 - slope - t * (length - 0.05),
                    a.cos() * rz,
                )
            });
            let mut cloth = cloth;
            cloth.damping = 1.2;
            cloth.radius = 0.03;
            cloth.iterations = 10;
            cloth.wind_factor = 2.4;
            cloth.shape = 5.0;
            cloth.colliders = Some(vec![1, 2, 3, 4, 5, 6, 7]);
            layout.robe = Some(rig.cloths.len());
            layout.robe_rows = rows;
            layout.robe_cols = cols;
            rig.cloths.push(cloth);
        }
        (rig, layout)
    }

    /// Anchors and colliders for the simulation from a solved pose.
    pub fn body_state(&self, posed: &Posed, wind: Vec3) -> BodyState {
        let anchor = |b: Bone| Anchor {
            position: posed.pos(b),
            rotation: posed.rot(b),
        };
        let chest = anchor(Bone::Chest);
        let neck = Anchor {
            position: posed.pos(Bone::Neck),
            rotation: posed.rot(Bone::Neck),
        };
        let b = self.build;
        let hs = head_scale(&posed.proportions);
        let colliders = vec![
            // 0: skull, matching the modelled cranium
            Collider::capsule(
                posed.attach(Bone::Head, Vec3::new(0.0, 0.11, -0.014) * hs),
                posed.attach(Bone::Head, Vec3::new(0.0, 0.142, -0.01) * hs),
                0.083 * hs,
            ),
            // 1: torso (its top ends at the shoulder line, not above the neck)
            Collider::capsule(
                posed.attach(Bone::Hips, Vec3::new(0.0, 0.0, 0.0)),
                posed.attach(Bone::Chest, Vec3::new(0.0, 0.04, -0.01)),
                0.098 * b,
            ),
            // 2: belly/front
            Collider::capsule(
                posed.attach(Bone::Spine, Vec3::new(0.0, -0.05, 0.0)),
                posed.attach(Bone::Chest, Vec3::new(0.0, -0.02, 0.0)),
                0.084 * b,
            ),
            // 3, 4: thighs
            Collider::capsule(
                posed.pos(Bone::UpperLegL),
                posed.pos(Bone::LowerLegL),
                0.066 * b,
            ),
            Collider::capsule(
                posed.pos(Bone::UpperLegR),
                posed.pos(Bone::LowerLegR),
                0.066 * b,
            ),
            // 5, 6: shins
            Collider::capsule(
                posed.pos(Bone::LowerLegL),
                posed.pos(Bone::FootL),
                0.048 * b,
            ),
            Collider::capsule(
                posed.pos(Bone::LowerLegR),
                posed.pos(Bone::FootR),
                0.048 * b,
            ),
            // 7: shoulders, across the upper back
            Collider::capsule(
                posed.pos(Bone::UpperArmL),
                posed.pos(Bone::UpperArmR),
                0.06 * b,
            ),
            // 8: the face, so a fringe rests on it
            Collider::sphere(
                posed.attach(Bone::Head, Vec3::new(0.0, 0.088, 0.024)),
                0.068,
            ),
        ];
        BodyState {
            anchors: vec![anchor(Bone::Head), chest, anchor(Bone::Hips), neck],
            colliders,
            wind,
            billow: 0.0,
            floor: None,
        }
    }
}
