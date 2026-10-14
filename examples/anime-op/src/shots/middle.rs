//! 32-44 s: the open source collage, eating, the line-up, the rivals and the close-ups.

use std::sync::Arc;

use fframes::{FFramesContext, Frame, Svgr};
use fframes_anime::camera::Camera;
use fframes_anime::glam::{Vec2, Vec3};
use fframes_anime::rig::Bone;
use fframes_anime::shape::Path;
use fframes_anime::toon::{HandShape, Look, Mouth};

use crate::actor::{Actor, LookFn, acting, look};
use crate::cast::{ALL, RIVALS, Who, member};
use crate::expr::{self, Acting};
use crate::fx::{self, bg, ease_out, hash2, smooth, spring, twos};
use crate::motion::{Library, Play, clip, in_place};
use crate::palette::{self, Paint};
use crate::procedural::{Arms, Body, build, motion};
use crate::rigid::{self, Baked2, Body2, Shape2};
use crate::shots::title::{GOTHIC, SANS};
use crate::sprites::{self, Sticker};
use crate::{H, Shot, W, stage};

const PASTEL: [&str; 6] = [
    "#FF6EC7", "#3FE0FF", "#B6FF3B", "#B794F6", "#FF9F1C", "#FFFFFF",
];

/// The open source collage: rainbow stripes, a cheering crowd in candy colours, stickers raining
/// and piling up, and the six in black walking through the corner.
pub struct Collage {
    crowd: Vec<(Actor, Paint)>,
    walkers: Vec<Actor>,
    stickers: Vec<(Sticker, f32, &'static str)>,
    pile: Baked2,
}

impl Collage {
    pub fn new(lib: &Library, len: f32) -> Self {
        let clips = [
            "byebye_happy",
            "call_normal",
            "byebye_active",
            "respond_normal",
            "guide_normal",
            "bye_proud",
            "byebye_happy",
            "call_normal",
        ];
        let who = [
            Who::Shreyas,
            Who::Theo,
            Who::Teej,
            Who::Dillon,
            Who::Chshersh,
            Who::Neogoose,
            Who::Flaky,
            Who::Bloat,
        ];
        let crowd = who
            .iter()
            .enumerate()
            .map(|(i, w)| {
                let sk = member(*w).skeleton();
                let row = i / 4;
                let x = (i % 4) as f32 * 1.05 - 1.6 + row as f32 * 0.5;
                let z = -1.2 - row as f32 * 1.4;
                let actor = Actor::new(
                    *w,
                    in_place(
                        lib,
                        clips[i],
                        &sk,
                        Play {
                            from: i as f32 * 0.37,
                            at: Vec3::new(x, 0.0, z),
                            yaw: (i as f32 - 3.5) * 6.0,
                            ..Default::default()
                        },
                    ),
                    {
                        let hands = Look {
                            hand_l: HandShape::Open,
                            hand_r: HandShape::Open,
                            ..Default::default()
                        };
                        let seed = 60 + i as u32;
                        let face = if i % 3 == 0 {
                            expr::ha(
                                Acting::new(hands).then(0.0, expr::laugh),
                                0.0,
                                len,
                                3.0 + (i % 4) as f32,
                            )
                        } else if i % 3 == 1 {
                            Acting::new(hands)
                                .then(0.0, expr::excited)
                                .lips(&[
                                    (0.0, Mouth::Open(0.9)),
                                    (0.4 + 0.1 * (i % 3) as f32, Mouth::Grin),
                                ])
                                .then(1.6 + 0.2 * (i % 5) as f32, expr::happy)
                                .then(2.6, expr::excited)
                                .blinks(seed, 1.6)
                        } else {
                            expr::talk(
                                Acting::new(hands).then(0.0, expr::happy),
                                seed,
                                0.0,
                                len,
                                5.0,
                            )
                            .blinks(seed, 2.0)
                            .darts(seed, 0.4, 0.6)
                        };
                        acting(face)
                    },
                    len,
                    2,
                    Vec3::ZERO,
                );
                let paint = Paint::two_tone(PASTEL[(i + 1) % 6])
                    .with_suit(PASTEL[i % 6])
                    .with_hair(PASTEL[(i + 3) % 6]);
                (actor, paint)
            })
            .collect();
        let styles = [
            "walk_proud",
            "walk_normal",
            "walk_happy",
            "walk_masculinity",
            "walk_chimpira",
            "walk_active",
        ];
        let walkers = ALL
            .iter()
            .enumerate()
            .map(|(i, w)| {
                let sk = member(*w).skeleton();
                Actor::new(
                    *w,
                    clip(
                        lib,
                        styles[i],
                        &sk,
                        Play {
                            from: i as f32 * 0.21,
                            yaw: -35.0,
                            at: Vec3::new(
                                5.4 + (i % 3) as f32 * 0.6,
                                0.0,
                                -1.0 + (i / 3) as f32 * 0.6,
                            ),
                            speed: 0.8,
                            ..Default::default()
                        },
                    ),
                    look(Look {
                        face: false,
                        ..Default::default()
                    }),
                    len,
                    2,
                    Vec3::ZERO,
                )
            })
            .collect();
        // Stickers rain in on the beats and pile up at the bottom.
        let kinds = [
            Sticker::Star,
            Sticker::Heart,
            Sticker::Puff,
            Sticker::Flower,
            Sticker::Badge("PR merged"),
            Sticker::Goose,
            Sticker::Badge("v1.0"),
            Sticker::Star,
            Sticker::Badge("★ 11k"),
            Sticker::Heart,
            Sticker::Puff,
            Sticker::Flower,
        ];
        let mut bodies = Vec::new();
        let mut stickers = Vec::new();
        for i in 0..36 {
            let kind = kinds[i % kinds.len()];
            let r = 46.0 + hash2(201, i as u32) * 40.0;
            let x = 120.0 + hash2(202, i as u32) * 1680.0;
            let y = -150.0 - i as f32 * 120.0 - hash2(203, i as u32) * 80.0;
            let mut b = Body2::ball(Vec2::new(x, y), r * 0.9);
            if let Sticker::Badge(_) = kind {
                b.shape = Shape2::Box(r * 1.5, r * 0.55);
            }
            b.vel = Vec2::new((hash2(204, i as u32) - 0.5) * 300.0, 600.0);
            b.spin = (hash2(205, i as u32) - 0.5) * 6.0;
            b.restitution = 0.45;
            bodies.push(b);
            stickers.push((kind, r, PASTEL[i % 5]));
        }
        let walls = [
            (Vec2::new(960.0, 1130.0), Vec2::new(1400.0, 60.0)),
            (Vec2::new(-60.0, 0.0), Vec2::new(60.0, 3000.0)),
            (Vec2::new(1980.0, 0.0), Vec2::new(60.0, 3000.0)),
        ];
        let pile = rigid::bake(&bodies, &walls, &[], 2600.0, len, 24.0);
        Collage {
            crowd,
            walkers,
            stickers,
            pile,
        }
    }
}

impl Shot for Collage {
    fn render<'a>(&'a self, t: f32, len: f32, _f: &Frame, _c: &FFramesContext<'a, '_>) -> Svgr<'a> {
        let th = twos(t);
        // rainbow stripes, slowly turning
        let mut stripes: Vec<Svgr<'a>> = Vec::new();
        for i in 0..14 {
            let color = PASTEL[i % 5];
            let y = -400.0 + i as f32 * 140.0 + (t * 40.0) % 140.0;
            stripes.push(fframes::svgr!(<rect x="-600" y={y} width="3200" height="72" fill={color} opacity="0.55" transform="rotate(-14 960 540)" />));
        }
        let burst = fx::sunburst(Vec2::new(960.0, 420.0), 32, th * 0.25, 120.0, 1900.0, 0.5);
        let zoom = 1.0 + 0.06 * (t / len);
        let cam = Camera::new(
            Vec3::new(0.0, 1.35, 5.4 / zoom),
            Vec3::new(0.0, 1.15, -1.5),
            40.0,
            W,
            H,
        );
        let crowd: Vec<(&Actor, Paint)> = self.crowd.iter().map(|(a, p)| (a, *p)).collect();
        // the friends small at the bottom right, under the stickers
        let mut walk_cam = Camera::new(
            Vec3::new(4.6, 1.0, 15.0),
            Vec3::new(4.6, 1.0, 0.0),
            30.0,
            W,
            H,
        );
        walk_cam.shift = Vec2::new(560.0, 330.0);
        let walkers: Vec<&Actor> = self.walkers.iter().collect();
        let poses = self.pile.at(th);
        let stickers: Vec<Svgr<'a>> = self
            .stickers
            .iter()
            .zip(poses)
            .map(|((kind, r, color), p)| sprites::sticker(*kind, p.pos, *r, p.angle, color))
            .collect();
        // confetti
        let mut confetti: Vec<Svgr<'a>> = Vec::new();
        for i in 0..70 {
            let x = (hash2(301, i) * 2100.0 + th * (40.0 + hash2(302, i) * 80.0)) % 2100.0 - 90.0;
            let y =
                (hash2(303, i) * 1300.0 + th * (220.0 + hash2(304, i) * 260.0)) % 1300.0 - 110.0;
            let a = th * (3.0 + hash2(305, i) * 6.0) + hash2(306, i) * 6.0;
            let w = 18.0 * a.cos().abs() + 3.0;
            let tf = format!("rotate({} {x} {y})", a.to_degrees() * 0.3);
            confetti.push(fframes::svgr!(<rect x={x - w * 0.5} y={y - 6.0} width={w} height="12" fill={PASTEL[(i % 6) as usize]} transform={tf} />));
        }
        let title_pop = spring(th - 0.1, 1.4, 6.0).min(1.05);
        let tf = format!(
            "translate(960 210) scale({}) rotate(-6) translate(-960 -210)",
            title_pop.max(0.0)
        );
        fframes::svgr!(
            <g>
                {bg("#FF8FD6")}
                {stripes}
                {fx::path(burst.build(), "#FFFFFF")}
                {stage::actors_painted(&crowd, t, &cam)}
                <g transform={tf}>
                    <text x="968" y="268" font-family={GOTHIC} font-size="190" text-anchor="middle" fill={palette::PURPLE}>"OPEN SOURCE"</text>
                    <text x="960" y="258" font-family={GOTHIC} font-size="190" text-anchor="middle" fill="#3FE0FF" stroke={palette::WHITE} stroke-width="10" stroke-linejoin="round">"OPEN SOURCE"</text>
                </g>
                {stage::actors(&walkers, t, &walk_cam, &Paint::silhouette(palette::INK))}
                {stickers}
                {confetti}
            </g>
        )
    }
}

