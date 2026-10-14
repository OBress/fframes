//! 71.5-86.7 s: the cape, the colosseum, the giant, the fight and the last cream puff.

use std::f32::consts::{PI, TAU};
use std::sync::Arc;

use fframes::{Color, FFramesContext, Frame, Svgr};
use fframes_anime::camera::Camera;
use fframes_anime::glam::{Quat, Vec2, Vec3};
use fframes_anime::rig::{Bone, Side};
use fframes_anime::shape::{self, Path};
use fframes_anime::toon::{HandShape, Look, Mouth};

use crate::actor::{Actor, acting, look};
use crate::cast::{Who, member};
use crate::emblem;
use crate::expr::{self, Acting};
use crate::fx::{self, bg, ease_out, hash2, smooth, spring, twos};
use crate::gpu;
use crate::motion::{Library, Play, in_place};
use crate::palette::{self, Paint};
use crate::physics2d::{BallSetup, BallState, bake_ball};
use crate::procedural::{Arms, Body, build, motion};
use crate::shots::title::{GOTHIC, SANS};
use crate::{H, Shot, W, stage};

const FPS: f32 = 24.0;

/// The arena: tiers of arches around the camera, projected in 3D so camera moves work.
pub fn colosseum<'a>(cam: &Camera) -> Svgr<'a> {
    let mut floor = Vec::new();
    for i in 0..96 {
        let a = i as f32 / 96.0 * TAU;
        let p = Vec3::new(a.cos() * 18.0, 0.0, a.sin() * 18.0);
        if cam.project(p).depth > 0.2 {
            floor.push(cam.to_screen(p));
        }
    }
    let mut bands = [Path::new(), Path::new()];
    let mut arches = Path::new();
    let mut rims = Path::new();
    for k in 0..5 {
        let r = 18.0 + k as f32 * 2.6;
        let (h0, h1) = (k as f32 * 2.4, (k + 1) as f32 * 2.4);
        let n = 64;
        for i in 0..n {
            let a0 = i as f32 / n as f32 * TAU;
            let a1 = (i + 1) as f32 / n as f32 * TAU;
            let at = |a: f32, h: f32, rr: f32| Vec3::new(a.cos() * rr, h, a.sin() * rr);
            let corners = [at(a0, h0, r), at(a1, h0, r), at(a1, h1, r), at(a0, h1, r)];
            if corners.iter().any(|c| cam.project(*c).depth < 0.3) {
                continue;
            }
            let pts: Vec<Vec2> = corners.iter().map(|c| cam.to_screen(*c)).collect();
            bands[k % 2].polygon(&pts);
            rims.polyline(&[pts[3], pts[2]]);
            // the arch opening
            let (b0, b1) = (a0 + (a1 - a0) * 0.22, a0 + (a1 - a0) * 0.78);
            let top = h1 - 0.45;
            let spring_h = top - (b1 - b0) * r * 0.5;
            let mut arch = vec![cam.to_screen(at(b0, h0 + 0.05, r - 0.01))];
            for j in 0..=8 {
                let u = j as f32 / 8.0;
                let a = b0 + (b1 - b0) * u;
                let lift = (u * PI).sin() * (top - spring_h);
                arch.push(cam.to_screen(at(a, spring_h + lift, r - 0.01)));
            }
            arch.push(cam.to_screen(at(b1, h0 + 0.05, r - 0.01)));
            arches.polygon(&arch);
        }
    }
    let mut fp = Path::new();
    if floor.len() > 3 {
        fp.polygon(&floor);
    }
    let [b0, b1] = bands;
    fframes::svgr!(
        <g>
            {bg(palette::YELLOW)}
            {fx::path(b0.build(), "#E2B908")}
            {fx::path(b1.build(), "#D1A707")}
            {fx::path(arches.build(), "#9C7604")}
            {fx::stroke(rims.build(), "#9C7604", 3.0)}
            <path d={fp.build()} fill="#F8E266" />
        </g>
    )
}

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

// ---------------------------------------------------------------------------------------------

/// A spinning jump that throws the robe out like a cape, cut on every few frames; then three
/// of them strike a pose.
pub struct Cape {
    hero: Actor,
    trio: Vec<Actor>,
}

const CAPE_CUTS: [f32; 8] = [0.0, 0.042, 0.084, 0.209, 0.292, 0.417, 0.542, 0.626];

impl Cape {
    pub fn new(len: f32) -> Self {
        let sk = member(Who::Neogoose).skeleton();
        let hip = sk.proportions.hip_height;
        // stands, leaps (the robe billowing up like wings as he drops), lands in a crouch
        const LEAP: f32 = 0.084;
        const LAND: f32 = 0.5;
        let hop = move |t: f32| {
            let u = ((t - LEAP) / (LAND - LEAP)).clamp(0.0, 1.0);
            4.0 * u * (1.0 - u) * 1.1
        };
        let hero = Actor::with_air(
            Who::Neogoose,
            motion(move |t| {
                let u = ((t - LEAP) / (LAND - LEAP)).clamp(0.0, 1.0);
                let crouch = smooth((t - LAND) / 0.08) * 0.2;
                let spread = smooth((t - LEAP) / 0.1);
                let body = Body {
                    root: Vec3::new(0.0, hip + hop(t) - crouch, 0.0),
                    yaw: 0.0 * u,
                    chest: Vec3::new(0.0, 6.0 - 4.0 * spread, 0.0),
                    head: Vec3::new(0.0, 18.0 * spread, 0.0),
                    ..Default::default()
                };
                build(
                    &sk,
                    body,
                    Arms {
                        left: Some(Vec3::new(0.24 + 0.1 * spread, -0.3 + 0.05 * spread, 0.08)),
                        right: Some(Vec3::new(-0.24 - 0.1 * spread, -0.3 + 0.05 * spread, 0.08)),
                        elbow_out: 0.8,
                        ..Default::default()
                    },
                    if t > LAND {
                        Some((Vec3::ZERO, 0.0, 0.3, 0.0))
                    } else {
                        None
                    },
                )
            }),
            acting(
                Acting::new(Look {
                    hand_l: HandShape::Fist,
                    hand_r: HandShape::Fist,
                    ..Default::default()
                })
                .then(0.0, expr::determined)
                .then(LEAP + 0.05, |l| Look {
                    pupils: 0.7,
                    mouth: Mouth::Grin,
                    ..expr::determined(l)
                })
                .then(LAND, |l| Look {
                    gaze: Vec2::new(0.3, 0.0),
                    ..expr::smug(l)
                }),
            ),
            len,
            1,
            Arc::new(move |t: f32| {
                // the air rushes up past him: the robe billows open like wings
                let k = smooth((t - LEAP) / 0.12) * (1.0 - smooth((t - LAND) / 0.15));
                (Vec3::new(0.0, 9.0 * k, 2.0 * k), 26.0 * k)
            }),
        );
        let poses: [(Who, f32, Vec3, Vec3); 3] = [
            (
                Who::Shreyas,
                -0.9,
                Vec3::new(0.25, 0.3, 0.2),
                Vec3::new(-0.1, 0.1, 0.32),
            ),
            (
                Who::Neogoose,
                0.0,
                Vec3::new(0.35, 0.65, 0.1),
                Vec3::new(-0.35, 0.65, 0.1),
            ),
            (
                Who::Chshersh,
                0.9,
                Vec3::new(0.1, 0.1, 0.32),
                Vec3::new(-0.3, 0.35, 0.15),
            ),
        ];
        let trio = poses
            .iter()
            .enumerate()
            .map(|(i, (w, x, l, r))| {
                let (x, l, r) = (*x, *l, *r);
                let crouch = if x == 0.0 { 0.18 } else { 0.0 };
                Actor::new(
                    *w,
                    idle(*w, move |t| {
                        (
                            Body {
                                root: Vec3::new(x, -crouch, 0.0),
                                yaw: -x * 15.0,
                                spine: Vec3::new(0.0, 8.0, 0.0),
                                head: Vec3::new(0.0, (t * 6.0).sin() * 2.0, 0.0),
                                ..Default::default()
                            },
                            Arms {
                                left: Some(l),
                                right: Some(r),
                                elbow_out: 1.2,
                                ..Default::default()
                            },
                        )
                    }),
                    acting(
                        Acting::new(Look {
                            hand_l: HandShape::Peace,
                            hand_r: HandShape::Fist,
                            ..Default::default()
                        })
                        .then(0.0, |l| Look {
                            mouth: Mouth::Grin,
                            sparkle: 0.8,
                            wink: if i == 1 { -1.0 } else { 0.0 },
                            ..expr::happy(l)
                        })
                        .blinks(170 + i as u32, 1.5),
                    ),
                    len,
                    2,
                    Vec3::ZERO,
                )
            })
            .collect();
        Cape { hero, trio }
    }
}

impl Shot for Cape {
    fn para(&self) -> f32 {
        0.5
    }

    fn render<'a>(
        &'a self,
        t: f32,
        _len: f32,
        _f: &Frame,
        _c: &FFramesContext<'a, '_>,
    ) -> Svgr<'a> {
        let k = CAPE_CUTS.iter().rposition(|c| t >= *c).unwrap_or(0);
        if k >= 7 {
            let cam = Camera::new(
                Vec3::new(0.0, 1.1, 3.4),
                Vec3::new(0.0, 1.15, 0.0),
                34.0,
                W,
                H,
            );
            let list: Vec<&Actor> = self.trio.iter().collect();
            return fframes::svgr!(<g>{bg(palette::YELLOW)}{stage::actors(&list, t, &cam, &Paint::two_tone(palette::YELLOW))}</g>);
        }
        // waist-up, then from below as he spins up and the robe flares like wings, then the
        // landing, all on yellow
        // the cameras are placed from where he stands, so the jump moves him through the frame
        let chest = self.hero.posed(0.0).pos(Bone::Chest);
        let paint = Paint::two_tone(palette::YELLOW);
        let cam = match k {
            0 | 1 => {
                let mut c = Camera::new(
                    Vec3::new(0.0, chest.y + 0.05, 1.6),
                    Vec3::new(0.0, chest.y + 0.1, 0.0),
                    40.0,
                    W,
                    H,
                );
                c.shift = Vec2::new(0.0, 160.0);
                c
            }
            2..=4 => {
                let mut c = Camera::new(
                    Vec3::new(0.12, chest.y - 0.45, 1.5),
                    Vec3::new(0.0, chest.y + 0.45, 0.0),
                    56.0,
                    W,
                    H,
                );
                c.roll = (k as f32 - 3.0) * 8.0;
                c
            }
            _ => Camera::new(
                Vec3::new(0.0, 0.7, 3.0),
                Vec3::new(0.0, chest.y - 0.15, 0.0),
                40.0,
                W,
                H,
            ),
        };
        let lines = if (2..=4).contains(&k) {
            fx::path(
                fx::focus_lines(cam.to_screen(chest), 70, 380.0, k as u32 * 11, 3.0).build(),
                palette::INK,
            )
        } else {
            Svgr::empty()
        };
        fframes::svgr!(<g>{bg(palette::YELLOW)}{lines}{self.hero.draw(t, &cam, &paint)}</g>)
    }
}

// ---------------------------------------------------------------------------------------------

