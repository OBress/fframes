//! The Golden Gate Bridge with its real dimensions, built from parts that the montage puts
//! up one by one.
//!
//! The space of the bridge: X along the bridge toward Marin (north), Y up, Z toward the bay
//! (east). The origin is the middle of the main span at sea level. Meters.

use std::f32::consts::FRAC_PI_2;
use std::sync::Arc;

use fframes_3d::*;

use super::{AMBER, span};

/// Half the main span: the towers stand at X = ±640.
pub const HALF_SPAN: f32 = 640.0;
/// The length of a side span, from a tower to an anchorage.
pub const SIDE_SPAN: f32 = 343.0;
/// The X of the anchorages.
pub const ANCHOR: f32 = HALF_SPAN + SIDE_SPAN;
/// The top of the towers above the water.
pub const TOWER_TOP: f32 = 227.0;
/// The sag of the main cables in the main span.
pub const SAG: f32 = 143.0;
/// The Z of the two main cables, 27.4 m apart. The legs of the towers and the stiffening
/// trusses stand in the same planes.
pub const CABLE_Z: f32 = 13.7;
/// The radius of a main cable.
pub const CABLE_RADIUS: f32 = 0.46;
/// The height of the road in the middle of the main span.
pub const ROAD_MID: f32 = 75.0;
/// The depth of the stiffening truss under the road.
pub const TRUSS_DEPTH: f32 = 7.6;
/// The distance between two pairs of suspenders.
pub const SUSPENDER_STEP: f32 = 15.2;
/// The length of a deck segment: four panels of the truss.
pub const SEGMENT: f32 = 30.4;
/// The end of the north viaduct and of the south approach, from the middle of the span.
pub const APPROACH: f32 = 1380.0;
/// International Orange.
pub const ORANGE: u32 = 0xc0362c;
/// The height of the cables where they go into the anchorages.
const ANCHOR_CABLE: f32 = 58.0;
/// The portal struts above the deck: the height of their middle and their height.
pub const PORTALS: [(f32, f32); 4] = [(100.0, 12.0), (138.0, 11.0), (172.0, 10.0), (205.0, 9.0)];
/// The sections of a tower leg from the pier to the top: the bottom, and the size of the
/// cross section along X and Z. Each section steps back from the one below it.
const SECTIONS: [(f32, f32, f32); 5] = [
    (13.0, 16.5, 10.0),
    (100.0, 15.0, 9.4),
    (138.0, 13.4, 8.8),
    (172.0, 11.8, 8.2),
    (205.0, 10.2, 7.6),
];

/// The size of the cross section of a tower leg at the height `y`: along X and along Z.
pub fn leg_size(y: f32) -> Vec2 {
    let s = SECTIONS
        .iter()
        .rev()
        .find(|s| s.0 <= y)
        .unwrap_or(&SECTIONS[0]);
    vec2(s.1, s.2)
}

/// The height of the road at `x`: an arch of 3 m over the main span, and down to the
/// ground on the approaches.
pub fn road_y(x: f32) -> f32 {
    let main = x.abs().min(HALF_SPAN) / HALF_SPAN;
    let side = (x.abs() - HALF_SPAN).max(0.0);
    ROAD_MID - 3.0 * main * main - 0.017 * side
}

/// The center of the main cable on the side `z` (±[`CABLE_Z`]) at `x`. In the main span it
/// hangs as a parabola from the saddles. In the side spans it hangs from the saddle down to
/// the anchorage with a sag of 5 % of the span.
pub fn cable_at(x: f32, z: f32) -> Vec3 {
    let y = if x.abs() <= HALF_SPAN {
        TOWER_TOP - SAG + SAG * (x / HALF_SPAN).powi(2)
    } else {
        let u = ((x.abs() - HALF_SPAN) / SIDE_SPAN).min(1.0);
        TOWER_TOP + (ANCHOR_CABLE - TOWER_TOP) * u - 0.05 * SIDE_SPAN * 4.0 * u * (1.0 - u)
    };
    vec3(x, y + 1.2, z)
}

