//! Typography shots: the opening burst with the goose crest, the giant blackletter title split
//! over four cuts with a cream puff bouncing through, the lyric cards and the title logo.

use fframes::{FFramesContext, Frame, Svgr};
use fframes_anime::glam::Vec2;

use crate::fx::{self, bg, ease_out, hash2, puff, spring, twos};
use crate::physics2d::{BallSetup, BallState, bake_ball};
use crate::{Shot, emblem, palette};

pub const BLACKLETTER: &str = "UnifrakturCook";
pub const LYRIC: &str = "Pirata One";
pub const GOTHIC: &str = "Dela Gothic One";
pub const SANS: &str = "Zen Kaku Gothic New";

/// 0..2 s: black, black shards burst open from the corner onto yellow, the goose spreads its
/// wings.
pub struct Open;

impl Shot for Open {
    fn para(&self) -> f32 {
        0e+00
    }

    fn render<'a>(
        &'a self,
        t: f32,
        _len: f32,
        _f: &Frame,
        _c: &FFramesContext<'a, '_>,
    ) -> Svgr<'a> {
        let th = twos(t);
        // the drum roll: a slash of light on each beat, the crest slams in on the last one
        const BEATS: [f32; 3] = [0.58, 0.975, 1.37];
        const SLAM: f32 = 1.74;
        if th < BEATS[0] {
            return bg(palette::INK);
        }
        let hits = BEATS.iter().filter(|b| th >= **b).count();
        let since = th - BEATS[hits - 1];
        // each slash snaps a third further open, overshooting on its first drawing
        let open = hits as f32 / 3.0 + if since < 1.0 / 12.0 { 0.08 } else { 0.0 };
        // after the slam the shards wheel out of the frame over four drawings
        let out = if th >= SLAM {
            ((th - SLAM) / (4.0 / 12.0)).min(1.0)
        } else {
            0.0
        };
        let corner = Vec2::new(1880.0, 1150.0);
        let shards = fx::sunburst(
            corner,
            9,
            -2.2 + open * 0.9 + out * 1.6,
            120.0 * (1.0 - open),
            2600.0,
            (0.95 - open * 0.72) * (1.0 - out),
        );
        let goose_t = th - SLAM;
        let mut layers: Vec<Svgr<'a>> = vec![bg(palette::YELLOW)];
        if goose_t >= 0.0 {
            // rays behind everything, then the crest with an overshooting wing spread
            let rays = fx::sunburst(
                Vec2::new(960.0, 560.0),
                28,
                goose_t * 0.35,
                260.0,
                1500.0,
                0.42,
            );
            layers.push(fx::path(rays.build(), palette::YELLOW_DEEP));
        }
        if out < 1.0 {
            layers.push(fx::path(shards.build(), palette::INK));
        }
        if goose_t >= 0.0 {
            let spread = spring(goose_t, 2.2, 5.5).clamp(0.0, 1.08);
            let flap = ((spring(goose_t, 2.2, 5.5) - 1.0) * 3.0).clamp(-1.0, 1.0);
            let scale = 400.0 + ease_out(goose_t / 0.5) * 40.0;
            let g = emblem::goose(Vec2::new(960.0, 560.0), scale, spread, flap);
            layers.push(fx::path(g.build(), palette::INK));
            // one white drawing as it lands
            if goose_t < 1.0 / 12.0 {
                layers.push(fframes::svgr!(<rect x="0" y="0" width="1920" height="1080" fill="#FFFFFF" opacity="0.75" />));
            }
        }
        fframes::svgr!(<g>{layers}</g>)
    }
}

/// Two giant blackletter letters with a cream puff bouncing through.
pub struct Letters {
    text: &'static str,
    radius: f32,
    puff: Vec<BallState>,
    puff_from: f32,
    variant: u32,
}

