//! Characters as 3D toon models (the way anime 3DCG departments build them): a body, a head and
//! hands modelled from rounded primitives in the rest pose, meshed once and skinned to the
//! skeleton every frame; hair locks, ties, scarves and robes come from the simulation. Faces,
//! seams and buttons are painted on the surface per pixel (like texture decals), the line art is
//! drawn on top by [`crate::ink`].

use std::borrow::Cow;
use std::f32::consts::{PI, TAU};

use glam::{Quat, Vec2, Vec3};

use crate::mesh::{EXTRA_SLOTS, Geo, Mesh, Skinning, spline, surface_nets};
use crate::raster::{CurveRef, Fragment, GroupSource, LineSource, Surface};
use crate::rig::{Bone, Posed, Side, Skeleton};
use crate::sdf::{Model, Op, Prim, Shape};
use crate::spring::Snapshot;
use crate::toon::{Design, Glasses, HAIR_SEGMENTS, HandShape, Look, Material, Mouth, RigLayout};

/// Material groups of the models.
pub mod group {
    pub const SKIN: u8 = 0;
    pub const SUIT: u8 = 1;
    pub const SHIRT: u8 = 2;
    pub const TIE: u8 = 3;
    pub const HAIR: u8 = 4;
    pub const BEARD: u8 = 5;
    pub const SHOE: u8 = 6;
    pub const SCARF: u8 = 7;
    pub const ROBE: u8 = 8;
    pub const LINING: u8 = 9;
    pub const LENS: u8 = 10;
    /// Hair locks alternate between two groups so a line is drawn between neighbours.
    pub const HAIR_A: u8 = 11;
    pub const HAIR_B: u8 = 12;
    /// The robe's hood behind the neck.
    pub const HOOD: u8 = 13;
    /// Horns: ridged, the body's colour.
    pub const HORN: u8 = 14;
}

/// Parts of a figure, the low byte of a surface tag (the actor index is above it).
pub mod part {
    pub const BODY: u32 = 0;
    pub const HEAD: u32 = 1;
    pub const HANDS: u32 = 2;
    pub const LOCKS: u32 = 3;
    pub const TIE: u32 = 4;
    pub const SCARF: u32 = 5;
    pub const ROBE: u32 = 6;
    pub const GLASSES: u32 = 7;
}

pub fn material(g: u8) -> Material {
    match g {
        group::SKIN => Material::Skin,
        group::SUIT | group::SHOE => Material::Suit,
        group::SHIRT => Material::Shirt,
        group::TIE | group::HORN => Material::Tie,
        group::HAIR | group::HAIR_A | group::HAIR_B => Material::Hair,
        group::BEARD => Material::Beard,
        group::SCARF => Material::Scarf,
        group::ROBE | group::HOOD => Material::Robe,
        group::LINING => Material::RobeLining,
        _ => Material::Ink,
    }
}

const HAND_SHAPES: [HandShape; 5] = [
    HandShape::Relaxed,
    HandShape::Fist,
    HandShape::Open,
    HandShape::Point,
    HandShape::Peace,
];

fn shape_index(s: HandShape) -> usize {
    HAND_SHAPES.iter().position(|h| *h == s).unwrap_or(0)
}

/// A character's meshes.
pub struct Figure {
    pub design: Design,
    pub rest: Posed,
    pub body: Mesh,
    pub head: Mesh,
    /// `[side][shape]`, left first.
    pub hands: [Vec<Mesh>; 2],
    /// Head size relative to the default proportions.
    pub hs: f32,
    /// Size of the hands relative to the modelled hand.
    hand_k: f32,
    /// Base of the skull in the rest pose.
    head_rest: Vec3,
    /// Rest chains of the hair locks (head frame).
    locks: Vec<Vec<Vec3>>,
}

impl Figure {
    /// The mesh of a hand in the rest pose (it moves rigidly with the hand bone): for props
    /// that collide with what is drawn.
    pub fn hand(&self, side: Side, shape: HandShape) -> &Mesh {
        &self.hands[usize::from(side == Side::Right)][shape_index(shape)]
    }

    /// The hollow inside a closed fist, from the wrist in the hand's rest frame (fingers along
    /// ±X, palm towards -Y): a bar or a sword hilt held in the fist runs through it along Z.
    pub fn grip(&self, side: Side) -> Vec3 {
        Vec3::new(0.076 * side.sign(), -0.027, 0.0) * self.hand_k
    }

    /// The middle of the palm's surface, from the wrist in the hand's rest frame (the palm
    /// faces -Y): where something held flat on the hand rests.
    pub fn palm(&self, side: Side) -> Vec3 {
        Vec3::new(0.058 * side.sign(), -0.016, 0.0) * self.hand_k
    }
}

/// Mesh resolution: 1 is the default, smaller is finer.
#[derive(Debug, Clone, Copy)]
pub struct Detail(pub f32);

/// Hands are drawn bigger than life in anime: they carry the acting, and a fist at arm's
/// length still reads.
const ANIME_HANDS: f32 = 1.2;

impl Figure {
    pub fn new(design: &Design, skeleton: &Skeleton, detail: Detail) -> Figure {
        let rest = skeleton.solve(&skeleton.rest_pose());
        let hs = crate::toon::head_scale(&skeleton.proportions);
        let k = detail.0;
        let head_rest = rest.pos(Bone::Head);

        let body_model = body_model(design, &rest);
        let (lo, hi) = bounds(&rest, 0.32 * design.build.max(1.0));
        let neck_cut = head_rest.y + 0.05 * hs;
        let body = surface_nets(
            &body_model,
            lo,
            hi,
            0.0065 * k * design.build.max(1.0).sqrt(),
            0.022,
            &|p| p.y < neck_cut,
        );

        let head_model = head_model(design, head_rest, hs);
        let reach = 0.2 * hs + design.hair.volume * 0.02;
        let head = surface_nets(
            &head_model,
            head_rest + Vec3::new(-reach, -0.06 * hs, -reach),
            head_rest + Vec3::new(reach, 0.36 * hs, reach),
            0.0027 * k * hs,
            0.02,
            &|_| true,
        );

        let locks: Vec<Vec<Vec3>> = design
            .hair
            .clumps
            .iter()
            .map(|c| {
                c.rest_points(HAIR_SEGMENTS)
                    .into_iter()
                    .map(|p| p * hs)
                    .collect()
            })
            .collect();
        let mut head = head;
        skin_locks(&mut head, &locks, &design.hair.clumps, head_rest);

        let hand_k = skeleton.proportions.hand / 0.18 * design.build.sqrt().min(1.3) * ANIME_HANDS;
        let hands = [Side::Left, Side::Right].map(|side| {
            let wrist = rest.pos(Bone::arm(side)[3]);
            let len = skeleton.proportions.hand;
            HAND_SHAPES
                .iter()
                .map(|shape| {
                    let m = hand_model(
                        side,
                        *shape,
                        wrist,
                        hand_k,
                        if design.slime { 5 } else { 4 },
                    );
                    let s = len * 1.25 * ANIME_HANDS;
                    surface_nets(
                        &m,
                        wrist - Vec3::splat(s),
                        wrist + Vec3::splat(s),
                        0.0021 * k * (len / 0.18),
                        0.02,
                        &|_| true,
                    )
                })
                .collect()
        });

        Figure {
            design: design.clone(),
            rest,
            body,
            head,
            hands,
            hand_k,
            hs,
            head_rest,
            locks,
        }
    }

    /// The posed surfaces of one frame. `tag` is the actor index shifted left by 8.
    pub fn surfaces<'a>(
        &'a self,
        posed: &Posed,
        sim: &Snapshot,
        layout: &RigLayout,
        look: &Look,
        tag: u32,
    ) -> Vec<Surface<'a>> {
        let mut skin = Skinning::new(&self.rest, posed);
        let head_rot = posed.rot(Bone::Head);
        let head_mat = glam::Mat3::from_quat(head_rot);
        for (c, (rest, chain)) in self.locks.iter().zip(layout.hair.clone()).enumerate() {
            let sim_pts = sim.chains.get(chain);
            let cur = |k: usize| {
                sim_pts
                    .and_then(|p| p.get(k).copied())
                    .unwrap_or_else(|| posed.attach(Bone::Head, rest[k]))
            };
            // The lock follows its chain by forward kinematics with the swing of every link
            // limited (softly), so fast moves bend the lock without tearing the mesh: anime hair
            // keeps its shape and volume however hard the head turns.
            let mut p = posed.attach(Bone::Head, rest[0]);
            let mut prev = Quat::IDENTITY;
            for k in 0..HAIR_SEGMENTS {
                let rest_dir = head_rot * (rest[k + 1] - rest[k]);
                let want = Quat::from_rotation_arc(
                    rest_dir.normalize_or(Vec3::Y),
                    (cur(k + 1) - cur(k)).normalize_or(rest_dir.normalize_or(Vec3::Y)),
                );
                let swing = soft_limit(soft_limit(want, 1.0) * prev.inverse(), 0.55) * prev;
                prev = swing;
                skin.set_extra(
                    c * HAIR_SEGMENTS + k,
                    self.head_rest + rest[k],
                    p,
                    glam::Mat3::from_quat(swing) * head_mat,
                );
                p += swing * rest_dir;
            }
        }
        let skin = skin;
        let mut out = Vec::with_capacity(8);
        let hair = |g: u8| matches!(g, group::HAIR | group::HAIR_A | group::HAIR_B);
        let borders = |m: &'a Mesh| -> Vec<CurveRef<'a>> {
            m.borders
                .iter()
                .map(|b| {
                    // strands inside the hair are drawn finer than outlines
                    let strand = hair(b.groups.0) && hair(b.groups.1);
                    CurveRef {
                        pts: Cow::Borrowed(b.pts.as_slice()),
                        weight: if strand { 0.6 } else { 1.0 },
                        strand,
                    }
                })
                .collect()
        };

        let (mut pos, mut nrm) = (Vec::new(), Vec::new());
        skin.apply(&self.body, &mut pos, &mut nrm);
        out.push(Surface {
            pos,
            nrm,
            tris: Cow::Borrowed(&self.body.tris),
            rest: Some(&self.body.rest),
            groups: GroupSource::Near(&self.body.near),
            back: None,
            tag: tag | part::BODY,
            lines: LineSource {
                contours: true,
                curves: borders(&self.body),
                weight: 1.0,
                fine_groups: &[],
            },
        });

        let (mut pos, mut nrm) = (Vec::new(), Vec::new());
        skin.apply(&self.head, &mut pos, &mut nrm);
        out.push(Surface {
            pos,
            nrm,
            tris: Cow::Borrowed(&self.head.tris),
            rest: Some(&self.head.rest),
            groups: GroupSource::Near(&self.head.near),
            back: None,
            tag: tag | part::HEAD,
            lines: LineSource {
                contours: true,
                curves: borders(&self.head),
                weight: 0.85,
                fine_groups: &[group::HAIR, group::HAIR_A, group::HAIR_B],
            },
        });

        for (si, side) in [Side::Left, Side::Right].into_iter().enumerate() {
            let shape = if side == Side::Left {
                look.hand_l
            } else {
                look.hand_r
            };
            let mesh = &self.hands[si][shape_index(shape)];
            let (mut pos, mut nrm) = (Vec::new(), Vec::new());
            let bone = Bone::arm(side)[3];
            skin.apply_rigid(mesh, bone.index(), &mut pos, &mut nrm);
            if (look.fists - 1.0).abs() > 1e-3 {
                // bigger (or smaller) about the wrist
                let wrist = posed.pos(bone);
                for p in &mut pos {
                    *p = wrist + (*p - wrist) * look.fists;
                }
            }
            out.push(Surface {
                pos,
                nrm,
                tris: Cow::Borrowed(&mesh.tris),
                rest: None,
                groups: GroupSource::Fixed(group::SKIN),
                back: None,
                tag: tag | part::HANDS,
                lines: LineSource {
                    contours: true,
                    curves: Vec::new(),
                    weight: 0.8,
                    fine_groups: &[],
                },
            });
        }

        let chest_fwd = posed.rot(Bone::Chest) * Vec3::Z;
        if let Some(tie) = layout.tie.and_then(|i| sim.chains.get(i)) {
            let mut pts = spline(tie, 3);
            let n = pts.len();
            let dir = (pts[n - 1] - pts[n - 2]).normalize_or(Vec3::NEG_Y);
            pts.push(pts[n - 1] + dir * 0.035);
            let m = pts.len();
            let widths: Vec<f32> = (0..m)
                .map(|i| {
                    let t = i as f32 / (m - 1) as f32;
                    if i == m - 1 {
                        0.003
                    } else {
                        0.02 + 0.016 * smooth(0.05, 0.6, t)
                    }
                })
                .collect();
            let mut geo = Geo::default();
            geo.ribbon(&pts, &widths, chest_fwd);
            out.push(dynamic(
                geo,
                group::TIE,
                Some(group::TIE),
                tag | part::TIE,
                0.8,
            ));
        }

        if let Some((a, b)) = layout.scarf {
            let mut geo = Geo::default();
            for ci in [a, b] {
                if let Some(chain) = sim.chains.get(ci) {
                    let pts = spline(chain, 3);
                    let widths = vec![0.075; pts.len()];
                    geo.ribbon(&pts, &widths, chest_fwd);
                }
            }
            out.push(dynamic(
                geo,
                group::SCARF,
                Some(group::SCARF),
                tag | part::SCARF,
                0.9,
            ));
        }

        if let Some(robe) = layout.robe
            && let Some(points) = sim.cloths.get(robe)
        {
            let mut geo = Geo::default();
            geo.sheet(points, layout.robe_rows, layout.robe_cols, 4, false);
            out.push(dynamic(
                geo,
                group::ROBE,
                Some(group::LINING),
                tag | part::ROBE,
                1.0,
            ));
        }

        if let Some(kind) = self.design.glasses {
            out.push(self.glasses(&skin, kind, tag | part::GLASSES));
        }
        out
    }

    fn glasses<'a>(&self, skin: &Skinning, kind: Glasses, tag: u32) -> Surface<'a> {
        let hs = self.hs;
        let head = |p: Vec3| skin.bone(Bone::Head.index(), self.head_rest + p * hs);
        let mut pos = Vec::new();
        let mut curves = Vec::new();
        let mut line = |pts: Vec<Vec3>, closed: bool| {
            let start = pos.len() as u32;
            pos.extend(pts);
            let end = pos.len() as u32;
            let mut ids: Vec<crate::mesh::EdgePoint> = (start..end)
                .map(|v| crate::mesh::EdgePoint { a: v, b: v, t: 0.0 })
                .collect();
            if closed {
                ids.push(ids[0]);
            }
            curves.push(CurveRef::new(Cow::Owned(ids)));
        };
        for s in [1.0_f32, -1.0] {
            let c = Vec3::new(0.035 * s, 0.098, 0.102);
            let ring: Vec<Vec3> = (0..32)
                .map(|i| {
                    let a = i as f32 / 32.0 * std::f32::consts::TAU;
                    let (x, y) = match kind {
                        Glasses::Round => (a.cos() * 0.023, a.sin() * 0.021),
                        Glasses::Rect => {
                            let (cx, cy) = (a.cos(), a.sin());
                            let k = (cx.abs().powi(4) + (cy * 1.4).abs().powi(4)).powf(0.25);
                            (cx / k * 0.026, cy / k * 0.026 / 1.4)
                        }
                    };
                    head(c + Vec3::new(x, y, -(x * s + 0.01).max(0.0) * 0.35))
                })
                .collect();
            line(ring, true);
            line(
                vec![
                    head(c + Vec3::new(0.025 * s, 0.004, -0.01)),
                    head(Vec3::new(0.075 * s, 0.104, 0.05)),
                    head(Vec3::new(0.091 * s, 0.104, -0.01)),
                ],
                false,
            );
        }
        line(
            vec![
                head(Vec3::new(0.012, 0.102, 0.103)),
                head(Vec3::new(0.0, 0.104, 0.106)),
                head(Vec3::new(-0.012, 0.102, 0.103)),
            ],
            false,
        );
        let n = pos.len();
        Surface {
            pos,
            nrm: vec![Vec3::ZERO; n],
            tris: Cow::Owned(Vec::new()),
            rest: None,
            groups: GroupSource::Fixed(group::LENS),
            back: None,
            tag,
            lines: LineSource {
                contours: false,
                curves,
                weight: 1.0,
                fine_groups: &[],
            },
        }
    }

    /// Head-local coordinates of a rest position, in default-size metres.
    pub fn head_local(&self, rest: Vec3) -> Vec3 {
        (rest - self.head_rest) / self.hs
    }
}

