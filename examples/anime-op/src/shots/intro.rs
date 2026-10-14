//! The hero's introduction (11-24 s): the catch, the glitch, eyes opening over the name bands,
//! wind, the barbell, push-ups, the cream puff pile and the dive.

use std::sync::Arc;

use fframes::{FFramesContext, Frame, Svgr};
use fframes_anime::camera::Camera;
use fframes_anime::glam::{Quat, Vec2, Vec3};
use fframes_anime::rig::{Bone, two_bone_ik};
use fframes_anime::shape::{self, Path};
use fframes_anime::toon::{HandShape, Look, Mouth};

use crate::actor::{Actor, LookFn, acting};
use crate::cast::{Who, member};
use crate::expr::{self, Acting};
use crate::fx::{self, bg, ease_in, ease_out, hash2, smooth, spring, twos};
use crate::palette::{self, Paint};
use crate::physics2d::{BallSetup, BallState, bake_ball};
use crate::procedural::{Arms, Body, beat, build, motion};
use crate::shots::title::BLACKLETTER;
use crate::{H, Shot, W};

const FPS: f32 = 24.0;

/// "Neogoose" rows scrolling in alternate directions behind the hero.
/// The name in giant blackletter scrolling across the frame, laid over the drawing like the
/// reference's "MASHLE" band (translucent, so the character shows through).
pub fn name_bands<'a>(_t: f32, color: &'static str, _rows: usize) -> Svgr<'a> {
    // it keeps scrolling across the cuts of the section, like one band laid over all of them
    let t = crate::song_time() - 14.0;
    let band = fx::text_band(
        "Neogoose",
        760.0,
        620.0,
        2700.0,
        -t * 420.0 - 300.0,
        color,
        BLACKLETTER,
    );
    fframes::svgr!(<g font-weight="700" opacity="0.85" style="mix-blend-mode:multiply">{band}</g>)
}

fn standing(hip: f32) -> Body {
    Body {
        root: Vec3::new(0.0, hip, 0.0),
        ..Default::default()
    }
}

// ---------------------------------------------------------------------------------------------

/// A twinkle falls from the sky, the hero's hand reaches up and catches a cream puff.
pub struct Catch {
    hero: Actor,
    puff: crate::physics3d::Baked3,
    /// Between the wrists of the cupped hands.
    bowl: Vec3,
}

const CATCH_REACH: f32 = 0.85;
const CATCH_HANDS: f32 = 1.25;
/// The cream puff's size in the catch.
const CATCH_PUFF: f32 = 1.15;

/// The hero's motion in the catch; `landed` is when the puff hit the hands (they give with it).
fn catch_motion(landed: Option<f32>) -> crate::motion::MotionFn {
    let sk = member(Who::Neogoose).skeleton();
    let hip = sk.proportions.hip_height;
    motion(move |t| {
        if t < CATCH_HANDS {
            let k = ease_out((t - CATCH_REACH) / 0.25);
            return build(
                &sk,
                standing(hip),
                Arms {
                    right: Some(Vec3::new(-0.12, 0.1 + 0.55 * k, 0.25 + 0.1 * k)),
                    right_palm: Some(Vec3::new(0.0, 0.2, 1.0)),
                    ..Default::default()
                },
                None,
            );
        }
        // the weight of the puff: the hands give a few centimetres and come back up
        let give = landed.map_or(0.0, |l| {
            let u = t - l;
            if u < 0.0 {
                0.0
            } else {
                0.035 * (u * 9.0).min(1.0) * (-u * 6.0).exp() * (1.0 + u * 4.0)
            }
        });
        // cupped hands, palms up, little fingers together: a bowl
        let body = Body {
            root: Vec3::new(0.0, hip, 0.0),
            chest: Vec3::new(0.0, 12.0, 0.0),
            head: Vec3::new(0.0, 25.0, 0.0),
            ..Default::default()
        };
        build(
            &sk,
            body,
            Arms {
                left: Some(Vec3::new(0.05, -0.06 - give, 0.25)),
                right: Some(Vec3::new(-0.05, -0.06 - give, 0.25)),
                elbow_out: 0.6,
                left_palm: Some(Vec3::new(-0.38, 1.0, -0.3)),
                right_palm: Some(Vec3::new(0.38, 1.0, -0.3)),
                left_fingers: Some(Vec3::new(0.05, 0.3, 1.0)),
                right_fingers: Some(Vec3::new(-0.05, 0.3, 1.0)),
            },
            None,
        )
    })
}

impl Catch {
    pub fn new(len: f32) -> Self {
        use fframes_anime::rig::Side;
        let face = Acting::new(Look {
            hand_l: HandShape::Open,
            hand_r: HandShape::Open,
            ..Default::default()
        })
        // something falls: brows up, eyes up
        .then(0.0, expr::surprised)
        .then(0.0, expr::gaze(0.1, 0.9))
        .then(0.0, expr::mouth(Mouth::Line))
        // a cream puff: the eyes light up
        .then(0.42, |l| Look {
            gaze: Vec2::new(0.0, 0.8),
            ..expr::excited(l)
        })
        // reach for it
        .then(CATCH_REACH - 0.08, |l| Look {
            gaze: Vec2::new(-0.2, 0.9),
            hand_r: HandShape::Open,
            ..expr::determined(l)
        })
        .then(CATCH_REACH + 0.2, |l| Look {
            gaze: Vec2::new(0.0, 0.4),
            ..expr::excited(l)
        })
        .then(CATCH_HANDS, |l| Look { face: false, ..l })
        .blinks(11, 2.5)
        .blink_at(0.3);
        let looks = acting(face);
        // Drop the puff into the hands twice: once to find when it lands, then again with the
        // hands giving under its weight.
        let fig = crate::render::figure(Who::Neogoose);
        let sk = member(Who::Neogoose).skeleton();
        let drop = |m: &crate::motion::MotionFn| {
            let bowl = {
                let p = sk.solve(&m(CATCH_HANDS));
                (p.pos(Bone::HandL) + p.pos(Bone::HandR)) * 0.5
            };
            let toss = crate::physics3d::Toss {
                hull: crate::props::puff_hull(),
                scale: CATCH_PUFF,
                pos: bowl + Vec3::new(0.01, 0.42, 0.1),
                rot: Quat::from_rotation_x(0.3) * Quat::from_rotation_z(-0.2),
                vel: Vec3::new(-0.02, -1.8, -0.05),
                spin: Vec3::new(-1.5, 0.8, 0.9),
                restitution: 0.25,
                friction: 1.0,
                release: CATCH_HANDS,
            };
            crate::physics3d::bake(
                &toss,
                &fig,
                &[
                    (Side::Left, HandShape::Open),
                    (Side::Right, HandShape::Open),
                ],
                &|t| sk.solve(&m(t)),
                &[],
                CATCH_HANDS,
                len,
            )
        };
        let first = drop(&catch_motion(None));
        let m = catch_motion(first.landed);
        let puff = drop(&m);
        let bowl = {
            let p = sk.solve(&m(CATCH_HANDS));
            (p.pos(Bone::HandL) + p.pos(Bone::HandR)) * 0.5
        };
        let hero = Actor::new(Who::Neogoose, m, looks, len, 2, Vec3::ZERO);
        Catch { hero, puff, bowl }
    }
}

