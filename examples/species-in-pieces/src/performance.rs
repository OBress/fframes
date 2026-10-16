//! Connected animal rigs and habitat interactions. The thirty numeric paths
//! retain the source's staggered morph; shared vertices move together during
//! actions, and planted toes are independent of the torso's motion.

use std::f32::consts::{PI, TAU};

use fframes::animation::{Animatable, AnimationRuntime, Easing};
use fframes::{Color, Svgr, svgr};

use crate::armadillo_ball::BALL;
use crate::choreography::{SECONDS_PER_SPECIES, species_at};
use crate::composition::COMPOSITIONS;
use crate::geometry::{Path, Piece};
use crate::habitat::{FRAMING, Framing};
use crate::source_data::{CELLS, SHAPES, SPECIES, TIMINGS};
use crate::supports::{ARMADILLO_SOIL, FROG_LEAF_EDGE, IGUANA_BRANCH, surface_y};

type Point = (f32, f32);
const COUNT: usize = 30;

#[derive(Debug)]
struct Morph {
    delay: f32,
    duration: f32,
    path: AnimationRuntime,
    color_delay: f32,
    color_duration: f32,
    color: AnimationRuntime,
}

#[derive(Debug)]
struct Rig {
    rest: [Piece; COUNT],
    blink: [Piece; COUNT],
    morph: [Morph; COUNT],
    topology: Topology,
    assembly_end: f32,
}

#[derive(Debug)]
struct Seam {
    vertex: usize,
    a: usize,
    b: usize,
    fraction: f32,
}

#[derive(Debug)]
struct Topology {
    shared: Vec<Vec<usize>>,
    seams: Vec<Seam>,
}

impl Topology {
    fn new(pieces: &[Piece; COUNT]) -> Self {
        let points: [Point; COUNT * 3] = std::array::from_fn(|i| pieces[i / 3].path.points[i % 3]);
        let mut shared: Vec<Vec<usize>> = Vec::new();
        for (i, p) in points.iter().enumerate() {
            if let Some(group) = shared.iter_mut().find(|group| {
                let q = points[group[0]];
                (p.0 - q.0).hypot((p.1 - q.1) * 0.7) < 0.001
            }) {
                group.push(i);
            } else {
                shared.push(vec![i]);
            }
        }
        let mut seams = Vec::new();
        // A corner can meet the middle of another triangle's edge. Retain its
        // barycentric position on that edge when a nonlinear joint bends it.
        for (vertex, p) in points.iter().enumerate() {
            let mut longest = 0.;
            let mut seam = None;
            for a in 0..COUNT * 3 {
                if a / 3 == vertex / 3 {
                    continue;
                }
                let b = a / 3 * 3 + (a % 3 + 1) % 3;
                let delta = (points[b].0 - points[a].0, (points[b].1 - points[a].1) * 0.7);
                let length_squared = delta.0 * delta.0 + delta.1 * delta.1;
                if length_squared < 4. {
                    continue;
                }
                let fraction = ((p.0 - points[a].0) * delta.0
                    + (p.1 - points[a].1) * 0.7 * delta.1)
                    / length_squared;
                let projection = mix(points[a], points[b], fraction);
                let distance = (p.0 - projection.0).hypot((p.1 - projection.1) * 0.7);
                if (0.045..0.955).contains(&fraction) && distance < 0.28 && length_squared > longest
                {
                    longest = length_squared;
                    seam = Some(Seam {
                        vertex,
                        a,
                        b,
                        fraction,
                    });
                }
            }
            if let Some(seam) = seam {
                seams.push(seam);
            }
        }
        Self { shared, seams }
    }

    fn connect(&self, mut points: [Point; COUNT * 3]) -> [Point; COUNT * 3] {
        for group in &self.shared {
            let mean = group.iter().fold((0., 0.), |sum, &i| {
                (
                    sum.0 + points[i].0 / group.len() as f32,
                    sum.1 + points[i].1 / group.len() as f32,
                )
            });
            for &i in group {
                points[i] = mean;
            }
        }
        for _ in 0..6 {
            for seam in &self.seams {
                points[seam.vertex] = mix(points[seam.a], points[seam.b], seam.fraction);
            }
        }
        points
    }
}

#[derive(Debug)]
pub(crate) struct Performance {
    rigs: [Rig; 18],
    ease: AnimationRuntime,
    title: [Piece; COUNT],
    scattered: [Piece; COUNT],
}

fn rotate(p: Point, root: Point, angle: f32) -> Point {
    // The reference percentages occupy a 100:70 rectangle. Rotate in that
    // physical space so an articulated part never changes its proportions.
    let (s, c) = angle.sin_cos();
    let (x, y) = (p.0 - root.0, (p.1 - root.1) * 0.7);
    (root.0 + c * x - s * y, root.1 + (s * x + c * y) / 0.7)
}

fn mix(a: Point, b: Point, p: f32) -> Point {
    (a.0 + (b.0 - a.0) * p, a.1 + (b.1 - a.1) * p)
}

