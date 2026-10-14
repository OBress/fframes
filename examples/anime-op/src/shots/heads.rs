//! Head and hand sheet of the cast at three angles, for designing faces and hair.

use fframes::{FFramesContext, Frame, Svgr};
use fframes_anime::camera::Camera;
use fframes_anime::glam::{Vec2, Vec3};
use fframes_anime::rig::Bone;
use fframes_anime::toon::{HandShape, Look, Mouth};

use crate::actor::{Actor, look};
use crate::cast::{ALL, member};
use crate::palette::{self, Paint};
use crate::procedural::{Body, reach_local};
use crate::{H, Shot, W};

pub struct Heads {
    actors: Vec<Actor>,
}

impl Heads {
    pub fn new() -> Self {
        let actors = ALL
            .iter()
            .map(|who| {
                let sk = member(*who).skeleton();
                let motion: crate::motion::MotionFn = std::sync::Arc::new(move |t: f32| {
                    let body = Body {
                        root: Vec3::new(0.0, sk.proportions.hip_height, 0.0),
                        head: Vec3::new(0.0, 0.0, 6.0 * (t * 2.0).sin()),
                        ..Default::default()
                    };
                    let mut pose = body.pose(&sk);
                    reach_local(
                        &sk,
                        &mut pose,
                        fframes_anime::rig::Side::Left,
                        Vec3::new(0.12, 0.2, 0.3),
                        0.8,
                    );
                    reach_local(
                        &sk,
                        &mut pose,
                        fframes_anime::rig::Side::Right,
                        Vec3::new(-0.18, 0.1, 0.32),
                        0.8,
                    );
                    pose
                });
                Actor::new(
                    *who,
                    motion,
                    look(Look {
                        mouth: Mouth::Smile,
                        hand_l: HandShape::Fist,
                        hand_r: HandShape::Open,
                        ..Default::default()
                    }),
                    4.0,
                    1,
                    Vec3::ZERO,
                )
            })
            .collect();
        Heads { actors }
    }
}

impl Shot for Heads {
    fn render<'a>(
        &'a self,
        t: f32,
        _len: f32,
        _frame: &Frame,
        _ctx: &FFramesContext<'a, '_>,
    ) -> Svgr<'a> {
        let paint = Paint::two_tone(palette::YELLOW);
        let mut cells: Vec<Svgr<'a>> = Vec::new();
        // 6 rows would be too small: 3 columns of angles x 6 characters in a 6x3 grid.
        let (cw, ch) = (W / 6.0, H / 3.0);
        for (i, a) in self.actors.iter().enumerate() {
            let head = a.posed(t).attach(Bone::Head, Vec3::new(0.0, 0.1, 0.0));
            for (j, yaw) in [0.0_f32, 40.0, 150.0].iter().enumerate() {
                let mut cam = Camera::orbit(head, 0.75, *yaw, 6.0, 30.0, W, H);
                let cx = cw * (i as f32 + 0.5);
                let cy = ch * (j as f32 + 0.5);
                cam.shift = Vec2::new(cx - W * 0.5, cy - H * 0.5);
                cam.fov_y = 30.0 * 3.0;
                let id = format!("cell{i}x{j}");
                let clip = format!("url(#{id})");
                let (x0, y0) = (cx - cw * 0.5 + 2.0, cy - ch * 0.5 + 2.0);
                cells.push(fframes::svgr!(
                    <g>
                        <clipPath id={id}><rect x={x0} y={y0} width={cw - 4.0} height={ch - 4.0} /></clipPath>
                        <g clip-path={clip}>{a.draw(t, &cam, &paint)}</g>
                    </g>
                ));
            }
        }
        fframes::svgr!(
            <g>
                <rect x="0" y="0" width="1920" height="1080" fill={palette::YELLOW} />
                {cells}
            </g>
        )
    }
}

/// One character's head from six angles (front, 3/4, profile, back 3/4, above, below).
pub struct HeroHead {
    a: Actor,
    dist: f32,
    views: [(f32, f32); 6],
}

impl HeroHead {
    pub fn new(who: crate::cast::Who) -> Self {
        Self::with_views(
            who,
            1.35,
            [
                (0.0, 0.0),
                (35.0, 5.0),
                (90.0, 0.0),
                (150.0, 5.0),
                (20.0, 50.0),
                (-25.0, -30.0),
            ],
        )
    }

