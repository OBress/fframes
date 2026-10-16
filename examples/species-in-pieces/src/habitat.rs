//! Eighteen articulated vector habitats. Geometry is constructed once; leaves,
//! stems, water and light sample independent four-pose cycles during rendering.

use std::f32::consts::{PI, TAU};

use fframes::animation::{Animatable, AnimationRuntime, Easing};
use fframes::{Color, Svgr, svgr};

use crate::geometry::{Path, Piece};
use crate::supports::{FROG_LEAF_EDGE, IGUANA_BRANCH, TAMARIN_BRANCH};

type Point = (f32, f32);

const fn rgb(hex: u32) -> Color {
    Color::rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

fn tint(color: Color, amount: f32) -> Color {
    if amount < 1. {
        color.apply_progress(&rgb(0x0010_1b40), (1. - amount).clamp(0., 1.))
    } else {
        color.apply_progress(&rgb(0x00ff_edaa), (amount - 1.).clamp(0., 1.))
    }
}

fn noise(seed: usize) -> f32 {
    let mut n = (seed as u32)
        .wrapping_mul(747_796_405)
        .wrapping_add(2_891_336_453);
    n = ((n >> ((n >> 28) + 4)) ^ n).wrapping_mul(277_803_737);
    ((n >> 22) ^ n) as f32 / u32::MAX as f32
}

#[derive(Clone, Debug, Default)]
struct Mesh(Vec<Piece>);

impl Mesh {
    fn tri(&mut self, a: Point, b: Point, c: Point, color: Color) {
        self.0.push(Piece {
            path: Path { points: [a, b, c] },
            color,
        });
    }

    fn quad(&mut self, p: [Point; 4], color: Color) {
        self.tri(p[0], p[1], p[2], color);
        self.tri(p[0], p[2], p[3], tint(color, 0.88));
    }

    fn fan(&mut self, points: &[Point], color: Color, seed: usize) {
        let center = points.iter().fold((0., 0.), |a, p| {
            (
                a.0 + p.0 / points.len() as f32,
                a.1 + p.1 / points.len() as f32,
            )
        });
        for i in 0..points.len() {
            self.tri(
                points[i],
                points[(i + 1) % points.len()],
                center,
                tint(color, 0.72 + noise(seed + i) * 0.52),
            );
        }
    }

    fn ribbon(&mut self, points: &[Point], width: f32, color: Color) {
        // Adjacent segments share their edge vertices at the bend. Separate
        // segment normals used to leave triangular holes in branches and logs.
        let edges: Vec<_> = points
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let before = points[i.saturating_sub(1)];
                let after = points[(i + 1).min(points.len() - 1)];
                let delta = (after.0 - before.0, after.1 - before.1);
                let length = delta.0.hypot(delta.1).max(1.);
                let radius = width * (1. - 0.72 * i as f32 / points.len() as f32);
                let n = (-delta.1 / length * radius, delta.0 / length * radius);
                ((p.0 + n.0, p.1 + n.1), (p.0 - n.0, p.1 - n.1))
            })
            .collect();
        for (i, p) in points.windows(2).enumerate() {
            for side in 0..2 {
                self.quad(
                    [
                        p[0],
                        if side == 0 { edges[i].0 } else { edges[i].1 },
                        if side == 0 {
                            edges[i + 1].0
                        } else {
                            edges[i + 1].1
                        },
                        p[1],
                    ],
                    tint(color, if side == 0 { 1.16 } else { 0.7 }),
                );
            }
        }
    }

    fn leaf(&mut self, root: Point, tip: Point, width: f32, color: Color, lobed: bool) {
        let d = (tip.0 - root.0, tip.1 - root.1);
        let length = d.0.hypot(d.1).max(1.);
        let n = (-d.1 / length, d.0 / length);
        let center = |p: f32| {
            (
                root.0 + d.0 * p + n.0 * length * 0.075 * (p * PI).sin(),
                root.1 + d.1 * p + n.1 * length * 0.075 * (p * PI).sin(),
            )
        };
        let count = if width > 40. { 8 } else { 4 };
        for i in 0..count {
            let a = i as f32 / count as f32;
            let b = (i + 1) as f32 / count as f32;
            for side in [-1., 1.] {
                let edge = |p: f32, step: usize| {
                    let c = center(p);
                    let notch = if lobed && step % 2 == 1 { 0.48 } else { 1. };
                    let w = width * (p * PI).sin().max(0.).powf(0.7) * side * notch;
                    (c.0 + n.0 * w, c.1 + n.1 * w)
                };
                let pigment = tint(
                    color,
                    if side < 0. {
                        1. + a * 0.15
                    } else {
                        0.64 + a * 0.16
                    },
                );
                self.tri(center(a), edge(a, i), edge(b, i + 1), pigment);
                self.tri(center(a), edge(b, i + 1), center(b), tint(pigment, 1.12));
            }
        }
        self.tri(
            root,
            (root.0 + n.0 * 2.5, root.1 + n.1 * 2.5),
            tip,
            tint(color, 1.4),
        );
    }
}

#[derive(Clone, Copy, Debug)]
enum Motion {
    Still,
    Hinge {
        root: Point,
        reach: f32,
        phase: f32,
        angle: f32,
    },
    Water {
        level: f32,
        phase: f32,
        amplitude: f32,
    },
    Light {
        phase: f32,
    },
    Cloud {
        phase: f32,
    },
    Bubble {
        origin: Point,
        rise: f32,
        phase: f32,
        period: f32,
    },
}

#[derive(Debug)]
struct Part {
    mesh: Mesh,
    motion: Motion,
    opacity: f32,
}

impl Part {
    fn draw(&self, t: f32, ease: &AnimationRuntime) -> Svgr<'static> {
        let cycle = |phase: f32| {
            let time = (t + phase).rem_euclid(4.);
            let states = [0., 1., -0.6, 0.3, 0.];
            let k = time.floor() as usize;
            states[k] + (states[k + 1] - states[k]) * ease.solve(&time.fract())
        };
        let pose = match self.motion {
            Motion::Hinge { phase, .. }
            | Motion::Water { phase, .. }
            | Motion::Light { phase }
            | Motion::Cloud { phase } => cycle(phase),
            Motion::Still | Motion::Bubble { .. } => 0.,
        };
        let opacity = self.opacity
            * match self.motion {
                Motion::Light { .. } => 0.7 + pose * 0.3,
                Motion::Bubble { phase, period, .. } => {
                    ((t / period + phase).fract() * PI).sin().powf(0.7)
                }
                _ => 1.,
            };
        let geometry: Svgr<'static> = self.mesh.0.iter().map(|piece| {
            let path = piece.path.map(|(x, y)| match self.motion {
                Motion::Still => (x, y),
                Motion::Hinge { root, reach, angle, .. } => {
                    let dx = x - root.0;
                    let dy = y - root.1;
                    let weight = (dx.hypot(dy) / reach).clamp(0., 1.);
                    let radians = pose * angle * weight.to_radians();
                    let (s, c) = radians.sin_cos();
                    (root.0 + c * dx - s * dy, root.1 + s * dx + c * dy)
                }
                Motion::Water { level, amplitude, phase } => {
                    let w = ((y - level).max(0.) / 500.).min(1.);
                    (x + (t * 0.7 + y * 0.014 + phase).sin() * amplitude * w,
                     y + (t * 1.1 + x * 0.006 + phase).sin() * amplitude * 0.3 * w)
                }
                Motion::Light { .. } => (x + pose * (y / 1080.).powi(2) * 35., y),
                Motion::Cloud { phase } => (x + (t * 0.1 + phase).sin() * 22., y),
                Motion::Bubble { origin, rise, phase, period } => {
                    let life = (t / period + phase).fract();
                    let scale = 1. + life * 0.25;
                    (origin.0 + (x - origin.0) * scale + (t * 1.4 + phase * TAU).sin() * 12.,
                     origin.1 + (y - origin.1) * scale - life * rise)
                }
            });
            let color = if matches!(self.motion, Motion::Hinge { .. }) {
                tint(piece.color, 1. + pose * 0.045)
            } else { piece.color };
            svgr!(<path d={path} fill={color} stroke={color} stroke-width="0.65" stroke-linejoin="round" />)
        }).collect();
        svgr!(<g opacity={opacity}>{geometry}</g>)
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Framing {
    pub x: f32,
    pub y: f32,
    pub width: f32,
}