impl Shot for Catch {
    fn render<'a>(
        &'a self,
        t: f32,
        _len: f32,
        _f: &Frame,
        _c: &FFramesContext<'a, '_>,
    ) -> Svgr<'a> {
        let th = twos(t);
        if th < CATCH_REACH {
            // the twinkle grows as it falls towards the camera
            let k = th / CATCH_REACH;
            let c = Vec2::new(960.0 + 40.0 * k, 260.0 + 280.0 * k * k);
            let r = 10.0 + 70.0 * k * k;
            let rot = th * 3.0;
            return fframes::svgr!(
                <g>
                    {bg(palette::YELLOW)}
                    {fx::path(fx::sparkle(c, r, rot).build(), palette::INK)}
                    {fx::path(fx::sparkle(c, r * 0.5, rot + 0.78).build(), palette::INK)}
                </g>
            );
        }
        if th < CATCH_HANDS {
            // worm's eye view: the arm shoots up into the frame
            let cam = Camera::new(
                Vec3::new(0.25, 0.9, 1.3),
                Vec3::new(-0.15, 2.3, 0.0),
                40.0,
                W,
                H,
            );
            let k = (th - CATCH_REACH) / (CATCH_HANDS - CATCH_REACH);
            let c = Vec2::new(900.0, 120.0 + 260.0 * ease_in(k));
            return fframes::svgr!(
                <g>
                    {bg(palette::YELLOW)}
                    {fx::path(fx::sparkle(c, 90.0, th * 3.0).build(), palette::INK)}
                    {self.hero.draw(t, &cam, &Paint::silhouette(palette::INK))}
                </g>
            );
        }
        // close on the cupped hands, room above them for the puff to drop in
        let cam = Camera::new(
            self.bowl + Vec3::new(0.03, 0.42, 0.62),
            self.bowl + Vec3::new(0.0, 0.07, 0.05),
            30.0,
            W,
            H,
        );
        let held = self.hero.held(t);
        let puff = crate::render::Placed::new(
            crate::props::cream_puff(),
            self.puff.affine(held, CATCH_PUFF),
            held,
        );
        fframes::svgr!(
            <g>
                {bg(palette::YELLOW)}
                {crate::render::draw_with(&[(&self.hero, Paint::line_art(palette::YELLOW))], &[puff], t, &cam)}
            </g>
        )
    }
}

// ---------------------------------------------------------------------------------------------

/// The cream puff in line art spinning through the dark with split colours and two glowing
/// rings opening like eyes.
pub struct Glitch;

impl Shot for Glitch {
    fn render<'a>(
        &'a self,
        t: f32,
        _len: f32,
        _f: &Frame,
        _c: &FFramesContext<'a, '_>,
    ) -> Svgr<'a> {
        let th = twos(t);
        let seed = (th * 24.0) as u32;
        let jit = |k: u32| {
            Vec2::new(
                (hash2(seed, k) - 0.5) * 24.0,
                (hash2(seed, k + 9) - 0.5) * 10.0,
            )
        };
        let c = Vec2::new(960.0 - th * 120.0, 540.0);
        let r = 330.0 + th * 60.0;
        let rot = th * 1.4;
        let lines = fx::focus_lines(c, 70, 520.0, seed, 3.0);
        let mut rings = Path::new();
        let open = ease_out((th - 0.25) / 0.35);
        if open > 0.0 {
            for x in [430.0, 1490.0] {
                for k in 0..3 {
                    rings.circle(Vec2::new(x, 520.0), (40.0 + k as f32 * 55.0) * open);
                }
            }
        }
        fframes::svgr!(
            <g>
                {bg("#141008")}
                {fx::path(lines.build(), "#2A2410")}
                {fx::stroke(rings.build(), palette::YELLOW_DEEP, 10.0)}
                {fx::puff_lines(c + jit(1), r, rot, palette::MAGENTA, 7.0)}
                {fx::puff_lines(c + jit(2), r, rot, palette::GREEN_LIGHT, 7.0)}
                {fx::puff_lines(c, r, rot, palette::YELLOW, 6.0)}
            </g>
        )
    }
}

// ---------------------------------------------------------------------------------------------

fn idle_head(
    sk: &fframes_anime::rig::Skeleton,
    yaw: impl Fn(f32) -> f32 + Send + Sync + 'static,
) -> crate::motion::MotionFn {
    let sk = sk.clone();
    let hip = sk.proportions.hip_height;
    motion(move |t| {
        let body = Body {
            root: Vec3::new(0.0, hip, 0.0),
            chest: Vec3::new(0.0, 0.0, 0.0),
            neck: Vec3::new(yaw(t) * 0.3, 0.0, 0.0),
            head: Vec3::new(yaw(t) * 0.7, 2.0 + (t * 3.0).sin() * 1.5, 0.0),
            ..Default::default()
        };
        build(&sk, body, Arms::default(), None)
    })
}

/// Extreme close-up: the eyes snap open under the ginger quiff, name bands behind.
pub struct Eyes {
    hero: Actor,
}

impl Eyes {
    pub fn new(len: f32) -> Self {
        let sk = member(Who::Neogoose).skeleton();
        let looks = acting(
            Acting::new(Look {
                eyes: 0.0,
                lids: 0.4,
                ..Default::default()
            })
            .then(0.28, |l| Look { eyes: 0.35, ..l })
            // snap open: wide awake, pupils shrinking to a point of focus
            .then(0.36, |l| Look {
                eyes: 1.0,
                lids: 0.0,
                brows: 0.5,
                pupils: 0.55,
                ..l
            })
            .then(0.52, |l| Look {
                pupils: 0.8,
                gaze: Vec2::new(0.45, 0.05),
                ..l
            })
            .then(0.66, |l| Look {
                gaze: Vec2::new(-0.15, 0.0),
                ..l
            })
            .then(0.76, |l| Look {
                gaze: Vec2::ZERO,
                brows: 0.0,
                ..expr::determined(l)
            }),
        );
        let hero = Actor::new(
            Who::Neogoose,
            idle_head(&sk, |_| 0.0),
            looks,
            len,
            2,
            Vec3::new(-1.5, 0.0, 0.0),
        );
        Eyes { hero }
    }
}

impl Shot for Eyes {
    fn render<'a>(
        &'a self,
        t: f32,
        _len: f32,
        _f: &Frame,
        _c: &FFramesContext<'a, '_>,
    ) -> Svgr<'a> {
        // an extreme close-up: the eyes fill the frame
        let posed = self.hero.posed(t);
        let eye = posed.attach(Bone::Head, Vec3::new(0.0, 0.112, 0.06));
        let dolly = 0.16 - 0.02 * smooth(t / 0.9);
        let mut cam = Camera::new(eye + Vec3::new(0.0, 0.0, dolly), eye, 30.0, W, H);
        // the fringe of the quiff over the top of the frame
        cam.shift = Vec2::new(0.0, -120.0);
        fframes::svgr!(
            <g>
                {bg(palette::YELLOW)}
                {name_bands(t, palette::PUFF, 4)}
                {self.hero.draw(t, &cam, &Paint::two_tone(palette::YELLOW))}
            </g>
        )
    }
}