/// A finger pointing at the sky, then the hero alone in the middle of the colosseum.
pub struct Point {
    hero: Actor,
}

const POINT_WIDE: f32 = 0.62;

impl Point {
    pub fn new(len: f32) -> Self {
        let hero = Actor::new(
            Who::Neogoose,
            idle(Who::Neogoose, |t| {
                let up = ease_out(t / 0.25);
                (
                    Body {
                        head: Vec3::new(0.0, -15.0 * up, 0.0),
                        ..Default::default()
                    },
                    Arms {
                        right: Some(Vec3::new(-0.15, 0.2 + 0.6 * up, 0.15)),
                        right_palm: Some(Vec3::new(0.0, 0.0, 1.0)),
                        right_fingers: Some(Vec3::new(0.0, 1.0, 0.0)),
                        ..Default::default()
                    },
                )
            }),
            acting(
                Acting::new(Look {
                    hand_r: HandShape::Point,
                    ..Default::default()
                })
                .then(0.0, |l| Look {
                    gaze: Vec2::new(0.0, 0.9),
                    brows: 0.4,
                    ..l
                })
                .then(0.65, |l| Look {
                    gaze: Vec2::new(0.1, 0.0),
                    ..expr::determined(l)
                })
                .blinks(175, 2.0),
            ),
            len,
            2,
            Vec3::new(1.2, 0.0, 0.3),
        );
        Point { hero }
    }
}

impl Shot for Point {
    fn para(&self) -> f32 {
        0.5
    }

    fn render<'a>(
        &'a self,
        t: f32,
        len: f32,
        frame: &Frame,
        _c: &FFramesContext<'a, '_>,
    ) -> Svgr<'a> {
        let th = twos(t);
        if th < POINT_WIDE {
            let hand = self.hero.posed(t).pos(Bone::HandR) + Vec3::new(0.0, 0.07, 0.0);
            let cam = Camera::orbit(hand, 0.55, -10.0, 0.0, 30.0, W, H);
            let mut slash = Path::new();
            let k = ease_out((th - 0.2) / 0.15);
            slash.polygon(&[
                Vec2::new(-100.0, 900.0),
                Vec2::new(-100.0 + 2200.0 * k, 900.0 - 900.0 * k),
                Vec2::new(-100.0 + 2200.0 * k, 930.0 - 900.0 * k),
                Vec2::new(-100.0, 940.0),
            ]);
            // light breaking out from behind the raised finger
            let tip = cam.to_screen(self.hero.posed(t).hand_tip(Side::Right));
            return fframes::svgr!(<g>{bg(palette::YELLOW)}{gpu::rays(frame, tip, Color::hex("#FFFBEB").with_alpha(170), 30.0)}{self.hero.draw(t, &cam, &Paint::line_art(palette::YELLOW))}{fx::path(slash.build(), palette::INK)}</g>);
        }
        let _ = len;
        // locked and low: the hero big in front of the empty arena, looking up
        let cam = Camera::new(
            Vec3::new(0.9, 0.55, 4.2),
            Vec3::new(0.0, 1.9, 0.0),
            42.0,
            W,
            H,
        );
        // a shaft of light on the hero alone in the middle of the arena
        let above = cam.to_screen(self.hero.posed(t).pos(Bone::Head)) - Vec2::new(0.0, 700.0);
        fframes::svgr!(<g>{colosseum(&cam)}{gpu::rays(frame, above, Color::hex("#FFFBEB").with_alpha(120), 20.0)}{gpu::rim_light(self.hero.draw(t, &cam, &Paint::two_tone(palette::YELLOW)), "#FFF7CC", Vec2::new(0.0, -1.0), 4.0)}</g>)
    }
}

// ---------------------------------------------------------------------------------------------

/// Draws the giant `k` times life size standing at `place` in the wide shot.
fn giant_in(wide: &Camera, place: Vec3, k: f32) -> Camera {
    wide.scaled_about(place, Vec3::ZERO, k)
}

/// SLOP's colours: the purple-to-pink gradient of every AI product page, acid-lime irises.
pub(crate) fn demon_paint() -> Paint {
    let mut p = Paint::two_tone("#FDF4FF")
        .with_suit("#6D28D9")
        .with_hair("#A21CAF")
        .with_accent("#D9F99D");
    p.scarf = Some("#F472B6");
    p.line = "#1E0B3A";
    p.ink = "#1E0B3A";
    p.shade = 0.35;
    p.terminator = 0.05;
    p
}

/// The villain's name card (名前テロップ): a black slab sweeps in, "SLOP" slams down with an RGB
/// split and a glitch (it is AI slop), the katakana above it and what it is under it.
fn name_card<'a>(t: f32) -> Svgr<'a> {
    const IN: f32 = 0.1;
    if t < IN {
        return Svgr::empty();
    }
    let u = t - IN;
    let f = (u * 24.0).floor() as u32;
    let slab = ease_out((u / 0.1).min(1.0));
    let slab_x = -1500.0 * (1.0 - slab);
    let slam = match f {
        0 => 1.45,
        1 => 0.9,
        2 => 1.05,
        _ => 1.0,
    };
    let split = if f < 6 { (6 - f) as f32 * 8.0 } else { 0.0 };
    let (sx, sy) = if f < 3 {
        ((f as f32 * 2.3).sin() * 16.0, (f as f32 * 1.7).cos() * 12.0)
    } else {
        (0.0, 0.0)
    };
    let (ax, ay) = (96.0 + sx, 330.0 + sy);
    let title = format!(
        "translate({ax:.1} {ay:.1}) scale({slam:.3}) translate({:.1} {:.1})",
        -ax, -ay
    );
    let slab_t = format!("translate({slab_x:.1} 0)");
    let (left, right, main, base) = (ax - split, ax + split, ax, ay);
    // glitch: on a few frames the word is sliced and the slices slip sideways
    let glitch = matches!(f, 4 | 7 | 8 | 12);
    let slices: Vec<Svgr<'a>> = if glitch {
        (0..4)
            .map(|k| {
                let y0 = 125.0 + k as f32 * 55.0;
                let dx = (hash2(f * 13 + k, 77) - 0.5) * 90.0;
                let id = format!("slopcut{f}x{k}");
                let url = format!("url(#{id})");
                let shift = format!("translate({dx:.1} 0)");
                let colour = if k % 2 == 0 { "#22D3EE" } else { "#F472B6" };
                fframes::svgr!(
                    <g>
                        <clipPath id={id}><rect x="0" y={y0} width="1400" height="34" /></clipPath>
                        <g clip-path={url} transform={shift}>
                            <rect x="0" y={y0} width="1400" height="34" fill={colour} opacity="0.35" />
                            <text x={ax} y={ay} font-family="Dela Gothic One" font-size="230" letter-spacing="8" fill="#FDF4FF">"SLOP"</text>
                        </g>
                    </g>
                )
            })
            .collect()
    } else {
        Vec::new()
    };
    let flash = if f == 0 { 0.55 } else { 0.0 };
    fframes::svgr!(
        <g>
            <rect x="0" y="0" width="1920" height="1080" fill="#FFFFFF" opacity={flash} />
            <g transform={slab_t}>
                <path d="M0 74 L880 44 L842 404 L0 424 Z" fill="#14081F" opacity="0.93" />
                <path d="M0 60 L882 30 L880 44 L0 74 Z" fill="#F472B6" />
                <path d="M0 424 L842 404 L839 416 L0 437 Z" fill="#8B5CF6" />
            </g>
            <text x="104" y="132" font-family="Zen Kaku Gothic New" font-weight="900" font-size="52" letter-spacing="16" fill="#F472B6">"スロップ"</text>
            <text x="790" y="128" text-anchor="end" font-family="Zen Kaku Gothic New" font-weight="900" font-size="30" letter-spacing="3" fill="#A78BFA">"生成AIの巨人"</text>
            <g transform={title}>
                <text x={left} y={base} font-family="Dela Gothic One" font-size="230" letter-spacing="8" fill="#22D3EE" opacity="0.85">"SLOP"</text>
                <text x={right} y={base} font-family="Dela Gothic One" font-size="230" letter-spacing="8" fill="#F472B6" opacity="0.85">"SLOP"</text>
                <text x={main} y={base} font-family="Dela Gothic One" font-size="230" letter-spacing="8" fill="#1E0B3A" stroke="#1E0B3A" stroke-width="18" stroke-linejoin="round">"SLOP"</text>
                <text x={main} y={base} font-family="Dela Gothic One" font-size="230" letter-spacing="8" fill="#FDF4FF">"SLOP"</text>
                {slices}
            </g>
            <text x="106" y="392" font-family="Dela Gothic One" font-size="34" letter-spacing="9" fill="#FDF4FF">"AI-GENERATED GIANT"</text>
        </g>
    )
}

/// The fight, on the reference's beats: the giant towers over the tiny hero, the friends
/// scream, a punch to its face sends teeth flying, it is launched tumbling across the arena, a
/// A scream: the jaw wide open and shaking between two drawings (an open mouth that bounces
/// reads as a laugh).
fn scream(a: Acting, from: f32, to: f32, rate: f32) -> Acting {
    let mut track = Vec::new();
    let mut t = from;
    let mut wide = true;
    while t < to {
        track.push((t, Mouth::O(if wide { 1.0 } else { 0.8 })));
        wide = !wide;
        t += 0.5 / rate;
    }
    a.lips(&track)
}

/// shoe in close-up, an aerial combo of afterimages, the hero flying off.
pub struct Fight {
    demon: Actor,
    imp: Actor,
    tumble: Actor,
    hero: Actor,
    /// The hero flying off after the last hit: its own actor so the robe streams behind him
    /// instead of being dragged across the arena.
    flyer: Actor,
    fist: Actor,
    combo: Vec<Actor>,
    friends: [Actor; 2],
    teeth: Vec<Vec<BallState>>,
}

const F_SCREAM: f32 = 0.67;
const F_PUNCH: f32 = 1.33;
/// The punch lands on the onset at 75.30 s.
const F_HIT: f32 = 1.59;
const F_LAUNCH: f32 = 1.75;
const F_SHOE: f32 = 2.3;
const F_COMBO: f32 = 2.75;
/// The aerial combo's hits, on the half beats (shot seconds): one silhouette at a time.
const F_HITS: [f32; 4] = [2.888, 3.079, 3.270, 3.461];
/// The shoe meets the giant on the beat at 76.25 s.
const F_SHOE_HIT: f32 = 2.544;
/// How much bigger than life the giant is drawn next to the hero in the arena.
const GIANT: f32 = 3.4;
const F_FLY: f32 = 3.85;

/// Where the launched giant's hips are in the arena at shot time `t`.
fn tumble_path(t: f32) -> Vec3 {
    let u = (t - F_LAUNCH).max(0.0);
    Vec3::new(-6.0 + u * 5.5, 3.5 + u * 5.0 - u * u * 1.6, -4.0 + u * 1.2)
}