impl Framing {
    pub fn map(self, p: Point) -> Point {
        (
            self.x + (p.0 - 50.) * self.width / 100.,
            self.y + (p.1 - 50.) * self.width * 0.007,
        )
    }
}

pub(crate) const FRAMING: [Framing; 18] = [
    Framing {
        x: 1190.,
        y: 410.,
        width: 1100.,
    },
    Framing {
        x: 1150.,
        y: 430.,
        width: 1170.,
    },
    Framing {
        x: 1250.,
        y: 435.,
        width: 1080.,
    },
    Framing {
        x: 1250.,
        y: 440.,
        width: 1000.,
    },
    Framing {
        x: 1290.,
        y: 370.,
        width: 1080.,
    },
    Framing {
        x: 1190.,
        y: 420.,
        width: 1180.,
    },
    Framing {
        x: 1240.,
        y: 483.,
        width: 1100.,
    },
    Framing {
        x: 1220.,
        y: 430.,
        width: 1160.,
    },
    Framing {
        x: 1250.,
        y: 430.,
        width: 900.,
    },
    Framing {
        x: 1240.,
        y: 509.,
        width: 1050.,
    },
    Framing {
        x: 1230.,
        y: 460.,
        width: 1000.,
    },
    Framing {
        x: 1240.,
        y: 508.,
        width: 980.,
    },
    Framing {
        x: 1220.,
        y: 502.,
        width: 1100.,
    },
    Framing {
        x: 1280.,
        y: 486.,
        width: 980.,
    },
    Framing {
        x: 1260.,
        y: 440.,
        width: 980.,
    },
    Framing {
        x: 1330.,
        y: 471.,
        width: 1150.,
    },
    Framing {
        x: 1220.,
        y: 450.,
        width: 1130.,
    },
    Framing {
        x: 1200.,
        y: 486.,
        width: 1100.,
    },
];

#[derive(Debug)]
pub(crate) struct Habitat {
    index: usize,
    sky: [Color; 2],
    back: Vec<Part>,
    front: Vec<Part>,
    ease: AnimationRuntime,
    particles: u8,
}

impl Habitat {
    pub fn new(index: usize) -> Self {
        let palettes = [
            [0x00ea_d58c, 0x005c_af9b],
            [0x0039_d3e5, 0x0006_5a91],
            [0x00ff_d4ad, 0x0093_6f8f],
            [0x007a_ccc1, 0x0021_4d70],
            [0x00ff_af72, 0x00c9_6573],
            [0x0074_ecdf, 0x0008_7f9b],
            [0x00ff_c6a2, 0x00f2_b24d],
            [0x00b9_efb1, 0x002f_bdb1],
            [0x0080_c5ab, 0x0012_6f72],
            [0x009e_d5dd, 0x00e7_bb9b],
            [0x00f2_e6a5, 0x0043_b7a7],
            [0x002e_285e, 0x004a_9286],
            [0x00d4_cde1, 0x0073_9ba7],
            [0x00ff_d6b6, 0x008e_d5e2],
            [0x00d9_eaa8, 0x005f_bdbe],
            [0x00f2_d58f, 0x003c_8f7c],
            [0x0058_dfe6, 0x0007_5c92],
            [0x00ff_baa1, 0x00e8_a07c],
        ];
        let mut h = Self {
            index,
            sky: palettes[index].map(rgb),
            back: Vec::new(),
            front: Vec::new(),
            ease: AnimationRuntime::new(1., &Easing::CubicBezier(0.7, 0.3, 0., 1.)),
            particles: 0,
        };
        match index {
            0 => h.hornbill(),
            1 => h.vaquita(),
            2 => h.tamarin(),
            3 => h.frog(),
            4 => h.owlet(),
            5 => h.turtle(),
            6 => h.oryx(),
            7 => h.iguana(),
            8 => h.seahorse(),
            9 => h.armadillo(),
            10 => h.sloth(),
            11 => h.kakapo(),
            12 => h.echidna(),
            13 => h.penguin(),
            14 => h.damselfly(),
            15 => h.bear(),
            16 => h.parrotfish(),
            _ => h.camel(),
        }
        h.details();
        h
    }

    fn add(&mut self, mesh: Mesh, motion: Motion, opacity: f32, front: bool) {
        let p = Part {
            mesh,
            motion,
            opacity,
        };
        if front {
            self.front.push(p);
        } else {
            self.back.push(p);
        }
    }

    fn still(&mut self, mesh: Mesh) {
        self.add(mesh, Motion::Still, 1., false);
    }

    fn leaf(&mut self, root: Point, tip: Point, width: f32, color: u32, lobed: bool, front: bool) {
        let mut mesh = Mesh::default();
        mesh.leaf(root, tip, width, rgb(color), lobed);
        let phase = (root.0 * 0.007 + root.1 * 0.011).rem_euclid(4.);
        self.add(
            mesh,
            Motion::Hinge {
                root,
                reach: (tip.0 - root.0).hypot(tip.1 - root.1),
                phase,
                angle: 4.8,
            },
            1.,
            front,
        );
    }

    fn branch(&mut self, points: &[Point], width: f32, color: u32) {
        let mut mesh = Mesh::default();
        mesh.ribbon(points, width, rgb(color));
        self.still(mesh);
    }

    /// A perch described by its actual upper contact edge, rather than an
    /// approximate centerline. Feet and claws use this same visible plane.
    fn perch(&mut self, top: &[Point], thickness: f32, color: u32) {
        let mut mesh = Mesh::default();
        for p in top.windows(2) {
            mesh.quad(
                [
                    p[0],
                    p[1],
                    (p[1].0, p[1].1 + thickness),
                    (p[0].0, p[0].1 + thickness),
                ],
                rgb(color),
            );
            mesh.tri(
                p[0],
                (p[1].0, p[1].1 + thickness * 0.4),
                (p[0].0, p[0].1 + thickness * 0.35),
                tint(rgb(color), 1.17),
            );
        }
        self.still(mesh);
    }

    fn ridge(&mut self, points: &[Point], bottom: f32, color: u32) {
        let mut mesh = Mesh::default();
        for (i, p) in points.windows(2).enumerate() {
            let mid = (p[0].0.midpoint(p[1].0) + 20., bottom);
            mesh.tri(
                p[0],
                p[1],
                mid,
                tint(rgb(color), 0.78 + noise(i + self.index * 17) * 0.46),
            );
            mesh.tri(p[0], mid, (p[0].0, bottom), tint(rgb(color), 0.93));
            mesh.tri(p[1], (p[1].0, bottom), mid, tint(rgb(color), 0.73));
        }
        self.still(mesh);
    }

    fn ground(&mut self, top: f32, color: u32, seed: usize) {
        let mut mesh = Mesh::default();
        let point = |row: usize, col: usize| {
            let x = -100.
                + col as f32 * 180.
                + if col > 0 && col < 12 {
                    (noise(row * 17 + col + seed) - 0.5) * 100.
                } else {
                    0.
                };
            let y = top
                + row as f32 * 125.
                + if row > 0 {
                    (noise(row * 31 + col + seed) - 0.5) * 55.
                } else {
                    0.
                };
            (x, y)
        };
        for row in 0..4 {
            for col in 0..12 {
                mesh.quad(
                    [
                        point(row, col),
                        point(row, col + 1),
                        point(row + 1, col + 1),
                        point(row + 1, col),
                    ],
                    tint(rgb(color), 0.78 + noise(seed + row * 13 + col) * 0.37),
                );
            }
        }
        self.still(mesh);
    }

