//! The circuit board on the jig of the watcher: the footprint of the chip U17 with gold pads,
//! traces that fan out to vias, small parts, white print, and the aluminum fixture under the
//! board. The camera sees it in a macro shot, so the parts have their real sizes in meters.

use std::f32::consts::FRAC_PI_2;

use fframes_3d::math::hash;
use fframes_3d::*;

/// The pins on each side of the chip, their pitch and the distance from the center of the
/// chip to the middle of their pads (`crate::city::chip`).
const PINS: usize = 44;
const PITCH: f32 = 0.0008;
const PAD_AT: f32 = 0.0198;
/// The size of the board: X, thickness, Z.
pub const SIZE: Vec3 = Vec3::new(0.24, 0.0016, 0.18);

/// A box from `a` to `b`, with its bottom on y = 0.
fn slab(a: Vec2, b: Vec2, width: f32, height: f32) -> Geometry {
    let length = a.distance(b);
    let mid = a.midpoint(b);
    let angle = (b.y - a.y).atan2(b.x - a.x);
    Geometry::cuboid(length + width, height, width)
        .rotate_y(-angle)
        .translate(mid.x, height * 0.5, mid.y)
}

/// Adds the board to `scene` at `origin`, the middle of the footprint on the top of the
/// board. The axes of the board are the world axes. `font` makes the white print.
pub fn build(scene: &mut Scene, origin: Vec3, font: &[u8]) -> NodeId {
    let root = scene.add(Node::group().at(origin.x, origin.y, origin.z));
    let gold = Material::metal(0xe8b85a, 0.2);
    let copper = Material::metal(0xc9955a, 0.32).clearcoat(0.6);
    let mut mask = Material::standard(0x0a0e13)
        .roughness(0.5)
        .clearcoat(0.6)
        .env_intensity(0.3);
    mask.clearcoat_roughness = 0.1;
    let print = Material::standard(0xe4e8ec).roughness(0.7);

    // The board: solder mask on top, the glass fiber core at the edges.
    scene.add_to(
        root,
        Node::mesh(
            Geometry::cuboid(SIZE.x, SIZE.y, SIZE.z).translate(0.01, -SIZE.y * 0.5, 0.02),
            mask.clone(),
        ),
    );

    // The pads of the footprint, and a trace from each pad: out, a bend of 45 degrees, out
    // again, and a via at its end. The traces of each side fan out from the middle.
    let mut pads = Vec::new();
    let mut traces = Vec::new();
    let mut vias = Vec::new();
    for side in 0..4 {
        let turn = Quat::from_rotation_y(side as f32 * FRAC_PI_2);
        let flat = |p: Vec2| {
            let v = turn * vec3(p.x, 0.0, p.y);
            vec2(v.x, v.z)
        };
        for k in 0..PINS {
            let across = (k as f32 - (PINS - 1) as f32 * 0.5) * PITCH;
            let seed = (side * PINS + k) as u32;
            pads.push(slab(
                flat(vec2(PAD_AT - 0.0009, across)),
                flat(vec2(PAD_AT + 0.0009, across)),
                0.00042,
                0.00004,
            ));
            let out = PAD_AT + 0.0009;
            let first = out + 0.001;
            let bend = vec2(first + across.abs() * 0.6, across * 1.6);
            let end = bend + vec2(0.002 + 0.007 * hash(seed + 400), 0.0);
            let points = [vec2(out, across), vec2(first, across), bend, end].map(flat);
            for w in points.windows(2) {
                traces.push(slab(w[0], w[1], 0.00022, 0.00003));
            }
            if hash(seed + 77) > 0.35 {
                vias.push(vec3(points[3].x, 0.0, points[3].y));
            }
        }
    }
    scene.add_to(root, Node::mesh(Geometry::merge(pads), gold.clone()));
    scene.add_to(root, Node::mesh(Geometry::merge(traces), copper));
    let via = Geometry::merge([
        Geometry::cylinder(0.00042, 0.00042, 0.00005, 12).translate(0.0, 0.000025, 0.0),
        Geometry::cylinder(0.0002, 0.0002, 0.00002, 10).translate(0.0, 0.00006, 0.0),
    ]);
    scene.add_to(
        root,
        Node::instanced(
            via,
            gold.clone(),
            Instances::new(vias.iter().map(|p| Mat4::from_translation(*p))),
        ),
    );

    // Small parts: capacitors and resistors with metal ends, a crystal, a second chip and a
    // coil. They stand outside the fan of the traces.
    let mut ceramic = Vec::new();
    let mut ends = Vec::new();
    for i in 0..46 {
        let seed = 3000 + i as u32;
        let a = hash(seed) * std::f32::consts::TAU;
        let r = 0.047 + 0.04 * hash(seed + 1);
        let at = vec2(a.cos() * r, a.sin() * r * 0.8);
        let (length, width) = if hash(seed + 2) > 0.5 {
            (0.001, 0.0005)
        } else {
            (0.0016, 0.0008)
        };
        let angle = if hash(seed + 3) > 0.5 { 0.0 } else { FRAC_PI_2 };
        let place = |g: Geometry| g.rotate_y(angle).translate(at.x, 0.0, at.y);
        ceramic.push(place(
            Geometry::cuboid(length * 0.6, width, width).translate(0.0, width * 0.5, 0.0),
        ));
        for e in [-1.0f32, 1.0] {
            ends.push(place(
                Geometry::cuboid(length * 0.22, width * 1.02, width * 1.02).translate(
                    e * length * 0.39,
                    width * 0.5,
                    0.0,
                ),
            ));
        }
    }
    scene.add_to(
        root,
        Node::mesh(
            Geometry::merge(ceramic),
            Material::standard(0x5d5a55).roughness(0.6),
        ),
    );
    scene.add_to(
        root,
        Node::mesh(Geometry::merge(ends), Material::metal(0xd9dce0, 0.25)),
    );
    let black = Material::standard(0x16181b).roughness(0.5);
    scene.add_to(
        root,
        Node::mesh(
            Geometry::rounded_box(0.007, 0.0009, 0.007, 0.0002, 1).translate(0.052, 0.00045, -0.03),
            black.clone(),
        ),
    );
    scene.add_to(
        root,
        Node::mesh(
            Geometry::rounded_box(0.005, 0.0012, 0.0032, 0.0003, 2)
                .translate(-0.042, 0.0006, 0.026),
            Material::metal(0xc8ccd2, 0.18),
        ),
    );
    scene.add_to(
        root,
        Node::mesh(
            Geometry::rounded_box(0.006, 0.003, 0.006, 0.0006, 2).translate(0.048, 0.0015, 0.034),
            Material::standard(0x2a2c30).roughness(0.6),
        ),
    );

    // White print: the outline corners of the footprint, the pin 1 dot and the name.
    let corner = 0.0225;
    let mut marks: Vec<Geometry> = [(1.0f32, 1.0f32), (1.0, -1.0), (-1.0, 1.0), (-1.0, -1.0)]
        .iter()
        .flat_map(|(x, z)| {
            let c = vec2(x * corner, z * corner);
            [
                slab(c, c - vec2(x * 0.003, 0.0), 0.00025, 0.00002),
                slab(c, c - vec2(0.0, z * 0.003), 0.00025, 0.00002),
            ]
        })
        .collect();
    marks.push(Geometry::cylinder(0.0006, 0.0006, 0.00002, 16).translate(
        -corner - 0.0015,
        0.0,
        -corner,
    ));
    // The name reads from above with +Z up in the picture, as the dive of shot 3 sees it.
    if let Ok(name) = Geometry::text(font, "U17", 0.0026, 0.00002, 0.0) {
        let c = name.bounds().center();
        marks.push(
            name.translate(-c.x, -c.y, 0.0)
                .rotate_x(FRAC_PI_2)
                .rotate_z(std::f32::consts::PI)
                .translate(0.012, 0.0, corner + 0.0035),
        );
    }
    scene.add_to(root, Node::mesh(Geometry::merge(marks), print));

    // Fiducials at two corners of the board.
    for (x, z) in [(-0.1f32, -0.06f32), (0.12, 0.1)] {
        scene.add_to(
            root,
            Node::mesh(
                Geometry::cylinder(0.0008, 0.0008, 0.00004, 20).translate(x, 0.00002, z),
                gold.clone(),
            ),
        );
    }

    // The fixture: an aluminum plate under the board with four pins at its corners.
    let aluminum = Material::metal(0xaab0b8, 0.32);
    scene.add_to(
        root,
        Node::mesh(
            Geometry::rounded_box(SIZE.x + 0.06, 0.02, SIZE.z + 0.06, 0.003, 2).translate(
                0.01,
                -SIZE.y - 0.01,
                0.02,
            ),
            aluminum.clone(),
        ),
    );
    let pins = Geometry::merge(
        [(-1.0f32, -1.0f32), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)].map(|(x, z)| {
            Geometry::cylinder(0.003, 0.003, 0.006, 16).translate(
                0.01 + x * (SIZE.x * 0.5 + 0.006),
                0.001,
                0.02 + z * (SIZE.z * 0.5 + 0.006),
            )
        }),
    );
    scene.add_to(root, Node::mesh(pins, Material::metal(0xe0e3e7, 0.15)));
    root
}