impl Letters {
    pub fn new(text: &'static str, len: f32, variant: u32) -> Self {
        let (setup, from) = match variant {
            // enters from the right edge, bounces off the floor
            0 => (
                BallSetup {
                    pos: Vec2::new(2200.0, 700.0),
                    vel: Vec2::new(-1500.0, -900.0),
                    radius: 210.0,
                    spin: -3.0,
                    gravity: 4200.0,
                    restitution: 0.62,
                    bounds: [-500.0, -2000.0, f32::INFINITY, 1010.0],
                },
                0.85,
            ),
            // drops from the top and bounces on the letters
            1 => (
                BallSetup {
                    pos: Vec2::new(1450.0, -260.0),
                    vel: Vec2::new(-260.0, 300.0),
                    radius: 190.0,
                    spin: 2.0,
                    gravity: 4600.0,
                    restitution: 0.7,
                    bounds: [-500.0, -2000.0, 2500.0, 1010.0],
                },
                0.15,
            ),
            // hangs huge next to the letter and wobbles
            2 => (
                BallSetup {
                    pos: Vec2::new(1380.0, 380.0),
                    vel: Vec2::new(-200.0, -1200.0),
                    radius: 260.0,
                    spin: 0.8,
                    gravity: 3800.0,
                    restitution: 0.55,
                    bounds: [900.0, -2000.0, 2050.0, 1000.0],
                },
                0.0,
            ),
            _ => (
                BallSetup {
                    pos: Vec2::new(420.0, 300.0),
                    vel: Vec2::new(300.0, -900.0),
                    radius: 250.0,
                    spin: -0.7,
                    gravity: 3800.0,
                    restitution: 0.55,
                    bounds: [-130.0, -2000.0, 1000.0, 980.0],
                },
                0.0,
            ),
        };
        // the puff bounces off the top of the letters (their cap height at this size)
        let (size, base) = letter_size(text);
        let rects = [[250.0, base - 0.66 * size, 1670.0, 1300.0]];
        let puff = bake_ball(
            setup,
            if variant == 1 { &rects } else { &[] },
            len + 0.2,
            24.0,
        );
        Letters {
            text,
            radius: setup.radius,
            puff,
            puff_from: from,
            variant,
        }
    }
}

/// Font size and baseline of the title letters: huge, bleeding off the frame edges.
fn letter_size(text: &str) -> (f32, f32) {
    if text.chars().count() == 1 {
        (1600.0, 1200.0)
    } else {
        (1380.0, 1150.0)
    }
}

impl Shot for Letters {
    fn para(&self) -> f32 {
        0e+00
    }

    fn render<'a>(&'a self, t: f32, len: f32, _f: &Frame, _c: &FFramesContext<'a, '_>) -> Svgr<'a> {
        let th = twos(t);
        // the letters drop in over two drawings with an overshoot, then a slow push in
        let drop = match (th * 12.0).floor() as i32 {
            0 => -150.0,
            1 => 28.0,
            _ => 0.0,
        };
        let scale = 1.0 + 0.05 * (th / len);
        let (size, y) = letter_size(self.text);
        let x = match self.variant {
            2 => 640.0,
            3 => 1300.0,
            _ => 960.0,
        };
        let transform = format!(
            "translate({x} {y}) scale({scale}) translate({} {})",
            -x,
            -y + drop
        );
        // a blackletter S reads as a G: that one letter in the plainer face
        let glyphs = |fill: &'static str, dx: f32, dy: f32| -> Svgr<'a> {
            let spans: Vec<Svgr<'a>> = self
                .text
                .chars()
                .map(|c| {
                    let font = if c == 'S' { LYRIC } else { BLACKLETTER };
                    let ch = c.to_string();
                    fframes::svgr!(<tspan font-family={font}>{ch}</tspan>)
                })
                .collect();
            fframes::svgr!(
                <text x={x + dx} y={y + dy} font-weight="700" font-size={size} text-anchor="middle" fill={fill}>{spans}</text>
            )
        };
        let mut layers: Vec<Svgr<'a>> = vec![bg(palette::YELLOW)];
        // an inline like a poster title: an ink drop shadow, a yellow gap, the ink letters
        let behind = fframes::svgr!(
            <g transform={transform.clone()}>
                {glyphs(palette::INK, 28.0, 22.0)}
                {glyphs(palette::YELLOW, 14.0, 11.0)}
            </g>
        );
        layers.push(behind);
        // the puff passes between the shadow and the letters
        if th >= self.puff_from {
            let s = BallState::at(&self.puff, th - self.puff_from, 24.0);
            layers.push(puff(s.pos, self.radius, s.rot, s.squash, s.normal, 7.0));
        }
        layers.push(fframes::svgr!(<g transform={transform}>{glyphs(palette::INK, 0.0, 0.0)}</g>));
        fframes::svgr!(<g>{layers}</g>)
    }
}

/// A full-screen lyric card.
pub struct LyricCard {
    pub color: &'static str,
}

impl Shot for LyricCard {
    fn para(&self) -> f32 {
        0e+00
    }

    fn render<'a>(
        &'a self,
        t: f32,
        _len: f32,
        _f: &Frame,
        _c: &FFramesContext<'a, '_>,
    ) -> Svgr<'a> {
        let th = twos(t);
        let lines = [
            "Bling-Bang-Bang,",
            "Bling-Bang-Bang,",
            "Bling-Bang-Bang-Born",
        ];
        let rows: Vec<Svgr<'a>> = lines
            .iter()
            .enumerate()
            .map(|(i, l)| {
                // the first line is there on the cut, the next ones slam in a drawing apart
                let at = i as f32 / 12.0;
                if th < at {
                    return Svgr::empty();
                }
                let slam = if th - at < 1.0 / 12.0 { 26.0 } else { 0.0 };
                let y = 380.0 + i as f32 * 230.0;
                let x = 70.0 - slam;
                let s = (*l).to_owned();
                fframes::svgr!(<text x={x} y={y} font-family={LYRIC} font-size="215" fill={self.color}>{s}</text>)
            })
            .collect();
        fframes::svgr!(<g>{bg(palette::INK)}{rows}</g>)
    }
}

