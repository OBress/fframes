//! The crowd of arms on the production lines. All arms draw as instances: one draw for each
//! part. Arms near the camera use the parts of the hero arm in [`super::arm`]. Arms farther
//! away use coarse copies of its links with about 1/50 of the triangles, and arms far away
//! use boxes. Arms out of the view do not draw.

use std::f32::consts::{PI, TAU};
use std::sync::Arc;

use fframes_3d::math::{hash, smoothstep};
use fframes_3d::*;

use super::arm::{
    self, A1, A3, Arm, EYE, Expression, GRIP, J2_UP, J4_AT, Joints, L2, L3, Palette, axle, hull,
    side_plate,
};

/// The parts of the coarse arm: the base and the six joints, each with a painted shell and
/// dark metal, in the frame of its joint.
pub struct Model {
    shells: [Geometry; 7],
    metals: [Geometry; 7],
    /// Boxes and cylinders of about 150 triangles for each arm, for arms far away.
    far: [Geometry; 7],
    /// The lens of the eye in the J6 frame. It glows in the color of each instance.
    eye: Geometry,
}

/// Every `step`-th point of a polygon.
fn coarse(shape: Vec<Vec2>, step: usize) -> Vec<Vec2> {
    shape.into_iter().step_by(step).collect()
}

/// A cylinder along Z from `z0` to `z1` at height `y`.
fn along_z(r0: f32, r1: f32, y: f32, z0: f32, z1: f32, segments: u32) -> Geometry {
    Geometry::cylinder(r1, r0, z1 - z0, segments)
        .rotate_x(deg(90.0))
        .translate(0.0, y, f32::midpoint(z0, z1))
}

impl Model {
    pub fn new() -> Self {
        let column = [
            vec2(0.0, 0.05),
            vec2(0.36, 0.05),
            vec2(0.36, 0.09),
            vec2(0.31, 0.13),
            vec2(0.3, 0.37),
            vec2(0.335, 0.405),
            vec2(0.335, 0.45),
            vec2(0.0, 0.45),
        ];
        let base_shell = Geometry::lathe(&column, 20);
        let base_metal = Geometry::rounded_box(1.0, 0.05, 1.0, 0.02, 1).translate(0.0, 0.025, 0.0);

        let cheek = coarse(
            hull(&[
                (vec2(A1, J2_UP), 0.235),
                (vec2(-0.08, 0.13), 0.22),
                (vec2(0.18, 0.1), 0.2),
            ]),
            4,
        );
        let j1_shell = Geometry::merge([
            side_plate(&cheek, 0.09, 0.0, -0.25),
            side_plate(&cheek, 0.09, 0.0, 0.25),
            Geometry::rounded_box(0.42, 0.18, 0.44, 0.05, 1).translate(0.0, 0.13, 0.02),
            Geometry::cylinder(0.1, 0.1, 0.3, 10).translate(0.0, 0.25, -0.33),
        ]);
        let j1_metal = Geometry::merge([
            Geometry::cylinder(0.34, 0.345, 0.07, 20).translate(0.0, 0.035, 0.0),
            axle(0.2, 0.42, 0.0, J2_UP, A1),
            axle(0.125, 0.22, -0.44, J2_UP, A1),
        ]);

        let beam = coarse(hull(&[(vec2(0.0, 0.0), 0.19), (vec2(0.0, L2), 0.15)]), 4);
        let j2_shell = side_plate(&beam, 0.3, 0.0, 0.0);
        let panel = coarse(
            hull(&[(vec2(0.0, 0.25), 0.1), (vec2(0.0, L2 - 0.25), 0.08)]),
            6,
        );
        let j2_metal = Geometry::merge([
            side_plate(&panel, 0.016, 0.0, -0.155),
            side_plate(&panel, 0.016, 0.0, 0.155),
            axle(0.1, 0.16, 0.26, L2, 0.0),
        ]);

        let j3_shell = Geometry::merge([
            Geometry::rounded_box(0.3, 0.3, 0.34, 0.07, 1).translate(0.0, A3, -0.18),
            along_z(0.13, 0.105, A3, 0.02, 0.52, 14),
        ]);
        let j3_metal = Geometry::merge(
            [(-0.07f32, 0.07f32), (0.07, 0.07), (0.0, -0.07)]
                .iter()
                .map(|(x, y)| along_z(0.05, 0.05, A3 + y, -0.47, -0.35, 8).translate(*x, 0.0, 0.0)),
        );

        let fork = coarse(
            hull(&[(vec2(0.2, 0.0), 0.06), (vec2(L3 - J4_AT, 0.0), 0.075)]),
            6,
        );
        let j4_shell = along_z(0.1, 0.095, 0.0, -0.06, 0.18, 14);
        let j4_metal = Geometry::merge([
            side_plate(&fork, 0.03, 0.0, -0.085),
            side_plate(&fork, 0.03, 0.0, 0.085),
        ]);

        let j5_shell = Geometry::merge([
            axle(0.065, 0.14, 0.0, 0.0, 0.0),
            along_z(0.06, 0.055, 0.0, 0.015, 0.145, 12),
        ]);
        let j5_metal = axle(0.045, 0.22, 0.0, 0.0, 0.0);

        let j6_shell = Geometry::rounded_box(0.11, 0.06, 0.09, 0.018, 1).translate(
            EYE.x,
            EYE.y - 0.002,
            EYE.z - 0.04,
        );
        let finger = Geometry::rounded_box(0.026, 0.05, 0.13, 0.008, 1);
        let j6_metal = Geometry::merge([
            along_z(0.068, 0.068, 0.0, 0.0, 0.07, 12),
            Geometry::rounded_box(0.26, 0.085, 0.08, 0.02, 1).translate(0.0, 0.0, 0.104),
            finger.clone().translate(-0.07, 0.0, GRIP - 0.06),
            finger.translate(0.07, 0.0, GRIP - 0.06),
        ]);
        let eye = Geometry::cylinder(0.02, 0.02, 0.004, 12)
            .rotate_x(deg(90.0))
            .translate(EYE.x, EYE.y, EYE.z + 0.006);

        let block = |w: f32, h: f32, d: f32, at: Vec3| {
            Geometry::cuboid(w, h, d).translate(at.x, at.y, at.z)
        };
        let far = [
            Geometry::cylinder(0.33, 0.35, 0.4, 8).translate(0.0, 0.25, 0.0),
            block(0.6, 0.5, 0.5, vec3(0.0, 0.25, 0.04)),
            block(0.3, L2 + 0.25, 0.34, vec3(0.0, L2 * 0.5, 0.0)),
            block(0.28, 0.28, 0.95, vec3(0.0, A3, 0.05)),
            block(0.2, 0.2, 0.42, vec3(0.0, 0.0, 0.19)),
            block(0.15, 0.13, 0.15, Vec3::ZERO),
            block(0.26, 0.09, 0.32, vec3(0.0, 0.0, 0.14)),
        ];

        Self {
            shells: [
                base_shell, j1_shell, j2_shell, j3_shell, j4_shell, j5_shell, j6_shell,
            ],
            metals: [
                base_metal, j1_metal, j2_metal, j3_metal, j4_metal, j5_metal, j6_metal,
            ],
            far,
            eye,
        }
    }
}

