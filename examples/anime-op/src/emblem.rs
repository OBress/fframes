//! Heraldry: the goose crest and the four house crests (fff, fframes, odiff, zlob).

use std::f32::consts::{PI, TAU};

use fframes_anime::glam::Vec2;
use fframes_anime::shape::Path;

/// A heraldic goose with its wings spread, flat and symmetric like a school crest: five straight
/// feather blades a side with slits of negative space between them, a body, a necktie for a
/// tail (the uniform), a long neck and a head with a quiff tuft. `spread` 0 = folded, 1 = fully
/// open; `flap` bends the feather tips (follow-through), `c` is the centre of the body, `s` the
/// scale (px per unit).
pub fn goose(c: Vec2, s: f32, spread: f32, flap: f32) -> Path {
    let p = |x: f32, y: f32| c + Vec2::new(x, y) * s;
    let mut path = Path::new();
    // body
    path.smooth_closed(
        &(0..16)
            .map(|i| {
                let a = i as f32 / 16.0 * TAU;
                p(a.cos() * 0.2, 0.08 + a.sin() * 0.3)
            })
            .collect::<Vec<_>>(),
        1.0,
    );
    // the tail is a necktie: a knot, then a blade widening to a point
    path.polygon(&[
        p(-0.07, 0.34),
        p(0.07, 0.34),
        p(0.05, 0.46),
        p(0.14, 0.9),
        p(0.0, 1.06),
        p(-0.14, 0.9),
        p(-0.05, 0.46),
    ]);
    // the neck rising straight, the head turned to the left with its beak
    path.smooth_closed(
        &[
            p(-0.065, -0.12),
            p(-0.075, -0.45),
            p(-0.05, -0.62),
            p(0.05, -0.62),
            p(0.075, -0.45),
            p(0.065, -0.12),
        ],
        0.8,
    );
    path.smooth_closed(
        &(0..14)
            .map(|i| {
                let a = i as f32 / 14.0 * TAU;
                p(a.cos() * 0.12, -0.72 + a.sin() * 0.1)
            })
            .collect::<Vec<_>>(),
        1.0,
    );
    path.polygon(&[p(-0.09, -0.76), p(-0.32, -0.71), p(-0.09, -0.66)]);
    // the quiff: three flicks off the top of the head
    for (x, lean) in [(-0.05_f32, -0.06_f32), (0.01, 0.0), (0.07, 0.07)] {
        path.polygon(&[
            p(x - 0.035, -0.79),
            p(x + lean, -0.98 + x.abs() * 0.6),
            p(x + 0.035, -0.79),
        ]);
    }
    // wings: straight blades fanned from the shoulders, the upper ones longest
    for sx in [-1.0_f32, 1.0] {
        let shoulder = p(0.16 * sx, -0.04);
        for k in 0..5 {
            let f = k as f32 / 4.0;
            // folded they hang down along the body; open they fan from level to steep
            let open = (-14.0 + 58.0 * f).to_radians();
            let folded = (-70.0 + 8.0 * f).to_radians();
            let a = folded + (open - folded) * spread;
            let dir = Vec2::new(a.cos() * sx, -a.sin());
            let side = Vec2::new(-dir.y, dir.x);
            let len = (0.55 + 0.25 * (1.0 - (f - 0.6).abs() * 1.4)) * s;
            let w = (0.075 - 0.01 * f) * s;
            let bend = Vec2::from_angle(-sx * flap * 0.25 * (0.4 + f));
            let tip_dir = Vec2::new(
                dir.x * bend.x - dir.y * bend.y,
                dir.x * bend.y + dir.y * bend.x,
            );
            let root = shoulder + dir * 0.04 * s;
            let shaft = root + dir * len * 0.8;
            path.polygon(&[
                root + side * w * 0.7,
                shaft + side * w,
                shaft + tip_dir * len * 0.2 + side * w * 0.15,
                shaft + tip_dir * len * 0.2 - side * w * 0.15,
                shaft - side * w,
                root - side * w * 0.7,
            ]);
        }
    }
    let _ = PI;
    path
}

/// A round seal: ring, inner ring and 24 ticks.
pub fn seal_ring(c: Vec2, r: f32) -> Path {
    let mut p = Path::new();
    p.circle(c, r);
    p.circle(c, r * 0.9);
    for i in 0..24 {
        let a = i as f32 / 24.0 * TAU;
        p.polyline(&[
            c + Vec2::from_angle(a) * r * 0.93,
            c + Vec2::from_angle(a) * r * 0.98,
        ]);
    }
    p
}

