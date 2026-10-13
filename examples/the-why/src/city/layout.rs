//! The plan of the city: the street grid, the diagonal boulevards, the canal, the parks, the
//! building lots, the construction sites and the video screens. The chip of shot 3 uses the
//! same plan at a small scale: the traces of the chip are the streets, its components are
//! the buildings, and its pins are two rows of long buildings at the left and right edge.
//!
//! The plan is in meters on the ground plane: `x` is the world X axis and `y` is the world
//! Z axis.

use fframes_3d::math::hash;
use fframes_3d::{Vec2, vec2};

/// Half the side of the square city, in meters.
pub const HALF: f32 = 2400.0;
/// Half the side of the part of the city that is the top face of the chip, in meters.
pub const CHIP_HALF: f32 = 810.0;
/// How far the pins reach out from the chip, in meters.
pub const PIN_REACH: f32 = 99.0;
/// The distance between two pins, in meters.
pub const PIN_PITCH: f32 = 36.0;
/// The pins on each side of the chip.
pub const PINS: usize = 44;
/// The width of the canal in the middle of the central boulevard, in meters.
pub const CANAL_WIDTH: f32 = 26.0;
/// The water is this many meters below the street.
pub const WATER_DEPTH: f32 = 4.0;
/// The height of one floor in meters.
pub const FLOOR: f32 = 3.6;
/// The width of one window bay in meters.
pub const BAY: f32 = 1.8;
/// The center of the downtown towers.
pub const DOWNTOWN: Vec2 = Vec2::new(0.0, 430.0);
/// The width and the height of a video screen, in meters.
pub const SCREEN: Vec2 = Vec2::new(30.0, 22.5);

/// A straight street strip.
#[derive(Debug, Clone, Copy)]
pub struct Street {
    pub from: Vec2,
    pub to: Vec2,
    /// The width in meters, sidewalks not included.
    pub width: f32,
    /// Arterials are gold traces on the chip, the other streets copper traces.
    pub arterial: bool,
    /// The width of the strip in the middle without road, in meters. The central
    /// boulevard has the canal there.
    pub median: f32,
}

impl Street {
    /// The unit direction from `from` to `to`.
    pub fn direction(&self) -> Vec2 {
        (self.to - self.from).normalize()
    }

    /// The distance from `p` to the center line.
    pub fn distance(&self, p: Vec2) -> f32 {
        let d = self.direction();
        let along = (p - self.from)
            .dot(d)
            .clamp(0.0, self.from.distance(self.to));
        p.distance(self.from + d * along)
    }
}

/// What a block holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Use {
    Buildings,
    Park,
    /// A construction site with a tower crane.
    Site,
}

/// The ground between four streets.
#[derive(Debug, Clone, Copy)]
pub struct Block {
    pub min: Vec2,
    pub max: Vec2,
    pub use_: Use,
}

/// The facade of a building.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Style {
    /// A glass curtain wall, for towers.
    Glass,
    /// Dark glass between light vertical fins, for towers.
    Fins,
    /// Concrete with punched windows, for the middle heights.
    Office,
    /// Dark low buildings with few lit windows.
    Low,
}

/// One box of a building. The tiers of a building stand on each other and get narrower.
#[derive(Debug, Clone, Copy)]
pub struct Tier {
    /// The full width on X and on Z, in meters.
    pub size: Vec2,
    /// The height of the roof above the street, in meters.
    pub top: f32,
}

#[derive(Debug, Clone)]
pub struct Building {
    pub center: Vec2,
    pub tiers: Vec<Tier>,
    pub style: Style,
    pub seed: u32,
    /// The building is a pin of the chip.
    pub pin: bool,
}

impl Building {
    pub fn height(&self) -> f32 {
        self.tiers.last().map_or(0.0, |t| t.top)
    }
}

/// A block where a tower is under construction.
#[derive(Debug, Clone, Copy)]
pub struct Site {
    /// The center of the steel frame of the new tower.
    pub center: Vec2,
    /// The footprint of the frame, in meters.
    pub size: Vec2,
    /// The number of floors that stand.
    pub floors: u32,
    /// The base of the crane mast.
    pub crane: Vec2,
    /// The height of the crane jib above the street, in meters.
    pub crane_height: f32,
    pub seed: u32,
}

