//! The friends arrive (24.6-32 s): Kovanikov panics at the door, the eye close-up, the walk of
//! two, Shreyas laughing, the six of them walking at the camera with long shadows.

use std::sync::Arc;

use fframes::{FFramesContext, Frame, Svgr};
use fframes_anime::camera::Camera;
use fframes_anime::glam::{Vec2, Vec3};
use fframes_anime::rig::{Bone, Side};
use fframes_anime::shape::Path;
use fframes_anime::toon::{HandShape, Look, Mouth};

use crate::actor::{Actor, acting};
use crate::cast::{Who, member};
use crate::expr::{self, Acting};
use crate::fx::{self, bg, ease_out, hash2, spring, twos};
use crate::motion::{Library, Play, in_place};
use crate::palette::{self, Paint};
use crate::procedural::{Arms, Body, build, motion};
use crate::shots::intro::name_bands;
use crate::{H, Shot, W, stage};

/// Light for the long evening shadows: low, from the right and behind.
const SUN: Vec3 = Vec3::new(-1.0, -0.32, -0.06);

/// The hero munching calmly, Kovanikov jumping back in shock.
pub struct Door {
    hero: Actor,
    friend: Actor,
    /// The cream puff in the hero's hand.
    puff: crate::physics3d::Baked3,
}

const JOLT: f32 = 0.22;

impl Door {
    pub fn new(len: f32) -> Self {
        let sk = member(Who::Neogoose).skeleton();
        let hip = sk.proportions.hip_height;
        let hero = Actor::new(
            Who::Neogoose,
            motion(move |t| {
                let body = Body {
                    root: Vec3::new(-0.45, hip, 0.0),
                    yaw: 25.0,
                    head: Vec3::new(-10.0, 4.0 + (t * 9.0).sin() * 2.0, 0.0),
                    ..Default::default()
                };
                build(
                    &sk,
                    body,
                    Arms {
                        right: Some(Vec3::new(-0.13, 0.1, 0.27)),
                        right_palm: Some(Vec3::new(0.15, 1.0, -0.1)),
                        right_fingers: Some(Vec3::new(-0.3, 0.1, 1.0)),
                        ..Default::default()
                    },
                    None,
                )
            }),
            // munching away, unbothered: a side-eye at the commotion, still chewing
            acting(expr::chew(
                Acting::new(Look {
                    lids: 0.35,
                    hand_r: HandShape::Relaxed,
                    ..Default::default()
                })
                .then(JOLT + 0.12, expr::gaze(0.85, 0.0))
                .then(JOLT + 0.5, |l| Look {
                    gaze: Vec2::new(0.6, 0.0),
                    ..expr::smug(l)
                })
                .blinks(21, 2.0),
                0.0,
                len,
                0.17,
            )),
            len,
            2,
            Vec3::ZERO,
        );
        let fig = crate::render::figure(Who::Neogoose);
        let hero_motion = hero.motion.clone();
        let hsk = member(Who::Neogoose).skeleton();
        let puff = crate::physics3d::in_palm(
            &fig,
            &|t| hsk.solve(&hero_motion(t)),
            fframes_anime::rig::Side::Right,
            HandShape::Relaxed,
            crate::props::puff_hull(),
            0.9,
            0.6,
            len,
        );
        let sk2 = member(Who::Chshersh).skeleton();
        let hip2 = sk2.proportions.hip_height;
        let friend = Actor::new(
            Who::Chshersh,
            motion(move |t| {
                // a jolt back with an overshooting spring, drawn on twos (the whole figure, not
                // only the pose: a jolt slid on ones reads as CG), and a shake that alternates
                // drawing by drawing for four drawings
                let t = fframes_anime::koma::hold(t, 24.0, 2);
                let k = spring(t - JOLT, 2.6, 7.0).max(0.0);
                let n = ((t - JOLT) * 12.0).round() as i32;
                let shake = if t > JOLT && n < 4 {
                    if n % 2 == 0 { 0.02 } else { -0.02 }
                } else {
                    0.0
                };
                let body = Body {
                    root: Vec3::new(0.55 + 0.18 * k + shake, hip2 - 0.04 * k, -0.1 * k),
                    yaw: -30.0,
                    spine: Vec3::new(0.0, -10.0 * k, 0.0),
                    chest: Vec3::new(0.0, -8.0 * k, 0.0),
                    head: Vec3::new(0.0, -6.0 * k, 4.0 * k),
                    ..Default::default()
                };
                if t < JOLT {
                    return build(&sk2, body, Arms::default(), None);
                }
                let up = k.min(1.2);
                build(
                    &sk2,
                    body,
                    Arms {
                        left: Some(Vec3::new(0.3, -0.05 + 0.5 * up, 0.1)),
                        right: Some(Vec3::new(-0.3, -0.05 + 0.5 * up, 0.1)),
                        elbow_out: 1.6,
                        left_palm: Some(Vec3::new(0.0, 0.1, 1.0)),
                        right_palm: Some(Vec3::new(0.0, 0.1, 1.0)),
                        left_fingers: None,
                        right_fingers: None,
                    },
                    None,
                )
            }),
            {
                // calm, then the jolt: shock, gloom, a stammer
                let face = Acting::new(Look::default())
                    .darts(8, 0.3, 0.4)
                    .then(JOLT, |l| Look {
                        mouth: Mouth::Open(1.0),
                        ..expr::shocked(l)
                    })
                    .then(JOLT + 0.45, |l| Look { tears: 0.4, ..l })
                    .lips(&[
                        (JOLT + 0.45, Mouth::Wavy),
                        (JOLT + 0.6, Mouth::O(0.7)),
                        (JOLT + 0.72, Mouth::Wavy),
                        (JOLT + 0.86, Mouth::Open(0.8)),
                    ])
                    .blink_at(JOLT + 0.62);
                Arc::new(move |t| {
                    let hand = if t > JOLT {
                        HandShape::Open
                    } else {
                        HandShape::Relaxed
                    };
                    Look {
                        hand_l: hand,
                        hand_r: hand,
                        glint: if t > JOLT {
                            ((t - JOLT) * 3.0).min(1.0)
                        } else {
                            0.0
                        },
                        ..face.at(t)
                    }
                })
            },
            len,
            2,
            Vec3::ZERO,
        );
        Door { hero, friend, puff }
    }
}