/// The path of a main cable from the south anchorage to the north anchorage, in steps of
/// about 8 m.
pub fn cable_path(z: f32) -> Vec<Vec3> {
    let n = (2.0 * ANCHOR / 8.0) as usize;
    (0..=n)
        .map(|i| cable_at(-ANCHOR + 2.0 * ANCHOR * i as f32 / n as f32, z))
        .collect()
}

/// The materials of the bridge.
pub struct Paints {
    /// International Orange on the steel.
    pub steel: Material,
    pub concrete: Material,
    pub road: Material,
}

impl Paints {
    /// Plain materials without textures, for the bridge far away.
    pub fn plain() -> Self {
        Self {
            steel: Material::standard(ORANGE).roughness(0.5),
            concrete: Material::standard(0xb8b2a8).roughness(0.9),
            road: Material::standard(0x2a2a2c).roughness(0.9),
        }
    }
}

/// A rectangle of `width` x `depth` with stepped corners: two steps of `step` at each
/// corner. The setbacks make the vertical lines of the art deco towers.
fn stepped(width: f32, depth: f32, step: f32) -> Vec<Vec2> {
    let (x, z, s) = (width * 0.5, depth * 0.5, step);
    // the steps of the +X +Y corner, counterclockwise; the other corners mirror it
    let corner = [
        vec2(x, z - 2.0 * s),
        vec2(x - s, z - 2.0 * s),
        vec2(x - s, z - s),
        vec2(x - 2.0 * s, z - s),
        vec2(x - 2.0 * s, z),
    ];
    let mut points = Vec::new();
    for (sx, sz) in [(1.0, 1.0), (-1.0, 1.0), (-1.0, -1.0), (1.0, -1.0)] {
        let mut part: Vec<Vec2> = corner.iter().map(|p| vec2(p.x * sx, p.y * sz)).collect();
        // a mirror in one axis turns the order around
        if sx * sz < 0.0 {
            part.reverse();
        }
        points.extend(part);
    }
    points
}

/// A shape in the XZ plane, extruded along Y from 0 to 1.
fn prism(shape: &[Vec2]) -> Geometry {
    Geometry::extrude(shape, 1.0, 0.0)
        .rotate_x(-FRAC_PI_2)
        .translate(0.0, 0.5, 0.0)
}

/// A box with the corners `min` and `max`.
fn block(min: Vec3, max: Vec3) -> Geometry {
    let size = max - min;
    let center = (min + max) * 0.5;
    Geometry::cuboid(size.x, size.y, size.z).translate(center.x, center.y, center.z)
}

/// One piece of a tower that the machines put up: an instance of one of the shapes of
/// [`Towers`], and the heights it spans.
pub struct Lift {
    pub shape: usize,
    pub transform: Mat4,
    pub bottom: f32,
    pub top: f32,
}

/// The two towers as lifts of a few shapes: one shape for each section of the legs, the
/// portal strut, the cross brace below the deck, and the crown with the saddle.
pub struct Towers {
    pub shapes: Vec<Geometry>,
    /// The lifts of both towers, in the order of their height.
    pub lifts: Vec<Lift>,
    /// The index of the shape of the crowns, which glow when the bridge is lit.
    pub crown: usize,
}