// ---------------------------------------------------------------------------------------------

/// The hero eats a cream puff in one bite against orange.
pub struct Eat {
    hero: Actor,
    /// The puff riding up on the palm to the mouth.
    puff: crate::physics3d::Baked3,
}

impl Eat {
    pub fn new(len: f32) -> Self {
        let sk = member(Who::Neogoose).skeleton();
        let hip = sk.proportions.hip_height;
        let hs = fframes_anime::toon::head_scale(&sk.proportions);
        let m = motion(move |t| {
            use crate::procedural::{chest_point, hand_facing, reach};
            use fframes_anime::rig::Side;
            // up to the mouth in three drawings, the bite on the onset (37.08 s), then the
            // hand drops out of the frame while he chews
            let k: f32 = match t {
                t if t < 0.08 => 0.0,
                t if t < 0.17 => 0.5,
                t if t < 0.29 => 0.9,
                t if t < 0.37 => 1.0,
                t if t < 0.45 => 0.4,
                _ => -0.4,
            };
            let body = Body {
                root: Vec3::new(0.0, hip, 0.0),
                yaw: 0.0,
                head: Vec3::new(0.0, 8.0 - 6.0 * k.max(0.0), 0.0),
                ..Default::default()
            };
            let mut pose = build(&sk, body, Arms::default(), None);
            let solved = sk.solve(&pose);
            let mouth = solved.attach(Bone::Head, Vec3::new(0.0, 0.046, 0.09) * hs);
            let start = chest_point(&sk, &pose, Vec3::new(-0.02, 0.1, 0.3));
            // the wrist under the chin: the puff on the fingers ends at the lips
            let end = mouth + Vec3::new(-0.02, -0.1, -0.07);
            reach(&sk, &mut pose, Side::Right, start.lerp(end, k), 0.3);
            pose.rot[Bone::HandR.index()] = hand_facing(
                Side::Right,
                Vec3::new(0.0, 0.3, 1.0),
                Vec3::new(0.0, 1.0, -0.2),
            );
            pose
        });
        let looks = acting(expr::chew(
            Acting::new(Look {
                hand_r: HandShape::Relaxed,
                ..Default::default()
            })
            .then(0.0, |l| Look {
                mouth: Mouth::Open(0.6),
                ..expr::excited(l)
            })
            .then(0.17, |l| Look {
                mouth: Mouth::Roar(1.0),
                ..l
            })
            .then(0.29, expr::bliss),
            0.33,
            0.7,
            0.09,
        ));
        let fig = crate::render::figure(Who::Neogoose);
        let skel = member(Who::Neogoose).skeleton();
        let puff = crate::physics3d::in_palm(
            &fig,
            &|t| skel.solve(&m(t)),
            fframes_anime::rig::Side::Right,
            HandShape::Relaxed,
            crate::props::puff_hull(),
            1.0,
            0.6,
            len,
        );
        Eat {
            hero: Actor::new(Who::Neogoose, m, looks, len, 2, Vec3::ZERO),
            puff,
        }
    }
}

