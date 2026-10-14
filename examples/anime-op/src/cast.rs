//! The cast: the author of fff, fframes, odiff and zlob and the people he talks to every day.

use fframes_anime::glam::Vec3;
use fframes_anime::rig::{Proportions, Skeleton};
use fframes_anime::toon::{Clump, Design, Glasses, Hair};

use crate::palette;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Who {
    /// Dmitriy Kovalenko, @neogoose_btw: fff, fframes, odiff, zlob.
    Neogoose,
    /// Dmitrii Kovanikov, @chshersh: OCaml, Haskell.
    Chshersh,
    /// Shreyas Mididoddi, @Shreyassanthu77: zig evangelist.
    Shreyas,
    /// Dillon Mulroy, @dillon_mulroy.
    Dillon,
    /// Theo Browne, @theo: T3 Code searches with fff.
    Theo,
    /// TJ DeVries, @teej_dv: Neovim core.
    Teej,
    /// The rivals: hooded figures named after what slows software down.
    Bloat,
    Flaky,
    Segfault,
    Latency,
    /// The giant at the end: the Bloat in its true form.
    Demon,
}

pub const RIVALS: [Who; 4] = [Who::Bloat, Who::Flaky, Who::Segfault, Who::Latency];

pub const ALL: [Who; 6] = [
    Who::Neogoose,
    Who::Chshersh,
    Who::Shreyas,
    Who::Dillon,
    Who::Theo,
    Who::Teej,
];

pub struct Member {
    pub who: Who,
    pub name: &'static str,
    pub handle: &'static str,
    pub katakana: &'static str,
    pub design: Design,
    pub proportions: Proportions,
    /// Hair colour in two-tone shots (`None` = ink).
    pub hair: Option<&'static str>,
    pub beard: Option<&'static str>,
    /// Their colour in solo shots.
    pub accent: &'static str,
}

impl Member {
    pub fn skeleton(&self) -> Skeleton {
        Skeleton::new(self.proportions)
    }
}

/// Hairline plane of a regular haircut (forehead at the front, nape at the back).
fn hairline(front_y: f32, back_y: f32) -> (Vec3, f32) {
    // Points relative to the skull centre (0, 0.12, -0.005).
    let f = Vec3::new(0.0, front_y - 0.12, 0.095);
    let b = Vec3::new(0.0, back_y - 0.12, -0.095);
    let s = Vec3::new(0.1, (front_y + back_y) * 0.5 - 0.12, 0.005);
    let n = (s - f).cross(b - f).normalize();
    let n = if n.y < 0.0 { -n } else { n };
    (n, n.dot(f))
}

fn clump(
    root: [f32; 3],
    dir: [f32; 3],
    length: f32,
    width: f32,
    curl: f32,
    stiffness: f32,
) -> Clump {
    Clump {
        root: Vec3::from(root),
        dir: Vec3::from(dir),
        length,
        width,
        curl,
        stiffness,
    }
}

/// A point on the skull (head frame): `az` degrees from the front towards the character's left,
/// `el` degrees up from the skull's equator.
pub fn scalp(az: f32, el: f32) -> Vec3 {
    let (a, e) = (az.to_radians(), el.to_radians());
    let c = Vec3::new(0.0, 0.127, -0.012);
    let r = Vec3::new(0.087, 0.097, 0.1);
    c + Vec3::new(
        e.cos() * a.sin() * r.x,
        e.sin() * r.y,
        e.cos() * a.cos() * r.z,
    )
}

/// A lock of hair growing from the scalp at (`az`, `el`), starting along `dir` (head frame) and
/// bending `curl` radians per link towards the head.
fn lock(
    az: f32,
    el: f32,
    dir: [f32; 3],
    length: f32,
    width: f32,
    curl: f32,
    stiffness: f32,
) -> Clump {
    clump(
        scalp(az, el).to_array(),
        dir,
        length,
        width,
        curl,
        stiffness,
    )
}

/// Direction pointing down the head at azimuth `az` (tilted outwards by `out`).
fn down_at(az: f32, out: f32) -> [f32; 3] {
    let a = az.to_radians();
    [a.sin() * out, -1.0, a.cos() * out]
}