    /// Close views turning from the front to the profile (noses, mouths, beards).
    pub fn turning(who: crate::cast::Who) -> Self {
        Self::with_views(
            who,
            0.95,
            [
                (0.0, 0.0),
                (20.0, 0.0),
                (38.0, 2.0),
                (55.0, 0.0),
                (-38.0, 2.0),
                (75.0, 0.0),
            ],
        )
    }

    fn with_views(who: crate::cast::Who, dist: f32, views: [(f32, f32); 6]) -> Self {
        let sk = member(who).skeleton();
        let motion: crate::motion::MotionFn = std::sync::Arc::new(move |t: f32| {
            let body = Body {
                root: Vec3::new(0.0, sk.proportions.hip_height, 0.0),
                head: Vec3::new(10.0 * (t * 1.5).sin(), 0.0, 0.0),
                ..Default::default()
            };
            body.pose(&sk)
        });
        HeroHead {
            a: Actor::new(
                who,
                motion,
                look(Look {
                    mouth: Mouth::Smile,
                    ..Default::default()
                }),
                4.0,
                1,
                Vec3::ZERO,
            ),
            dist,
            views,
        }
    }
}

impl Shot for HeroHead {
    fn render<'a>(
        &'a self,
        t: f32,
        _len: f32,
        _frame: &Frame,
        _ctx: &FFramesContext<'a, '_>,
    ) -> Svgr<'a> {
        let paint = Paint::two_tone(palette::YELLOW);
        let head = self.a.posed(t).pos(Bone::Head) + Vec3::new(0.0, 0.1, 0.0);
        let (cw, ch) = (W / 3.0, H / 2.0);
        let cells: Vec<Svgr<'a>> = self
            .views
            .iter()
            .enumerate()
            .map(|(i, (yaw, pitch))| {
                let cx = cw * ((i % 3) as f32 + 0.5);
                let cy = ch * ((i / 3) as f32 + 0.5);
                let mut cam = Camera::orbit(head + Vec3::new(0.0, 0.06, 0.0), self.dist, *yaw, *pitch, 30.0, W, H);
                cam.shift = Vec2::new(cx - W * 0.5, cy - H * 0.5);
                let id = format!("hh{i}");
                let clip = format!("url(#{id})");
                let mut debug: Vec<Svgr<'a>> = Vec::new();
                if std::env::var("ANIME_OP_CHAINS").is_ok() {
                    let frame = self.a.frame(t);
                    let colors = ["#E6194B", "#3CB44B", "#4363D8", "#F58231", "#911EB4", "#42D4F4", "#F032E6", "#BFEF45", "#469990", "#9A6324", "#800000", "#000075", "#FFFFFF", "#A9A9A9"];
                    for (k, ci) in self.a.layout().hair.clone().enumerate() {
                        let Some(chain) = frame.sim.chains.get(ci) else { continue };
                        let pts: Vec<Vec2> = chain.iter().map(|p| cam.to_screen(*p)).collect();
                        let mut path = fframes_anime::shape::Path::new();
                        path.polyline(&pts);
                        let color = colors[k % colors.len()];
                        let end = pts[pts.len() - 1];
                        let label = format!("{k}");
                        debug.push(fframes::svgr!(
                            <g>
                                <path d={path.build()} fill="none" stroke={color} stroke-width="4" />
                                <text x={end.x} y={end.y} font-family="Zen Kaku Gothic New" font-size="22" fill={color}>{label}</text>
                            </g>
                        ));
                    }
                }
                fframes::svgr!(
                    <g>
                        <clipPath id={id}><rect x={cx - cw * 0.5 + 2.0} y={cy - ch * 0.5 + 2.0} width={cw - 4.0} height={ch - 4.0} /></clipPath>
                        <g clip-path={clip}>{self.a.draw(t, &cam, &paint)}{debug}</g>
                    </g>
                )
            })
            .collect();
        fframes::svgr!(<g><rect x="0" y="0" width="1920" height="1080" fill={palette::YELLOW} />{cells}</g>)
    }
}

/// The five hand shapes up close, palm and back, for designing hands.
pub struct Hands {
    actors: Vec<Actor>,
}