/// The rotation with its angle squashed smoothly below `max` radians.
fn soft_limit(q: Quat, max: f32) -> Quat {
    let (axis, angle) = q.to_axis_angle();
    let angle = if angle > std::f32::consts::PI {
        angle - std::f32::consts::TAU
    } else {
        angle
    };
    Quat::from_axis_angle(axis, max * (angle / max).tanh())
}

fn dynamic(geo: Geo, front: u8, back: Option<u8>, tag: u32, weight: f32) -> Surface<'static> {
    let curves = geo
        .open_edges()
        .into_iter()
        .map(|c| CurveRef::new(Cow::Owned(c)))
        .collect();
    Surface {
        pos: geo.pos,
        nrm: geo.nrm,
        tris: Cow::Owned(geo.tris),
        rest: None,
        groups: GroupSource::Fixed(front),
        back,
        tag,
        lines: LineSource {
            contours: true,
            curves,
            weight,
            fine_groups: &[],
        },
    }
}

fn smooth(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn bounds(rest: &Posed, pad: f32) -> (Vec3, Vec3) {
    let mut lo = Vec3::splat(f32::INFINITY);
    let mut hi = Vec3::splat(f32::NEG_INFINITY);
    for p in rest.p {
        lo = lo.min(p);
        hi = hi.max(p);
    }
    (lo - Vec3::splat(pad), hi + Vec3::splat(pad))
}

/// A rotation whose local Y is `axis` and local Z leans towards `toward`.
fn frame(axis: Vec3, toward: Vec3) -> Quat {
    let y = axis.normalize_or(Vec3::Y);
    let z = (toward - y * toward.dot(y)).normalize_or(y.any_orthonormal_vector());
    let x = y.cross(z);
    Quat::from_mat3(&glam::Mat3::from_cols(x, y, z))
}

/// A limb segment from `a` to `b`, its cross section flattened `flat` times across `toward`.
#[allow(clippy::too_many_arguments)]
fn limb(a: Vec3, b: Vec3, ra: f32, rb: f32, flat: f32, toward: Vec3, g: u8, bone: Bone) -> Prim {
    let axis = b - a;
    let len = axis.length();
    Prim::new(
        Shape::Cone {
            a: Vec3::ZERO,
            b: Vec3::new(0.0, len, 0.0),
            ra,
            rb,
        },
        g,
        bone,
    )
    .at(a)
    .rot(frame(axis, toward))
    .stretch(Vec3::new(1.0, 1.0, flat))
}

fn cone(a: Vec3, b: Vec3, ra: f32, rb: f32, g: u8, bone: Bone) -> Prim {
    Prim::new(Shape::Cone { a, b, ra, rb }, g, bone)
}

fn ellipsoid(at: Vec3, r: Vec3, g: u8, bone: Bone) -> Prim {
    Prim::new(Shape::Ellipsoid(r), g, bone).at(at)
}

/// Half space `n · (p - at) > 0` kept by a clip.
fn keep_side(at: Vec3, n: Vec3, g: u8, bone: Bone) -> Prim {
    Prim::new(Shape::Below, g, bone)
        .at(at)
        .rot(Quat::from_rotation_arc(Vec3::Y, -n.normalize()))
        .op(Op::Clip)
}

fn body_model(d: &Design, j: &Posed) -> Model {
    use group::*;
    if d.slime {
        return slime_body(d, j);
    }
    let b = d.build;
    let mut m = Model::default();
    let hips = j.pos(Bone::Hips);
    let spine = j.pos(Bone::Spine);
    let chest = j.pos(Bone::Chest);
    let neck = j.pos(Bone::Neck);
    let head = j.pos(Bone::Head);
    let hs = crate::toon::head_scale(&j.proportions);
    let fwd = Vec3::Z;
    // The uniform: a shirt tucked into slim trousers, the robe's sleeves over the arms (the
    // robe itself is cloth). Monsters are all body.
    let top = if d.muscles { SUIT } else { SHIRT };
    let sleeve = if d.muscles { SUIT } else { ROBE };
    let belt = hips.y + 0.03;

    // Torso: a slim ribcage and waist; a giant's is a V of muscle.
    if d.muscles {
        let ua = j.pos(Bone::UpperArmL);
        m.push(ellipsoid(
            chest + Vec3::new(0.0, 0.07, 0.0),
            Vec3::new(0.23, 0.2, 0.14) * b,
            SUIT,
            Bone::Chest,
        ));
        m.push(
            ellipsoid(
                spine + Vec3::new(0.0, 0.0, 0.01),
                Vec3::new(0.135, 0.14, 0.11) * b,
                SUIT,
                Bone::Spine,
            )
            .blend(0.08),
        );
        for s in [1.0_f32, -1.0] {
            // pecs, lats, traps, deltoids
            m.push(
                ellipsoid(
                    chest + Vec3::new(0.1 * s, 0.11, 0.08) * b,
                    Vec3::new(0.11, 0.08, 0.07) * b,
                    SUIT,
                    Bone::Chest,
                )
                .blend(0.05),
            );
            m.push(
                ellipsoid(
                    chest + Vec3::new(0.16 * s, 0.0, -0.03) * b,
                    Vec3::new(0.08, 0.16, 0.1) * b,
                    SUIT,
                    Bone::Chest,
                )
                .blend(0.06),
            );
            m.push(
                cone(
                    neck + Vec3::new(0.0, 0.02, -0.03),
                    Vec3::new(ua.x * s, ua.y + 0.04, -0.03),
                    0.07 * b,
                    0.06 * b,
                    SUIT,
                    Bone::Chest,
                )
                .blend(0.06),
            );
            m.push(
                ellipsoid(
                    Vec3::new(ua.x * s, ua.y, ua.z) + Vec3::new(0.0, 0.02, 0.0),
                    Vec3::new(0.13, 0.12, 0.12) * b,
                    SUIT,
                    if s > 0.0 {
                        Bone::UpperArmL
                    } else {
                        Bone::UpperArmR
                    },
                )
                .blend(0.06),
            );
        }
    } else {
        m.push(ellipsoid(
            chest + Vec3::new(0.0, 0.05, 0.004),
            Vec3::new(0.122 * b, 0.15, 0.088 * b),
            top,
            Bone::Chest,
        ));
        m.push(
            ellipsoid(
                spine + Vec3::new(0.0, 0.03, 0.002),
                Vec3::new(0.108 * b, 0.13, 0.08 * b),
                top,
                Bone::Spine,
            )
            .blend(0.08),
        );
    }
    let shoulder_y = j.pos(Bone::UpperArmL).y;
    let sx = j.pos(Bone::UpperArmL).x;
    // shoulders sloping down from the neck, under the robe's yoke
    for s in [1.0_f32, -1.0] {
        m.push(
            limb(
                Vec3::new(0.03 * s, shoulder_y + 0.035, -0.012),
                Vec3::new((sx - 0.02) * s, shoulder_y - 0.004, -0.012),
                0.04 * b,
                0.046 * b,
                0.8,
                fwd,
                sleeve,
                Bone::Chest,
            )
            .blend(0.06),
        );
    }
    if d.muscles {
        // a shaggy mane over the shoulders and down the back, in rows of pointed tufts
        for row in 0..4 {
            let n = 9 - row;
            for i in 0..n {
                let u = i as f32 / (n - 1) as f32 * 2.0 - 1.0;
                let a = u * (1.25 - row as f32 * 0.12);
                let r = 0.14 * b - row as f32 * 0.01;
                let base = chest
                    + Vec3::new(
                        a.sin() * r * 1.15,
                        0.24 - row as f32 * 0.07,
                        -a.cos().abs() * r * 0.5 - 0.02,
                    );
                let jitter = ((i * 7 + row * 3) % 5) as f32 / 4.0;
                let out = Vec3::new(a.sin() * 0.8, -0.55 - row as f32 * 0.1, -0.5).normalize();
                let len = (0.09 + 0.05 * jitter) * b.sqrt();
                m.push(
                    cone(
                        base,
                        base + out * len,
                        0.035 * b.sqrt(),
                        0.003,
                        group::HAIR,
                        Bone::Chest,
                    )
                    .blend(0.025),
                );
            }
        }
        m.push(ellipsoid(
            hips + Vec3::new(0.0, -0.035, -0.005),
            Vec3::new(0.14 * b, 0.12, 0.1 * b),
            SUIT,
            Bone::Hips,
        ));
    } else {
        // the shirt ends at the belt, the trousers start there
        m.push(keep_side(
            Vec3::new(0.0, belt, 0.0),
            Vec3::Y,
            SHIRT,
            Bone::Hips,
        ));
        m.push(ellipsoid(
            hips + Vec3::new(0.0, -0.035, -0.004),
            Vec3::new(0.118 * b, 0.115, 0.088 * b),
            SUIT,
            Bone::Hips,
        ));
        m.push(keep_side(
            Vec3::new(0.0, belt, 0.0),
            Vec3::NEG_Y,
            SUIT,
            Bone::Hips,
        ));
    }

    // Arms in the robe's sleeves, widening to an open cuff, the shirt cuff inside.
    for side in [Side::Left, Side::Right] {
        let s = side.sign();
        let [_, upper, lower, hand] = Bone::arm(side);
        let (a, e, w) = (j.pos(upper), j.pos(lower), j.pos(hand));
        let dir = (w - e).normalize();
        let mb: f32 = if d.muscles { 1.45 } else { 1.0 };
        // the robe's sleeves are wide all the way down, the arm is lost inside them
        let full = if d.muscles { 1.0 } else { 1.22 };
        m.push(
            ellipsoid(
                a + Vec3::new(-0.01 * s, 0.0, -0.008),
                Vec3::new(0.05, 0.048, 0.046) * b * mb.sqrt() * full,
                sleeve,
                upper,
            )
            .blend(0.045),
        );
        m.push(
            limb(
                a,
                e,
                0.041 * b * mb * full,
                0.037 * b * mb * full * full,
                0.92,
                fwd,
                sleeve,
                upper,
            )
            .blend(0.04),
        );
        if d.muscles {
            m.push(
                limb(
                    e,
                    w - dir * 0.012,
                    0.038 * b * mb,
                    0.034 * b * mb,
                    0.9,
                    fwd,
                    SUIT,
                    lower,
                )
                .blend(0.035),
            );
            // biceps, triceps and a heavy forearm
            m.push(
                ellipsoid(
                    a.lerp(e, 0.55) + Vec3::new(0.0, 0.012, 0.02),
                    Vec3::new(0.085, 0.05, 0.05) * b,
                    SUIT,
                    upper,
                )
                .blend(0.04),
            );
            m.push(
                ellipsoid(
                    a.lerp(e, 0.45) + Vec3::new(0.0, 0.0, -0.025),
                    Vec3::new(0.09, 0.048, 0.045) * b,
                    SUIT,
                    upper,
                )
                .blend(0.04),
            );
            m.push(
                ellipsoid(
                    e.lerp(w, 0.28),
                    Vec3::new(0.075, 0.048, 0.05) * b,
                    SUIT,
                    lower,
                )
                .blend(0.04),
            );
            // shaggy fur on the forearm
            for k in 0..5 {
                let p = e.lerp(w, 0.15 + k as f32 * 0.14) + Vec3::new(0.0, 0.045 * b, -0.01);
                let out = Vec3::new(s * 0.35, 0.6, -0.7).normalize();
                m.push(cone(p, p + out * 0.06, 0.02 * b, 0.002, group::HAIR, lower).blend(0.02));
            }
        } else {
            // the bell sleeve, cut flat at the cuff
            m.push(
                limb(
                    e,
                    w + dir * 0.004,
                    0.046 * b,
                    0.074 * b,
                    0.9,
                    fwd,
                    ROBE,
                    lower,
                )
                .blend(0.03),
            );
            m.push(keep_side(w + dir * 0.004, -dir, ROBE, lower));
            m.push(cone(
                w - dir * 0.03,
                w + dir * 0.006,
                0.029,
                0.028,
                SHIRT,
                lower,
            ));
        }
    }

    // Legs: slim trousers and small shoes with a flat sole.
    for side in [Side::Left, Side::Right] {
        let [thigh, shin, foot, toes] = Bone::leg(side);
        let (h, k, a, t) = (j.pos(thigh), j.pos(shin), j.pos(foot), j.pos(toes));
        let lb = if d.muscles { 1.55 } else { 1.0 };
        m.push(
            limb(
                h + Vec3::new(0.0, 0.05, 0.0),
                k,
                0.062 * b * lb,
                0.043 * b * lb,
                0.95,
                fwd,
                SUIT,
                thigh,
            )
            .blend(0.05),
        );
        m.push(
            limb(
                k,
                a + Vec3::new(0.0, 0.012, 0.0),
                0.043 * b * lb,
                0.038 * b * lb,
                0.95,
                fwd,
                SUIT,
                shin,
            )
            .blend(0.03),
        );
        let ground = t.y;
        let heel = Vec3::new(a.x, ground + 0.032, a.z - 0.018);
        let toe = Vec3::new(t.x, ground + 0.022, t.z + 0.03);
        m.push(limb(heel, toe, 0.036, 0.027, 0.85, Vec3::Y, SHOE, foot));
        m.push(
            ellipsoid(
                a + Vec3::new(0.0, -0.026, 0.01),
                Vec3::new(0.036, 0.036, 0.052),
                SHOE,
                foot,
            )
            .blend(0.025),
        );
        m.push(keep_side(Vec3::new(0.0, ground, 0.0), Vec3::Y, SHOE, foot).blend(0.006));
    }

    // Neck, collar, tie knot, the robe's hood, scarf.
    m.push(cone(
        neck + Vec3::new(0.0, -0.03, -0.005),
        head + Vec3::new(0.0, 0.04, -0.012) * hs,
        0.037 * hs.max(b * 0.8),
        0.034 * hs,
        SKIN,
        Bone::Neck,
    ));
    if !d.muscles {
        m.push(
            limb(
                neck + Vec3::new(0.0, -0.035, 0.0),
                neck + Vec3::new(0.0, 0.03, 0.006),
                0.05 * b,
                0.043,
                1.0,
                fwd,
                SHIRT,
                Bone::Neck,
            )
            .blend(0.004),
        );
        // collar points falling onto the chest
        for s in [1.0_f32, -1.0] {
            m.push(
                limb(
                    neck + Vec3::new(0.01 * s, 0.016, 0.045),
                    neck + Vec3::new(0.038 * s, -0.04, 0.054),
                    0.011,
                    0.005,
                    0.45,
                    fwd,
                    SHIRT,
                    Bone::Neck,
                )
                .blend(0.005),
            );
        }
        // the hood bunched behind the neck
        m.push(ellipsoid(
            neck + Vec3::new(0.0, -0.035, -0.07),
            Vec3::new(0.105, 0.07, 0.05) * b,
            HOOD,
            Bone::Chest,
        ));
        m.push(
            Prim::new(
                Shape::Torus {
                    major: 0.07,
                    minor: 0.024,
                },
                HOOD,
                Bone::Chest,
            )
            .at(neck + Vec3::new(0.0, -0.01, -0.012))
            .rot(Quat::from_rotation_x(0.35))
            .stretch(Vec3::new(1.1, 1.0, 1.0))
            .blend(0.03),
        );
        m.push(
            keep_side(
                neck + Vec3::new(0.0, 0.0, 0.03),
                Vec3::NEG_Z,
                HOOD,
                Bone::Chest,
            )
            .blend(0.02),
        );
    }
    if d.tie {
        m.push(limb(
            neck + Vec3::new(0.0, 0.006, 0.05),
            neck + Vec3::new(0.0, -0.028, 0.057),
            0.012,
            0.009,
            0.6,
            fwd,
            TIE,
            Bone::Neck,
        ));
    }
    if d.scarf {
        m.push(
            Prim::new(
                Shape::Torus {
                    major: 0.052,
                    minor: 0.028,
                },
                SCARF,
                Bone::Neck,
            )
            .at(neck + Vec3::new(0.0, 0.0, 0.008))
            .rot(Quat::from_rotation_x(-0.25)),
        );
        m.push(
            ellipsoid(
                neck + Vec3::new(0.03, -0.035, 0.062),
                Vec3::new(0.032, 0.028, 0.02),
                SCARF,
                Bone::Neck,
            )
            .blend(0.02),
        );
    }
    m
}

fn head_model(d: &Design, h: Vec3, hs: f32) -> Model {
    use group::*;
    if d.slime {
        return slime_head(d, h, hs);
    }
    let mut m = Model::default();
    let at = |x: f32, y: f32, z: f32| h + Vec3::new(x, y, z) * hs;
    let s = hs;
    let hood = d.hood;

    // The face: a tall block cut by two jaw planes and the underside of the jaw, which meet
    // at a pointed chin (the anime V jaw), then the cranium over it.
    let chin = at(0.0, -0.004, 0.068);
    let face_c = at(0.0, 0.078, 0.018);
    let face_r = Vec3::new(0.066, 0.088, 0.076) * s;
    m.push(ellipsoid(face_c, face_r, SKIN, Bone::Head));
    let corners = [at(0.062, 0.06, -0.022), at(-0.062, 0.06, -0.022)];
    // jaw planes: (point, outward normal)
    let mut jaw = Vec::new();
    for (i, corner) in corners.iter().enumerate() {
        let side = if i == 0 { 1.0 } else { -1.0 };
        let v = *corner - chin;
        let w = Vec3::new(side, 0.0, 0.25);
        jaw.push((w - v * (w.dot(v) / v.length_squared())).normalize());
    }
    let under = (corners[0] - chin).cross(corners[1] - chin).normalize();
    jaw.push(if under.y > 0.0 { -under } else { under });
    for (k, n) in jaw.iter().enumerate() {
        m.push(keep_side(chin, -*n, SKIN, Bone::Head).blend(if k < 2 { 0.014 } else { 0.016 } * s));
    }
    m.push(
        ellipsoid(
            at(0.0, 0.128, -0.012),
            Vec3::new(0.087, 0.097, 0.1) * s,
            SKIN,
            Bone::Head,
        )
        .blend(0.03 * s),
    );
    // nose: a small wedge with a pointed tip (anime noses are a point in the profile, not a
    // bulb)
    m.push(
        cone(
            at(0.0, 0.104, 0.084),
            at(0.0, 0.08, 0.095),
            0.003 * s,
            0.0034 * s,
            SKIN,
            Bone::Head,
        )
        .blend(0.008 * s),
    );
    // ears
    for side in [1.0_f32, -1.0] {
        m.push(
            Prim::new(
                Shape::Ellipsoid(Vec3::new(0.009, 0.024, 0.015) * s),
                SKIN,
                Bone::Head,
            )
            .at(at(0.087 * side, 0.088, -0.012))
            .rot(Quat::from_rotation_y(0.3 * side) * Quat::from_rotation_x(-0.12))
            .blend(0.006 * s),
        );
    }

    // A monster: a beast's muzzle, a heavy brow ridge, a wide jaw, pointed ears, a goatee and
    // ram horns curling back around the ears.
    if d.muscles {
        m.push(
            ellipsoid(
                at(0.0, 0.06, 0.085),
                Vec3::new(0.042, 0.04, 0.05) * s,
                SKIN,
                Bone::Head,
            )
            .blend(0.03 * s),
        );
        for side in [1.0_f32, -1.0] {
            let c = at(0.092 * side, 0.15, -0.03);
            let mut prev: Option<(Vec3, f32)> = None;
            let n = 22;
            for i in 0..=n {
                let u = i as f32 / n as f32;
                let th = u * 1.6 * PI;
                let r = (0.075 - 0.04 * u) * s;
                // the curl opens sideways and back, so it shows from the front as well
                let out = Vec3::new(0.75 * side, 0.0, -0.66);
                let p = c
                    + (Vec3::Y * th.cos() + out * th.sin()) * r
                    + Vec3::new(0.0, 0.0, 0.03 * u) * s;
                let w = (0.03 * (1.0 - u * 0.85) + 0.0025) * s;
                if let Some((q, wq)) = prev {
                    m.push(cone(q, p, wq, w, HORN, Bone::Head).blend(0.004 * s));
                }
                prev = Some((p, w));
            }
        }
        m.push(
            limb(
                at(-0.055, 0.118, 0.078),
                at(0.055, 0.118, 0.078),
                0.016 * s,
                0.016 * s,
                0.7,
                Vec3::Y,
                SKIN,
                Bone::Head,
            )
            .blend(0.022 * s),
        );
        for side in [1.0_f32, -1.0] {
            m.push(
                cone(
                    at(0.055 * side, 0.035, 0.035),
                    at(0.012 * side, 0.004, 0.072),
                    0.026 * s,
                    0.02 * s,
                    SKIN,
                    Bone::Head,
                )
                .blend(0.03 * s),
            );
            m.push(
                cone(
                    at(0.086 * side, 0.098, -0.01),
                    at(0.135 * side, 0.142, -0.035),
                    0.017 * s,
                    0.002 * s,
                    SKIN,
                    Bone::Head,
                )
                .blend(0.01 * s),
            );
        }
        for i in 0..7 {
            let u = i as f32 / 6.0 * 2.0 - 1.0;
            let base = at(0.03 * u, 0.0 + 0.012 * u * u, 0.07 - 0.012 * u * u);
            let tip = base + Vec3::new(0.012 * u, -0.06 - 0.015 * (1.0 - u.abs()), 0.01) * s;
            m.push(cone(base, tip, 0.013 * s, 0.002 * s, HAIR, Bone::Head).blend(0.008 * s));
        }
    }

    // Hair volume over the skull, cut at the hairline.
    let c = at(0.0, 0.12, -0.005);
    let v = d.hair.volume;
    m.push(ellipsoid(
        at(0.0, 0.127, -0.012) + Vec3::new(0.0, 0.004, -0.002) * (v - 1.0) * 10.0 * s,
        Vec3::new(0.088 * v + 0.004, 0.098 * v + 0.006, 0.1 * v + 0.004) * s,
        HAIR,
        Bone::Head,
    ));
    if hood {
        // the hood falls behind to the shoulders; the hairline cut below opens it at the face
        m.push(
            cone(
                at(0.0, 0.12, -0.04),
                at(0.0, -0.06, -0.08),
                0.1 * v * s,
                0.12 * v * s,
                HAIR,
                Bone::Head,
            )
            .blend(0.05 * s),
        );
    }
    let n = d.hair.cap_normal;
    m.push(keep_side(c + n * d.hair.cap_offset * s, n, HAIR, Bone::Head).blend(0.012 * s));
    // fringe tufts along the front of the hairline
    if d.hair.fringe > 0.0 && !hood {
        let count = 7;
        for i in 0..count {
            let u = i as f32 / (count - 1) as f32 * 2.0 - 1.0;
            let x = u * 0.062;
            let z = (0.098_f32.powi(2) - x * x).max(0.0).sqrt() - 0.008;
            let y0 = hairline_y(d, x, z);
            let len =
                d.hair.fringe * (1.0 - 0.35 * u.abs()) * (0.85 + 0.3 * ((i * 7 % 5) as f32 / 4.0));
            m.push(
                cone(
                    at(x, y0 + 0.012, z - 0.004),
                    at(x * 1.08 + 0.004 * u, y0 - len, z + 0.006),
                    0.014 * s,
                    0.002 * s,
                    HAIR,
                    Bone::Head,
                )
                .blend(0.008 * s),
            );
        }
    }

    // Locks of hair along the rest chains of the spring bones, flattened onto the head.
    let skull = at(0.0, 0.127, -0.012);
    for (i, clump) in d.hair.clumps.iter().enumerate() {
        let g = if i % 2 == 0 { HAIR_A } else { HAIR_B };
        let pts = spline(
            &clump
                .rest_points(HAIR_SEGMENTS)
                .iter()
                .map(|p| h + *p * s)
                .collect::<Vec<_>>(),
            3,
        );
        let n = pts.len();
        for k in 0..n - 1 {
            let t0 = k as f32 / (n - 1) as f32;
            let t1 = (k + 1) as f32 / (n - 1) as f32;
            let r = |t: f32| {
                clump.width * s * 0.5 * (1.0 - t).powf(0.9) * (0.8 + 0.2 * smooth(0.0, 0.25, t))
            };
            let radial = ((pts[k] + pts[k + 1]) * 0.5 - skull).normalize_or(Vec3::Y);
            m.push(
                limb(
                    pts[k],
                    pts[k + 1],
                    r(t0).max(0.0015),
                    r(t1).max(0.001),
                    0.45,
                    radial,
                    g,
                    Bone::Head,
                )
                .blend(0.004 * s),
            );
        }
    }

    // A short full beard: the face grown a few millimetres below the cheek line, a shaggy edge
    // along the jaw and a moustache.
    if d.beard > 0.0 {
        let k = d.beard;
        let t = 0.0035 * s;
        // the skin, inflated: face, jaw, cranium
        m.push(ellipsoid(
            face_c,
            face_r + Vec3::splat(t),
            BEARD,
            Bone::Head,
        ));
        for (i, n) in jaw.iter().enumerate() {
            m.push(
                keep_side(
                    chin + *n * t * if i < 2 { 1.0 } else { 1.6 },
                    -*n,
                    BEARD,
                    Bone::Head,
                )
                .blend(if i < 2 { 0.014 } else { 0.016 } * s),
            );
        }
        m.push(
            ellipsoid(
                at(0.0, 0.128, -0.012),
                Vec3::new(0.087, 0.097, 0.1) * s + Vec3::splat(t),
                BEARD,
                Bone::Head,
            )
            .blend(0.03 * s),
        );
        // cheek line: from in front of the ears down over the cheeks to under the nose
        let lift = (k - 0.5) * 0.02;
        let p0 = at(0.0, 0.052 + lift * 0.3, 0.1);
        let p1 = at(0.086, 0.064 + lift, 0.0);
        let p2 = at(-0.086, 0.064 + lift, 0.0);
        let normal = (p1 - p0).cross(p2 - p0).normalize();
        let normal = if normal.y > 0.0 { -normal } else { normal };
        m.push(keep_side(p0, normal, BEARD, Bone::Head).blend(0.006 * s));
        // sideburns stop in front of the ears
        m.push(keep_side(at(0.0, 0.0, -0.022), Vec3::Z, BEARD, Bone::Head).blend(0.008 * s));
        // tufts along the jaw give the beard a shaggy edge
        for i in 0..11 {
            let u = i as f32 / 10.0 * 2.0 - 1.0;
            let side = u.signum();
            let a = u.abs();
            // along the jaw from the chin (a = 0) to below the ear (a = 1)
            let base = chin.lerp(corners[usize::from(side <= 0.0)], a * 0.9)
                + Vec3::new(0.0, 0.004, -0.004) * s;
            let out = Vec3::new(0.5 * side * a, -1.0 + 0.35 * a, 0.3 - 0.5 * a).normalize();
            let len = (0.011 + 0.006 * ((i * 7 % 3) as f32 / 2.0)) * s;
            m.push(
                cone(
                    base,
                    base + out * len,
                    0.0075 * s,
                    0.0012 * s,
                    BEARD,
                    Bone::Head,
                )
                .blend(0.005 * s),
            );
        }
        // moustache
        for side in [1.0_f32, -1.0] {
            m.push(
                cone(
                    at(0.002 * side, 0.057, 0.085),
                    at(0.022 * side, 0.05, 0.076),
                    0.0042 * s,
                    0.0025 * s,
                    BEARD,
                    Bone::Head,
                )
                .blend(0.008 * s),
            );
        }
    }
    m
}

/// Binds the vertices of hair locks to the links of their spring chains (slots after the
/// skeleton's bones), fading to the head at the root.
fn skin_locks(
    mesh: &mut Mesh,
    locks: &[Vec<Vec3>],
    clumps: &[crate::toon::Clump],
    head_rest: Vec3,
) {
    let polylines: Vec<Vec<Vec3>> = locks
        .iter()
        .map(|l| spline(&l.iter().map(|p| head_rest + *p).collect::<Vec<_>>(), 3))
        .collect();
    for (v, p) in mesh.rest.iter().enumerate() {
        let g = mesh.near[v][0].0;
        if g != group::HAIR_A && g != group::HAIR_B {
            continue;
        }
        let mut best = (f32::INFINITY, 0, 0.0);
        for (c, line) in polylines.iter().enumerate() {
            let total: f32 = line.windows(2).map(|w| (w[1] - w[0]).length()).sum();
            let mut acc = 0.0;
            for w in line.windows(2) {
                let ab = w[1] - w[0];
                let l = ab.length();
                let t = ((*p - w[0]).dot(ab) / (l * l).max(1e-12)).clamp(0.0, 1.0);
                let d = (*p - w[0] - ab * t).length();
                if d < best.0 {
                    best = (d, c, (acc + t * l) / total.max(1e-9));
                }
                acc += l;
            }
        }
        let (d, c, t) = best;
        let hs = locks_scale(locks, clumps);
        if d > clumps[c].width * hs * 0.9 {
            continue;
        }
        let u = t * HAIR_SEGMENTS as f32;
        let k = (u.floor() as usize).min(HAIR_SEGMENTS - 1);
        let f = (u - k as f32).clamp(0.0, 1.0);
        let root = 1.0 - smooth(0.0, 0.2, t);
        let slot = |k: usize| (EXTRA_SLOTS + c * HAIR_SEGMENTS + k) as u8;
        let mut w = [
            (Bone::Head.index() as u8, root),
            (slot(k), (1.0 - root) * (1.0 - f)),
            (0, 0.0),
            (0, 0.0),
        ];
        if k + 1 < HAIR_SEGMENTS {
            w[2] = (slot(k + 1), (1.0 - root) * f);
        } else {
            w[1].1 = 1.0 - root;
        }
        mesh.skin[v] = w;
    }
}

/// How much the lock chains were scaled from the design's (the head scale).
fn locks_scale(locks: &[Vec<Vec3>], clumps: &[crate::toon::Clump]) -> f32 {
    locks.first().zip(clumps.first()).map_or(1.0, |(l, c)| {
        (l[0].length() / c.root.length().max(1e-6)).max(0.1)
    })
}

/// Height of the hairline at a point of the forehead (head-local).
fn hairline_y(d: &Design, x: f32, z: f32) -> f32 {
    // n · (p - c) = o  →  solve for y
    let n = d.hair.cap_normal;
    let c = Vec3::new(0.0, 0.12, -0.005);
    if n.y.abs() < 1e-3 {
        return 0.17;
    }
    c.y + (d.hair.cap_offset - n.x * (x - c.x) - n.z * (z - c.z)) / n.y
}

/// Finger joint angles for a hand shape: (spread degrees, curl per joint in degrees) for the
/// index, middle, ring and little finger.
fn hand_pose(shape: HandShape) -> [(f32, [f32; 3]); 4] {
    let curled = [88.0, 95.0, 70.0];
    match shape {
        HandShape::Relaxed => [
            (3.0, [14.0, 22.0, 12.0]),
            (0.0, [18.0, 26.0, 14.0]),
            (-3.0, [22.0, 30.0, 16.0]),
            (-6.0, [26.0, 34.0, 18.0]),
        ],
        HandShape::Open => [
            (8.0, [0.0, 4.0, 2.0]),
            (1.0, [0.0, 4.0, 2.0]),
            (-6.0, [0.0, 4.0, 2.0]),
            (-13.0, [2.0, 5.0, 3.0]),
        ],
        HandShape::Fist => [(2.0, curled), (0.0, curled), (-2.0, curled), (-4.0, curled)],
        HandShape::Point => [
            (2.0, [0.0, 3.0, 2.0]),
            (0.0, curled),
            (-2.0, curled),
            (-4.0, curled),
        ],
        HandShape::Peace => [
            (13.0, [0.0, 3.0, 2.0]),
            (-11.0, [0.0, 3.0, 2.0]),
            (-2.0, curled),
            (-4.0, curled),
        ],
    }
}

/// A hand at the wrist `w` of the rest pose: fingers along ±X, palm down, thumb forward.
fn hand_model(side: Side, shape: HandShape, w: Vec3, k: f32, fingers: usize) -> Model {
    use group::SKIN;
    let s = side.sign();
    let bone = Bone::arm(side)[3];
    let mut m = Model::default();
    let local = |x: f32, y: f32, z: f32| w + Vec3::new(x * s, y, z) * k;
    // the palm: a flattened rounded box, wider at the knuckles
    m.push(
        Prim::new(
            Shape::Box {
                half: Vec3::new(0.042, 0.0115, 0.036) * k,
                r: 0.011 * k,
            },
            SKIN,
            bone,
        )
        .at(local(0.05, -0.002, 0.0)),
    );
    m.push(
        limb(
            local(-0.025, 0.0, 0.0),
            local(0.02, -0.001, 0.0),
            0.023 * k,
            0.026 * k,
            0.75,
            Vec3::Y,
            SKIN,
            bone,
        )
        .blend(0.012 * k),
    );
    // thenar pad
    m.push(
        ellipsoid(
            local(0.03, -0.01, 0.024),
            Vec3::new(0.03, 0.014, 0.018) * k,
            SKIN,
            bone,
        )
        .blend(0.012 * k),
    );

    let fingers_n = fingers;
    let fingers = hand_pose(shape);
    let bases = [0.026_f32, 0.009, -0.009, -0.025];
    let lengths = [
        [0.044, 0.026, 0.021],
        [0.048, 0.029, 0.023],
        [0.045, 0.027, 0.021],
        [0.035, 0.021, 0.018],
    ];
    let radii = [0.0088_f32, 0.009, 0.0085, 0.0076];
    let reach = [0.092_f32, 0.095, 0.091, 0.084];
    let count = fingers_n.max(2);
    for i in 0..count {
        // more fingers than a hand should have spread over the same knuckles
        let f = (i as f32 * 3.0 / (count - 1) as f32).round() as usize;
        let base = bases[0] + (bases[3] - bases[0]) * i as f32 / (count - 1) as f32;
        let (spread, curls) = fingers[f];
        let mut p = local(reach[f], -0.002, base);
        // direction in the hand's local frame (left hand), mirrored at the end
        let mut dir = Quat::from_rotation_y(spread.to_radians()) * Vec3::X;
        let r = radii[f] * k;
        for (seg, curl) in curls.iter().enumerate() {
            // curl towards the palm: rotate about the axis across the hand
            let axis = dir.cross(Vec3::NEG_Y).normalize_or(Vec3::Z);
            dir = Quat::from_axis_angle(axis, curl.to_radians()) * dir;
            let d = Vec3::new(dir.x * s, dir.y, dir.z);
            let next = p + d * lengths[f][seg] * k;
            let (ra, rb) = (
                r * (1.0 - seg as f32 * 0.07),
                r * (0.93 - seg as f32 * 0.07),
            );
            m.push(cone(p, next, ra, rb, SKIN, bone).blend(if seg == 0 {
                0.006 * k
            } else {
                0.0015 * k
            }));
            p = next;
        }
    }
    // thumb: joint positions per shape (left hand, metres)
    let thumb_joints: [[f32; 3]; 4] = match shape {
        HandShape::Relaxed => [
            [0.018, -0.008, 0.028],
            [0.044, -0.022, 0.056],
            [0.07, -0.033, 0.068],
            [0.092, -0.04, 0.07],
        ],
        HandShape::Open => [
            [0.018, -0.006, 0.028],
            [0.04, -0.01, 0.063],
            [0.061, -0.012, 0.088],
            [0.08, -0.012, 0.105],
        ],
        HandShape::Fist => [
            [0.018, -0.008, 0.028],
            [0.044, -0.03, 0.05],
            [0.07, -0.056, 0.044],
            [0.09, -0.062, 0.014],
        ],
        HandShape::Point | HandShape::Peace => [
            [0.018, -0.008, 0.028],
            [0.044, -0.03, 0.05],
            [0.068, -0.056, 0.04],
            [0.084, -0.062, 0.004],
        ],
    };
    let thumb_r = [0.0128, 0.0108, 0.0096];
    for seg in 0..3 {
        let [x0, y0, z0] = thumb_joints[seg];
        let [x1, y1, z1] = thumb_joints[seg + 1];
        m.push(
            cone(
                local(x0, y0, z0),
                local(x1, y1, z1),
                thumb_r[seg] * k,
                thumb_r[seg] * 0.92 * k,
                SKIN,
                bone,
            )
            .blend(if seg == 0 { 0.012 * k } else { 0.002 * k }),
        );
    }
    m
}

/// Colours of the materials in a shot, linear 0..1 RGB.
#[derive(Debug, Clone, Copy)]
pub struct Palette {
    pub fill: [Vec3; MATERIALS],
    pub line: Vec3,
    /// How much darker shadows are (0 = flat).
    pub shade: f32,
    /// Where the shadow starts: `n · light` below this is in shadow (-0.25 = only the far side,
    /// 0.2 = sculpted, lots of shadow).
    pub terminator: f32,
    /// Direction towards the light (world).
    pub light: Vec3,
    /// Draw painted details (faces, seams).
    pub details: bool,
}

pub const MATERIALS: usize = 12;

pub fn material_index(m: Material) -> usize {
    match m {
        Material::Suit => 0,
        Material::Robe => 1,
        Material::RobeLining => 2,
        Material::Shirt => 3,
        Material::Tie => 4,
        Material::Skin => 5,
        Material::Hair => 6,
        Material::Beard => 7,
        Material::Scarf => 8,
        Material::Ink => 9,
        Material::Paper => 10,
        Material::Accent => 11,
    }
}

impl Palette {
    pub fn color(&self, m: Material) -> Vec3 {
        self.fill[material_index(m)]
    }
}

/// One character in a shot, for the shader.
pub struct Cast<'a> {
    pub figure: &'a Figure,
    pub look: Look,
    pub palette: &'a Palette,
    /// The direction to the camera in the head's frame (+z = straight into the face): how far
    /// the face is turned decides which painted lines a contour already draws.
    pub view: Vec3,
}