pub fn member(who: Who) -> Member {
    let base = Proportions::anime_adult();
    match who {
        Who::Neogoose => {
            // A ginger quiff: narrow pointed locks rise off the forehead and sweep back over the
            // crown, the back and sides combed back.
            let (n, o) = hairline(0.172, 0.06);
            let mut clumps = Vec::new();
            for (i, az) in [-52.0_f32, -36.0, -20.0, -5.0, 10.0, 25.0, 40.0, 55.0]
                .iter()
                .enumerate()
            {
                let len = 0.13 + ((i * 5) % 3) as f32 * 0.018;
                clumps.push(lock(
                    *az,
                    28.0 - az.abs() * 0.1,
                    [az * 0.012, 1.0, -0.12],
                    len,
                    0.05,
                    0.6,
                    260.0,
                ));
            }
            for (i, az) in [-48.0_f32, -24.0, 0.0, 24.0, 48.0].iter().enumerate() {
                clumps.push(lock(
                    *az,
                    50.0,
                    [az * 0.01, 0.55, -0.85],
                    0.11 + (i % 2) as f32 * 0.02,
                    0.05,
                    0.45,
                    200.0,
                ));
            }
            for az in [120.0_f32, 145.0, 170.0, 195.0, 220.0, 245.0] {
                clumps.push(lock(az, 40.0, down_at(az, 0.2), 0.1, 0.042, 0.45, 150.0));
            }
            for s in [1.0_f32, -1.0] {
                clumps.push(lock(
                    80.0 * s,
                    25.0,
                    [0.0, 0.15, -1.0],
                    0.075,
                    0.04,
                    0.5,
                    120.0,
                ));
            }
            Member {
                who,
                name: "Dmitriy Kovalenko",
                handle: "@neogoose_btw",
                katakana: "ネオグース",
                design: Design {
                    hair: Hair {
                        cap_normal: n,
                        cap_offset: o,
                        volume: 1.03,
                        fringe: 0.0,
                        clumps,
                    },
                    glasses: None,
                    beard: 0.75,
                    robe: true,
                    scarf: false,
                    tie: true,
                    robe_length: 1.20,
                    build: 1.04,
                    muscles: false,
                    hood: false,
                    slime: false,
                },
                proportions: base,
                hair: Some(palette::ORANGE),
                beard: Some(palette::ORANGE),
                accent: palette::ORANGE,
            }
        }
        Who::Chshersh => {
            // Side part on his right, a heavy fringe swept across the forehead to his left.
            let (n, o) = hairline(0.165, 0.07);
            let mut clumps = Vec::new();
            for (i, el) in [56.0_f32, 47.0, 38.0, 29.0, 20.0].iter().enumerate() {
                clumps.push(lock(
                    -28.0 + i as f32 * 5.0,
                    *el,
                    [1.0, -0.3, 0.45],
                    0.125 - i as f32 * 0.008,
                    0.06,
                    0.55,
                    110.0,
                ));
            }
            for el in [58.0_f32, 42.0, 26.0] {
                clumps.push(lock(
                    -42.0,
                    el,
                    [-1.0, -0.25, 0.1],
                    0.075,
                    0.056,
                    0.5,
                    120.0,
                ));
            }
            for az in [125.0_f32, 160.0, 195.0, 230.0] {
                clumps.push(lock(az, 38.0, down_at(az, 0.35), 0.11, 0.062, 0.3, 90.0));
            }
            for s in [1.0_f32, -1.0] {
                clumps.push(lock(
                    85.0 * s,
                    24.0,
                    [0.25 * s, -1.0, 0.0],
                    0.07,
                    0.05,
                    0.3,
                    100.0,
                ));
            }
            Member {
                who,
                name: "Dmitrii Kovanikov",
                handle: "@chshersh",
                katakana: "コワニコフ",
                design: Design {
                    hair: Hair {
                        cap_normal: n,
                        cap_offset: o,
                        volume: 1.07,
                        fringe: 0.022,
                        clumps,
                    },
                    glasses: Some(Glasses::Rect),
                    beard: 0.0,
                    robe: true,
                    scarf: false,
                    tie: true,
                    robe_length: 1.20,
                    build: 1.06,
                    muscles: false,
                    hood: false,
                    slime: false,
                },
                proportions: base.scaled(0.98),
                hair: None,
                beard: None,
                accent: palette::BLUE,
            }
        }
        Who::Shreyas => {
            // Short, soft and wavy: combed forward, the tips at the front flick up.
            let (n, o) = hairline(0.17, 0.085);
            let mut clumps = Vec::new();
            for (i, az) in [-48.0_f32, -24.0, 0.0, 24.0, 48.0].iter().enumerate() {
                clumps.push(lock(
                    *az,
                    48.0,
                    [az * 0.006, 0.1, 1.0],
                    0.095 + (i % 2) as f32 * 0.012,
                    0.06,
                    0.3,
                    90.0,
                ));
            }
            for az in [-55.0_f32, -18.0, 18.0, 55.0] {
                clumps.push(lock(
                    az,
                    66.0,
                    [az * 0.008, 0.35, -1.0],
                    0.085,
                    0.064,
                    0.32,
                    80.0,
                ));
            }
            for az in [135.0_f32, 158.0, 180.0, 202.0, 225.0] {
                clumps.push(lock(az, 32.0, down_at(az, 0.25), 0.100, 0.046, 0.45, 80.0));
            }
            for s in [1.0_f32, -1.0] {
                clumps.push(lock(
                    80.0 * s,
                    26.0,
                    [0.2 * s, -0.8, -0.6],
                    0.065,
                    0.05,
                    0.3,
                    90.0,
                ));
            }
            Member {
                who,
                name: "Shreyas Mididoddi",
                handle: "@Shreyassanthu77",
                katakana: "シュレヤス",
                design: Design {
                    hair: Hair {
                        cap_normal: n,
                        cap_offset: o,
                        volume: 1.09,
                        fringe: 0.016,
                        clumps,
                    },
                    glasses: Some(Glasses::Round),
                    beard: 0.0,
                    robe: true,
                    scarf: false,
                    tie: true,
                    robe_length: 1.15,
                    build: 0.98,
                    muscles: false,
                    hood: false,
                    slime: false,
                },
                proportions: base.scaled(0.97),
                hair: Some(palette::WHITE),
                beard: Some(palette::WHITE),
                accent: palette::TEAL,
            }
        }
        Who::Dillon => {
            // Swept back with volume on top (a soft pompadour), one lock falling forward.
            let (n, o) = hairline(0.18, 0.075);
            let mut clumps = Vec::new();
            for (i, az) in [-34.0_f32, -12.0, 12.0, 34.0].iter().enumerate() {
                clumps.push(lock(
                    *az,
                    30.0,
                    [az * 0.008, 1.0, -0.1],
                    0.16 - (i % 2) as f32 * 0.02,
                    0.074,
                    0.68,
                    150.0,
                ));
            }
            for s in [1.0_f32, -1.0] {
                clumps.push(lock(
                    68.0 * s,
                    26.0,
                    [0.1 * s, 0.2, -1.0],
                    0.11,
                    0.055,
                    0.3,
                    120.0,
                ));
            }
            for az in [135.0_f32, 158.0, 180.0, 202.0, 225.0] {
                clumps.push(lock(az, 35.0, down_at(az, 0.25), 0.110, 0.046, 0.45, 110.0));
            }
            clumps.push(lock(14.0, 36.0, [0.3, -0.2, 1.0], 0.11, 0.032, 0.6, 28.0));
            Member {
                who,
                name: "Dillon Mulroy",
                handle: "@dillon_mulroy",
                katakana: "ディロン",
                design: Design {
                    hair: Hair {
                        cap_normal: n,
                        cap_offset: o,
                        volume: 1.07,
                        fringe: 0.012,
                        clumps,
                    },
                    glasses: None,
                    beard: 0.0,
                    robe: true,
                    scarf: false,
                    tie: true,
                    robe_length: 1.23,
                    build: 1.02,
                    muscles: false,
                    hood: false,
                    slime: false,
                },
                proportions: base.scaled(1.01),
                hair: None,
                beard: None,
                accent: palette::GREEN_LIGHT,
            }
        }
        Who::Theo => {
            // A messy fringe falling over the forehead: soft, it swings.
            let (n, o) = hairline(0.165, 0.06);
            let mut clumps = Vec::new();
            for (i, az) in [-52.0_f32, -31.0, -10.0, 10.0, 31.0, 52.0]
                .iter()
                .enumerate()
            {
                clumps.push(lock(
                    *az,
                    52.0,
                    [az * 0.008, 0.2, 1.0],
                    0.15 + (i % 3) as f32 * 0.015,
                    0.058,
                    0.62,
                    26.0 + (i % 2) as f32 * 14.0,
                ));
            }
            for az in [-62.0_f32, -21.0, 21.0, 62.0] {
                clumps.push(lock(
                    az,
                    72.0,
                    [az * 0.01, 0.3, -1.0],
                    0.1,
                    0.064,
                    0.42,
                    45.0,
                ));
            }
            for az in [135.0_f32, 158.0, 180.0, 202.0, 225.0] {
                clumps.push(lock(az, 35.0, down_at(az, 0.25), 0.120, 0.046, 0.45, 60.0));
            }
            for s in [1.0_f32, -1.0] {
                clumps.push(lock(
                    84.0 * s,
                    26.0,
                    [0.2 * s, -1.0, 0.1],
                    0.085,
                    0.052,
                    0.3,
                    60.0,
                ));
            }
            Member {
                who,
                name: "Theo Browne",
                handle: "@theo",
                katakana: "テオ",
                design: Design {
                    hair: Hair {
                        cap_normal: n,
                        cap_offset: o,
                        volume: 1.1,
                        fringe: 0.03,
                        clumps,
                    },
                    glasses: None,
                    beard: 0.0,
                    robe: true,
                    scarf: false,
                    tie: true,
                    robe_length: 1.25,
                    build: 0.97,
                    muscles: false,
                    hood: false,
                    slime: false,
                },
                proportions: base.scaled(1.02),
                hair: None,
                beard: None,
                accent: palette::MAGENTA,
            }
        }
        Who::Bloat | Who::Flaky | Who::Segfault | Who::Latency => rival(who),
        Who::Demon => demon(),
        Who::Teej => {
            // Short: a side-swept fringe and a few spikes on top.
            let (n, o) = hairline(0.175, 0.085);
            let mut clumps = Vec::new();
            for (i, az) in [-40.0_f32, -20.0, 0.0, 20.0].iter().enumerate() {
                clumps.push(lock(
                    *az,
                    36.0,
                    [0.85, -0.05, 0.55],
                    0.095 + (i % 2) as f32 * 0.012,
                    0.055,
                    0.42,
                    120.0,
                ));
            }
            for az in [-35.0_f32, 5.0, 45.0] {
                clumps.push(lock(
                    az,
                    68.0,
                    [az * 0.008, 0.35, -1.0],
                    0.075,
                    0.055,
                    0.4,
                    140.0,
                ));
            }
            for az in [135.0_f32, 158.0, 180.0, 202.0, 225.0] {
                clumps.push(lock(az, 35.0, down_at(az, 0.25), 0.095, 0.046, 0.45, 120.0));
            }
            for s in [1.0_f32, -1.0] {
                clumps.push(lock(
                    80.0 * s,
                    25.0,
                    [0.15 * s, -0.4, -1.0],
                    0.065,
                    0.05,
                    0.3,
                    110.0,
                ));
            }
            Member {
                who,
                name: "TJ DeVries",
                handle: "@teej_dv",
                katakana: "ティージェイ",
                design: Design {
                    hair: Hair {
                        cap_normal: n,
                        cap_offset: o,
                        volume: 1.06,
                        fringe: 0.02,
                        clumps,
                    },
                    glasses: None,
                    beard: 0.0,
                    robe: true,
                    scarf: false,
                    tie: true,
                    robe_length: 1.20,
                    build: 1.03,
                    muscles: false,
                    hood: false,
                    slime: false,
                },
                proportions: base,
                hair: None,
                beard: None,
                accent: palette::MOSS,
            }
        }
    }
}