impl Shot for Eat {
    fn render<'a>(
        &'a self,
        t: f32,
        _len: f32,
        _f: &Frame,
        _c: &FFramesContext<'a, '_>,
    ) -> Svgr<'a> {
        let head = self.hero.posed(0.0).pos(Bone::Head) + Vec3::new(0.0, 0.05, 0.0);
        let mut cam = Camera::orbit(head, 0.95, -80.0, 4.0, 30.0, W, H);
        cam.shift = Vec2::new(-120.0, 140.0);
        let held = self.hero.held(t);
        let bite = held >= 0.29;
        let mut paint = Paint::two_tone(palette::ORANGE)
            .with_hair(palette::YELLOW)
            .with_suit(palette::INK);
        paint.beard = Some(palette::YELLOW);
        let puff = if bite {
            Vec::new()
        } else {
            // settled in the palm, then carried by the hand up to the mouth
            use fframes_anime::glam::Affine3A;
            let hand = |p: &fframes_anime::rig::Posed| {
                Affine3A::from_rotation_translation(p.rot(Bone::HandR), p.pos(Bone::HandR))
            };
            let rel = hand(&self.hero.posed(0.0)).inverse() * self.puff.affine(0.0, 1.0);
            vec![crate::render::Placed::new(
                crate::props::cream_puff(),
                hand(&self.hero.posed(t)) * rel,
                held,
            )]
        };
        fframes::svgr!(
            <g>
                {bg(palette::ORANGE)}
                {crate::render::draw_with(&[(&self.hero, paint)], &puff, t, &cam)}
            </g>
        )
    }
}

// ---------------------------------------------------------------------------------------------

/// All six in a row, waving, each in their colour.
pub struct Lineup {
    actors: Vec<(Actor, Paint)>,
}

impl Lineup {
    pub fn new(_lib: &Library, len: f32) -> Self {
        // the hero in front waving, the friends crowding in around him: a waist-up group shot
        let order = [
            Who::Chshersh,
            Who::Dillon,
            Who::Neogoose,
            Who::Shreyas,
            Who::Theo,
            Who::Teej,
        ];
        let spots = [
            (-1.0_f32, -0.55_f32, 25.0_f32),
            (-0.55, -0.25, 12.0),
            (0.0, 0.25, -5.0),
            (0.5, -0.3, -12.0),
            (0.92, -0.6, -22.0),
            (1.3, -0.85, -30.0),
        ];
        let suits = [
            palette::BLUE,
            palette::INK,
            palette::INK,
            palette::BLUE,
            palette::INK,
            palette::BLUE,
        ];
        let actors = order
            .iter()
            .enumerate()
            .map(|(i, w)| {
                let sk = member(*w).skeleton();
                let hip = sk.proportions.hip_height;
                let (x, z, yaw) = spots[i];
                let hero = *w == Who::Neogoose;
                let lean = [8.0_f32, -6.0, 0.0, 5.0, -10.0, 4.0][i];
                let motion = crate::procedural::motion(move |t| {
                    let sway = (t * 2.2 + i as f32).sin();
                    let body = crate::procedural::Body {
                        root: Vec3::new(x, hip, z),
                        yaw,
                        chest: Vec3::new(lean * 0.4, 0.0, lean * 0.3 + sway * 1.5),
                        head: Vec3::new(-yaw * 0.4, 4.0, lean * 0.6),
                        ..Default::default()
                    };
                    // a wave at shoulder height, palm to us, three drawings on twos
                    let wave = [0.0_f32, 0.05, -0.03][((t * 12.0) as usize) % 3];
                    let arms = if hero {
                        crate::procedural::Arms {
                            left: Some(Vec3::new(0.22 + wave, 0.22, 0.25)),
                            left_palm: Some(Vec3::new(0.0, 0.0, 1.0)),
                            left_fingers: Some(Vec3::new(0.15 + wave * 3.0, 1.0, 0.0)),
                            right: Some(Vec3::new(-0.18, -0.3, 0.08)),
                            elbow_out: 1.0,
                            ..Default::default()
                        }
                    } else {
                        crate::procedural::Arms {
                            left: Some(Vec3::new(0.2, -0.32, 0.08)),
                            right: Some(Vec3::new(-0.2, -0.32, 0.08)),
                            elbow_out: 0.4,
                            ..Default::default()
                        }
                    };
                    crate::procedural::build(&sk, body, arms, None)
                });
                // loose fists at the sides read cleaner than half-open hands at this size
                let hands = Look {
                    hand_l: if hero {
                        HandShape::Open
                    } else {
                        HandShape::Fist
                    },
                    hand_r: HandShape::Fist,
                    fists: 0.85,
                    ..Default::default()
                };
                let seed = 80 + i as u32;
                let face = match w {
                    // waving at us, then a wink
                    Who::Neogoose => Acting::new(hands)
                        .then(0.0, |l| Look {
                            sparkle: 0.6,
                            ..expr::happy(l)
                        })
                        .then(0.55, |l| Look {
                            wink: -1.0,
                            sparkle: 1.0,
                            mouth: Mouth::Grin,
                            ..expr::happy(l)
                        }),
                    Who::Chshersh => expr::talk(
                        Acting::new(hands).then(0.0, |l| Look {
                            gaze: Vec2::new(0.7, 0.0),
                            ..expr::happy(l)
                        }),
                        seed,
                        0.0,
                        0.8,
                        6.0,
                    ),
                    Who::Theo => Acting::new(hands)
                        .then(0.0, |l| Look {
                            mouth: Mouth::Grin,
                            ..expr::smug(l)
                        })
                        .then(0.6, |l| Look {
                            gaze: Vec2::new(-0.5, 0.0),
                            ..l
                        }),
                    Who::Teej => expr::talk(
                        Acting::new(hands).then(0.0, expr::excited),
                        seed,
                        0.0,
                        1.1,
                        7.0,
                    ),
                    Who::Dillon => Acting::new(hands).then(0.0, |l| Look {
                        gaze: Vec2::new(-0.4, -0.1),
                        ..expr::bored(l)
                    }),
                    _ => Acting::new(hands)
                        .then(0.0, expr::happy)
                        .then(0.45, |l| Look {
                            mouth: Mouth::Cat,
                            ..l
                        }),
                }
                .blinks(seed, 1.8)
                .darts(seed, 0.15, 0.5);
                let actor = Actor::new(*w, motion, acting(face), len, 2, Vec3::new(0.6, 0.0, 0.0));
                let mut paint = Paint::two_tone(palette::YELLOW).with_suit(suits[i]);
                if suits[i] == palette::BLUE {
                    paint.paper = "#5C8FE0";
                }
                if hero {
                    paint.paper = "#F7A63C";
                }
                (actor, paint)
            })
            .collect();
        Lineup { actors }
    }
}