    fn rock(&mut self, x: f32, y: f32, width: f32, height: f32, color: u32, front: bool) {
        let mut mesh = Mesh::default();
        mesh.fan(
            &[
                (x - width * 0.5, y),
                (x - width * 0.42, y - height * 0.52),
                (x - width * 0.12, y - height),
                (x + width * 0.25, y - height * 0.83),
                (x + width * 0.52, y - height * 0.28),
                (x + width * 0.37, y + 14.),
            ],
            rgb(color),
            x as usize,
        );
        self.add(mesh, Motion::Still, 1., front);
    }

    fn sun(&mut self, center: Point, radius: f32, color: u32) {
        let p: Vec<_> = (0..12)
            .map(|i| {
                let a = i as f32 * TAU / 12.;
                (center.0 + a.cos() * radius, center.1 + a.sin() * radius)
            })
            .collect();
        let mut mesh = Mesh::default();
        mesh.fan(&p, rgb(color), 5);
        self.add(mesh, Motion::Still, 0.95, false);
    }

    fn clouds(&mut self, color: u32, base: f32) {
        for i in 0..5 {
            let x = i as f32 * 460. - 130.;
            let y = base + noise(i + self.index) * 100.;
            let mut mesh = Mesh::default();
            mesh.fan(
                &[
                    (x - 100., y + 20.),
                    (x + 80., y - 12.),
                    (x + 210., y - 5.),
                    (x + 310., y - 28.),
                    (x + 520., y + 24.),
                ],
                rgb(color),
                i + 4,
            );
            self.add(mesh, Motion::Cloud { phase: i as f32 }, 0.3, false);
        }
    }

    fn beams(&mut self, color: u32, underwater: bool) {
        for i in 0..7 {
            let x = 320. + i as f32 * 265.;
            let mut mesh = Mesh::default();
            mesh.tri((x, -40.), (x + 60., -40.), (x - 450., 1060.), rgb(color));
            self.add(
                mesh,
                Motion::Light {
                    phase: i as f32 * 0.43,
                },
                if underwater { 0.11 } else { 0.055 },
                false,
            );
        }
    }

    fn tree(&mut self, base: Point, top: Point, width: f32, color: u32, foliage: u32, seed: usize) {
        self.branch(
            &[
                base,
                (base.0 * 0.45 + top.0 * 0.55, top.1 + (base.1 - top.1) * 0.5),
                top,
            ],
            width,
            color,
        );
        let mut bark = Mesh::default();
        for k in 0..18 {
            let p = noise(seed * 23 + k);
            let y = top.1 + (base.1 - top.1) * p;
            let x = top.0 + (base.0 - top.0) * p;
            let shift = (noise(k + seed + 73) - 0.5) * width;
            bark.tri(
                (x + shift, y),
                (x + shift + width * 0.16, y + 20.),
                (x + shift + width * 0.05, y + 110. + noise(k) * 130.),
                tint(rgb(color), 0.6 + noise(k + 30) * 0.7),
            );
        }
        for side in [-1., 1.] {
            bark.tri(
                (base.0 + side * width * 0.6, base.1 - 220.),
                (base.0 + side * width * 2., base.1),
                (base.0, base.1),
                tint(rgb(color), 0.78),
            );
        }
        self.still(bark);
        for i in 0..5 {
            // The bear's head needs a clear silhouette below the canopy.
            if self.index == 15 && i == 4 {
                continue;
            }
            let y = top.1 + 40. + i as f32 * 70.;
            let x = top.0 + (base.0 - top.0) * (y - top.1) / (base.1 - top.1);
            let side = if i % 2 == 0 { -1. } else { 1. };
            let tip = (x + side * width * (3. + noise(seed + i) * 2.), y - 55.);
            self.branch(&[(x, y + 30.), tip], width * 0.21, color);
            for k in 0..4 {
                let root = (tip.0 + side * k as f32 * 12., tip.1);
                self.leaf(
                    root,
                    (
                        root.0 + side * (70. + k as f32 * 16.),
                        root.1 - 40. + k as f32 * 21.,
                    ),
                    23.,
                    foliage,
                    false,
                    false,
                );
            }
        }
    }

    fn distant_forest(&mut self, color: u32, height: f32, seed: usize) {
        let mut mesh = Mesh::default();
        for i in 0..26 {
            let x = i as f32 * 82. - 50.;
            let y = height + noise(i + seed) * 190.;
            let w = 85. + noise(i + 3 + seed) * 80.;
            let pigment = tint(rgb(color), 0.8 + noise(i + 5 + seed) * 0.3);
            mesh.tri(
                (x - 8., 1100.),
                (x + 14., 1100.),
                (x + 3., y - 25.),
                tint(pigment, 0.76),
            );
            mesh.fan(
                &[
                    (x - w, y + 55.),
                    (x - w * 0.7, y - 5.),
                    (x - w * 0.2, y - 55.),
                    (x + w * 0.48, y - 32.),
                    (x + w, y + 35.),
                    (x + w * 0.5, y + 90.),
                ],
                pigment,
                i + seed,
            );
        }
        // The tamarin's lower clearing is distant, mist-softened forest.
        // Keep those trunks quieter than the foreground animal and lettering.
        self.add(
            mesh,
            Motion::Still,
            if self.index == 2 { 0.38 } else { 1. },
            false,
        );
    }

    fn fern(&mut self, root: Point, length: f32, lean: f32, color: u32, front: bool) {
        let mut spine = Mesh::default();
        let center = |p: f32| (root.0 + lean * p * p, root.1 - length * p);
        let points: Vec<_> = (0..9).map(|i| center(i as f32 / 8.)).collect();
        spine.ribbon(&points, 3., tint(rgb(color), 1.3));
        self.add(spine, Motion::Still, 1., front);
        for i in 1..10 {
            let p = i as f32 / 11.;
            let start = center(p);
            let spread = length * 0.29 * (1. - p).powf(0.6);
            for side in [-1., 1.] {
                self.leaf(
                    start,
                    (start.0 + spread * side, start.1 - length * 0.095),
                    spread * 0.13,
                    color,
                    true,
                    front,
                );
            }
        }
    }

    fn grasses(&mut self, x: f32, y: f32, height: f32, color: u32, front: bool) {
        for i in 0..9 {
            let seed = x.max(0.) as usize + i;
            let base = (x + i as f32 * 7. - 28., y);
            self.leaf(
                base,
                (
                    x + (noise(seed) - 0.5) * height,
                    y - height * (0.4 + noise(seed + 20) * 0.6),
                ),
                height * 0.027,
                color,
                false,
                front,
            );
        }
    }

    fn tropical_edges(&mut self, colors: [u32; 3], seed: usize) {
        for i in 0..6 {
            let x = -160. + i as f32 * 110.;
            self.leaf(
                (x, -90.),
                (
                    x + 260. + noise(seed + i) * 130.,
                    160. + noise(seed + i + 1) * 250.,
                ),
                60. + noise(seed + i + 2) * 45.,
                colors[i % 3],
                i % 2 == 0,
                true,
            );
        }
        for i in 0..4 {
            self.leaf(
                (2020., 80. + i as f32 * 140.),
                (1600. - noise(seed + i) * 130., 120. + i as f32 * 175.),
                70.,
                colors[(i + 1) % 3],
                i % 2 == 0,
                true,
            );
        }
        self.fern((85., 1090.), 490., 160., colors[0], true);
        self.fern((1820., 1130.), 470., -130., colors[1], true);
    }