/// The towers at X = ±[`HALF_SPAN`]. Each leg rises in lifts of about 7 m.
pub fn towers() -> Towers {
    let mut shapes: Vec<Geometry> = SECTIONS
        .iter()
        .map(|&(_, w, d)| prism(&stepped(w, d, 0.9)))
        .collect();
    // the portal: a band with recessed panels between ribs (the fluting), and a cornice
    let portal = shapes.len();
    let mut parts = vec![block(vec3(-0.5, 0.0, -0.5), vec3(0.5, 1.0, 0.5))];
    for k in 0..9 {
        let z = -0.5 + (k as f32 + 0.5) / 9.0;
        parts.push(block(
            vec3(-0.56, 0.08, z - 0.02),
            vec3(0.56, 0.92, z + 0.02),
        ));
    }
    parts.push(block(vec3(-0.6, 0.92, -0.5), vec3(0.6, 1.0, 0.5)));
    shapes.push(Geometry::merge(parts));
    let brace = shapes.len();
    shapes.push(Geometry::cuboid(1.0, 1.0, 1.0).translate(0.0, 0.5, 0.0));
    // the crown on each leg: two steps and the saddle housing for the cable
    let crown = shapes.len();
    let (w, d) = (SECTIONS[4].1, SECTIONS[4].2);
    shapes.push(Geometry::merge([
        prism(&stepped(w * 0.86, d * 0.86, 0.6)).scale(1.0, 2.6, 1.0),
        prism(&stepped(w * 0.7, d * 0.7, 0.5))
            .scale(1.0, 1.4, 1.0)
            .translate(0.0, 2.6, 0.0),
        Geometry::cylinder(2.4, 2.4, 7.0, 16)
            .rotate_z(FRAC_PI_2)
            .scale(1.0, 0.7, 1.0)
            .translate(0.0, 4.0, 0.0),
    ]));

    let mut lifts = Vec::new();
    for x in [-HALF_SPAN, HALF_SPAN] {
        for z in [-CABLE_Z, CABLE_Z] {
            for (k, &(bottom, _, _)) in SECTIONS.iter().enumerate() {
                let top = SECTIONS.get(k + 1).map_or(TOWER_TOP - 5.0, |s| s.0);
                let n = ((top - bottom) / 7.0).ceil() as usize;
                for i in 0..n {
                    let (y0, y1) = (
                        bottom + (top - bottom) * i as f32 / n as f32,
                        bottom + (top - bottom) * (i + 1) as f32 / n as f32,
                    );
                    lifts.push(Lift {
                        shape: k,
                        transform: Mat4::from_translation(vec3(x, y0, z))
                            * Mat4::from_scale(vec3(1.0, y1 - y0, 1.0)),
                        bottom: y0,
                        top: y1,
                    });
                }
            }
            lifts.push(Lift {
                shape: crown,
                transform: Mat4::from_translation(vec3(x, TOWER_TOP - 5.0, z)),
                bottom: TOWER_TOP - 5.0,
                top: TOWER_TOP,
            });
        }
        // the portals between the legs, as wide as the legs at their height
        for &(y, h) in &PORTALS {
            let leg = SECTIONS
                .iter()
                .rev()
                .find(|s| s.0 <= y)
                .map_or(16.0, |s| s.1);
            lifts.push(Lift {
                shape: portal,
                transform: Mat4::from_translation(vec3(x, y - h * 0.5, 0.0))
                    * Mat4::from_scale(vec3(leg * 0.86, h, 2.0 * CABLE_Z)),
                bottom: y - h * 0.5,
                top: y + h * 0.5,
            });
        }
        // the strut under the deck and the cross braces below it
        lifts.push(Lift {
            shape: portal,
            transform: Mat4::from_translation(vec3(x, road_y(x) - TRUSS_DEPTH - 6.0, 0.0))
                * Mat4::from_scale(vec3(13.0, 6.0, 2.0 * CABLE_Z)),
            bottom: road_y(x) - TRUSS_DEPTH - 6.0,
            top: road_y(x) - TRUSS_DEPTH,
        });
        for (y0, y1) in [(16.0f32, 40.0f32), (40.0, 62.0)] {
            for side in [-1.0f32, 1.0] {
                let a = vec3(x, y0, side * (CABLE_Z - 4.0));
                let b = vec3(x, y1, -side * (CABLE_Z - 4.0));
                lifts.push(Lift {
                    shape: brace,
                    transform: span(a, b, 1.0) * Mat4::from_scale(vec3(3.0, 1.0, 1.6)),
                    bottom: y0,
                    top: y1,
                });
            }
        }
    }
    lifts.sort_by(|a, b| a.bottom.total_cmp(&b.bottom));
    Towers {
        shapes,
        lifts,
        crown,
    }
}

