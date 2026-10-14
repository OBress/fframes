//! 44-50.5 s: TJ jumps with his dog, the keyboard split screen, the teal pair, the strobe of
//! reveals and the flood of geese.

use fframes::{FFramesContext, Frame, Svgr};
use fframes_anime::camera::Camera;
use fframes_anime::glam::{Vec2, Vec3};
use fframes_anime::rig::{Bone, Side};
use fframes_anime::shape::Path;
use fframes_anime::toon::{HandShape, Look, Mouth};

use crate::actor::{Actor, acting, look};
use crate::cast::{ALL, Who, member};
use crate::expr::{self, Acting};
use crate::fx::{self, bg, ease_out, hash2, twos};
use crate::motion::{Library, Play, clip, in_place};
use crate::palette::{self, Paint};
use crate::procedural::{Arms, Body, build, motion};
use crate::rigid::{self, Baked2, Body2, Kinematic, Shape2};
use crate::shots::title::{GOTHIC, SANS};
use crate::sprites;
use crate::{H, Shot, W};

/// Height of a jump at `t` (s): crouch, launch, airborne, land.
fn jump_arc(t: f32, at: f32, air: f32) -> (f32, f32) {
    let u = t - at;
    if u < -0.12 {
        (0.0, 0.0)
    } else if u < 0.0 {
        // crouch
        (0.0, (u + 0.12) / 0.12)
    } else if u < air {
        let k = u / air;
        (4.0 * k * (1.0 - k) * 0.75, 0.0)
    } else if u < air + 0.1 {
        (0.0, 1.0 - (u - air) / 0.1)
    } else {
        (0.0, 0.0)
    }
}

/// TJ and his dog jump, "nvim" in teal behind them on orange.
pub struct Jump {
    tj: Actor,
}

const JUMP_AT: f32 = 0.12;
const AIR: f32 = 0.42;

impl Jump {
    pub fn new(len: f32) -> Self {
        let sk = member(Who::Teej).skeleton();
        let hip = sk.proportions.hip_height;
        let m = motion(move |t| {
            let (h, crouch) = jump_arc(t, JUMP_AT, AIR);
            let airborne = h > 0.0;
            let body = Body {
                root: Vec3::new(
                    0.0,
                    hip - 0.18 * crouch + h - if airborne { 0.12 } else { 0.0 },
                    0.0,
                ),
                yaw: -20.0,
                spine: Vec3::new(0.0, 12.0 * crouch, 0.0),
                head: Vec3::new(0.0, -10.0 * crouch, 0.0),
                ..Default::default()
            };
            let mut pose = build(
                &sk,
                body,
                Arms {
                    left: Some(if airborne {
                        Vec3::new(0.32, 0.55, 0.15)
                    } else {
                        Vec3::new(0.25, -0.3, 0.1)
                    }),
                    right: Some(if airborne {
                        Vec3::new(-0.2, 0.62, 0.12)
                    } else {
                        Vec3::new(-0.25, -0.3, 0.1)
                    }),
                    elbow_out: 1.0,
                    ..Default::default()
                },
                if airborne {
                    None
                } else {
                    Some((Vec3::ZERO, -20.0, 0.16, 0.02))
                },
            );
            if airborne {
                // tuck the legs
                for side in [Side::Left, Side::Right] {
                    let s = side.sign();
                    let solved = sk.solve(&pose);
                    let hipj = solved.pos(Bone::leg(side)[0]);
                    let target = hipj + Vec3::new(0.08 * s, -0.42, 0.25 + 0.1 * s);
                    fframes_anime::rig::two_bone_ik(
                        &sk,
                        &mut pose,
                        Bone::leg(side)[0],
                        target,
                        hipj + Vec3::new(0.0, 0.0, 1.0),
                        None,
                    );
                }
            }
            pose
        });
        let looks = acting(
            Acting::new(Look {
                hand_l: HandShape::Open,
                hand_r: HandShape::Peace,
                ..Default::default()
            })
            // crouching for it: a grin of anticipation
            .then(0.0, |l| Look {
                mouth: Mouth::Grin,
                sparkle: 0.7,
                ..expr::determined(l)
            })
            // up: a whoop, eyes squeezed shut
            .then(JUMP_AT, |l| Look {
                mouth: Mouth::Open(1.0),
                ..expr::laugh(l)
            }),
        );
        Jump {
            tj: Actor::new(Who::Teej, m, looks, len, 2, Vec3::ZERO),
        }
    }
}