fn set_contact_triangle(
    pieces: &mut [Piece; COUNT],
    index: usize,
    piece: usize,
    framing: Framing,
    screen: [Point; 3],
) {
    pieces[piece].path.points = screen.map(|point| {
        let point = COMPOSITIONS[index].map(point);
        (
            50. + (point.0 - framing.x) * 100. / framing.width,
            50. + (point.1 - framing.y) / (framing.width * 0.007),
        )
    });
}

fn canonical(index: usize, state: usize) -> [Piece; COUNT] {
    let mut pieces = std::array::from_fn(|i| SHAPES[CELLS[SPECIES[index].states[state][i]].shape]);
    // The source draws these feet at different heights or as detached shards.
    // Keep their roots within the ankle/belly and give each sole a real plane.
    match index {
        2 => {
            pieces[20].path.points[2].1 = 63.5;
            pieces[21].path.points[2].1 = 63.5;
            let f = FRAMING[index];
            let seated = Framing {
                y: 540. - 13.5 * f.width * 0.007,
                ..f
            };
            set_contact_triangle(
                &mut pieces,
                index,
                20,
                seated,
                [(1168., 489.), (1146., 540.), (1078., 540.)],
            );
        }
        3 => {
            let f = FRAMING[index];
            set_contact_triangle(
                &mut pieces,
                index,
                20,
                f,
                [
                    (792.5, 603.1),
                    (825., surface_y(FROG_LEAF_EDGE, 1920. - 825.)),
                    (753.5, 617.),
                ],
            );
            set_contact_triangle(
                &mut pieces,
                index,
                28,
                f,
                [(418.5, 597.), (511., 680.5), (591.5, 659.4)],
            );
            set_contact_triangle(
                &mut pieces,
                index,
                29,
                f,
                [(583., 663.), (511., 680.5), (643., 656.)],
            );
        }
        7 => {
            let f = FRAMING[index];
            let near = (992., surface_y(IGUANA_BRANCH, 1920. - 992.) + 6.);
            let toe = (1030., surface_y(IGUANA_BRANCH, 1920. - 1030.) + 7.);
            set_contact_triangle(
                &mut pieces,
                index,
                14,
                f,
                [(1000., 356.), (1074., 459.), (1100., 452.)],
            );
            set_contact_triangle(
                &mut pieces,
                index,
                15,
                f,
                [(972., 355.), (931., 413.), (956., 434.)],
            );
            set_contact_triangle(
                &mut pieces,
                index,
                16,
                f,
                [(931., 413.), (956., 434.), near],
            );
            set_contact_triangle(&mut pieces, index, 17, f, [(956., 434.), near, toe]);
        }
        9 => {
            let f = FRAMING[index];
            let grounded = Framing {
                y: ARMADILLO_SOIL - 34.786 * f.width * 0.007,
                ..f
            };
            set_contact_triangle(
                &mut pieces,
                index,
                8,
                grounded,
                [(833., 699.), (878., ARMADILLO_SOIL), (933., ARMADILLO_SOIL)],
            );
            set_contact_triangle(
                &mut pieces,
                index,
                9,
                grounded,
                [(734., 687.), (688., ARMADILLO_SOIL), (729., ARMADILLO_SOIL)],
            );
            set_contact_triangle(
                &mut pieces,
                index,
                28,
                grounded,
                [(593., 687.), (552., ARMADILLO_SOIL), (603., ARMADILLO_SOIL)],
            );
        }
        4 => {
            pieces[28].path.points = [(44.7, 57.7), (41.2, 65.8), (46., 65.8)];
            pieces[29].path.points = [(47.2, 59.5), (44.8, 65.8), (49.6, 65.8)];
        }
        11 => {
            pieces[18].path.points = [(44.8, 81.2), (39., 86.), (43.8, 86.)];
            pieces[19].path.points = [(44.8, 81.2), (43.8, 86.), (47., 86.)];
            pieces[20].path.points = [(44., 83.2), (36.5, 86.), (43.8, 86.)];
        }
        13 => {
            pieces[6].path.points = [(43., 82.8), (53., 89.), (40., 89.)];
            pieces[7].path.points = [(51., 80.8), (62., 89.), (52., 89.)];
        }
        15 => {
            for i in [24, 28] {
                for p in &mut pieces[i].path.points {
                    if p.1 > 83. {
                        p.1 = 86.5;
                    }
                }
            }
        }
        _ => {}
    }
    // CSS percentages contain small rounding differences at shared seams.
    // Weld once, before articulation, rather than covering cracks every frame.
    let source: [Point; COUNT * 3] = std::array::from_fn(|i| pieces[i / 3].path.points[i % 3]);
    let mut visited = [false; COUNT * 3];
    for i in 0..COUNT * 3 {
        if visited[i] {
            continue;
        }
        let members: Vec<_> = (i..COUNT * 3)
            .filter(|&j| {
                !visited[j]
                    && (source[i].0 - source[j].0).hypot((source[i].1 - source[j].1) * 0.7) < 0.34
            })
            .collect();
        let mean = members.iter().fold((0., 0.), |sum, &j| {
            (
                sum.0 + source[j].0 / members.len() as f32,
                sum.1 + source[j].1 / members.len() as f32,
            )
        });
        for j in members {
            pieces[j / 3].path.points[j % 3] = mean;
            visited[j] = true;
        }
    }
    // Constrain each sole, not the lowest point of the complete silhouette.
    let contacts: &[(usize, usize, f32)] = match index {
        6 => &[(4, 2, 86.), (6, 2, 86.), (13, 2, 86.), (15, 2, 86.)],
        9 => &[
            (8, 1, 84.786),
            (8, 2, 84.786),
            (9, 1, 84.786),
            (9, 2, 84.786),
            (28, 1, 84.786),
            (28, 2, 84.786),
        ],
        12 => &[
            (13, 2, 84.143),
            (28, 2, 84.143),
            (29, 1, 84.143),
            (29, 2, 84.143),
        ],
        17 => &[
            (0, 2, 85.571),
            (1, 1, 85.571),
            (14, 2, 85.571),
            (15, 2, 85.571),
        ],
        _ => &[],
    };
    for &(piece, vertex, y) in contacts {
        pieces[piece].path.points[vertex].1 = y;
    }
    pieces
}

