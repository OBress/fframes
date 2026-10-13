//! The machines that prepare the launch in the time-lapse: the transporter erector that
//! rolls the rocket out of the hangar and raises it, the crawler crane that builds the
//! tower, and the hangar. Each is built once as a few rigid meshes; the frames move them.

use fframes_3d::*;

use super::super::{mix, span};

/// A unit box that stands on Y = 0..1, for [`span`].
fn unit() -> Geometry {
    Geometry::cuboid(1.0, 1.0, 1.0).translate(0.0, 0.5, 0.0)
}

/// The members of a lattice from `a` to `b` as transforms of the unit box: four chords, a
/// frame at each of the `bays` and a diagonal on each face. `side` turns the faces around
/// the axis, and the width goes from `width.0` at `a` to `width.1` at `b`.
fn lattice(a: Vec3, b: Vec3, side: Vec3, width: (f32, f32), bays: usize, chord: f32) -> Vec<Mat4> {
    let d = (b - a).normalize_or(Vec3::Y);
    let u = (side - d * side.dot(d)).normalize_or(d.any_orthonormal_vector());
    let v = d.cross(u);
    let corners = [(1.0, 1.0), (-1.0, 1.0), (-1.0, -1.0), (1.0, -1.0)];
    let corner = |s: f32, k: usize| {
        let w = mix(width.0, width.1, s) * 0.5;
        let (cu, cv) = corners[k % 4];
        a.lerp(b, s) + (u * cu + v * cv) * w
    };
    let mut out = Vec::new();
    for k in 0..4 {
        out.push(span(corner(0.0, k), corner(1.0, k), chord));
    }
    for i in 0..bays {
        let (s0, s1) = (i as f32 / bays as f32, (i + 1) as f32 / bays as f32);
        for k in 0..4 {
            out.push(span(corner(s1, k), corner(s1, k + 1), chord * 0.6));
            out.push(span(corner(s0, k), corner(s1, k + 1), chord * 0.5));
        }
    }
    out
}

/// One mesh of unit boxes placed by `transforms`.
fn boxes(transforms: impl IntoIterator<Item = Mat4>) -> Geometry {
    let unit = unit();
    Geometry::merge(transforms.into_iter().map(|m| unit.clone().transform(m)))
}

/// A box with the corners `min` and `max`.
fn block(min: Vec3, max: Vec3) -> Geometry {
    let size = max - min;
    let center = (min + max) * 0.5;
    Geometry::cuboid(size.x, size.y, size.z).translate(center.x, center.y, center.z)
}

/// The parts of the transporter erector, in the space of its pivot: X along the
/// transporter away from the pad, Y up. The rocket and the strongback turn around the
/// pivot.
pub struct Erector {
    /// The chassis on its rail bogies, 10 m longer than the strongback, with the hinge
    /// frame at the pivot, the rams and the cab. Its rails are at `Y = -rail`.
    pub chassis: Geometry,
    /// The wheels and the dark parts of the chassis.
    pub wheels: Geometry,
    /// The strongback when it stands up: a lattice tower along +Y with the clamps that hold
    /// the rocket. The axis of the rocket is at `X = -axis`.
    pub strongback: Geometry,
    /// The places of the work lights on the chassis.
    pub lamps: Vec<Vec3>,
}