impl Shot for Door {
    fn render<'a>(
        &'a self,
        t: f32,
        _len: f32,
        _f: &Frame,
        _c: &FFramesContext<'a, '_>,
    ) -> Svgr<'a> {
        let th = twos(t);
        let cam = Camera::new(
            Vec3::new(0.05, 1.45, 2.05),
            Vec3::new(0.05, 1.38, 0.0),
            30.0,
            W,
            H,
        );
        // a tall arched wooden door behind them: it bangs open in three drawings on the jolt
        let mut arch = Path::new();
        let mut arch_pts = vec![Vec2::new(1180.0, 1100.0), Vec2::new(1180.0, 200.0)];
        for i in 0..=12 {
            let a = std::f32::consts::PI + i as f32 / 12.0 * std::f32::consts::PI;
            arch_pts.push(Vec2::new(1420.0 + a.cos() * 240.0, 200.0 + a.sin() * 160.0));
        }
        arch_pts.push(Vec2::new(1660.0, 1100.0));
        arch.polygon(&arch_pts);
        let open = match ((th - JOLT) * 12.0).floor() as i32 {
            i32::MIN..=-1 => 0.0,
            0 => 0.55,
            1 => 1.08,
            _ => 1.0,
        };
        // the leaf swings on its hinge on the left, towards us: narrower, taller at the free edge
        let w = 480.0 * (1.0 - 0.78 * open);
        let lift = 70.0 * open;
        let mut leaf = Path::new();
        leaf.polygon(&[
            Vec2::new(1180.0, 1100.0),
            Vec2::new(1180.0, 200.0),
            Vec2::new(1180.0 + w, 200.0 - lift),
            Vec2::new(1180.0 + w, 1100.0 + lift),
        ]);
        let mut door = Path::new();
        for k in 1..5 {
            let x = 1180.0 + w * k as f32 / 5.0;
            door.polyline(&[
                Vec2::new(x, 200.0 - lift * k as f32 / 5.0),
                Vec2::new(x, 1100.0),
            ]);
        }
        let interior = if open > 0.0 {
            "#2A1E12"
        } else {
            palette::YELLOW_DEEP
        };
        let mut shock = Path::new();
        if th > JOLT {
            // shock lines around his head
            let head =
                cam.to_screen(self.friend.posed(t).pos(Bone::Head) + Vec3::new(0.0, 0.12, 0.0));
            for i in 0..7 {
                let a = -2.6 + i as f32 * 0.32;
                let d = Vec2::from_angle(a);
                shock.polyline(&[head + d * 190.0, head + d * (250.0 + hash2(5, i) * 40.0)]);
            }
        }
        let held = self.hero.held(t);
        let puff = crate::render::Placed::new(
            crate::props::cream_puff(),
            self.puff.affine(held, 0.9),
            held,
        );
        fframes::svgr!(
            <g>
                {bg(palette::YELLOW)}
                <path d={arch.build()} fill={interior} stroke={palette::INK} stroke-width="8" />
                <path d={leaf.build()} fill="#C99A2E" stroke={palette::INK} stroke-width="8" stroke-linejoin="round" />
                {fx::stroke(door.build(), "#8A6418", 6.0)}
                {crate::render::draw_with(&[(&self.hero, Paint::two_tone(palette::YELLOW)), (&self.friend, Paint::two_tone(palette::YELLOW))], &[puff], t, &cam)}
                {fx::stroke(shock.build(), palette::INK, 8.0)}
            </g>
        )
    }
}