impl Default for Performance {
    fn default() -> Self {
        Self {
            rigs: std::array::from_fn(|index| Rig {
                rest: canonical(index, 1),
                blink: canonical(index, 5),
                topology: Topology::new(&canonical(index, 1)),
                assembly_end: (0..COUNT).fold(2.4_f32, |end, i| {
                    let cell = CELLS[SPECIES[index].states[0][i]];
                    let path = TIMINGS[cell.path_timing];
                    let color = TIMINGS[cell.color_timing];
                    end.max(path.delay + path.duration)
                        .max(color.delay + color.duration)
                }),
                morph: std::array::from_fn(|i| {
                    let cell = CELLS[SPECIES[index].states[0][i]];
                    let path = TIMINGS[cell.path_timing];
                    let color = TIMINGS[cell.color_timing];
                    Morph {
                        delay: path.delay,
                        duration: path.duration,
                        path: path.runtime(1.),
                        color_delay: color.delay,
                        color_duration: color.duration,
                        color: color.runtime(1.),
                    }
                }),
            }),
            ease: AnimationRuntime::new(1., &Easing::EaseInOut),
            title: crate::title_card::assembled(),
            scattered: crate::title_card::scattered(),
        }
    }
}

impl Performance {
    fn ramp(&self, start: f32, end: f32, t: f32) -> f32 {
        self.ease
            .solve(&((t - start) / (end - start)).clamp(0., 1.))
    }

    fn window(&self, start: f32, end: f32, edge: f32, t: f32) -> f32 {
        self.ramp(start, start + edge, t) * (1. - self.ramp(end - edge, end, t))
    }

    fn joint(&self, p: Point, root: Point, angle: f32, weight: f32) -> Point {
        mix(p, rotate(p, root, angle), weight.clamp(0., 1.))
    }

    fn framing(&self, index: usize, t: f32) -> Framing {
        let mut f = FRAMING[index];
        match index {
            0 => f.y = 410.,
            1 => {
                let dive = self.ramp(2.8, 9.7, t);
                f.x -= dive * 610.;
                f.y += dive * 155. - (dive * PI).sin() * 45.;
                f.width *= 1. - dive * 0.18;
            }
            2 => f.y = 540. - (63.5 - 50.) * f.width * 0.007,
            4 => f.y = 490. - (65.8 - 50.) * f.width * 0.007,
            5 => {
                // Cross the frame at a fixed depth. Both ends remain visible,
                // so the next morph starts from the turtle at the right edge.
                f.width = 920.;
                f.x = 400. + 1180. * self.ramp(2.4, SECONDS_PER_SPECIES, t);
            }
            6 => f.y = 745. - 36. * f.width * 0.007,
            9 => f.y = ARMADILLO_SOIL - (84.786 - 50.) * f.width * 0.007,
            11 => {
                f.x = 1340. - Self::walk_clock(t) * 55.;
                f.y = 770. - 36. * f.width * 0.007;
            }
            12 => f.y = 765. - (84.143 - 50.) * f.width * 0.007,
            13 => f.y = 760. - 39. * f.width * 0.007,
            14 => {
                let flight = self.window(7., 9.1, 0.7, t);
                f.x += flight * 60.;
                f.y -= flight * 95.;
            }
            15 => f.y = 765. - 36.5 * f.width * 0.007,
            16 => {
                f.x = 1110. + self.ramp(2.5, 4.8, t) * 90. + self.ramp(7.2, 10., t) * 235.;
                f.y -= self.ramp(7.2, 10., t) * 80.;
            }
            17 => f.y = 760. - (85.571 - 50.) * f.width * 0.007,
            _ => {}
        }
        f
    }

    fn walk_clock(t: f32) -> f32 {
        (t - 2.8).clamp(0., 2.58)
    }

    fn bird_foot(&self, p: Point, t: f32, phase: f32) -> Point {
        let period = 0.86;
        let stance = 0.68;
        let speed = 55. / (FRAMING[11].width / 100.);
        let cycle = (Self::walk_clock(t) / period + phase).rem_euclid(1.);
        let stride = speed * period * stance;
        let (dx, dy) = if cycle < stance {
            ((cycle / stance - 0.5) * stride, 0.)
        } else {
            let u = (cycle - stance) / (1. - stance);
            ((0.5 - self.ramp(0., 1., u)) * stride, -(u * PI).sin() * 4.)
        };
        let weight = self.ramp(64., 81., p.1);
        (p.0 + dx * weight, p.1 + dy * weight)
    }