/// The transporter erector. `axis` is the distance from the pivot to the axis of the
/// rocket, `body` the radius of the rocket, `rail` the height of the pivot above the
/// rails, and `length` the length of the strongback.
pub fn erector(axis: f32, body: f32, rail: f32, length: f32) -> Erector {
    let floor = -rail;
    let end = length + 10.0;
    // two box girders over the rails and cross beams. The strongback lies on them.
    let mut chassis = Vec::new();
    let mut wheels = Vec::new();
    for z in [-3.0f32, 3.0] {
        chassis.push(block(
            vec3(-1.5, floor + 0.7, z - 0.7),
            vec3(end, floor + 1.25, z + 0.7),
        ));
        // a bogie with four wheels every 9 m
        let mut x = 2.0;
        while x < end - 2.0 {
            wheels.push(block(
                vec3(x - 2.4, floor + 0.1, z - 0.9),
                vec3(x + 2.4, floor + 0.75, z + 0.9),
            ));
            for dx in [-1.5f32, 1.5] {
                wheels.push(
                    Geometry::cylinder(0.5, 0.5, 0.5, 12)
                        .rotate_x(std::f32::consts::FRAC_PI_2)
                        .translate(x + dx, floor + 0.55, z + z.signum()),
                );
            }
            x += 9.0;
        }
        // the hinge: an A frame on each side up to the pivot
        chassis.push(boxes([
            span(vec3(-1.2, floor + 1.25, z), vec3(0.0, 0.0, z * 0.7), 0.6),
            span(vec3(1.4, floor + 1.25, z), vec3(0.0, 0.0, z * 0.7), 0.6),
        ]));
        // the cylinders of the rams lie beside the strongback; the frames draw the rods
        chassis.push(block(
            vec3(14.0, floor + 1.25, z * 0.73 - 0.4),
            vec3(22.0, floor + 1.9, z * 0.73 + 0.4),
        ));
    }
    for i in 0..13 {
        let x = i as f32 * 6.0;
        chassis.push(block(
            vec3(x - 0.4, floor + 0.75, -3.0),
            vec3(x + 0.4, floor + 1.2, 3.0),
        ));
    }
    // the cab at the far end, and the power unit
    chassis.push(Geometry::rounded_box(4.0, 3.2, 5.0, 0.3, 2).translate(end, floor + 2.85, 0.0));
    chassis.push(block(
        vec3(length + 1.0, floor + 1.25, -2.5),
        vec3(length + 8.0, floor + 3.4, 2.5),
    ));

    // the strongback: a lattice of 2.4 m with the clamps at three heights
    let mut back = lattice(
        vec3(0.0, 0.0, 0.0),
        vec3(0.0, length, 0.0),
        Vec3::X,
        (2.6, 2.0),
        (length / 3.0) as usize,
        0.35,
    );
    for y in [6.0f32, length * 0.42, length * 0.78] {
        for z in [-1.0f32, 1.0] {
            let from = vec3(-0.9, y, z * 1.1);
            let to = vec3(-axis + body * 0.2, y, z * (body + 0.35));
            back.push(span(from, to, 0.45));
        }
        back.push(span(
            vec3(-axis + body * 0.2, y, -(body + 0.35)),
            vec3(-axis + body * 0.2, y, body + 0.35),
            0.35,
        ));
    }
    // the cable tray and the pipes of the propellants along the strongback
    back.push(span(
        vec3(-1.4, 2.0, 0.0),
        vec3(-1.4, length * 0.8, 0.0),
        0.5,
    ));
    let lamps = vec![
        vec3(end, floor + 3.3, 2.6),
        vec3(end, floor + 3.3, -2.6),
        vec3(1.0, floor + 1.8, 3.8),
        vec3(1.0, floor + 1.8, -3.8),
        vec3(length * 0.5, floor + 1.6, 3.8),
    ];
    Erector {
        chassis: Geometry::merge(chassis),
        wheels: Geometry::merge(wheels),
        strongback: boxes(back),
        lamps,
    }
}

/// The foot of the boom of the crawler crane, in its space.
pub const CRAWLER_FOOT: Vec3 = Vec3::new(3.0, 6.5, 0.0);
/// The top of the gantry of the crawler crane, where the pendant lines start.
pub const CRAWLER_GANTRY: Vec3 = Vec3::new(-4.0, 16.0, 0.0);

/// The parts of a crawler crane with a lattice boom, in its own space: X forward, Y up.
pub struct Crawler {
    /// The tracks, the house, the counterweight and the gantry.
    pub body: Geometry,
    /// The boom along +X from its foot, `length` meters long. The frames turn it around Z
    /// to luff it.
    pub boom: Geometry,
}