/// A hooded rival: the hood is a hair volume pulled down over the eyes, no face but the eyes.
fn rival(who: Who) -> Member {
    let base = Proportions::anime_adult();
    let (name, katakana, build, scale, spikes) = match who {
        Who::Bloat => ("BLOAT", "ブロート", 1.55, 1.08, 0),
        Who::Flaky => ("FLAKY TEST", "フレーキー", 0.85, 1.06, 3),
        Who::Segfault => ("SEGFAULT", "セグフォルト", 1.0, 1.0, 6),
        _ => ("LATENCY", "レイテンシ", 0.95, 1.12, 1),
    };
    let mut clumps = Vec::new();
    for i in 0..spikes {
        let x = -0.06 + i as f32 * 0.12 / spikes.max(1) as f32;
        clumps.push(clump(
            [x, 0.22, -0.02],
            [x * 4.0, 1.0, -0.4],
            0.12,
            0.07,
            -0.2,
            300.0,
        ));
    }
    // the hood frames the face from the brow to under the jaw
    let (hood_n, hood_o) = hairline(0.165, -0.08);
    // the hood's point hangs behind
    clumps.push(clump(
        [0.0, 0.22, -0.06],
        [0.0, 0.3, -1.0],
        0.2,
        0.12,
        0.7,
        22.0,
    ));
    Member {
        who,
        name,
        handle: "",
        katakana,
        design: Design {
            hair: Hair {
                cap_normal: hood_n,
                cap_offset: hood_o,
                volume: 1.26,
                fringe: 0.0,
                clumps,
            },
            glasses: None,
            beard: 0.0,
            robe: true,
            scarf: false,
            tie: false,
            robe_length: 1.40,
            build,
            muscles: false,
            hood: true,
            slime: false,
        },
        proportions: base.scaled(scale),
        hair: None,
        beard: None,
        accent: palette::RED,
    }
}