    fn peck(&self, t: f32) -> f32 {
        // Three deliberate bites with time to raise the head between them.
        [6.25, 7.25, 8.25]
            .into_iter()
            .map(|center| self.window(center - 0.42, center + 0.42, 0.32, t))
            .sum()
    }

    fn flipper(&self, p: Point, t: f32, far: bool) -> Point {
        let phase = ((t - 2.4) / 1.9 + if far { 0.035 } else { 0. }).rem_euclid(1.);
        let stroke = if phase < 0.36 {
            1. - 2. * self.ramp(0., 0.36, phase)
        } else {
            -1. + 2. * self.ramp(0.36, 1., phase)
        };
        let span = (50. - p.1).max(0.);
        let elevation = if far { -0.23 } else { 0.23 } + stroke * 0.77;
        let target = (
            p.0 - span * (0.08 + (1. - stroke) * 0.12),
            50. - span * elevation,
        );
        let q = mix(p, target, self.ramp(0., 8., span));
        if far { (q.0 - 3., q.1 - 2.) } else { q }
    }

    fn articulate(&self, index: usize, i: usize, p: Point, t: f32) -> Point {
        let r = |a, b, v| self.ramp(a, b, v);
        let w = |a, b, e| self.window(a, b, e, t);
        match index {
            0 => {
                let look = -0.29 * w(3.4, 6.5, 0.9) + 0.12 * w(7., 9.3, 0.7);
                self.joint(
                    p,
                    (60., 37.),
                    look,
                    r(49., 61., p.0) * (1. - r(30., 44., p.1)),
                )
            }
            1 => {
                let tail = r(49., 86., p.0);
                let q = (p.0, p.1 + (t * TAU / 1.55 - p.0 * 0.04).sin() * 4.2 * tail);
                if (15..=18).contains(&i) {
                    self.joint(
                        q,
                        (30., 59.),
                        -0.12 + (t * 2.4).sin() * 0.08,
                        r(58., 68., p.1),
                    )
                } else {
                    q
                }
            }
            2 => {
                let q = self.joint(
                    p,
                    (36., 40.),
                    -0.23 * w(4., 7.4, 0.85),
                    (1. - r(31., 43., p.0)) * (1. - r(35., 49., p.1)),
                );
                if i >= 28 {
                    self.joint(q, (64., 62.), 0.24 * w(3., 9.3, 1.3), r(62., 76., p.1))
                } else {
                    q
                }
            }
            3 => self.joint(
                p,
                (38., 43.),
                -0.07 * w(4.4, 6.2, 0.55),
                (1. - r(34., 44., p.0)) * (1. - r(39., 51., p.1)),
            ),
            4 => {
                let tilt = -0.22 * w(3.3, 5.6, 0.8) + 0.19 * w(6.1, 8.8, 0.8);
                self.joint(p, (47., 28.), tilt, 1. - r(22., 34., p.1))
            }
            5 => {
                if (21..=24).contains(&i) {
                    self.flipper(p, t, false)
                } else if (1..=2).contains(&i) {
                    self.joint(
                        p,
                        (29., 55.),
                        (t * TAU / 1.9 - 0.3).sin() * 0.12,
                        r(56., 65., p.1),
                    )
                } else {
                    p
                }
            }
            6 => {
                let neck = r(60., 72., p.0).max(1. - r(22., 35., p.1)) * (1. - r(42., 62., p.1));
                self.joint(
                    p,
                    (61., 55.),
                    0.72 * w(3.3, 9.2, 1.3) + (t * 8.).sin() * 0.012 * w(5., 7.8, 0.5),
                    neck,
                )
            }
            7 => {
                let q = self.joint(
                    p,
                    (30., 41.),
                    0.22 * w(3.2, 7.2, 1.),
                    (1. - r(21., 35., p.0)) * (1. - r(40., 54., p.1)),
                );
                (
                    q.0,
                    q.1 + if i >= 25 {
                        (t * 1.2 - p.0 * 0.06).sin() * 3. * r(56., 97., p.0) * w(3., 9.5, 0.8)
                    } else {
                        0.
                    },
                )
            }
            8 => {
                let hold = 1. - r(68., 79., p.0) * r(52., 64., p.1);
                let q = self.joint(
                    p,
                    (77., 64.),
                    (t * 1.1).sin() * 0.055 * w(2.5, 10., 0.8),
                    hold,
                );
                self.joint(
                    q,
                    (50., 41.),
                    -0.18 * w(5., 6.8, 0.45),
                    1. - r(30., 43., p.1),
                )
            }
            9 if matches!(i, 8 | 9 | 28) => p,
            9 => self.joint(
                p,
                (33., 63.),
                -0.2 * w(2.65, 3.7, 0.4) - (t * 8.).sin().max(0.) * 0.02 * w(2.8, 3.5, 0.2),
                1. - r(24., 37., p.0),
            ),
            10 => {
                if i >= 28 {
                    self.joint(p, (63.15, 54.), -0.38 * r(6.4, 9.7, t), 1.)
                } else {
                    self.joint(
                        p,
                        (65., 42.),
                        0.1 * r(7.7, 9.8, t),
                        r(61., 69., p.0) * (1. - r(30., 45., p.1)),
                    )
                }
            }
            11 => {
                if (17..=21).contains(&i) {
                    self.bird_foot(p, t, 0.)
                } else {
                    let feed = -0.1 * w(5.6, 9.4, 0.5) - 0.3 * self.peck(t);
                    self.joint(p, (27., 43.), feed, 1. - r(23., 37., p.0))
                }
            }
            12 => {
                if (13..=14).contains(&i) {
                    let paw = (t * TAU / 1.1).sin().max(0.) * w(4.1, 7.7, 0.5);
                    let weight = r(70., 83., p.1);
                    (p.0 + paw * 1.7 * weight, p.1 - paw * 2.8 * weight)
                } else {
                    self.joint(
                        p,
                        (33., 63.),
                        -0.19 * w(3.5, 8.7, 1.) - (t * 5.).sin().max(0.) * 0.022 * w(4.5, 7.5, 0.5),
                        1. - r(23., 39., p.0),
                    )
                }
            }
            13 => {
                if i == 26 {
                    self.joint(p, (58., 54.), -0.28 * w(5.3, 8.6, 0.9), r(53., 63., p.1))
                } else {
                    self.joint(
                        p,
                        (54., 29.),
                        1.18 * w(3.8, 7.7, 1.1) + (t * 7.).sin() * 0.025 * w(5.1, 6.8, 0.4),
                        1. - r(22., 36., p.1),
                    )
                }
            }
            14 => {
                if i >= 26 {
                    let flutter = w(3.8, 5.1, 0.25).max(w(6.8, 9.3, 0.3));
                    let factor = 1. - flutter * (0.5 + (t * TAU * 7.3).sin() * 0.43);
                    (69.5 + (p.0 - 69.5) * factor, 53.7 + (p.1 - 53.7) * factor)
                } else {
                    p
                }
            }
            15 => {
                if (17..=18).contains(&i) {
                    self.joint(
                        p,
                        (43., 30.),
                        -0.55 * w(5., 8.9, 0.7) + (t * 9.).sin() * 0.07 * w(5.8, 8.2, 0.4),
                        r(29., 40., p.1),
                    )
                } else {
                    self.joint(
                        p,
                        (45., 24.),
                        -0.2 * w(3.2, 5.2, 0.7),
                        1. - r(19., 33., p.1),
                    )
                }
            }
            16 => {
                let tail = 1. - r(18., 38., p.0);
                let q = (p.0, p.1 + (t * TAU / 0.95 - p.0 * 0.07).sin() * 2.8 * tail);
                if i >= 27 {
                    self.joint(
                        q,
                        (81., 47.),
                        (t * 11.).sin() * 0.045 * w(5., 7., 0.3),
                        r(80., 88., p.0),
                    )
                } else {
                    q
                }
            }
            _ => {
                let neck = r(59., 71., p.0) * (1. - r(48., 65., p.1));
                self.joint(
                    p,
                    (63., 49.),
                    0.5 * w(3.4, 9.4, 1.2) + (t * 7.).sin() * 0.015 * w(5.5, 8., 0.4),
                    neck,
                )
            }
        }
    }