// ---------------------------------------------------------------------------------------------

/// Waist-up with a cream puff, a leopard-spotted puff halo behind.
pub struct Pattern {
    hero: Actor,
    /// The puff resting in the right palm.
    puff: crate::physics3d::Baked3,
}

impl Pattern {
    pub fn new(len: f32) -> Self {
        let sk = member(Who::Neogoose).skeleton();
        let hip = sk.proportions.hip_height;
        let m = motion(move |t| {
            let turn = -35.0 * (1.0 - spring(t - 0.3, 1.2, 6.0));
            let body = Body {
                root: Vec3::new(0.0, hip, 0.0),
                chest: Vec3::new(turn * 0.3, 0.0, 0.0),
                head: Vec3::new(turn * 0.7, 3.0, 0.0),
                ..Default::default()
            };
            build(
                &sk,
                body,
                // the puff presented on an upturned palm by the shoulder, like a waiter's
                // tray: fingers out to the side, the palm flat under it
                Arms {
                    right: Some(Vec3::new(-0.19, 0.1, 0.27)),
                    right_palm: Some(Vec3::new(0.05, 1.0, -0.12)),
                    right_fingers: Some(Vec3::new(-0.55, 0.08, 0.85)),
                    elbow_out: 0.5,
                    ..Default::default()
                },
                None,
            )
        });
        let looks = acting(expr::chew(
            Acting::new(Look {
                hand_r: HandShape::Relaxed,
                ..Default::default()
            })
            .then(0.0, |l| Look {
                squint: 0.5,
                blush: 0.3,
                gaze: Vec2::new(0.2, -0.3),
                ..l
            })
            .then(0.6, expr::bliss)
            .then(1.25, |l| Look {
                gaze: Vec2::new(0.5, -0.2),
                sparkle: 0.8,
                ..expr::happy(l)
            })
            .blinks(4, 2.0),
            0.05,
            1.2,
            0.12,
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
            0.7,
            len,
        );
        Pattern {
            hero: Actor::new(Who::Neogoose, m, looks, len, 2, Vec3::ZERO),
            puff,
        }
    }
}

impl Shot for Pattern {
    fn render<'a>(
        &'a self,
        t: f32,
        _len: f32,
        _f: &Frame,
        _c: &FFramesContext<'a, '_>,
    ) -> Svgr<'a> {
        let th = twos(t);
        let cam = Camera::new(
            Vec3::new(0.12, 1.56, 1.3),
            Vec3::new(0.0, 1.53, 0.0),
            30.0,
            W,
            H,
        );
        // halo puff with spots
        let c = Vec2::new(960.0, 470.0);
        let r = 700.0 + th * 40.0;
        let mut spots = Path::new();
        for i in 0..26 {
            let a = hash2(31, i) * std::f32::consts::TAU + th * 0.2;
            let d = r * (0.25 + hash2(32, i) * 0.7);
            let p = c + Vec2::from_angle(a) * d;
            let rr = 26.0 + hash2(33, i) * 34.0;
            let pts: Vec<Vec2> = (0..9)
                .map(|k| {
                    let ang = k as f32 / 9.0 * std::f32::consts::TAU;
                    p + Vec2::from_angle(ang) * rr * (0.75 + 0.4 * hash2(i, k))
                })
                .collect();
            spots.smooth_closed(&pts, 1.0);
        }
        let mut dots = Path::new();
        for i in 0..14 {
            let p = Vec2::new(hash2(41, i) * 1920.0, hash2(42, i) * 1080.0);
            dots.circle(p, 22.0 + hash2(43, i) * 18.0);
        }
        let held = self.hero.held(t);
        let puff = crate::render::Placed::new(
            crate::props::cream_puff(),
            self.puff.affine(held, 1.0),
            held,
        );
        fframes::svgr!(
            <g>
                {bg(palette::YELLOW)}
                <circle cx={c.x} cy={c.y} r={r} fill={palette::PUFF} />
                {fx::path(spots.build(), palette::INK)}
                {fx::path(dots.build(), palette::GREEN)}
                {crate::render::draw_with(&[(&self.hero, Paint::two_tone(palette::YELLOW))], &[puff], t, &cam)}
                {name_bands(t, palette::PUFF, 4)}
            </g>
        )
    }
}

// ---------------------------------------------------------------------------------------------

/// Two panels: the face from the front, the back of the head turning.
pub struct Split {
    hero: Actor,
}

impl Split {
    pub fn new(len: f32) -> Self {
        let sk = member(Who::Neogoose).skeleton();
        let hero = Actor::new(
            Who::Neogoose,
            idle_head(&sk, |t| -20.0 + t * 40.0),
            acting(
                Acting::new(Look::default())
                    .blink_at(0.06)
                    .then(0.24, |l| Look {
                        gaze: Vec2::new(0.85, 0.0),
                        ..expr::smug(l)
                    }),
            ),
            len,
            2,
            Vec3::new(1.5, 0.0, 0.0),
        );
        Split { hero }
    }
}

impl Shot for Split {
    fn render<'a>(
        &'a self,
        t: f32,
        _len: f32,
        _f: &Frame,
        _c: &FFramesContext<'a, '_>,
    ) -> Svgr<'a> {
        let head = self.hero.posed(t).pos(Bone::Head) + Vec3::new(0.0, 0.1, 0.0);
        let mut front = Camera::new(head + Vec3::new(0.05, 0.0, 0.55), head, 30.0, W, H);
        front.shift = Vec2::new(-480.0, 0.0);
        let mut back = Camera::new(head + Vec3::new(-0.35, 0.05, -0.45), head, 30.0, W, H);
        back.shift = Vec2::new(480.0, 0.0);
        let paint = Paint::two_tone(palette::YELLOW);
        fframes::svgr!(
            <g>
                {bg(palette::YELLOW)}
                <clipPath id="splitl"><rect x="0" y="0" width="955" height="1080" /></clipPath>
                <clipPath id="splitr"><rect x="965" y="0" width="955" height="1080" /></clipPath>
                <g clip-path="url(#splitl)">{self.hero.draw(t, &front, &paint)}</g>
                <g clip-path="url(#splitr)">{self.hero.draw(t, &back, &paint)}</g>
                <rect x="952" y="0" width="16" height="1080" fill={palette::INK} />
                {name_bands(t, palette::PUFF, 4)}
            </g>
        )
    }
}

// ---------------------------------------------------------------------------------------------

/// Hair in a gale.
pub struct Wind {
    hero: Actor,
}