impl Shot for Jump {
    fn render<'a>(
        &'a self,
        t: f32,
        _len: f32,
        _f: &Frame,
        _c: &FFramesContext<'a, '_>,
    ) -> Svgr<'a> {
        let th = twos(t);
        let cam = Camera::new(
            Vec3::new(0.6, 1.2, 4.6),
            Vec3::new(0.6, 1.45, 0.0),
            34.0,
            W,
            H,
        );
        // the dog jumps a beat later; its ears and tail lag behind its vertical speed
        let (dh, _) = jump_arc(th, JUMP_AT + 0.05, AIR);
        let (dh2, _) = jump_arc(th + 0.04, JUMP_AT + 0.05, AIR);
        let lift = (dh2 - dh) * 18.0;
        let dog_c = cam.to_screen(Vec3::new(1.35, 0.42 + dh * 1.1, 0.2));
        let dog = sprites::dog(
            dog_c,
            0.62 * cam.project(Vec3::new(1.35, 0.4, 0.2)).scale,
            -1.0,
            th * 2.5,
            lift,
        );
        let word = ease_out(th / 0.12);
        fframes::svgr!(
            <g>
                {bg(palette::ORANGE)}
                <text x="960" y="760" font-family={GOTHIC} font-size={560.0 * (0.8 + 0.2 * word)} text-anchor="middle" fill={palette::TEAL} opacity={word}>"nvim"</text>
                {self.tj.draw(t, &cam, &Paint::two_tone(palette::ORANGE))}
                {fx::path(dog.build(), palette::INK)}
            </g>
        )
    }
}

// ---------------------------------------------------------------------------------------------

/// Split screen: TJ hammering a keyboard (keycaps flying off) on orange, scrolling code on
/// green with a big V.
pub struct Keys {
    tj: Actor,
    /// One flight per key, from its own keystroke on.
    caps: Vec<Baked2>,
    labels: Vec<char>,
}

/// TJ's keystrokes (shot seconds): the 13 onsets of the vocal roll, one key each.
const STROKES: [f32; 13] = [
    0.01, 0.20, 0.43, 0.59, 0.78, 0.94, 1.15, 1.34, 1.52, 1.73, 1.89, 2.10, 2.31,
];

/// How far a hand is lifted for a keystroke at `at`: up two drawings before, slammed down on it.
fn strike(t: f32, at: f32) -> f32 {
    match t - at {
        d if (-0.17..-0.085).contains(&d) => 0.6,
        d if (-0.085..0.0).contains(&d) => 1.0,
        _ => 0.0,
    }
}

impl Keys {
    pub fn new(len: f32) -> Self {
        let sk = member(Who::Teej).skeleton();
        let hip = sk.proportions.hip_height;
        let m = motion(move |t| {
            // the hands take turns: each lifts and slams down on its keystrokes
            let lift = |side: usize| {
                STROKES
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| i % 2 == side)
                    .map(|(_, at)| strike(t, *at))
                    .fold(0.0_f32, f32::max)
            };
            let body = Body {
                root: Vec3::new(0.0, hip - 0.3, 0.0),
                spine: Vec3::new(0.0, 18.0, 0.0),
                chest: Vec3::new(0.0, 10.0, 0.0),
                head: Vec3::new(0.0, 14.0, 0.0),
                ..Default::default()
            };
            build(
                &sk,
                body,
                Arms {
                    left: Some(Vec3::new(0.16, -0.18 + 0.12 * lift(0), 0.42)),
                    right: Some(Vec3::new(-0.16, -0.18 + 0.12 * lift(1), 0.42)),
                    elbow_out: 1.1,
                    left_palm: Some(Vec3::new(0.0, -1.0, 0.0)),
                    right_palm: Some(Vec3::new(0.0, -1.0, 0.0)),
                    left_fingers: None,
                    right_fingers: None,
                },
                None,
            )
        });
        let tj = Actor::new(
            Who::Teej,
            m,
            look(Look {
                face: false,
                hand_l: HandShape::Relaxed,
                hand_r: HandShape::Relaxed,
                ..Default::default()
            }),
            len,
            2,
            Vec3::ZERO,
        );
        let labels: Vec<char> = "hjkl:wq!iddgg".chars().collect();
        // each key flies out of the keyboard on its keystroke and lands on the desk
        let walls = [
            (Vec2::new(480.0, 890.0), Vec2::new(700.0, 60.0)),
            (Vec2::new(-60.0, 0.0), Vec2::new(60.0, 3000.0)),
            (Vec2::new(1000.0, 0.0), Vec2::new(40.0, 3000.0)),
        ];
        let caps = (0..labels.len())
            .map(|i| {
                let x = 120.0 + i as f32 * 60.0;
                let mut b = Body2::ball(Vec2::new(x, 760.0), 40.0);
                b.shape = Shape2::Box(42.0, 38.0);
                b.vel = Vec2::new(
                    (hash2(402, i as u32) - 0.3) * 500.0,
                    -1700.0 - hash2(403, i as u32) * 700.0,
                );
                b.spin = (hash2(404, i as u32) - 0.5) * 16.0;
                b.restitution = 0.45;
                rigid::bake(&[b], &walls, &[], 4200.0, len, 24.0)
            })
            .collect();
        Keys { tj, caps, labels }
    }
}