    fn flower(&mut self, root: Point, height: f32, color: u32, tropical: bool) {
        let top = (root.0 + 20., root.1 - height);
        self.branch(&[root, top], 4., 0x0040_8b50);
        for i in 0..5 {
            let p = (
                root.0 + 20. * i as f32 / 5.,
                root.1 - height * i as f32 / 5.,
            );
            let side = if i % 2 == 0 { 1. } else { -1. };
            if tropical {
                self.leaf(p, (p.0 + side * 90., p.1 - 50.), 26., color, false, true);
                self.leaf(
                    (p.0 + side * 20., p.1 - 10.),
                    (p.0 + side * 105., p.1 - 45.),
                    9.,
                    0x00f0_ad37,
                    false,
                    true,
                );
            } else {
                for petal in 0..5 {
                    let a = petal as f32 * TAU / 5.;
                    self.leaf(
                        p,
                        (p.0 + a.cos() * 32., p.1 + a.sin() * 32.),
                        14.,
                        color,
                        false,
                        true,
                    );
                }
            }
        }
    }

    fn water(&mut self, level: f32, bottom: f32, color: u32, seed: usize) {
        let mut mesh = Mesh::default();
        for row in 0..8 {
            for col in 0..11 {
                let y = level + row as f32 * (bottom - level) / 8.;
                let x = col as f32 * 210. - 100. + (row % 2) as f32 * 72.;
                let width = 70. + noise(seed + row * 13 + col) * 110.;
                mesh.tri(
                    (x, y),
                    (x + width, y - 5. - row as f32 * 2.),
                    (x + width * 0.7 + 65., y + 9. + row as f32 * 4.),
                    tint(rgb(color), 0.8 + noise(seed + col + row) * 0.5),
                );
            }
        }
        self.add(
            mesh,
            Motion::Water {
                level: level - 250.,
                phase: seed as f32,
                amplitude: 16.,
            },
            0.28,
            false,
        );
    }

    fn ocean(&mut self, grass: bool, reef: bool) {
        self.particles = 1;
        self.beams(0x00cc_fff0, true);
        let seabed: &[Point] = if self.index == 5 {
            &[
                (-80., 770.),
                (220., 680.),
                (480., 875.),
                (800., 960.),
                (1200., 960.),
                (1480., 875.),
                (1620., 740.),
                (2010., 820.),
            ]
        } else {
            &[
                (-80., 770.),
                (220., 680.),
                (480., 810.),
                (800., 780.),
                (1200., 850.),
                (1620., 740.),
                (2010., 820.),
            ]
        };
        self.ridge(seabed, 1100., if reef { 0x001c_547e } else { 0x0036_9c9d });
        self.ground(
            if self.index == 5 { 960. } else { 890. },
            if reef { 0x0043_7d9a } else { 0x009f_bda2 },
            11 + self.index,
        );
        self.water(
            -15.,
            if self.index == 1 { 60. } else { 155. },
            0x009a_efe2,
            self.index + 34,
        );
        if grass {
            for i in 0..9 {
                let x = i as f32 * 245.;
                let y = 970. + noise(i) * 100.;
                let height = if self.index == 5 && (400.0..=1550.0).contains(&x) {
                    // Keep the lower title and its map in open water beneath the flippers.
                    y - 950.
                } else if self.index == 8 && (650.0..=1300.0).contains(&x) {
                    y - 930.
                } else {
                    140. + noise(i + 6) * 170.
                };
                self.grasses(x, y, height, 0x0036_9b70, i % 2 == 0);
            }
            self.grasses(80., 1120., 650., 0x0018_756d, true);
            self.grasses(
                if self.index == 5 { 2040. } else { 1870. },
                1120.,
                550.,
                0x0023_7e63,
                true,
            );
        }
    }

    fn coral_branch(
        mesh: &mut Mesh,
        root: Point,
        angle: f32,
        length: f32,
        depth: usize,
        color: Color,
    ) {
        let tip = (root.0 + angle.cos() * length, root.1 + angle.sin() * length);
        mesh.ribbon(
            &[
                root,
                (root.0.midpoint(tip.0) + 7., root.1.midpoint(tip.1)),
                tip,
            ],
            length * 0.11,
            color,
        );
        if depth > 0 {
            for a in [-0.55, 0.18, 0.62] {
                Self::coral_branch(
                    mesh,
                    tip,
                    angle + a,
                    length * 0.63,
                    depth - 1,
                    tint(color, 1.08),
                );
            }
        } else {
            let radius = 4.5;
            let poly: Vec<_> = (0..6)
                .map(|i| {
                    let angle = i as f32 * TAU / 6.;
                    (tip.0 + angle.cos() * radius, tip.1 + angle.sin() * radius)
                })
                .collect();
            mesh.fan(&poly, tint(color, 1.2), tip.0.abs() as usize);
        }
    }

    fn coral(&mut self, root: Point, height: f32, color: u32, front: bool) {
        let mut mesh = Mesh::default();
        Self::coral_branch(&mut mesh, root, -PI * 0.5, height * 0.41, 3, rgb(color));
        self.add(
            mesh,
            Motion::Hinge {
                root,
                reach: height,
                phase: root.0 * 0.006,
                angle: 1.1,
            },
            1.,
            front,
        );
    }

    fn acacia(&mut self, x: f32, y: f32, size: f32) {
        self.branch(
            &[
                (x, y),
                (x + size * 0.05, y - size * 0.45),
                (x - size * 0.22, y - size * 0.75),
            ],
            size * 0.035,
            0x0053_424c,
        );
        for i in 0..5 {
            let cx = x + (i as f32 - 2.) * size * 0.18;
            let cy = y - size * (0.72 + noise(i) * 0.15);
            self.branch(
                &[(x + size * 0.05, y - size * 0.45), (cx, cy)],
                size * 0.015,
                0x0053_424c,
            );
            let mut mesh = Mesh::default();
            mesh.fan(
                &[
                    (cx - size * 0.24, cy),
                    (cx - size * 0.18, cy - size * 0.1),
                    (cx + size * 0.1, cy - size * 0.12),
                    (cx + size * 0.26, cy),
                    (cx + size * 0.15, cy + size * 0.07),
                ],
                rgb(0x0076_834b),
                i + 8,
            );
            self.add(
                mesh,
                Motion::Hinge {
                    root: (x, y),
                    reach: size,
                    phase: i as f32 * 0.3,
                    angle: 0.8,
                },
                1.,
                false,
            );
        }
    }

    fn vine(&mut self, start: Point, length: f32, color: u32, seed: usize) {
        let points: Vec<_> = (0..12)
            .map(|i| {
                let p = i as f32 / 11.;
                (
                    start.0 + (p * TAU + seed as f32).sin() * 38.,
                    start.1 + p * length,
                )
            })
            .collect();
        self.branch(&points, 3., 0x006e_8657);
        for (i, p) in points.iter().enumerate().skip(1) {
            let side = if i % 2 == 0 { -1. } else { 1. };
            self.leaf(*p, (p.0 + side * 75., p.1 + 42.), 25., color, false, true);
        }
    }

    fn spray(&mut self, root: Point, spread: f32, color: u32, front: bool) {
        for i in 0..7 {
            let angle = -PI * (0.1 + 0.8 * i as f32 / 6.);
            let tip = (root.0 + angle.cos() * spread, root.1 + angle.sin() * spread);
            self.leaf(root, tip, spread * 0.16, color, i % 2 == 0, front);
        }
    }