/// The parts of one deck segment of [`SEGMENT`] m along X, centered on X = 0 with the road
/// at Y = 0: the steel (two stiffening trusses, floor beams, lateral braces, railings and
/// lamp posts), the road, and the lamp heads.
pub fn deck_segment() -> (Geometry, Geometry, Geometry) {
    let half = SEGMENT * 0.5;
    let panel = SEGMENT / 4.0;
    let mut steel = Vec::new();
    for z in [-CABLE_Z, CABLE_Z] {
        // the chords of the truss and its verticals and diagonals
        steel.push(block(vec3(-half, -1.2, z - 0.5), vec3(half, -0.2, z + 0.5)));
        steel.push(block(
            vec3(-half, -TRUSS_DEPTH, z - 0.5),
            vec3(half, -TRUSS_DEPTH + 0.9, z + 0.5),
        ));
        for k in 0..4 {
            let x0 = -half + k as f32 * panel;
            steel.push(block(
                vec3(x0 - 0.3, -TRUSS_DEPTH, z - 0.35),
                vec3(x0 + 0.3, -0.2, z + 0.35),
            ));
            let (a, b) = if k % 2 == 0 {
                (vec3(x0, -TRUSS_DEPTH + 0.5, z), vec3(x0 + panel, -0.7, z))
            } else {
                (vec3(x0, -0.7, z), vec3(x0 + panel, -TRUSS_DEPTH + 0.5, z))
            };
            steel.push(
                Geometry::cuboid(1.0, 1.0, 1.0)
                    .translate(0.0, 0.5, 0.0)
                    .transform(span(a, b, 1.0) * Mat4::from_scale(vec3(0.4, 1.0, 0.4))),
            );
            // a floor beam across the deck at each panel point
            steel.push(block(
                vec3(x0 - 0.35, -1.6, -CABLE_Z),
                vec3(x0 + 0.35, -0.3, CABLE_Z),
            ));
        }
    }
    // lateral braces between the bottom chords
    for side in [-1.0f32, 1.0] {
        let a = vec3(-half, -TRUSS_DEPTH + 0.4, -CABLE_Z * side);
        let b = vec3(half, -TRUSS_DEPTH + 0.4, CABLE_Z * side);
        steel.push(
            Geometry::cuboid(1.0, 1.0, 1.0)
                .translate(0.0, 0.5, 0.0)
                .transform(span(a, b, 1.0) * Mat4::from_scale(vec3(0.35, 1.0, 0.35))),
        );
    }
    // the sidewalks outside the cables, their railings, and the lamp posts at the curbs
    let mut lamps = Vec::new();
    for side in [-1.0f32, 1.0] {
        steel.push(block(
            vec3(-half, -0.35, side * 14.0 - 1.5),
            vec3(half, -0.1, side * 14.0 + 1.5),
        ));
        steel.push(block(
            vec3(-half, 0.0, side * 15.6 - 0.08),
            vec3(half, 1.2, side * 15.6 + 0.08),
        ));
        steel.push(block(
            vec3(-half, 0.0, side * 9.6 - 0.25),
            vec3(half, 0.8, side * 9.6 + 0.25),
        ));
        for x in [-half * 0.5, half * 0.5] {
            let z = side * 10.2;
            steel.push(Geometry::cylinder(0.14, 0.2, 8.5, 6).translate(x, 4.25, z));
            steel.push(block(
                vec3(x - 0.15, 8.3, z - 0.15),
                vec3(x + 0.15, 8.6, z + 0.15),
            ));
            lamps.push(block(
                vec3(x - 0.3, 8.6, z - 0.3),
                vec3(x + 0.3, 9.3, z + 0.3),
            ));
        }
    }
    let road = block(vec3(-half, -0.3, -9.4), vec3(half, 0.02, 9.4));
    (Geometry::merge(steel), road, Geometry::merge(lamps))
}

/// The transform of the deck segment centered at `x` on the road, along the arch of the
/// road.
pub fn segment_at(x: f32) -> Mat4 {
    let slope = (road_y(x + 1.0) - road_y(x - 1.0)) * 0.5;
    Mat4::from_translation(vec3(x, road_y(x), 0.0)) * Mat4::from_rotation_z(slope.atan())
}

