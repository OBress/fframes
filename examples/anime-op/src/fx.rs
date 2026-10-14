//! Graphic pieces of the opening: speed lines, sunbursts, ink splashes, sparkles, text bands,
//! the cream puff, ornate banner frames.

use std::f32::consts::{PI, TAU};

use fframes::Svgr;
use fframes_anime::glam::Vec2;
use fframes_anime::shape::{self, Path};

use crate::palette;

/// Deterministic noise in 0..1.
pub fn hash(n: u32) -> f32 {
    let mut x = n.wrapping_mul(0x9E37_79B9) ^ 0x85EB_CA6B;
    x ^= x >> 15;
    x = x.wrapping_mul(0x2C1B_3C6D);
    x ^= x >> 12;
    x = x.wrapping_mul(0x297A_2D39);
    x ^= x >> 15;
    (x & 0x00FF_FFFF) as f32 / 16_777_216.0
}

pub fn hash2(a: u32, b: u32) -> f32 {
    hash(a.wrapping_mul(7919).wrapping_add(b.wrapping_mul(104_729)))
}

pub fn clamp01(x: f32) -> f32 {
    x.clamp(0.0, 1.0)
}

pub fn smooth(x: f32) -> f32 {
    let x = clamp01(x);
    x * x * (3.0 - 2.0 * x)
}

pub fn ease_out(x: f32) -> f32 {
    let x = clamp01(x);
    1.0 - (1.0 - x).powi(3)
}

pub fn ease_in(x: f32) -> f32 {
    let x = clamp01(x);
    x * x * x
}

/// Damped spring from 0 to 1 (overshoots), `t` in seconds.
pub fn spring(t: f32, freq: f32, damping: f32) -> f32 {
    if t <= 0.0 {
        return 0.0;
    }
    1.0 - (-damping * t).exp() * (freq * TAU * t).cos()
}

/// Time held on twos (24 fps).
pub fn twos(t: f32) -> f32 {
    fframes_anime::koma::hold(t, 24.0, 2)
}

pub fn threes(t: f32) -> f32 {
    fframes_anime::koma::hold(t, 24.0, 3)
}

pub fn rect<'a>(x: f32, y: f32, w: f32, h: f32, fill: &'static str) -> Svgr<'a> {
    fframes::svgr!(<rect x={x} y={y} width={w} height={h} fill={fill} />)
}

pub fn bg<'a>(fill: &'static str) -> Svgr<'a> {
    rect(-10.0, -10.0, 1940.0, 1100.0, fill)
}

pub fn letterbox<'a>(h: f32) -> Svgr<'a> {
    fframes::svgr!(
        <g>
            <rect x="-10" y="-10" width="1940" height={h + 10.0} fill={palette::INK} />
            <rect x="-10" y={1080.0 - h} width="1940" height={h + 10.0} fill={palette::INK} />
        </g>
    )
}

pub fn path<'a>(d: String, fill: &'static str) -> Svgr<'a> {
    fframes::svgr!(<path d={d} fill={fill} />)
}

pub fn stroke<'a>(d: String, color: &'static str, width: f32) -> Svgr<'a> {
    fframes::svgr!(<path d={d} fill="none" stroke={color} stroke-width={width} stroke-linecap="round" stroke-linejoin="round" />)
}

/// Radial wedges around `c` (a sunburst); `n` wedges, `fill` fraction of each wedge is solid.
pub fn sunburst(c: Vec2, n: usize, rotation: f32, inner: f32, outer: f32, fill: f32) -> Path {
    let mut p = Path::new();
    for i in 0..n {
        let a0 = rotation + i as f32 * TAU / n as f32;
        let a1 = a0 + TAU / n as f32 * fill;
        let pts = [
            c + Vec2::from_angle(a0) * inner,
            c + Vec2::from_angle(a0) * outer,
            c + Vec2::from_angle(a1) * outer,
            c + Vec2::from_angle(a1) * inner,
        ];
        p.polygon(&pts);
    }
    p
}

