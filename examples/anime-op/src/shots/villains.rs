//! 50.5-62.4 s, the quiet before the chorus: "listen", the rivals revealed under black arches,
//! the Bloat turning around, grins in the dark of the hoods.

use std::f32::consts::PI;
use std::sync::Arc;

use fframes::{FFramesContext, Frame, Svgr};
use fframes_anime::camera::Camera;
use fframes_anime::glam::{Vec2, Vec3};
use fframes_anime::rig::Bone;
use fframes_anime::shape::Path;
use fframes_anime::toon::{HandShape, Look, Mouth};

use crate::actor::{Actor, acting, look};
use crate::cast::{Who, member};
use crate::expr::{self, Acting};
use crate::fx::{self, bg, ease_out, smooth, twos};
use crate::jojo::{self, Strike, StrikeKey};
use crate::palette::{self, Paint};
use crate::procedural::{Arms, Body, build, motion};
use crate::shots::title::BLACKLETTER;
use crate::{H, Shot, W};

fn idle(
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

/// Two faces in neon lines on navy, hands cupped behind their ears: "listen".
pub struct Listen {
    a: Actor,
    b: Actor,
}

impl Listen {
    pub fn new(len: f32) -> Self {
        let arms = Arms {
            left: Some(Vec3::new(0.15, 0.29, 0.0)),
            right: Some(Vec3::new(-0.15, 0.29, 0.0)),
            elbow_out: 1.6,
            left_palm: Some(Vec3::new(0.0, 0.0, 1.0)),
            right_palm: Some(Vec3::new(0.0, 0.0, 1.0)),
            left_fingers: None,
            right_fingers: None,
        };
        // straining to hear: brows up, eyes to the side, then something is heard
        let looks = acting(
            Acting::new(Look {
                hand_l: HandShape::Open,
                hand_r: HandShape::Open,
                ..Default::default()
            })
            .then(0.0, |l| Look {
                brows: 0.8,
                gaze: Vec2::new(0.7, 0.2),
                mouth: Mouth::O(0.3),
                ..l
            })
            .then(0.26, |l| Look {
                gaze: Vec2::new(-0.1, 0.0),
                ..expr::surprised(l)
            }),
        );
        Listen {
            a: Actor::new(
                Who::Neogoose,
                idle(Who::Neogoose, move |t| {
                    (
                        Body {
                            root: Vec3::new(-0.42, 0.0, 0.0),
                            yaw: 12.0,
                            head: Vec3::new(0.0, 4.0 + t * 4.0, 0.0),
                            ..Default::default()
                        },
                        arms,
                    )
                }),
                looks.clone(),
                len,
                2,
                Vec3::ZERO,
            ),
            b: Actor::new(
                Who::Chshersh,
                idle(Who::Chshersh, move |t| {
                    (
                        Body {
                            root: Vec3::new(0.42, 0.0, 0.0),
                            yaw: -12.0,
                            head: Vec3::new(0.0, 4.0 + t * 4.0, 0.0),
                            ..Default::default()
                        },
                        arms,
                    )
                }),
                looks,
                len,
                2,
                Vec3::ZERO,
            ),
        }
    }
}

impl Shot for Listen {
    fn render<'a>(
        &'a self,
        t: f32,
        _len: f32,
        _f: &Frame,
        _c: &FFramesContext<'a, '_>,
    ) -> Svgr<'a> {
        let cam = Camera::new(
            Vec3::new(0.0, 1.6, 1.3),
            Vec3::new(0.0, 1.6, 0.0),
            32.0,
            W,
            H,
        );
        let navy = "#14284F";
        let mut green = Paint::line_art(navy);
        green.line = "#7CFF4F";
        green.hair = Some(navy);
        green.beard = Some(navy);
        let mut red = Paint::line_art(navy);
        red.line = "#FF4F5E";
        red.hair = Some(navy);
        fframes::svgr!(
            <g>
                {bg(navy)}
                {self.a.draw(t, &cam, &green)}
                {self.b.draw(t, &cam, &red)}
                <text x="960" y="560" font-family={BLACKLETTER} font-weight="700" font-size="110" text-anchor="middle" fill={palette::YELLOW}>"listen"</text>
            </g>
        )
    }
}

// ---------------------------------------------------------------------------------------------