impl Shot for Keys {
    fn render<'a>(
        &'a self,
        t: f32,
        _len: f32,
        _f: &Frame,
        _c: &FFramesContext<'a, '_>,
    ) -> Svgr<'a> {
        let th = twos(t);
        let mut cam = Camera::orbit(Vec3::new(0.0, 0.95, 0.0), 2.6, 60.0, 12.0, 30.0, W, H);
        cam.shift = Vec2::new(-470.0, 40.0);
        let typed = STROKES.iter().filter(|s| t >= **s).count();
        // the keys in flight, on ones: they are thrown, not drawn
        let caps: Vec<Svgr<'a>> = self
            .caps
            .iter()
            .zip(&self.labels)
            .zip(STROKES)
            .filter(|(_, at)| t >= *at)
            .map(|((baked, l), at)| {
                let p = baked.at(t - at)[0];
                sprites::keycap(p.pos, 84.0, p.angle, *l)
            })
            .collect();
        // the keyboard on the desk, a key gone from it for every keystroke
        let slots: Vec<Svgr<'a>> = (0..13)
            .map(|i| {
                let x = 92.0 + i as f32 * 60.0;
                let fill = if i < typed { palette::INK } else { "#3A2A1A" };
                fframes::svgr!(<rect x={x} y="790" width="52" height="34" rx="6" fill={fill} stroke="#5A4630" stroke-width="3" />)
            })
            .collect();
        // the code being typed, a line per keystroke, the cursor blinking at its end
        let code = [
            "local fff = require('fff')",
            "fff.setup({",
            "  max_threads = 8,",
            "})",
            "vim.keymap.set('n', 'ff',",
            "  fff.find_files)",
            "vim.keymap.set('n', 'fg',",
            "  fff.grep)",
            "-- fd → fff",
            "-- 11,068 stars",
            ":w",
            ":wq",
            ":wq!",
        ];
        let first = typed.saturating_sub(9);
        let lines: Vec<Svgr<'a>> = (first..typed)
            .map(|i| {
                let y = 150.0 + (i - first) as f32 * 92.0;
                let mut s = code[i].to_owned();
                if i + 1 == typed && (th * 4.0).floor() as i32 % 2 == 0 {
                    s.push('_');
                }
                fframes::svgr!(<text x="1010" y={y} font-family={SANS} font-weight="900" font-size="50" fill="#D8FFB0" stroke={palette::INK} stroke-width="8" paint-order="stroke">{s}</text>)
            })
            .collect();
        let mut vee = Path::new();
        vee.polygon(&[
            Vec2::new(1270.0, 220.0),
            Vec2::new(1420.0, 220.0),
            Vec2::new(1460.0, 640.0),
            Vec2::new(1560.0, 220.0),
            Vec2::new(1710.0, 220.0),
            Vec2::new(1530.0, 860.0),
            Vec2::new(1390.0, 860.0),
        ]);
        fframes::svgr!(
            <g>
                {bg(palette::ORANGE)}
                <rect x="0" y="830" width="960" height="250" fill={palette::INK} />
                <clipPath id="keysl"><rect x="0" y="0" width="960" height="1080" /></clipPath>
                <g clip-path="url(#keysl)">
                    {self.tj.draw(t, &cam, &Paint::silhouette(palette::INK))}
                    <rect x="70" y="776" width="820" height="62" rx="10" fill="#2A1E12" stroke={palette::INK} stroke-width="4" />
                    {slots}
                    {caps}
                </g>
                <rect x="960" y="0" width="960" height="1080" fill={palette::GREEN} />
                <path d={vee.build()} fill={palette::INK} stroke={palette::GREEN_LIGHT} stroke-width="6" opacity="0.55" />
                {lines}
                <rect x="950" y="0" width="20" height="1080" fill={palette::INK} />
            </g>
        )
    }
}