    fn pinnate_crown(&mut self, root: Point, length: f32, color: u32) {
        for j in 0..5 {
            let a = -PI * 0.85 + j as f32 * PI * 0.37;
            let tip = (root.0 + a.cos() * length, root.1 + a.sin() * length * 0.6);
            self.branch(&[root, tip], 3., color);
            for i in 1..10 {
                let p = i as f32 / 10.;
                let center = (root.0 + (tip.0 - root.0) * p, root.1 + (tip.1 - root.1) * p);
                let fan = length * 0.22 * (1. - p).sqrt();
                for side in [-1., 1.] {
                    self.leaf(
                        center,
                        (
                            center.0 + (a + side * 0.9).cos() * fan,
                            center.1 + (a + side * 0.9).sin() * fan + 30.,
                        ),
                        8.,
                        color,
                        false,
                        true,
                    );
                }
            }
        }
    }

    fn bubbles(&mut self, origin: Point, rise: f32, count: usize, seed: usize) {
        for i in 0..count {
            let center = (origin.0 + (noise(seed + i) - 0.5) * 90., origin.1);
            let radius = 5. + noise(seed + i + 31) * 6.;
            let mut mesh = Mesh::default();
            for side in 0..10 {
                let a = side as f32 * TAU / 10.;
                let b = (side + 1) as f32 * TAU / 10.;
                let p =
                    |angle: f32, r: f32| (center.0 + angle.cos() * r, center.1 + angle.sin() * r);
                mesh.quad(
                    [
                        p(a, radius),
                        p(b, radius),
                        p(b, radius * 0.76),
                        p(a, radius * 0.76),
                    ],
                    rgb(if side >= 5 { 0x00d8_fff1 } else { 0x008f_d8cd }),
                );
            }
            self.add(
                mesh,
                Motion::Bubble {
                    origin: center,
                    rise,
                    phase: i as f32 / count as f32,
                    period: 7. + noise(seed + i + 51) * 2.,
                },
                0.7,
                false,
            );
        }
    }

    fn stream(&mut self) {
        let mut mesh = Mesh::default();
        for i in 0..18 {
            let y = 660. + i as f32 * 25.;
            let x = 390. + (y - 660.) * 0.9;
            let width = 45. + (y - 660.) * 0.17;
            mesh.quad(
                [
                    (x - width, y),
                    (x + width, y - 5.),
                    (x + width + 28., y + 5.),
                    (x - width + 30., y + 12.),
                ],
                rgb(0x008c_e2d6),
            );
        }
        self.add(
            mesh,
            Motion::Water {
                level: 500.,
                phase: 2.,
                amplitude: 11.,
            },
            0.7,
            false,
        );
    }

    fn sea_fan(&mut self, root: Point, height: f32, color: u32) {
        let mut mesh = Mesh::default();
        for i in 0..15 {
            let a = -PI * (0.12 + i as f32 * 0.76 / 14.);
            let length = height * (0.8 + noise(i) * 0.2);
            let tip = (root.0 + a.cos() * length * 0.65, root.1 + a.sin() * length);
            mesh.ribbon(
                &[
                    root,
                    (root.0 + (tip.0 - root.0) * 0.45, root.1 - length * 0.4),
                    tip,
                ],
                3.,
                rgb(color),
            );
            for j in 1..5 {
                let p = j as f32 / 6.;
                let c = (root.0 + (tip.0 - root.0) * p, root.1 + (tip.1 - root.1) * p);
                mesh.ribbon(&[c, (c.0 + 22., c.1 - 30.)], 1.5, tint(rgb(color), 1.2));
            }
        }
        self.add(
            mesh,
            Motion::Hinge {
                root,
                reach: height,
                phase: root.0 * 0.003,
                angle: 2.4,
            },
            1.,
            true,
        );
    }

    fn details(&mut self) {
        match self.index {
            0 => {
                self.vine((470., -30.), 590., 0x0052_8a50, 2);
                self.vine((1800., -80.), 370., 0x0082_a449, 5);
                self.spray((270., 850.), 260., 0x0026_7963, true);
                self.flower((160., 680.), 180., 0x00cc_7c9e, false);
                self.spray((1730., 730.), 200., 0x0049_9a66, false);
            }
            1 => {
                let mut floor = Mesh::default();
                for i in 0..14 {
                    let x = i as f32 * 140.;
                    floor.tri(
                        (x, 990.),
                        (x + 90., 975.),
                        (x + 170., 1000.),
                        rgb(0x00d4_dfb9),
                    );
                }
                self.add(
                    floor,
                    Motion::Water {
                        level: 600.,
                        phase: 1.,
                        amplitude: 12.,
                    },
                    0.3,
                    false,
                );
            }
            2 => {
                self.vine((600., -40.), 540., 0x007c_a74f, 4);
                self.vine((1730., -100.), 400., 0x008f_9a4e, 1);
                self.spray((260., 720.), 230., 0x007c_9b4f, true);
                self.spray((1760., 830.), 160., 0x0057_934a, true);
                self.flower((260., 720.), 240., 0x00e8_7161, true);
            }
            3 => {
                self.vine((640., -80.), 360., 0x002e_986a, 3);
                self.spray((550., 1070.), 200., 0x0042_9f68, true);
                self.fern((1710., 1130.), 580., -190., 0x0024_7b68, true);
            }
            4 => {
                self.vine((1770., -100.), 350., 0x00c7_8d52, 3);
                self.spray((70., 890.), 260., 0x009f_7159, true);
            }
            5 => {
                self.grasses(630., 1050., 165., 0x006b_ac70, false);
                self.grasses(1380., 1140., 140., 0x0047_9d77, true);
                self.spray((230., 1050.), 140., 0x0032_9e86, true);
            }
            6 => {
                self.grasses(240., 1110., 240., 0x00d5_ad5c, true);
                self.grasses(1770., 1090., 240., 0x00b8_9059, true);
            }
            7 => {
                self.pinnate_crown((70., 190.), 500., 0x0049_8b4e);
                self.vine((1660., -40.), 310., 0x0091_b64f, 5);
                self.spray((1700., 960.), 260., 0x0030_896a, true);
            }
            8 => {
                self.grasses(490., 1100., 490., 0x0043_8f6c, false);
                self.grasses(1910., 1070., 870., 0x0062_a67a, true);
            }
            9 => {
                self.spray((120., 1130.), 185., 0x0091_9c6a, true);
                self.grasses(1750., 1110., 240., 0x00c9_ac78, true);
            }
            10 => {
                self.vine((1670., -60.), 430., 0x0063_a35b, 2);
                self.spray((1710., 940.), 210., 0x0038_9d77, true);
                self.spray((330., 530.), 170., 0x0064_9f60, true);
            }
            11 => {
                self.fern((1660., 1060.), 390., 140., 0x005a_a387, true);
                self.spray((490., 1070.), 170., 0x005a_a77c, true);
            }
            12 => {
                self.pinnate_crown((140., 220.), 450., 0x0058_8f74);
                self.pinnate_crown((1810., 250.), 370., 0x0054_8f7f);
                self.vine((560., -40.), 380., 0x0080_9767, 4);
                self.spray((660., 1090.), 170., 0x0064_8b68, true);
            }
            13 => {
                self.spray((300., 1090.), 140., 0x008b_9c68, true);
            }
            14 => {
                self.vine((500., -80.), 370., 0x008e_aa5c, 4);
                self.flower((450., 1090.), 380., 0x00de_9bb1, false);
                for i in 0..4 {
                    let x = 1650. + i as f32 * 52.;
                    self.branch(
                        &[(x, 1090.), (x - 30., 540. + i as f32 * 30.)],
                        3.,
                        0x007c_9054,
                    );
                    self.leaf(
                        (x - 30., 620. + i as f32 * 30.),
                        (x - 33., 530. + i as f32 * 30.),
                        9.,
                        0x0092_7355,
                        false,
                        true,
                    );
                }
            }
            15 => {
                self.vine((1740., -80.), 450., 0x0069_9252, 3);
                self.spray((230., 1050.), 240., 0x003c_8a64, true);
                self.spray((1620., 1040.), 200., 0x0041_7e62, true);
                self.fern((100., 660.), 240., 70., 0x006a_9a63, true);
            }
            16 => {
                self.sea_fan((200., 870.), 470., 0x00c1_9ac7);
                self.sea_fan((1760., 1100.), 330., 0x0098_7db6);
                self.coral((460., 1120.), 330., 0x00f1_ae7c, true);
                self.coral((1640., 1160.), 260., 0x00d4_91b4, true);
                self.spray((650., 1130.), 220., 0x0050_9b91, true);
            }
            _ => {
                self.rock(550., 1080., 300., 180., 0x00b8_8162, true);
                self.grasses(1620., 1100., 250., 0x00bb_a16e, true);
            }
        }
    }

