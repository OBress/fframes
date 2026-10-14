//! The four house banners between the title letters: one per project, like the houses of the
//! magic academy.

use fframes::{FFramesContext, Frame, Svgr};
use fframes_anime::glam::Vec2;
use fframes_anime::shape::Path;

use crate::fx::{self, ease_out, hash2, twos};
use crate::shots::title::{BLACKLETTER, GOTHIC, LYRIC, SANS};
use crate::{Shot, emblem, palette};

#[derive(Clone, Copy)]
pub enum House {
    Fff,
    Fframes,
    Odiff,
    Zlob,
}

pub struct Banner {
    pub house: House,
}

struct Style {
    band: &'static str,
    tint: &'static str,
    name: &'static str,
    tagline: &'static str,
    stars: &'static str,
}

fn style(h: House) -> Style {
    match h {
        House::Fff => Style {
            band: palette::RED,
            tint: "#DE7F72",
            name: "fff",
            tagline: "最速のファイル検索",
            stars: "★ 11,068",
        },
        House::Fframes => Style {
            band: palette::BLUE,
            tint: "#9097DA",
            name: "fframes",
            tagline: "爆速ビデオレンダリング",
            stars: "★ 2,602",
        },
        House::Odiff => Style {
            band: palette::OLIVE,
            tint: "#EADF94",
            name: "odiff",
            tagline: "SIMD 画像比較",
            stars: "★ 3,230",
        },
        House::Zlob => Style {
            band: palette::MOSS,
            tint: "#8CC181",
            name: "zlob",
            tagline: "爆速グロブ",
            stars: "★ 230",
        },
    }
}

impl Shot for Banner {
    fn para(&self) -> f32 {
        0e+00
    }

    fn render<'a>(&'a self, t: f32, len: f32, _f: &Frame, _c: &FFramesContext<'a, '_>) -> Svgr<'a> {
        let th = twos(t);
        let st = style(self.house);
        let gold = "#E9B949";
        let (top, h) = (150.0, 780.0);
        let push = 1.0 + 0.04 * (t / len);
        let c = Vec2::new(960.0, top + h * 0.5);
        let crest_scale = 330.0 * push;
        let crest = match self.house {
            House::Fff => emblem::search(c, crest_scale),
            House::Fframes => emblem::film(c, crest_scale),
            House::Odiff => emblem::diff(c, crest_scale),
            House::Zlob => emblem::glob(c, crest_scale),
        };
        let ring = emblem::seal_ring(c, 350.0 * push);
        let frame = fx::ornate_frame(30.0, top, 1860.0, h, 30.0);
        // sunbursts at both ends of the band
        let mut rays = Path::new();
        for x in [100.0, 1820.0] {
            rays.append(&fx::sunburst(
                Vec2::new(x, c.y),
                18,
                th * 0.4,
                28.0,
                150.0,
                0.35,
            ));
        }
        // the band slides in from the side on the first drawings
        let slide = (1.0 - ease_out(t / 0.16)) * 220.0;
        let band_tf = format!("translate({slide} 0)");
        let name_x = 420.0 + (1.0 - ease_out(t / 0.25)) * -80.0;
        let tag_op = ease_out((th - 0.15) / 0.2);
        let mut sparkles = Path::new();
        for i in 0..6 {
            let phase = (th * 3.0 + hash2(9, i) * 3.0).fract();
            let p = Vec2::new(
                500.0 + hash2(1, i) * 920.0,
                top + 80.0 + hash2(2, i) * (h - 160.0),
            );
            sparkles.append(&fx::sparkle(
                p,
                30.0 * (1.0 - (phase - 0.5).abs() * 2.0),
                0.3,
            ));
        }
        let mut layers: Vec<Svgr<'a>> = vec![
            fx::bg(palette::INK),
            fframes::svgr!(<rect x="-20" y={top} width="1960" height={h} fill={st.band} />),
            fx::stroke(frame.build(), gold, 5.0),
            fx::path(rays.build(), gold),
            fframes::svgr!(<path d={ring.build()} fill="none" stroke={gold} stroke-width="9" opacity="0.8" />),
        ];
        if let House::Zlob = self.house {
            // the coin is zlob's crest
            layers.push(coin(th, c));
        } else {
            layers.push(fframes::svgr!(<path d={crest.build()} fill={st.tint} fill-rule="nonzero" opacity="0.55" />));
        }
        layers.push(fx::path(sparkles.build(), palette::WHITE));
        // the name fits the left of the band; a blackletter z reads as a 3
        let size = (1250.0 / st.name.chars().count() as f32).min(250.0);
        let spans = |fill: &'static str| -> Vec<Svgr<'a>> {
            st.name
                .chars()
                .map(|ch| {
                    let font = if ch == 'z' { LYRIC } else { BLACKLETTER };
                    let ch = ch.to_string();
                    fframes::svgr!(<tspan font-family={font} fill={fill}>{ch}</tspan>)
                })
                .collect()
        };
        let tagline = st.tagline.to_owned();
        let stars = st.stars.to_owned();
        layers.push(fframes::svgr!(
            <g>
                <text x={name_x + 10.0} y={c.y + 80.0} font-weight="700" font-size={size} text-anchor="middle">{spans(palette::INK)}</text>
                <text x={name_x} y={c.y + 70.0} font-weight="700" font-size={size} text-anchor="middle" stroke={palette::INK} stroke-width="10" stroke-linejoin="round">{spans(palette::INK)}</text>
                <text x={name_x} y={c.y + 70.0} font-weight="700" font-size={size} text-anchor="middle">{spans(palette::WHITE)}</text>
                <g opacity={tag_op}>
                    <text x="1500" y={c.y - 20.0} font-family={GOTHIC} font-size="62" text-anchor="middle" fill={palette::INK} stroke={palette::INK} stroke-width="12" stroke-linejoin="round">{tagline.clone()}</text>
                    <text x="1500" y={c.y - 20.0} font-family={GOTHIC} font-size="62" text-anchor="middle" fill={palette::WHITE}>{tagline}</text>
                    <text x="1500" y={c.y + 90.0} font-family={SANS} font-weight="900" font-size="72" text-anchor="middle" fill={palette::INK} stroke={palette::INK} stroke-width="12" stroke-linejoin="round">{stars.clone()}</text>
                    <text x="1500" y={c.y + 90.0} font-family={SANS} font-weight="900" font-size="72" text-anchor="middle" fill={gold}>{stars}</text>
                </g>
            </g>
        ));
        // one white glint across the band as it lands, two drawings
        let g = ((th - 0.17) / 0.17).clamp(0.0, 1.0);
        if th > 0.17 && th < 0.34 {
            let x = -400.0 + g * 2600.0;
            let glint = format!(
                "M{:.0} {top} L{:.0} {top} L{:.0} {} L{:.0} {} Z",
                x,
                x + 140.0,
                x - 160.0,
                top + h,
                x - 300.0,
                top + h
            );
            layers.push(fframes::svgr!(<path d={glint} fill="#FFFFFF" opacity="0.55" style="mix-blend-mode:screen" />));
        }
        fframes::svgr!(<g transform={band_tf}>{layers}</g>)
    }
}