// ---------------------------------------------------------------------------------------------

/// Theo in shadow and Shreyas smiling, tinted teal.
pub struct Teal {
    a: Actor,
    b: Actor,
}

impl Teal {
    pub fn new(len: f32) -> Self {
        let ska = member(Who::Theo).skeleton();
        let hipa = ska.proportions.hip_height;
        let ma = motion(move |t| {
            build(
                &ska,
                Body {
                    root: Vec3::new(-0.35, hipa, -0.3),
                    yaw: 30.0,
                    head: Vec3::new(10.0, 20.0, 0.0),
                    chest: Vec3::new(0.0, 10.0 + t * 2.0, 0.0),
                    ..Default::default()
                },
                Arms::default(),
                None,
            )
        });
        let skb = member(Who::Shreyas).skeleton();
        let hipb = skb.proportions.hip_height;
        let mb = motion(move |t| {
            build(
                &skb,
                Body {
                    root: Vec3::new(0.35, hipb, 0.1),
                    yaw: -20.0,
                    head: Vec3::new(-12.0 + t * 10.0, 0.0, -6.0),
                    ..Default::default()
                },
                Arms::default(),
                None,
            )
        });
        Teal {
            a: Actor::new(
                Who::Theo,
                ma,
                // in shadow, eyes down, then looking up at us
                acting(
                    Acting::new(Look {
                        lids: 0.6,
                        gaze: Vec2::new(0.2, -0.7),
                        gloom: 0.4,
                        ..Default::default()
                    })
                    .then(0.4, |l| Look {
                        gaze: Vec2::new(0.0, 0.2),
                        gloom: 0.4,
                        ..expr::smug(l)
                    }),
                ),
                len,
                2,
                Vec3::ZERO,
            ),
            b: Actor::new(
                Who::Shreyas,
                mb,
                acting(
                    Acting::new(Look::default())
                        .then(0.0, expr::bliss)
                        .then(0.45, |l| Look {
                            blush: 0.5,
                            gaze: Vec2::new(-0.4, 0.0),
                            ..expr::happy(l)
                        }),
                ),
                len,
                2,
                Vec3::ZERO,
            ),
        }
    }
}

impl Shot for Teal {
    fn render<'a>(&'a self, t: f32, len: f32, _f: &Frame, _c: &FFramesContext<'a, '_>) -> Svgr<'a> {
        let _ = len;
        let cam = Camera::new(
            Vec3::new(0.0, 1.5, 1.75),
            Vec3::new(0.0, 1.45, 0.0),
            30.0,
            W,
            H,
        );
        // Theo in the shadow side of the frame, but his face still acts: dark teal with lines
        let mut dark = Paint::two_tone("#2E8C82")
            .with_suit(palette::TEAL_DARK)
            .with_hair(palette::TEAL_DARK);
        dark.line = "#0A2328";
        let light = Paint::two_tone("#6BD6B7")
            .with_suit(palette::TEAL_DARK)
            .with_hair(palette::WHITE);
        fframes::svgr!(
            <g>
                {bg(palette::TEAL)}
                {self.a.draw(t, &cam, &dark)}
                {self.b.draw(t, &cam, &light)}
            </g>
        )
    }
}

// ---------------------------------------------------------------------------------------------

/// Two-frame cuts: each of the six strikes a pose on its own colour with its name.
pub struct Strobe {
    actors: Vec<Actor>,
}

const STROBE_CUTS: [f32; 9] = [0.0, 0.125, 0.208, 0.375, 0.458, 0.542, 0.625, 0.708, 0.875];