/// The black arch shapes of the green section (half-round panels).
fn arches(offset: f32) -> Path {
    let mut p = Path::new();
    for (x, w, top) in [
        (-120.0, 380.0, 120.0),
        (520.0, 300.0, 300.0),
        (1240.0, 420.0, 80.0),
        (1780.0, 300.0, 260.0),
    ] {
        let x = x + offset;
        let mut pts = vec![Vec2::new(x, 1100.0), Vec2::new(x, top + w * 0.5)];
        for i in 0..=16 {
            let a = PI + i as f32 / 16.0 * PI;
            pts.push(Vec2::new(
                x + w * 0.5 + a.cos() * w * 0.5,
                top + w * 0.5 + a.sin() * w * 0.5,
            ));
        }
        pts.push(Vec2::new(x + w, 1100.0));
        pts.push(Vec2::new(x + w - 26.0, 1100.0));
        for i in (0..=16).rev() {
            let a = PI + i as f32 / 16.0 * PI;
            pts.push(Vec2::new(
                x + w * 0.5 + a.cos() * (w * 0.5 - 26.0),
                top + w * 0.5 + a.sin() * (w * 0.5 - 26.0),
            ));
        }
        pts.push(Vec2::new(x + 26.0, top + w * 0.5));
        pts.push(Vec2::new(x + 26.0, 1100.0));
        p.polygon(&pts);
    }
    p
}

/// Green: the hero, then the rivals one by one under the arches, then four panels of eyes.
pub struct Villains {
    hero: Actor,
    tall: Actor,
    red: Actor,
    eyes: Vec<Actor>,
    /// When each rival hits its poses (shot seconds).
    snaps: [[f32; 2]; 2],
}

const V_HERO: f32 = 1.55;
const V_TALL: f32 = 3.0;
const V_RED: f32 = 4.3;

/// The first beat of the song at or after shot time `t` of a shot starting at `start`.
fn beat_after(start: f32, t: f32) -> f32 {
    crate::procedural::beat_time(crate::procedural::beat(start + t).ceil()) - start
}

/// A rival striking JoJo poses on a timesheet: standing until the first snap, then each pose
/// hit on its beat with an overshoot; hands and the grin change with the poses.
fn posing(
    who: Who,
    poses: [(Strike, HandShape, HandShape); 2],
    enter: f32,
    snaps: [f32; 2],
    len: f32,
) -> Actor {
    let sk = member(who).skeleton();
    let mut keys = vec![StrikeKey {
        time: enter - 0.6,
        pose: jojo::STAND,
        snap: false,
    }];
    for (pose, at) in poses.iter().zip(snaps) {
        keys.push(StrikeKey {
            time: at,
            pose: pose.0,
            snap: true,
        });
    }
    let dance = jojo::strike_sheet(&sk, &keys, Vec3::ZERO, 0.0);
    let mut face = Acting::new(Look {
        hand_l: HandShape::Relaxed,
        hand_r: HandShape::Relaxed,
        eyes: 0.55,
        mouth: Mouth::Smirk(1.0),
        ..Default::default()
    });
    for (i, ((_, l, r), at)) in poses.iter().zip(snaps).enumerate() {
        let (l, r) = (*l, *r);
        face = face.then(at, move |look| Look {
            hand_l: l,
            hand_r: r,
            eyes: 1.0,
            mouth: Mouth::Roar(0.45 + 0.15 * i as f32),
            ..look
        });
    }
    Actor::on_sheet(
        who,
        dance.motion,
        dance.sheet,
        dance.between,
        acting(face),
        len,
    )
}