impl Wind {
    pub fn new(len: f32) -> Self {
        let sk = member(Who::Neogoose).skeleton();
        let wind = Arc::new(|t: f32| {
            let gust = 0.6 + 0.4 * (t * 9.0).sin() * (t * 3.7).cos();
            Vec3::new(-11.0 * gust, 2.0 * (t * 13.0).sin(), 3.0 * (t * 7.0).cos())
        });
        let looks = acting(
            Acting::new(Look {
                eyes: 0.6,
                squint: 0.7,
                brow_angle: 0.5,
                mouth: Mouth::Teeth(0.3),
                ..Default::default()
            })
            .then(0.5, |l| Look {
                eyes: 0.85,
                mouth: Mouth::Line,
                ..expr::determined(l)
            })
            .blinks(13, 0.45)
            .darts(5, 0.15, 0.3),
        );
        let hero = Actor::with_wind(
            Who::Neogoose,
            idle_head(&sk, |t| 25.0 - t * 6.0),
            looks,
            len,
            2,
            wind,
        );
        Wind { hero }
    }
}

impl Shot for Wind {
    fn render<'a>(
        &'a self,
        t: f32,
        _len: f32,
        _f: &Frame,
        _c: &FFramesContext<'a, '_>,
    ) -> Svgr<'a> {
        let th = twos(t);
        let head = self.hero.posed(t).pos(Bone::Head) + Vec3::new(0.0, 0.12, 0.0);
        let cam = Camera::orbit(head, 0.5, 32.0, 6.0, 30.0, W, H);
        let lines = fx::speed_lines(3.0, 26, (th * 24.0) as u32, th * 3000.0, 6.0);
        fframes::svgr!(
            <g>
                {bg(palette::YELLOW)}
                {fx::path(lines.build(), "#B89A00")}
                {self.hero.draw(t, &cam, &Paint::two_tone(palette::YELLOW))}
            </g>
        )
    }
}

// ---------------------------------------------------------------------------------------------

/// How high the barbell is (0 = at the chest, 1 = overhead) at a song time: a press on every
/// other beat.
fn press(song_t: f32) -> f32 {
    let b = beat(song_t);
    // locked out on every other beat: the press starts 0.44 beats before it
    let phase = (b * 0.5 + 0.22).fract();
    // press fast (first 25% of two beats), hold, lower slowly
    if phase < 0.22 {
        ease_out(phase / 0.22)
    } else if phase < 0.55 {
        1.0
    } else {
        1.0 - smooth((phase - 0.55) / 0.4)
    }
}

/// The hero presses a giant barbell; every lockout hits like an impact frame.
pub struct Barbell {
    hero: Actor,
    start: f32,
    /// The hollows of the fists, where the bar runs (hand frame).
    grips: [Vec3; 2],
}

/// Where the hands hold the bar, either side of its middle.
const GRIP: f32 = 0.36;

/// The hand on the bar: fingers up and back over it at the rack, straight up at the lockout,
/// the palm under it.
fn bar_hand(p: f32) -> (Vec3, Vec3) {
    let along = Vec3::new(0.0, 0.62, -0.78)
        .lerp(Vec3::new(0.0, 1.0, -0.22), p)
        .normalize();
    let palm = Vec3::new(0.0, 0.78, 0.62)
        .lerp(Vec3::new(0.0, 0.22, 1.0), p)
        .normalize();
    (along, palm)
}

impl Barbell {
    pub fn new(start: f32, len: f32) -> Self {
        use fframes_anime::rig::Side;
        let sk = member(Who::Neogoose).skeleton();
        let hip = sk.proportions.hip_height;
        let fig = crate::render::figure(Who::Neogoose);
        let grips = [fig.grip(Side::Left), fig.grip(Side::Right)];
        let arm = sk.proportions.upper_arm + sk.proportions.forearm;
        let m = motion(move |t| {
            let p = press(start + t);
            let dip = (1.0 - p) * 0.06;
            let body = Body {
                root: Vec3::new(0.0, hip - dip - 0.03, 0.0),
                spine: Vec3::new(0.0, -3.0 * p, 0.0),
                head: Vec3::new(0.0, -10.0 * p, 0.0),
                ..Default::default()
            };
            let mut pose = build(
                &sk,
                body,
                Arms::default(),
                Some((Vec3::ZERO, 0.0, 0.2, 0.0)),
            );
            let solved = sk.solve(&pose);
            let shoulders = (solved.pos(Bone::UpperArmL) + solved.pos(Bone::UpperArmR)) * 0.5;
            // the bar: racked on the collarbones, pressed to arm's length over the head
            let bar = shoulders + Vec3::new(0.0, 0.02 + (arm * 0.9) * p, 0.16 - 0.17 * p);
            let (along, palm) = bar_hand(p);
            for (i, side) in [Side::Left, Side::Right].into_iter().enumerate() {
                let s = side.sign();
                let grip = bar + Vec3::X * GRIP * s;
                let rot = crate::procedural::hand_facing(side, along, palm);
                let wrist = grip - rot * grips[i];
                let shoulder = solved.pos(Bone::arm(side)[1]);
                let pole = shoulder + Vec3::new(0.55 * s, -0.45, 0.35 - 0.3 * p);
                two_bone_ik(&sk, &mut pose, Bone::arm(side)[1], wrist, pole, None);
                pose.rot[Bone::arm(side)[3].index()] = rot;
            }
            pose
        });
        let looks: LookFn = Arc::new(move |t| {
            let p = press(start + t);
            let hands = Look {
                hand_l: HandShape::Fist,
                hand_r: HandShape::Fist,
                ..Default::default()
            };
            if p > 0.92 {
                // locked out overhead: a roar of triumph
                Look {
                    sparkle: 0.6,
                    ..expr::shout(hands)
                }
            } else if p > 0.35 {
                let mut l = expr::strain(hands);
                l.eyes = 1.0 - 0.6 * p;
                l.mouth = Mouth::Teeth(p);
                l.vein = ((p - 0.5) * 3.0).clamp(0.0, 1.0);
                l
            } else {
                Look {
                    brow_angle: 0.4 + p,
                    ..expr::determined(hands)
                }
            }
        });
        Barbell {
            hero: Actor::new(Who::Neogoose, m, looks, len, 2, Vec3::ZERO),
            start,
            grips,
        }
    }