impl Hands {
    pub fn new() -> Self {
        let shapes = [
            HandShape::Relaxed,
            HandShape::Fist,
            HandShape::Open,
            HandShape::Point,
            HandShape::Peace,
        ];
        let actors = shapes
            .iter()
            .map(|shape| {
                let sk = member(crate::cast::Who::Neogoose).skeleton();
                let motion: crate::motion::MotionFn = std::sync::Arc::new(move |_t: f32| {
                    let body = Body {
                        root: Vec3::new(0.0, sk.proportions.hip_height, 0.0),
                        ..Default::default()
                    };
                    let mut pose = body.pose(&sk);
                    reach_local(
                        &sk,
                        &mut pose,
                        fframes_anime::rig::Side::Right,
                        Vec3::new(-0.15, 0.15, 0.4),
                        0.8,
                    );
                    pose
                });
                Actor::new(
                    crate::cast::Who::Neogoose,
                    motion,
                    look(Look {
                        hand_r: *shape,
                        hand_l: *shape,
                        ..Default::default()
                    }),
                    1.0,
                    1,
                    Vec3::ZERO,
                )
            })
            .collect();
        Hands { actors }
    }
}

impl Shot for Hands {
    fn render<'a>(
        &'a self,
        t: f32,
        _len: f32,
        _frame: &Frame,
        _ctx: &FFramesContext<'a, '_>,
    ) -> Svgr<'a> {
        let paint = Paint::line_art(palette::YELLOW);
        let (cw, ch) = (W / 5.0, H / 3.0);
        let mut cells: Vec<Svgr<'a>> = Vec::new();
        for (i, a) in self.actors.iter().enumerate() {
            let posed = a.posed(t);
            let hand = posed.attach(Bone::HandR, Vec3::new(-0.08, 0.0, 0.0));
            for (j, (yaw, pitch)) in [(0.0_f32, 10.0_f32), (90.0, 20.0), (200.0, -40.0)]
                .iter()
                .enumerate()
            {
                let mut cam = Camera::orbit(hand, 1.5, *yaw, *pitch, 30.0, W, H);
                let cx = cw * (i as f32 + 0.5);
                let cy = ch * (j as f32 + 0.5);
                cam.shift = Vec2::new(cx - W * 0.5, cy - H * 0.5);
                let id = format!("hand{i}x{j}");
                let clip = format!("url(#{id})");
                cells.push(fframes::svgr!(
                    <g>
                        <clipPath id={id}><rect x={cx - cw * 0.5 + 2.0} y={cy - ch * 0.5 + 2.0} width={cw - 4.0} height={ch - 4.0} /></clipPath>
                        <g clip-path={clip}>{a.draw(t, &cam, &paint)}</g>
                    </g>
                ));
            }
        }
        fframes::svgr!(<g><rect x="0" y="0" width="1920" height="1080" fill={palette::YELLOW} />{cells}</g>)
    }
}

/// A model sheet (設定): one character standing, front, three quarter, side and back, in
/// two-tone above and line art below, with a head-height scale.
pub struct Sheet {
    a: Actor,
}

impl Sheet {
    pub fn new(who: crate::cast::Who) -> Self {
        let sk = member(who).skeleton();
        let motion: crate::motion::MotionFn = std::sync::Arc::new(move |_t: f32| {
            let body = Body {
                root: Vec3::new(0.0, sk.proportions.hip_height, 0.0),
                ..Default::default()
            };
            let mut pose = body.pose(&sk);
            reach_local(
                &sk,
                &mut pose,
                fframes_anime::rig::Side::Left,
                Vec3::new(0.2, -0.3, 0.04),
                0.3,
            );
            reach_local(
                &sk,
                &mut pose,
                fframes_anime::rig::Side::Right,
                Vec3::new(-0.2, -0.3, 0.04),
                0.3,
            );
            pose
        });
        Sheet {
            a: Actor::new(
                who,
                motion,
                look(Look {
                    mouth: Mouth::Line,
                    ..Default::default()
                }),
                2.0,
                1,
                Vec3::ZERO,
            ),
        }
    }
}