/// The X of the middle of each deck segment of the suspended spans and the approaches.
pub fn segment_centers() -> Vec<f32> {
    let n = (APPROACH / SEGMENT) as i32;
    (-n..n).map(|i| (i as f32 + 0.5) * SEGMENT).collect()
}

/// The suspenders: for each step along the suspended spans and each cable, two ropes from
/// the cable down to the top of the truss. Each is the start and the end point.
pub fn suspenders() -> Vec<(Vec3, Vec3)> {
    let mut out = Vec::new();
    let n = (ANCHOR / SUSPENDER_STEP) as i32;
    for i in -n..=n {
        let x = i as f32 * SUSPENDER_STEP;
        if (x.abs() - HALF_SPAN).abs() < 8.0 || x.abs() > ANCHOR - 10.0 {
            continue;
        }
        for z in [-CABLE_Z, CABLE_Z] {
            for dx in [-0.35f32, 0.35] {
                let top = cable_at(x + dx, z) - Vec3::Y * CABLE_RADIUS;
                out.push((top, vec3(x + dx, road_y(x) - 0.2, z)));
            }
        }
    }
    out
}

/// The concrete of the ends: the piers under the towers with the fender ring around the
/// south pier, the anchorages, the pylons beside the road at the anchorages, and the piers
/// of the approaches. `ground(x, z)` is the height of the land at a point of the bridge.
pub fn concrete(ground: &dyn Fn(f32, f32) -> f32) -> Geometry {
    let mut parts = Vec::new();
    for x in [-HALF_SPAN, HALF_SPAN] {
        parts.push(block(
            vec3(x - 22.0, -6.0, -24.0),
            vec3(x + 22.0, 13.0, 24.0),
        ));
    }
    // the fender: an oval wall around the south pier in the water
    parts.push(
        Geometry::tube_ring(1.0, 0.94, 1.0, 64)
            .scale(48.0, 9.0, 34.0)
            .translate(-HALF_SPAN, 3.5, 0.0),
    );
    for x in [-ANCHOR, ANCHOR] {
        // the north anchorage lies in the hill; the south one is short, the arch follows it
        let out = x.signum();
        let length = if x > 0.0 { 64.0 } else { 20.0 };
        let g = ground(x + out * length * 0.5, 0.0);
        parts.push(block(
            vec3(x.min(x + out * length), g - 10.0, -26.0),
            vec3(x.max(x + out * length), ANCHOR_CABLE + 6.0, 26.0),
        ));
        // the pylons: tall stepped concrete towers beside the road
        for z in [-17.5f32, 17.5] {
            let foot = ground(x, z) - 4.0;
            parts.push(
                prism(&stepped(9.0, 7.0, 0.7))
                    .scale(1.0, road_y(x) + 34.0 - foot, 1.0)
                    .translate(x, foot, z),
            );
            parts.push(
                prism(&stepped(7.0, 5.4, 0.6))
                    .scale(1.0, 5.0, 1.0)
                    .translate(x, road_y(x) + 34.0, z),
            );
        }
    }
    // the piers of the approaches; the arch carries the road over Fort Point
    let mut x = ANCHOR + 64.0 + SEGMENT;
    while x < APPROACH {
        for side in [-1.0f32, 1.0] {
            let px = side * x;
            if px < 0.0 && x < ANCHOR + 125.0 {
                continue;
            }
            for z in [-CABLE_Z, CABLE_Z] {
                let g = ground(px, z);
                let top = road_y(px) - TRUSS_DEPTH;
                if top > g + 1.0 {
                    parts.push(block(
                        vec3(px - 1.8, g - 3.0, z - 1.8),
                        vec3(px + 1.8, top, z + 1.8),
                    ));
                }
            }
        }
        x += 2.0 * SEGMENT;
    }
    Geometry::merge(parts)
}