    /// The bar and plates for the drawing at `t`: through both fists, the ends whipping down
    /// with the press.
    fn barbell(&self, t: f32, flat: Option<&'static str>) -> Vec<crate::render::Placed<'static>> {
        use fframes_anime::glam::Affine3A;
        use fframes_anime::rig::Side;
        let th = self.hero.held(t);
        let posed = self.hero.posed(t);
        let grip = |side: Side, i: usize| {
            let hand = Bone::arm(side)[3];
            posed.pos(hand) + posed.rot(hand) * self.grips[i]
        };
        let l = grip(Side::Left, 0);
        let r = grip(Side::Right, 1);
        let mid = (l + r) * 0.5;
        let dir = (l - r).normalize_or(Vec3::X);
        // the plates lag the press: the bar bends with its acceleration
        let acc = press(self.start + th + 0.04) - 2.0 * press(self.start + th)
            + press(self.start + th - 0.04);
        let bend = (acc * 3.2).clamp(-0.16, 0.16);
        let mut out = Vec::new();
        let place = |prop, at| {
            let mut p = crate::render::Placed::new(prop, at, th);
            if let Some(c) = flat {
                p = p.flat(c);
            }
            p
        };
        let inner = 0.56;
        let end = 1.13;
        // the middle straight through the fists
        out.push(place(
            crate::props::bar(),
            Affine3A::from_scale_rotation_translation(
                Vec3::new(inner * 2.0, 1.0, 1.0),
                Quat::from_rotation_arc(Vec3::X, dir),
                mid - dir * inner,
            ),
        ));
        for s in [1.0_f32, -1.0] {
            let root = mid + dir * inner * s;
            let out_dir = Quat::from_axis_angle(dir.cross(Vec3::Y).normalize_or(Vec3::Z), bend * s)
                * (dir * s);
            out.push(place(
                crate::props::bar(),
                Affine3A::from_scale_rotation_translation(
                    Vec3::new(end - inner, 1.0, 1.0),
                    Quat::from_rotation_arc(Vec3::X, out_dir),
                    root,
                ),
            ));
            let along = |d: f32| root + out_dir * (d - inner);
            let turn = Quat::from_rotation_arc(Vec3::X, out_dir);
            out.push(place(
                crate::props::plate(),
                Affine3A::from_scale_rotation_translation(Vec3::ONE, turn, along(0.86)),
            ));
            out.push(place(
                crate::props::plate(),
                Affine3A::from_scale_rotation_translation(Vec3::splat(0.84), turn, along(0.945)),
            ));
        }
        out
    }
}

impl Shot for Barbell {
    fn render<'a>(
        &'a self,
        t: f32,
        _len: f32,
        _f: &Frame,
        _c: &FFramesContext<'a, '_>,
    ) -> Svgr<'a> {
        let th = twos(t);
        // a low-angle medium shot: the face reads, the plates tower at the frame edges
        let cam = Camera::orbit(Vec3::new(0.0, 1.62, 0.0), 2.5, 3.0, -9.0, 34.0, W, H);
        let posed = self.hero.posed(t);
        let mid = (posed.pos(Bone::HandL) + posed.pos(Bone::HandR)) * 0.5;
        // Each lockout: two frames of an inverted impact frame, then focus lines.
        let b = beat(self.start + th);
        let since = ((b * 0.5).fract()) * 2.0 * crate::procedural::PERIOD;
        let flash = since < 0.09 && press(self.start + th) > 0.9;
        let (back, paint) = if flash {
            (bg(palette::INK), Paint::silhouette(palette::YELLOW))
        } else {
            (bg(palette::YELLOW), Paint::line_art(palette::YELLOW))
        };
        let props = self.barbell(t, flash.then_some(palette::YELLOW));
        let mut impact = Vec::new();
        if !flash && since < 0.2 && press(self.start + th) > 0.9 {
            let c = cam.to_screen(mid);
            impact.push(fx::path(
                fx::focus_lines(c, 90, 420.0 + since * 900.0, (th * 24.0) as u32, 4.0).build(),
                palette::INK,
            ));
        }
        let bands = if flash {
            Svgr::empty()
        } else {
            name_bands(t, palette::PUFF, 4)
        };
        fframes::svgr!(
            <g>
                {back}
                {impact}
                {crate::render::draw_with(&[(&self.hero, paint)], &props, t, &cam)}
                {bands}
            </g>
        )
    }
}

// ---------------------------------------------------------------------------------------------

/// How far the plank is pushed up (0 = chest above the floor, 1 = arm locked out) at a song
/// time: one push-up a beat, lowered under control and driven up explosively so the lockout
/// lands on the beat.
fn pushup(song_t: f32) -> f32 {
    let phase = beat(song_t).rem_euclid(1.0);
    if phase < 0.3 {
        1.0
    } else if phase < 0.74 {
        1.0 - smooth((phase - 0.3) / 0.44)
    } else if phase < 0.86 {
        0.0
    } else {
        ease_in((phase - 0.86) / 0.14)
    }
}

/// The plank of a one-arm push-up: toes planted at `toes` (the middle between both feet), the
/// body a straight line from them rising `rise` degrees, legs spread for balance, the right
/// palm flat on the floor at `hand` and the left fist on the small of the back.
fn plank(
    sk: &fframes_anime::rig::Skeleton,
    toes: Vec3,
    rise: f32,
    hand: Option<Vec3>,
) -> fframes_anime::rig::Pose {
    use fframes_anime::rig::Side;
    let body = Body {
        yaw: 90.0,
        hips: Vec3::new(0.0, 90.0 - rise, 0.0),
        neck: Vec3::new(0.0, -38.0, 0.0),
        head: Vec3::new(0.0, -22.0, 0.0),
        ..Default::default()
    };
    let mut pose = body.pose(sk);
    let hips = pose.get(Bone::Hips);
    for side in [Side::Left, Side::Right] {
        // legs apart, feet flexed so the balls of the feet take the weight
        let spread = hips * Quat::from_rotation_z(-9_f32.to_radians() * side.sign());
        let [thigh, shin, foot, toe] = Bone::leg(side);
        pose.rot[thigh.index()] = spread;
        pose.rot[shin.index()] = spread;
        pose.rot[foot.index()] = spread * Quat::from_rotation_x(-55_f32.to_radians());
        pose.rot[toe.index()] = spread * Quat::from_rotation_x(-95_f32.to_radians());
    }
    // move the whole plank so the toes rest on their spot
    let solved = sk.solve(&pose);
    let feet = (solved.pos(Bone::ToesL) + solved.pos(Bone::ToesR)) * 0.5;
    pose.root += toes - feet;
    let solved = sk.solve(&pose);
    if let Some(hand) = hand {
        let shoulder = solved.pos(Bone::UpperArmR);
        // the elbow bends back along the body, close to the ribs
        let pole = shoulder + Vec3::new(-0.6, 0.2, 0.25);
        two_bone_ik(sk, &mut pose, Bone::UpperArmR, hand, pole, None);
        pose.rot[Bone::HandR.index()] = crate::procedural::hand_facing(
            Side::Right,
            Vec3::new(0.85, 0.0, -0.5).normalize(),
            Vec3::NEG_Y,
        );
    }
    // the other fist rests on the small of the back
    let hips_rot = pose.get(Bone::Hips);
    let small = solved.pos(Bone::Hips) + hips_rot * Vec3::new(0.0, 0.1, -0.13);
    let pole = small + hips_rot * Vec3::new(0.45, 0.35, -0.3);
    two_bone_ik(sk, &mut pose, Bone::UpperArmL, small, pole, None);
    pose
}

const PUSH_TOES: Vec3 = Vec3::new(-0.78, 0.0, 0.0);

/// One-arm push-ups between giant weight plates, seen from the side.
pub struct Pushups {
    hero: Actor,
    start: f32,
    /// Where the palm is planted.
    hand: Vec3,
}