impl Default for Model {
    fn default() -> Self {
        Self::new()
    }
}

/// The world transforms of the base and the six joint frames of an arm whose base stands at
/// `base`.
pub fn frames(base: Mat4, q: &Joints) -> [Mat4; 7] {
    let mut out = [base; 7];
    for (i, local) in arm::chain(q).iter().enumerate() {
        out[i + 1] = out[i] * *local;
    }
    out
}

/// The meshes of the hero arm, merged for each joint frame and material: `(frame, geometry,
/// material)`. Frame 0 is the base.
pub struct Detail(Vec<(usize, Geometry, Material)>);

impl Detail {
    /// Builds a hero arm and moves each of its meshes into the frame of its joint. The iris
    /// is left out: the crowd draws its own eyes. So is the gas spring, which has no joint.
    pub fn bake(palette: &Palette, font: &[u8]) -> Self {
        let mut scene = Scene::new();
        let arm = Arm::build(&mut scene, palette, font);
        let face = Expression {
            ring: 2.0,
            status: 3.0,
            eye: 0.0,
            lid: 1.0,
        };
        arm.apply(&mut scene, &Joints::default(), arm::OPEN * 0.4, &face);
        let frames = arm.frames();
        let world = scene.world_matrices();
        let mut parts: Vec<(usize, String, Material, Vec<Geometry>)> = Vec::new();
        for (id, node) in scene.nodes() {
            let Object::Mesh(mesh) = &node.object else {
                continue;
            };
            if id == arm.iris {
                continue;
            }
            // the nearest ancestor that is a joint frame
            let mut parent = node.parent();
            let frame = loop {
                match parent {
                    Some(p) => match frames.iter().position(|f| *f == p) {
                        Some(i) => break Some(i),
                        None => parent = scene[p].parent(),
                    },
                    None => break None,
                }
            };
            let Some(frame) = frame else {
                continue;
            };
            let to_frame = world[frames[frame].index()].inverse() * world[id.index()];
            let geometry = mesh.geometry.as_ref().clone().transform(to_frame);
            let key = format!("{:?}", mesh.material);
            match parts.iter_mut().find(|(f, k, ..)| *f == frame && *k == key) {
                Some((.., list)) => list.push(geometry),
                None => parts.push((frame, key, mesh.material.clone(), vec![geometry])),
            }
        }
        Self(
            parts
                .into_iter()
                .map(|(frame, _, material, list)| (frame, Geometry::merge(list), material))
                .collect(),
        )
    }
}