impl Villains {
    pub fn new(start: f32, len: f32) -> Self {
        let wind = Vec3::new(1.5, 0.0, 0.0);
        let hero = Actor::new(
            Who::Neogoose,
            idle(Who::Neogoose, |t| {
                (
                    Body {
                        head: Vec3::new(-25.0 + smooth((t - 0.6) / 0.3) * 25.0, 0.0, 0.0),
                        ..Default::default()
                    },
                    Arms::default(),
                )
            }),
            acting(
                Acting::new(Look::default())
                    .then(0.0, |l| Look {
                        gaze: Vec2::new(-0.5, 0.1),
                        lids: 0.3,
                        ..l
                    })
                    .then(0.5, expr::gaze(0.4, 0.0))
                    .then(0.8, |l| Look {
                        gaze: Vec2::new(0.6, 0.0),
                        ..expr::determined(l)
                    })
                    .blinks(131, 2.4),
            ),
            len,
            2,
            wind,
        );
        // LATENCY leans back behind a spread hand, then points at you; SEGFAULT crosses its arms
        // over its face, then throws its hips out with a hand on its chest
        let tall_first = beat_after(start, V_HERO + 0.22);
        let red_first = beat_after(start, V_TALL + 0.12);
        let snaps = [
            [tall_first, beat_after(start, tall_first + 0.6)],
            [red_first, beat_after(start, red_first + 0.6)],
        ];
        let tall = posing(
            Who::Latency,
            [
                (jojo::LEAN, HandShape::Open, HandShape::Open),
                (jojo::POINT, HandShape::Fist, HandShape::Point),
            ],
            V_HERO,
            snaps[0],
            len,
        );
        let red = posing(
            Who::Segfault,
            [
                (jojo::CROSS, HandShape::Open, HandShape::Open),
                (jojo::CHEST, HandShape::Open, HandShape::Open),
            ],
            V_TALL,
            snaps[1],
            len,
        );
        let eyes = [Who::Shreyas, Who::Dillon, Who::Theo, Who::Teej]
            .iter()
            .enumerate()
            .map(|(i, w)| {
                Actor::new(
                    *w,
                    idle(*w, |t| {
                        (
                            Body {
                                head: Vec3::new(t * 4.0, 0.0, 0.0),
                                ..Default::default()
                            },
                            Arms::default(),
                        )
                    }),
                    acting(
                        Acting::new(Look::default())
                            .then(0.0, |l| Look {
                                gaze: Vec2::new(0.3, 0.0),
                                ..expr::determined(l)
                            })
                            // each narrows its eyes as its own panel lands, not in unison
                            .then(V_RED + 0.3 + i as f32 * 0.09, |l| Look {
                                gaze: Vec2::ZERO,
                                pupils: 0.6,
                                brow_angle: 1.0,
                                squint: 0.6,
                                ..l
                            })
                            .blinks(140 + *w as u32, 3.2)
                            .darts(140 + *w as u32, 0.12, 0.5),
                    ),
                    len,
                    2,
                    Vec3::ZERO,
                )
            })
            .collect();
        Villains {
            hero,
            tall,
            red,
            eyes,
            snaps,
        }
    }
}