/// Focus lines (集中線): thin wedges converging on `c` that leave a clear area of radius `clear`.
/// They change every drawing (`seed` from the held time).
pub fn focus_lines(c: Vec2, count: usize, clear: f32, seed: u32, thickness: f32) -> Path {
    let mut p = Path::new();
    for i in 0..count {
        let a = (i as f32 + hash2(seed, i as u32) * 0.8) / count as f32 * TAU;
        let r0 = clear * (0.85 + hash2(seed + 7, i as u32) * 0.6);
        let w = thickness * (0.3 + hash2(seed + 3, i as u32) * 1.4) / 1000.0;
        let dir = Vec2::from_angle(a);
        let side = Vec2::new(-dir.y, dir.x);
        let far = c + dir * 2400.0;
        p.polygon(&[
            c + dir * r0,
            far + side * 2400.0 * w,
            far - side * 2400.0 * w,
        ]);
    }
    p
}

/// Parallel speed lines (流線) crossing the frame in direction `angle`.
pub fn speed_lines(angle: f32, count: usize, seed: u32, offset: f32, thickness: f32) -> Path {
    let dir = Vec2::from_angle(angle);
    let side = Vec2::new(-dir.y, dir.x);
    let center = Vec2::new(960.0, 540.0);
    let mut p = Path::new();
    for i in 0..count {
        let lane = (hash2(seed, i as u32) - 0.5) * 2600.0;
        let len = 300.0 + hash2(seed + 1, i as u32) * 1400.0;
        let pos = ((hash2(seed + 2, i as u32) * 4000.0 + offset) % 4000.0) - 2000.0;
        let w = thickness * (0.3 + hash2(seed + 3, i as u32));
        let a = center + side * lane + dir * pos;
        let b = a + dir * len;
        p.polygon(&[
            a + side * w * 0.1,
            b + side * w,
            b - side * w,
            a - side * w * 0.1,
        ]);
    }
    p
}

/// An ink splash: a blob with spikes and a few droplets.
pub fn splash(c: Vec2, r: f32, seed: u32, spikes: usize) -> Path {
    let n = spikes * 4;
    let mut pts = Vec::with_capacity(n);
    for i in 0..n {
        let a = i as f32 / n as f32 * TAU;
        let spike = if i % 4 == 0 {
            1.0 + hash2(seed, i as u32) * 1.1
        } else {
            0.75 + hash2(seed, i as u32) * 0.25
        };
        pts.push(c + Vec2::from_angle(a) * r * spike);
    }
    let mut p = Path::new();
    p.smooth_closed(&pts, 0.6);
    for i in 0..spikes {
        let a = hash2(seed + 11, i as u32) * TAU;
        let d = r * (1.6 + hash2(seed + 12, i as u32) * 1.2);
        let rr = r * (0.05 + hash2(seed + 13, i as u32) * 0.12);
        p.circle(c + Vec2::from_angle(a) * d, rr);
    }
    p
}

/// A four-pointed sparkle.
pub fn sparkle(c: Vec2, r: f32, rotation: f32) -> Path {
    let mut pts = Vec::with_capacity(8);
    for i in 0..8 {
        let rr = if i % 2 == 0 { r } else { r * 0.18 };
        pts.push(c + Vec2::from_angle(rotation + i as f32 * PI / 4.0) * rr);
    }
    let mut p = Path::new();
    p.smooth_closed(&pts, 0.25);
    p
}

/// A line of repeated text scrolling horizontally (the "MASHLE MASHLE" bands).
pub fn text_band<'a>(
    text: &str,
    y: f32,
    size: f32,
    advance: f32,
    offset: f32,
    fill: &'static str,
    family: &'static str,
) -> Svgr<'a> {
    let shift = offset.rem_euclid(advance);
    let items: Vec<Svgr<'a>> = (0..((2200.0 / advance) as usize + 2))
        .map(|i| {
            let x = -advance + shift + i as f32 * advance;
            let s = text.to_owned();
            fframes::svgr!(<text x={x} y={y} font-family={family} font-size={size} fill={fill}>{s}</text>)
        })
        .collect();
    fframes::svgr!(<g>{items}</g>)
}