impl Fight {
    pub fn new(lib: &Library, len: f32) -> Self {
        let dsk = member(Who::Demon).skeleton();
        let dhip = dsk.proportions.hip_height;
        let sk1 = dsk.clone();
        let demon = Actor::new(
            Who::Demon,
            motion(move |t| {
                let roar = smooth(t / 0.25);
                let breathe = (t * 6.0).sin() * 2.0;
                // after the punch lands the head is knocked aside
                let hit = spring(t - F_HIT, 2.0, 6.0).max(0.0);
                let reach = 0.08 * (t * 5.0).sin();
                build(
                    &sk1,
                    Body {
                        root: Vec3::new(0.0, dhip - 0.16, 0.0),
                        spine: Vec3::new(0.0, 22.0, 0.0),
                        chest: Vec3::new(-25.0 * hit, 6.0 - 8.0 * roar + breathe, 10.0 * hit),
                        // glaring down at the hero, then knocked aside by the punch
                        head: Vec3::new(
                            -40.0 * hit,
                            if t < F_SCREAM {
                                38.0 - 8.0 * roar
                            } else {
                                -32.0 * roar + 20.0 * hit
                            },
                            25.0 * hit,
                        ),
                        ..Default::default()
                    },
                    Arms {
                        left: Some(Vec3::new(0.56, -0.38 + reach, 0.5)),
                        right: Some(Vec3::new(-0.56, -0.36 - reach, 0.5)),
                        elbow_out: 2.6,
                        ..Default::default()
                    },
                    Some((Vec3::ZERO, 0.0, 0.4, 0.08)),
                )
            }),
            Arc::new(|t| Look {
                mouth: Mouth::Roar(if t < F_HIT { 1.0 } else { 0.6 }),
                accent_eyes: true,
                pupils: 1.2,
                brows: -1.0,
                hand_l: if t < F_SCREAM {
                    HandShape::Open
                } else {
                    HandShape::Fist
                },
                hand_r: if t < F_SCREAM {
                    HandShape::Open
                } else {
                    HandShape::Fist
                },
                ..Default::default()
            }),
            len,
            2,
            Vec3::ZERO,
        );
        // the same giant flailing in the air
        let sk2 = dsk.clone();
        let tumble = Actor::new(
            Who::Demon,
            motion(move |t| {
                let u = (t - F_LAUNCH).max(0.0);
                let flail = (u * 9.0).sin();
                let body = Body {
                    root: Vec3::new(0.0, dhip, 0.0),
                    spine: Vec3::new(0.0, -20.0 + 10.0 * flail, 0.0),
                    head: Vec3::new(0.0, -30.0, 15.0 * flail),
                    ..Default::default()
                };
                let mut pose = build(
                    &sk2,
                    body,
                    Arms {
                        left: Some(Vec3::new(0.55, 0.45 + 0.2 * flail, -0.1)),
                        right: Some(Vec3::new(-0.6, 0.2 - 0.2 * flail, 0.1)),
                        elbow_out: 1.0,
                        ..Default::default()
                    },
                    None,
                );
                for side in [Side::Left, Side::Right] {
                    let solved = sk2.solve(&pose);
                    let hip = solved.pos(fframes_anime::rig::Bone::leg(side)[0]);
                    let target =
                        hip + Vec3::new(0.3 * side.sign(), -0.65 + 0.15 * flail * side.sign(), 0.3);
                    fframes_anime::rig::two_bone_ik(
                        &sk2,
                        &mut pose,
                        fframes_anime::rig::Bone::leg(side)[0],
                        target,
                        hip + Vec3::new(0.0, 0.0, 1.0),
                        None,
                    );
                }
                let spin = Quat::from_rotation_z(-u * 3.2) * Quat::from_rotation_x(u * 1.3);
                let hips = pose.root;
                let mut p = pose.transformed(spin, Vec3::ZERO);
                p.root = hips;
                p
            }),
            Arc::new(|_| Look {
                mouth: Mouth::Roar(1.0),
                accent_eyes: true,
                pupils: 0.4,
                brows: 1.0,
                hand_l: HandShape::Open,
                hand_r: HandShape::Open,
                ..Default::default()
            }),
            len,
            2,
            Vec3::ZERO,
        );
        let hsk = member(Who::Neogoose).skeleton();
        let hhip = hsk.proportions.hip_height;
        let hsk2 = hsk.clone();
        let hero = Actor::new(
            Who::Neogoose,
            motion(move |t| {
                if t < F_FLY {
                    build(
                        &hsk2,
                        Body {
                            root: Vec3::new(0.0, hhip, 1.6),
                            head: Vec3::new(0.0, -20.0, 0.0),
                            ..Default::default()
                        },
                        Arms::default(),
                        Some((Vec3::new(0.0, 0.0, 1.6), 0.0, 0.18, 0.0)),
                    )
                } else {
                    let u = t - F_FLY;
                    build(
                        &hsk2,
                        Body {
                            root: Vec3::new(-2.0 + u * 8.0, 2.5 + u * 3.0, 0.0),
                            yaw: 90.0,
                            hips: Vec3::new(0.0, 70.0, 0.0),
                            ..Default::default()
                        },
                        Arms {
                            left: Some(Vec3::new(0.1, 0.7, 0.1)),
                            right: Some(Vec3::new(-0.2, 0.2, 0.3)),
                            ..Default::default()
                        },
                        None,
                    )
                }
            }),
            look(Look {
                face: false,
                hand_l: HandShape::Fist,
                hand_r: HandShape::Fist,
                ..Default::default()
            }),
            len,
            2,
            Vec3::new(3.0, 0.0, 0.0),
        );
        // the punch: an arm and fist thrown forward from the right
        let hsk3 = hsk.clone();
        let fist = Actor::new(
            Who::Neogoose,
            motion(move |t| {
                // cocked back off-screen, then two drawings to land on the hit, an overshoot
                let k = match t - F_HIT {
                    d if d < -0.09 => -0.25,
                    d if d < -0.045 => 0.45,
                    d if d < 0.0 => 0.85,
                    d if d < 0.045 => 1.06,
                    _ => 1.0,
                };
                build(
                    &hsk3,
                    Body {
                        root: Vec3::new(0.0, hhip, 0.0),
                        yaw: -20.0,
                        chest: Vec3::new(-20.0 * k, 0.0, 0.0),
                        ..Default::default()
                    },
                    Arms {
                        right: Some(Vec3::new(-0.2 + 0.1 * k, 0.12, 0.2 + 0.55 * k)),
                        elbow_out: 0.5,
                        ..Default::default()
                    },
                    None,
                )
            }),
            look(Look {
                face: false,
                hand_r: HandShape::Fist,
                ..Default::default()
            }),
            len,
            1,
            Vec3::ZERO,
        );
        let kick = in_place(
            lib,
            "kick_normal",
            &hsk,
            Play {
                from: 0.55,
                looped: false,
                speed: 1.6,
                ..Default::default()
            },
        );
        let punch = in_place(
            lib,
            "punch_normal",
            &hsk,
            Play {
                from: 0.8,
                looped: false,
                speed: 1.6,
                ..Default::default()
            },
        );
        let combo = (0..4)
            .map(|i| {
                let m = if i % 2 == 0 {
                    kick.clone()
                } else {
                    punch.clone()
                };
                let yaw = [70.0_f32, -110.0, 160.0, 20.0][i];
                Actor::new(
                    Who::Neogoose,
                    motion(move |t| {
                        // each strike lands on its half beat (the clip's impact is 0.12 s in)
                        let p = m((t - F_HITS[i] + 0.12).max(0.0));
                        p.transformed(Quat::from_rotation_y(yaw.to_radians()), Vec3::ZERO)
                    }),
                    look(Look {
                        face: false,
                        hand_l: HandShape::Fist,
                        hand_r: HandShape::Fist,
                        ..Default::default()
                    }),
                    len,
                    2,
                    Vec3::ZERO,
                )
            })
            .collect();
        let scream = |who: Who, yaw: f32, x: f32| {
            Actor::new(
                who,
                idle(who, move |t| {
                    let shake = (t * 50.0).sin() * 2.0;
                    (
                        Body {
                            root: Vec3::new(x, 0.0, 0.0),
                            yaw,
                            head: Vec3::new(shake, -5.0, 0.0),
                            ..Default::default()
                        },
                        // hands clapped to the cheeks: a scream
                        Arms {
                            left: Some(Vec3::new(0.11, 0.24, 0.1)),
                            right: Some(Vec3::new(-0.11, 0.24, 0.1)),
                            elbow_out: 0.9,
                            left_palm: Some(Vec3::new(-1.0, 0.0, 0.1)),
                            right_palm: Some(Vec3::new(1.0, 0.0, 0.1)),
                            left_fingers: Some(Vec3::new(0.0, 1.0, 0.1)),
                            right_fingers: Some(Vec3::new(0.0, 1.0, 0.1)),
                        },
                    )
                }),
                acting(
                    scream(
                        Acting::new(Look {
                            hand_l: HandShape::Open,
                            hand_r: HandShape::Open,
                            ..Default::default()
                        })
                        .then(0.0, |l| Look {
                            pupils: 0.3,
                            brow_angle: -1.0,
                            tears: 0.6,
                            mouth: Mouth::O(1.0),
                            ..expr::shocked(l)
                        }),
                        0.0,
                        F_PUNCH + 1.0,
                        7.0,
                    )
                    .darts(180 + (x * 10.0).abs() as u32, 0.25, 0.09),
                ),
                len,
                2,
                Vec3::ZERO,
            )
        };
        let friends = [
            scream(Who::Theo, 15.0, -0.3),
            scream(Who::Shreyas, -15.0, 0.3),
        ];
        let teeth = (0..10)
            .map(|i| {
                let a = -PI * (0.1 + hash2(811, i) * 0.6);
                bake_ball(
                    BallSetup {
                        pos: Vec2::new(900.0, 640.0),
                        vel: Vec2::from_angle(a) * (900.0 + hash2(812, i) * 1600.0),
                        radius: 14.0 + hash2(813, i) * 16.0,
                        spin: 9.0,
                        gravity: 2600.0,
                        restitution: 0.3,
                        bounds: [-500.0, -3000.0, 2500.0, 1400.0],
                    },
                    &[],
                    len,
                    FPS,
                )
            })
            .collect();
        // a small demon clinging on, clawing and hissing
        let isk = dsk.clone();
        let imp = Actor::new(
            Who::Demon,
            motion(move |t| {
                let claw = (t * 9.0).sin();
                build(
                    &isk,
                    Body {
                        root: Vec3::new(0.0, dhip * 0.8, 0.0),
                        spine: Vec3::new(0.0, 35.0, 0.0),
                        head: Vec3::new(10.0 * claw, -20.0, 0.0),
                        ..Default::default()
                    },
                    Arms {
                        left: Some(Vec3::new(0.35, 0.3 + 0.1 * claw, 0.45)),
                        right: Some(Vec3::new(-0.35, 0.2 - 0.1 * claw, 0.45)),
                        elbow_out: 1.4,
                        ..Default::default()
                    },
                    Some((Vec3::ZERO, 0.0, 0.3, 0.2)),
                )
            }),
            acting(
                Acting::new(Look {
                    accent_eyes: true,
                    hand_l: HandShape::Open,
                    hand_r: HandShape::Open,
                    ..Default::default()
                })
                .then(0.0, |l| Look {
                    brow_angle: 1.0,
                    mouth: Mouth::Roar(0.8),
                    gaze: Vec2::new(0.0, -0.5),
                    ..l
                })
                .lips(&[
                    (0.0, Mouth::Roar(0.6)),
                    (0.25, Mouth::Roar(1.0)),
                    (0.6, Mouth::Roar(0.7)),
                    (0.9, Mouth::Roar(1.0)),
                    (F_HIT, Mouth::Roar(1.0)),
                ])
                .then(F_HIT, |l| Look {
                    eyes: 0.15,
                    brow_angle: -0.6,
                    ..l
                })
                .then(F_LAUNCH, |l| Look {
                    eyes: 1.0,
                    pupils: 0.3,
                    brows: 1.0,
                    brow_angle: 0.0,
                    ..l
                }),
            ),
            len,
            2,
            Vec3::ZERO,
        );
        // the last hit: from beside the tumbling giant, up and out past the lens
        let hsk4 = hsk.clone();
        let flyer = Actor::new(
            Who::Neogoose,
            motion(move |t| {
                let u = t - F_FLY;
                build(
                    &hsk4,
                    Body {
                        root: tumble_path(t)
                            + Vec3::new(-1.6 + u * 7.0, -0.6 + u * 4.0, 1.2 + u * 6.0),
                        yaw: 70.0,
                        hips: Vec3::new(0.0, 55.0, 0.0),
                        ..Default::default()
                    },
                    Arms {
                        left: Some(Vec3::new(0.1, 0.7, 0.1)),
                        right: Some(Vec3::new(-0.2, 0.2, 0.3)),
                        ..Default::default()
                    },
                    None,
                )
            }),
            look(Look {
                face: false,
                hand_l: HandShape::Fist,
                hand_r: HandShape::Fist,
                ..Default::default()
            }),
            len,
            2,
            Vec3::ZERO,
        );
        Fight {
            demon,
            imp,
            tumble,
            hero,
            flyer,
            fist,
            combo,
            friends,
            teeth,
        }
    }
}