/// The instanced nodes of the crowd in a scene. [`Crowd::pose`] fills their instances.
pub struct Crowd {
    shells: [NodeId; 7],
    metals: [NodeId; 7],
    far: [NodeId; 7],
    /// The parts of the near arms, with their joint frames.
    detail: Vec<(usize, NodeId)>,
    eyes: NodeId,
}

/// One arm of the crowd: where it stands and what it does.
#[derive(Debug, Clone, Copy)]
pub struct Station {
    /// The base on the floor. The arm faces its local +Z.
    pub base: Mat4,
    /// The offset of its work cycle in seconds.
    pub phase: f32,
}

/// A posed arm: its base, its joints, the color of its eye and a tint of its paint (wear).
#[derive(Debug, Clone, Copy)]
pub struct Posed {
    pub base: Mat4,
    pub joints: Joints,
    pub eye: Rgb,
    pub tint: Rgb,
}

/// How an arm of the crowd draws.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lod {
    /// Out of the view.
    Hidden,
    /// Boxes, for arms far away.
    Far,
    /// The coarse copies of the links.
    Coarse,
    /// The parts of the hero arm.
    Detail,
}

/// The paint of the arm at station `index`: some arms are older and their paint is less
/// white.
pub fn tint(index: usize) -> Rgb {
    let age = hash(index as u32 * 13 + 5);
    let grey = 1.0 - 0.16 * age * age;
    Rgb::linear(grey, grey * (1.0 - 0.02 * age), grey * (1.0 - 0.06 * age))
}

impl Crowd {
    /// Adds the instanced nodes to `scene`, without instances.
    pub fn build(scene: &mut Scene, model: &Model, detail: &Detail, palette: &Palette) -> Self {
        let empty = || Arc::new(Instances::new([]));
        let add = |scene: &mut Scene, geometry: &Geometry, material: &Material| {
            scene.add(Node::instanced(geometry.clone(), material.clone(), empty()))
        };
        let shells = std::array::from_fn(|i| add(scene, &model.shells[i], &palette.white));
        let metals = std::array::from_fn(|i| add(scene, &model.metals[i], &palette.graphite));
        let far = std::array::from_fn(|i| add(scene, &model.far[i], &palette.white));
        let detail = detail
            .0
            .iter()
            .map(|(frame, geometry, material)| (*frame, add(scene, geometry, material)))
            .collect();
        let eyes = scene.add(
            Node::instanced(model.eye.clone(), Material::glow(Rgb::WHITE, 1.0), empty())
                .shadows(false, false),
        );
        Self {
            shells,
            metals,
            far,
            detail,
            eyes,
        }
    }

    /// Gives all parts of the crowd a rim of `color`: light from behind at their edges.
    pub fn rim(&self, scene: &mut Scene, color: Rgb) {
        let nodes = self.shells.iter().chain(&self.metals).chain(&self.far);
        for node in nodes.chain(self.detail.iter().map(|(_, n)| n)) {
            if let Some(m) = scene[*node].material_mut() {
                m.rim = color;
                m.rim_power = 4.0;
            }
        }
    }

    /// Sets the instances of all arms in `scene`. `lod(arm)` chooses how each arm draws.
    pub fn pose(&self, scene: &mut Scene, arms: &[Posed], lod: impl Fn(&Posed) -> Lod) {
        let drawn: Vec<([Mat4; 7], Lod, &Posed)> = arms
            .iter()
            .map(|a| (a, lod(a)))
            .filter(|(_, lod)| *lod != Lod::Hidden)
            .map(|(a, lod)| (frames(a.base, &a.joints), lod, a))
            .collect();
        let set = |scene: &mut Scene, node: NodeId, instances: Instances| {
            if let Object::Mesh(mesh) = &mut scene[node].object {
                mesh.instances = Some(Arc::new(instances));
            }
        };
        let link = |i: usize, lod: Lod| -> Instances {
            let arms = drawn.iter().filter(|f| f.1 == lod);
            Instances::new(arms.clone().map(|f| f.0[i])).colors(arms.map(|f| f.2.tint))
        };
        for i in 0..7 {
            set(scene, self.shells[i], link(i, Lod::Coarse));
            set(scene, self.metals[i], link(i, Lod::Coarse));
            set(scene, self.far[i], link(i, Lod::Far));
        }
        for (frame, node) in &self.detail {
            set(scene, *node, link(*frame, Lod::Detail));
        }
        set(
            scene,
            self.eyes,
            Instances::new(drawn.iter().map(|f| f.0[6])).colors(drawn.iter().map(|f| f.2.eye)),
        );
    }
}