impl Strobe {
    pub fn new(lib: &Library, len: f32) -> Self {
        let clips = [
            "slash_normal",
            "punch_normal",
            "kick_normal",
            "slash_normal",
            "punch_normal",
            "kick_normal",
        ];
        let actors = ALL
            .iter()
            .enumerate()
            .map(|(i, w)| {
                let sk = member(*w).skeleton();
                Actor::new(
                    *w,
                    in_place(
                        lib,
                        clips[i],
                        &sk,
                        Play {
                            from: 0.9 + i as f32 * 0.15,
                            looped: false,
                            yaw: 35.0,
                            ..Default::default()
                        },
                    ),
                    look(Look {
                        face: false,
                        hand_l: HandShape::Fist,
                        hand_r: HandShape::Fist,
                        ..Default::default()
                    }),
                    len,
                    1,
                    Vec3::ZERO,
                )
            })
            .collect();
        Strobe { actors }
    }
}

impl Shot for Strobe {
    fn render<'a>(
        &'a self,
        t: f32,
        _len: f32,
        _f: &Frame,
        _c: &FFramesContext<'a, '_>,
    ) -> Svgr<'a> {
        let k = STROBE_CUTS.iter().rposition(|c| t >= *c).unwrap_or(0);
        // the friends one per cut, the long last cut for the hero
        const ORDER: [usize; 9] = [1, 2, 3, 4, 5, 3, 1, 2, 0];
        let a = &self.actors[ORDER[k]];
        let colors = [
            palette::YELLOW,
            palette::RED,
            palette::BLUE,
            palette::YELLOW,
            palette::MAGENTA,
            palette::GREEN,
            palette::ORANGE,
            palette::INK,
            palette::YELLOW,
        ];
        let back = colors[k];
        let ink = if back == palette::INK {
            palette::YELLOW
        } else {
            palette::INK
        };
        let hero = k == STROBE_CUTS.len() - 1;
        let cam = if hero {
            // the hero last, the whole figure, in colour, facing us
            Camera::orbit(Vec3::new(0.25, 1.15, 0.0), 3.6, -12.0, -4.0, 36.0, W, H)
        } else {
            Camera::orbit(
                Vec3::new(0.0, 1.0, 0.0),
                3.4 - (k % 3) as f32 * 0.6,
                -30.0 + (k % 4) as f32 * 25.0,
                (k % 2) as f32 * -10.0,
                36.0,
                W,
                H,
            )
        };
        let paint = if hero {
            Paint::two_tone(back)
        } else {
            Paint::silhouette(ink)
        };
        // one big word per cut, slammed in on the free side, outlined so it reads over anything
        let word = a
            .member
            .name
            .split_whitespace()
            .last()
            .unwrap_or(a.member.name)
            .to_uppercase();
        let handle = a.member.handle.to_owned();
        let paper = if back == palette::INK {
            palette::INK
        } else {
            back
        };
        let left = k % 2 == 0;
        let (x, anchor) = if left {
            (70.0, "start")
        } else {
            (1850.0, "end")
        };
        let size = (1500.0 / word.chars().count().max(4) as f32).min(250.0)
            * if hero { 0.75 } else { 1.0 };
        let since = t - STROBE_CUTS[k];
        let pop = if since < 1.0 / 24.0 { 1.12 } else { 1.0 };
        // the hero's name low on the left, clear of his face
        let y0 = if hero { 840.0 } else { 330.0 };
        let tf = format!(
            "translate({x} {y0}) scale({pop}) translate({} {})",
            -x, -300.0
        );
        fframes::svgr!(
            <g>
                {bg(back)}
                {fx::path(fx::focus_lines(Vec2::new(960.0, 540.0), 60, 380.0, k as u32 * 13, 3.0).build(), if back == palette::INK { "#2A2410" } else { palette::YELLOW_DEEP })}
                {a.draw(t, &cam, &paint)}
                <g transform={tf}>
                    <text x={x} y="300" font-family={GOTHIC} font-size={size} text-anchor={anchor} fill={ink} stroke={paper} stroke-width="14" paint-order="stroke" stroke-linejoin="round">{word}</text>
                    <text x={x} y="390" font-family={SANS} font-weight="900" font-size="54" text-anchor={anchor} fill={ink} stroke={paper} stroke-width="10" paint-order="stroke">{handle}</text>
                </g>
            </g>
        )
    }
}

// ---------------------------------------------------------------------------------------------

/// A flood of geese pours in and piles up; the hero dashes through and sends them flying.
pub struct Flood {
    hero: Actor,
    geese: Baked2,
    flips: Vec<bool>,
    sizes: Vec<f32>,
}

const DASH_FROM: f32 = 0.35;