impl Shot for Fight {
    fn render<'a>(
        &'a self,
        t: f32,
        len: f32,
        frame: &Frame,
        _c: &FFramesContext<'a, '_>,
    ) -> Svgr<'a> {
        let th = twos(t);
        let dp = demon_paint();
        if th < F_SCREAM {
            // low behind the hero: the giant towers over him and fills the frame
            // a slow tilt up the giant, from its chest to its horns
            let rise = ease_out(t / 0.6);
            // low behind the hero, looking up: his head at the bottom of the frame, the giant
            // filling it up to its horns
            let cam = Camera::new(
                Vec3::new(0.0, 6.2, 7.0),
                Vec3::new(0.0, 9.2 + 0.7 * rise, -1.0),
                54.0,
                W,
                H,
            );
            let giant = cam.scaled_about(Vec3::new(0.0, 0.0, -1.0), Vec3::ZERO, 7.0);
            let lines =
                fx::focus_lines(Vec2::new(960.0, 300.0), 90, 560.0, (th * 24.0) as u32, 3.0);
            // the hero composited small at the bottom of the frame, seen from behind
            let hero_head = self.hero.posed(t).pos(Bone::Head);
            let mut hero_cam = Camera::orbit(hero_head, 6.0, 180.0, -6.0, 44.0, W, H);
            hero_cam.shift = Vec2::new(0.0, 470.0);
            // imps clinging to the giant's arms and shoulders
            let posed = self.demon.posed(t);
            let world = |p: Vec3| Vec3::new(0.0, 0.0, -1.0) + p * 7.0;
            let spots = [
                (
                    posed.pos(Bone::LowerArmL).lerp(posed.pos(Bone::HandL), 0.4),
                    0.0_f32,
                ),
                (
                    posed.pos(Bone::LowerArmR).lerp(posed.pos(Bone::HandR), 0.4),
                    1.0,
                ),
                (
                    posed
                        .pos(Bone::UpperArmL)
                        .lerp(posed.pos(Bone::LowerArmL), 0.5),
                    2.0,
                ),
                (
                    posed
                        .pos(Bone::UpperArmR)
                        .lerp(posed.pos(Bone::LowerArmR), 0.5),
                    3.0,
                ),
            ];
            let imps: Vec<Svgr<'a>> = spots
                .iter()
                .map(|(p, k)| {
                    let mut c = cam.scaled_about(world(*p), Vec3::new(0.0, 1.0, 0.0), 0.85);
                    c.roll = (k - 1.5) * 25.0;
                    self.imp.draw(t + k * 0.13, &c, &dp)
                })
                .collect();
            let _ = lines;
            return fframes::svgr!(
                <g>
                    {bg(palette::YELLOW)}
                    {gpu::rays(frame, Vec2::new(960.0, 260.0), Color::hex("#C084FC").with_alpha(110), 28.0)}
                    {gpu::focus_lines(frame, Vec2::new(960.0, 300.0), 560.0, Color::hex("#C79A06"), 150.0)}
                    {self.demon.draw(t, &giant, &dp)}
                    {imps}
                    {self.hero.draw(t, &hero_cam, &Paint::silhouette(palette::INK))}
                    {name_card(t)}
                </g>
            );
        }
        if th < F_PUNCH {
            let cam = Camera::new(
                Vec3::new(0.0, 1.58, 1.25),
                Vec3::new(0.0, 1.56, 0.0),
                30.0,
                W,
                H,
            );
            let list: Vec<&Actor> = self.friends.iter().collect();
            return fframes::svgr!(<g>{bg(palette::YELLOW)}{gpu::focus_lines(frame, Vec2::new(960.0, 500.0), 420.0, Color::hex(palette::INK), 120.0)}{stage::actors(&list, t, &cam, &Paint::two_tone(palette::YELLOW))}</g>);
        }
        if th < F_LAUNCH {
            // the punch lands on the giant's face; teeth fly
            let head = self.demon.posed(t).pos(Bone::Head) + Vec3::new(0.0, 0.08, 0.0);
            let mut cam = Camera::orbit(head, 0.55, -15.0, 0.0, 30.0, W, H);
            cam.shift = Vec2::new(-180.0, 40.0);
            let fist_hand = self.fist.posed(t).pos(Bone::HandR);
            let mut fcam = Camera::orbit(
                fist_hand + Vec3::new(0.0, 0.0, -0.1),
                0.9,
                75.0,
                0.0,
                30.0,
                W,
                H,
            );
            fcam.shift = Vec2::new(-80.0, 60.0);
            // where the fist meets the face
            let contact = fcam.to_screen(fist_hand);
            // the face squashes for two drawings as the fist lands
            let squash = (0.0..2.0 / 12.0).contains(&(th - F_HIT));
            let face = cam.to_screen(head);
            let demon = self.demon.draw(t, &cam, &dp);
            let demon = if squash {
                let tf = format!(
                    "translate({:.1} {:.1}) scale(1.16 0.8) translate({:.1} {:.1})",
                    face.x, face.y, -face.x, -face.y
                );
                fframes::svgr!(<g transform={tf}>{demon}</g>)
            } else {
                demon
            };
            let mut layers: Vec<Svgr<'a>> = vec![bg("#7A1410"), demon];
            if th >= F_HIT {
                let k = (th - F_HIT) / 0.3;
                layers.push(fx::path(
                    fx::splash(contact, 160.0 + 300.0 * k, 77, 8).build(),
                    palette::WHITE,
                ));
                for (i, st) in self.teeth.iter().enumerate() {
                    let s = BallState::at(st, th - F_HIT, FPS);
                    let r = 14.0 + hash2(813, i as u32) * 16.0;
                    let tooth = [
                        s.pos + Vec2::new(-r, -r * 0.6),
                        s.pos + Vec2::new(r, -r * 0.6),
                        s.pos + Vec2::new(r * 0.6, r),
                        s.pos + Vec2::new(-r * 0.6, r),
                    ];
                    let mut p = Path::new();
                    p.polygon(&tooth);
                    layers.push(fframes::svgr!(<path d={p.build()} fill={palette::WHITE} stroke={palette::INK} stroke-width="3" />));
                }
            }
            if th >= F_HIT - 0.045 {
                // only the arm: the rest of the hero is out of this shot
                layers.push(crate::render::draw_in(
                    &[(&self.fist, Paint::two_tone(palette::YELLOW))],
                    t,
                    &fcam,
                    (0.0, 260.0, 1920.0, 820.0),
                ));
            }
            let hit = (th - F_HIT) / 0.25;
            layers.push(gpu::burst(
                frame,
                contact,
                300.0,
                hit,
                Color::hex("#FFFFFF"),
                Color::hex("#F472B6"),
                3.0,
            ));
            let split = if (0.0..0.5).contains(&hit) {
                22.0 * (1.0 - hit * 2.0)
            } else {
                0.0
            };
            return gpu::rgb_split(fframes::svgr!(<g>{layers}</g>), split, Vec2::new(1.0, 0.3));
        }
        if th < F_SHOE {
            // launched: the camera tracks the giant spinning up out of a white burst, the hero
            // small below after the kick
            let cam_at = |t: f32| {
                let place = tumble_path(t);
                let mut cam = Camera::new(
                    place + Vec3::new(4.0, -3.0, 10.0),
                    place + Vec3::new(1.2, -0.6, 0.0),
                    44.0,
                    W,
                    H,
                );
                cam.roll = -12.0;
                cam
            };
            let place = tumble_path(t);
            let cam = cam_at(t);
            let before = cam_at(t - 2.0 / 24.0);
            let giant = cam.scaled_about(place, self.tumble.posed(t).pos(Bone::Hips), GIANT);
            let giant_before = before.scaled_about(
                tumble_path(t - 2.0 / 24.0),
                self.tumble.posed(t - 2.0 / 24.0).pos(Bone::Hips),
                GIANT,
            );
            let pan = fx::screen_velocity(place, &cam, &before, 2.0);
            let burst = ease_out((th - F_LAUNCH) / 0.25);
            let c = cam.to_screen(tumble_path(F_LAUNCH));
            // a cel-shaded dust cloud blown out of the launch
            let cloud = gpu::smoke(
                frame,
                c + Vec2::new(0.0, 40.0 - 0.85 * 300.0 * burst.max(0.3)),
                300.0 * burst.max(0.3),
                (0.5 + burst * 0.7).min(1.1),
                gpu::SmokeColors {
                    light: Color::hex("#FFF8E1"),
                    mid: Color::hex("#E9D9A0"),
                    dark: Color::hex("#B8A066"),
                    line: Color::hex(palette::INK),
                },
                4.0,
            );
            return fframes::svgr!(
                <g>
                    {fx::blur_layer(colosseum(&cam), pan)}
                    {cloud}
                    {gpu::burst(frame, c, 260.0, (th - F_LAUNCH) / 0.3, Color::hex("#FFFFFF"), Color::hex("#C084FC"), 5.0)}
                    {self.tumble.draw_moving(t, &giant, &giant_before, &dp)}
                    {self.hero.draw_moving(t, &cam, &before, &Paint::silhouette(palette::INK))}
                </g>
            );
        }
        if th < F_COMBO {
            // the shoe: an extreme close-up of the kick with smears
            let a = &self.combo[0];
            // locked on where the shoe will be when it lands; it slams into the frame
            let foot = a.posed(F_SHOE_HIT + 0.35).pos(Bone::FootR);
            let mut cam = Camera::orbit(
                foot + Vec3::new(0.0, 0.02, 0.05),
                0.42,
                120.0,
                12.0,
                34.0,
                W,
                H,
            );
            cam.shift = Vec2::new(60.0, 40.0);
            let lines = fx::speed_lines(-0.4, 40, (th * 24.0) as u32, th * 7000.0, 12.0);
            let mut shoe = Paint::two_tone(palette::CREAM).with_suit("#3D3226");
            shoe.shade = 0.25;
            let swoosh = fx::speed_lines(0.35, 18, (th * 24.0) as u32 + 7, th * 5000.0, 30.0);
            let impact = gpu::burst(
                frame,
                cam.to_screen(foot) + Vec2::new(-120.0, -40.0),
                360.0,
                (th - F_SHOE_HIT) / 0.25,
                Color::hex("#FFFFFF"),
                Color::hex("#2E8C82"),
                9.0,
            );
            return fframes::svgr!(<g>{bg(palette::YELLOW)}{fx::path(swoosh.build(), palette::ORANGE)}{fx::path(lines.build(), "#2E8C82")}{a.smears(t + 0.35, &cam, palette::ORANGE)}{a.draw(t + 0.35, &cam, &shoe)}{impact}</g>);
        }
        // aerial combo: the camera rides along with the tumbling giant, the hero hitting it from
        // all sides
        let cam_at = |t: f32| {
            let place = tumble_path(t);
            let mut cam = Camera::new(
                place + Vec3::new(-2.0, -2.0, 8.5),
                place + Vec3::new(0.0, -0.3, 0.0),
                44.0,
                W,
                H,
            );
            cam.roll = 8.0 * (fframes_anime::koma::hold(t, 24.0, 2) * 3.0).sin();
            cam
        };
        // hit-stop: the giant and the camera freeze for two frames on every hit
        let stop = F_HITS
            .iter()
            .find(|h| (0.0..2.0 / 24.0).contains(&(th - **h)))
            .copied();
        let tg = stop.unwrap_or(t);
        let place = tumble_path(tg);
        let cam = cam_at(tg);
        let before = cam_at(tg - 2.0 / 24.0);
        let hips = self.tumble.posed(tg).pos(Bone::Hips);
        let giant = cam.scaled_about(place, hips, GIANT);
        let pan = if stop.is_some() {
            Vec2::ZERO
        } else {
            fx::screen_velocity(place, &cam, &before, 2.0)
        };
        let mut layers: Vec<Svgr<'a>> = vec![
            fx::blur_layer(colosseum(&cam), pan),
            self.tumble.draw(tg, &giant, &dp),
        ];
        // the giant's size on screen: strikes land on its outline
        let tp = self.tumble.posed(tg);
        let center = giant.to_screen(tp.pos(Bone::Chest));
        let reach = (giant.to_screen(tp.pos(Bone::Head)) - giant.to_screen(tp.pos(Bone::Hips)))
            .length()
            * 0.75;
        for (i, a) in self.combo.iter().enumerate() {
            let hit = F_HITS[i];
            // one silhouette at a time: a drawing of the wind-up, the strike, gone
            if th < hit - 0.085 || th >= hit + 0.125 || th > F_FLY {
                continue;
            }
            let ang = i as f32 * 1.9 + 0.5;
            let dir = Vec2::from_angle(ang);
            let contact = center + dir * reach;
            let at = contact + dir * 210.0;
            let hips_a = a.posed(t).pos(Bone::Hips);
            let mut c = Camera::orbit(hips_a, 8.5, 0.0, 0.0, 44.0, W, H);
            c.shift = at - Vec2::new(960.0, 540.0);
            let mut c_before = c;
            c_before.shift = at - Vec2::new(960.0, 540.0) + dir * 120.0;
            layers.push(a.draw_moving(t, &c, &c_before, &Paint::silhouette(palette::INK)));
            layers.push(gpu::burst(
                frame,
                contact,
                220.0,
                (th - hit) / 0.22,
                Color::hex("#FFFFFF"),
                Color::hex("#F472B6"),
                i as f32 * 1.7,
            ));
        }
        if th >= F_FLY {
            layers.push(
                self.flyer
                    .draw_moving(t, &cam, &before, &Paint::silhouette(palette::INK)),
            );
        }
        let _ = len;
        fframes::svgr!(<g>{layers}</g>)
    }
}