/// Shades a fragment of a cast member: the material's colour, a cel shadow, and painted details.
pub fn shade(cast: &[Cast], f: &Fragment) -> Vec3 {
    let Some(c) = cast.get((f.tag >> 8) as usize) else {
        return Vec3::ZERO;
    };
    let pal = c.palette;
    let mat = match material(f.group) {
        // the rivals wear dark shirts under their hoods
        Material::Shirt if c.figure.design.hood => Material::Suit,
        m => m,
    };
    let mut color = pal.color(mat);
    let part = f.tag & 0xff;
    if c.figure.design.slime
        && matches!(
            f.group,
            group::SUIT | group::SKIN | group::HAIR | group::HAIR_A | group::HAIR_B
        )
    {
        color = goo(f, pal, c.figure.rest.head_top().y);
    }
    if c.figure.design.hood && f.group == group::SKIN {
        // everything under the hood is in its shadow
        color = pal.color(Material::Hair) * 0.45 + pal.color(Material::Accent) * 0.06;
    }
    if pal.details && c.look.face && part == part::HEAD && f.group == group::SKIN {
        let q = c.figure.head_local(f.rest);
        if q.z > 0.0 {
            color = face(
                color,
                q,
                f.px / c.figure.hs,
                c.view,
                &c.look,
                pal,
                &c.figure.design,
            );
        }
        if q.x.abs() > 0.076 && !c.figure.design.hood {
            color = ear(color, q, f.px / c.figure.hs, pal);
        }
    }
    if pal.details && part == part::BODY && f.group == group::SHOE {
        color = shoe(color, f.rest, &c.figure.rest, f.px, pal);
    }
    if pal.details && part == part::HEAD && f.group == group::HORN {
        // ridges across the horn: rings around the curl's centre
        let q = c.figure.head_local(f.rest);
        let a = (q.y - 0.15).atan2(((q.x.abs() - 0.092) * 0.75 - (q.z + 0.03) * 0.66).max(-1.0));
        let ring = ((a / TAU * 13.0).fract() - 0.5).abs() * 2.0;
        let line = 1.0 - smooth(0.12, 0.2, ring);
        color = mix(color, pal.line, line * 0.85);
    }
    if pal.details && part == part::HEAD && f.group == group::BEARD {
        let q = c.figure.head_local(f.rest);
        let px = f.px / c.figure.hs;
        color = beard(color, q, px, pal);
        // the mouth opens through the beard
        if c.look.face && q.z > 0.0 && q.y < 0.062 {
            color = mouth(color, Vec2::new(q.x, q.y), px, c.look.mouth, pal);
        }
    }
    if pal.details && part == part::BODY && f.group == group::SHIRT {
        let belt = c.figure.rest.pos(Bone::Hips).y + 0.03 - c.figure.rest.pos(Bone::Chest).y;
        color = shirt(
            color,
            f.rest - c.figure.rest.pos(Bone::Chest),
            belt,
            f.px,
            pal,
        );
    }
    if pal.details
        && part == part::BODY
        && c.figure.design.muscles
        && !c.figure.design.slime
        && f.group == group::SUIT
    {
        color = muscles(
            color,
            f.rest - c.figure.rest.pos(Bone::Chest),
            f.px,
            pal,
            c.figure.design.build,
        );
    }
    // cel shadow, a clean edge; faces stay clean like on anime model sheets
    let face_skin =
        part == part::HEAD && f.group == group::SKIN && c.figure.head_local(f.rest).z > -0.03;
    if pal.shade > 0.0 && !face_skin {
        let lit = f.normal.dot(pal.light);
        let k = smooth(pal.terminator - 0.04, pal.terminator + 0.04, lit);
        color *= 1.0 - pal.shade * (1.0 - k);
    }
    color
}