/// A crawler crane with a boom of `length` meters, as big cranes that stack rockets.
pub fn crawler(length: f32) -> Crawler {
    let gantry = CRAWLER_GANTRY;
    let mut body = vec![
        // the tracks and the car body
        Geometry::rounded_box(16.0, 2.6, 2.4, 0.5, 2).translate(0.0, 1.3, -4.6),
        Geometry::rounded_box(16.0, 2.6, 2.4, 0.5, 2).translate(0.0, 1.3, 4.6),
        block(vec3(-4.0, 2.0, -3.6), vec3(4.0, 3.2, 3.6)),
        // the house with its cab, and the counterweight
        block(vec3(-6.0, 3.2, -3.4), vec3(5.0, 7.2, 3.4)),
        Geometry::rounded_box(2.4, 2.6, 2.0, 0.2, 2).translate(5.4, 5.0, 3.2),
        block(vec3(-11.0, 3.4, -4.0), vec3(-6.0, 9.0, 4.0)),
    ];
    body.push(boxes([
        span(vec3(-3.0, 7.2, -2.5), gantry, 0.7),
        span(vec3(-3.0, 7.2, 2.5), gantry, 0.7),
        span(vec3(1.0, 7.2, 0.0), gantry, 0.6),
    ]));
    let boom = boxes(lattice(
        Vec3::ZERO,
        vec3(length, 0.0, 0.0),
        Vec3::Y,
        (3.2, 1.4),
        (length / 4.0) as usize,
        0.45,
    ));
    let boom = Geometry::merge([
        boom,
        // the head with its sheaves
        Geometry::cylinder(0.9, 0.9, 1.2, 12)
            .rotate_x(std::f32::consts::FRAC_PI_2)
            .translate(length, 0.0, 0.0),
    ]);
    Crawler {
        body: Geometry::merge(body),
        boom,
    }
}

/// The hangar of the rockets: a hall `length` meters long along X, with its open doors on
/// the -X face, which looks to the pad. Its origin is the middle of the door at the
/// ground. Returns the walls and the light of the inside, seen through the door.
pub fn hangar(length: f32, width: f32, height: f32) -> (Geometry, Geometry) {
    let (w, h) = (width * 0.5, height);
    let door = (width * 0.7 * 0.5, height * 0.82);
    let walls = Geometry::merge([
        block(vec3(length - 1.0, 0.0, -w), vec3(length, h, w)),
        block(vec3(0.0, 0.0, -w), vec3(length, h, -w + 1.0)),
        block(vec3(0.0, 0.0, w - 1.0), vec3(length, h, w)),
        block(
            vec3(-1.0, h - 1.5, -w - 1.0),
            vec3(length + 1.0, h + 1.0, w + 1.0),
        ),
        // the face around the door, and the door leaves pushed to the sides
        block(vec3(0.0, door.1, -w), vec3(1.2, h, w)),
        block(vec3(0.0, 0.0, -w), vec3(1.2, h, -door.0)),
        block(vec3(0.0, 0.0, door.0), vec3(1.2, h, w)),
        block(vec3(-2.2, 0.0, -w - 6.0), vec3(-1.0, door.1, -w + 2.0)),
        block(vec3(-2.2, 0.0, w - 2.0), vec3(-1.0, door.1, w + 6.0)),
    ]);
    // the lit back wall, the far side wall and the floor, seen through the door
    let inside = Geometry::merge([
        Geometry::plane(2.0 * door.0, door.1)
            .rotate_y(-std::f32::consts::FRAC_PI_2)
            .translate(length - 1.1, door.1 * 0.5, 0.0),
        Geometry::plane(length - 2.0, door.1).translate(length * 0.5, door.1 * 0.5, -w + 1.1),
        Geometry::plane(2.0 * door.0, length - 2.0)
            .rotate_x(-std::f32::consts::FRAC_PI_2)
            .translate(length * 0.5, 0.05, 0.0),
    ]);
    (walls, inside)
}