impl Shot for Lineup {
    fn render<'a>(&'a self, t: f32, len: f32, _f: &Frame, _c: &FFramesContext<'a, '_>) -> Svgr<'a> {
        // a locked 3D layout, the camera move done as a 2D pan of the composite (no parallax
        // to fight the drawings held on twos)
        let mut cam = Camera::new(
            Vec3::new(0.05, 1.42, 1.75),
            Vec3::new(0.12, 1.3, 0.0),
            46.0,
            W,
            H,
        );
        cam.shift = Vec2::new(-60.0, 120.0);
        let pan = format!("translate({:.1} 0)", 70.0 * (t / len));
        let list: Vec<(&Actor, Paint)> = self.actors.iter().map(|(a, p)| (a, *p)).collect();
        // the hero's wink sparkles
        let mut kira = fframes_anime::shape::Path::new();
        if let Some((hero, _)) = self
            .actors
            .iter()
            .find(|(a, _)| a.member.who == Who::Neogoose)
        {
            let since = crate::fx::twos(t) - 0.55;
            if since >= 0.0 {
                let eye = cam.to_screen(
                    hero.posed(t)
                        .attach(Bone::Head, Vec3::new(-0.04, 0.11, 0.09)),
                );
                for (i, (dx, dy, r)) in [
                    (-70.0, -40.0, 46.0),
                    (-110.0, 30.0, 26.0),
                    (-40.0, -100.0, 20.0),
                ]
                .iter()
                .enumerate()
                {
                    let pop = crate::fx::ease_out((since - i as f32 * 0.06) / 0.1);
                    kira.append(&crate::fx::sparkle(eye + Vec2::new(*dx, *dy), r * pop, 0.2));
                }
            }
        }
        fframes::svgr!(
            <g>
                {bg(palette::YELLOW)}
                <g transform={pan}>
                    {stage::actors_painted(&list, t, &cam)}
                    <path d={kira.build()} fill={palette::WHITE} stroke={palette::INK} stroke-width="4" />
                </g>
            </g>
        )
    }
}

// ---------------------------------------------------------------------------------------------

/// The rivals in olive, framed by jagged comic panel borders, name plates.
pub struct Rivals {
    actors: Vec<Actor>,
}

impl Rivals {
    pub fn new(lib: &Library, len: f32) -> Self {
        let clips = [
            "walk_chimpira",
            "respond_normal",
            "guide_normal",
            "walk_not-confident",
        ];
        let actors = RIVALS
            .iter()
            .enumerate()
            .map(|(i, w)| {
                let sk = member(*w).skeleton();
                let x = (i as f32 - 1.5) * 0.85;
                Actor::new(
                    *w,
                    in_place(
                        lib,
                        clips[i],
                        &sk,
                        Play {
                            from: i as f32 * 0.5,
                            at: Vec3::new(x, 0.0, -((i % 2) as f32) * 0.6),
                            speed: 0.6,
                            ..Default::default()
                        },
                    ),
                    acting(
                        Acting::new(Look {
                            accent_eyes: true,
                            mouth: if i % 2 == 0 { Mouth::Grin } else { Mouth::Line },
                            gaze: Vec2::new(0.4 * (i as f32 - 1.5), 0.0),
                            ..Default::default()
                        })
                        .then(0.5 + 0.15 * i as f32, |l| Look {
                            gaze: Vec2::ZERO,
                            mouth: Mouth::Grin,
                            eyes: 0.6,
                            ..l
                        })
                        .blinks(90 + i as u32, 2.6),
                    ),
                    len,
                    2,
                    Vec3::new(1.0, 0.0, 0.0),
                )
            })
            .collect();
        Rivals { actors }
    }
}