/// Coverage of a region with signed distance `d` (negative inside) at pixel size `px`.
fn cover(d: f32, px: f32) -> f32 {
    (0.5 - d / px.max(1e-6)).clamp(0.0, 1.0)
}

fn mix(a: Vec3, b: Vec3, t: f32) -> Vec3 {
    a + (b - a) * t
}

/// Distance to a polyline and the position along it (0..1).
fn poly_dist(p: Vec2, pts: &[Vec2]) -> (f32, f32) {
    let mut best = (f32::INFINITY, 0.0);
    let total: f32 = pts.windows(2).map(|w| (w[1] - w[0]).length()).sum();
    let mut acc = 0.0;
    for w in pts.windows(2) {
        let ab = w[1] - w[0];
        let l = ab.length();
        let t = ((p - w[0]).dot(ab) / (l * l).max(1e-12)).clamp(0.0, 1.0);
        let d = (p - w[0] - ab * t).length();
        if d < best.0 {
            best = (d, (acc + t * l) / total.max(1e-9));
        }
        acc += l;
    }
    best
}

/// A stroke of varying width along a polyline: coverage.
fn stroke(p: Vec2, pts: &[Vec2], w0: f32, w1: f32, px: f32) -> f32 {
    let (d, t) = poly_dist(p, pts);
    let w = (w0 + (w1 - w0) * t).max(px * 0.9);
    cover(d - w * 0.5, px)
}