impl Flood {
    pub fn new(lib: &Library, len: f32) -> Self {
        let sk = member(Who::Neogoose).skeleton();
        let hero = Actor::new(
            Who::Neogoose,
            clip(
                lib,
                "dash_active",
                &sk,
                Play {
                    from: 0.5,
                    yaw: -90.0,
                    at: Vec3::new(4.5, 0.0, 0.0),
                    speed: 1.3,
                    ..Default::default()
                },
            ),
            acting(
                Acting::new(Look {
                    hand_l: HandShape::Fist,
                    hand_r: HandShape::Fist,
                    ..Default::default()
                })
                .then(0.0, |l| Look {
                    mouth: Mouth::Grin,
                    ..expr::determined(l)
                })
                .then(DASH_FROM, expr::shout)
                .then(DASH_FROM + 0.5, |l| Look {
                    mouth: Mouth::Open(0.9),
                    sparkle: 0.8,
                    ..expr::laugh(l)
                })
                .blinks(121, 2.0),
            ),
            len,
            2,
            Vec3::ZERO,
        );
        let n = 110;
        let mut bodies = Vec::with_capacity(n);
        let mut flips = Vec::with_capacity(n);
        let mut sizes = Vec::with_capacity(n);
        for i in 0..n {
            let s = 95.0 + hash2(501, i as u32) * 50.0;
            let x = 60.0 + hash2(502, i as u32) * 1800.0;
            // the first ones are already pouring in on the cut
            let y = 300.0 - (i as f32) * 14.0 - hash2(503, i as u32) * 80.0;
            let mut b = Body2::ball(Vec2::new(x, y), s * 0.55);
            b.vel = Vec2::new((hash2(504, i as u32) - 0.5) * 400.0, 900.0);
            b.spin = (hash2(505, i as u32) - 0.5) * 8.0;
            b.restitution = 0.35;
            bodies.push(b);
            flips.push(hash2(506, i as u32) > 0.5);
            sizes.push(s);
        }
        let walls = [
            (Vec2::new(960.0, 1120.0), Vec2::new(1400.0, 60.0)),
            (Vec2::new(-80.0, 0.0), Vec2::new(60.0, 4000.0)),
            (Vec2::new(2000.0, 0.0), Vec2::new(60.0, 4000.0)),
        ];
        // The hero's body, seen from the camera below, sweeps right to left through the pile.
        let pusher = Kinematic {
            shape: Shape2::Capsule(170.0, 170.0),
            path: Box::new(|t: f32| {
                let k = ((t - DASH_FROM) / 0.9).clamp(0.0, 1.0);
                Vec2::new(2300.0 - k * 2800.0, 640.0)
            }),
        };
        let geese = rigid::bake(&bodies, &walls, &[pusher], 3000.0, len, 24.0);
        Flood {
            hero,
            geese,
            flips,
            sizes,
        }
    }
}

impl Shot for Flood {
    fn render<'a>(
        &'a self,
        t: f32,
        _len: f32,
        _f: &Frame,
        _c: &FFramesContext<'a, '_>,
    ) -> Svgr<'a> {
        let th = twos(t);
        let geese: Vec<Svgr<'a>> = self
            .geese
            .at(th)
            .iter()
            .enumerate()
            .map(|(i, p)| sprites::goose(p.pos, self.sizes[i], p.angle, self.flips[i], 3.0))
            .collect();
        // the hero dashes through at the pusher's position
        let k = ((th - DASH_FROM) / 0.9).clamp(0.0, 1.0);
        let x_screen = 2300.0 - k * 2800.0;
        let cam = Camera::new(
            Vec3::new(0.0, 1.0, 4.4),
            Vec3::new(0.0, 1.0, 0.0),
            34.0,
            W,
            H,
        );
        let hero_x = self.hero.posed(t).pos(Bone::Hips);
        let mut cam2 = cam;
        cam2.shift = Vec2::new(x_screen - cam.to_screen(hero_x).x, 0.0);
        let lines = fx::speed_lines(0.0, 30, (th * 24.0) as u32, th * 6000.0, 5.0);
        let hero = if th > DASH_FROM {
            self.hero.draw(t, &cam2, &Paint::two_tone(palette::YELLOW))
        } else {
            Svgr::empty()
        };
        fframes::svgr!(
            <g>
                {bg(palette::YELLOW)}
                {fx::path(lines.build(), palette::YELLOW_DEEP)}
                {geese}
                {hero}
            </g>
        )
    }
}