    fn pose(&self, index: usize, t: f32) -> [Piece; COUNT] {
        let action_time = if index == 9 { t.min(3.5) } else { t };
        let f = self.framing(index, action_time);
        let blink = [3.1, 8.9]
            .into_iter()
            .map(|at| self.window(at, at + 0.18, 0.07, action_time))
            .sum::<f32>();
        let mut pieces: [Piece; COUNT] = std::array::from_fn(|i| {
            self.rigs[index].rest[i].apply_progress(&self.rigs[index].blink[i], blink)
        });
        let points = self.rigs[index].topology.connect(std::array::from_fn(|i| {
            self.articulate(index, i / 3, pieces[i / 3].path.points[i % 3], action_time)
        }));
        for (i, piece) in pieces.iter_mut().enumerate() {
            piece.path = Path {
                points: std::array::from_fn(|v| f.map(points[i * 3 + v])),
            };
        }
        if index == 9 {
            self.armadillo_pose(pieces, t)
        } else {
            pieces
        }
    }

    fn armadillo_pose(&self, mut pieces: [Piece; COUNT], t: f32) -> [Piece; COUNT] {
        const RADIUS: f32 = 180.;
        const CENTER_X: f32 = 1190.;

        if t <= 3.5 {
            return pieces;
        }
        let turn = 2. * self.ramp(5.25, 8.65, t);
        // Roll about each successive contact vertex. Rotation and travel stay
        // coupled, so a planted corner does not slide across the ground.
        let step = TAU / 16.;
        let half = step / 2.;
        let contact = ((turn + half) / step).floor();
        let local = turn - contact * step;
        let travel = RADIUS * (2. * contact * half.sin() + local.sin());
        let height = RADIUS * local.cos();
        let (sin, cos) = turn.sin_cos();
        for (i, piece) in pieces.iter_mut().enumerate() {
            let (start, finish) = match i {
                0..=7 => (3.5, 4.45),
                8 | 9 | 28 => (3.6, 4.75),
                _ => (3.72, 5.15),
            };
            let curl = self.ramp(start, finish, t);
            let target = BALL[i].map(|(x, y)| {
                (
                    CENTER_X + travel + RADIUS * (x * cos - y * sin),
                    ARMADILLO_SOIL - height + RADIUS * (x * sin + y * cos),
                )
            });
            piece.path = piece.path.apply_progress(&target, curl);
        }
        // As the paws retract, the lower shell takes their weight. Preserve
        // contact during the fold with translation, without stretching it.
        let bottom = pieces
            .iter()
            .flat_map(|piece| piece.path.points)
            .map(|point| point.1)
            .fold(f32::NEG_INFINITY, f32::max);
        let settle = ARMADILLO_SOIL - bottom;
        pieces.map(|piece| Piece {
            path: piece.path.translated(0., settle),
            ..piece
        })
    }