/// zlob's gold star coin: flipped, spinning in place and slowing down until it lands face on.
fn coin<'a>(t: f32, c: Vec2) -> Svgr<'a> {
    let x = c.x;
    let settle = 0.5;
    let k = (t / settle).min(1.0);
    // three full turns, slowing to a stop face on
    let spin = std::f32::consts::TAU * 3.0 * (1.0 - (1.0 - k) * (1.0 - k));
    let y = c.y - 60.0 * (1.0 - k) * (k * 4.0).min(1.0);
    let w = spin.cos();
    let r = 210.0;
    let rx = (w.abs() * r).max(10.0);
    let face = if w > 0.0 { "#F2C230" } else { "#C8901A" };
    let star: Vec<Vec2> = (0..10)
        .map(|i| {
            let rr = if i % 2 == 0 { 125.0 } else { 52.0 };
            let a = -std::f32::consts::FRAC_PI_2 + i as f32 * std::f32::consts::PI / 5.0;
            Vec2::new(x + a.cos() * rr * w.abs(), y + a.sin() * rr)
        })
        .collect();
    let mut sp = Path::new();
    sp.polygon(&star);
    fframes::svgr!(
        <g>
            <ellipse cx={x + 14.0} cy={y} rx={rx} ry={r} fill="#8A5A0E" />
            <ellipse cx={x} cy={y} rx={rx} ry={r} fill={face} stroke={palette::INK} stroke-width="8" />
            <ellipse cx={x} cy={y} rx={rx * 0.82} ry={r * 0.82} fill="none" stroke="#8A5A0E" stroke-width="6" opacity={w.abs()} />
            <path d={sp.build()} fill="#FFE27A" stroke={palette::INK} stroke-width="5" opacity={w.abs()} />
        </g>
    )
}