/// The title logo: katakana over a slammed red blackletter NEOGOOSE between arrows, and the arc
/// subtitle.
pub struct Logo;

impl Shot for Logo {
    fn para(&self) -> f32 {
        0e+00
    }

    fn render<'a>(&'a self, t: f32, _len: f32, f: &Frame, _c: &FFramesContext<'a, '_>) -> Svgr<'a> {
        let th = twos(t);
        // slams in from huge (over the first 3 drawings), then breathes
        let slam = spring(th, 1.6, 7.0);
        let scale = 1.0 + (1.0 - slam) * 2.2;
        let shake = if th < 0.4 {
            (hash2(7, (th * 24.0) as u32) - 0.5) * 30.0 * (1.0 - th / 0.4)
        } else {
            0.0
        };
        let transform = format!(
            "translate({} 520) scale({scale}) translate(-960 -520)",
            960.0 + shake
        );
        let kata = ease_out((th - 0.25) / 0.2);
        let sub = ease_out((th - 0.45) / 0.3);
        let clip_w = (1100.0 * sub).max(1.0);
        let arrow_l = format!(
            "M{} 465 L{} 520 L{} 575 Z",
            250.0 - 40.0 * kata,
            320.0 - 40.0 * kata,
            250.0 - 40.0 * kata
        );
        let arrow_r = format!(
            "M{} 465 L{} 520 L{} 575 Z",
            1670.0 + 40.0 * kata,
            1600.0 + 40.0 * kata,
            1670.0 + 40.0 * kata
        );
        // GPU finish: light fanning out behind the title, a glint across it once it lands, and
        // sparkles twinkling around it
        let rays = crate::gpu::rays(
            f,
            Vec2::new(960.0, 520.0),
            fframes::Color::hex("#FFF7CC").with_alpha((80.0 * slam.clamp(0.0, 1.0)) as u8),
            20.0,
        );
        let glint = crate::gpu::shine(
            f,
            (180.0, 330.0, 1560.0, 330.0),
            (th - 0.5) / 0.7,
            fframes::Color::hex("#FFFFFF").with_alpha(200),
        );
        let mut twinkles = fframes_anime::shape::Path::new();
        for i in 0..4 {
            let phase = (th * 2.5 + hash2(31, i) * 3.0).fract();
            let p = Vec2::new(260.0 + hash2(32, i) * 1400.0, 340.0 + hash2(33, i) * 380.0);
            twinkles.append(&crate::fx::sparkle(
                p,
                34.0 * (1.0 - (phase - 0.5).abs() * 2.0),
                0.3,
            ));
        }
        let twinkle_op = ease_out((th - 0.4) / 0.3);
        fframes::svgr!(
            <g>
                {bg(palette::YELLOW)}
                {rays}
                <g transform={transform}>
                    <text x="972" y="602" font-family={BLACKLETTER} font-weight="700" font-size="330" text-anchor="middle" fill={palette::INK} stroke={palette::INK} stroke-width="34" stroke-linejoin="round">"Neogoose"</text>
                    <text x="960" y="590" font-family={BLACKLETTER} font-weight="700" font-size="330" text-anchor="middle" fill={palette::WHITE} stroke={palette::INK} stroke-width="30" stroke-linejoin="round">"Neogoose"</text>
                    <text x="960" y="590" font-family={BLACKLETTER} font-weight="700" font-size="330" text-anchor="middle" fill={palette::TITLE_RED} stroke={palette::WHITE} stroke-width="12" stroke-linejoin="round">"Neogoose"</text>
                    <text x="960" y="590" font-family={BLACKLETTER} font-weight="700" font-size="330" text-anchor="middle" fill={palette::TITLE_RED}>"Neogoose"</text>
                </g>
                <path d={arrow_l} fill={palette::TITLE_RED} stroke={palette::INK} stroke-width="6" opacity={kata} />
                <path d={arrow_r} fill={palette::TITLE_RED} stroke={palette::INK} stroke-width="6" opacity={kata} />
                <text x="420" y={330.0 + (1.0 - kata) * 40.0} font-family={GOTHIC} font-size="110" fill={palette::TITLE_RED} stroke={palette::INK} stroke-width="6" stroke-linejoin="round" opacity={kata}>"ネオグース"</text>
                <clipPath id="logosub"><rect x="560" y="640" width={clip_w} height="140" /></clipPath>
                <text x="1440" y="740" clip-path="url(#logosub)" font-family={GOTHIC} font-size="86" text-anchor="end" fill={palette::INK}>"爆速オープンソース編"</text>
                {glint}
                <path d={twinkles.build()} fill={palette::WHITE} stroke={palette::INK} stroke-width="3" opacity={twinkle_op} />
            </g>
        )
    }
}