impl Pushups {
    pub fn new(start: f32, len: f32) -> Self {
        let sk = member(Who::Neogoose).skeleton();
        // Find the plank angles: locked out, the shoulder is an arm's length above the palm;
        // down, the chest is a fist above the floor.
        let shoulder_at = |rise: f32| {
            sk.solve(&plank(&sk, PUSH_TOES, rise, None))
                .pos(Bone::UpperArmR)
        };
        let p = &sk.proportions;
        let arm = p.upper_arm + p.forearm;
        let solve = |height: f32| {
            let (mut lo, mut hi) = (0.0_f32, 45.0_f32);
            for _ in 0..30 {
                let mid = (lo + hi) * 0.5;
                if shoulder_at(mid).y < height {
                    lo = mid;
                } else {
                    hi = mid;
                }
            }
            (lo + hi) * 0.5
        };
        let palm_height = 0.035;
        let up = solve(palm_height + arm * 0.97);
        let down = solve(palm_height + arm * 0.42);
        let top = shoulder_at(up);
        let hand = Vec3::new(top.x + 0.03, palm_height, top.z - 0.02);
        let m = motion(move |t| {
            let rise = down + (up - down) * pushup(start + t);
            plank(&sk, PUSH_TOES, rise, Some(hand))
        });
        let looks: LookFn = Arc::new(move |t| {
            let k = pushup(start + t);
            let phase = beat(start + t).rem_euclid(1.0);
            let base = Look {
                hand_r: HandShape::Open,
                hand_l: HandShape::Fist,
                sweat: 0.7,
                ..expr::determined(Look::default())
            };
            if phase > 0.86 || phase < 0.1 {
                // driving up: teeth clenched, eyes squeezed
                Look {
                    mouth: Mouth::Teeth(0.8),
                    squint: 0.85,
                    brow_angle: 1.0,
                    vein: 0.8,
                    ..base
                }
            } else if k < 0.5 {
                // at the bottom: a breath in
                Look {
                    mouth: Mouth::O(0.35),
                    squint: 0.4,
                    ..base
                }
            } else {
                Look {
                    mouth: Mouth::Line,
                    squint: 0.5,
                    ..base
                }
            }
        });
        Pushups {
            // a gale along the body streams the robe back over the legs (and keeps the plank
            // readable instead of a heap of cloth on the floor)
            hero: Actor::on_floor(
                Who::Neogoose,
                m,
                looks,
                len,
                2,
                Vec3::new(-7.0, 2.5, 0.0),
                0.0,
            ),
            start,
            hand,
        }
    }
}

impl Shot for Pushups {
    fn render<'a>(
        &'a self,
        t: f32,
        _len: f32,
        _f: &Frame,
        _c: &FFramesContext<'a, '_>,
    ) -> Svgr<'a> {
        let th = twos(t);
        let cam = Camera::new(
            Vec3::new(0.0, 0.5, 2.85),
            Vec3::new(0.0, 0.4, 0.0),
            32.0,
            W,
            H,
        );
        let disc = |cx: f32| -> Svgr<'a> {
            fframes::svgr!(
                <g>
                    <ellipse cx={cx} cy="540" rx="150" ry="470" fill={palette::INK} />
                    <ellipse cx={cx + 60.0} cy="540" rx="150" ry="470" fill={palette::INK} />
                    <ellipse cx={cx + 60.0} cy="540" rx="100" ry="320" fill="none" stroke={palette::GREEN} stroke-width="14" />
                    <ellipse cx={cx + 60.0} cy="540" rx="38" ry="110" fill={palette::GREEN} />
                </g>
            )
        };
        // the floor where the 3D ground is: a line through the contact points
        let a = cam.to_screen(Vec3::new(-4.0, 0.0, self.hand.z));
        let b = cam.to_screen(Vec3::new(4.0, 0.0, self.hand.z));
        let mut floor = Path::new();
        floor.polyline(&[a, b]);
        // contact shadows under the palm and the toes, and the body's shadow between them
        let posed = self.hero.posed(t);
        let ground = |p: Vec3| cam.to_screen(Vec3::new(p.x, 0.0, p.z));
        let palm = ground(self.hand);
        let toes = ground((posed.pos(Bone::ToesL) + posed.pos(Bone::ToesR)) * 0.5);
        let chest = ground(posed.pos(Bone::Chest));
        let lift = pushup(self.start + th);
        let mut shadow = Path::new();
        shadow.smooth_closed(
            &shape::ellipse(
                (toes + chest) * 0.5 + Vec2::new(0.0, 3.0),
                (chest.x - toes.x).abs() * 0.55,
                9.0 + 6.0 * (1.0 - lift),
                0.0,
                20,
            ),
            1.0,
        );
        shadow.smooth_closed(
            &shape::ellipse(palm + Vec2::new(0.0, 2.0), 46.0, 7.0, 0.0, 16),
            1.0,
        );
        // sweat flicked off the head at every lockout
        let since = beat(self.start + th).rem_euclid(1.0) * crate::procedural::PERIOD;
        let mut drops = Path::new();
        if since < 0.25 {
            let head = cam.to_screen(posed.pos(Bone::Head) + Vec3::new(0.05, 0.12, 0.0));
            let n = beat(self.start + th).floor() as u32;
            for i in 0..3u32 {
                let a = -2.2 + 0.7 * i as f32 + 0.4 * hash2(n, i);
                let v = Vec2::new(a.cos(), a.sin()) * (520.0 + 200.0 * hash2(n + 7, i));
                let p = head + v * since + Vec2::new(0.0, 1400.0) * since * since;
                let dir = (v + Vec2::new(0.0, 2800.0) * since).normalize_or(Vec2::Y);
                let side = Vec2::new(-dir.y, dir.x);
                let r = 9.0;
                drops.smooth_closed(
                    &[p + dir * r * 2.6, p + side * r, p - dir * r, p - side * r],
                    1.0,
                );
            }
        }
        fframes::svgr!(
            <g>
                {bg(palette::YELLOW)}
                {fx::stroke(floor.build(), palette::INK, 12.0)}
                {fx::path(shadow.build(), palette::YELLOW_DEEP)}
                {disc(-30.0)}
                {disc(1710.0)}
                {self.hero.draw(t, &cam, &Paint::line_art(palette::YELLOW))}
                <path d={drops.build()} fill="#FFFFFF" stroke={palette::INK} stroke-width="3" />
                {name_bands(t, palette::PUFF, 4)}
            </g>
        )
    }
}

// ---------------------------------------------------------------------------------------------

/// A heap of cream puffs, more dropping on top, the hero's face lighting up beside it.
pub struct Pile {
    hero: Actor,
    drops: Vec<(f32, f32, Vec<BallState>)>,
}

/// The heap: a dome of puffs on the plate, the top ones first (drawn behind).
const PILE: [(f32, f32, f32); 15] = [
    (560.0, 470.0, 92.0),
    (440.0, 575.0, 98.0),
    (680.0, 580.0, 96.0),
    (330.0, 690.0, 104.0),
    (560.0, 690.0, 110.0),
    (790.0, 700.0, 102.0),
    (220.0, 815.0, 108.0),
    (440.0, 815.0, 116.0),
    (670.0, 820.0, 112.0),
    (900.0, 830.0, 104.0),
    (110.0, 945.0, 112.0),
    (330.0, 950.0, 124.0),
    (560.0, 955.0, 128.0),
    (790.0, 950.0, 120.0),
    (1010.0, 960.0, 110.0),
];