// ---------------------------------------------------------------------------------------------

/// The hero's eye, very close, a slow push in and a blink.
pub struct EyeClose {
    hero: Actor,
}

impl EyeClose {
    pub fn new(len: f32) -> Self {
        let sk = member(Who::Neogoose).skeleton();
        let hip = sk.proportions.hip_height;
        let hero = Actor::new(
            Who::Neogoose,
            motion(move |t| {
                let body = Body {
                    root: Vec3::new(0.0, hip, 0.0),
                    head: Vec3::new(-8.0 + t * 3.0, 2.0, 0.0),
                    ..Default::default()
                };
                build(&sk, body, Arms::default(), None)
            }),
            // watching something off to the side, a slow blink, then turning the eye on us
            acting(
                Acting::new(Look {
                    pupils: 0.8,
                    lids: 0.25,
                    gaze: Vec2::new(-0.6, 0.1),
                    ..Default::default()
                })
                .then(0.4, |l| Look {
                    gaze: Vec2::new(-0.3, 0.0),
                    lids: 0.4,
                    ..l
                })
                .blink_at(0.74)
                .then(0.95, |l| Look {
                    gaze: Vec2::new(0.25, 0.05),
                    ..expr::determined(l)
                })
                .then(1.25, |l| Look { pupils: 0.65, ..l }),
            ),
            len,
            2,
            Vec3::new(-2.0, 0.0, 0.5),
        );
        EyeClose { hero }
    }
}

impl Shot for EyeClose {
    fn render<'a>(&'a self, t: f32, len: f32, _f: &Frame, _c: &FFramesContext<'a, '_>) -> Svgr<'a> {
        let posed = self.hero.posed(t);
        let eye = posed.attach(Bone::Head, Vec3::new(0.035, 0.1, 0.085));
        // an extreme close-up: the eye fills the frame, a slow push
        let dist = 0.16 - 0.025 * (t / len);
        let mut cam = Camera::orbit(eye, dist, 38.0, 2.0, 30.0, W, H);
        cam.shift = Vec2::new(-260.0, 40.0);
        fframes::svgr!(
            <g>
                {bg(palette::YELLOW)}
                {self.hero.draw(t, &cam, &Paint::two_tone(palette::YELLOW))}
            </g>
        )
    }
}

// ---------------------------------------------------------------------------------------------

/// The cast in two-tone with their long cast shadows on the ground.
fn with_shadows<'a>(actors: &[(&Actor, Paint)], t: f32, cam: &Camera) -> Svgr<'a> {
    let shadow = cam.shadow(SUN);
    let ink = Paint::silhouette(palette::INK);
    let shadows: Vec<(&Actor, Paint)> = actors.iter().map(|(a, _)| (*a, ink)).collect();
    fframes::svgr!(<g>{stage::actors_painted(&shadows, t, &shadow)}{stage::actors_painted(actors, t, cam)}</g>)
}