impl Shot for Sheet {
    fn render<'a>(
        &'a self,
        t: f32,
        _len: f32,
        _frame: &Frame,
        _ctx: &FFramesContext<'a, '_>,
    ) -> Svgr<'a> {
        let (cw, ch) = (W / 4.0, H / 2.0);
        let mut cells: Vec<Svgr<'a>> = Vec::new();
        let posed = self.a.posed(t);
        let top = posed.head_top().y;
        let center = Vec3::new(0.0, top * 0.5, 0.0);
        let slop = self.a.member.who == crate::cast::Who::Demon;
        for (j, paint) in [
            if slop {
                crate::shots::finale::demon_paint()
            } else {
                Paint::two_tone(palette::YELLOW)
            },
            Paint::line_art(palette::YELLOW),
        ]
        .iter()
        .enumerate()
        {
            for (i, yaw) in [0.0_f32, 35.0, 90.0, 180.0].iter().enumerate() {
                let mut cam = Camera::orbit(center, 9.0, *yaw, 0.0, 26.0, W, H);
                let cx = cw * (i as f32 + 0.5);
                let cy = ch * (j as f32 + 0.5);
                cam.shift = Vec2::new(cx - W * 0.5, cy - H * 0.5);
                let id = format!("sheet{i}x{j}");
                let clip = format!("url(#{id})");
                // head heights
                let mut ticks = fframes_anime::shape::Path::new();
                let head = posed.proportions.head_height;
                let mut y = 0.0;
                while y <= top + 0.01 {
                    let p = cam.to_screen(Vec3::new(-0.45, y, 0.0));
                    ticks.polyline(&[p, p + Vec2::new(14.0, 0.0)]);
                    y += head;
                }
                cells.push(fframes::svgr!(
                    <g>
                        <clipPath id={id}><rect x={cx - cw * 0.5 + 2.0} y={cy - ch * 0.5 + 2.0} width={cw - 4.0} height={ch - 4.0} /></clipPath>
                        <g clip-path={clip}>{self.a.draw(t, &cam, paint)}<path d={ticks.build()} stroke={palette::INK} stroke-width="2" fill="none" /></g>
                    </g>
                ));
            }
        }
        fframes::svgr!(<g><rect x="0" y="0" width="1920" height="1080" fill={palette::YELLOW} />{cells}</g>)
    }
}