/// A magenta cloud billows where the giant landed, the hero tiny in the arena.
pub struct Smoke {
    hero: Actor,
}

impl Smoke {
    pub fn new(len: f32) -> Self {
        Smoke {
            hero: Actor::new(
                Who::Neogoose,
                idle(Who::Neogoose, |_| {
                    (
                        Body {
                            root: Vec3::new(-3.0, 0.0, 2.0),
                            yaw: 60.0,
                            ..Default::default()
                        },
                        Arms::default(),
                    )
                }),
                look(Look {
                    face: false,
                    ..Default::default()
                }),
                len,
                2,
                Vec3::new(2.0, 0.0, 0.0),
            ),
        }
    }
}

impl Shot for Smoke {
    fn render<'a>(
        &'a self,
        t: f32,
        _len: f32,
        frame: &Frame,
        _c: &FFramesContext<'a, '_>,
    ) -> Svgr<'a> {
        let th = twos(t);
        let cam = Camera::new(
            Vec3::new(0.0, 6.0, 20.0),
            Vec3::new(0.0, 2.0, 0.0),
            40.0,
            W,
            H,
        );
        // a cauliflower of smoke: lumps outlined as one shape, lit from the upper left
        let base = Vec2::new(1180.0, 800.0);
        // cracks running out over the arena floor, shards of the floor kicked up
        let mut cracks = Path::new();
        let mut shards = Path::new();
        for i in 0..14 {
            let a = PI * (0.05 + 0.9 * hash2(921, i)) + PI;
            let len = 260.0 + hash2(922, i) * 520.0;
            let mut p = base + Vec2::new(0.0, 60.0);
            let mut pts = vec![p];
            for k in 0..5 {
                let dir =
                    Vec2::from_angle(a + (hash2(923 + k, i) - 0.5) * 0.7) * Vec2::new(1.0, -0.32);
                p += dir * len / 5.0;
                pts.push(p);
            }
            cracks.polyline(&pts);
            let s = 26.0 + hash2(924, i) * 44.0;
            let q = base + Vec2::new((hash2(925, i) - 0.5) * 900.0, 70.0 + hash2(926, i) * 60.0);
            let tilt = hash2(927, i) * 0.8 - 0.4;
            shards.polygon(&[
                q + Vec2::new(-s, 0.0),
                q + Vec2::from_angle(-PI * 0.5 + tilt) * s * 1.6,
                q + Vec2::new(s * 0.7, 0.0),
            ]);
        }
        let mut rocks = Path::new();
        for i in 0..16 {
            let v = Vec2::from_angle(-PI * (0.15 + hash2(905, i) * 0.7))
                * (900.0 + hash2(906, i) * 1100.0);
            let p = base + v * th + Vec2::new(0.0, 1600.0 * th * th);
            let s = 10.0 + hash2(907, i) * 22.0;
            // a splat of goo, stretched along its flight
            let dir = (v + Vec2::new(0.0, 3200.0 * th)).normalize_or(Vec2::Y);
            let side = dir.perp();
            rocks.smooth_closed(
                &[
                    p + dir * s * 1.6,
                    p + side * s * 0.8,
                    p - dir * s * 0.9,
                    p - side * s * 0.8,
                ],
                1.0,
            );
        }
        // the cloud: billowing goo-pink smoke on the GPU, three cel tones and an ink line,
        // a flatter dust ring rolling out along the floor under it
        let colors = gpu::SmokeColors {
            light: Color::hex("#F0479B"),
            mid: Color::hex("#C40764"),
            dark: Color::hex("#7A0848"),
            line: Color::hex(palette::INK),
        };
        // a burst, then a slow billow
        let grow = 0.4 + 0.65 * (1.0 - (-th * 9.0).exp());
        let cloud = gpu::smoke(
            frame,
            base + Vec2::new(0.0, 60.0 - 0.85 * 560.0),
            560.0,
            grow,
            colors,
            3.0,
        );
        let dust = gpu::smoke(
            frame,
            base + Vec2::new(-40.0, 70.0 - 0.85 * 220.0),
            220.0,
            (0.5 + th * 1.3).min(1.25),
            gpu::SmokeColors {
                light: Color::hex("#FDE68A"),
                mid: Color::hex("#E8C64A"),
                dark: Color::hex("#B8901A"),
                line: Color::hex(palette::INK),
            },
            9.0,
        );
        fframes::svgr!(
            <g>
                {colosseum(&cam)}
                {self.hero.draw_moving(t, &cam, &cam, &Paint::silhouette(palette::INK))}
                {fx::stroke(cracks.build(), palette::INK, 3.0)}
                <path d={shards.build()} fill="#E8C64A" stroke={palette::INK} stroke-width="3" stroke-linejoin="round" />
                <g transform="translate(0 0) scale(1 1)">{dust}</g>
                {cloud}
                {gpu::burst(frame, base + Vec2::new(0.0, -80.0), 420.0, th / 0.25, Color::hex("#FFFFFF"), Color::hex("#F472B6"), 11.0)}
                <path d={rocks.build()} fill="#4C1255" stroke={palette::INK} stroke-width="3" />
            </g>
        )
    }
}

/// Strobe between the hero's face and the flying kick into the giant.
pub struct Kick {
    hero: Actor,
    demon: Actor,
}

const KICK_CUTS: [f32; 8] = [0.0, 0.084, 0.167, 0.25, 0.334, 0.417, 0.501, 0.626];

impl Kick {
    pub fn new(lib: &Library, len: f32) -> Self {
        let hsk = member(Who::Neogoose).skeleton();
        let kick = in_place(
            lib,
            "kick_normal",
            &hsk,
            Play {
                from: 0.7,
                looped: false,
                speed: 1.0,
                yaw: -90.0,
                ..Default::default()
            },
        );
        // the strike cuts each hold the kick at full extension (the impact drawing), with a
        // little follow-through; the clip's own timing never lines up with the cuts
        let peak = {
            let mut best = (0.0_f32, 0.0_f32);
            for i in 0..90 {
                let t = i as f32 / 60.0;
                let p = hsk.solve(&kick(t));
                let d = (p.pos(Bone::FootR) - p.pos(Bone::Hips)).length();
                if d > best.1 {
                    best = (t, d);
                }
            }
            best.0
        };
        let hero = Actor::new(
            Who::Neogoose,
            motion(move |t| {
                let k = KICK_CUTS.iter().rposition(|c| t >= *c).unwrap_or(0);
                let since = t - KICK_CUTS[k];
                let mut p = kick(peak - 0.05 + since * 0.4);
                p.root += Vec3::new(0.0, 0.8, 0.0);
                p
            }),
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
                .then(0.3, expr::shout)
                .then(0.58, |l| Look {
                    mouth: Mouth::Teeth(0.8),
                    ..expr::determined(l)
                }),
            ),
            len,
            1,
            Vec3::new(8.0, 0.0, 0.0),
        );
        let demon = Actor::new(
            Who::Demon,
            idle(Who::Demon, |t| {
                (
                    Body {
                        head: Vec3::new(20.0, -20.0 - t * 30.0, 10.0),
                        spine: Vec3::new(0.0, -10.0, 0.0),
                        ..Default::default()
                    },
                    Arms::default(),
                )
            }),
            acting(
                Acting::new(Look {
                    accent_eyes: true,
                    ..Default::default()
                })
                .then(0.0, |l| Look {
                    pupils: 0.4,
                    brows: 1.0,
                    mouth: Mouth::Open(0.8),
                    ..l
                })
                .then(0.35, |l| Look {
                    eyes: 0.1,
                    brow_angle: -1.0,
                    mouth: Mouth::Roar(1.0),
                    ..l
                }),
            ),
            len,
            2,
            Vec3::ZERO,
        );
        Kick { hero, demon }
    }
}