/// The face, painted in head-local coordinates (metres of the default head).
/// `view` is the direction to the camera in the head's frame: lines that a contour draws once
/// the face turns (the nose) fade out.
fn face(
    skin: Vec3,
    q: Vec3,
    px: f32,
    view: Vec3,
    look: &Look,
    pal: &Palette,
    design: &Design,
) -> Vec3 {
    let ink = pal.line;
    let paper = pal.color(Material::Paper);
    let mut c = skin;
    let p = Vec2::new(q.x, q.y);
    let hood = design.hood;
    if design.slime {
        return slop_face(c, p, px, look, pal);
    }
    if design.muscles {
        return monster_face(c, p, px, look, pal);
    }
    if look.gloom > 0.0 && !hood {
        c = gloom(c, p, px, look.gloom, ink);
    }

    for s in [1.0_f32, -1.0] {
        // eye frame: x towards the outer corner
        let e = Vec2::new((p.x - 0.034 * s) * s, p.y - 0.097);
        if e.x.abs() > 0.045 || e.y.abs() > 0.05 {
            continue;
        }
        // Most of this box is cheek and forehead: only pixels near the eye (or its tears) and
        // near the brow run their part (a close-up shades millions of these).
        let near_eye = (-0.024..0.03).contains(&e.x)
            && e.y < 0.022
            && e.y > if look.tears > 0.3 { -0.05 } else { -0.016 };
        let near_brow = (-0.026..0.034).contains(&e.x) && e.y > -0.002;
        if !near_eye && !near_brow {
            continue;
        }
        let open = look.eye_open(s);
        // the gaze in the eye frame (x towards the outer corner)
        let gaze = Vec2::new(look.gaze.x * s, look.gaze.y);
        if hood {
            if !near_eye {
                continue;
            }
            // glowing almond eyes slanting up to the outer corners, a slit pupil
            let q = Vec2::new(e.x - 0.002, e.y + 0.001 - 0.32 * e.x);
            let h = 0.0075 * open.max(0.08);
            let d = ((q * Vec2::new(1.0 / 0.019, 1.0 / h)).length() - 1.0) * h;
            c = mix(c, pal.color(Material::Accent), cover(d, px));
            let g = Vec2::new(gaze.x * 0.009, gaze.y * 0.003);
            let slit = (((q - g) * Vec2::new(1.0 / 0.0022, 1.0 / 0.0065)).length() - 1.0) * 0.0022;
            c = mix(c, ink, cover(slit.max(d), px));
            continue;
        }
        // MASHLE eyes: a rounded outline with lots of white, a small solid iris, a heavy upper
        // lash that flicks out at the outer corner. Lids and brows bend the outline.
        let half = 0.0165;
        let x0 = 0.001;
        let arch = |x: f32| (1.0 - ((x - x0) / half).powi(2)).max(0.0);
        // 0 at the inner corner, 1 at the outer one
        let along = |x: f32| ((x - x0) / (2.0 * half) + 0.5).clamp(0.0, 1.0);
        let anger = look.brow_angle.max(0.0);
        let worry = (-look.brow_angle).max(0.0);
        let upper = |x: f32| {
            0.0005
                + 0.0108 * open * arch(x).powf(0.6) * (1.0 - 0.5 * look.lids - 0.15 * look.squint)
                + 0.014 * (x - x0)
                - open * (anger * 0.0055 * (1.0 - along(x)) + worry * 0.0035 * along(x))
        };
        let lower = |x: f32| {
            -0.0072 * open * arch(x).powf(0.55) * (1.0 - 0.8 * look.squint) - 0.0005
                + 0.0026 * look.squint * arch(x)
        };
        if !near_eye {
            // only the brow below
        } else if open < 0.15 {
            // closed: a curve, up when laughing or winking
            let dip = if look.happy || look.wink != 0.0 {
                0.006
            } else {
                -0.004
            };
            let pts: [Vec2; 7] = std::array::from_fn(|i| {
                let x = -0.017 + i as f32 * 0.036 / 6.0;
                Vec2::new(x, dip * (1.0 - ((x - 0.002) / 0.019).powi(2)))
            });
            c = mix(c, ink, stroke(e, &pts, 0.0016, 0.0026, px));
        } else {
            let inside_x = (e.x - x0).abs() - half;
            let d_white = inside_x.max(e.y - upper(e.x)).max(lower(e.x) - e.y);
            let white = mix(paper, pal.color(Material::Accent), look.bloodshot);
            c = mix(c, white, cover(d_white, px));
            // a small solid iris (a dot when shocked) that follows the gaze, cut by the lids
            let r = 0.0056 * look.pupils.clamp(0.3, 1.6);
            let ic = Vec2::new(
                x0 - 0.0005 + gaze.x.clamp(-1.2, 1.2) * 0.0078,
                0.0012 * open + gaze.y.clamp(-1.2, 1.2) * 0.0042,
            );
            let d_iris = ((e - ic) * Vec2::new(0.95, 1.0)).length() - r;
            let iris_col = if look.bloodshot > 0.5 {
                paper
            } else if look.accent_eyes {
                pal.color(Material::Accent)
            } else {
                ink
            };
            c = mix(c, iris_col, cover(d_iris.max(d_white), px));
            if look.bloodshot > 0.5 {
                // a ringed iris with a pin of a pupil
                c = mix(c, ink, cover((d_iris.abs() - 0.00045).max(d_white), px));
                let pin = (e - ic).length() - r * 0.28;
                c = mix(c, ink, cover(pin.max(d_white), px));
            }
            let hl = ic + Vec2::new(-r * 0.3, r * 0.35);
            if look.sparkle > 0.0 {
                // four-pointed stars of light
                let k = r * (0.55 + 0.75 * look.sparkle);
                let sq = (e - hl).abs() / k;
                let star = (sq.x.sqrt() + sq.y.sqrt() - 1.0) * k * 0.35;
                c = mix(c, paper, cover(star.max(d_white), px));
                let small = (e - ic - Vec2::new(r * 0.5, -r * 0.45)).length() - r * 0.2;
                c = mix(c, paper, cover(small.max(d_white), px));
            } else {
                // a pinprick of light in close-ups
                c = mix(
                    c,
                    paper,
                    cover(((e - hl).length() - r * 0.22).max(d_white), px)
                        * smooth(0.00035, 0.0007, -px + 0.001),
                );
            }
            if look.tears > 0.0 {
                // welling up: a wobbling shine along the lower lid
                let band = e.y - lower(e.x) - 0.0022 - 0.0005 * (e.x * 900.0).sin();
                let shine = band.abs() - 0.0009 * look.tears;
                c = mix(c, paper, cover(shine.max(d_white), px));
            }
            // the outline: thin all round
            c = mix(c, ink, cover(d_white.abs() - 0.00045, px));
            // upper lash, heavier towards the outer corner, with a flick
            let lash: [Vec2; 10] = std::array::from_fn(|i| {
                if i == 9 {
                    Vec2::new(x0 + half + 0.005, upper(x0 + half) + 0.0018)
                } else {
                    let x = x0 - half + i as f32 * (2.0 * half) / 8.0;
                    Vec2::new(x, upper(x) + 0.0004)
                }
            });
            let (d, t) = poly_dist(e, &lash);
            let w = 0.0008 + 0.0024 * smooth(0.15, 0.7, t) * (1.0 - smooth(0.88, 1.0, t) * 0.75);
            c = mix(c, ink, cover(d - w.max(px * 0.6), px));
            // a crease line over the lid (gone under a heavy lid)
            if look.lids < 0.6 {
                let crease: [Vec2; 5] = std::array::from_fn(|i| {
                    let x = x0 - half * 0.4 + i as f32 * half * 1.2 / 4.0;
                    Vec2::new(x, upper(x) + 0.004 + 0.002 * look.lids)
                });
                c = mix(c, ink, stroke(e, &crease, 0.0002, 0.0007, px));
            }
            // the lower lid pushed up: a short line under the eye
            if look.squint > 0.3 {
                let lid: [Vec2; 4] = std::array::from_fn(|i| {
                    let x = x0 - half * 0.2 + i as f32 * half * 0.9 / 3.0;
                    Vec2::new(x, lower(x) - 0.0022)
                });
                c = mix(
                    c,
                    ink,
                    stroke(e, &lid, 0.0002, 0.0007, px) * smooth(0.3, 0.6, look.squint),
                );
            }
        }
        if look.tears > 0.3 && near_eye {
            // a stream down the cheek from the outer corner
            let fall = 0.012 + 0.022 * look.tears;
            let stream = [
                Vec2::new(0.008, -0.006),
                Vec2::new(0.0095, -0.006 - fall * 0.5),
                Vec2::new(0.0085, -0.006 - fall),
            ];
            c = mix(c, ink, stroke(e, &stream, 0.0042, 0.0032, px));
            c = mix(c, paper, stroke(e, &stream, 0.0028, 0.0018, px));
        }
        if !near_brow {
            continue;
        }
        // brow: raised, tilted against the other one, angled (anger pulls the inner end down)
        let by = 0.029 + look.brows * 0.006 + look.brow_tilt * s * 0.005;
        let tilt = -look.brows.min(0.0) * 0.006;
        let a = look.brow_angle;
        let brow = [
            (-0.018, -0.004 - tilt),
            (-0.004, 0.001),
            (0.012, 0.0015),
            (0.026, -0.003),
        ]
        .map(|(x, y)| {
            let k = (x + 0.018) / 0.044;
            Vec2::new(x, by + y - a * 0.011 * (1.0 - k) + a * 0.003 * k)
        });
        let (d, t) = poly_dist(e, &brow);
        let w = (0.0042 * (1.0 - t * 0.6)).max(px * 1.8);
        let brow_col = pal.color(Material::Hair);
        let light = brow_col.dot(Vec3::new(0.3, 0.59, 0.11)) > 0.7;
        let brow_col = if brow_col == pal.color(Material::Skin) || light {
            ink
        } else {
            brow_col
        };
        c = mix(c, brow_col, cover(d - w * 0.5, px));
    }
    if hood {
        // a crescent grin full of teeth from cheek to cheek
        let open = match look.mouth {
            Mouth::Grin | Mouth::Teeth(_) | Mouth::Smirk(_) => 0.35,
            Mouth::Roar(o) | Mouth::Open(o) => 0.3 + o * 0.7,
            Mouth::Talk { open, .. } => 0.3 + open * 0.7,
            _ => return c,
        };
        let wide = 0.056;
        let x = p.x / wide;
        if x.abs() < 1.0 {
            let arc = 1.0 - x * x;
            // corners pulled up to the cheeks
            let top = 0.05 + 0.03 * x * x;
            let bottom = top - (0.006 + 0.026 * open) * arc.sqrt();
            let d = (p.y - top).max(bottom - p.y).max((x.abs() - 1.0) * wide);
            c = mix(c, pal.color(Material::Paper), cover(d, px));
            // gaps between the teeth and the line where the jaws meet
            let tooth = 0.0105;
            let gap = ((p.x / tooth + 0.5).fract() - 0.5).abs() * tooth - 0.0006;
            let meet = (p.y - top.midpoint(bottom)).abs() - 0.0005;
            c = mix(c, ink, cover(gap.min(meet).max(d), px));
            c = mix(c, ink, cover(d.abs() - 0.0008, px));
        }
        return c;
    }

    // nose: a short line down the shaded side, a hint of the nostril
    let nose = [
        Vec2::new(0.006, 0.096),
        Vec2::new(0.0045, 0.081),
        Vec2::new(-0.0015, 0.0745),
    ];
    // turned, the nose is a point of the contour: a painted line beside it would double it
    let nose_k = 1.0 - smooth(0.3, 0.45, view.x.abs());
    if nose_k > 0.0 && p.x.abs() < 0.012 && (0.068..0.102).contains(&p.y) {
        c = mix(c, ink, stroke(p, &nose, 0.0005, 0.0013, px) * nose_k);
    }
    c = mouth(c, p, px, look.mouth, pal);

    if look.blush > 0.0 {
        for s in [1.0_f32, -1.0] {
            let cc = Vec2::new(0.047 * s, 0.07);
            if (p.x - cc.x).abs() > 0.022 || (p.y - cc.y).abs() > 0.012 {
                continue;
            }
            let k = ((p - cc) / Vec2::new(0.018, 0.0075)).length();
            c = mix(
                c,
                Vec3::new(0.95, 0.35, 0.35),
                0.22 * look.blush * (1.0 - smooth(0.6, 1.0, k)),
            );
            for i in 0..3 {
                let x = -0.008 + 0.0058 * i as f32;
                let hatch = [
                    cc + Vec2::new(x, -0.0042),
                    cc + Vec2::new(x + 0.0038, 0.0045),
                ];
                c = mix(
                    c,
                    ink,
                    stroke(p, &hatch, 0.0011, 0.0006, px) * look.blush.min(1.0),
                );
            }
        }
    }
    if look.sweat > 0.0 {
        // a drop at the right temple, outlined, with a glint
        let r = 0.0058 * (0.6 + 0.5 * look.sweat);
        let q = p - Vec2::new(-0.064, 0.122);
        let k = (q.y / (2.6 * r)).clamp(0.0, 1.0);
        let d = if q.y <= 0.0 {
            q.length() - r
        } else {
            (q.x.abs() - r * (1.0 - k).powf(0.8)).max(q.y - 2.6 * r)
        };
        c = mix(c, paper, cover(d, px));
        c = mix(c, ink, cover(d.abs() - 0.00065, px));
        let glint = (q - Vec2::new(r * 0.35, -r * 0.1)).length() - r * 0.18;
        c = mix(c, ink, cover(glint, px) * 0.6);
    }
    if look.vein > 0.0 {
        // the anger mark: four bent strokes around a point, high on the left of the forehead
        let v = (p - Vec2::new(0.052, 0.136)) / look.vein.clamp(0.3, 1.0);
        if v.length() < 0.013 {
            for k in 0..4 {
                let (sn, cs) = (k as f32 * std::f32::consts::FRAC_PI_2).sin_cos();
                let rot = |x: f32, y: f32| Vec2::new(x * cs - y * sn, x * sn + y * cs);
                let arc = [
                    rot(0.0016, 0.0068),
                    rot(0.0026, 0.0026),
                    rot(0.0068, 0.0016),
                ];
                c = mix(c, ink, stroke(v, &arc, 0.0016, 0.0016, px));
            }
        }
    }
    c
}

