//! Small flat drawings: TJ's dog, geese for the flood, collage stickers, keycaps.

use std::f32::consts::{PI, TAU};

use fframes::Svgr;
use fframes_anime::glam::Vec2;
use fframes_anime::shape::Path;

use crate::palette;

fn rot(v: Vec2, a: f32) -> Vec2 {
    let (s, c) = a.sin_cos();
    Vec2::new(v.x * c - v.y * s, v.x * s + v.y * c)
}

/// A German Shepherd in profile (facing +x when `dir` = 1). `run` is the gait phase in turns,
/// `lift` how much the ears and tail are thrown by the body's vertical speed (a spring).
pub fn dog(c: Vec2, s: f32, dir: f32, run: f32, lift: f32) -> Path {
    let p = |x: f32, y: f32| c + Vec2::new(x * dir, y) * s;
    let mut path = Path::new();
    // body
    path.smooth_closed(
        &[
            p(-0.55, -0.08),
            p(-0.2, -0.26),
            p(0.35, -0.24),
            p(0.58, -0.1),
            p(0.52, 0.14),
            p(0.0, 0.2),
            p(-0.5, 0.14),
        ],
        1.0,
    );
    // a round head straight on the shoulders (no neck), a short snout and a nose
    path.smooth_closed(
        &(0..14)
            .map(|i| {
                let a = i as f32 / 14.0 * TAU;
                p(0.62 + a.cos() * 0.25, -0.34 + a.sin() * 0.23)
            })
            .collect::<Vec<_>>(),
        1.0,
    );
    path.smooth_closed(
        &[p(0.78, -0.4), p(1.02, -0.34), p(1.04, -0.22), p(0.8, -0.16)],
        0.9,
    );
    path.circle(p(1.04, -0.32), 0.05 * s);
    // floppy ears hanging down the side of the head, flapping up with `lift`
    for k in 0..2 {
        let base = p(0.52 + k as f32 * 0.1, -0.5);
        let hang = rot(Vec2::new(-0.12 * dir, 0.24), dir * lift * 0.8);
        let tip = base + hang * s;
        let side = Vec2::new(-hang.y, hang.x).normalize_or(Vec2::X) * 0.07 * s;
        path.smooth_closed(
            &[base - side, tip - side * 0.6, tip + side * 0.6, base + side],
            1.0,
        );
    }
    // tail: a curve flicking with the lift
    let tail_base = p(-0.52, -0.05);
    let tail_dir = rot(
        Vec2::new(-dir, 0.35),
        dir * (lift * 0.8 + (run * TAU * 2.0).sin() * 0.15),
    );
    let mid = tail_base + tail_dir * 0.3 * s;
    let tip = mid + rot(tail_dir, dir * 0.4) * 0.28 * s;
    let side = Vec2::new(-tail_dir.y, tail_dir.x);
    path.polygon(&[
        tail_base + side * 0.06 * s,
        mid + side * 0.05 * s,
        tip,
        mid - side * 0.05 * s,
        tail_base - side * 0.06 * s,
    ]);
    // legs: two pairs in a gallop
    for (k, hip) in [(0, -0.38_f32), (1, 0.38)] {
        for j in 0..2 {
            let ph = run * TAU + k as f32 * PI + j as f32 * 0.6;
            let swing = ph.sin() * 0.45;
            let top = p(hip + j as f32 * 0.06, 0.08);
            let knee = top + rot(Vec2::new(0.0, 0.22), dir * swing) * s;
            let foot = knee + rot(Vec2::new(0.0, 0.2), dir * (swing - 0.3 * ph.cos().max(0.0))) * s;
            let w = 0.075 * s;
            path.polygon(&[
                top - Vec2::new(w, 0.0),
                knee - Vec2::new(w * 0.7, 0.0),
                foot - Vec2::new(w * 0.5, 0.0),
                foot + Vec2::new(w * 0.9, 0.0),
                knee + Vec2::new(w * 0.7, 0.0),
                top + Vec2::new(w, 0.0),
            ]);
        }
    }
    path
}

/// A white goose seen from the side, rotated by `angle`.
pub fn goose<'a>(c: Vec2, s: f32, angle: f32, flip: bool, line: f32) -> Svgr<'a> {
    let f = if flip { -1.0 } else { 1.0 };
    let p = |x: f32, y: f32| c + rot(Vec2::new(x * f, y), angle) * s;
    let mut body = Path::new();
    body.smooth_closed(
        &[
            p(-0.55, 0.0),
            p(-0.3, -0.28),
            p(0.2, -0.3),
            p(0.45, -0.1),
            p(0.4, 0.2),
            p(-0.1, 0.32),
            p(-0.5, 0.2),
            p(-0.75, -0.1),
        ],
        1.0,
    );
    body.smooth_closed(
        &[
            p(0.25, -0.2),
            p(0.38, -0.7),
            p(0.5, -0.88),
            p(0.68, -0.86),
            p(0.62, -0.72),
            p(0.5, -0.66),
            p(0.45, -0.2),
        ],
        0.9,
    );
    let mut beak = Path::new();
    beak.polygon(&[p(0.64, -0.86), p(0.92, -0.78), p(0.62, -0.74)]);
    let mut wing = Path::new();
    wing.smooth_open(
        &[
            p(-0.4, -0.08),
            p(-0.05, -0.18),
            p(0.25, -0.02),
            p(-0.1, 0.12),
        ],
        1.0,
    );
    let eye = p(0.56, -0.8);
    fframes::svgr!(
        <g stroke={palette::INK} stroke-width={line} stroke-linejoin="round">
            <path d={body.build()} fill={palette::WHITE} />
            <path d={beak.build()} fill={palette::ORANGE} />
            <path d={wing.build()} fill="none" stroke-linecap="round" />
            <circle cx={eye.x} cy={eye.y} r={0.035 * s} fill={palette::INK} stroke="none" />
        </g>
    )
}