impl Shot for Kick {
    fn render<'a>(
        &'a self,
        t: f32,
        _len: f32,
        frame: &Frame,
        _c: &FFramesContext<'a, '_>,
    ) -> Svgr<'a> {
        let k = KICK_CUTS.iter().rposition(|c| t >= *c).unwrap_or(0);
        let since = t - KICK_CUTS[k];
        if k % 2 == 0 && k < 7 {
            let head = self.hero.posed(t).pos(Bone::Head) + Vec3::new(0.0, 0.1, 0.0);
            let cam = Camera::orbit(head, 0.5, -60.0 + k as f32 * 8.0, 0.0, 30.0, W, H);
            let back = if k % 4 == 0 {
                palette::YELLOW
            } else {
                palette::ORANGE
            };
            // the face between the kicks: a fighting aura flaring up behind it
            let aura = gpu::aura(
                frame,
                Vec2::new(960.0, 640.0),
                Vec2::new(1500.0, 1300.0),
                Color::hex("#FFF3B0").with_alpha(235),
                Color::hex(if k % 4 == 0 { "#F97316" } else { "#DC2626" }).with_alpha(220),
                0.95 + 0.25 * (k as f32 * 1.3).sin(),
                k as f32,
            );
            // the face keeps its yellow on the orange cuts
            let mut face = Paint::two_tone(palette::YELLOW);
            face.shade = 0.0;
            return fframes::svgr!(<g>{bg(back)}{aura}{self.hero.draw(t, &cam, &face)}</g>);
        }
        let hips = self.hero.posed(t).pos(Bone::Hips);
        let cam = Camera::orbit(hips, 3.2, 0.0, 0.0, 36.0, W, H);
        let mut gcam = Camera::orbit(
            self.demon.posed(t).pos(Bone::Head),
            1.2,
            -30.0,
            0.0,
            36.0,
            W,
            H,
        );
        // the giant's face sits where the kick lands: the foot at the moment of the hit
        let hit_at = KICK_CUTS[k] + 0.04;
        // the striking limb: the hand or foot thrown furthest from the body
        let p = self.hero.posed(hit_at);
        let hip_s = cam.to_screen(p.pos(Bone::Hips));
        let foot = [Bone::HandL, Bone::HandR, Bone::FootL, Bone::FootR]
            .iter()
            .map(|b| cam.to_screen(p.pos(*b)))
            .max_by(|a, b| (*a - hip_s).length().total_cmp(&(*b - hip_s).length()))
            .unwrap_or(hip_s);
        let face =
            gcam.to_screen(self.demon.posed(hit_at).pos(Bone::Head) + Vec3::new(0.0, 0.1, 0.0));
        gcam.shift = foot - face + Vec2::new(-70.0, 0.0);
        let hit = since / 0.22;
        let split = if hit < 0.4 {
            18.0 * (1.0 - hit / 0.4)
        } else {
            0.0
        };
        gpu::rgb_split(
            fframes::svgr!(
                <g>
                    {bg(palette::YELLOW)}
                    {gpu::focus_lines(frame, Vec2::new(500.0, 650.0), 260.0, Color::hex(palette::INK), 160.0)}
                    {self.demon.draw(t, &gcam, &demon_paint())}
                    {gpu::aura(frame, Vec2::new(960.0, 560.0), Vec2::new(900.0, 1100.0), Color::hex("#FFF3B0").with_alpha(200), Color::hex("#F97316").with_alpha(170), 0.8, k as f32 + 0.5)}
                    {gpu::rim_light(self.hero.draw(t, &cam, &Paint::two_tone(palette::YELLOW)), "#FFE08A", Vec2::new(0.4, -0.9), 6.0)}
                    {gpu::burst(frame, foot, 300.0, hit, Color::hex("#FFFFFF"), Color::hex("#F472B6"), k as f32)}
                </g>
            ),
            split,
            Vec2::new(1.0, -0.2),
        )
    }
}

/// Black ink swirling through the frame, the hero smeared across it.
pub struct Swirl {
    hero: Actor,
}

impl Swirl {
    pub fn new(len: f32) -> Self {
        let sk = member(Who::Neogoose).skeleton();
        let hip = sk.proportions.hip_height;
        Swirl {
            hero: Actor::new(
                Who::Neogoose,
                motion(move |t| {
                    build(
                        &sk,
                        Body {
                            root: Vec3::new(3.0 - t * 6.0, hip + 0.5, 0.0),
                            yaw: -90.0,
                            hips: Vec3::new(0.0, 70.0, 0.0),
                            ..Default::default()
                        },
                        Arms {
                            left: Some(Vec3::new(0.1, 0.7, 0.1)),
                            ..Default::default()
                        },
                        None,
                    )
                }),
                look(Look {
                    face: false,
                    ..Default::default()
                }),
                len,
                2,
                Vec3::ZERO,
            ),
        }
    }
}

impl Shot for Swirl {
    fn render<'a>(&'a self, t: f32, len: f32, _f: &Frame, _c: &FFramesContext<'a, '_>) -> Svgr<'a> {
        let th = twos(t);
        let c = Vec2::new(960.0, 540.0);
        let mut ink = Path::new();
        for arm in 0..5 {
            let base = arm as f32 / 5.0 * TAU + th * 7.0;
            let pts: Vec<Vec2> = (0..18)
                .map(|i| {
                    let u = i as f32 / 17.0;
                    c + Vec2::from_angle(base + u * 3.2) * (60.0 + u * 1100.0 * ease_out(th / 0.5))
                })
                .collect();
            let widths: Vec<f32> = (0..18)
                .map(|i| 90.0 * (1.0 - i as f32 / 17.0) + 6.0)
                .collect();
            ink.smooth_closed(&shape::ribbon(&pts, &widths), 0.6);
        }
        let cam = Camera::new(
            Vec3::new(0.0, 1.4, 4.0),
            Vec3::new(0.0, 1.4, 0.0),
            34.0,
            W,
            H,
        );
        // motion smear: ghosts of the last drawings
        let ghosts: Vec<Svgr<'a>> = (1..5)
            .rev()
            .map(|k| {
                self.hero.draw(
                    (t - k as f32 * 0.04).max(0.0),
                    &cam,
                    &Paint::silhouette(palette::YELLOW_DEEP),
                )
            })
            .collect();
        let _ = len;
        fframes::svgr!(
            <g>
                {bg(palette::YELLOW)}
                {ghosts}
                {self.hero.draw_moving(t, &cam, &cam, &Paint::silhouette(palette::INK))}
                {fx::path(ink.build(), palette::INK)}
            </g>
        )
    }
}

// ---------------------------------------------------------------------------------------------

/// Two friends watch from the stands, worried, then turn: the goose crest on their backs.
pub struct Watch {
    a: Actor,
    b: Actor,
}

const WATCH_TURN: f32 = 0.55;

impl Watch {
    pub fn new(len: f32) -> Self {
        let turn = |x: f32, who: Who| {
            idle(who, move |t| {
                let k = smooth((t - WATCH_TURN) / 0.25);
                (
                    Body {
                        root: Vec3::new(x, 0.0, 0.0),
                        yaw: 180.0 * k - x * 20.0,
                        head: Vec3::new(0.0, 5.0, 0.0),
                        ..Default::default()
                    },
                    Arms {
                        left: Some(Vec3::new(0.06, 0.16, 0.3)),
                        right: Some(Vec3::new(-0.06, 0.2, 0.3)),
                        elbow_out: 0.8,
                        ..Default::default()
                    },
                )
            })
        };
        let looks = acting(
            Acting::new(Look {
                hand_l: HandShape::Fist,
                hand_r: HandShape::Relaxed,
                ..Default::default()
            })
            .then(0.0, |l| Look {
                mouth: Mouth::Frown,
                pupils: 0.7,
                ..expr::worried(l)
            })
            .then(0.35, |l| Look {
                gaze: Vec2::new(0.8, 0.0),
                ..l
            })
            .then(0.6, |l| Look {
                mouth: Mouth::Teeth(0.4),
                ..expr::determined(l)
            })
            .darts(190, 0.3, 0.2)
            .blinks(191, 1.2),
        );
        Watch {
            a: Actor::new(
                Who::Shreyas,
                turn(-0.33, Who::Shreyas),
                looks.clone(),
                len,
                2,
                Vec3::new(1.0, 0.0, 0.0),
            ),
            b: Actor::new(
                Who::Chshersh,
                turn(0.33, Who::Chshersh),
                looks,
                len,
                2,
                Vec3::new(1.0, 0.0, 0.0),
            ),
        }
    }
}

impl Shot for Watch {
    fn render<'a>(
        &'a self,
        t: f32,
        _len: f32,
        _f: &Frame,
        _c: &FFramesContext<'a, '_>,
    ) -> Svgr<'a> {
        let th = twos(t);
        let cam = Camera::new(
            Vec3::new(0.0, 1.45, 2.4),
            Vec3::new(0.0, 1.32, 0.0),
            32.0,
            W,
            H,
        );
        let back_cam = Camera::new(
            Vec3::new(0.0, 5.0, -12.0),
            Vec3::new(0.0, 4.0, 6.0),
            40.0,
            W,
            H,
        );
        let mut crests: Vec<Svgr<'a>> = Vec::new();
        if th > WATCH_TURN + 0.15 {
            for a in [&self.a, &self.b] {
                let posed = a.posed(t);
                let back = posed.attach(Bone::Chest, Vec3::new(0.0, 0.05, -0.2));
                let p = cam.project(back);
                crests.push(fx::path(
                    emblem::goose(p.p, 0.11 * p.scale, 1.0, 0.0).build(),
                    palette::YELLOW,
                ));
            }
        }
        fframes::svgr!(
            <g>
                {colosseum(&back_cam)}
                {stage::actors(&[&self.a, &self.b], t, &cam, &Paint::two_tone(palette::YELLOW))}
                {crests}
            </g>
        )
    }
}

// ---------------------------------------------------------------------------------------------

/// The charge: the hero dives across the arena, a white flash, and a magic circle opens under
/// the giant reaching for the last cream puff.
pub struct Charge {
    hero: Actor,
    demon: Actor,
}

const C_DIVE: f32 = 0.55;
const C_FLASH: f32 = 1.84;
/// The borrow checker zaps the giant's hand on the beat at 84.24 s.
const C_ZAP: f32 = 2.82;