/// Outlines of a cream puff (Mash's favourite), drawn like the 3D one in `props`: a lumpy,
/// cracked choux cap dusted with sugar, a rolling band of cream bulging out under it and a
/// smaller golden base. `squash` > 1 flattens it along `normal` (the last impact), < 1
/// stretches it. Returns (base, cap, cream, cracks, sugar) path data.
pub fn puff_paths(c: Vec2, r: f32, rotation: f32, squash: f32, normal: Vec2) -> [String; 5] {
    let (s, co) = rotation.sin_cos();
    let n = normal.normalize_or(Vec2::Y);
    let tangent = Vec2::new(-n.y, n.x);
    let tf = |v: Vec2| -> Vec2 {
        let v = v * r;
        let w = Vec2::new(v.x * co - v.y * s, v.x * s + v.y * co);
        c + n * (w.dot(n) / squash) + tangent * (w.dot(tangent) * squash)
    };
    // the cap: a dome of five puffed lumps over a flat underside at y = 0.05
    let mut cap = Vec::new();
    for i in 0..=40 {
        let u = i as f32 / 40.0;
        let a = PI + u * PI;
        let lump = 1.0 + 0.09 * (u * PI * 5.0).sin().abs().powf(0.6) - 0.05;
        cap.push(Vec2::new(
            a.cos() * 0.98 * lump,
            a.sin() * 0.78 * lump + 0.02,
        ));
    }
    cap.push(Vec2::new(0.9, 0.06));
    cap.push(Vec2::new(-0.9, 0.06));
    // the cream: piped rosettes bulging out past the cap, round at both ends, swelling below
    let mut cream = Vec::new();
    let arc = |out: &mut Vec<Vec2>, c: Vec2, r: f32, a0: f32, a1: f32, n: usize| {
        for i in 0..=n {
            let a = a0 + (a1 - a0) * i as f32 / n as f32;
            out.push(c + Vec2::new(a.cos(), a.sin()) * r);
        }
    };
    cream.push(Vec2::new(-0.9, 0.04));
    cream.push(Vec2::new(0.9, 0.04));
    // the right end rolls out and down
    arc(
        &mut cream,
        Vec2::new(0.93, 0.19),
        0.16,
        -PI * 0.5,
        PI * 0.5,
        6,
    );
    // rosettes along the bottom, right to left
    let rosettes = 5;
    for k in 0..rosettes {
        let x = 0.72 - k as f32 * 1.44 / (rosettes - 1) as f32;
        arc(
            &mut cream,
            Vec2::new(x, 0.27),
            0.19,
            PI * 0.15,
            PI * 0.85,
            5,
        );
    }
    arc(
        &mut cream,
        Vec2::new(-0.93, 0.19),
        0.16,
        PI * 0.5,
        PI * 1.5,
        6,
    );
    // the base: a smaller, rounder bowl
    let mut base = Vec::new();
    for i in 0..=16 {
        let a = i as f32 / 16.0 * PI;
        base.push(Vec2::new(a.cos() * 0.84, 0.32 + a.sin() * 0.28));
    }
    let to = |pts: &[Vec2]| pts.iter().map(|p| tf(*p)).collect::<Vec<_>>();
    let mut cp = Path::new();
    cp.smooth_closed(&to(&cap), 1.0);
    let mut crp = Path::new();
    crp.smooth_closed(&to(&cream), 1.0);
    let mut bp = Path::new();
    bp.smooth_closed(&to(&base), 1.0);
    // cracks of the craquelin between the lumps
    let mut cracks = Path::new();
    let lines: [&[(f32, f32)]; 5] = [
        &[(-0.55, -0.62), (-0.42, -0.4), (-0.48, -0.18)],
        &[(-0.1, -0.78), (0.02, -0.55), (-0.06, -0.3)],
        &[(0.38, -0.68), (0.5, -0.42), (0.62, -0.22)],
        &[(-0.42, -0.4), (-0.2, -0.36)],
        &[(0.02, -0.55), (0.24, -0.5)],
    ];
    for pts in lines {
        let p: Vec<Vec2> = pts.iter().map(|(x, y)| tf(Vec2::new(*x, *y))).collect();
        cracks.polyline(&p);
    }
    // powdered sugar on the crown
    let mut sugar = Path::new();
    for i in 0..16u32 {
        let h = |k: u32| hash2(i * 7 + 3, k);
        let p = Vec2::new(
            -0.7 + 1.4 * h(1),
            -0.72 + 0.32 * h(2) + 0.25 * (h(1) - 0.5).abs(),
        );
        sugar.circle(tf(p), r * (0.018 + 0.016 * h(3)));
    }
    [
        bp.build(),
        cp.build(),
        crp.build(),
        cracks.build(),
        sugar.build(),
    ]
}