/// A horned, muscular giant (drawn huge by its own camera).
fn demon() -> Member {
    let (n, o) = hairline(0.2, 0.1);
    // goo tendrils sliding off the top and back of the head; they swing with the springs
    let mut clumps = Vec::new();
    for (i, (x, z)) in [
        (-0.06_f32, -0.02_f32),
        (0.0, -0.06),
        (0.06, -0.03),
        (0.08, 0.02),
        (-0.09, 0.0),
    ]
    .into_iter()
    .enumerate()
    {
        clumps.push(clump(
            [x, 0.2, z],
            [x * 4.0, -0.3, -0.6 + z * 4.0],
            0.12 + 0.05 * (i % 3) as f32,
            0.05,
            -0.2,
            30.0,
        ));
    }
    Member {
        who: Who::Demon,
        name: "SLOP",
        handle: "",
        katakana: "スロップ",
        design: Design {
            hair: Hair {
                cap_normal: n,
                cap_offset: o,
                volume: 1.04,
                fringe: 0.03,
                clumps,
            },
            glasses: None,
            beard: 0.0,
            robe: false,
            scarf: false,
            tie: false,
            robe_length: 1.15,
            build: 1.3,
            muscles: true,
            hood: false,
            slime: true,
        },
        // bottom-heavy goo: short stumpy legs, long gorilla arms, a big head sunk into the
        // shoulders
        proportions: Proportions {
            hip_height: 0.76,
            hip_width: 0.14,
            spine: 0.16,
            chest: 0.22,
            neck: 0.1,
            head_offset: 0.05,
            head_height: 0.36,
            clavicle: 0.05,
            shoulder_width: 0.27,
            upper_arm: 0.36,
            forearm: 0.34,
            hand: 0.22,
            thigh: 0.32,
            shin: 0.3,
            ..Proportions::anime_adult()
        },
        hair: None,
        beard: None,
        accent: palette::YELLOW,
    }
}