/// A walk clip that stays in place (the camera travels with the walkers), with the arms
/// overridden: one hand to the mouth (eating), the other holding a bag.
fn walker(
    lib: &Library,
    who: Who,
    style: &str,
    from: f32,
    at: Vec3,
    eat: bool,
) -> crate::motion::MotionFn {
    let sk = member(who).skeleton();
    let base = in_place(
        lib,
        style,
        &sk,
        Play {
            from,
            speed: 0.85,
            at,
            ..Default::default()
        },
    );
    if !eat {
        return base;
    }
    Arc::new(move |t| {
        let mut pose = base(t);
        // eating: the head comes down to the puff
        let chest = pose.get(fframes_anime::rig::Bone::Chest);
        pose.rot[fframes_anime::rig::Bone::Head.index()] =
            chest * fframes_anime::glam::Quat::from_rotation_x(0.12);
        pose.rot[fframes_anime::rig::Bone::Neck.index()] = chest;
        let chew = (t * 9.0).sin() * 0.008;
        crate::procedural::reach_local(
            &sk,
            &mut pose,
            Side::Right,
            Vec3::new(-0.06, 0.17 + chew, 0.22),
            0.35,
        );
        crate::procedural::reach_local(
            &sk,
            &mut pose,
            Side::Left,
            Vec3::new(0.2, -0.28, 0.06),
            0.25,
        );
        pose
    })
}

/// The hero (eating a cream puff, a bag in the other hand) and Kovanikov walking at the camera.
pub struct WalkTwo {
    hero: Actor,
    friend: Actor,
}

impl WalkTwo {
    pub fn new(lib: &Library, len: f32) -> Self {
        let hero = Actor::new(
            Who::Neogoose,
            walker(
                lib,
                Who::Neogoose,
                "walk_proud",
                1.0,
                Vec3::new(0.3, 0.0, -0.1),
                true,
            ),
            acting(expr::chew(
                Acting::new(Look {
                    hand_r: HandShape::Fist,
                    hand_l: HandShape::Fist,
                    ..Default::default()
                })
                .then(0.0, |l| Look {
                    squint: 0.5,
                    lids: 0.3,
                    blush: 0.3,
                    ..l
                })
                .then(0.8, |l| Look {
                    gaze: Vec2::new(-0.7, 0.0),
                    ..expr::bored(l)
                })
                .blinks(31, 1.8),
                0.0,
                0.8,
                0.14,
            )),
            len,
            2,
            Vec3::ZERO,
        );
        let friend = Actor::new(
            Who::Chshersh,
            walker(
                lib,
                Who::Chshersh,
                "walk_not-confident",
                0.4,
                Vec3::new(-0.28, 0.0, -0.3),
                false,
            ),
            acting(expr::talk(
                Acting::new(Look::default())
                    .then(0.0, |l| Look {
                        gaze: Vec2::new(0.8, 0.0),
                        ..expr::worried(l)
                    })
                    .then(0.9, |l| Look {
                        gaze: Vec2::new(0.6, 0.1),
                        pupils: 0.55,
                        ..expr::surprised(l)
                    })
                    .blinks(32, 1.4),
                33,
                0.0,
                0.85,
                6.0,
            )),
            len,
            2,
            Vec3::ZERO,
        );
        WalkTwo { hero, friend }
    }
}

impl Shot for WalkTwo {
    fn render<'a>(&'a self, t: f32, len: f32, _f: &Frame, _c: &FFramesContext<'a, '_>) -> Svgr<'a> {
        let mut cam = Camera::new(
            Vec3::new(-0.2, 1.0, 3.6 - 0.15 * (t / len)),
            Vec3::new(-0.2, 0.96, 0.0),
            34.0,
            W,
            H,
        );
        cam.shift = Vec2::new(200.0, -30.0);
        let hand = self.hero.posed(t).hand_tip(Side::Left);
        let p = cam.project(hand + Vec3::new(0.0, -0.07, 0.03));
        let bag_w = 0.17 * p.scale;
        let bag_h = 0.19 * p.scale;
        let posed = self.hero.posed(t);
        let mouth = posed.attach(fframes_anime::rig::Bone::Head, Vec3::new(0.0, 0.05, 0.12));
        let puff = cam.project(mouth.lerp(posed.hand_tip(Side::Right), 0.35));
        let pr = 0.045 * puff.scale;
        let paint = Paint::two_tone(palette::YELLOW);
        fframes::svgr!(
            <g>
                {bg(palette::YELLOW)}
                {with_shadows(&[(&self.friend, paint), (&self.hero, paint)], t, &cam)}
                <rect x={p.p.x - bag_w * 0.5} y={p.p.y - bag_h * 0.1} width={bag_w} height={bag_h} rx={bag_w * 0.06} fill={palette::ORANGE} stroke={palette::INK} stroke-width="2.5" />
                <path d={format!("M{} {} l{} {}", p.p.x - bag_w * 0.5, p.p.y + bag_h * 0.12, bag_w, 0.0)} fill="none" stroke={palette::INK} stroke-width="2" opacity="0.6" />
                <ellipse cx={puff.p.x} cy={puff.p.y} rx={pr * 1.15} ry={pr} fill={palette::PUFF} stroke={palette::INK} stroke-width="2.5" />
            </g>
        )
    }
}