/// A painted cream puff.
pub fn puff<'a>(c: Vec2, r: f32, rotation: f32, squash: f32, normal: Vec2, line: f32) -> Svgr<'a> {
    let [bpd, cpd, crpd, ckd, sgd] = puff_paths(c, r, rotation, squash, normal);
    fframes::svgr!(
        <g stroke={palette::INK} stroke-width={line} stroke-linejoin="round">
            <path d={bpd} fill="#F59D38" />
            <path d={crpd} fill="#FFF2D2" />
            <path d={cpd} fill={palette::PUFF} />
            <path d={ckd} fill="none" stroke="#A8530F" stroke-width={line * 0.6} stroke-linecap="round" />
            <path d={sgd} fill="#FFF8E1" stroke="none" />
        </g>
    )
}

/// A cream puff drawn as lines only, in one colour.
pub fn puff_lines<'a>(c: Vec2, r: f32, rotation: f32, color: &'static str, line: f32) -> Svgr<'a> {
    let [bpd, cpd, crpd, ckd, _] = puff_paths(c, r, rotation, 1.0, Vec2::Y);
    fframes::svgr!(
        <g stroke={color} stroke-width={line} fill="none" stroke-linejoin="round" stroke-linecap="round">
            <path d={bpd} />
            <path d={crpd} />
            <path d={cpd} />
            <path d={ckd} />
        </g>
    )
}

/// An ornamental frame: double border, corner fans and a centre medallion ring.
pub fn ornate_frame(x: f32, y: f32, w: f32, h: f32, inset: f32) -> Path {
    let mut p = Path::new();
    let r = |x0: f32, y0: f32, x1: f32, y1: f32| {
        [
            Vec2::new(x0, y0),
            Vec2::new(x1, y0),
            Vec2::new(x1, y1),
            Vec2::new(x0, y1),
        ]
    };
    p.polyline(&r(x + inset, y + inset, x + w - inset, y + h - inset))
        .close();
    p.polyline(&r(
        x + inset * 1.6,
        y + inset * 1.6,
        x + w - inset * 1.6,
        y + h - inset * 1.6,
    ))
    .close();
    for (cx, cy, a0) in [
        (x + inset, y + inset, 0.0),
        (x + w - inset, y + inset, PI * 0.5),
        (x + w - inset, y + h - inset, PI),
        (x + inset, y + h - inset, PI * 1.5),
    ] {
        let c = Vec2::new(cx, cy);
        for k in 0..6 {
            let a = a0 + (k as f32 + 0.5) / 6.0 * PI * 0.5;
            p.polyline(&[
                c + Vec2::from_angle(a) * inset * 0.8,
                c + Vec2::from_angle(a) * inset * 2.6,
            ]);
        }
    }
    // scrolls along the long edges
    for k in 0..4 {
        let fx = x + w * (0.12 + k as f32 * 0.25);
        for (yy, dir) in [(y + inset * 1.6, 1.0), (y + h - inset * 1.6, -1.0)] {
            p.smooth_open(
                &[
                    Vec2::new(fx - inset * 2.0, yy),
                    Vec2::new(fx - inset, yy + dir * inset * 0.9),
                    Vec2::new(fx, yy + dir * inset * 0.3),
                    Vec2::new(fx + inset, yy + dir * inset * 0.9),
                    Vec2::new(fx + inset * 2.0, yy),
                ],
                1.0,
            );
        }
    }
    p
}