impl Pile {
    pub fn new(len: f32) -> Self {
        let sk = member(Who::Neogoose).skeleton();
        let looks = acting(
            Acting::new(Look {
                sparkle: 1.0,
                pupils: 1.3,
                brows: 0.6,
                mouth: Mouth::Smile,
                ..Default::default()
            })
            .then(0.3, |l| Look {
                tears: 0.4,
                ..expr::excited(l)
            })
            .then(0.55, |l| Look {
                gaze: Vec2::new(0.3, 0.2),
                ..l
            })
            .blink_at(0.12),
        );
        let hero = Actor::new(
            Who::Neogoose,
            idle_head(&sk, |_| -40.0),
            looks,
            len,
            2,
            Vec3::ZERO,
        );
        let drops = (0..4)
            .map(|i| {
                let x = 420.0 + i as f32 * 150.0;
                let from = 0.05 + i as f32 * 0.12;
                let floor = 460.0 - (i % 2) as f32 * 40.0;
                let states = bake_ball(
                    BallSetup {
                        pos: Vec2::new(x, -150.0),
                        vel: Vec2::new(40.0 - i as f32 * 25.0, 300.0),
                        radius: 95.0,
                        spin: 1.0 - i as f32 * 0.6,
                        gravity: 5000.0,
                        restitution: 0.35,
                        bounds: [100.0, -3000.0, 1100.0, floor],
                    },
                    &[],
                    len,
                    FPS,
                );
                (from, 95.0, states)
            })
            .collect();
        Pile { hero, drops }
    }
}

impl Shot for Pile {
    fn render<'a>(
        &'a self,
        t: f32,
        _len: f32,
        _f: &Frame,
        _c: &FFramesContext<'a, '_>,
    ) -> Svgr<'a> {
        let th = twos(t);
        let head = self.hero.posed(t).pos(Bone::Head) + Vec3::new(0.0, 0.08, 0.0);
        let mut cam = Camera::orbit(head, 0.7, -38.0, 2.0, 30.0, W, H);
        cam.shift = Vec2::new(520.0, 120.0);
        let mut puffs: Vec<Svgr<'a>> = PILE
            .iter()
            .enumerate()
            .map(|(i, (x, y, r))| {
                let wobble = (th * 9.0 + i as f32).sin() * 0.03;
                fx::puff(
                    Vec2::new(*x, *y),
                    *r,
                    wobble + (i as f32 * 0.7).sin() * 0.25,
                    1.0,
                    Vec2::Y,
                    6.0,
                )
            })
            .collect();
        for (from, r, states) in &self.drops {
            if th >= *from {
                let s = BallState::at(states, th - from, FPS);
                puffs.push(fx::puff(s.pos, *r, s.rot, s.squash, s.normal, 6.0));
            }
        }
        let mut sparkles = Path::new();
        for i in 0..5 {
            let ph = (th * 2.5 + hash2(61, i)).fract();
            sparkles.append(&fx::sparkle(
                Vec2::new(150.0 + hash2(62, i) * 950.0, 250.0 + hash2(63, i) * 400.0),
                50.0 * (1.0 - (ph - 0.5).abs() * 2.0),
                0.0,
            ));
        }
        fframes::svgr!(
            <g>
                {bg(palette::YELLOW)}
                {self.hero.draw(t, &cam, &Paint::two_tone(palette::YELLOW))}
                <ellipse cx="580" cy="1010" rx="640" ry="90" fill={palette::WHITE} stroke={palette::INK} stroke-width="8" />
                {puffs}
                {fx::path(sparkles.build(), palette::WHITE)}
                {name_bands(t, palette::PUFF, 4)}
            </g>
        )
    }
}

/// The empty plate, crumbs still bouncing.
pub struct Plate {
    crumbs: Vec<Vec<BallState>>,
    /// The hero peering in from the edge at the empty plate.
    hero: Actor,
}

impl Plate {
    pub fn new(len: f32) -> Self {
        let sk = member(Who::Neogoose).skeleton();
        // all gone: eyes wide on the empty plate, then the gloom sets in
        let looks = acting(
            Acting::new(Look::default())
                .then(0.0, |l| Look {
                    gaze: Vec2::new(-0.7, -0.6),
                    ..expr::surprised(l)
                })
                .then(0.3, |l| Look {
                    gaze: Vec2::new(-0.6, -0.7),
                    tears: 0.5,
                    mouth: Mouth::Wavy,
                    ..expr::shocked(l)
                }),
        );
        // the same drawing as the pile shot, a beat later: only the face and the plate changed
        let hero = Actor::new(
            Who::Neogoose,
            idle_head(&sk, |_| -40.0),
            looks,
            len,
            2,
            Vec3::ZERO,
        );
        let crumbs = (0..9)
            .map(|i| {
                bake_ball(
                    BallSetup {
                        pos: Vec2::new(200.0 + hash2(71, i) * 760.0, 760.0 - hash2(72, i) * 200.0),
                        vel: Vec2::new((hash2(73, i) - 0.5) * 300.0, -200.0 - hash2(74, i) * 400.0),
                        radius: 9.0 + hash2(75, i) * 10.0,
                        spin: 4.0,
                        gravity: 4000.0,
                        restitution: 0.15,
                        bounds: [0.0, -3000.0, 1150.0, 985.0],
                    },
                    &[],
                    len,
                    FPS,
                )
            })
            .collect();
        Plate { crumbs, hero }
    }
}

impl Shot for Plate {
    fn render<'a>(
        &'a self,
        t: f32,
        _len: f32,
        _f: &Frame,
        _c: &FFramesContext<'a, '_>,
    ) -> Svgr<'a> {
        let th = twos(t);
        let crumbs: Vec<Svgr<'a>> = self
            .crumbs
            .iter()
            .enumerate()
            .map(|(i, st)| {
                let s = BallState::at(st, th, FPS);
                let r = 9.0 + hash2(75, i as u32) * 10.0;
                // a jagged flake of choux
                let pts: Vec<Vec2> = (0..5)
                    .map(|k| {
                        let a = s.rot + k as f32 / 5.0 * std::f32::consts::TAU;
                        s.pos + Vec2::from_angle(a) * r * (0.6 + 0.6 * hash2(76 + k, i as u32))
                    })
                    .collect();
                let mut flake = Path::new();
                flake.polygon(&pts);
                fframes::svgr!(<path d={flake.build()} fill={palette::PUFF} stroke={palette::INK} stroke-width="2.5" stroke-linejoin="round" />)
            })
            .collect();
        let head = self.hero.posed(t).pos(Bone::Head) + Vec3::new(0.0, 0.08, 0.0);
        let mut cam = Camera::orbit(head, 0.7, -38.0, 2.0, 30.0, W, H);
        cam.shift = Vec2::new(520.0, 120.0);
        fframes::svgr!(
            <g>
                {bg(palette::YELLOW)}
                {self.hero.draw(t, &cam, &Paint::two_tone(palette::YELLOW))}
                <ellipse cx="580" cy="1010" rx="640" ry="90" fill={palette::WHITE} stroke={palette::INK} stroke-width="8" />
                {crumbs}
                {name_bands(t, palette::PUFF, 4)}
            </g>
        )
    }
}