impl Shot for Rivals {
    fn render<'a>(&'a self, t: f32, len: f32, _f: &Frame, _c: &FFramesContext<'a, '_>) -> Svgr<'a> {
        let th = twos(t);
        // low and close: the rivals loom, walking at the camera
        let _ = len;
        let cam = Camera::new(
            Vec3::new(0.0, 0.85, 2.85),
            Vec3::new(0.0, 1.3, 0.0),
            44.0,
            W,
            H,
        );
        let list: Vec<&Actor> = self.actors.iter().collect();
        // jagged panel borders
        let mut borders = Path::new();
        borders.polyline(&[
            Vec2::new(150.0, -10.0),
            Vec2::new(150.0, 300.0),
            Vec2::new(90.0, 330.0),
            Vec2::new(150.0, 360.0),
            Vec2::new(150.0, 1090.0),
        ]);
        borders.polyline(&[
            Vec2::new(1750.0, -10.0),
            Vec2::new(1750.0, 600.0),
            Vec2::new(1820.0, 640.0),
            Vec2::new(1750.0, 680.0),
            Vec2::new(1750.0, 1090.0),
        ]);
        // name plates slammed in on the onsets, one slot each, with what they do to you
        let onsets = [0.01, 0.22, 0.41, 0.59];
        let names: Vec<Svgr<'a>> = self
            .actors
            .iter()
            .enumerate()
            .map(|(i, a)| {
                let since = th - onsets[i % 4];
                if since < 0.0 {
                    return Svgr::empty();
                }
                let head = cam.to_screen(a.posed(t).head_top());
                let up = i % 2 == 0;
                let p = Vec2::new(head.x, if up { 120.0 } else { 230.0 });
                let pop = if since < 1.0 / 12.0 { 1.25 } else { 1.0 };
                let tf = format!(
                    "translate({:.1} {:.1}) rotate(-4) scale({pop}) translate({:.1} {:.1})",
                    p.x, p.y, -p.x, -p.y
                );
                let name = a.member.name.to_owned();
                let what = match a.member.who {
                    Who::Bloat => "412 MB node_modules",
                    Who::Flaky => "passes 1 / 3",
                    Who::Segfault => "core dumped",
                    _ => "p99 9.8 s",
                };
                let w = 44.0 * name.chars().count() as f32 + 70.0;
                fframes::svgr!(
                    <g transform={tf}>
                        <rect x={p.x - w * 0.5} y={p.y - 52.0} width={w} height="96" fill={palette::INK} />
                        <text x={p.x} y={p.y - 2.0} font-family={GOTHIC} font-size="54" text-anchor="middle" fill={palette::YELLOW}>{name}</text>
                        <text x={p.x} y={p.y + 34.0} font-family={SANS} font-weight="900" font-size="26" text-anchor="middle" fill="#E9CF4C">{what}</text>
                    </g>
                )
            })
            .collect();
        let names_only = names;
        // the rivals in sepia: hoods and robes in umber, faces lost in the hoods' shadow, eyes
        // and grins glowing
        let mut sepia = Paint::two_tone("#D9BE4A")
            .with_suit("#1E1704")
            .with_hair("#2A2006");
        sepia.line = "#2B2006";
        sepia.accent = Some(palette::YELLOW);
        sepia.shade = 0.22;
        sepia.terminator = 0.0;
        fframes::svgr!(
            <g>
                {bg("#E9CF4C")}
                {stage::actors(&list, t, &cam, &sepia)}
                {names_only}
                {fx::stroke(borders.build(), palette::INK, 22.0)}
            </g>
        )
    }
}

// ---------------------------------------------------------------------------------------------

/// A close-up of one character's head, the camera orbiting it.
fn closeup_cam(a: &Actor, t: f32, dist: f32, yaw: f32, pitch: f32, shift: Vec2) -> Camera {
    let head = a.posed(t).pos(Bone::Head) + Vec3::new(0.0, 0.1, 0.0);
    let mut cam = Camera::orbit(head, dist, yaw, pitch, 30.0, W, H);
    cam.shift = shift;
    cam
}

fn standing(
    who: Who,
    f: impl Fn(f32) -> (Body, Arms) + Send + Sync + 'static,
) -> crate::motion::MotionFn {
    let sk = member(who).skeleton();
    let hip = sk.proportions.hip_height;
    motion(move |t| {
        let (mut body, arms) = f(t);
        body.root += Vec3::new(0.0, hip, 0.0);
        build(&sk, body, arms, None)
    })
}

/// The outline of a rectangular lens of glasses on a head (frame points, `side` 1 = the
/// character's left), as `figure` models them.
fn lens_outline(cam: &Camera, posed: &fframes_anime::rig::Posed, hs: f32, side: f32) -> Vec<Vec2> {
    let c = Vec3::new(0.035 * side, 0.098, 0.102);
    (0..24)
        .map(|i| {
            let a = i as f32 / 24.0 * std::f32::consts::TAU;
            let (cx, cy) = (a.cos(), a.sin());
            let k = (cx.abs().powi(4) + (cy * 1.4).abs().powi(4)).powf(0.25);
            let (x, y) = (cx / k * 0.026, cy / k * 0.026 / 1.4);
            let p = c + Vec3::new(x, y, -(x * side + 0.01).max(0.0) * 0.35 + 0.002);
            cam.to_screen(posed.attach(Bone::Head, p * hs))
        })
        .collect()
}

/// When Kovanikov's glasses flash (shot seconds): beat 105.
const PUSH: f32 = 0.25;

/// How far the chin has lifted, drawing by drawing: three drawings up, an overshoot, settle.
fn push_glasses(t: f32) -> f32 {
    match t {
        t if t < PUSH - 0.17 => 0.0,
        t if t < PUSH - 0.08 => 0.35,
        t if t < PUSH => 0.8,
        t if t < PUSH + 0.08 => 1.05,
        _ => 1.0,
    }
}

/// Kovanikov lifts his chin and his glasses flash; diagonal bars cut the frame.
pub struct Glint {
    a: Actor,
}