impl Shot for Villains {
    fn render<'a>(
        &'a self,
        t: f32,
        _len: f32,
        frame: &Frame,
        _c: &FFramesContext<'a, '_>,
    ) -> Svgr<'a> {
        let th = twos(t);
        let slide = -t * 30.0;
        let arch = fx::path(arches(slide).build(), palette::INK);
        let mut green_paint = Paint::two_tone(palette::GREEN_LIGHT)
            .with_suit("#06210A")
            .with_hair("#06210A");
        green_paint.beard = Some("#06210A");
        // in the close-up the hero keeps his orange: the one warm colour in the green
        let mut hero_paint = green_paint;
        hero_paint.hair = Some(palette::PUFF);
        hero_paint.beard = Some(palette::PUFF);
        if th < V_HERO {
            let close = th > 0.8;
            let cam = if close {
                let head = self.hero.posed(t).pos(Bone::Head) + Vec3::new(0.0, 0.08, 0.0);
                let mut c = Camera::orbit(head, 0.55, -70.0, 0.0, 30.0, W, H);
                c.shift = Vec2::new(420.0, 60.0);
                c
            } else {
                Camera::new(
                    Vec3::new(0.0, 1.3, 3.3),
                    Vec3::new(0.0, 1.2, 0.0),
                    34.0,
                    W,
                    H,
                )
            };
            return fframes::svgr!(<g>{bg(palette::GREEN)}{arch}{self.hero.draw(t, &cam, &hero_paint)}</g>);
        }
        if th < V_RED {
            let first = th < V_TALL;
            let (a, snaps, enter) = if first {
                (&self.tall, self.snaps[0], V_HERO)
            } else {
                (&self.red, self.snaps[1], V_TALL)
            };
            // which pose is on screen and how long since it hit
            let hit = snaps.iter().rposition(|s| *s <= th);
            let since = hit.map_or(f32::INFINITY, |i| th - snaps[i]);
            let impact = since < 1.5 / 24.0;
            // a low camera looking up, pushing in; the frame tilts one way, then the other
            let k = ((th - enter) / (V_TALL - V_HERO)).clamp(0.0, 1.0);
            let cam = Camera::new(
                Vec3::new(0.25, 0.75, 3.7 - 0.45 * k),
                Vec3::new(0.05, 1.25, 0.0),
                34.0,
                W,
                H,
            );
            let dutch = match hit {
                None => 0.0,
                Some(i) => {
                    let to = if i % 2 == 0 { -7.0 } else { 6.0 };
                    let from = if i == 0 { 0.0 } else { -7.0 };
                    from + (to - from) * ease_out((since * 24.0 / 3.0).min(1.0))
                }
            };
            let (robe, hood, lining, eye) = if first {
                (palette::BLUE, "#1C2160", "#D9B44A", "#FFE14D")
            } else {
                (palette::RED, "#4A0F0B", "#F2E6C9", "#7CFFB2")
            };
            let mut paint = Paint::two_tone("#E9F7DF")
                .with_suit("#0F1216")
                .with_robe(robe)
                .with_lining(lining)
                .with_hair(hood)
                .with_accent(eye);
            paint.line = palette::INK;
            let posed = a.posed(t);
            let head = cam.to_screen(posed.pos(Bone::Head) + Vec3::new(0.0, 0.1, 0.0));
            let hips = cam.to_screen(posed.pos(Bone::Hips));
            let aura = crate::gpu::aura(
                frame,
                hips + Vec2::new(0.0, -140.0),
                Vec2::new(950.0, 1400.0),
                fframes::Color::hex(robe).with_alpha(150),
                fframes::Color::hex("#04140A").with_alpha(200),
                0.85 + if since < 0.3 { 0.3 } else { 0.0 },
                if first { 1.0 } else { 2.0 },
            );
            // the signature of each: LATENCY waits behind a loading spinner, SEGFAULT glitches
            let halo = if first {
                spinner(head, 230.0, th)
            } else {
                Svgr::empty()
            };
            let drawn = if impact {
                // the impact frame: inverted, a black silhouette on magenta
                a.draw(t, &cam, &Paint::silhouette(palette::INK))
            } else {
                crate::gpu::rim_light(
                    a.draw(t, &cam, &paint),
                    "#B9F5C6",
                    Vec2::new(-0.5, -0.9),
                    6.0,
                )
            };
            let drawn = if !first && since < 0.2 {
                crate::gpu::rgb_split(drawn, 26.0 * (1.0 - since / 0.2), Vec2::new(1.0, 0.15))
            } else {
                drawn
            };
            let lines = if since < 0.35 {
                crate::gpu::focus_lines(
                    frame,
                    head,
                    330.0,
                    fframes::Color::hex(palette::INK).with_alpha(200),
                    140.0,
                )
            } else {
                Svgr::empty()
            };
            let back = if impact {
                bg(palette::MAGENTA)
            } else {
                bg(palette::GREEN)
            };
            let menace_side = if hit == Some(1) { 1.0 } else { -1.0 };
            let menacing = hit.map_or(Svgr::empty(), |i| {
                menace(
                    since,
                    head + Vec2::new(menace_side * 330.0, 60.0),
                    menace_side,
                    i as u32 + if first { 0 } else { 7 },
                )
            });
            let card = rival_card(&a.member, th - snaps[0], first);
            let tilt = format!("rotate({dutch:.2} 960 540)");
            let zoom = 1.0 + dutch.abs() * 0.012;
            let scale = format!("translate(960 540) scale({zoom:.3}) translate(-960 -540)");
            return fframes::svgr!(
                <g>
                    {back}
                    <g transform={tilt}>
                        <g transform={scale}>
                            {if impact { fx::path(arches(slide).build(), palette::INK) } else { arch }}
                            {if impact { Svgr::empty() } else { aura }}
                            {lines}
                            {halo}
                            {drawn}
                        </g>
                    </g>
                    {menacing}
                    {card}
                </g>
            );
        }
        // four panels of eyes sliding in
        let panels: Vec<Svgr<'a>> = self
            .eyes
            .iter()
            .enumerate()
            .map(|(i, a)| {
                let colors = [
                    palette::ORANGE,
                    palette::YELLOW_DEEP,
                    "#5BB8E8",
                    palette::RED,
                ];
                let w = 480.0;
                let x = i as f32 * w;
                // the panels slide in on ones (a compositing move, not a drawing)
                let enter = ease_out((t - V_RED - i as f32 * 0.07) / 0.26);
                let y = (1.0 - enter) * 1100.0 * if i % 2 == 0 { 1.0 } else { -1.0 };
                let posed = a.posed(t);
                let eye = posed.attach(Bone::Head, Vec3::new(0.035, 0.1, 0.085));
                let mut cam = Camera::orbit(eye, 0.24, 20.0 - i as f32 * 10.0, 0.0, 30.0, W, H);
                cam.shift = Vec2::new(x + w * 0.5 - 960.0, y);
                let id = format!("vpanel{i}");
                let clip = format!("url(#{id})");
                fframes::svgr!(
                    <g>
                        <clipPath id={id}><rect x={x} y={y} width={w} height="1080" /></clipPath>
                        <g clip-path={clip}>
                            <rect x={x} y={y} width={w} height="1080" fill={colors[i]} />
                            {crate::render::draw_in(&[(a, Paint::two_tone(colors[i]))], t, &cam, (x, y, w, 1080.0))}
                        </g>
                        <rect x={x - 6.0} y={y} width="12" height="1080" fill={palette::INK} />
                    </g>
                )
            })
            .collect();
        fframes::svgr!(<g>{bg(palette::GREEN)}{panels}</g>)
    }
}