// ---------------------------------------------------------------------------------------------

/// Shreyas laughing, close, in line art: the whole head bounces and the white hair with it.
pub struct Laugh {
    friend: Actor,
}

impl Laugh {
    pub fn new(len: f32) -> Self {
        let sk = member(Who::Shreyas).skeleton();
        let hip = sk.proportions.hip_height;
        let friend = Actor::new(
            Who::Shreyas,
            motion(move |t| {
                // a laugh in beats: the shoulders jump on each "ha" and drop back over two
                // drawings (on twos, not a buzz)
                let since = (t * 5.0).fract() / 5.0;
                let bob = if since < 2.0 / 24.0 {
                    1.0
                } else if since < 4.0 / 24.0 {
                    0.45
                } else {
                    0.0
                };
                let body = Body {
                    root: Vec3::new(0.0, hip - 0.01 * bob, 0.0),
                    yaw: -15.0,
                    chest: Vec3::new(0.0, -4.0 - 3.0 * bob, 0.0),
                    head: Vec3::new(10.0, -6.0 - 5.0 * bob, 6.0),
                    ..Default::default()
                };
                build(
                    &sk,
                    body,
                    Arms {
                        left: Some(Vec3::new(0.06, -0.15, 0.3)),
                        ..Default::default()
                    },
                    None,
                )
            }),
            acting(expr::ha(
                Acting::new(Look {
                    hand_l: HandShape::Relaxed,
                    ..Default::default()
                })
                .then(0.0, expr::laugh)
                .then(0.6, |l| Look {
                    tears: 0.7,
                    blush: 0.7,
                    ..l
                })
                .then(1.15, |l| Look {
                    eyes: 0.5,
                    squint: 0.8,
                    happy: false,
                    tears: 0.7,
                    ..expr::happy(l)
                }),
                0.0,
                1.15,
                5.0,
            )),
            len,
            2,
            Vec3::ZERO,
        );
        Laugh { friend }
    }
}

impl Shot for Laugh {
    fn render<'a>(&'a self, t: f32, len: f32, _f: &Frame, _c: &FFramesContext<'a, '_>) -> Svgr<'a> {
        let th = twos(t);
        let head = self.friend.posed(0.0).pos(Bone::Head) + Vec3::new(0.0, 0.1, 0.0);
        let _ = len;
        let mut cam = Camera::orbit(head, 0.55, -20.0, -4.0, 30.0, W, H);
        cam.roll = -8.0;
        cam.shift = Vec2::new(160.0, 60.0);
        // "ha" marks popping around him
        let has: Vec<Svgr<'a>> = (0..5)
            .map(|i| {
                let born = i as f32 * 0.25;
                let age = th - born;
                let pop = ease_out(age / 0.12);
                let x = 260.0 + hash2(91, i) * 520.0;
                let y = 200.0 + hash2(92, i) * 600.0;
                let size = 110.0 * pop;
                let op = if age > 0.0 { 1.0 } else { 0.0 };
                fframes::svgr!(<text x={x} y={y} font-family="Dela Gothic One" font-size={size} fill={palette::INK} opacity={op} transform={format!("rotate({} {x} {y})", -15.0 + hash2(93, i) * 30.0)}>"ハ"</text>)
            })
            .collect();
        fframes::svgr!(
            <g>
                {bg(palette::YELLOW)}
                {has}
                {self.friend.draw(t, &cam, &Paint::line_art(palette::YELLOW))}
            </g>
        )
    }
}

// ---------------------------------------------------------------------------------------------

/// All six walk at the camera in black silhouettes, long shadows stretching left.
pub struct WalkGroup {
    actors: Vec<Actor>,
}