impl Glint {
    pub fn new(len: f32) -> Self {
        let m = standing(Who::Chshersh, |t| {
            let k = push_glasses(t);
            (
                Body {
                    // the chin lifts a touch as the finger lands
                    head: Vec3::new(-16.0, 5.0 - 4.0 * k, 0.0),
                    ..Default::default()
                },
                Arms::default(),
            )
        });
        let face = Acting::new(Look {
            lids: 0.3,
            gaze: Vec2::new(-0.5, 0.0),
            ..Default::default()
        })
        .then(PUSH - 0.04, |l| Look {
            gaze: Vec2::ZERO,
            ..expr::smug(l)
        })
        .then(0.6, |l| Look { lids: 0.7, ..l });
        let looks: LookFn = Arc::new(move |t| face.at(t));
        Glint {
            a: Actor::new(Who::Chshersh, m, looks, len, 2, Vec3::new(-1.5, 0.0, 0.0)),
        }
    }
}

impl Shot for Glint {
    fn render<'a>(
        &'a self,
        t: f32,
        _len: f32,
        frame: &Frame,
        _c: &FFramesContext<'a, '_>,
    ) -> Svgr<'a> {
        let th = twos(t);
        // a locked extreme close-up on the near lens
        let cam = closeup_cam(&self.a, 0.0, 0.27, -24.0, 0.0, Vec2::new(-260.0, 160.0));
        let mut bars = Path::new();
        for (x, w) in [(1350.0, 70.0), (1500.0, 30.0), (1600.0, 110.0)] {
            bars.polygon(&[
                Vec2::new(x, -20.0),
                Vec2::new(x + w, -20.0),
                Vec2::new(x + w - 420.0, 1100.0),
                Vec2::new(x - 420.0, 1100.0),
            ]);
        }
        // the flash: the lenses go white the drawing the finger lands, a glint sweeps across
        // and a star pops on the rim
        let posed = self.a.posed(t);
        let hs = fframes_anime::toon::head_scale(&self.a.skeleton.proportions);
        let flash = if th < PUSH {
            0.0
        } else {
            1.0 - smooth((th - PUSH - 0.3) / 0.3)
        };
        let mut lenses = Vec::new();
        let mut star = Svgr::empty();
        if flash > 0.0 {
            for side in [1.0_f32, -1.0] {
                let ring = lens_outline(&cam, &posed, hs, side);
                let mut lens = Path::new();
                lens.polygon(&ring);
                let lo = ring
                    .iter()
                    .fold(Vec2::splat(f32::INFINITY), |a, b| a.min(*b));
                let hi = ring
                    .iter()
                    .fold(Vec2::splat(f32::NEG_INFINITY), |a, b| a.max(*b));
                let id = format!("glintlens{}", if side > 0.0 { "l" } else { "r" });
                let url = format!("url(#{id})");
                let d = lens.build();
                let sweep = crate::gpu::shine(
                    frame,
                    (lo.x, lo.y, hi.x - lo.x, hi.y - lo.y),
                    ((th - PUSH) / 0.35).clamp(0.0, 1.0),
                    fframes::Color::hex("#FFFFFF"),
                );
                lenses.push(fframes::svgr!(
                    <g opacity={flash}>
                        <clipPath id={id}><path d={d.clone()} /></clipPath>
                        <path d={d} fill="#EEF7FF" />
                        <g clip-path={url}>{sweep}</g>
                    </g>
                ));
                if side < 0.0 {
                    let pop = match ((th - PUSH) * 12.0).floor() as i32 {
                        0 => 1.4,
                        1 => 1.0,
                        2 => 0.7,
                        _ => 0.0,
                    };
                    if pop > 0.0 {
                        let c = Vec2::new(hi.x - (hi.x - lo.x) * 0.15, lo.y + (hi.y - lo.y) * 0.1);
                        let r = (hi.x - lo.x) * 0.45 * pop;
                        star = fframes::svgr!(
                            <g>
                                {fx::path(fx::sparkle(c, r * 1.15, 0.2).build(), palette::INK)}
                                {fx::path(fx::sparkle(c, r, 0.2).build(), palette::WHITE)}
                            </g>
                        );
                    }
                }
            }
        }
        fframes::svgr!(
            <g>
                {bg(palette::YELLOW)}
                {self.a.draw(t, &cam, &Paint::two_tone(palette::YELLOW))}
                {lenses}
                {star}
                {fx::path(bars.build(), palette::INK)}
            </g>
        )
    }
}

/// Theo panics in pink, three of him: "eh, way too fast..." in green rows behind.
pub struct Panic {
    a: Actor,
}

impl Panic {
    pub fn new(len: f32) -> Self {
        let m = standing(Who::Theo, |t| {
            // "no no no": the hands wave side to side out of step, the head shakes, on twos
            let k = fframes_anime::koma::hold(t, 24.0, 2);
            let shake = (k * 55.0).sin() * 3.0;
            let wave_l = (k * 21.0).sin();
            let wave_r = (k * 21.0 + 1.9).sin();
            (
                Body {
                    head: Vec3::new(shake, -9.0, 3.0 * (k * 9.0).sin()),
                    chest: Vec3::new(3.0 * (k * 11.0).sin(), 0.0, 0.0),
                    ..Default::default()
                },
                Arms {
                    left: Some(Vec3::new(0.16 + 0.03 * wave_l, 0.22 + 0.015 * wave_r, 0.12)),
                    right: Some(Vec3::new(
                        -0.16 + 0.03 * wave_r,
                        0.22 + 0.015 * wave_l,
                        0.12,
                    )),
                    elbow_out: 1.1,
                    left_palm: Some(Vec3::new(0.0, 0.05, 1.0)),
                    right_palm: Some(Vec3::new(0.0, 0.05, 1.0)),
                    left_fingers: Some(Vec3::new(0.1, 1.0, 0.0)),
                    right_fingers: Some(Vec3::new(-0.1, 1.0, 0.0)),
                },
            )
        });
        // pinprick pupils, sweat, the mouth flapping between "o" and a wobble on twos
        let flap: Vec<(f32, Mouth)> = (0..12)
            .map(|i| {
                let m = if i % 2 == 0 {
                    Mouth::O(0.7)
                } else {
                    Mouth::Wavy
                };
                (i as f32 * 2.0 / 24.0, m)
            })
            .collect();
        let looks = acting(
            Acting::new(Look {
                hand_l: HandShape::Open,
                hand_r: HandShape::Open,
                fists: 0.85,
                ..Default::default()
            })
            .then(0.0, |l| Look {
                pupils: 0.35,
                lids: 0.0,
                brows: 1.0,
                brow_angle: -1.0,
                gloom: 0.7,
                sweat: 0.9,
                tears: 0.3,
                ..l
            })
            .lips(&flap)
            .darts(102, 0.5, 0.12),
        );
        Panic {
            a: Actor::new(Who::Theo, m, looks, len, 2, Vec3::ZERO),
        }
    }
}