/// The expression sheet (表情集) of a cast member: twelve faces, the last row blinking, darting
/// and singing over time.
pub struct Expressions {
    cells: Vec<(&'static str, Actor)>,
}

pub fn expressions() -> Vec<(&'static str, Look)> {
    let base = Look::default();
    vec![
        ("neutral", base),
        (
            "happy",
            Look {
                squint: 0.6,
                brows: 0.3,
                mouth: Mouth::Smile,
                ..base
            },
        ),
        (
            "laugh",
            Look {
                eyes: 0.0,
                happy: true,
                brows: 0.5,
                mouth: Mouth::Open(0.8),
                blush: 0.5,
                ..base
            },
        ),
        (
            "angry",
            Look {
                brow_angle: 1.0,
                pupils: 0.75,
                mouth: Mouth::Teeth(0.6),
                vein: 1.0,
                ..base
            },
        ),
        (
            "shocked",
            Look {
                pupils: 0.4,
                brows: 1.0,
                mouth: Mouth::O(0.8),
                gloom: 0.9,
                sweat: 1.0,
                ..base
            },
        ),
        (
            "smug",
            Look {
                lids: 0.65,
                brow_tilt: 0.7,
                mouth: Mouth::Smirk(1.0),
                gaze: Vec2::new(0.5, 0.0),
                ..base
            },
        ),
        (
            "worried",
            Look {
                brow_angle: -1.0,
                mouth: Mouth::Wavy,
                sweat: 0.8,
                gaze: Vec2::new(-0.8, -0.2),
                ..base
            },
        ),
        (
            "determined",
            Look {
                brow_angle: 0.6,
                squint: 0.45,
                mouth: Mouth::Line,
                ..base
            },
        ),
        (
            "wink",
            Look {
                wink: 1.0,
                sparkle: 1.0,
                mouth: Mouth::Cat,
                ..base
            },
        ),
        (
            "crying",
            Look {
                brow_angle: -1.0,
                tears: 1.0,
                mouth: Mouth::Open(0.5),
                ..base
            },
        ),
        (
            "shy",
            Look {
                blush: 1.0,
                gaze: Vec2::new(0.3, -0.8),
                lids: 0.3,
                mouth: Mouth::Pout,
                ..base
            },
        ),
        (
            "bored",
            Look {
                lids: 1.0,
                gaze: Vec2::new(0.0, -0.3),
                mouth: Mouth::Line,
                brows: -0.4,
                ..base
            },
        ),
    ]
}

impl Expressions {
    pub fn new(who: crate::cast::Who) -> Self {
        let sk = member(who).skeleton();
        let motion: crate::motion::MotionFn = std::sync::Arc::new(move |_t: f32| {
            let body = Body {
                root: Vec3::new(0.0, sk.proportions.hip_height, 0.0),
                ..Default::default()
            };
            body.pose(&sk)
        });
        let mut cells: Vec<(&'static str, Actor)> = expressions()
            .into_iter()
            .map(|(name, l)| {
                (
                    name,
                    Actor::new(who, motion.clone(), look(l), 4.0, 2, Vec3::ZERO),
                )
            })
            .collect();
        // acting over time: blinks and darts, a sung line, a change of mind
        let acting = [
            (
                "blinks + darts",
                fframes_anime::acting::Acting::new(Look::default())
                    .blinks(7, 1.2)
                    .darts(3, 0.7, 0.5),
            ),
            (
                "bling-bang-bang-born",
                fframes_anime::acting::Acting::new(Look {
                    squint: 0.3,
                    ..Default::default()
                })
                .sing(&[0.2, 0.4, 0.6, 1.0, 1.2, 1.4, 1.6], "iaaiaao", 0.12)
                .blinks(9, 2.0),
            ),
            (
                "neutral > shocked > angry",
                fframes_anime::acting::Acting::new(Look::default())
                    .then(1.0, |l| Look {
                        pupils: 0.4,
                        brows: 1.0,
                        mouth: Mouth::O(0.8),
                        sweat: 1.0,
                        ..l
                    })
                    .then(2.2, |l| Look {
                        pupils: 0.8,
                        brows: 0.0,
                        brow_angle: 1.0,
                        mouth: Mouth::Teeth(0.8),
                        vein: 1.0,
                        sweat: 0.0,
                        ..l
                    })
                    .blinks(5, 3.0),
            ),
        ];
        for (name, a) in acting {
            cells.push((
                name,
                Actor::new(
                    who,
                    motion.clone(),
                    std::sync::Arc::new(move |t| a.at(t)),
                    4.0,
                    2,
                    Vec3::ZERO,
                ),
            ));
        }
        Expressions { cells }
    }
}

impl Shot for Expressions {
    fn render<'a>(
        &'a self,
        t: f32,
        _len: f32,
        _frame: &Frame,
        _ctx: &FFramesContext<'a, '_>,
    ) -> Svgr<'a> {
        let (cw, ch) = (W / 5.0, H / 3.0);
        let paint = Paint::two_tone(palette::YELLOW);
        let mut cells: Vec<Svgr<'a>> = Vec::new();
        for (i, (name, a)) in self.cells.iter().enumerate() {
            let posed = a.posed(t);
            let head = posed.attach(Bone::Head, Vec3::new(0.0, 0.11, 0.0));
            let mut cam = Camera::new(head + Vec3::new(0.0, 0.0, 3.2), head, 22.0, W, H);
            let (cx, cy) = (cw * ((i % 5) as f32 + 0.5), ch * ((i / 5) as f32 + 0.5));
            cam.shift = Vec2::new(cx - W * 0.5, cy - H * 0.5 - 12.0);
            let label = name.to_string();
            let id = format!("expr{i}");
            let clip = format!("url(#{id})");
            let (x0, y0) = (cx - cw * 0.5 + 3.0, cy - ch * 0.5 + 3.0);
            let cell = a.draw(t, &cam, &paint);
            cells.push(fframes::svgr!(
                <g>
                    <clipPath id={id}><rect x={x0} y={y0} width={cw - 6.0} height={ch - 6.0} /></clipPath>
                    <g clip-path={clip}>{cell}</g>
                </g>
            ));
            cells.push(fframes::svgr!(
                <text x={x0 + 8.0} y={y0 + 26.0} font-family="Zen Kaku Gothic New" font-weight="700" font-size="22" fill={palette::INK}>{label}</text>
            ));
        }
        fframes::svgr!(
            <g>
                <rect x="0" y="0" width="1920" height="1080" fill={palette::YELLOW} />
                {cells}
            </g>
        )
    }
}

/// The props from six angles, for modelling them.
pub struct Props;