/// A video screen on the face of a tower.
#[derive(Debug, Clone, Copy)]
pub struct Screen {
    /// The building that carries it.
    pub building: usize,
    /// The center of the screen on the ground plane, in front of the wall.
    pub at: Vec2,
    /// The height of the center above the street, in meters.
    pub height: f32,
    /// The direction that the screen faces.
    pub facing: Vec2,
}

/// A crossing of two streets.
#[derive(Debug, Clone, Copy)]
pub struct Intersection {
    pub at: Vec2,
    /// The widths of the avenue (along Z) and of the street (along X).
    pub widths: Vec2,
    /// Whether one of the two streets is an arterial.
    pub arterial: bool,
}

/// The whole plan.
pub struct Plan {
    pub streets: Vec<Street>,
    pub blocks: Vec<Block>,
    pub buildings: Vec<Building>,
    pub sites: Vec<Site>,
    pub screens: Vec<Screen>,
    pub intersections: Vec<Intersection>,
    /// The crossing of the central boulevard where the traffic lights change in view.
    pub key_crossing: Vec2,
}

/// The two rows of pins, as rectangles `(min, max)`. No street crosses them.
pub fn pin_bands() -> [(Vec2, Vec2); 2] {
    let (inner, outer) = (CHIP_HALF, CHIP_HALF + PIN_REACH + 5.0);
    let z = PIN_PITCH * PINS as f32 * 0.5 + 10.0;
    [
        (vec2(inner, -z), vec2(outer, z)),
        (vec2(-outer, -z), vec2(-inner, z)),
    ]
}

/// The parts of the segment `a`-`b` as the parameter range `t0..t1` inside the rectangle
/// `min`-`max`. `None` when it does not enter the rectangle.
pub fn clip_range(a: Vec2, b: Vec2, min: Vec2, max: Vec2) -> Option<(f32, f32)> {
    let d = b - a;
    let (mut t0, mut t1) = (0.0f32, 1.0f32);
    for (p, q) in [
        (-d.x, a.x - min.x),
        (d.x, max.x - a.x),
        (-d.y, a.y - min.y),
        (d.y, max.y - a.y),
    ] {
        if p == 0.0 {
            if q < 0.0 {
                return None;
            }
        } else {
            let r = q / p;
            if p < 0.0 {
                t0 = t0.max(r);
            } else {
                t1 = t1.min(r);
            }
        }
    }
    (t0 < t1).then_some((t0, t1))
}

/// Parallel grid lines from `-HALF` to `HALF` as `(position, width, arterial)`. The
/// `fixed` lines come first. The other lines fill the gaps between them about `spacing`
/// meters apart.
fn grid_lines(
    seed: u32,
    spacing: f32,
    widths: (f32, f32),
    fixed: &[(f32, f32, bool)],
) -> Vec<(f32, f32, bool)> {
    let mut lines = fixed.to_vec();
    let mut edges: Vec<f32> = fixed.iter().map(|l| l.0).collect();
    edges.extend([-HALF - spacing * 0.5, HALF + spacing * 0.5]);
    edges.sort_by(f32::total_cmp);
    for (k, gap) in edges.windows(2).enumerate() {
        let n = ((gap[1] - gap[0]) / spacing).round().max(1.0) as u32;
        for i in 1..n {
            let s = seed + k as u32 * 101 + i * 7;
            let at = gap[0] + (gap[1] - gap[0]) * (i as f32 + 0.3 * (hash(s) - 0.5)) / n as f32;
            if at.abs() < HALF {
                let arterial = hash(s + 1) < 0.3;
                lines.push((at, if arterial { widths.1 } else { widths.0 }, arterial));
            }
        }
    }
    lines.sort_by(|a, b| a.0.total_cmp(&b.0));
    lines
}

/// How tall the buildings can be at `p`, in meters: towers downtown, low blocks at the
/// edges.
fn height_limit(p: Vec2) -> f32 {
    let d = p.distance(DOWNTOWN);
    24.0 + 290.0 * (-(d / 420.0).powi(2)).exp() + 70.0 * (-(d / 1150.0).powi(2)).exp()
}

/// Rounds `v` down to a multiple of `step`, at least `step`.
fn quantize(v: f32, step: f32) -> f32 {
    ((v / step).floor() * step).max(step)
}