/// Vertical gloom lines (縦線) hanging over a darkened upper face.
fn gloom(skin: Vec3, p: Vec2, px: f32, amount: f32, ink: Vec3) -> Vec3 {
    let mut c = mix(skin, ink, 0.3 * amount * smooth(0.088, 0.13, p.y));
    let pitch = 0.0062;
    let k = (p.x / pitch).round();
    if k.abs() > 9.0 {
        return c;
    }
    let lx = p.x - k * pitch;
    let h = ((k * 12.9898).sin() * 43_758.547).fract().abs();
    let top = 0.175;
    let bottom = top - (0.035 + 0.04 * h) * amount.sqrt();
    if p.y < top && p.y > bottom {
        let w = 0.0011 * ((p.y - bottom) / (top - bottom)).sqrt();
        c = mix(c, ink, cover(lx.abs() - w * 0.5, px));
    }
    c
}

/// The mouth, in face coordinates.
fn mouth(skin: Vec3, p: Vec2, px: f32, shape: Mouth, pal: &Palette) -> Vec3 {
    // every mouth shape lives in this box: nothing to draw on the rest of the face
    if p.x.abs() > 0.045 || !(-0.005..0.08).contains(&p.y) {
        return skin;
    }
    let ink = pal.line;
    let paper = pal.color(Material::Paper);
    let mut c = skin;
    let m = |x: f32, y: f32| Vec2::new(x, y);
    let line_w = 0.0019;
    // mouths carry the acting: never thinner than a pen line
    let line = |c: Vec3, pts: &[Vec2], w0: f32, w1: f32| {
        mix(
            c,
            ink,
            stroke(p, pts, w0.max(px * 1.5), w1.max(px * 1.5), px),
        )
    };
    match shape {
        Mouth::Line | Mouth::Talk { open: ..0.05, .. } => {
            c = line(
                c,
                &[m(0.015, 0.0485), m(0.0, 0.0475), m(-0.015, 0.0485)],
                line_w * 0.7,
                line_w,
            );
        }
        Mouth::Smile => {
            c = line(
                c,
                &[
                    m(0.02, 0.0525),
                    m(0.01, 0.046),
                    m(0.0, 0.0445),
                    m(-0.01, 0.046),
                    m(-0.02, 0.0525),
                ],
                line_w,
                line_w,
            );
        }
        Mouth::Frown => {
            c = line(
                c,
                &[m(0.016, 0.0435), m(0.0, 0.049), m(-0.016, 0.0435)],
                line_w,
                line_w,
            );
        }
        Mouth::Smirk(side) => {
            let s = if side >= 0.0 { 1.0 } else { -1.0 };
            c = line(
                c,
                &[
                    m(-0.014 * s, 0.0474),
                    m(-0.002 * s, 0.0466),
                    m(0.01 * s, 0.049),
                    m(0.0165 * s, 0.0545),
                ],
                line_w * 0.6,
                line_w * 1.1,
            );
            // the dimple at the raised corner
            c = line(
                c,
                &[m(0.0175 * s, 0.0505), m(0.0195 * s, 0.0535)],
                line_w * 0.5,
                line_w * 0.5,
            );
        }
        Mouth::Wavy => {
            let pts: [Vec2; 11] = std::array::from_fn(|i| {
                let x = -0.017 + i as f32 * 0.034 / 10.0;
                m(
                    x,
                    0.048 + 0.0016 * (x / 0.017 * 2.5 * std::f32::consts::PI).sin(),
                )
            });
            c = line(c, &pts, line_w * 0.8, line_w * 0.8);
        }
        Mouth::Cat => {
            c = line(
                c,
                &[
                    m(-0.013, 0.0515),
                    m(-0.0065, 0.0455),
                    m(0.0, 0.0495),
                    m(0.0065, 0.0455),
                    m(0.013, 0.0515),
                ],
                line_w * 0.9,
                line_w * 0.9,
            );
        }
        Mouth::Pout => {
            c = line(
                c,
                &[m(-0.0075, 0.0462), m(0.0, 0.0488), m(0.0075, 0.0462)],
                line_w,
                line_w,
            );
            c = line(
                c,
                &[m(-0.005, 0.0425), m(0.0, 0.0414), m(0.005, 0.0425)],
                line_w * 0.5,
                line_w * 0.5,
            );
        }
        Mouth::O(o) => {
            let r = Vec2::new(0.0045 + 0.003 * o, 0.005 + 0.007 * o);
            let q = p - m(0.0, 0.047);
            let d = ((q / r).length() - 1.0) * r.min_element();
            c = mix(c, ink, cover(d, px));
        }
        Mouth::Teeth(o) => {
            // clenched: a band of teeth, gaps between them, the line where they meet
            let half = Vec2::new(0.019 + 0.006 * o, 0.0042 + 0.0042 * o);
            let q = p - m(0.0, 0.0472);
            let rr = half.y * 0.8;
            let b = q.abs() - half + Vec2::splat(rr);
            let d = b.max(Vec2::ZERO).length() + b.max_element().min(0.0) - rr;
            c = mix(c, paper, cover(d, px));
            let tooth = 0.0062;
            let gap = ((q.x / tooth + 0.5).fract() - 0.5).abs() * tooth - 0.00035;
            let meet = q.y.abs() - 0.0003;
            c = mix(c, ink, cover(gap.min(meet).max(d), px));
            c = mix(c, ink, cover(d.abs() - 0.0007, px));
        }
        Mouth::Grin | Mouth::Open(_) | Mouth::Roar(_) | Mouth::Talk { .. } => {
            let (wide, depth, top, round) = match shape {
                Mouth::Grin => (0.025, 0.017, 0.053, false),
                Mouth::Open(o) => (0.016 + o * 0.004, 0.006 + 0.03 * o, 0.052, false),
                Mouth::Roar(o) => (0.03, 0.01 + 0.042 * o, 0.058, false),
                Mouth::Talk { open, wide } => (
                    0.0075 + 0.0165 * wide,
                    0.0025 + 0.03 * open,
                    0.0505 + 0.002 * wide,
                    wide < 0.45,
                ),
                _ => (0.0, 0.0, 0.0, false),
            };
            let d = if round {
                // "o" and "u": a whole ellipse hanging from the lip line
                let r = Vec2::new(wide, depth * 0.5);
                let q = p - m(0.0, top - depth * 0.5);
                ((q / r).length() - 1.0) * r.min_element()
            } else {
                // a D shape: a flat (slightly smiling) top over half an ellipse
                let q = p - m(0.0, top);
                let r = Vec2::new(wide, depth);
                let k = (q / r).length();
                let d_ellipse = (k - 1.0) * r.min_element();
                let d_top = q.y - 0.003 * (q.x / wide).powi(2);
                d_ellipse.max(d_top)
            };
            c = mix(c, ink, cover(d, px));
            // teeth along the top, a tongue at the bottom
            let teeth = match shape {
                Mouth::Grin | Mouth::Roar(_) => Some(0.0045),
                Mouth::Talk { open, wide } if wide > 0.6 && open > 0.15 => Some(0.0032),
                _ => None,
            };
            if let Some(h) = teeth {
                let band = (-h - (p.y - top)).max(d);
                c = mix(c, paper, cover(band, px));
            }
            let tongue = match shape {
                Mouth::Open(o) | Mouth::Roar(o) => o > 0.3,
                Mouth::Talk { open, .. } => open > 0.5,
                _ => false,
            };
            if tongue {
                let tq = (p - m(0.0, top - depth)) / Vec2::new(wide * 0.75, depth * 0.55);
                let tongue = (tq.length() - 1.0) * (depth * 0.55).min(wide * 0.75);
                c = mix(
                    c,
                    // a darker tone of the paper, like the shadow inside the mouth: the
                    // accent colour (teal, green) reads as a stained tongue
                    mix(paper, ink, 0.45),
                    cover(tongue.max(d), px),
                );
            }
            c = mix(c, ink, cover(d.abs() - px * 0.6, px));
        }
    }
    c
}