    fn hornbill(&mut self) {
        self.sun((1080., 220.), 34., 0x00ff_f3b1);
        self.ridge(
            &[
                (-20., 560.),
                (230., 380.),
                (490., 510.),
                (730., 330.),
                (1040., 490.),
                (1420., 360.),
                (1940., 480.),
            ],
            1100.,
            0x007c_aba4,
        );
        self.distant_forest(0x0044_8e86, 630., 22);
        self.distant_forest(0x0020_6f65, 810., 7);
        self.tree(
            (120., 1190.),
            (230., -180.),
            115.,
            0x0016_4c53,
            0x002b_8870,
            3,
        );
        self.perch(
            &[
                (40., 930.),
                (680., 730.),
                (1110., 600.),
                (1350., 600.),
                (1630., 440.),
                (2030., 400.),
            ],
            40.,
            0x0056_654c,
        );
        self.beams(0x00ff_f0b0, false);
        self.tropical_edges([0x0013_796d, 0x003e_9c65, 0x0093_b84b], 10);
        self.particles = 2;
    }

    fn vaquita(&mut self) {
        self.ocean(false, false);
        self.rock(80., 1000., 520., 510., 0x0035_6f92, true);
        self.rock(350., 1020., 340., 290., 0x003b_8f9e, false);
        self.water(10., 55., 0x00e5_fff3, 11);
    }

    fn tamarin(&mut self) {
        self.sun((1400., 150.), 80., 0x00ff_e8aa);
        self.distant_forest(0x00ae_939c, 450., 3);
        for i in 0..9 {
            // A break in the near trunks opens the lower-left forest clearing.
            if matches!(i, 3 | 4 | 6) {
                continue;
            }
            let x = i as f32 * 245. - 60.;
            self.tree(
                (x, 1100.),
                (x - 120., -90.),
                25.,
                0x006e_6c89,
                0x0088_9b73,
                i,
            );
        }
        self.perch(TAMARIN_BRANCH, 48., 0x009c_663e);
        self.tree(
            (180., 1120.),
            (280., -80.),
            75.,
            0x0049_394e,
            0x007f_9f4c,
            3,
        );
        self.tropical_edges([0x002c_665d, 0x005d_963f, 0x008a_a449], 40);
        self.flower((300., 1040.), 440., 0x00e7_513b, true);
        self.beams(0x00ff_d48d, false);
        self.particles = 2;
    }

    fn frog(&mut self) {
        self.distant_forest(0x0041_9a89, 340., 15);
        for x in [100., 300., 1740.] {
            self.tree(
                (x, 1000.),
                (x + 60., -80.),
                35.,
                0x0024_616d,
                0x0049_a169,
                2,
            );
        }
        self.ground(720., 0x0035_4b61, 12);
        self.ridge(
            &[(200., 600.), (450., 650.), (710., 720.), (960., 1100.)],
            1140.,
            0x0023_9dba,
        );
        self.stream();
        self.rock(500., 940., 420., 300., 0x0061_507e, false);
        self.rock(1700., 960., 480., 270., 0x004a_506a, false);
        // The support leaf is fixed at the feet; only its outer blades turn.
        let mut platform = Mesh::default();
        platform.fan(
            &[
                FROG_LEAF_EDGE[0],
                FROG_LEAF_EDGE[1],
                FROG_LEAF_EDGE[2],
                FROG_LEAF_EDGE[3],
                (1420., 746.),
                (1190., 693.),
                (1010., 598.),
            ],
            rgb(0x003b_9a68),
            31,
        );
        platform.ribbon(
            &[(1560., 698.), (1370., 706.), (1140., 646.), (880., 535.)],
            3.,
            rgb(0x0085_bc79),
        );
        self.branch(
            &[(1560., 698.), (1670., 800.), (1750., 1080.)],
            12.,
            0x003b_7c57,
        );
        self.still(platform);
        self.tropical_edges([0x0012_6c63, 0x0018_9b75, 0x0063_ab55], 4);
        self.leaf((-80., 950.), (250., 410.), 100., 0x0019_7864, true, true);
        self.flower((85., 1080.), 650., 0x00e6_4446, true);
        self.particles = 3;
    }

    fn owlet(&mut self) {
        self.sun((720., 280.), 70., 0x00ff_df9d);
        self.ridge(
            &[
                (-40., 570.),
                (350., 390.),
                (620., 540.),
                (850., 450.),
                (1120., 590.),
                (1600., 460.),
                (1990., 630.),
            ],
            1100.,
            0x00af_718c,
        );
        self.distant_forest(0x009c_6275, 670., 20);
        self.distant_forest(0x008e_5362, 820., 8);
        self.tree(
            (1850., 1140.),
            (1720., -150.),
            100.,
            0x0053_374f,
            0x00c1_834a,
            8,
        );
        self.perch(
            &[
                (770., 330.),
                (1100., 490.),
                (1380., 490.),
                (1640., 700.),
                (1940., 850.),
            ],
            35.,
            0x0069_4249,
        );
        self.tree(
            (-40., 1100.),
            (100., 320.),
            45.,
            0x0076_5059,
            0x00d4_9b58,
            5,
        );
        self.grasses(430., 1100., 280., 0x00b2_7c4e, true);
        for i in 0..6 {
            self.leaf(
                (1920., i as f32 * 100.),
                (1570. - i as f32 * 25., 140. + i as f32 * 95.),
                45.,
                0x00b7_8049,
                false,
                true,
            );
        }
        self.particles = 4;
    }

    fn turtle(&mut self) {
        self.ocean(true, false);
        self.rock(230., 1040., 410., 190., 0x0066_7b9a, false);
        self.rock(1740., 1060., 350., 220., 0x0055_798f, true);
        self.grasses(200., 1080., 470., 0x0046_a267, true);
        self.grasses(1870., 1120., 290., 0x0073_aa67, true);
    }

    fn oryx(&mut self) {
        self.sun((1780., 180.), 66., 0x00ff_e7a0);
        self.clouds(0x00ff_e2b0, 250.);
        self.ridge(
            &[
                (-40., 540.),
                (190., 480.),
                (340., 500.),
                (500., 450.),
                (930., 470.),
                (1150., 520.),
                (1590., 450.),
                (1980., 500.),
            ],
            800.,
            0x00ad_8799,
        );
        self.ground(745., 0x00c5_8c48, 16);
        self.acacia(180., 790., 400.);
        self.acacia(1750., 750., 190.);
        for i in 0..18 {
            let x = i as f32 * 120.;
            let map_clearing = (420. ..960.).contains(&x);
            self.grasses(
                x,
                if map_clearing {
                    1090.
                } else {
                    820. + noise(i) * 200.
                },
                if map_clearing {
                    60.
                } else {
                    60. + noise(i + 3) * 100.
                },
                0x00c7_a65f,
                i % 3 == 0,
            );
        }
        self.rock(100., 1090., 450., 200., 0x00a9_6a50, true);
        self.particles = 5;
    }