impl WalkGroup {
    pub fn new(lib: &Library, len: f32) -> Self {
        let styles = [
            "walk_proud",
            "walk_normal",
            "walk_happy",
            "walk_masculinity",
            "walk_chimpira",
            "walk_active",
        ];
        // shoulder to shoulder, the hero in the middle, a little in front
        let order = [
            Who::Chshersh,
            Who::Dillon,
            Who::Neogoose,
            Who::Theo,
            Who::Shreyas,
            Who::Teej,
        ];
        let spots = [
            (-1.05, -0.25),
            (-0.64, -0.45),
            (-0.24, -0.05),
            (0.15, -0.4),
            (0.55, -0.2),
            (0.95, -0.5),
        ];
        let actors = order
            .iter()
            .enumerate()
            .map(|(i, who)| {
                let (x, z) = spots[i];
                let eat = *who == Who::Neogoose;
                // everyone in their own mood: a joke told, a laugh, the hero eating, cool Dillon
                let fists = Look {
                    hand_l: HandShape::Fist,
                    hand_r: HandShape::Fist,
                    ..Default::default()
                };
                let seed = 40 + i as u32;
                let face = match who {
                    Who::Teej => expr::talk(
                        Acting::new(fists)
                            .then(0.0, |l| Look {
                                gaze: Vec2::new(0.6, 0.1),
                                ..expr::excited(l)
                            })
                            .then(1.0, expr::laugh),
                        seed,
                        0.0,
                        1.0,
                        6.5,
                    ),
                    Who::Shreyas => {
                        expr::ha(Acting::new(fists).then(0.0, expr::laugh), 0.0, 1.6, 4.5)
                    }
                    Who::Neogoose => expr::chew(
                        Acting::new(fists).then(0.0, |l| Look {
                            squint: 0.5,
                            blush: 0.4,
                            ..expr::happy(l)
                        }),
                        0.0,
                        1.6,
                        0.15,
                    ),
                    Who::Dillon => Acting::new(fists)
                        .then(0.0, |l| Look {
                            gaze: Vec2::new(-0.3, 0.0),
                            ..expr::smug(l)
                        })
                        .then(0.9, |l| Look {
                            gaze: Vec2::new(0.5, 0.0),
                            ..l
                        }),
                    Who::Theo => Acting::new(fists)
                        .then(0.0, |l| Look {
                            gaze: Vec2::new(0.7, 0.0),
                            ..expr::happy(l)
                        })
                        .then(0.7, |l| Look {
                            mouth: Mouth::Grin,
                            ..expr::laugh(l)
                        }),
                    _ => Acting::new(fists)
                        .then(0.0, expr::happy)
                        .then(0.6, |l| Look {
                            gaze: Vec2::new(0.6, 0.0),
                            mouth: Mouth::Open(0.4),
                            ..expr::happy(l)
                        }),
                }
                .blinks(seed, 2.2)
                .darts(seed, 0.2, 0.7);
                Actor::new(
                    *who,
                    walker(
                        lib,
                        *who,
                        styles[i % styles.len()],
                        0.3 * i as f32,
                        Vec3::new(x, 0.0, z),
                        eat,
                    ),
                    acting(face),
                    len,
                    2,
                    Vec3::ZERO,
                )
            })
            .collect();
        WalkGroup { actors }
    }
}

impl Shot for WalkGroup {
    fn render<'a>(&'a self, t: f32, len: f32, _f: &Frame, _c: &FFramesContext<'a, '_>) -> Svgr<'a> {
        let mut cam = Camera::new(
            Vec3::new(0.0, 1.05, 3.9 - 0.15 * (t / len)),
            Vec3::new(0.0, 0.98, 0.0),
            34.0,
            W,
            H,
        );
        cam.shift = Vec2::new(-330.0, -10.0);
        let paint = Paint::two_tone(palette::YELLOW);
        let list: Vec<(&Actor, Paint)> = self.actors.iter().map(|a| (a, paint)).collect();
        fframes::svgr!(
            <g>
                {bg(palette::YELLOW)}
                {with_shadows(&list, t, &cam)}
            </g>
        )
    }
}

#[allow(dead_code)]
fn bands<'a>(t: f32) -> Svgr<'a> {
    name_bands(t, palette::PUFF, 4)
}