/// The steel arch over Fort Point at the south end, and the posts from the arch to the
/// road. `ground` is the height of the land at the ends of the arch.
pub fn fort_point_arch(ground: f32) -> Geometry {
    let (x0, x1) = (-ANCHOR - 20.0, -ANCHOR - 120.0);
    let crown = road_y(x0) - TRUSS_DEPTH - 3.0;
    let mut parts = Vec::new();
    for z in [-CABLE_Z, CABLE_Z] {
        let path: Vec<Vec3> = (0..=24)
            .map(|i| {
                let u = i as f32 / 24.0;
                let y = ground + (crown - ground) * (1.0 - (2.0 * u - 1.0).powi(2));
                vec3(x0 + (x1 - x0) * u, y, z)
            })
            .collect();
        parts.push(Geometry::tube(&path, 1.3, 8));
        for k in 1..10 {
            let p = path[k * 24 / 10];
            parts.push(block(
                vec3(p.x - 0.4, p.y, z - 0.4),
                vec3(p.x + 0.4, road_y(p.x) - TRUSS_DEPTH, z + 0.4),
            ));
        }
    }
    Geometry::merge(parts)
}

/// Fort Point: the brick fort of 1861 under the arch, at the water.
pub fn fort_point(ground: f32) -> Geometry {
    let x = -ANCHOR - 70.0;
    Geometry::merge([
        block(
            vec3(x - 36.0, ground - 2.0, -40.0),
            vec3(x + 36.0, ground + 14.0, 8.0),
        ),
        block(
            vec3(x - 34.0, ground + 14.0, -38.0),
            vec3(x + 34.0, ground + 15.2, 6.0),
        ),
    ])
}

/// The Golden Gate for the distance, finished and lit, at `scale` times its size. The
/// root is a group at the middle of the main span on the water line. It has no terrain:
/// `ground` gives the height of the land for the ends, in the space of the bridge.
pub fn bridge(paints: &Paints, scale: f32, ground: &dyn Fn(f32, f32) -> f32) -> Scene {
    let mut scene = Scene::new();
    let root = scene.add(Node::group().scaled(scale));
    let towers = towers();
    for (i, shape) in towers.shapes.iter().enumerate() {
        let transforms: Vec<Mat4> = towers
            .lifts
            .iter()
            .filter(|l| l.shape == i)
            .map(|l| l.transform)
            .collect();
        scene.add_to(
            root,
            Node::instanced(
                shape.clone(),
                paints.steel.clone(),
                Instances::new(transforms),
            ),
        );
    }
    for z in [-CABLE_Z, CABLE_Z] {
        scene.add_to(
            root,
            Node::mesh(
                Geometry::tube(&cable_path(z), CABLE_RADIUS * 1.6, 8),
                paints.steel.clone(),
            ),
        );
    }
    let ropes: Vec<Mat4> = suspenders()
        .into_iter()
        .map(|(a, b)| span(a, b, 0.12))
        .collect();
    scene.add_to(
        root,
        Node::instanced(
            Geometry::cylinder(1.0, 1.0, 1.0, 4).translate(0.0, 0.5, 0.0),
            paints.steel.clone(),
            Instances::new(ropes),
        ),
    );
    let (steel, road, lamps) = deck_segment();
    let segments: Arc<Instances> = Arc::new(Instances::new(
        segment_centers().into_iter().map(segment_at),
    ));
    scene.add_to(
        root,
        Node::instanced(steel, paints.steel.clone(), segments.clone()),
    );
    scene.add_to(
        root,
        Node::instanced(road, paints.road.clone(), segments.clone()),
    );
    scene.add_to(
        root,
        Node::instanced(lamps, Material::unlit(Rgb::hex(AMBER) * 6.0), segments),
    );
    scene.add_to(root, Node::mesh(concrete(ground), paints.concrete.clone()));
    scene.add_to(
        root,
        Node::mesh(
            fort_point_arch(ground(-ANCHOR - 70.0, 0.0)),
            paints.steel.clone(),
        ),
    );
    // the red lights on the tops of the towers
    let beacons = [-HALF_SPAN, HALF_SPAN]
        .into_iter()
        .flat_map(|x| {
            [-CABLE_Z, CABLE_Z]
                .map(|z| Particle::new(vec3(x, TOWER_TOP + 3.0, z), 1.2, Rgb::hex(0xff2010) * 20.0))
        })
        .collect();
    scene.add_to(root, Node::particles(beacons, Blending::Additive));
    scene
}