impl Shot for Panic {
    fn render<'a>(
        &'a self,
        t: f32,
        _len: f32,
        _f: &Frame,
        _c: &FFramesContext<'a, '_>,
    ) -> Svgr<'a> {
        let rows: Vec<Svgr<'a>> = (0..5)
            .map(|i| {
                let y = 120.0 + i as f32 * 220.0;
                fx::text_band(
                    "え、速すぎ…",
                    y,
                    110.0,
                    760.0,
                    t * 300.0 * if i % 2 == 0 { 1.0 } else { -1.0 },
                    palette::GREEN_LIGHT,
                    GOTHIC,
                )
            })
            .collect();
        let pink = "#F27BB0";
        let mut paint = Paint::two_tone(pink);
        paint.shade = 0.0;
        // three of him side by side, the outer two a drawing out of step
        let copies: Vec<Svgr<'a>> = [(-640.0_f32, 2.0 / 24.0), (640.0, 4.0 / 24.0), (0.0, 0.0)]
            .iter()
            .map(|(x, lag)| {
                let cam = closeup_cam(&self.a, 0.0, 1.05, 0.0, 0.0, Vec2::new(*x, 120.0));
                self.a.draw((t + lag).max(0.0), &cam, &paint)
            })
            .collect();
        fframes::svgr!(
            <g>
                {bg(palette::YELLOW)}
                {rows}
                {copies}
            </g>
        )
    }
}

/// Dillon's eye snaps open blood red on the beat; black bars slide across.
pub struct RedEye {
    a: Actor,
}

/// The snap (shot seconds): beat 110; the glare narrows on beat 111.
const RED_SNAP: f32 = 0.53;
const RED_GLARE: f32 = 0.91;

impl RedEye {
    pub fn new(len: f32) -> Self {
        // chin down, looking up from under the brows; held, the drawing does the work
        let m = standing(Who::Dillon, |_| {
            (
                Body {
                    head: Vec3::new(22.0, 12.0, -4.0),
                    ..Default::default()
                },
                Arms::default(),
            )
        });
        let looks = acting(
            Acting::new(Look {
                lids: 0.85,
                gaze: Vec2::new(-0.4, -0.5),
                gloom: 0.5,
                ..Default::default()
            })
            // the lid lifts a drawing before the snap
            .then(RED_SNAP - 0.09, |l| Look {
                lids: 0.45,
                gaze: Vec2::new(-0.5, -0.1),
                ..l
            })
            // open, onto us, blood red with a pin of a pupil
            .then(RED_SNAP, |l| Look {
                lids: 0.0,
                brows: 0.3,
                pupils: 0.75,
                bloodshot: 1.0,
                accent_eyes: true,
                gaze: Vec2::new(-0.55, 0.2),
                ..l
            })
            .then(RED_GLARE, |l| Look {
                brows: 0.0,
                brow_angle: 1.0,
                squint: 0.7,
                lids: 0.3,
                ..l
            }),
        );
        RedEye {
            a: Actor::new(Who::Dillon, m, looks, len, 2, Vec3::new(1.5, 0.0, 0.0)),
        }
    }
}

impl Shot for RedEye {
    fn render<'a>(
        &'a self,
        t: f32,
        _len: f32,
        frame: &Frame,
        _c: &FFramesContext<'a, '_>,
    ) -> Svgr<'a> {
        let th = twos(t);
        // a locked extreme close-up of the near eye, from the side and a little below
        let hs = fframes_anime::toon::head_scale(&self.a.skeleton.proportions);
        let posed = self.a.posed(0.0);
        // the eye away from the side the fringe falls to
        let eye_local = Vec3::new(-0.034, 0.097, 0.085) * hs;
        let mut cam = closeup_cam(&self.a, 0.0, 0.24, -30.0, 4.0, Vec2::ZERO);
        let eye_at = posed.attach(Bone::Head, eye_local);
        cam.shift = Vec2::new(860.0, 500.0) - cam.to_screen(eye_at);
        let slide = ease_out(t / 0.3) * 300.0;
        let mut bars = Path::new();
        for (x, w) in [(820.0, 90.0), (980.0, 30.0)] {
            let x = x + slide;
            bars.polygon(&[
                Vec2::new(x + 500.0, -20.0),
                Vec2::new(x + w + 500.0, -20.0),
                Vec2::new(x + w, 1100.0),
                Vec2::new(x, 1100.0),
            ]);
        }
        // the impact frame: one drawing inverted, ink on red, as the eye snaps open
        if (RED_SNAP..RED_SNAP + 2.0 / 24.0).contains(&th) {
            return fframes::svgr!(
                <g>
                    {bg(palette::RED)}
                    {self.a.draw(t, &cam, &Paint::two_tone(palette::RED).with_accent(palette::YELLOW))}
                    {crate::gpu::focus_lines(frame, Vec2::new(860.0, 500.0), 260.0, fframes::Color::hex(palette::INK), 150.0)}
                </g>
            );
        }
        let eye = cam.project(self.a.posed(t).attach(Bone::Head, eye_local));
        let open = if th < RED_SNAP { 0.0 } else { 1.0 };
        let glow = (0.02 * eye.scale).min(170.0);
        let pulse = 0.8 + 0.2 * (th * 14.0).sin().abs();
        fframes::svgr!(
            <g>
                {bg(palette::YELLOW_DEEP)}
                {self.a.draw(t, &cam, &Paint::two_tone(palette::YELLOW_DEEP).with_accent(palette::RED))}
                <defs>
                    <filter id="redeyeglow" x="-100%" y="-100%" width="300%" height="300%">
                        <feGaussianBlur stdDeviation="22" />
                    </filter>
                </defs>
                <g style="mix-blend-mode:screen" opacity={open * pulse}>
                    <ellipse cx={eye.p.x} cy={eye.p.y} rx={glow * 1.3} ry={glow * 0.7} fill="#FF2A1A" filter="url(#redeyeglow)" />
                </g>
                {crate::gpu::rays(frame, eye.p, fframes::Color::hex("#FF7A2F").with_alpha((90.0 * open) as u8), 22.0)}
                {fx::path(bars.build(), palette::INK)}
            </g>
        )
    }
}