// ---------------------------------------------------------------------------------------------

/// The Bloat turns around in black against magenta, its robe swinging out; Theo watches.
pub struct Turn {
    bloat: Actor,
    theo: Actor,
}

impl Turn {
    pub fn new(len: f32) -> Self {
        let bloat = Actor::new(
            Who::Bloat,
            idle(Who::Bloat, move |t| {
                // the turn in three keys on beats 151-153: back, three quarters, front with an
                // overshoot; the robe flares with each snap
                let yaw = match t {
                    t if t < 0.95 => 180.0,
                    t if t < 1.04 => 128.0,
                    t if t < 1.33 => 135.0,
                    t if t < 1.42 => 32.0,
                    t if t < 1.71 => 40.0,
                    t if t < 1.8 => -9.0,
                    _ => 0.0,
                };
                (
                    Body {
                        yaw,
                        head: Vec3::new(0.0, 10.0, 0.0),
                        ..Default::default()
                    },
                    Arms::default(),
                )
            }),
            look(Look {
                mouth: Mouth::Grin,
                ..Default::default()
            }),
            len,
            2,
            Vec3::ZERO,
        );
        let theo = Actor::new(
            Who::Theo,
            idle(Who::Theo, |t| {
                (
                    Body {
                        yaw: 70.0,
                        head: Vec3::new(10.0, 2.0 + t, 0.0),
                        ..Default::default()
                    },
                    Arms::default(),
                )
            }),
            acting(
                Acting::new(Look::default())
                    .then(0.0, |l| Look {
                        mouth: Mouth::Frown,
                        sweat: 0.6,
                        ..expr::worried(l)
                    })
                    .then(1.4, |l| Look {
                        gloom: 0.7,
                        ..expr::shocked(l)
                    })
                    .then(2.5, |l| Look {
                        mouth: Mouth::Teeth(0.5),
                        ..expr::determined(l)
                    })
                    .blinks(151, 1.5)
                    .darts(152, 0.2, 0.6),
            ),
            len,
            2,
            Vec3::ZERO,
        );
        Turn { bloat, theo }
    }
}

impl Shot for Turn {
    fn render<'a>(&'a self, t: f32, len: f32, _f: &Frame, _c: &FFramesContext<'a, '_>) -> Svgr<'a> {
        let th = twos(t);
        let _ = len;
        // locked: the Bloat big on the right, cut at the chest
        let cam = Camera::new(
            Vec3::new(0.05, 1.45, 1.55),
            Vec3::new(-0.32, 1.62, 0.0),
            36.0,
            W,
            H,
        );
        let head = self.theo.posed(t).pos(Bone::Head) + Vec3::new(0.0, 0.1, 0.0);
        let mut tcam = Camera::orbit(head, 0.85, 90.0, 0.0, 30.0, W, H);
        tcam.shift = Vec2::new(-640.0, 0.0);
        let mut pink = Paint::line_art(palette::MAGENTA);
        pink.line = "#3A0218";
        // the hood in ink, the face inside lit by its eyes and a magenta grin
        let mut bloat_paint = Paint::two_tone(palette::MAGENTA)
            .with_suit(palette::INK)
            .with_hair(palette::INK)
            .with_lining(palette::MAGENTA_DARK)
            .with_accent(palette::YELLOW);
        bloat_paint.shade = 0.0;
        let mut arrow = Path::new();
        arrow.polyline(&[
            Vec2::new(80.0, 1000.0 - th * 40.0),
            Vec2::new(900.0, 520.0 - th * 40.0),
        ]);
        arrow.polygon(&[
            Vec2::new(900.0, 520.0 - th * 40.0),
            Vec2::new(830.0, 520.0 - th * 40.0),
            Vec2::new(880.0, 580.0 - th * 40.0),
        ]);
        fframes::svgr!(
            <g>
                {bg(palette::MAGENTA)}
                <clipPath id="turnl"><rect x="0" y="0" width="760" height="1080" /></clipPath>
                <g clip-path="url(#turnl)">{self.theo.draw(t, &tcam, &pink)}</g>
                {fx::stroke(arrow.build(), "#3A0218", 18.0)}
                {self.bloat.draw(t, &cam, &bloat_paint)}
            </g>
        )
    }
}