impl Plan {
    pub fn new() -> Self {
        // The chip edge is a ring road at the left and the right, and a second road runs
        // outside the pins.
        let ring = CHIP_HALF - 9.0;
        let outer = CHIP_HALF + PIN_REACH + 16.0;
        let avenue_lines = grid_lines(
            11,
            185.0,
            (22.0, 32.0),
            &[
                (0.0, 64.0, true),
                (ring, 18.0, true),
                (-ring, 18.0, true),
                (outer, 22.0, false),
                (-outer, 22.0, false),
            ],
        );
        let street_lines = grid_lines(23, 95.0, (15.0, 26.0), &[(0.0, 20.0, true)]);
        let bands = pin_bands();

        // The streets, cut where the pins are.
        let mut streets = Vec::new();
        let mut add = |street: Street| {
            let mut pieces = vec![(street.from, street.to)];
            for (min, max) in bands {
                pieces = pieces
                    .into_iter()
                    .flat_map(|(a, b)| match clip_range(a, b, min, max) {
                        Some((t0, t1)) => vec![(a, a + (b - a) * t0), (a + (b - a) * t1, b)],
                        None => vec![(a, b)],
                    })
                    .filter(|(a, b)| a.distance(*b) > 1.0)
                    .collect();
            }
            for (from, to) in pieces {
                streets.push(Street { from, to, ..street });
            }
        };
        for &(x, width, arterial) in &avenue_lines {
            add(Street {
                from: vec2(x, -HALF),
                to: vec2(x, HALF),
                width,
                arterial,
                median: if x == 0.0 { CANAL_WIDTH } else { 0.0 },
            });
        }
        for &(z, width, arterial) in &street_lines {
            add(Street {
                from: vec2(-HALF, z),
                to: vec2(HALF, z),
                width,
                arterial,
                median: 0.0,
            });
        }
        // Two boulevards at 45 degrees, like the diagonal traces of a circuit board: the
        // lines x - z = 450 and x + z = -900.
        let diagonals = [
            Street {
                from: vec2(450.0 - HALF, -HALF),
                to: vec2(HALF, HALF - 450.0),
                width: 30.0,
                arterial: true,
                median: 0.0,
            },
            Street {
                from: vec2(-HALF, HALF - 900.0),
                to: vec2(HALF - 900.0, -HALF),
                width: 26.0,
                arterial: true,
                median: 0.0,
            },
        ];
        for d in diagonals {
            add(d);
        }

        let in_band = |p: Vec2| {
            bands
                .iter()
                .any(|(min, max)| p.cmpge(*min).all() && p.cmple(*max).all())
        };
        let mut intersections = Vec::new();
        for &(x, aw, a_art) in &avenue_lines {
            for &(z, sw, s_art) in &street_lines {
                let at = vec2(x, z);
                if !in_band(at) {
                    intersections.push(Intersection {
                        at,
                        widths: vec2(aw, sw),
                        arterial: a_art || s_art,
                    });
                }
            }
        }
        // The traffic lights in view: the first small street ahead of the flight.
        let key_crossing = street_lines
            .iter()
            .filter(|l| !l.2 && l.0 > 380.0)
            .map(|l| vec2(0.0, l.0))
            .next()
            .unwrap_or(vec2(0.0, 420.0));

        // The blocks between the streets.
        let mut blocks = Vec::new();
        for a in avenue_lines.windows(2) {
            for s in street_lines.windows(2) {
                let min = vec2(a[0].0 + a[0].1 * 0.5, s[0].0 + s[0].1 * 0.5);
                let max = vec2(a[1].0 - a[1].1 * 0.5, s[1].0 - s[1].1 * 0.5);
                let center = (min + max) * 0.5;
                if in_band(center) || max.x - min.x < 10.0 || max.y - min.y < 10.0 {
                    continue;
                }
                // A park on the left of the start, and a smaller one far out.
                let park = (center.x > -760.0
                    && center.x < -330.0
                    && center.y > -140.0
                    && center.y < 240.0)
                    || (center.x > 1050.0
                        && center.x < 1400.0
                        && center.y > 900.0
                        && center.y < 1150.0);
                blocks.push(Block {
                    min,
                    max,
                    use_: if park { Use::Park } else { Use::Buildings },
                });
            }
        }

        // The construction sites: blocks along the flight over the central boulevard.
        let mut sites = Vec::new();
        for (k, target) in [
            vec2(-110.0, 340.0),
            vec2(120.0, 520.0),
            vec2(-260.0, 1000.0),
            vec2(470.0, 120.0),
        ]
        .into_iter()
        .enumerate()
        {
            let Some(block) = blocks
                .iter_mut()
                .filter(|b| b.use_ == Use::Buildings)
                .min_by(|a, b| {
                    let ca = (a.min + a.max) * 0.5;
                    let cb = (b.min + b.max) * 0.5;
                    ca.distance(target).total_cmp(&cb.distance(target))
                })
            else {
                continue;
            };
            block.use_ = Use::Site;
            let size = block.max - block.min;
            let seed = 500 + k as u32 * 17;
            let frame = vec2(
                quantize(size.x * 0.55, 4.0 * BAY),
                quantize(size.y * 0.6, 4.0 * BAY),
            );
            let center = (block.min + block.max) * 0.5;
            // The crane stands between the frame and the boulevard, so the flight passes
            // close to it.
            let toward = if center.x > 0.0 { -1.0 } else { 1.0 };
            let crane = vec2(center.x + toward * (frame.x * 0.5 + 9.0), center.y);
            // The frames next to the flight are lower, so their cranes work at the height of
            // the camera.
            let floors = if center.x.abs() < 300.0 {
                14 + (hash(seed) * 6.0) as u32
            } else {
                18 + (hash(seed) * 18.0) as u32
            };
            sites.push(Site {
                center,
                size: frame,
                floors,
                crane,
                crane_height: floors as f32 * FLOOR + 30.0 + hash(seed + 1) * 12.0,
                seed,
            });
        }

        // The buildings: each block splits into lots, each lot gets one building.
        let mut buildings = Vec::new();
        for (bi, block) in blocks.iter().enumerate() {
            if block.use_ != Use::Buildings {
                continue;
            }
            let mut lots = Vec::new();
            let sidewalk = 5.0;
            split_lot(
                block.min + Vec2::splat(sidewalk),
                block.max - Vec2::splat(sidewalk),
                bi as u32 * 977,
                &mut lots,
            );
            for (li, (min, max)) in lots.into_iter().enumerate() {
                let seed = bi as u32 * 7919 + li as u32 * 104_729;
                let center = (min + max) * 0.5;
                // Keep the diagonal boulevards free.
                let half = (max - min) * 0.5;
                if diagonals
                    .iter()
                    .any(|d| d.distance(center) < d.width * 0.5 + half.length() + 4.0)
                {
                    continue;
                }
                if let Some(building) = building(center, max - min, seed) {
                    buildings.push(building);
                }
            }
        }
        // The pins: long low buildings in two rows.
        for side in [-1.0f32, 1.0] {
            for k in 0..PINS {
                let seed = 90_000 + k as u32 * 31 + u32::from(side > 0.0) * 7;
                let height = quantize(20.0 + 16.0 * hash(seed), FLOOR);
                buildings.push(Building {
                    center: vec2(
                        side * (CHIP_HALF + PIN_REACH * 0.5),
                        (k as f32 - (PINS - 1) as f32 * 0.5) * PIN_PITCH,
                    ),
                    tiers: vec![Tier {
                        size: vec2(PIN_REACH, 8.0 * BAY),
                        top: height,
                    }],
                    style: Style::Office,
                    seed,
                    pin: true,
                });
            }
        }

        // The video screens: on the face toward the flight of five towers along it.
        let mut screens: Vec<Screen> = Vec::new();
        for target in [
            vec2(-70.0, 250.0),
            vec2(80.0, 380.0),
            vec2(-60.0, 520.0),
            vec2(90.0, 660.0),
            vec2(-120.0, 800.0),
        ] {
            let candidate = buildings
                .iter()
                .enumerate()
                .filter(|(i, b)| {
                    b.height() > 110.0
                        && b.tiers[0].size.x >= SCREEN.x + 4.0
                        && b.tiers[0].top > 70.0
                        && screens.iter().all(|s| s.building != *i)
                })
                .min_by(|a, b| {
                    a.1.center
                        .distance(target)
                        .total_cmp(&b.1.center.distance(target))
                });
            if let Some((i, b)) = candidate {
                let base = b.tiers[0];
                screens.push(Screen {
                    building: i,
                    at: b.center - vec2(0.0, base.size.y * 0.5 + 0.8),
                    height: (base.top - SCREEN.y * 0.5 - 8.0).min(105.0),
                    facing: vec2(0.0, -1.0),
                });
            }
        }

        Self {
            streets,
            blocks,
            buildings,
            sites,
            screens,
            intersections,
            key_crossing,
        }
    }
}

