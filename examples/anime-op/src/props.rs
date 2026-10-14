//! The props of the opening, modelled like the cast (rounded primitives, cel shaded, inked):
//! the cream puff (シュークリーム) and the barbell.

use std::f32::consts::{PI, TAU};
use std::sync::OnceLock;

use fframes_anime::glam::{Quat, Vec3};
use fframes_anime::prop::{self, Prop};
use fframes_anime::rig::Bone;
use fframes_anime::sdf::{Model, Op, Prim, Shape};

use crate::palette;
use crate::render::rgb;

/// Material groups of the cream puff.
pub mod puff {
    /// The craquelin cap, cracked and dusted with sugar.
    pub const CRUST: u8 = 0;
    /// Whipped cream piped in rosettes between the halves.
    pub const CREAM: u8 = 1;
    /// The bottom shell.
    pub const BASE: u8 = 2;
}

/// Size of the cream puff: it sits on y = 0, about this wide.
pub const PUFF_WIDTH: f32 = 0.11;
/// Its height.
pub const PUFF_HEIGHT: f32 = 0.085;

fn sphere(at: Vec3, r: f32, g: u8) -> Prim {
    Prim::new(Shape::Sphere(r), g, Bone::Hips).at(at)
}

fn ellipsoid(at: Vec3, r: Vec3, g: u8) -> Prim {
    Prim::new(Shape::Ellipsoid(r), g, Bone::Hips).at(at)
}

/// Keeps what is above the plane `y` (or below it with `above = false`).
fn cut(y: f32, above: bool, g: u8, blend: f32) -> Prim {
    let flip = if above {
        Quat::from_rotation_x(PI)
    } else {
        Quat::IDENTITY
    };
    Prim::new(Shape::Below, g, Bone::Hips)
        .at(Vec3::new(0.0, y, 0.0))
        .rot(flip)
        .op(Op::Clip)
        .blend(blend)
}

fn puff_model() -> Model {
    use puff::*;
    let mut m = Model::default();
    // the bottom shell: a flat-bottomed bowl
    m.push(ellipsoid(
        Vec3::new(0.0, 0.017, 0.0),
        Vec3::new(0.046, 0.02, 0.046),
        BASE,
    ));
    m.push(cut(0.002, true, BASE, 0.006));
    m.push(cut(0.028, false, BASE, 0.004));
    // the cream: a ring of piped rosettes bulging out between the halves
    m.push(
        ellipsoid(
            Vec3::new(0.0, 0.031, 0.0),
            Vec3::new(0.043, 0.012, 0.043),
            CREAM,
        )
        .blend(0.006),
    );
    // soft swells of cream, blended into one rolling band (separate blobs read as teeth)
    for k in 0..8 {
        let a = k as f32 / 8.0 * TAU + 0.2;
        let y = 0.031 + 0.003 * (a * 3.0).sin();
        m.push(
            sphere(
                Vec3::new(a.cos() * 0.043, y, a.sin() * 0.043),
                0.0125 + 0.0012 * (a * 5.0).cos(),
                CREAM,
            )
            .blend(0.011),
        );
    }
    // the cap: puffed up lumps of choux on a low dome, sitting a little askew
    let tilt = Quat::from_rotation_z(0.08) * Quat::from_rotation_x(-0.05);
    let cap = |p: Vec3| Vec3::new(0.0, 0.037, 0.0) + tilt * (p - Vec3::new(0.0, 0.037, 0.0));
    m.push(ellipsoid(
        cap(Vec3::new(0.0, 0.046, 0.0)),
        Vec3::new(0.047, 0.026, 0.047),
        CRUST,
    ));
    for k in 0..6 {
        let a = k as f32 / 6.0 * TAU + 0.3;
        let wobble = (k as f32 * 2.3).sin();
        m.push(
            sphere(
                cap(Vec3::new(
                    a.cos() * 0.031,
                    0.056 + 0.005 * wobble,
                    a.sin() * 0.031,
                )),
                0.024 + 0.003 * (k as f32 * 1.7).cos(),
                CRUST,
            )
            .blend(0.008),
        );
    }
    m.push(sphere(cap(Vec3::new(0.004, 0.068, -0.003)), 0.022, CRUST).blend(0.009));
    m.push(cut(0.037, true, CRUST, 0.005));
    m
}

/// Cracks of the choux cap, a dusting of powdered sugar on top, and a warmer tone where it baked
/// darker.
fn puff_detail(rest: Vec3, group: u8, color: Vec3, px: f32) -> Vec3 {
    let crack_color = rgb("#A8530F");
    match group {
        puff::CRUST => {
            // darker where it baked longer, lighter on the crown
            let up = (rest - Vec3::new(0.0, 0.03, 0.0)).normalize_or(Vec3::Y).y;
            let mut c = color.lerp(rgb("#F7B24A"), ((up - 0.35) / 0.6).clamp(0.0, 1.0) * 0.6);
            // a few cracks of the craquelin: only some cell borders split
            let (d1, d2, id) = prop::cells(rest, 0.03, 11);
            if id > 0.45 {
                let w = (px * 0.8).max(0.0008) * (0.6 + id);
                c = c.lerp(crack_color, prop::cover(d2 - d1 - w, px) * 0.85);
            }
            // powdered sugar where the cap faces up
            if up > 0.5 {
                let (s1, _, sid) = prop::cells(rest, 0.006, 29);
                let r = (0.0011 + 0.0008 * sid) * ((up - 0.5) / 0.3).clamp(0.0, 1.0);
                if sid > 0.3 {
                    c = c.lerp(rgb(palette::WHITE), prop::cover(s1 - r, px));
                }
            }
            c
        }
        _ => color,
    }
}