// ---------------------------------------------------------------------------------------------

/// Two grinning hoods in close-up, laughing: purple, then ochre.
pub struct Grins {
    a: Actor,
    b: Actor,
}

impl Grins {
    pub fn new(len: f32) -> Self {
        let make = |who: Who, phase: f32| {
            let laugh = move |t: f32| (t * 15.0 + phase).sin();
            Actor::new(
                who,
                idle(who, move |t| {
                    let l = laugh(t);
                    (
                        Body {
                            head: Vec3::new(
                                9.0 * (t * 1.4 + phase).sin(),
                                -8.0 + l * 4.0,
                                12.0 * (t * 0.9 + phase).sin(),
                            ),
                            chest: Vec3::new(0.0, l * 2.5, 0.0),
                            ..Default::default()
                        },
                        Arms::default(),
                    )
                }),
                {
                    let face = Acting::new(Look::default())
                        .then(0.0, |l| Look {
                            eyes: 0.6,
                            gaze: Vec2::new(0.4, 0.0),
                            ..l
                        })
                        .then(0.6 + phase * 0.2, |l| Look {
                            eyes: 1.0,
                            gaze: Vec2::ZERO,
                            ..l
                        })
                        .then(1.2 + phase * 0.2, |l| Look { eyes: 0.4, ..l })
                        .blinks(160 + phase as u32, 1.6);
                    Arc::new(move |t| Look {
                        mouth: Mouth::Roar(0.35 + 0.3 * laugh(t).abs()),
                        ..face.at(t)
                    })
                },
                len,
                2,
                Vec3::new(0.8, 0.2, 0.4),
            )
        };
        Grins {
            a: make(Who::Bloat, 0.0),
            b: make(Who::Segfault, 1.3),
        }
    }
}

impl Shot for Grins {
    fn render<'a>(
        &'a self,
        t: f32,
        _len: f32,
        _f: &Frame,
        _c: &FFramesContext<'a, '_>,
    ) -> Svgr<'a> {
        let th = twos(t);
        let first = th < 1.0;
        let (actor, back, hood, eye, teeth) = if first {
            (&self.a, "#7B2CF0", palette::INK, "#5BE8FF", "#E7C8FF")
        } else {
            (
                &self.b,
                palette::YELLOW_DEEP,
                "#6A4A06",
                palette::YELLOW,
                palette::WHITE,
            )
        };
        let local = if first { t } else { t - 1.0 };
        let mut paint = Paint::two_tone(back)
            .with_suit(hood)
            .with_hair(hood)
            .with_accent(eye);
        paint.paper = teeth;
        paint.line = palette::INK;
        let head = actor
            .posed(t)
            .attach(Bone::Head, Vec3::new(0.0, 0.095, 0.02));
        let cam = Camera::orbit(
            head,
            0.62 - local * 0.06,
            -18.0 + local * 12.0,
            4.0,
            30.0,
            W,
            H,
        );
        fframes::svgr!(
            <g>
                {bg(back)}
                {actor.draw(t, &cam, &paint)}
            </g>
        )
    }
}