/// Splits a lot in two along its longer side until it is small enough. Small lots are for
/// low buildings, so the limit grows with the height that is possible there.
fn split_lot(min: Vec2, max: Vec2, seed: u32, out: &mut Vec<(Vec2, Vec2)>) {
    let size = max - min;
    let limit = 26.0 + height_limit((min + max) * 0.5) * 0.16;
    let long = size.max_element();
    if long <= limit || long < 22.0 {
        out.push((min, max));
        return;
    }
    let gap = 4.0 + 4.0 * hash(seed);
    let at = 0.35 + 0.3 * hash(seed + 1);
    if size.x >= size.y {
        let cut = min.x + size.x * at;
        split_lot(min, vec2(cut - gap * 0.5, max.y), seed * 3 + 1, out);
        split_lot(vec2(cut + gap * 0.5, min.y), max, seed * 3 + 2, out);
    } else {
        let cut = min.y + size.y * at;
        split_lot(min, vec2(max.x, cut - gap * 0.5), seed * 3 + 1, out);
        split_lot(vec2(min.x, cut + gap * 0.5), max, seed * 3 + 2, out);
    }
}

/// The building on a lot of `size` meters, or `None` for an empty lot.
fn building(center: Vec2, size: Vec2, seed: u32) -> Option<Building> {
    if size.min_element() < 9.0 || hash(seed + 3) < 0.04 {
        return None;
    }
    let limit = height_limit(center);
    // Downtown, some lots get a tower. The others get blocks of medium height, so the
    // towers stand out of a lower city.
    let tower =
        size.min_element() > 26.0 && hash(seed + 2) < ((limit - 90.0) / 260.0).clamp(0.0, 0.5);
    let height = if tower {
        quantize(limit * (0.5 + 0.5 * hash(seed).powf(0.7)), FLOOR).max(100.0)
    } else {
        quantize(12.0 + limit.min(120.0) * (0.15 + 0.6 * hash(seed)), FLOOR)
    };
    // A tower stands free on its lot. A lower building fills the lot.
    let mut footprint = if tower {
        let side = (26.0 + height * 0.1).min(size.min_element());
        vec2(side, side) + (size - Vec2::splat(side)).max(Vec2::ZERO) * 0.5 * hash(seed + 5)
    } else {
        size
    };
    footprint = vec2(quantize(footprint.x, BAY), quantize(footprint.y, BAY));
    let style = if tower {
        if hash(seed + 7) < 0.5 {
            Style::Glass
        } else {
            Style::Fins
        }
    } else if height > 36.0 {
        if hash(seed + 7) < 0.3 {
            Style::Glass
        } else {
            Style::Office
        }
    } else {
        Style::Low
    };
    // Towers step back two or three times, like the towers of the 1930s.
    let mut tiers = Vec::new();
    let step = 4.0 * BAY;
    if tower && hash(seed + 11) < 0.7 {
        let steps = if height > 180.0 { 3 } else { 2 };
        let mut base = footprint;
        for s in 0..steps {
            let top = if s + 1 == steps {
                height
            } else {
                quantize(
                    height * (0.55 + 0.17 * s as f32 + 0.1 * hash(seed + 13 + s)),
                    FLOOR,
                )
            };
            tiers.push(Tier { size: base, top });
            base = (base - Vec2::splat(step)).max(Vec2::splat(step));
        }
    } else if height > 40.0 && hash(seed + 17) < 0.35 {
        // A podium with a narrower block on top.
        let podium = quantize(height * 0.3, FLOOR);
        tiers.push(Tier {
            size: footprint,
            top: podium,
        });
        tiers.push(Tier {
            size: (footprint - Vec2::splat(step)).max(Vec2::splat(step)),
            top: height,
        });
    } else {
        tiers.push(Tier {
            size: footprint,
            top: height,
        });
    }
    Some(Building {
        center,
        tiers,
        style,
        seed,
        pin: false,
    })
}