/// When Shreyas slams up the card (shot seconds): beat 112.
const LGTM: f32 = 0.18;

/// How far the card arm is up, drawing by drawing: two drawings up, an overshoot, settle.
fn lgtm_arm(t: f32) -> f32 {
    match t {
        t if t < LGTM - 0.09 => 0.0,
        t if t < LGTM => 0.55,
        t if t < LGTM + 0.09 => 1.08,
        _ => 1.0,
    }
}

/// Shreyas and Dillon side by side, vertical bars: Shreyas slams up an LGTM card on the beat.
pub struct Bars {
    a: Actor,
    b: Actor,
}

impl Bars {
    pub fn new(len: f32) -> Self {
        let ma = standing(Who::Shreyas, |t| {
            let k = lgtm_arm(t);
            (
                Body {
                    root: Vec3::new(-0.3, 0.0, 0.0),
                    yaw: 20.0,
                    head: Vec3::new(-15.0 + 9.0 * k.min(1.0), -3.0 * k, 4.0),
                    ..Default::default()
                },
                Arms {
                    left: Some(Vec3::new(0.2 + 0.06 * k, -0.3 + 0.44 * k, 0.05 + 0.17 * k)),
                    left_palm: Some(Vec3::new(-0.3, 0.0, 1.0)),
                    left_fingers: Some(Vec3::new(-0.05, 1.0, -0.1)),
                    elbow_out: 1.1,
                    ..Default::default()
                },
            )
        });
        let mb = standing(Who::Dillon, |_| {
            (
                Body {
                    root: Vec3::new(0.3, 0.0, -0.1),
                    yaw: -15.0,
                    head: Vec3::new(8.0, 2.0, 0.0),
                    ..Default::default()
                },
                Arms::default(),
            )
        });
        Bars {
            a: Actor::new(
                Who::Shreyas,
                ma,
                acting(
                    Acting::new(Look {
                        hand_l: HandShape::Fist,
                        ..Default::default()
                    })
                    .then(0.0, |l| Look {
                        gaze: Vec2::new(-0.7, 0.0),
                        ..expr::happy(l)
                    })
                    .then(LGTM, |l| Look {
                        gaze: Vec2::new(0.0, 0.1),
                        sparkle: 1.0,
                        ..expr::laugh(l)
                    })
                    .blinks(112, 1.7),
                ),
                len,
                2,
                Vec3::ZERO,
            ),
            b: Actor::new(
                Who::Dillon,
                mb,
                acting(
                    Acting::new(Look::default())
                        .then(0.0, |l| Look {
                            gaze: Vec2::new(0.6, 0.0),
                            ..expr::bored(l)
                        })
                        // the card: a side-eye and a smirk
                        .then(LGTM + 0.17, |l| Look {
                            gaze: Vec2::new(-0.9, 0.1),
                            ..expr::smug(l)
                        })
                        .blinks(113, 3.0),
                ),
                len,
                2,
                Vec3::ZERO,
            ),
        }
    }
}

impl Shot for Bars {
    fn render<'a>(
        &'a self,
        t: f32,
        _len: f32,
        _f: &Frame,
        _c: &FFramesContext<'a, '_>,
    ) -> Svgr<'a> {
        let th = twos(t);
        let cam = Camera::new(
            Vec3::new(-0.1, 1.5, 1.25),
            Vec3::new(0.0, 1.45, 0.0),
            30.0,
            W,
            H,
        );
        let mut bars = Path::new();
        for (x, w) in [
            (1480.0, 60.0),
            (1580.0, 24.0),
            (1660.0, 80.0),
            (1800.0, 36.0),
        ] {
            bars.polygon(&[
                Vec2::new(x, -10.0),
                Vec2::new(x + w, -10.0),
                Vec2::new(x + w, 1090.0),
                Vec2::new(x, 1090.0),
            ]);
        }
        let mut diag = Path::new();
        diag.polygon(&[
            Vec2::new(1300.0, 1090.0),
            Vec2::new(1920.0, 380.0),
            Vec2::new(1920.0, 440.0),
            Vec2::new(1350.0, 1090.0),
        ]);
        // the card in the fist, slammed up with the arm
        let k = lgtm_arm(th);
        let card = if k > 0.0 {
            let hand = cam.project(self.a.posed(t).pos(Bone::HandL));
            let s = hand.scale * 0.0011;
            let tf = format!(
                "translate({:.1} {:.1}) rotate({:.1}) scale({s:.3})",
                hand.p.x,
                hand.p.y - 70.0 * s,
                8.0 + 6.0 * (1.0 - k.min(1.0))
            );
            fframes::svgr!(
                <g transform={tf}>
                    <rect x="-175" y="-110" width="350" height="190" rx="14" fill={palette::RED} stroke={palette::INK} stroke-width="10" />
                    <rect x="-157" y="-92" width="314" height="154" rx="8" fill="none" stroke={palette::WHITE} stroke-width="5" />
                    <text x="0" y="6" font-family={GOTHIC} font-size="72" text-anchor="middle" fill={palette::WHITE}>"LGTM"</text>
                    <text x="0" y="52" font-family={GOTHIC} font-size="28" letter-spacing="6" text-anchor="middle" fill={palette::YELLOW}>"APPROVED"</text>
                </g>
            )
        } else {
            Svgr::empty()
        };
        fframes::svgr!(
            <g>
                {bg(palette::YELLOW)}
                {stage::actors(&[&self.a, &self.b], t, &cam, &Paint::two_tone(palette::YELLOW))}
                {card}
                {fx::path(bars.build(), palette::INK)}
                {fx::path(diag.build(), palette::INK)}
            </g>
        )
    }
}