    fn iguana(&mut self) {
        self.ridge(
            &[
                (-30., 530.),
                (180., 370.),
                (450., 420.),
                (700., 520.),
                (880., 390.),
                (1040., 520.),
                (1990., 560.),
            ],
            660.,
            0x006c_ae9f,
        );
        self.ground(610., 0x0013_8fbb, 11);
        self.water(630., 1080., 0x008e_e3df, 7);
        self.distant_forest(0x003d_9468, 930., 3);
        self.tree(
            (200., 1100.),
            (60., -110.),
            90.,
            0x0063_7c55,
            0x0085_b34a,
            4,
        );
        self.perch(IGUANA_BRANCH, 50., 0x009b_9d70);
        self.tropical_edges([0x003c_9c62, 0x0069_b54f, 0x0026_7a64], 14);
        self.flower((300., 1030.), 380., 0x00ee_626e, false);
        self.particles = 2;
    }

    fn seahorse(&mut self) {
        self.ocean(true, false);
        self.grasses(450., 1070., 820., 0x002b_9170, true);
        self.grasses(1740., 1080., 760., 0x004f_a374, true);
        self.branch(
            &[
                (1510., 1000.),
                (1493., 780.),
                (1493., 518.),
                (1480., 360.),
                (1440., 280.),
            ],
            8.,
            0x0056_8b4e,
        );
        self.leaf((1470., 650.), (1600., 340.), 17., 0x006c_a25b, false, false);
        self.rock(120., 1090., 350., 250., 0x0052_7a78, true);
        self.bubbles((940., 1020.), 990., 6, 81);
        self.bubbles((1630., 1050.), 1030., 5, 97);
        self.particles = 0;
    }

    fn armadillo(&mut self) {
        self.clouds(0x00e8_f0d1, 100.);
        self.ridge(
            &[
                (-20., 630.),
                (230., 500.),
                (450., 580.),
                (800., 470.),
                (1060., 580.),
                (1480., 490.),
                (1980., 570.),
            ],
            900.,
            0x00a1_8aab,
        );
        self.ground(765., 0x00c7_7b5d, 73);
        self.tree(
            (160., 920.),
            (110., 120.),
            22.,
            0x0071_536b,
            0x009a_9b70,
            25,
        );
        self.tree(
            (1920., 960.),
            (1820., 360.),
            24.,
            0x007c_5b60,
            0x009b_9b68,
            3,
        );
        for (x, height) in [(140., 470.), (260., 340.), (1660., 270.)] {
            self.branch(&[(x, 850.), (x - 4., 850. - height)], 22., 0x0075_895c);
            self.branch(
                &[(x, 690.), (x - 70., 650.), (x - 70., 540.)],
                13.,
                0x0073_865b,
            );
            self.branch(
                &[(x, 740.), (x + 65., 690.), (x + 65., 590.)],
                12.,
                0x007a_925a,
            );
        }
        for i in 0..12 {
            let x = i as f32 * 180.;
            let map_clearing = (1120. ..1680.).contains(&(1920. - x));
            self.grasses(
                x,
                if map_clearing {
                    1090.
                } else {
                    860. + noise(i) * 240.
                },
                if map_clearing { 55. } else { 95. },
                0x00c2_a270,
                i % 2 == 0,
            );
        }
        self.rock(240., 1090., 540., 250., 0x00b6_6e62, true);
        self.particles = 5;
    }

    fn sloth(&mut self) {
        self.distant_forest(0x0069_ab7e, 500., 7);
        self.ground(710., 0x0027_a6a1, 12);
        self.water(690., 1080., 0x0094_e3c5, 21);
        self.tree((180., 990.), (70., -80.), 70., 0x0045_5b59, 0x005e_9f52, 2);
        self.ridge(
            &[
                (-80., 950.),
                (130., 910.),
                (350., 965.),
                (500., 1050.),
                (600., 1130.),
            ],
            1190.,
            0x0058_6853,
        );
        for (points, width) in [
            (
                [(145., 570.), (215., 655.), (285., 800.), (330., 995.)],
                24.,
            ),
            ([(155., 690.), (95., 765.), (50., 875.), (20., 990.)], 21.),
            (
                [(170., 735.), (290., 790.), (370., 900.), (430., 1060.)],
                19.,
            ),
            (
                [(140., 820.), (195., 870.), (230., 945.), (245., 1040.)],
                15.,
            ),
        ] {
            self.branch(&points, width, 0x005c_6d60);
        }
        self.branch(
            &[
                (155., 750.),
                (680., 520.),
                (1030., 330.),
                (1252., 210.),
                (1500., 100.),
                (1990., -40.),
            ],
            42.,
            0x0072_6b54,
        );
        self.tropical_edges([0x002f_8a68, 0x005d_9e50, 0x0021_8c79], 25);

        self.particles = 2;
    }

    fn kakapo(&mut self) {
        // An intimate foraging floor, not a skyline of repeated tree crowns.
        // The clear moss path is shared with both feet and the berry sapling.
        for (x, top, width) in [
            (60., -100., 90.),
            (410., -80., 35.),
            (1710., -120., 48.),
            (1940., -90., 105.),
        ] {
            self.branch(
                &[(x, 900.), (x - 40., 420.), (x + 20., top)],
                width,
                0x0038_425a,
            );
        }
        self.ground(770., 0x0045_8874, 2);
        for (root, length, lean) in [
            ((170., 790.), 610., 90.),
            ((490., 800.), 390., -40.),
            ((1750., 800.), 590., -120.),
        ] {
            self.fern(root, length, lean, 0x0049_8b84, false);
        }
        self.beams(0x00c4_dbb1, false);
        for i in 0..13 {
            let x = 160. + i as f32 * 137.;
            let y = 840. + noise(i + 82) * 150.;
            self.leaf((x, y), (x - 45., y - 8.), 4., 0x006a_a183, false, false);
        }
        for (x, y, length, lean) in [(170., 1110., 780., 150.), (1870., 1140., 780., -140.)] {
            self.fern((x, y), length, lean, 0x0063_b598, true);
        }
        self.particles = 6;
    }

    fn echidna(&mut self) {
        self.ridge(
            &[
                (-20., 570.),
                (240., 400.),
                (450., 520.),
                (700., 330.),
                (1070., 520.),
                (1380., 350.),
                (1980., 590.),
            ],
            1100.,
            0x0087_97ac,
        );
        self.distant_forest(0x0067_8f9c, 640., 13);
        self.clouds(0x00d6_d2dc, 400.);
        self.ground(765., 0x005d_7761, 14);
        for x in [130., 340., 1800.] {
            self.tree(
                (x, 1100.),
                (x + 30., 120.),
                33.,
                0x0045_646a,
                0x007b_ab6e,
                3,
            );
        }
        self.fern((180., 1090.), 650., 80., 0x0033_907c, true);
        self.fern((1820., 1120.), 660., -120., 0x005a_9e6b, true);
        self.flower((480., 1080.), 248., 0x00c6_8b58, true);
        self.rock(850., 1110., 540., 220., 0x006b_686f, true);
        self.particles = 3;
    }