fn h01(i: u32, k: u32) -> f32 {
    let mut h = i.wrapping_mul(0x9E37_79B9) ^ k.wrapping_mul(0x85EB_CA6B);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    (h & 0xFFFF) as f32 / 65_536.0
}

/// A drip of goo hanging from `base`: a tapering string with a drop at the end.
fn drip(m: &mut Model, g: u8, base: Vec3, len: f32, r: f32, bone: Bone) {
    let tip = base - Vec3::Y * len;
    m.push(cone(base + Vec3::Y * r * 0.5, tip, r, r * 0.45, g, bone).blend(r * 0.8));
    m.push(ellipsoid(tip, Vec3::splat(r * 0.75), g, bone).blend(r * 0.5));
}

/// SLOP's body: a sagging pear of glossy goo with lumps bulging out of it, thick arms swelling
/// at the wrists, stumpy legs melting into puddles, drips hanging everywhere.
fn slime_body(d: &Design, j: &Posed) -> Model {
    use group::SUIT;
    let b = d.build;
    let mut m = Model::default();
    let hips = j.pos(Bone::Hips);
    let spine = j.pos(Bone::Spine);
    let chest = j.pos(Bone::Chest);
    let neck = j.pos(Bone::Neck);
    let head = j.pos(Bone::Head);
    // a pear of goo: narrow, sloping shoulders, everything sagging into a huge belly that hangs
    // over the hips and swallows the tops of the legs
    m.push(ellipsoid(
        spine + Vec3::new(0.0, -0.1, 0.04),
        Vec3::new(0.25, 0.24, 0.21) * b,
        SUIT,
        Bone::Spine,
    ));
    m.push(
        ellipsoid(
            chest + Vec3::new(0.0, 0.04, -0.01),
            Vec3::new(0.16, 0.16, 0.14) * b,
            SUIT,
            Bone::Chest,
        )
        .blend(0.12),
    );
    m.push(
        ellipsoid(
            hips + Vec3::new(0.0, -0.08, 0.02),
            Vec3::new(0.24, 0.15, 0.19) * b,
            SUIT,
            Bone::Hips,
        )
        .blend(0.1),
    );
    // the belly sags forward, one shoulder hunched higher than the other
    m.push(
        ellipsoid(
            spine + Vec3::new(0.03, -0.2, 0.12),
            Vec3::new(0.21, 0.17, 0.15) * b,
            SUIT,
            Bone::Spine,
        )
        .blend(0.08),
    );
    m.push(
        ellipsoid(
            chest + Vec3::new(0.1, 0.12, -0.03),
            Vec3::new(0.11, 0.1, 0.11) * b,
            SUIT,
            Bone::Chest,
        )
        .blend(0.09),
    );
    // lumps bulging out of it, lopsided
    for i in 0..16 {
        let a = h01(i, 1) * TAU;
        let y = -0.25 + h01(i, 2) * 0.4;
        let r = (0.04 + 0.05 * h01(i, 3)) * b;
        let wide = 0.2 - 0.06 * ((y + 0.25) / 0.4);
        let p = spine + Vec3::new(a.sin() * wide * b, y, a.cos() * wide * 0.85 * b);
        let bone = if y > 0.12 {
            Bone::Chest
        } else if y < -0.06 {
            Bone::Hips
        } else {
            Bone::Spine
        };
        m.push(ellipsoid(p, Vec3::splat(r), SUIT, bone).blend(0.05));
    }
    // a thick column of goo up into the head
    m.push(
        cone(
            neck + Vec3::new(0.0, -0.04, 0.0),
            head + Vec3::new(0.0, 0.05, 0.0),
            0.11 * b,
            0.09 * b,
            SUIT,
            Bone::Neck,
        )
        .blend(0.06),
    );
    for side in [Side::Left, Side::Right] {
        let s = side.sign();
        let [_, upper, lower, hand] = Bone::arm(side);
        let (a, e, w) = (j.pos(upper), j.pos(lower), j.pos(hand));
        // sloping shoulders: the arm grows out of the side of the mass
        m.push(
            ellipsoid(
                a + Vec3::new(-0.05 * s, -0.03, 0.0),
                Vec3::new(0.09, 0.085, 0.09) * b,
                SUIT,
                Bone::Chest,
            )
            .blend(0.1),
        );
        m.push(cone(a, e, 0.075 * b, 0.065 * b, SUIT, upper).blend(0.06));
        m.push(cone(e, w, 0.07 * b, 0.08 * b, SUIT, lower).blend(0.04));
        m.push(
            ellipsoid(
                w - (w - e).normalize() * 0.02,
                Vec3::splat(0.07 * b),
                SUIT,
                lower,
            )
            .blend(0.03),
        );
        let _ = hand;
        // drips hanging under the forearm and the elbow
        for k in 0..3 {
            let p = e.lerp(w, 0.2 + 0.3 * k as f32) - Vec3::Y * 0.055 * b;
            let len = 0.05 + 0.05 * h01(k + 7 * u32::from(s > 0.0), 4);
            drip(&mut m, SUIT, p, len, 0.018 * b, lower);
        }
        drip(&mut m, SUIT, e - Vec3::Y * 0.06 * b, 0.07, 0.02 * b, upper);
    }
    for side in [Side::Left, Side::Right] {
        let [thigh, shin, foot, toes] = Bone::leg(side);
        let (h, k, a, t) = (j.pos(thigh), j.pos(shin), j.pos(foot), j.pos(toes));
        m.push(cone(h + Vec3::Y * 0.05, k, 0.12 * b, 0.09 * b, SUIT, thigh).blend(0.08));
        m.push(cone(k, a, 0.09 * b, 0.1 * b, SUIT, shin).blend(0.05));
        let ground = t.y;
        // the feet melt into puddles
        m.push(
            ellipsoid(
                Vec3::new(a.x, ground + 0.03, f32::midpoint(a.z, t.z)),
                Vec3::new(0.12, 0.045, 0.15) * b,
                SUIT,
                foot,
            )
            .blend(0.05),
        );
        m.push(keep_side(Vec3::new(0.0, ground, 0.0), Vec3::Y, SUIT, foot).blend(0.01));
    }
    // drips from the belly and the chest
    for i in 0..9 {
        let a = (h01(i, 5) - 0.5) * 2.4;
        let p = spine
            + Vec3::new(
                a.sin() * 0.22 * b,
                -0.33 - 0.04 * h01(i, 6),
                a.cos() * 0.19 * b + 0.04,
            );
        drip(
            &mut m,
            SUIT,
            p,
            0.07 + 0.12 * h01(i, 7),
            0.028 * b,
            Bone::Hips,
        );
    }
    for i in 0..5 {
        let a = (h01(i, 8) - 0.5) * 2.0;
        let p = chest + Vec3::new(a.sin() * 0.2 * b, -0.02, a.cos() * 0.15 * b);
        drip(
            &mut m,
            SUIT,
            p,
            0.04 + 0.05 * h01(i, 9),
            0.016 * b,
            Bone::Chest,
        );
    }
    m
}

/// SLOP's head: a lopsided bulb of goo with a sagging jaw, drool hanging from the chin and goo
/// tendrils along the spring chains (so they swing).
fn slime_head(d: &Design, h: Vec3, hs: f32) -> Model {
    use group::{HAIR, SKIN};
    let mut m = Model::default();
    let s = hs;
    let at = |x: f32, y: f32, z: f32| h + Vec3::new(x, y, z) * s;
    m.push(ellipsoid(
        at(0.0, 0.1, 0.0),
        Vec3::new(0.11, 0.115, 0.1) * s,
        SKIN,
        Bone::Head,
    ));
    m.push(
        ellipsoid(
            at(0.045, 0.175, -0.015),
            Vec3::splat(0.07 * s),
            SKIN,
            Bone::Head,
        )
        .blend(0.04 * s),
    );
    m.push(
        ellipsoid(
            at(-0.075, 0.085, 0.0),
            Vec3::splat(0.055 * s),
            SKIN,
            Bone::Head,
        )
        .blend(0.035 * s),
    );
    m.push(
        ellipsoid(
            at(0.0, 0.03, 0.035),
            Vec3::new(0.1, 0.055, 0.075) * s,
            SKIN,
            Bone::Head,
        )
        .blend(0.05 * s),
    );
    for i in 0..6 {
        let u = i as f32 / 5.0 * 2.0 - 1.0;
        let base = at(0.075 * u, -0.005 + 0.02 * u * u, 0.06 - 0.03 * u * u);
        let len = (0.02 + 0.025 * h01(i, 11)) * s;
        drip(&mut m, SKIN, base, len, 0.011 * s, Bone::Head);
    }
    for clump in &d.hair.clumps {
        let pts = spline(
            &clump
                .rest_points(HAIR_SEGMENTS)
                .iter()
                .map(|p| h + *p * s)
                .collect::<Vec<_>>(),
            3,
        );
        let n = pts.len();
        let r = |t: f32| clump.width * s * 0.5 * (1.0 - 0.6 * t);
        for k in 0..n - 1 {
            let (t0, t1) = (k as f32 / (n - 1) as f32, (k + 1) as f32 / (n - 1) as f32);
            m.push(cone(pts[k], pts[k + 1], r(t0), r(t1), HAIR, Bone::Head).blend(0.012 * s));
        }
        if let Some(tip) = pts.last() {
            m.push(
                ellipsoid(*tip, Vec3::splat(clump.width * s * 0.32), HAIR, Bone::Head)
                    .blend(0.006 * s),
            );
        }
    }
    m
}

/// Glossy goo: the AI gradient from violet at the feet to pink at the head, bubbles trapped
/// inside, hard-edged highlights and a pink rim of light.
fn goo(f: &Fragment, pal: &Palette, top: f32) -> Vec3 {
    let low = pal.color(Material::Suit);
    let high = pal.color(Material::Scarf);
    let k = smooth(0.15, top, f.rest.y);
    let mut c = mix(low, high, k);
    // bubbles: one per cell of a 6 cm grid in the rest pose, some of them
    let cell = (f.rest / 0.06).floor();
    let id = (cell.x as i32 as u32).wrapping_mul(73_856_093)
        ^ (cell.y as i32 as u32).wrapping_mul(19_349_663)
        ^ (cell.z as i32 as u32).wrapping_mul(83_492_791);
    if h01(id, 1) > 0.8 {
        let centre = (cell + Vec3::new(h01(id, 2), h01(id, 3), h01(id, 4)) * 0.6 + 0.2) * 0.06;
        let r = 0.006 + 0.01 * h01(id, 5);
        let d = (f.rest - centre).length() - r;
        let ring = cover(d.abs() - 0.0018, f.px * 0.5);
        c = mix(
            c,
            mix(c, pal.color(Material::Paper), 0.5),
            cover(d, f.px * 0.5) * 0.35,
        );
        c = mix(c, pal.color(Material::Paper), ring * 0.6);
    }
    let n = f.normal;
    let half = (pal.light + f.view).normalize_or(f.view);
    let spec = n.dot(half);
    c = mix(
        c,
        pal.color(Material::Paper),
        smooth(0.965, 0.972, spec) * 0.9,
    );
    let rim = 1.0 - n.dot(f.view).abs();
    c = mix(c, high * 1.15, smooth(0.62, 0.7, rim) * 0.55);
    c
}

/// SLOP's face: five mismatched eyes each looking its own way, the AI sparkle on the forehead,
/// a grin from cheek to cheek with two rows of far too many teeth.
fn slop_face(skin: Vec3, p: Vec2, px: f32, look: &Look, pal: &Palette) -> Vec3 {
    let ink = pal.line;
    let paper = pal.color(Material::Paper);
    let glow = pal.color(Material::Accent);
    let mut c = skin;
    let eyes = [
        (Vec2::new(0.034, 0.118), 0.027, Vec2::new(0.3, 0.1)),
        (Vec2::new(-0.036, 0.124), 0.019, Vec2::new(-0.5, 0.3)),
        (Vec2::new(-0.006, 0.168), 0.011, Vec2::new(0.6, -0.5)),
        (Vec2::new(0.075, 0.078), 0.012, Vec2::new(-0.7, 0.0)),
        (Vec2::new(-0.074, 0.088), 0.008, Vec2::new(0.2, 0.7)),
    ];
    for (i, (centre, r, own)) in eyes.iter().enumerate() {
        let q = p - *centre;
        if q.length() > r * 1.8 {
            continue;
        }
        // the eyes blink out of step, the small ones first
        let lag = i as f32 * 0.12;
        let open = ((look.eyes - lag) / (1.0 - lag)).clamp(0.0, 1.0);
        let h = r * open.max(0.06);
        let d = ((q / Vec2::new(*r, h)).length() - 1.0) * h.min(*r);
        c = mix(c, paper, cover(d, px));
        let g = (*own * 0.7 + look.gaze).clamp(Vec2::splat(-1.0), Vec2::splat(1.0));
        let ir = r * 0.5 * look.pupils.clamp(0.3, 1.5);
        let ic = g * (r - ir) * 0.8;
        let di = (q - ic).length() - ir;
        c = mix(c, glow, cover(di.max(d), px));
        c = mix(c, ink, cover(((q - ic).length() - ir * 0.42).max(d), px));
        let glint = (q - ic - Vec2::new(-ir * 0.35, ir * 0.35)).length() - ir * 0.22;
        c = mix(c, paper, cover(glint.max(d), px));
        c = mix(c, ink, cover(d.abs() - 0.0012_f32.max(px * 0.8), px));
    }
    // the AI sparkle
    let sq = (p - Vec2::new(0.04, 0.178)).abs() / 0.017;
    let star = (sq.x.sqrt() + sq.y.sqrt() - 1.0) * 0.006;
    c = mix(c, paper, cover(star, px));
    c = mix(c, ink, cover(star.abs() - 0.0007, px));
    // the grin
    let open = match look.mouth {
        Mouth::Roar(o) | Mouth::Open(o) => 0.25 + 0.75 * o,
        Mouth::Talk { open, .. } => 0.25 + 0.75 * open,
        Mouth::Grin | Mouth::Teeth(_) => 0.2,
        _ => 0.12,
    };
    let wide = 0.085;
    let x = p.x / wide;
    if x.abs() < 1.05 {
        let arc = (1.0 - x * x).max(0.0);
        let top = 0.058 + 0.028 * x * x;
        let bottom = top - (0.008 + 0.04 * open) * arc.sqrt();
        let d = (p.y - top).max(bottom - p.y).max((x.abs() - 1.0) * wide);
        c = mix(c, mix(ink, pal.color(Material::Scarf), 0.25), cover(d, px));
        let tooth = 0.0068;
        let gx = ((p.x / tooth + 0.5).fract() - 0.5).abs() * tooth;
        let th = 0.0072 + 0.003 * open;
        let teeth = ((top - p.y) - th).min((p.y - bottom) - th * 0.9).max(d);
        c = mix(c, paper, cover(teeth, px));
        c = mix(c, ink, cover((gx - 0.0005).max(teeth), px));
        c = mix(c, ink, cover(d.abs() - 0.001, px));
    }
    c
}