// ---------------------------------------------------------------------------------------------

/// Superman dive across the frame, the robe streaming behind.
pub struct Dive {
    hero: Actor,
}

impl Dive {
    pub fn new(len: f32) -> Self {
        Self::with_speed(len, 9.0, true)
    }

    pub fn with_speed(len: f32, speed: f32, arms_forward: bool) -> Self {
        let sk = member(Who::Neogoose).skeleton();
        let m = motion(move |t| {
            let x = 0.6 - t * speed;
            let body = Body {
                root: Vec3::new(x, 1.1 + (t * 7.0).sin() * 0.03, 0.0),
                yaw: -90.0,
                hips: Vec3::new(0.0, 88.0, 0.0),
                neck: Vec3::new(0.0, -40.0, 0.0),
                head: Vec3::new(0.0, -30.0, 0.0),
                ..Default::default()
            };
            build(
                &sk,
                body,
                if arms_forward {
                    Arms {
                        left: Some(Vec3::new(0.1, 0.74, 0.06)),
                        right: Some(Vec3::new(-0.08, 0.7, 0.12)),
                        elbow_out: 0.6,
                        ..Default::default()
                    }
                } else {
                    Arms::default()
                },
                None,
            )
        });
        let looks = acting(
            Acting::new(Look {
                hand_l: HandShape::Fist,
                hand_r: HandShape::Fist,
                ..Default::default()
            })
            .then(0.0, |l| Look {
                mouth: Mouth::Grin,
                ..expr::determined(l)
            })
            .then(0.42, expr::shout),
        );
        // flying left at 9 m/s through still air: the drag streams the robe and hair behind
        let hero = Actor::new(Who::Neogoose, m, looks, len, 2, Vec3::ZERO);
        Dive { hero }
    }
}

impl Shot for Dive {
    fn render<'a>(
        &'a self,
        t: f32,
        _len: f32,
        _f: &Frame,
        _c: &FFramesContext<'a, '_>,
    ) -> Svgr<'a> {
        let th = twos(t);
        // The camera tracks the flight a little slower than the hero, who drifts across the frame.
        let hx = 0.6 - t * 9.0;
        let cx = hx * 0.93 - 0.45;
        let cam = Camera::new(Vec3::new(cx, 1.2, 2.5), Vec3::new(cx, 1.1, 0.0), 32.0, W, H);
        let lines = fx::speed_lines(0.0, 40, (th * 24.0) as u32, th * 9000.0, 7.0);
        fframes::svgr!(
            <g>
                {bg(palette::YELLOW)}
                {fx::path(lines.build(), palette::YELLOW_DEEP)}
                {self.hero.draw(t, &cam, &Paint::two_tone(palette::YELLOW))}
                {name_bands(t * 3.0, palette::PUFF, 4)}
            </g>
        )
    }
}

// ---------------------------------------------------------------------------------------------

/// Extreme close-up: a cream puff disappears in one bite, cream flies.
pub struct Chomp {
    hero: Actor,
    cream: Vec<Vec<BallState>>,
}

const BITE: f32 = 0.2;

impl Chomp {
    pub fn new(len: f32) -> Self {
        let sk = member(Who::Neogoose).skeleton();
        let looks = acting(expr::chew(
            Acting::new(Look {
                mouth: Mouth::Open(1.0),
                pupils: 1.15,
                brows: 0.7,
                sparkle: 0.7,
                ..Default::default()
            })
            .then(BITE, expr::bliss),
            BITE + 0.1,
            0.54,
            0.09,
        ));
        let hero = Actor::new(
            Who::Neogoose,
            idle_head(&sk, |_| 15.0),
            looks,
            len,
            2,
            Vec3::ZERO,
        );
        let cream = (0..14)
            .map(|i| {
                let a = -std::f32::consts::FRAC_PI_2 + (hash2(81, i) - 0.5) * 2.6;
                let v = 900.0 + hash2(82, i) * 1500.0;
                bake_ball(
                    BallSetup {
                        pos: Vec2::new(1000.0, 760.0),
                        vel: Vec2::from_angle(a) * v,
                        radius: 14.0 + hash2(83, i) * 22.0,
                        spin: 0.0,
                        gravity: 3800.0,
                        restitution: 0.3,
                        bounds: [-500.0, -3000.0, 2500.0, 1400.0],
                    },
                    &[],
                    len,
                    FPS,
                )
            })
            .collect();
        Chomp { hero, cream }
    }
}

impl Shot for Chomp {
    fn render<'a>(
        &'a self,
        t: f32,
        _len: f32,
        _f: &Frame,
        _c: &FFramesContext<'a, '_>,
    ) -> Svgr<'a> {
        let th = twos(t);
        let head = self.hero.posed(t).pos(Bone::Head) + Vec3::new(0.0, 0.06, 0.0);
        let cam = Camera::orbit(head, 0.34, 15.0, 0.0, 30.0, W, H);
        let mut layers: Vec<Svgr<'a>> = vec![
            bg(palette::YELLOW),
            self.hero.draw(t, &cam, &Paint::two_tone(palette::YELLOW)),
        ];
        if th < BITE {
            let k = th / BITE;
            layers.push(fx::puff(
                Vec2::new(1000.0 + 300.0 * (1.0 - k), 780.0 + 200.0 * (1.0 - k)),
                260.0 - 60.0 * k,
                0.3,
                1.0,
                Vec2::Y,
                8.0,
            ));
        } else {
            for (i, st) in self.cream.iter().enumerate() {
                let s = BallState::at(st, th - BITE, FPS);
                let r = 14.0 + hash2(83, i as u32) * 22.0;
                layers.push(fframes::svgr!(<circle cx={s.pos.x} cy={s.pos.y} r={r} fill={palette::CREAM} stroke={palette::INK} stroke-width="4" />));
            }
        }
        fframes::svgr!(<g>{layers}</g>)
    }
}

/// Debug view of the dive from three sides.
pub struct DiveLab(pub Dive);

impl Shot for DiveLab {
    fn render<'a>(
        &'a self,
        t: f32,
        _len: f32,
        _f: &Frame,
        _c: &FFramesContext<'a, '_>,
    ) -> Svgr<'a> {
        let a = &self.0.hero;
        let hips = a.posed(t).pos(Bone::Hips);
        let mut views = Vec::new();
        for (i, (yaw, pitch)) in [(0.0_f32, 0.0_f32), (0.0, 80.0), (90.0, 5.0)]
            .iter()
            .enumerate()
        {
            let mut cam = Camera::orbit(hips, 4.5, *yaw, *pitch, 30.0, W, H);
            cam.shift = Vec2::new((i as f32 - 1.0) * 640.0, 0.0);
            views.push(a.draw(t, &cam, &Paint::two_tone(palette::YELLOW)));
        }
        fframes::svgr!(<g>{bg(palette::YELLOW)}{views}</g>)
    }
}