/// fframes: a film frame with sprockets and a lightning bolt.
pub fn film(c: Vec2, s: f32) -> Path {
    let mut p = Path::new();
    let r = |x0: f32, y0: f32, x1: f32, y1: f32| {
        [
            c + Vec2::new(x0, y0) * s,
            c + Vec2::new(x1, y0) * s,
            c + Vec2::new(x1, y1) * s,
            c + Vec2::new(x0, y1) * s,
        ]
    };
    p.polygon(&r(-0.62, -0.42, 0.62, -0.32));
    p.polygon(&r(-0.62, 0.32, 0.62, 0.42));
    for i in 0..6 {
        let x = -0.55 + i as f32 * 0.22;
        p.polygon(&r(x, -0.29, x + 0.12, -0.2));
        p.polygon(&r(x, 0.2, x + 0.12, 0.29));
    }
    let b = |x: f32, y: f32| c + Vec2::new(x, y) * s;
    p.polygon(&[
        b(0.08, -0.17),
        b(-0.14, 0.03),
        b(-0.01, 0.03),
        b(-0.09, 0.17),
        b(0.15, -0.05),
        b(0.02, -0.05),
    ]);
    p
}

/// odiff: two overlapping images with highlighted differing pixels.
pub fn diff(c: Vec2, s: f32) -> Path {
    let mut p = Path::new();
    let sq = |x: f32, y: f32, w: f32| {
        [
            c + Vec2::new(x, y) * s,
            c + Vec2::new(x + w, y) * s,
            c + Vec2::new(x + w, y + w) * s,
            c + Vec2::new(x, y + w) * s,
        ]
    };
    p.polygon(&sq(-0.55, -0.45, 0.6));
    p.polygon(&sq(-0.05, -0.15, 0.6));
    for (x, y) in [
        (0.05, 0.0),
        (0.2, 0.15),
        (0.35, -0.05),
        (0.15, 0.3),
        (0.4, 0.25),
    ] {
        p.polygon(&sq(x, y, 0.09));
    }
    p
}

/// zlob: the glob `**/*` as stars with a bolt.
pub fn glob(c: Vec2, s: f32) -> Path {
    let mut p = Path::new();
    for (x, y, r) in [(-0.38, -0.12, 0.2), (-0.08, -0.12, 0.2), (0.38, 0.14, 0.2)] {
        let cc = c + Vec2::new(x, y) * s;
        let pts: Vec<Vec2> = (0..12)
            .map(|i| {
                let rr = if i % 2 == 0 { r } else { r * 0.42 };
                cc + Vec2::from_angle(-PI / 2.0 + i as f32 * PI / 6.0) * rr * s
            })
            .collect();
        p.polygon(&pts);
    }
    let b = |x: f32, y: f32| c + Vec2::new(x, y) * s;
    p.polygon(&[b(0.24, -0.42), b(0.06, 0.42), b(0.12, 0.42), b(0.3, -0.42)]);
    p
}

/// fff: a magnifying glass over three stacked file sheets.
pub fn search(c: Vec2, s: f32) -> Path {
    let mut p = Path::new();
    for k in 0..3 {
        let o = Vec2::new(-0.18 + k as f32 * 0.12, -0.32 + k as f32 * 0.1) * s;
        let r = |x: f32, y: f32| c + o + Vec2::new(x, y) * s;
        p.polygon(&[
            r(-0.25, -0.2),
            r(0.12, -0.2),
            r(0.22, -0.1),
            r(0.22, 0.3),
            r(-0.25, 0.3),
        ]);
    }
    let lens = c + Vec2::new(0.12, 0.08) * s;
    p.circle(lens, 0.3 * s);
    p.circle(lens, 0.2 * s);
    let d = Vec2::new(0.7, 0.7).normalize();
    let side = Vec2::new(-d.y, d.x);
    let a = lens + d * 0.28 * s;
    let b = lens + d * 0.62 * s;
    p.polygon(&[
        a + side * 0.07 * s,
        b + side * 0.09 * s,
        b - side * 0.09 * s,
        a - side * 0.07 * s,
    ]);
    p
}