impl Charge {
    pub fn new(len: f32) -> Self {
        let sk = member(Who::Neogoose).skeleton();
        let hero = Actor::new(
            Who::Neogoose,
            motion(move |t| {
                let x = 6.0 - (t - C_DIVE) * 9.0;
                build(
                    &sk,
                    Body {
                        root: Vec3::new(x, 1.0, 0.0),
                        yaw: -90.0,
                        hips: Vec3::new(0.0, 85.0, 0.0),
                        neck: Vec3::new(0.0, -40.0, 0.0),
                        head: Vec3::new(0.0, -30.0, 0.0),
                        ..Default::default()
                    },
                    Arms {
                        left: Some(Vec3::new(0.1, 0.74, 0.06)),
                        right: Some(Vec3::new(-0.08, 0.7, 0.12)),
                        elbow_out: 0.6,
                        ..Default::default()
                    },
                    None,
                )
            }),
            acting(
                Acting::new(Look {
                    hand_l: HandShape::Fist,
                    hand_r: HandShape::Open,
                    ..Default::default()
                })
                .then(0.0, |l| Look {
                    mouth: Mouth::Grin,
                    ..expr::determined(l)
                })
                .then(C_DIVE, expr::shout)
                .then(C_FLASH, |l| Look {
                    eyes: 0.35,
                    squint: 1.0,
                    mouth: Mouth::Teeth(1.0),
                    ..expr::determined(l)
                })
                .then(C_FLASH + 0.5, |l| Look {
                    sparkle: 1.0,
                    mouth: Mouth::Grin,
                    ..expr::determined(l)
                })
                .blinks(195, 2.5),
            ),
            len,
            2,
            Vec3::ZERO,
        );
        let demon = Actor::new(
            Who::Demon,
            idle(Who::Demon, |t| {
                // a mass of goo never holds still: it sways, breathes and wobbles as it
                // reaches greedily for the puff
                let k = fframes_anime::koma::hold(t, 24.0, 2);
                (
                    Body {
                        spine: Vec3::new(
                            8.0 * (k * 2.1).sin(),
                            35.0 + 5.0 * (k * 4.3).sin(),
                            6.0 * (k * 1.7).cos(),
                        ),
                        chest: Vec3::new(0.0, 3.0 * (k * 5.1).sin(), -4.0 * (k * 2.6).sin()),
                        head: Vec3::new(
                            10.0 * (k * 1.3).sin(),
                            -15.0 + 6.0 * (k * 3.7).sin(),
                            8.0 * (k * 2.9).sin(),
                        ),
                        ..Default::default()
                    },
                    Arms {
                        right: Some(Vec3::new(
                            -0.1 + 0.08 * (k * 3.3).sin(),
                            -0.4 + t * 0.05 + 0.05 * (k * 6.0).sin(),
                            0.7,
                        )),
                        right_palm: Some(Vec3::new(0.0, -1.0, 0.0)),
                        ..Default::default()
                    },
                )
            }),
            acting(
                Acting::new(Look {
                    accent_eyes: true,
                    hand_r: HandShape::Open,
                    ..Default::default()
                })
                .then(0.0, |l| Look {
                    lids: 0.4,
                    gaze: Vec2::new(0.0, -0.8),
                    mouth: Mouth::Open(0.6),
                    ..l
                })
                .then(C_FLASH, |l| Look {
                    lids: 0.0,
                    pupils: 0.3,
                    brows: 1.0,
                    gaze: Vec2::ZERO,
                    mouth: Mouth::Roar(1.0),
                    ..l
                }),
            ),
            len,
            2,
            Vec3::ZERO,
        );
        Charge { hero, demon }
    }
}

impl Shot for Charge {
    fn render<'a>(
        &'a self,
        t: f32,
        len: f32,
        frame: &Frame,
        _c: &FFramesContext<'a, '_>,
    ) -> Svgr<'a> {
        let th = twos(t);
        if th < C_DIVE {
            let cam = Camera::new(
                Vec3::new(0.0, 2.0, 14.0),
                Vec3::new(0.0, 4.0, 0.0),
                40.0,
                W,
                H,
            );
            let g = giant_in(&cam, Vec3::new(-1.5, 0.0, -1.0), 3.5);
            return fframes::svgr!(<g>{colosseum(&cam)}{self.demon.draw(t, &g, &demon_paint())}</g>);
        }
        if th < C_FLASH {
            let hx = 6.0 - (t - C_DIVE) * 9.0;
            let cam = Camera::new(
                Vec3::new(hx - 0.4, 1.4, 3.6),
                Vec3::new(hx - 0.4, 1.2, 0.0),
                36.0,
                W,
                H,
            );
            let floor = Camera::new(
                Vec3::new(hx * 0.2, 1.6, 14.0),
                Vec3::new(hx * 0.2, 1.6, 0.0),
                40.0,
                W,
                H,
            );
            let lines = fx::speed_lines(0.0, 40, (th * 24.0) as u32, th * 9000.0, 8.0);
            // a golden aura streaming off the diving hero
            let at = cam.to_screen(self.hero.posed(t).pos(Bone::Hips));
            let aura = gpu::aura_towards(
                frame,
                at + Vec2::new(120.0, -10.0),
                Vec2::new(620.0, 1200.0),
                Color::hex("#FFF7CC").with_alpha(220),
                Color::hex("#F59E0B").with_alpha(200),
                0.95,
                2.0,
                90.0,
            );
            return fframes::svgr!(<g>{colosseum(&floor)}{fx::path(lines.build(), "#C79A06")}{aura}{self.hero.draw_moving(t, &cam, &cam, &Paint::two_tone(palette::YELLOW))}</g>);
        }
        if th < C_FLASH + 0.09 {
            return fframes::svgr!(
                <g>
                    {bg(palette::WHITE)}
                    {gpu::burst(frame, Vec2::new(960.0, 540.0), 600.0, (t - C_FLASH) / 0.12, Color::hex("#FFF7CC"), Color::hex("#F59E0B"), 4.0)}
                </g>
            );
        }
        // the magic circle seen from above, the giant's hand reaching for the puff
        let u = (t - C_FLASH) / (len - C_FLASH);
        let c = Vec2::new(960.0, 560.0);
        let mut rings = Path::new();
        for (k, r) in [(0, 470.0), (1, 430.0), (2, 300.0), (3, 260.0), (4, 140.0)] {
            rings.circle(c, r * (0.9 + 0.1 * ease_out(u * 3.0)));
            let _ = k;
        }
        let rot = th * 0.8;
        let mut star = Path::new();
        let pts: Vec<Vec2> = (0..12)
            .map(|i| c + Vec2::from_angle(rot + i as f32 * TAU * 5.0 / 12.0) * 430.0)
            .collect();
        star.polyline(&pts).close();
        let runes: Vec<Svgr<'a>> = ["fn", "{}", "=>", "&'a", "mut", "impl", "::", "?", "dyn", "<T>", "async", "loop"]
            .iter()
            .enumerate()
            .map(|(i, s)| {
                let a = -rot * 1.3 + i as f32 / 12.0 * TAU;
                let p = c + Vec2::from_angle(a) * 365.0;
                let tf = format!("rotate({} {} {})", (a + PI / 2.0).to_degrees(), p.x, p.y);
                let s = (*s).to_owned();
                fframes::svgr!(<text x={p.x} y={p.y + 12.0} font-family={GOTHIC} font-size="34" text-anchor="middle" fill={palette::RED} transform={tf}>{s}</text>)
            })
            .collect();
        let hand = self.demon.posed(t).hand_tip(Side::Right);
        let mut g = Camera::new(Vec3::new(0.0, 6.0, 1.2), hand, 40.0, W, H);
        g.shift = Vec2::new(-100.0, -200.0);
        // the borrow checker: on beat 84.24 the circle refuses the giant's mutable borrow of the
        // puff, the hand is zapped back and the puff pops up out of the frame (into the hero's
        // hand in the next shot)
        let since = th - C_ZAP;
        let zapped = since >= 0.0;
        let hand_s = g.to_screen(hand);
        if zapped {
            let away = (hand_s - c).normalize_or(Vec2::NEG_Y);
            g.shift += away * 420.0 * (since / 0.17).min(1.0);
        }
        let puff_bob = (th * 8.0).sin() * 6.0;
        let puff_at = if zapped {
            c + Vec2::new(
                60.0 * since,
                puff_bob - 2600.0 * since + 1500.0 * since * since,
            )
        } else {
            c + Vec2::new(0.0, puff_bob)
        };
        let error = if zapped {
            let pop = if since < 1.0 / 12.0 { 1.08 } else { 1.0 };
            let tf = format!("translate(960 900) scale({pop}) translate(-960 -900)");
            fframes::svgr!(
                <g transform={tf}>
                    <rect x="150" y="770" width="1620" height="250" rx="16" fill="#120A0A" stroke={palette::RED} stroke-width="8" />
                    <text x="200" y="840" font-family={SANS} font-weight="900" font-size="50" fill="#FF4D4D">"error[E0502]"<tspan fill="#FFFFFF">": cannot borrow 'puff' as mutable"</tspan></text>
                    <text x="200" y="910" font-family={SANS} font-weight="700" font-size="40" fill="#8AB4FF">"  --> colosseum.rs:84:24"</text>
                    <text x="200" y="980" font-family={SANS} font-weight="700" font-size="40" fill="#FFD86B">"   ^^^^ the hero already owns it"</text>
                </g>
            )
        } else {
            Svgr::empty()
        };
        let zap = if zapped {
            gpu::burst(
                frame,
                hand_s,
                420.0,
                since / 0.2,
                Color::hex("#FFFFFF"),
                Color::hex("#FF4D4D"),
                13.0,
            )
        } else {
            Svgr::empty()
        };
        // the circle glows: a blurred copy of its lines under them, rays out of its middle
        let glow_rings = rings.build();
        let glow_star = star.build();
        fframes::svgr!(
            <g>
                {bg(palette::YELLOW)}
                {gpu::rays(frame, c, Color::hex("#FB923C").with_alpha(150), 34.0)}
                <defs>
                    <filter id="circleglow" x="-20%" y="-20%" width="140%" height="140%">
                        <feGaussianBlur stdDeviation="16" />
                    </filter>
                </defs>
                <g filter="url(#circleglow)" opacity="0.9">
                    <path d={glow_rings.clone()} fill="none" stroke="#FF6A3D" stroke-width="34" />
                    <path d={glow_star.clone()} fill="none" stroke="#FF6A3D" stroke-width="22" />
                </g>
                {fx::stroke(glow_rings, palette::RED, 10.0)}
                {fx::stroke(glow_star, palette::RED, 6.0)}
                {runes}
                {fx::puff(puff_at, 120.0, if zapped { since * 9.0 } else { 0.0 }, 1.0, Vec2::Y, 6.0)}
                {self.demon.draw(t, &g, &demon_paint())}
                {zap}
                {error}
            </g>
        )
    }
}

// ---------------------------------------------------------------------------------------------

/// The cream puff drops into the hero's hand.
pub struct Caught {
    hero: Actor,
    puff: crate::physics3d::Baked3,
}

/// Size of the cream puff caught at the end.
const CAUGHT_PUFF: f32 = 1.2;