    fn penguin(&mut self) {
        self.clouds(0x00ff_efd6, 130.);
        self.ridge(
            &[
                (-20., 420.),
                (280., 300.),
                (600., 420.),
                (870., 330.),
                (1140., 410.),
                (1600., 290.),
                (1980., 420.),
            ],
            560.,
            0x008d_a4b7,
        );
        self.ground(450., 0x0022_8aa9, 28);
        self.water(450., 770., 0x008e_d9dc, 35);
        // Surf belongs behind the dry granite and its shoreline silhouette.
        let mut foam = Mesh::default();
        for i in 0..12 {
            let x = i as f32 * 170. - 30.;
            foam.quad(
                [
                    (x, 610.),
                    (x + 120., 590.),
                    (x + 210., 615.),
                    (x + 60., 630.),
                ],
                rgb(0x00e5_eee0),
            );
        }
        self.add(
            foam,
            Motion::Water {
                level: 100.,
                phase: 2.,
                amplitude: 20.,
            },
            0.8,
            false,
        );
        self.ridge(
            &[
                (-40., 1000.),
                (380., 880.),
                (650., 820.),
                (1000., 760.),
                (1550., 760.),
                (1980., 790.),
            ],
            1150.,
            0x00c6_b6a1,
        );
        self.rock(60., 990., 400., 320., 0x00d3_c3ae, false);
        self.rock(370., 1060., 430., 280., 0x00cb_a88f, true);

        for i in 0..5 {
            self.leaf(
                (1690., 1050.),
                (1580. + i as f32 * 60., 860. - noise(i) * 110.),
                24.,
                0x007e_9c71,
                false,
                true,
            );
        }
        self.flower((1800., 1110.), 250., 0x00da_716c, false);
        self.particles = 0;
    }

    fn damselfly(&mut self) {
        // A low, distant bank leaves the translucent wings against open sky.
        self.distant_forest(0x00a6_ba8e, 540., 11);
        self.ground(600., 0x003c_a7a7, 11);
        self.water(590., 1080., 0x00c2_e4b9, 23);
        self.rock(220., 960., 470., 390., 0x00d3_cfb2, false);
        self.rock(660., 860., 360., 190., 0x00c6_c6a7, false);
        for (x, height) in [
            (1630., 350.),
            (1720., 440.),
            (1810., 550.),
            (1900., 620.),
            (1990., 680.),
        ] {
            self.grasses(x, 1090., height, 0x0077_9c63, true);
        }
        self.tree((100., 1100.), (50., -50.), 42., 0x006b_8062, 0x0098_b55d, 3);
        self.flower((250., 1010.), 520., 0x00dc_82a2, false);
        self.beams(0x00ff_e8a2, false);
        self.particles = 2;
    }

    fn bear(&mut self) {
        self.distant_forest(0x0091_ad79, 330., 5);
        self.distant_forest(0x0056_8b6b, 560., 30);
        self.ground(765., 0x0060_755c, 2);
        self.tree(
            (1840., 1160.),
            (1640., -130.),
            90.,
            0x0045_4552,
            0x0050_8866,
            8,
        );
        self.beams(0x00ff_e49d, false);
        // This scene is reflected. Shorter upper leaves leave a broad opening
        // on screen right, and the left foliage stays away from the muzzle.
        let colors = [0x0025_685c, 0x005a_9862, 0x003b_8072];
        for i in 0..6 {
            let x = -160. + i as f32 * 100.;
            self.leaf(
                (x, -90.),
                (
                    (x + 150. + noise(32 + i) * 90.).min(480.),
                    110. + noise(33 + i) * 100.,
                ),
                45. + noise(34 + i) * 20.,
                colors[i % 3],
                i % 2 == 0,
                true,
            );
        }
        for i in 0..4 {
            self.leaf(
                (2020., 80. + i as f32 * 140.),
                (1660. - noise(32 + i) * 100., 120. + i as f32 * 175.),
                60.,
                colors[(i + 1) % 3],
                i % 2 == 0,
                true,
            );
        }
        self.fern((85., 1090.), 490., 160., colors[0], true);
        self.fern((1820., 1130.), 470., -130., colors[1], true);
        self.fern((240., 1130.), 650., -40., 0x0061_995e, true);
        self.particles = 2;
    }

    fn parrotfish(&mut self) {
        self.ocean(false, true);
        self.rock(60., 1060., 630., 630., 0x0023_4d7d, false);
        for (x, y, height, color) in [
            (120., 820., 490., 0x00f9_a56e),
            (370., 1020., 560., 0x00ce_83b0),
            (610., 1150., 430., 0x00f5_9d70),
            (1840., 1100., 420., 0x00a8_87c2),
            (1560., 1150., 280., 0x00df_8fab),
        ] {
            self.coral((x, y), height, color, true);
        }
        self.grasses(220., 1190., 550., 0x005c_b49b, true);
    }

    fn camel(&mut self) {
        self.clouds(0x00ff_dbb7, 90.);
        self.ridge(
            &[
                (-60., 470.),
                (170., 350.),
                (350., 400.),
                (590., 250.),
                (830., 390.),
                (1080., 290.),
                (1360., 400.),
                (1680., 270.),
                (1990., 410.),
            ],
            850.,
            0x00a2_8da5,
        );
        self.ridge(
            &[
                (-60., 720.),
                (180., 650.),
                (420., 670.),
                (680., 540.),
                (850., 700.),
                (1210., 610.),
                (1530., 660.),
                (1790., 530.),
                (2000., 650.),
            ],
            950.,
            0x00d0_8066,
        );
        self.ground(760., 0x00be_845d, 29);
        self.rock(1800., 820., 560., 430., 0x00b2_735c, false);
        self.rock(220., 1050., 570., 300., 0x00a1_6a58, true);
        for i in 0..9 {
            self.grasses(
                230. + i as f32 * 205.,
                880. + noise(i) * 200.,
                105.,
                0x00c6_ad77,
                true,
            );
        }
        self.particles = 5;
    }

    pub fn background(&self, t: f32) -> Svgr<'static> {
        let id = format!("sky-{}", self.index);
        let url = format!("url(#{id})");
        let geometry: Svgr<'static> = self
            .back
            .iter()
            .map(|part| part.draw(t, &self.ease))
            .collect();
        svgr!(<g>
            <defs><linearGradient id={id} x1="0" y1="0" x2="0" y2="1">
                <stop offset="0" stop-color={self.sky[0]} /><stop offset="1" stop-color={self.sky[1]} />
            </linearGradient></defs>
            <rect width="1920" height="1080" fill={url} />{geometry}
        </g>)
    }

    pub fn foreground(&self, t: f32) -> Svgr<'static> {
        let plants: Svgr<'static> = self
            .front
            .iter()
            .map(|part| part.draw(t, &self.ease))
            .collect();
        let count = match self.particles {
            0 => 0,
            3 => 16,
            _ => 7,
        };
        let particles: Svgr<'static> = (0..count)
            .map(|i| {
                let life =
                    (t * if self.particles == 3 { 0.5 } else { 0.12 } + noise(i + 30)).fract();
                let base_x = noise(i * 7 + self.index * 19) * 1920.;
                let y = if self.particles == 1 || self.particles == 6 {
                    1080. * (1. - life)
                } else {
                    life * 1080.
                };
                let x = base_x
                    + (t * 0.45 + i as f32).sin() * if self.particles == 3 { 4. } else { 30. };
                let size = if self.particles == 4 {
                    5.
                } else {
                    1. + noise(i + 9) * 1.5
                };
                let color = match self.particles {
                    1 | 3 | 7 => rgb(0x00b9_f4e6),
                    4 | 5 => rgb(0x00ed_c587),
                    6 => rgb(0x00d5_ed97),
                    _ => rgb(0x00f3_df9a),
                };
                let path = if self.particles == 3 {
                    Path {
                        points: [(x, y - 7.), (x - 0.5, y + 4.), (x + 0.6, y)],
                    }
                } else {
                    Path {
                        points: [(x, y - size), (x - size * 0.7, y + size), (x + size, y)],
                    }
                };
                let opacity = (life * PI).sin() * if self.particles == 3 { 0.45 } else { 0.65 };
                svgr!(<path d={path} fill={color} opacity={opacity} />)
            })
            .collect();
        svgr!(<g>{plants}{particles}</g>)
    }
}