#[derive(Debug, Clone, Copy)]
pub enum Sticker {
    Star,
    Heart,
    Flower,
    Badge(&'static str),
    Puff,
    Goose,
}

/// A die-cut sticker (white border) of radius `r` at `c`, rotated by `angle`.
pub fn sticker<'a>(kind: Sticker, c: Vec2, r: f32, angle: f32, color: &'static str) -> Svgr<'a> {
    match kind {
        Sticker::Puff => crate::fx::puff(c, r, angle, 1.0, Vec2::Y, 4.0),
        Sticker::Goose => goose(c, r * 1.1, angle, false, 3.0),
        Sticker::Badge(text) => {
            let tf = format!("rotate({} {} {})", angle.to_degrees(), c.x, c.y);
            let label = text.to_owned();
            fframes::svgr!(
                <g transform={tf}>
                    <rect x={c.x - r * 1.5} y={c.y - r * 0.55} width={r * 3.0} height={r * 1.1} rx={r * 0.55} fill={palette::WHITE} stroke={palette::INK} stroke-width="4" />
                    <rect x={c.x - r * 1.35} y={c.y - r * 0.4} width={r * 2.7} height={r * 0.8} rx={r * 0.4} fill={color} />
                    <text x={c.x} y={c.y + r * 0.22} font-family="Dela Gothic One" font-size={r * 0.55} text-anchor="middle" fill={palette::WHITE}>{label}</text>
                </g>
            )
        }
        _ => {
            let pts: Vec<Vec2> = match kind {
                Sticker::Star => (0..10)
                    .map(|i| {
                        let rr = if i % 2 == 0 { r } else { r * 0.45 };
                        c + rot(
                            Vec2::from_angle(-PI / 2.0 + i as f32 * PI / 5.0) * rr,
                            angle,
                        )
                    })
                    .collect(),
                Sticker::Heart => (0..32)
                    .map(|i| {
                        let t = i as f32 / 32.0 * TAU;
                        let x = 16.0 * t.sin().powi(3);
                        let y = -(13.0 * t.cos()
                            - 5.0 * (2.0 * t).cos()
                            - 2.0 * (3.0 * t).cos()
                            - (4.0 * t).cos());
                        c + rot(Vec2::new(x, y) * (r / 17.0), angle)
                    })
                    .collect(),
                _ => (0..40)
                    .map(|i| {
                        let t = i as f32 / 40.0 * TAU;
                        let rr = r * (0.65 + 0.35 * (t * 5.0).cos().abs());
                        c + rot(Vec2::from_angle(t) * rr, angle)
                    })
                    .collect(),
            };
            let mut p = Path::new();
            p.smooth_closed(
                &pts,
                if matches!(kind, Sticker::Star) {
                    0.0
                } else {
                    1.0
                },
            );
            let d = p.build();
            let d2 = d.clone();
            fframes::svgr!(
                <g>
                    <path d={d} fill={palette::WHITE} stroke={palette::WHITE} stroke-width={r * 0.25} stroke-linejoin="round" />
                    <path d={d2} fill={color} stroke={palette::INK} stroke-width="3" stroke-linejoin="round" />
                </g>
            )
        }
    }
}

/// A keycap seen from the front-top, with a letter.
pub fn keycap<'a>(c: Vec2, s: f32, angle: f32, label: char) -> Svgr<'a> {
    let tf = format!("rotate({} {} {})", angle.to_degrees(), c.x, c.y);
    let l = label.to_string();
    fframes::svgr!(
        <g transform={tf}>
            <rect x={c.x - s * 0.5} y={c.y - s * 0.42} width={s} height={s * 0.9} rx={s * 0.14} fill={palette::INK} />
            <rect x={c.x - s * 0.4} y={c.y - s * 0.5} width={s * 0.8} height={s * 0.72} rx={s * 0.12} fill={palette::WHITE} stroke={palette::INK} stroke-width="3" />
            <text x={c.x} y={c.y + s * 0.05} font-family="Zen Kaku Gothic New" font-weight="900" font-size={s * 0.4} text-anchor="middle" fill={palette::INK}>{l}</text>
        </g>
    )
}