/// The cream puff, built once.
pub fn cream_puff() -> &'static Prop {
    static PUFF: OnceLock<Prop> = OnceLock::new();
    PUFF.get_or_init(|| {
        Prop::new(
            &puff_model(),
            Vec3::new(-0.07, -0.01, -0.07),
            Vec3::new(0.07, 0.1, 0.07),
            0.0018,
        )
        .with_colors(&[
            (puff::CRUST, rgb(palette::PUFF)),
            (puff::CREAM, rgb("#FFF2D2")),
            (puff::BASE, rgb("#F59D38")),
        ])
        .with_detail(puff_detail)
    })
}

/// Points of the cream puff's surface for a convex collider (metres, unscaled).
pub fn puff_hull() -> Vec<Vec3> {
    let p = cream_puff();
    // every few vertices is plenty for a hull
    p.mesh.rest.iter().step_by(7).copied().collect()
}

/// Material groups of the barbell.
pub mod bell {
    pub const STEEL: u8 = 0;
    /// The plates, painted with a ring.
    pub const PLATE: u8 = 1;
    pub const HUB: u8 = 2;
}

/// Radius of the bar.
pub const BAR_RADIUS: f32 = 0.0135;
/// Radius and thickness of the big plate (the small one is the same plate scaled).
pub const PLATE_RADIUS: f32 = 0.42;
pub const PLATE_WIDTH: f32 = 0.075;

/// Keeps the side of the plane through `at` that `normal` points away from.
fn slab(at: Vec3, normal: Vec3, g: u8, blend: f32) -> Prim {
    Prim::new(Shape::Below, g, Bone::Hips)
        .at(at)
        .rot(Quat::from_rotation_arc(Vec3::Y, normal))
        .op(Op::Clip)
        .blend(blend)
}

/// One metre of bar along +X from the origin (scaled to its length when placed).
pub fn bar() -> &'static Prop {
    static BAR: OnceLock<Prop> = OnceLock::new();
    BAR.get_or_init(|| {
        let mut m = Model::default();
        m.push(Prim::new(
            Shape::Cone {
                a: Vec3::ZERO,
                b: Vec3::X,
                ra: BAR_RADIUS,
                rb: BAR_RADIUS,
            },
            bell::STEEL,
            Bone::Hips,
        ));
        Prop::new(
            &m,
            Vec3::new(-0.03, -0.03, -0.03),
            Vec3::new(1.03, 0.03, 0.03),
            0.0035,
        )
        .with_colors(&[(bell::STEEL, rgb("#2A2520"))])
    })
}

/// A weight plate on the X axis, centred on the origin.
pub fn plate() -> &'static Prop {
    static PLATE: OnceLock<Prop> = OnceLock::new();
    PLATE.get_or_init(|| {
        let (r, w) = (PLATE_RADIUS, PLATE_WIDTH);
        let mut m = Model::default();
        m.push(Prim::new(Shape::Sphere(r), bell::PLATE, Bone::Hips));
        m.push(slab(
            Vec3::new(w * 0.5, 0.0, 0.0),
            Vec3::X,
            bell::PLATE,
            0.012,
        ));
        m.push(slab(
            Vec3::new(-w * 0.5, 0.0, 0.0),
            Vec3::NEG_X,
            bell::PLATE,
            0.012,
        ));
        // the hub sticks out of both faces
        m.push(Prim::new(Shape::Sphere(0.075), bell::HUB, Bone::Hips));
        m.push(slab(
            Vec3::new(w * 0.5 + 0.018, 0.0, 0.0),
            Vec3::X,
            bell::HUB,
            0.006,
        ));
        m.push(slab(
            Vec3::new(-w * 0.5 - 0.018, 0.0, 0.0),
            Vec3::NEG_X,
            bell::HUB,
            0.006,
        ));
        Prop::new(
            &m,
            Vec3::new(-0.08, -r - 0.02, -r - 0.02),
            Vec3::new(0.08, r + 0.02, r + 0.02),
            0.0055,
        )
        .with_colors(&[
            (bell::PLATE, rgb("#1A1612")),
            (bell::HUB, rgb(palette::YELLOW_DEEP)),
        ])
        .with_detail(plate_detail)
    })
}

/// A ring painted on each face of a plate, like the reference's green-striped plates.
fn plate_detail(rest: Vec3, group: u8, color: Vec3, px: f32) -> Vec3 {
    if group != bell::PLATE || rest.x.abs() < PLATE_WIDTH * 0.35 {
        return color;
    }
    let r = (rest.y * rest.y + rest.z * rest.z).sqrt();
    let ring = (r - PLATE_RADIUS * 0.64).abs() - 0.011;
    let hole = (r - 0.105).abs() - 0.006;
    color
        .lerp(rgb(palette::GREEN_LIGHT), prop::cover(ring, px))
        .lerp(rgb(palette::GREEN), prop::cover(hole, px))
}