/// The length of the work cycle in seconds. All arms run the same cycle, so the hall moves in
/// one rhythm.
pub const CYCLE: f32 = 4.0;

/// What an arm does at a moment of its work cycle.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Work {
    /// The grip point in the frame of the base, in meters.
    pub tip: Vec3,
    /// The direction of the tool in the frame of the base.
    pub tool: Vec3,
    /// Whether the torch burns: the arm welds.
    pub welding: bool,
}

/// The work of an arm at `t` seconds into its cycle: it reaches for the part on the conveyor,
/// welds a seam along it and goes back up.
pub fn work(t: f32) -> Work {
    let t = t.rem_euclid(CYCLE);
    let up = vec3(0.0, 1.55, 1.05);
    let seam_a = vec3(-0.45, 1.075, 1.85);
    let seam_b = vec3(0.45, 1.075, 1.85);
    let down = vec3(0.0, -1.0, 0.35);
    let (tip, tool) = if t < 1.0 {
        let k = smoothstep(0.0, 1.0, t);
        (up.lerp(seam_a, k), vec3(0.0, -0.4, 1.0).lerp(down, k))
    } else if t < 2.8 {
        (seam_a.lerp(seam_b, smoothstep(1.0, 2.8, t)), down)
    } else {
        let k = smoothstep(2.8, CYCLE, t);
        (seam_b.lerp(up, k), down.lerp(vec3(0.0, -0.4, 1.0), k))
    };
    Work {
        tip,
        tool: tool.normalize(),
        welding: (1.1..2.7).contains(&t),
    }
}

/// The joints of an arm that does `work`. `hint` picks the nearest of the wrist solutions.
pub fn joints(work: &Work, hint: &Joints) -> Joints {
    let side = Vec3::Y.cross(work.tool).normalize_or(Vec3::X);
    arm::solve(work.tip, arm::frame(side, work.tool), hint)
}

/// The joints of an arm that turns to `look` (in the frame of its base) and looks along it.
/// Its eye is `height` meters over the floor, 0.55 m out from its base in the direction of
/// the look.
pub fn looking(look: Vec3, height: f32, hint: &Joints) -> Joints {
    let out = look.with_y(0.0).normalize_or(Vec3::Z) * 0.55;
    let tool = arm::looking(look, 0.0);
    arm::solve(out + Vec3::Y * height + look.normalize() * 0.1, tool, hint)
}

/// The stations of `lines` production lines at `x` positions, each with arms on both sides
/// every `spacing` meters from `z0` to `z1`.
pub fn stations(lines: &[f32], z0: f32, z1: f32, spacing: f32) -> Vec<Station> {
    let mut out = Vec::new();
    let count = ((z0 - z1) / spacing) as usize;
    for (line, &x) in lines.iter().enumerate() {
        for i in 0..=count {
            let z = z0 - i as f32 * spacing;
            for side in [-1.0f32, 1.0] {
                // the arm stands beside the conveyor and faces it
                let yaw = if side < 0.0 { PI * 0.5 } else { -PI * 0.5 };
                let base = Mat4::from_rotation_translation(
                    Quat::from_rotation_y(yaw),
                    vec3(x + side * 2.2, 0.0, z),
                );
                // a wave runs down each line; the two sides work in counter time
                let seed = (line * 1000 + i * 2) as u32 + u32::from(side > 0.0);
                let phase = i as f32 * 0.35
                    + if side > 0.0 { CYCLE * 0.5 } else { 0.0 }
                    + line as f32 * 0.9
                    + 0.04 * hash(seed);
                out.push(Station { base, phase });
            }
        }
    }
    out
}

/// The default rest joints: the arm stands upright.
pub fn rest() -> Joints {
    Joints {
        q: [0.0, 0.2, 0.9, 0.0, 0.6, 0.0],
    }
}

/// The turn of an angle toward `target` by the fraction `k`, the short way around.
pub fn turn(from: f32, target: f32, k: f32) -> f32 {
    let d = (target - from + PI).rem_euclid(TAU) - PI;
    from + d * k
}