impl Shot for Props {
    fn render<'a>(
        &'a self,
        t: f32,
        _len: f32,
        _frame: &Frame,
        _ctx: &FFramesContext<'a, '_>,
    ) -> Svgr<'a> {
        let (cw, ch) = (W / 3.0, H / 2.0);
        let views = [
            (0.0_f32, 8.0_f32),
            (30.0, 25.0),
            (90.0, 0.0),
            (0.0, 60.0),
            (-40.0, -12.0),
            (160.0, 35.0),
        ];
        let puff = crate::props::cream_puff();
        let cells: Vec<Svgr<'a>> = views
            .iter()
            .enumerate()
            .map(|(i, (yaw, pitch))| {
                let cx = cw * ((i % 3) as f32 + 0.5);
                let cy = ch * ((i / 3) as f32 + 0.5);
                let mut cam = Camera::orbit(Vec3::new(0.0, 0.04, 0.0), 0.42, *yaw + t * 20.0, *pitch, 30.0, W, H);
                cam.shift = Vec2::new(cx - W * 0.5, cy - H * 0.5);
                let at = fframes_anime::glam::Affine3A::IDENTITY;
                let drawn = crate::render::draw_with(&[], &[crate::render::Placed::new(puff, at, t)], t, &cam);
                let id = format!("pp{i}");
                let clip = format!("url(#{id})");
                fframes::svgr!(
                    <g>
                        <clipPath id={id}><rect x={cx - cw * 0.5 + 2.0} y={cy - ch * 0.5 + 2.0} width={cw - 4.0} height={ch - 4.0} /></clipPath>
                        <g clip-path={clip}>{drawn}</g>
                    </g>
                )
            })
            .collect();
        fframes::svgr!(<g><rect x="0" y="0" width="1920" height="1080" fill={palette::YELLOW} />{cells}</g>)
    }
}

/// A rival striking every JoJo pose, front and three quarters.
pub struct Poses {
    actors: Vec<(Actor, &'static str)>,
}

impl Poses {
    pub fn new(who: crate::cast::Who) -> Self {
        use crate::jojo::{CHEST, CROSS, LEAN, POINT, STAND};
        let sk = member(who).skeleton();
        let actors = [
            (STAND, "STAND"),
            (LEAN, "LEAN"),
            (POINT, "POINT"),
            (CROSS, "CROSS"),
            (CHEST, "CHEST"),
        ]
        .into_iter()
        .map(|(pose, name)| {
            let sk = sk.clone();
            let m: crate::motion::MotionFn =
                std::sync::Arc::new(move |_| pose.build(&sk, Vec3::ZERO, 0.0));
            let hands = match name {
                "POINT" => (HandShape::Fist, HandShape::Point),
                "STAND" => (HandShape::Relaxed, HandShape::Relaxed),
                _ => (HandShape::Open, HandShape::Open),
            };
            (
                Actor::new(
                    who,
                    m,
                    look(Look {
                        hand_l: hands.0,
                        hand_r: hands.1,
                        mouth: Mouth::Roar(0.4),
                        ..Default::default()
                    }),
                    1.0,
                    1,
                    Vec3::ZERO,
                ),
                name,
            )
        })
        .collect();
        Poses { actors }
    }
}

impl Shot for Poses {
    fn render<'a>(
        &'a self,
        t: f32,
        _len: f32,
        _frame: &Frame,
        _ctx: &FFramesContext<'a, '_>,
    ) -> Svgr<'a> {
        let n = self.actors.len();
        let (cw, ch) = (W / n as f32, H / 2.0);
        let paint = Paint::two_tone(palette::GREEN_LIGHT)
            .with_suit(palette::BLUE)
            .with_lining("#D9B44A");
        let cells: Vec<Svgr<'a>> = self
            .actors
            .iter()
            .enumerate()
            .flat_map(|(i, (a, name))| {
                [0.0_f32, 35.0].into_iter().enumerate().map(move |(row, yaw)| {
                    let cx = cw * (i as f32 + 0.5);
                    let cy = ch * (row as f32 + 0.5);
                    let mut cam = Camera::orbit(Vec3::new(0.0, 1.0, 0.0), 9.5, yaw, 6.0, 30.0, W, H);
                    cam.shift = Vec2::new(cx - W * 0.5, cy - H * 0.5);
                    let id = format!("jj{i}x{row}");
                    let clip = format!("url(#{id})");
                    let label = (*name).to_owned();
                    fframes::svgr!(
                        <g>
                            <clipPath id={id}><rect x={cx - cw * 0.5 + 2.0} y={cy - ch * 0.5 + 2.0} width={cw - 4.0} height={ch - 4.0} /></clipPath>
                            <g clip-path={clip}>{a.draw(t, &cam, &paint)}</g>
                            <text x={cx - cw * 0.5 + 12.0} y={cy - ch * 0.5 + 34.0} font-family="Dela Gothic One" font-size="26" fill={palette::INK}>{label}</text>
                        </g>
                    )
                })
            })
            .collect();
        fframes::svgr!(<g><rect x="0" y="0" width="1920" height="1080" fill={palette::GREEN_LIGHT} />{cells}</g>)
    }
}