/// ゴゴゴ (menacing): the sound-effect katakana of a JoJo pose, popping in one after another in a
/// slanted column beside the figure and trembling there.
fn menace<'a>(since: f32, anchor: Vec2, side: f32, seed: u32) -> Svgr<'a> {
    let glyphs: Vec<Svgr<'a>> = (0..5u32)
        .filter_map(|k| {
            let shown = since - k as f32 * 1.5 / 24.0;
            if shown < 0.0 {
                return None;
            }
            let f = (shown * 24.0).floor() as u32;
            let pop = match f {
                0 => 1.4,
                1 => 0.9,
                2 => 1.06,
                _ => 1.0,
            };
            // drawn on twos: the tremble is a new drawing every other frame
            let n = (since * 12.0).floor() as u32;
            let jx = (fx::hash2(seed * 31 + k, n) - 0.5) * 7.0;
            let jy = (fx::hash2(seed * 37 + k, n + 9) - 0.5) * 7.0;
            let x = anchor.x + side * (k as f32 * 46.0) + jx;
            // the column starts under the top of the frame, however high the head is
            let top = (anchor.y - 230.0).max(245.0);
            let y = top + k as f32 * 112.0 - shown * 26.0 + jy;
            let size = (128.0 - k as f32 * 9.0) * pop;
            let rot = side * (8.0 - k as f32 * 4.0);
            let tf = format!("translate({x:.1} {y:.1}) rotate({rot:.1}) skewX(-12)");
            Some(fframes::svgr!(
                <g transform={tf}>
                    <text x="0" y="0" font-family="Dela Gothic One" font-size={size} text-anchor="middle" fill="none" stroke={palette::INK} stroke-width="16" stroke-linejoin="round">"ゴ"</text>
                    <text x="0" y="0" font-family="Dela Gothic One" font-size={size} text-anchor="middle" fill="#9B2CF0" stroke="#F5E6FF" stroke-width="3">"ゴ"</text>
                </g>
            ))
        })
        .collect();
    fframes::svgr!(<g>{glyphs}</g>)
}

/// A loading spinner as a halo: a ring of dots, the lit one stepping round on twos with a
/// fading tail.
fn spinner<'a>(c: Vec2, r: f32, t: f32) -> Svgr<'a> {
    let step = (t * 12.0).floor();
    let dots: Vec<Svgr<'a>> = (0..12)
        .map(|i| {
            let a = (i as f32 / 12.0) * std::f32::consts::TAU - std::f32::consts::FRAC_PI_2;
            let p = c + Vec2::new(a.cos(), a.sin()) * r;
            let age = ((step - i as f32).rem_euclid(12.0)) / 12.0;
            let o = (1.0 - age).powf(1.6) * 0.9 + 0.08;
            let rr = 15.0 + 9.0 * (1.0 - age);
            fframes::svgr!(<circle cx={p.x} cy={p.y} r={rr} fill="#BFF0FF" opacity={o} stroke={palette::INK} stroke-width="4" />)
        })
        .collect();
    fframes::svgr!(<g>{dots}</g>)
}

/// The rival's name card (名前テロップ) slammed in with its first pose: a slanted slab, the name,
/// the katakana over it.
fn rival_card<'a>(m: &crate::cast::Member, t: f32, left: bool) -> Svgr<'a> {
    if t < 0.0 {
        return Svgr::empty();
    }
    let f = (t * 24.0).floor() as u32;
    let slam = match f {
        0 => 1.35,
        1 => 0.92,
        2 => 1.04,
        _ => 1.0,
    };
    let slide = (1.0 - ease_out((t / 0.12).min(1.0))) * 900.0;
    let (x, anchor, sx) = if left {
        (90.0, "start", -slide)
    } else {
        (1830.0, "end", slide)
    };
    let y = 930.0;
    let slab = if left {
        "M0 800 L760 772 L740 990 L0 1010 Z"
    } else {
        "M1920 800 L1160 772 L1180 990 L1920 1010 Z"
    };
    let tf = format!("translate({sx:.1} 0)");
    let pop = format!(
        "translate({x:.1} {y:.1}) scale({slam:.3}) translate({:.1} {:.1})",
        -x, -y
    );
    let name = m.name.to_owned();
    let kana = m.katakana.to_owned();
    fframes::svgr!(
        <g transform={tf}>
            <path d={slab} fill={palette::INK} opacity="0.92" />
            <g transform={pop}>
                <text x={x} y={y - 92.0} font-family="Dela Gothic One" font-size="46" text-anchor={anchor} fill="#F5E6FF" letter-spacing="10">{kana}</text>
                <text x={x} y={y} font-family="Dela Gothic One" font-size="118" text-anchor={anchor} fill={palette::YELLOW} letter-spacing="6">{name}</text>
            </g>
        </g>
    )
}