    fn composed_pose(&self, index: usize, t: f32) -> [Piece; COUNT] {
        self.pose(index, t).map(|piece| Piece {
            path: piece.path.map(|p| COMPOSITIONS[index].map(p)),
            ..piece
        })
    }

    pub fn sample(&self, t: f32) -> [Piece; COUNT] {
        let index = species_at(t);
        let local = t - index as f32 * SECONDS_PER_SPECIES;
        let assembly_end = self.rigs[index].assembly_end;
        let to = self.composed_pose(index, local.max(assembly_end));
        if local >= assembly_end {
            return to;
        }
        let from = if index == 0 {
            self.title
        } else {
            self.composed_pose(index - 1, SECONDS_PER_SPECIES)
        };
        let entry = self.composed_pose(index, assembly_end);
        self.morph(index, local, &from, &entry)
    }

    fn morph(
        &self,
        index: usize,
        time: f32,
        from: &[Piece; COUNT],
        to: &[Piece; COUNT],
    ) -> [Piece; COUNT] {
        std::array::from_fn(|i| {
            let m = &self.rigs[index].morph[i];
            let path_progress = if time <= m.delay {
                0.
            } else if time >= m.delay + m.duration {
                1.
            } else {
                m.path.solve(&(time - m.delay)).clamp(0., 1.)
            };
            let color_progress = if time <= m.color_delay {
                0.
            } else if time >= m.color_delay + m.color_duration {
                1.
            } else {
                m.color.solve(&(time - m.color_delay)).clamp(0., 1.)
            };
            Piece {
                path: from[i].path.apply_progress(&to[i].path, path_progress),
                color: from[i].color.apply_progress(&to[i].color, color_progress),
            }
        })
    }

    pub fn opening(&self, t: f32) -> [Piece; COUNT] {
        self.morph(0, t, &self.scattered, &self.title)
    }

    pub fn ending(&self, t: f32) -> [Piece; COUNT] {
        if t >= crate::title_card::SCATTER_START {
            return self.morph(
                0,
                t - crate::title_card::SCATTER_START,
                &self.title,
                &self.scattered,
            );
        }
        let from = self.composed_pose(17, SECONDS_PER_SPECIES);
        self.morph(0, t, &from, &self.title)
    }

    pub fn order(&self, index: usize) -> [usize; COUNT] {
        let mut order = std::array::from_fn(|i| i);
        order.sort_by_key(|&i| {
            let layer = match index {
                5 if (21..=24).contains(&i) => 2,
                11 if (17..=21).contains(&i) => 0,
                13 if i == 6 || i == 7 => 0,
                _ => 1,
            };
            (layer, CELLS[SPECIES[index].states[1][i]].z, i)
        });
        order
    }