/// A text card on a colour with a line-art singer (the "now singin'" beat).
pub struct Compilin {
    singer: crate::actor::Actor,
}

impl Compilin {
    pub fn new(len: f32) -> Self {
        use crate::cast::{Who, member};
        use crate::procedural::{Arms, Body, build, motion};
        use fframes_anime::glam::Vec3;
        use fframes_anime::toon::Look;
        let sk = member(Who::Neogoose).skeleton();
        let hip = sk.proportions.hip_height;
        let m = motion(move |t| {
            let sway = (t * 5.0).sin() * 4.0;
            build(
                &sk,
                Body {
                    root: Vec3::new(0.0, hip, 0.0),
                    yaw: -40.0,
                    head: Vec3::new(sway, -28.0, 6.0),
                    chest: Vec3::new(0.0, -8.0, 0.0),
                    ..Default::default()
                },
                Arms::default(),
                None,
            )
        });
        Compilin {
            singer: crate::actor::Actor::new(
                Who::Neogoose,
                m,
                // "now compilin'", eyes shut, singing it out
                crate::actor::acting(
                    crate::expr::Acting::new(Look {
                        eyes: 0.0,
                        brow_angle: -0.7,
                        brows: 0.4,
                        ..Default::default()
                    })
                    .sing(&[0.04, 0.2, 0.33, 0.47], "aoii", 0.12)
                    .then(0.62, |l| Look {
                        eyes: 0.6,
                        brow_angle: 0.0,
                        ..l
                    }),
                ),
                len,
                2,
                Vec3::new(-2.0, 0.0, 0.0),
            ),
        }
    }
}

impl Shot for Compilin {
    fn render<'a>(
        &'a self,
        t: f32,
        _len: f32,
        _f: &Frame,
        _c: &FFramesContext<'a, '_>,
    ) -> Svgr<'a> {
        use fframes_anime::camera::Camera;
        use fframes_anime::glam::Vec3;
        let head =
            self.singer.posed(t).pos(fframes_anime::rig::Bone::Head) + Vec3::new(0.0, 0.08, 0.0);
        let mut cam = Camera::orbit(head, 0.62, -75.0, -10.0, 30.0, crate::W, crate::H);
        cam.shift = Vec2::new(430.0, 60.0);
        // a coloured cel like the reference's singer: violet skin, the robe a shade darker,
        // ink lines, the hair and beard in coral
        let mut lines = crate::palette::Paint::two_tone("#B79CFF").with_suit("#21095E");
        lines.line = "#160640";
        lines.hair = Some("#FF7A6B");
        lines.beard = Some("#FF7A6B");
        lines.shade = 0.18;
        let singer = self.singer.draw(t, &cam, &lines);
        let th = twos(t);
        let words = "now compilin\u{2019}";
        let shown: String = words
            .chars()
            .take(((th / 0.4) * words.chars().count() as f32).ceil() as usize)
            .collect();
        // a terminal cursor blinking on twos
        let cursor = if (th * 4.0).floor() as i32 % 2 == 0 {
            1.0
        } else {
            0.0
        };
        let notes: Vec<Svgr<'a>> = (0..14)
            .map(|i| {
                let born = i as f32 * 0.05;
                let age = th - born;
                let x = 1150.0 + hash2(3, i) * 600.0 + age * 120.0;
                let y = 760.0 - age * 600.0 - hash2(4, i) * 200.0;
                let r = 8.0 + hash2(5, i) * 10.0;
                let op = if age > 0.0 {
                    (1.0 - age / 0.7).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                fframes::svgr!(<circle cx={x} cy={y} r={r} fill="none" stroke="#FFD86B" stroke-width="4" opacity={op} />)
            })
            .collect();
        fframes::svgr!(
            <g>
                {bg(palette::PURPLE)}
                {singer}
                {notes}
                <text x="120" y="600" font-family={LYRIC} font-size="200" fill="#FF5A60" stroke={palette::INK} stroke-width="6" paint-order="stroke">{shown}<tspan fill="#FFD86B" opacity={cursor}>"_"</tspan></text>
            </g>
        )
    }
}

/// A solid frame.
pub struct Solid(pub &'static str);

impl Shot for Solid {
    fn para(&self) -> f32 {
        0e+00
    }

    fn render<'a>(
        &'a self,
        _t: f32,
        _len: f32,
        _f: &Frame,
        _c: &FFramesContext<'a, '_>,
    ) -> Svgr<'a> {
        bg(self.0)
    }
}