fn caught_motion(landed: Option<f32>) -> crate::motion::MotionFn {
    idle(Who::Neogoose, move |t| {
        // the hand gives under the weight and comes back up
        let give = landed.map_or(0.0, |l| {
            let u = t - l;
            if u < 0.0 {
                0.0
            } else {
                0.04 * (u * 9.0).min(1.0) * (-u * 6.0).exp() * (1.0 + u * 4.0)
            }
        });
        (
            Body::default(),
            Arms {
                right: Some(Vec3::new(-0.05, 0.05 - give, 0.38)),
                right_palm: Some(Vec3::new(0.0, 1.0, -0.15)),
                right_fingers: Some(Vec3::new(-0.2, 0.15, 1.0)),
                ..Default::default()
            },
        )
    })
}

impl Caught {
    pub fn new(len: f32) -> Self {
        // drop it twice: once to find when it lands, then with the hand giving under it
        let fig = crate::render::figure(Who::Neogoose);
        let sk = member(Who::Neogoose).skeleton();
        let drop = |m: &crate::motion::MotionFn| {
            let p = sk.solve(&m(0.0));
            let palm = p.pos(Bone::HandR) + p.rot(Bone::HandR) * fig.palm(Side::Right);
            let toss = crate::physics3d::Toss {
                hull: crate::props::puff_hull(),
                scale: CAUGHT_PUFF,
                pos: palm + Vec3::new(0.02, 0.5, 0.0),
                rot: Quat::from_rotation_z(0.35) * Quat::from_rotation_x(-0.2),
                vel: Vec3::new(-0.03, -1.5, 0.0),
                spin: Vec3::new(1.0, 0.5, -1.6),
                restitution: 0.3,
                friction: 1.1,
                release: 0.0,
            };
            crate::physics3d::bake(
                &toss,
                &fig,
                &[(Side::Right, HandShape::Relaxed)],
                &|t| sk.solve(&m(t)),
                &[],
                0.0,
                len,
            )
        };
        let first = drop(&caught_motion(None));
        let m = caught_motion(first.landed);
        let puff = drop(&m);
        let hero = Actor::new(
            Who::Neogoose,
            m,
            acting(
                Acting::new(Look {
                    hand_r: HandShape::Relaxed,
                    ..Default::default()
                })
                .then(0.0, |l| Look {
                    gaze: Vec2::new(0.0, 0.9),
                    ..expr::surprised(l)
                })
                .then(0.4, |l| Look {
                    gaze: Vec2::new(0.0, -0.6),
                    sparkle: 1.0,
                    ..expr::happy(l)
                })
                .blinks(197, 2.0),
            ),
            len,
            2,
            Vec3::ZERO,
        );
        Caught { hero, puff }
    }
}

impl Shot for Caught {
    fn para(&self) -> f32 {
        0.5
    }

    fn render<'a>(
        &'a self,
        t: f32,
        _len: f32,
        _f: &Frame,
        _c: &FFramesContext<'a, '_>,
    ) -> Svgr<'a> {
        let hand = self.hero.posed(0.0).pos(Bone::HandR);
        let mut cam = Camera::orbit(hand, 0.75, -25.0, 25.0, 30.0, W, H);
        // once it has landed, a tilt up (a 2D pan of the cel) to his face lighting up
        let tilt = smooth((t - 0.45) / 0.4);
        cam.shift = Vec2::new(0.0, 230.0 + 560.0 * tilt);
        let held = self.hero.held(t);
        let puff = crate::render::Placed::new(
            crate::props::cream_puff(),
            self.puff.affine(held, CAUGHT_PUFF),
            held,
        );
        fframes::svgr!(
            <g>
                {bg(palette::YELLOW)}
                {crate::render::draw_with(&[(&self.hero, Paint::two_tone(palette::YELLOW))], &[puff], t, &cam)}
            </g>
        )
    }
}

/// The hero looks at it... and opens wide.
pub struct Bite {
    hero: Actor,
    puff: crate::physics3d::Baked3,
}

/// The bite lands on the onset at 86.53 s.
const CHOMP: f32 = 0.99;

/// How far the hand has brought the puff up to the mouth, drawing by drawing.
fn to_mouth(t: f32) -> f32 {
    match t {
        t if t < 0.45 => 0.0,
        t if t < 0.54 => 0.3,
        t if t < 0.62 => 0.7,
        t if t < 0.7 => 0.95,
        _ => 1.0,
    }
}

impl Bite {
    pub fn new(len: f32) -> Self {
        let bsk = member(Who::Neogoose).skeleton();
        let hs = fframes_anime::toon::head_scale(&bsk.proportions);
        let hip = bsk.proportions.hip_height;
        let m = motion(move |t| {
            use crate::procedural::{chest_point, hand_facing, reach};
            let k = to_mouth(t);
            // the chin comes down to meet it, the body leans in; the head rocks back with the bite
            let nod = if t >= CHOMP { 2.0 } else { 10.0 * k };
            let mut pose = build(
                &bsk,
                Body {
                    root: Vec3::new(0.0, hip, 0.0),
                    chest: Vec3::new(0.0, 8.0 * k, 0.0),
                    head: Vec3::new(-12.0 + 8.0 * k, 8.0 + nod, 0.0),
                    ..Default::default()
                },
                Arms::default(),
                None,
            );
            let solved = bsk.solve(&pose);
            let mouth = solved.attach(Bone::Head, Vec3::new(0.0, 0.046, 0.09) * hs);
            let start = chest_point(&bsk, &pose, Vec3::new(-0.17, 0.11, 0.33));
            // at the mouth: the palm just under and in front of the lips
            let end = mouth + Vec3::new(0.02, -0.13, 0.04);
            reach(&bsk, &mut pose, Side::Right, start.lerp(end, k), 0.6);
            let fingers = Vec3::new(-0.35, 0.1, 1.0).lerp(Vec3::new(0.15, 0.25, 1.0), k);
            let palm = Vec3::new(0.0, 1.0, -0.1).lerp(Vec3::new(0.0, 1.0, -0.4), k);
            pose.rot[Bone::HandR.index()] = hand_facing(Side::Right, fingers, palm);
            pose
        });
        let fig = crate::render::figure(Who::Neogoose);
        let sk = member(Who::Neogoose).skeleton();
        let puff = crate::physics3d::in_palm(
            &fig,
            &|t| sk.solve(&m(t)),
            Side::Right,
            HandShape::Relaxed,
            crate::props::puff_hull(),
            1.1,
            0.6,
            len,
        );
        let face = Acting::new(Look {
            hand_r: HandShape::Relaxed,
            ..Default::default()
        })
        // admiring it
        .then(0.0, |l| Look {
            gaze: Vec2::new(-0.3, -0.7),
            lids: 0.3,
            mouth: Mouth::Smirk(1.0),
            ..l
        })
        .then(0.3, |l| Look {
            gaze: Vec2::new(-0.3, -0.7),
            sparkle: 1.0,
            blush: 0.6,
            ..expr::happy(l)
        })
        // up it comes, the mouth opening wider than the face
        .then(0.62, |l| Look {
            mouth: Mouth::Open(0.7),
            pupils: 0.6,
            gaze: Vec2::new(-0.2, -0.5),
            ..expr::excited(l)
        })
        .then(0.82, |l| Look {
            mouth: Mouth::Roar(1.0),
            eyes: 1.0,
            pupils: 0.5,
            ..l
        })
        // chomp: eyes shut in bliss, chewing
        .then(CHOMP, expr::bliss)
        .blink_at(0.2);
        let looks = acting(expr::chew(face, CHOMP + 0.08, len, 0.09));
        let hero = Actor::new(Who::Neogoose, m, looks, len, 2, Vec3::ZERO);
        Bite { hero, puff }
    }
}

impl Shot for Bite {
    fn para(&self) -> f32 {
        0.5
    }

    fn render<'a>(
        &'a self,
        t: f32,
        _len: f32,
        frame: &Frame,
        _c: &FFramesContext<'a, '_>,
    ) -> Svgr<'a> {
        let th = twos(t);
        let rest = self.hero.posed(0.0);
        let head = rest.pos(Bone::Head) + Vec3::new(0.0, 0.05, 0.0);
        // locked: the face up on the right, the puff in the palm below it on the left
        let mut cam = Camera::orbit(head, 1.12, 15.0, 0.0, 30.0, W, H);
        cam.shift = Vec2::new(230.0, 40.0);
        let held = self.hero.held(t);
        let mut props = Vec::new();
        if held < CHOMP {
            // settled in the palm by the physics, then carried by the hand
            use fframes_anime::glam::Affine3A;
            let hand = |p: &fframes_anime::rig::Posed| {
                Affine3A::from_rotation_translation(p.rot(Bone::HandR), p.pos(Bone::HandR))
            };
            let settle = 0.4;
            let rel = hand(&self.hero.posed(settle)).inverse() * self.puff.affine(settle, 1.1);
            let at = if held < settle {
                self.puff.affine(held, 1.1)
            } else {
                hand(&self.hero.posed(t)) * rel
            };
            props.push(crate::render::Placed::new(
                crate::props::cream_puff(),
                at,
                held,
            ));
        }
        let drawn = crate::render::draw_with(
            &[(&self.hero, Paint::two_tone(palette::YELLOW))],
            &props,
            t,
            &cam,
        );
        if th < CHOMP {
            return fframes::svgr!(<g>{bg(palette::YELLOW)}{drawn}</g>);
        }
        // the chomp: cream splats off the mouth, focus lines, パクッ
        let since = th - CHOMP;
        let hs = fframes_anime::toon::head_scale(&self.hero.skeleton.proportions);
        let mouth = cam.to_screen(
            self.hero
                .posed(t)
                .attach(Bone::Head, Vec3::new(0.0, 0.046, 0.09) * hs),
        );
        let mut splats = Path::new();
        for i in 0..7u32 {
            let a = -2.6 + 2.2 * hash2(1201, i);
            let v = Vec2::from_angle(a) * (900.0 + 700.0 * hash2(1202, i));
            let p = mouth + v * since + Vec2::new(0.0, 2400.0) * since * since;
            let dir = (v + Vec2::new(0.0, 4800.0 * since)).normalize_or(Vec2::Y);
            let side = dir.perp();
            let r = 16.0 + 14.0 * hash2(1203, i);
            splats.smooth_closed(
                &[p + dir * r * 2.2, p + side * r, p - dir * r, p - side * r],
                1.0,
            );
        }
        let pop = match (since * 12.0).floor() as i32 {
            0 => 1.35,
            1 => 0.95,
            _ => 1.0,
        };
        let word = format!(
            "translate({:.1} {:.1}) rotate(-10) scale({pop:.2})",
            mouth.x - 330.0,
            mouth.y - 170.0
        );
        fframes::svgr!(
            <g>
                {bg(palette::YELLOW)}
                {gpu::focus_lines(frame, mouth, 380.0, Color::hex(palette::INK), 140.0)}
                {drawn}
                <path d={splats.build()} fill="#FFF2D2" stroke={palette::INK} stroke-width="4" />
                <g transform={word}>
                    <text x="0" y="0" font-family={GOTHIC} font-size="170" text-anchor="middle" fill="none" stroke={palette::INK} stroke-width="22" stroke-linejoin="round">"パクッ"</text>
                    <text x="0" y="0" font-family={GOTHIC} font-size="170" text-anchor="middle" fill={palette::TITLE_RED} stroke={palette::WHITE} stroke-width="4">"パクッ"</text>
                </g>
            </g>
        )
    }
}