/// A demon's face: glowing eyes under a scowl, slit nostrils, a roaring mouth of fangs.
fn monster_face(skin: Vec3, p: Vec2, px: f32, look: &Look, pal: &Palette) -> Vec3 {
    let ink = pal.line;
    let glow = pal.color(Material::Accent);
    let mut c = skin;
    for s in [1.0_f32, -1.0] {
        let e = Vec2::new((p.x - 0.036 * s) * s, p.y - 0.098);
        // eye: an almond slanting down to the nose, glowing, with a slit pupil
        let q = Vec2::new(e.x, e.y + 0.18 * e.x);
        let h = 0.007 * look.eye_open(s).max(0.08);
        let d = ((q * Vec2::new(1.0 / 0.019, 1.0 / h)).length() - 1.0) * h;
        c = mix(c, glow, cover(d, px));
        let g = Vec2::new(look.gaze.x * s * 0.009, look.gaze.y * 0.003);
        let slit = (((q - g) * Vec2::new(1.0 / 0.0024, 1.0 / 0.0068)).length() - 1.0) * 0.0024;
        c = mix(c, ink, cover(slit.max(d), px));
        c = mix(c, ink, cover(d.abs() - 0.0009, px));
        // a heavy scowl over it
        let lift = look.brows * 0.005 - look.brow_angle.min(0.0) * 0.004;
        let brow = [
            Vec2::new(-0.02, 0.006 + lift * 1.6),
            Vec2::new(0.0, 0.012 + lift),
            Vec2::new(0.024, 0.02 + lift * 0.5),
        ];
        c = mix(c, ink, stroke(e, &brow, 0.007, 0.003, px));
        // wrinkles between the brows
        let wr = [Vec2::new(-0.026, 0.0), Vec2::new(-0.03, 0.02)];
        c = mix(c, ink, stroke(e, &wr, 0.0018, 0.0008, px));
        // slit nostrils
        let n = Vec2::new(p.x - 0.008 * s, p.y - 0.072);
        let dn = ((n * Vec2::new(1.0 / 0.0025, 1.0 / 0.0045)).length() - 1.0) * 0.0025;
        c = mix(c, ink, cover(dn, px));
    }
    // a roaring mouth: dark maw, a zigzag of fangs top and bottom
    let open = match look.mouth {
        Mouth::Roar(o) | Mouth::Open(o) => 0.2 + o * 0.8,
        Mouth::Talk { open, .. } => 0.2 + open * 0.8,
        Mouth::Grin | Mouth::Teeth(_) => 0.15,
        _ => 0.08,
    };
    let wide = 0.04;
    let x = p.x / wide;
    if x.abs() < 1.2 {
        let arc = (1.0 - x * x).max(0.0);
        let top = 0.05 + 0.006 * x * x;
        let bottom = top - (0.006 + 0.04 * open) * arc.sqrt();
        let d = (p.y - top).max(bottom - p.y).max((x.abs() - 1.0) * wide);
        c = mix(c, mix(ink, pal.color(Material::Accent), 0.12), cover(d, px));
        let tooth = 0.009;
        let saw = ((p.x / tooth + 0.5).fract() - 0.5).abs() * 2.0;
        let fang_top = top - p.y - (1.0 - saw) * 0.007 * (0.4 + open);
        let fang_bottom = p.y - bottom - (1.0 - saw) * 0.006 * (0.4 + open);
        let teeth = fang_top.min(fang_bottom).max(d);
        c = mix(c, pal.color(Material::Paper), cover(teeth, px));
        c = mix(c, ink, cover(d.abs() - 0.0009, px));
    }
    c
}

/// A laced shoe: a sole in the paper colour, laces crossing over the tongue, a seam at the toe
/// cap.
fn shoe(base: Vec3, rest: Vec3, skeleton: &Posed, px: f32, pal: &Palette) -> Vec3 {
    let (ankle, toes) = if rest.x > 0.0 {
        (skeleton.pos(Bone::FootL), skeleton.pos(Bone::ToesL))
    } else {
        (skeleton.pos(Bone::FootR), skeleton.pos(Bone::ToesR))
    };
    let q = rest - ankle;
    let ground = toes.y - ankle.y;
    let paper = pal.color(Material::Paper);
    let mut c = base;
    // the sole
    let sole = q.y - ground - 0.011;
    c = mix(c, mix(paper, pal.line, 0.25), cover(sole, px));
    c = mix(c, pal.line, cover(sole.abs() - 0.0009, px));
    // laces over the instep: an X every 1.4 cm along the foot
    let along = q.z - 0.012;
    if (0.0..0.07).contains(&along) && q.y > ground + 0.03 {
        let k = (along / 0.014).fract() - 0.5;
        let across = q.x / 0.018;
        let d1 = (across - k * 2.0).abs() * 0.009;
        let d2 = (across + k * 2.0).abs() * 0.009;
        let lace = d1.min(d2) - 0.0011;
        c = mix(c, paper, cover(lace.max(across.abs() * 0.018 - 0.018), px));
    }
    // the toe cap seam
    let cap = (q.z - 0.085 - 0.6 * q.x * q.x / 0.02).abs() - 0.0007;
    if q.y > ground + 0.012 {
        c = mix(c, pal.line, cover(cap, px));
    }
    c
}

/// The inside of an ear: a C curling towards the face and the notch of the canal.
fn ear(base: Vec3, q: Vec3, px: f32, pal: &Palette) -> Vec3 {
    let e = Vec2::new(q.z + 0.012, q.y - 0.088);
    if e.length() > 0.03 {
        return base;
    }
    let arc: Vec<Vec2> = (0..9)
        .map(|i| {
            let a = (70.0 + i as f32 * 25.0_f32).to_radians();
            Vec2::new(0.002 + a.cos() * 0.0085, a.sin() * 0.0145)
        })
        .collect();
    let c = mix(base, pal.line, stroke(e, &arc, 0.0007, 0.0013, px));
    let notch = [
        Vec2::new(0.004, 0.003),
        Vec2::new(0.0, -0.001),
        Vec2::new(0.003, -0.006),
    ];
    mix(c, pal.line, stroke(e, &notch, 0.0011, 0.0005, px))
}

/// Strands of a beard: short strokes combed down, jittered, fading out when small.
fn beard(base: Vec3, q: Vec3, px: f32, pal: &Palette) -> Vec3 {
    let fade = 1.0 - smooth(0.0006, 0.0011, px);
    if fade <= 0.0 {
        return base;
    }
    // unwrap the jaw: across the face by angle, down by height
    let u = q.x.atan2(q.z.max(-0.05) + 0.03) * 0.06;
    let p = Vec2::new(u, q.y);
    let cell = Vec2::new(0.0085, 0.0105);
    let g = (p / cell).floor();
    let mut c = base;
    for dy in -1..=1 {
        for dx in -1..=1 {
            let cg = g + Vec2::new(dx as f32, dy as f32);
            let h = hash2f(cg);
            if h.x > 0.62 {
                continue;
            }
            let o = (cg + Vec2::new(0.2 + h.x * 0.6, 0.2 + h.y * 0.6)) * cell;
            let lean = (h.y - 0.5) * 0.004 + u * 0.08;
            let pts = [
                o,
                o + Vec2::new(lean * 0.5, -0.0035),
                o + Vec2::new(lean, -0.0068),
            ];
            c = mix(c, pal.line, stroke(p, &pts, 0.0006, 0.0002, px) * fade);
        }
    }
    c
}

fn hash2f(p: Vec2) -> Vec2 {
    let h = |x: f32| (x.sin() * 43758.547).fract().abs();
    Vec2::new(
        h(p.dot(Vec2::new(127.1, 311.7))),
        h(p.dot(Vec2::new(269.5, 183.3))),
    )
}

/// Shirt details on the front (chest-local rest coordinates): the button placket and the folds
/// where the shirt is tucked in at the belt.
fn shirt(base: Vec3, q: Vec3, belt: f32, px: f32, pal: &Palette) -> Vec3 {
    if q.z < 0.0 {
        return base;
    }
    let mut c = base;
    let ink = pal.line;
    let p = Vec2::new(q.x, q.y);
    let w = 0.0016_f32.max(px * 0.8);
    // the placket, beside the tie
    let placket = [Vec2::new(0.012, 0.17), Vec2::new(0.012, belt + 0.01)];
    c = mix(c, ink, stroke(p, &placket, w * 0.5, w * 0.7, px));
    // tucked in: a sag over the belt and creases pulled into it
    let sag = [
        Vec2::new(-0.09, belt + 0.012),
        Vec2::new(-0.03, belt + 0.004),
        Vec2::new(0.03, belt + 0.006),
        Vec2::new(0.09, belt + 0.014),
    ];
    c = mix(c, ink, stroke(p, &sag, w, w * 0.6, px));
    for (x, dx, len) in [
        (-0.055_f32, -0.012_f32, 0.05_f32),
        (0.045, 0.01, 0.06),
        (0.075, 0.02, 0.035),
    ] {
        let crease = [
            Vec2::new(x, belt + 0.012),
            Vec2::new(x + dx, belt + 0.012 + len),
        ];
        c = mix(c, ink, stroke(p, &crease, w * 0.9, w * 0.3, px));
    }
    c
}

/// Ink lines of a muscular chest and belly.
fn muscles(base: Vec3, q: Vec3, px: f32, pal: &Palette, b: f32) -> Vec3 {
    if q.z < 0.0 {
        return base;
    }
    let mut c = base;
    let p = Vec2::new(q.x / b, q.y);
    let w = 0.004_f32.max(px);
    for s in [1.0_f32, -1.0] {
        let pec = [
            Vec2::new(0.01 * s, 0.035),
            Vec2::new(0.06 * s, 0.015),
            Vec2::new(0.12 * s, 0.05),
            Vec2::new(0.14 * s, 0.1),
        ];
        c = mix(c, pal.line, stroke(p, &pec, w * 1.4, w * 0.5, px));
        for k in 0..3 {
            let y = -0.05 - k as f32 * 0.065;
            let ab = [
                Vec2::new(0.012 * s, y),
                Vec2::new(0.045 * s, y - 0.01),
                Vec2::new(0.062 * s, y + 0.008),
            ];
            c = mix(c, pal.line, stroke(p, &ab, w, w * 0.6, px));
        }
    }
    let centre = [Vec2::new(0.0, 0.04), Vec2::new(0.0, -0.26)];
    mix(c, pal.line, stroke(p, &centre, w, w * 0.6, px))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rig::Proportions;
    use crate::toon::Hair;

    pub fn sample_design() -> Design {
        Design {
            hair: Hair {
                cap_normal: Vec3::new(0.0, 0.9, 0.3).normalize(),
                cap_offset: 0.0,
                volume: 1.05,
                fringe: 0.02,
                clumps: Vec::new(),
            },
            glasses: Some(Glasses::Round),
            beard: 0.6,
            robe: false,
            scarf: false,
            tie: true,
            robe_length: 1.0,
            build: 1.0,
            muscles: false,
            hood: false,
            slime: false,
        }
    }

    #[test]
    fn figure_meshes_are_reasonable() {
        let sk = Skeleton::new(Proportions::anime_adult());
        let t0 = std::time::Instant::now();
        let fig = Figure::new(&sample_design(), &sk, Detail(1.6));
        eprintln!(
            "body {} tris, head {} tris, hand {} tris in {:?}",
            fig.body.tris.len(),
            fig.head.tris.len(),
            fig.hands[0][0].tris.len(),
            t0.elapsed()
        );
        assert!(fig.body.tris.len() > 1000);
        assert!(fig.head.tris.len() > 1000);
        // hands sit at the wrists
        let wrist = fig.rest.pos(Bone::HandL);
        let c: Vec3 =
            fig.hands[0][0].rest.iter().copied().sum::<Vec3>() / fig.hands[0][0].rest.len() as f32;
        assert!(c.x > wrist.x, "left hand extends to +x");
    }
}