/// Points of a circle, for building shapes.
pub fn circle_pts(c: Vec2, r: f32, n: usize) -> Vec<Vec2> {
    shape::ellipse(c, r, r, 0.0, n)
}

/// A spiky impact star.
pub fn star_burst(c: Vec2, r: f32, seed: u32) -> Path {
    let n = 12;
    let pts: Vec<Vec2> = (0..n * 2)
        .map(|i| {
            let a = i as f32 / (n * 2) as f32 * TAU;
            let rr = if i % 2 == 0 {
                r * (0.8 + hash2(seed, i as u32) * 0.6)
            } else {
                r * 0.35
            };
            c + Vec2::from_angle(a) * rr
        })
        .collect();
    let mut p = Path::new();
    p.polygon(&pts);
    p
}

static BLURS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Motion blur (撮影のブラー), on the GPU: under the sharp drawing, a copy smeared along
/// `velocity` (pixels per frame) trails behind it, the way compositors blur a fast cel.
/// `pivot` is a point on the moving thing (the blur is rotated about it).
pub fn motion_blur<'a>(content: Svgr<'a>, velocity: Vec2, pivot: Vec2) -> Svgr<'a> {
    let speed = velocity.length();
    if speed < 6.0 {
        return content;
    }
    let angle = velocity.y.atan2(velocity.x).to_degrees();
    let id = format!(
        "mblur{}",
        BLURS.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    );
    let url = format!("url(#{id})");
    let deviation = format!("{:.1} 0", (speed * 0.45).min(120.0));
    let turn = format!("rotate({angle:.2} {:.1} {:.1})", pivot.x, pivot.y);
    let back = -velocity * 0.6;
    let unturn = format!(
        "rotate({:.2} {:.1} {:.1}) translate({:.1} {:.1})",
        -angle, pivot.x, pivot.y, back.x, back.y
    );
    let trail = content.clone();
    fframes::svgr!(
        <g>
            <defs>
                <filter id={id} x="-100%" y="-100%" width="300%" height="300%">
                    <feGaussianBlur stdDeviation={deviation} />
                </filter>
            </defs>
            <g opacity="0.8" transform={turn}>
                <g filter={url}>
                    <g transform={unturn}>{trail}</g>
                </g>
            </g>
            {content}
        </g>
    )
}

/// A whole layer smeared along `velocity` (pixels per frame), no sharp copy: the background
/// of a shot whose camera races after the action (背景ブレ).
pub fn blur_layer<'a>(content: Svgr<'a>, velocity: Vec2) -> Svgr<'a> {
    let speed = velocity.length();
    if speed < 4.0 {
        return content;
    }
    let angle = velocity.y.atan2(velocity.x).to_degrees();
    let id = format!(
        "lblur{}",
        BLURS.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    );
    let url = format!("url(#{id})");
    let deviation = format!("{:.1} 0.5", (speed * 0.55).min(140.0));
    let turn = format!("rotate({angle:.2} 960 540)");
    let unturn = format!("rotate({:.2} 960 540)", -angle);
    fframes::svgr!(
        <g>
            <defs>
                <filter id={id} x="-25%" y="-25%" width="150%" height="150%">
                    <feGaussianBlur stdDeviation={deviation} edgeMode="duplicate" />
                </filter>
            </defs>
            <g transform={turn}>
                <g filter={url}>
                    <g transform={unturn}>{content}</g>
                </g>
            </g>
        </g>
    )
}

/// Screen velocity (pixels per frame) of a world point seen by a moving camera.
pub fn screen_velocity(
    p: fframes_anime::glam::Vec3,
    now: &fframes_anime::camera::Camera,
    before: &fframes_anime::camera::Camera,
    frames: f32,
) -> Vec2 {
    (now.to_screen(p) - before.to_screen(p)) / frames.max(1e-3)
}