    pub fn appendages(&self, index: usize, t: f32) -> Svgr<'static> {
        let alpha = self.ramp(2., 2.5, t);
        let f = self.framing(index, t);
        let geometry: Svgr<'static> = match index {
            5 => (21..=24)
                .map(|i| {
                    let piece = self.rigs[index].rest[i];
                    let path = piece.path.map(|p| f.map(self.flipper(p, t, true)));
                    let color = piece.color.apply_progress(&Color::rgb(13, 84, 99), 0.28);
                    svgr!(<path d={path} fill={color} />)
                })
                .collect(),
            11 => (17..=21)
                .map(|i| {
                    let piece = self.rigs[index].rest[i];
                    let path = piece.path.map(|p| {
                        let q = self.bird_foot(p, t, 0.5);
                        f.map((q.0 + 6., q.1))
                    });
                    let color = piece.color.apply_progress(&Color::rgb(28, 49, 29), 0.32);
                    svgr!(<path d={path} fill={color} />)
                })
                .collect(),
            14 => (26..30)
                .map(|i| {
                    let piece = self.rigs[index].rest[i];
                    let path = piece.path.map(|p| {
                        let q = self.articulate(index, i, p, t - 0.017);
                        f.map((69.5 + (q.0 - 69.5) * 0.85, 53.7 + (q.1 - 53.7) * 0.72))
                    });
                    svgr!(<path d={path} fill="#c6ead8" opacity="0.6" />)
                })
                .collect(),
            _ => Svgr::empty(),
        };
        svgr!(<g opacity={alpha}>{geometry}</g>)
    }

    fn anchor(&self, index: usize, t: f32, i: usize, vertex: usize) -> Point {
        self.pose(index, t)[i].path.points[vertex]
    }

    pub fn props(&self, index: usize, t: f32) -> Svgr<'static> {
        match index {
            0 | 2 | 11 => {
                let (tip, root, color) = match index {
                    0 => {
                        let tip = self.anchor(0, 4.9, 29, 1);
                        (tip, (2030., 430.), Color::rgb(217, 73, 50))
                    }
                    2 => {
                        let tip = self.anchor(2, 5.4, 1, 0);
                        (
                            (tip.0 - 10., tip.1 + 1.),
                            (720., 418.),
                            Color::rgb(237, 103, 50),
                        )
                    }
                    _ => {
                        let tip = self.anchor(11, 6.25, 4, 1);
                        (tip, (tip.0 - 95., 855.), Color::rgb(245, 79, 111))
                    }
                };
                let stem = Path {
                    points: [root, (root.0 + 7., root.1), tip],
                };
                let fruit: Svgr<'static> = (0..7)
                    .map(|i| {
                        let angle = i as f32 * 2.4;
                        let radius = if i == 0 { 0. } else { 18. };
                        let center = (tip.0 + angle.cos() * radius, tip.1 + angle.sin() * radius);
                        let eaten = if index == 11 && i < 3 {
                            self.ramp(6.25 + i as f32, 6.42 + i as f32, t)
                        } else if index == 2 && i == 0 {
                            self.ramp(5.4, 5.65, t)
                        } else if index == 0 && i == 0 {
                            self.ramp(4.9, 5.15, t)
                        } else {
                            0.
                        };
                        let berry = diamond(center, 9., color);
                        svgr!(<g opacity={1. - eaten}>{berry}</g>)
                    })
                    .collect();
                let leaf = Path {
                    points: [
                        (root.0 + 2., root.1 - 60.),
                        (tip.0 - 45., tip.1 + 10.),
                        (tip.0 - 17., tip.1 + 55.),
                    ],
                };
                svgr!(<g><path d={stem} fill="#526b45" /><path d={leaf} fill="#6ca34e" />{fruit}</g>)
            }
            3 => {
                let mouth = self.anchor(index, t.max(2.4), 0, 0);
                let target = (mouth.0 - 150., mouth.1 - 22.);
                let fly = (
                    target.0 + (t * 4.).sin() * 25.,
                    target.1 + (t * 3.).cos() * 16.,
                );
                let snap = self.window(5.25, 5.62, 0.11, t);
                let end = mix(mouth, fly, snap);
                let tongue = Path {
                    points: [(mouth.0, mouth.1 + 2.), end, (mouth.0, mouth.1 - 2.)],
                };
                let insect = diamond(fly, 4., Color::rgb(43, 56, 47));
                svgr!(<g opacity={self.ramp(2.3, 2.8, t)}>
                    <path d={tongue} fill="#d57967" />
                    <g opacity={1. - self.ramp(5.4, 5.47, t)}>{insect}</g>
                </g>)
            }
            6 | 17 => {
                let (piece, vertex, time) = if index == 6 { (23, 2, 6.) } else { (29, 2, 6.) };
                let tip = self.anchor(index, time, piece, vertex);
                let ground = if index == 6 { 745. } else { 760. };
                let root = (tip.0 + 55., ground + 30.);
                let leaves: Svgr<'static> = (0..8)
                    .map(|i| {
                        let y = tip.1 + i as f32 * 23.;
                        let x = tip.0 + i as f32 * 7.;
                        let sway = (t * 1.4 + i as f32).sin() * 3.;
                        let path = Path {
                            points: [
                                (x + 20., y + 20.),
                                (x - 34. + sway, y - 12.),
                                (x - 8., y + 28.),
                            ],
                        };
                        let color = if i % 2 == 0 { "#91a448" } else { "#698e40" };
                        svgr!(<path d={path} fill={color} />)
                    })
                    .collect();
                let stem = Path {
                    points: [root, tip, (root.0 + 5., root.1)],
                };
                svgr!(<g><path d={stem} fill="#786e44" />{leaves}</g>)
            }
            8 => {
                let mouth = self.anchor(index, t.max(2.4), 0, 2);
                let p = self.ramp(4.8, 5.8, t);
                let snack = diamond(
                    (mouth.0 - (1. - p) * 90., mouth.1 - (1. - p) * 25.),
                    3.,
                    Color::rgb(231, 220, 172),
                );
                svgr!(<g opacity={self.window(3.8, 5.85, 0.2, t)}>{snack}</g>)
            }
            9 => svgr!(<g opacity={1. - self.ramp(4.3, 5.1, t)}>{self.digging(t)}</g>),
            12 => {
                let nose = self.anchor(index, 6., 1, 0);
                let litter: Svgr<'static> = (0..9)
                    .map(|i| {
                        let phase = (t * 0.85 + i as f32 * 0.23).fract();
                        let action = self.window(4.2, 7.8, 0.4, t);
                        let origin = (nose.0 + 20. + i as f32 * 11., 765.);
                        let p = (
                            origin.0 + phase * 26. * action,
                            origin.1 - (phase * PI).sin() * 14. * action,
                        );
                        let path = Path {
                            points: [(p.0 - 8., p.1), (p.0 + 5., p.1 - 4.), (p.0 + 12., p.1 + 2.)],
                        };
                        svgr!(<path d={path} fill="#9a7951" />)
                    })
                    .collect();
                litter
            }
            10 => {
                let tip = self.anchor(index, 9.7, 29, 1);
                let root = (1980., tip.1 - 130.);
                let stem = Path {
                    points: [root, tip, (root.0 + 5., root.1)],
                };
                let leaves: Svgr<'static> = (0..5)
                    .map(|i| {
                        let x = tip.0 + i as f32 * 32.;
                        let y = tip.1 + i as f32 * 15.;
                        let bend = self.ramp(8.4, 9.7, t) * (t * 1.1).sin() * 3.;
                        let path = Path {
                            points: [
                                (x + 30., y + 12.),
                                (x - 8., y - 25. + bend),
                                (x - 20., y + 6.),
                            ],
                        };
                        svgr!(<path d={path} fill="#78ad53" />)
                    })
                    .collect();
                svgr!(<g><path d={stem} fill="#626a43" />{leaves}</g>)
            }
            14 => {
                let a = self.anchor(index, 2.4, 21, 2);
                let b = self.anchor(index, 2.4, 23, 1);
                let root = (b.0 + 230., 1100.);
                let left = (a.0 - 25., a.1 + 5.);
                let perch = Path {
                    points: [left, a, (b.0 + 8., b.1 + 8.)],
                };
                let stem = Path {
                    points: [b, root, (root.0 + 14., root.1)],
                };
                svgr!(<g><path d={stem} fill="#557b48" /><path d={perch} fill="#6d9258" /></g>)
            }
            16 => {
                let tip = self.anchor(index, 5.8, 28, 1);
                let root = (tip.0 + 180., 1080.);
                let middle = (tip.0 + 100., 720.);
                let stem = Path {
                    points: [root, middle, (root.0 + 35., root.1)],
                };
                let branch = Path {
                    points: [middle, tip, (middle.0 + 23., middle.1)],
                };
                let corals: Svgr<'static> = (0..7)
                    .map(|i| {
                        let y = tip.1 + i as f32 * 43.;
                        let x = tip.0 + i as f32 * 13.;
                        let p = Path {
                            points: [(x + 18., y + 45.), (x - 36., y - 5.), (x - 23., y - 12.)],
                        };
                        svgr!(<path d={p} fill="#f1b482" />)
                    })
                    .collect();
                svgr!(<g><path d={stem} fill="#d0828c" /><path d={branch} fill="#e49a91" />{corals}</g>)
            }
            _ => Svgr::empty(),
        }
    }

    fn digging(&self, t: f32) -> Svgr<'static> {
        let nose = self.anchor(9, 3.4, 5, 0);
        let x = nose.0;
        let ground = ARMADILLO_SOIL;
        let hollow = Path {
            points: [
                (x - 22., ground),
                (x + 138., ground),
                (x + 58., ground + 25.),
            ],
        };
        let rim = Path {
            points: [
                (x + 58., ground + 25.),
                (x + 138., ground),
                (x + 155., ground + 22.),
            ],
        };
        let furrows: Svgr<'static> = (0..3)
            .map(|i| {
                let y = ground + 9. + i as f32 * 5.;
                let length = 18. + self.ramp(2.7, 3.5, t) * (48. + i as f32 * 12.);
                let path = Path {
                    points: [(x + 30., y), (x + 30. + length, y - 2.), (x + 45., y + 3.)],
                };
                svgr!(<path d={path} fill="#663e30" />)
            })
            .collect();
        let soil: Svgr<'static> = (0..9).map(|i| {
            let life = ((t - 2.7) / 0.79 - i as f32 * 0.13).rem_euclid(1.);
            let distance = 45. + i as f32 * 12.;
            let p = (x + 32. + life * distance, ground - (life * PI).sin() * (19. + (i % 3) as f32 * 9.));
            let size = 3. + (i % 4) as f32;
            let a = Path { points: [(p.0 - size, p.1), (p.0 + size, p.1 - size * 0.7), (p.0 + size * 0.6, p.1 + size * 0.5)] };
            let b = Path { points: [a.points[0], a.points[2], (p.0 - size * 0.5, p.1 + size * 0.8)] };
            let opacity = self.window(2.7, 3.7, 0.3, t) * (1. - self.ramp(0.88, 1., life));
            svgr!(<g opacity={opacity}><path d={a} fill="#9e6646" /><path d={b} fill="#694331" /></g>)
        }).collect();
        svgr!(<g>
            <path d={hollow} fill="#88533c" /><path d={rim} fill="#ac7050" />
            {furrows}{soil}
        </g>)
    }
}

fn diamond(center: Point, radius: f32, color: Color) -> Svgr<'static> {
    let (x, y) = center;
    let a = Path {
        points: [(x, y - radius), (x + radius, y), (x - radius, y)],
    };
    let b = Path {
        points: [(x - radius, y), (x + radius, y), (x, y + radius)],
    };
    let shadow = color.apply_progress(&Color::rgb(26, 43, 50), 0.2);
    svgr!(<g><path d={a} fill={color} /><path d={b} fill={shadow} /></g>)
}
