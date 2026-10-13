//! Part 1, 0 to 5 s: the bridge. Blue hour over a canyon 330 m wide and 200 m deep.
//!
//! The bridge runs along X over the canyon, which runs along Z. The road is at
//! [`DECK_Y`]. The village is on the +X rim. The camera looks along -Z, down the canyon,
//! towards the last light of the day.
//!
//! - 0.0: a close shot of the warm pilot cable. It crosses the picture at its middle.
//! - 0.15 to 2.1: the camera pulls back 480 m and shows the canyon.
//! - 0.85 to 2.4: two pylons rise from ledges in the canyon walls.
//! - 2.25 to 3.9: the deck grows out from both pylons, and the stay cables spin down to it.
//! - 3.9: the two halves meet. The pilot cable becomes the light strip of the road.
//! - 3.9 to 4.9: the lamps, the cables and the windows of the village light up.
//! - 4.25 to 5.0: the first car crosses. Its headlights flare up, and the flare is the
//!   match cut to the pen of the greenhouse.

use std::f32::consts::{FRAC_PI_2, PI};
use std::sync::Arc;

use fframes_3d::math::{hash, progress, smoothstep};
use fframes_3d::*;

use super::{
    AMBER, AMBER_EDGE, COLD, COLD_BLUE, camera, ease, ease_out, fbm, mix, noise2, sky, span,
};
use crate::assets::Assets;

/// The height of the road in meters. The rims of the canyon are at 0.
const DECK_Y: f32 = 8.0;
/// The height of the pilot cable, above the median of the road.
const PILOT_Y: f32 = DECK_Y + 1.4;
/// The X of the pylons. They stand on ledges in the canyon walls.
const TOWER_X: f32 = 140.0;
/// The height of the ledges under the pylons.
const TOWER_FOOT: f32 = -56.0;
/// The height where the pylon legs meet.
const APEX_Y: f32 = 62.0;
/// The top of the pylons.
const TOWER_TOP: f32 = 103.0;
/// The length of a deck segment.
const SEGMENT: f32 = 10.0;
/// The stay cables of each pylon on each side, in each cable plane.
const STAYS: usize = 13;
/// The Z of the cable anchors at the edges of the deck.
const STAY_Z: f32 = 10.6;
/// The X of the ends of the deck, where the road continues on land.
const DECK_END: f32 = 285.0;

const TOWERS_START: f32 = 0.85;
const TOWERS_END: f32 = 2.4;
const DECK_START: f32 = 2.25;
/// The two halves of the deck meet.
const CLOSURE: f32 = 3.9;
/// The time from one deck segment to the next.
const SEGMENT_TIME: f32 = (CLOSURE - DECK_START) / 14.0;
/// The car enters the bridge.
const CAR_START: f32 = 4.25;
/// The time-lapse speed of the car in m/s.
const CAR_SPEED: f32 = 540.0;

/// The X of the river at `z`: the canyon bends to +X in both directions.
fn river_x(z: f32) -> f32 {
    85.0 * (1.0 - (z / 340.0).cos()) + 18.0 * (z / 150.0).sin() * smoothstep(80.0, 400.0, z.abs())
}

/// The cross section of a wall, from the river up to the rim: the distance from the river
/// and the height in meters, and how far the noise moves the point.
const WALL: [(f32, f32, f32); 16] = [
    (7.0, -202.0, 0.0),
    (16.0, -198.6, 2.0),
    (40.0, -187.0, 6.0),
    (58.0, -171.0, 9.0),
    (63.0, -133.0, 11.0),
    (69.0, -125.0, 11.0),
    (92.0, -117.0, 13.0),
    (99.0, -86.0, 13.0),
    (104.0, -79.0, 12.0),
    (124.0, -70.0, 10.0),
    (128.0, -59.0, 9.0),
    (151.0, -56.0, 9.0),
    (154.0, -39.0, 9.0),
    (157.0, -6.0, 9.0),
    (161.0, -1.2, 8.0),
    (168.0, 0.0, 8.0),
];

/// The plateau beyond the rim: the distance from the river and the height.
const PLATEAU: [(f32, f32); 6] = [
    (168.0, 0.0),
    (230.0, 0.3),
    (420.0, 1.5),
    (900.0, 5.0),
    (1800.0, 16.0),
    (4200.0, 70.0),
];

/// Point `k` of the cross section of the wall on `side` (+1 or -1) at `z`, as the distance
/// from the river and the height. `k` indexes [`WALL`], then [`PLATEAU`] from 1. The noise
/// moves the cliffs and the ledges, less near the bridge, so the pylons keep their ledges.
fn profile_point(side: f32, z: f32, k: usize) -> Vec2 {
    let seed = if side > 0.0 { 11 } else { 47 };
    if k >= WALL.len() {
        let rim = profile_point(side, z, WALL.len() - 1);
        let (offset, height) = PLATEAU[k - WALL.len() + 1];
        return vec2(rim.x + offset - PLATEAU[0].0, height);
    }
    let near = 0.2 + 0.8 * smoothstep(25.0, 140.0, z.abs());
    let (offset, height, amount) = WALL[k];
    let shared = fbm(seed, z / 140.0, 0.5, 3);
    let own = fbm(seed + k as u32 * 13, z / 45.0, 1.5, 2);
    // side gullies: the upper walls step back where a dry wash comes in
    let gully = (1.0 - (2.0 * noise2(seed + 2, z / 90.0, 0.5) - 1.0).abs()).powi(6) * k as f32
        / WALL.len() as f32;
    let wander = (amount * (1.3 * shared + 0.6 * own) + 30.0 * gully) * near;
    let width = 1.0 + 0.08 * (1.0 - (z / 420.0).cos());
    let dip = if k == 0 || k + 1 == WALL.len() {
        0.0
    } else {
        3.0 * fbm(seed + 5 + k as u32, z / 210.0, 2.5, 2) * near
    };
    vec2(offset * width + wander, height + dip)
}

/// The world position of a cross section point.
fn wall_world(side: f32, z: f32, p: Vec2) -> Vec3 {
    vec3(river_x(z) + side * p.x, p.y, z)
}

/// The Z of the columns of the canyon mesh: dense near the bridge, coarse far away.
fn columns(from: f32, to: f32) -> Vec<f32> {
    let mut zs = vec![from];
    let mut z = from;
    while z < to {
        z = (z + (2.5 + (z.abs() - 160.0).max(0.0) * 0.022).min(40.0)).min(to);
        zs.push(z);
    }
    zs
}

/// The rows of a cross section: the index of a point and the fraction to the next point,
/// in steps of about `step` meters.
fn rows(points: &[Vec2], step: f32) -> Vec<(usize, f32)> {
    let mut out: Vec<(usize, f32)> = (0..points.len() - 1)
        .flat_map(|k| {
            let steps = ((points[k + 1] - points[k]).length() / step)
                .ceil()
                .max(1.0) as usize;
            (0..steps).map(move |s| (k, s as f32 / steps as f32))
        })
        .collect();
    out.push((points.len() - 1, 0.0));
    out
}

/// The color of the rock layers at the height `y`: red sandstone, buff and gray bands.
fn strata(y: f32, z: f32) -> Vec3 {
    let bands = [0xb07658, 0xd2ae86, 0x9c7d6d, 0xc28a62].map(|c| Rgb::hex(c).0);
    let s = (y + 210.0) / 23.0 + 0.6 * fbm(3, z / 300.0, y / 80.0, 2);
    let i = s.floor().rem_euclid(bands.len() as f32) as usize;
    let color = bands[i].lerp(
        bands[(i + 1) % bands.len()],
        smoothstep(0.75, 1.0, s.fract()),
    );
    // the bottom of the canyon gets less light from the sky
    color * mix(0.45, 1.0, smoothstep(-200.0, -40.0, y))
}

/// A grid mesh of columns with `rows` vertices each. The triangles face the side where the
/// columns go along +Z and the rows go away from the river on `side`.
fn grid(
    side: f32,
    rows: usize,
    positions: Vec<Vec3>,
    uvs: Vec<Vec2>,
    colors: Vec<Vec4>,
) -> Geometry {
    let columns = (positions.len() / rows) as u32;
    let r = rows as u32;
    let mut indices = Vec::new();
    for i in 0..columns - 1 {
        for j in 0..r - 1 {
            let (a, b, c, d) = (
                i * r + j,
                (i + 1) * r + j,
                (i + 1) * r + j + 1,
                i * r + j + 1,
            );
            if side > 0.0 {
                indices.extend([a, b, d, b, c, d]);
            } else {
                indices.extend([a, d, b, b, d, c]);
            }
        }
    }
    let mut g = Geometry::new(positions, Vec::new(), uvs, indices);
    g.colors = colors;
    g
}

/// One wall of the canyon from the river to the rim, and the plateau beyond the rim.
fn canyon_side(side: f32, zs: &[f32]) -> (Geometry, Geometry) {
    let seed = if side > 0.0 { 201 } else { 307 };
    let base: Vec<Vec2> = WALL.iter().map(|&(x, y, _)| vec2(x, y)).collect();
    let wall_rows = rows(&base, 3.5);
    let flat: Vec<Vec2> = PLATEAU.iter().map(|&(x, y)| vec2(x, y)).collect();
    let plateau_rows = rows(&flat, 40.0);

    let (mut wall, mut wall_uv, mut wall_color) = (Vec::new(), Vec::new(), Vec::new());
    let (mut ground, mut ground_uv, mut ground_color) = (Vec::new(), Vec::new(), Vec::new());
    for &z in zs {
        let points: Vec<Vec2> = (0..WALL.len() + PLATEAU.len() - 1)
            .map(|k| profile_point(side, z, k))
            .collect();
        let at = |k: usize, f: f32| points[k].lerp(points[(k + 1).min(points.len() - 1)], f);
        let mut arc = 0.0;
        let mut last: Option<Vec3> = None;
        for &(k, f) in &wall_rows {
            let p = at(k, f);
            // rough rock: the wall moves in and out by up to 2 m, but the rim stays clean
            let rough = 2.4 * fbm(seed, z / 8.0, p.y / 8.0, 3) * smoothstep(-2.0, -10.0, p.y);
            let world = wall_world(side, z, p + vec2(rough, 0.0));
            arc += last.map_or(0.0, |l| l.distance(world));
            last = Some(world);
            wall.push(world);
            wall_uv.push(vec2(z, arc) / 3.0);
            wall_color.push(strata(world.y, z).extend(1.0));
        }
        let rim = points[WALL.len() - 1];
        for &(k, f) in &plateau_rows {
            let p = at(WALL.len() - 1 + k, f);
            let out = p.x - rim.x;
            let swell = 1.6 * fbm(seed + 3, z / 90.0, out / 90.0, 2) * smoothstep(5.0, 60.0, out);
            ground.push(wall_world(side, z, p) + vec3(0.0, swell, 0.0));
            ground_uv.push(vec2(z, side * out));
            let dust = 0.8 + 0.2 * noise2(seed, z / 40.0, out / 40.0);
            ground_color.push((Rgb::hex(0xc8a07a).0 * dust).extend(1.0));
        }
    }
    (
        grid(side, wall_rows.len(), wall, wall_uv, wall_color),
        grid(side, plateau_rows.len(), ground, ground_uv, ground_color),
    )
}

/// A point on the wall of `side` at `z`, `f` of the way from profile point `k` to the
/// next. Rocks and the pylon feet stand on these points.
fn on_wall(side: f32, z: f32, k: usize, f: f32) -> Vec3 {
    let p = profile_point(side, z, k).lerp(profile_point(side, z, k + 1), f);
    wall_world(side, z, p)
}

/// A box of `width` x `depth` from `a` to `b`: one lift of a pylon. Its X stays along the
/// world X.
fn lift(a: Vec3, b: Vec3, width: f32, depth: f32) -> Mat4 {
    let d = b - a;
    let rotation = Quat::from_rotation_arc(Vec3::Y, d.normalize_or(Vec3::Y));
    Mat4::from_scale_rotation_translation(vec3(width, d.length(), depth), rotation, a)
}

/// The lifts of one pylon at `x`, in the order of their height: each is a transform of the
/// unit box on Y = 0..1, and the height of its bottom.
fn pylon(x: f32) -> Vec<(Mat4, f32)> {
    let mut lifts = vec![(
        lift(
            vec3(x, TOWER_FOOT - 4.0, 0.0),
            vec3(x, TOWER_FOOT + 1.0, 0.0),
            13.0,
            16.0,
        ),
        TOWER_FOOT - 4.0,
    )];
    let mut leg = |a: Vec3, b: Vec3, width: f32, depth: f32| {
        let n = (a.distance(b) / 5.2).ceil() as usize;
        for i in 0..n {
            let p = a.lerp(b, i as f32 / n as f32);
            let q = a.lerp(b, (i + 1) as f32 / n as f32);
            lifts.push((lift(p, q, width, depth), p.y));
        }
    };
    let knee = DECK_Y - 3.6;
    for z in [-1.0f32, 1.0] {
        leg(
            vec3(x, TOWER_FOOT + 1.0, z * 3.6),
            vec3(x, knee, z * 14.8),
            4.6,
            3.4,
        );
        leg(vec3(x, knee, z * 14.8), vec3(x, APEX_Y, z * 2.7), 4.2, 3.2);
    }
    leg(
        vec3(x, APEX_Y - 3.0, 0.0),
        vec3(x, TOWER_TOP, 0.0),
        7.4,
        5.8,
    );
    lifts.push((
        lift(
            vec3(x, TOWER_TOP, 0.0),
            vec3(x, TOWER_TOP + 2.2, 0.0),
            6.2,
            4.6,
        ),
        TOWER_TOP,
    ));
    // the cross beam under the deck
    lifts.push((
        lift(
            vec3(x, knee - 1.6, -14.8),
            vec3(x, knee - 1.6, 14.8),
            5.0,
            3.2,
        ),
        knee - 1.0,
    ));
    lifts.sort_by(|a, b| a.1.total_cmp(&b.1));
    lifts
}

/// A deck segment that grows out from the end nearer to its pylon.
struct Piece {
    /// The X of the center.
    x: f32,
    /// The direction of growth along X, away from the pylon.
    dir: f32,
    /// The time the segment starts to grow.
    start: f32,
    /// The cantilever front that the segment extends, `0..4`. `None` for the pier tables
    /// and the closing segment.
    front: Option<usize>,
}

/// A stay cable that spins down from the pylon to the deck.
struct Stay {
    top: Vec3,
    bottom: Vec3,
    start: f32,
}

/// A window or a street lamp of the village.
struct Window {
    transform: Mat4,
    /// The time its light comes on.
    on: f32,
    /// From amber (0) to cream (1).
    warmth: f32,
}

/// The node ids that the frames animate.
struct Ids {
    lifts: [NodeId; 2],
    deck: [NodeId; 3],
    lamps: NodeId,
    stays: NodeId,
    stay_glow: NodeId,
    pilot: NodeId,
    strip: NodeId,
    sparks: NodeId,
    cranes: [NodeId; 2],
    crane_lights: [NodeId; 2],
    travellers: NodeId,
    tip_lights: [NodeId; 4],
    windows: NodeId,
    village_lights: [NodeId; 3],
    deck_lights: [NodeId; 2],
    car: NodeId,
    car_lights: NodeId,
}

/// The canyon, the bridge and the village at blue hour.
pub struct Bridge {
    scene: Scene,
    ids: Ids,
    lifts: [Vec<(Mat4, f32)>; 2],
    pieces: Vec<Piece>,
    stays: Vec<Stay>,
    windows: Vec<Window>,
    landmark: Scene,
}

/// Builds the scene: the canyon from `extent.0` to `extent.1` along Z, with the plateau
/// when `plateau` is set, the bridge parts, the village and the lights.
fn build(assets: &Assets, extent: (f32, f32), plateau: bool) -> (Scene, Ids, Vec<Window>) {
    let mut scene = Scene::new()
        .environment(Environment::map(
            sky(
                Rgb::hex(0x0d1a3c) * 0.9,
                Rgb::hex(0x5a7cb4) * 0.42,
                Rgb::hex(0xff8f52) * 0.6,
                -1.35,
            ),
            1.0,
        ))
        .background(Background::Environment { blur: 0.0 });
    add_canyon(&mut scene, assets, extent, plateau);

    // the pylons, built lift by lift
    let concrete = assets.concrete.material(0xc4c2bc, 0.9, 0.0);
    let unit_box = Arc::new(Geometry::rounded_box(1.0, 1.0, 1.0, 0.05, 2).translate(0.0, 0.5, 0.0));
    let lift_ids = [0, 1].map(|_| {
        scene.add(Node::instanced(
            unit_box.clone(),
            concrete.clone(),
            Instances::default(),
        ))
    });

    // the deck: girder, road, and steel trim, one copy for each segment
    let deck = deck_segment();
    let deck_ids = [
        (deck.0, assets.concrete.material(0xb8b6b0, 0.85, 0.0)),
        (deck.1, assets.asphalt.material(0x8a8a8a, 0.9, 0.0)),
        (deck.2, Material::metal(0x9aa0a8, 0.45)),
    ]
    .map(|(g, m)| scene.add(Node::instanced(g, m, Instances::default())));
    let lamps = scene.add(
        Node::instanced(deck.3, Material::unlit(Rgb::WHITE), Instances::default())
            .shadows(false, false),
    );

    // the stay cables, and the warm light that runs up them when the bridge is done
    let cable = Arc::new(Geometry::cylinder(1.0, 1.0, 1.0, 6).translate(0.0, 0.5, 0.0));
    let stay_node = scene.add(Node::instanced(
        cable.clone(),
        Material::standard(0xe6e8ea).roughness(0.35),
        Instances::default(),
    ));
    let stay_glow = scene.add(
        Node::instanced(
            cable.clone(),
            Material::unlit(Rgb::WHITE).additive().no_fog(),
            Instances::default(),
        )
        .shadows(false, false),
    );

    // the pilot cable: a unit tube along X that the frames scale to stay visible
    let pilot = scene.add(
        Node::mesh(
            Geometry::tube(&[vec3(-330.0, 0.0, 0.0), vec3(330.0, 0.0, 0.0)], 1.0, 10),
            Material::unlit(Rgb::hex(AMBER) * 9.0),
        )
        .at(0.0, PILOT_Y, 0.0)
        .shadows(false, false),
    );
    // the light strip of the road that the pilot cable becomes
    let strip = scene.add(
        Node::mesh(
            Geometry::cuboid(2.0 * DECK_END, 0.12, 0.28),
            Material::unlit(Rgb::hex(AMBER_EDGE) * 7.0),
        )
        .at(0.0, DECK_Y + 0.85, 0.0)
        .shadows(false, false)
        .hidden(),
    );
    let sparks = scene.add(Node::particles(Vec::new(), Blending::Additive));

    // a climbing crane on the top of each pylon: it rises with the pylon
    let crane_yellow = Material::standard(0xb89a3a).roughness(0.6);
    let mut cranes = [NodeId(0); 2];
    let mut crane_lights = [NodeId(0); 2];
    for (i, x) in [-TOWER_X, TOWER_X].into_iter().enumerate() {
        cranes[i] = scene.add(Node::group().at(x, 0.0, 0.0));
        let crane = Geometry::merge([
            Geometry::cuboid(1.6, 14.0, 1.6).translate(0.0, 7.0, 0.0),
            Geometry::cuboid(34.0, 1.3, 1.3).translate(15.0, 14.0, 0.0),
            Geometry::cuboid(11.0, 1.1, 1.1).translate(-6.0, 14.0, 0.0),
            Geometry::cuboid(0.8, 4.5, 0.8).translate(0.0, 16.2, 0.0),
        ]);
        scene.add_to(cranes[i], Node::mesh(crane, crane_yellow.clone()));
        scene.add_to(
            cranes[i],
            Node::mesh(
                Geometry::cuboid(3.0, 2.4, 2.0),
                Material::metal(0x3a3e44, 0.55),
            )
            .at(-10.0, 12.6, 0.0),
        );
        scene.add_to(
            cranes[i],
            Node::mesh(Geometry::sphere(0.45, 8, 6), Material::glow(COLD, 16.0))
                .at(31.5, 13.2, 0.0),
        );
        crane_lights[i] = scene.add_to(
            cranes[i],
            Node::light(Light::point(COLD, 0.0, 120.0)).at(20.0, 11.0, 0.0),
        );
    }

    // the form travellers at the four tips of the deck, with cold flood lights
    let traveller = Geometry::merge([
        Geometry::cuboid(1.0, 6.0, 1.0).translate(0.0, 3.0, -11.0),
        Geometry::cuboid(1.0, 6.0, 1.0).translate(0.0, 3.0, 11.0),
        Geometry::cuboid(1.2, 1.2, 24.0).translate(0.0, 6.2, 0.0),
        Geometry::cuboid(6.0, 0.8, 1.0).translate(-2.5, 6.2, -11.0),
        Geometry::cuboid(6.0, 0.8, 1.0).translate(-2.5, 6.2, 11.0),
    ])
    .with_color(Rgb::hex(0x4a5058));
    let floods = Geometry::cuboid(0.6, 0.3, 22.0)
        .translate(0.4, 5.4, 0.0)
        .with_color(Rgb::hex(COLD) * 12.0);
    let travellers = scene.add(Node::instanced(
        Geometry::merge([traveller, floods]),
        Material::unlit(Rgb::WHITE),
        Instances::default(),
    ));
    let tip_lights =
        [0; 4].map(|_| scene.add(Node::light(Light::point(COLD, 6000.0, 120.0)).hidden()));

    // the village on the +X rim, and the road to it
    let (windows, village_lights) = add_village(&mut scene, assets);
    let window_geometry = Geometry::plane(1.0, 1.0);
    let window_ids = scene.add(
        Node::instanced(
            window_geometry,
            Material::unlit(Rgb::WHITE),
            Instances::default(),
        )
        .shadows(false, false),
    );
    let deck_lights = [-1.0, 1.0].map(|f: f32| {
        scene.add(
            Node::light(Light::point(AMBER, 0.0, 220.0))
                .at(f * 150.0, DECK_Y + 10.0, 0.0)
                .hidden(),
        )
    });

    // the first car: a dark body with head and tail lights
    let car = scene.add(Node::group().hidden());
    scene.add_to(
        car,
        Node::mesh(
            Geometry::rounded_box(4.6, 1.4, 1.9, 0.3, 2).translate(0.0, 0.9, 0.0),
            Material::standard(0x1a1c20).roughness(0.3).clearcoat(1.0),
        ),
    );
    // the head lights with their glare just in front of the car, and the tail lights
    let car_lights = scene.add_to(
        car,
        Node::particles(
            vec![
                Particle::new(vec3(3.2, 0.8, 0.0), 3.0, Rgb::hex(AMBER) * 3.0),
                Particle::new(vec3(2.4, 0.8, 0.65), 0.45, Rgb::hex(0xfff0d8) * 70.0),
                Particle::new(vec3(2.4, 0.8, -0.65), 0.45, Rgb::hex(0xfff0d8) * 70.0),
                Particle::new(vec3(-2.4, 0.9, 0.7), 0.3, Rgb::hex(0xff2a10) * 20.0),
                Particle::new(vec3(-2.4, 0.9, -0.7), 0.3, Rgb::hex(0xff2a10) * 20.0),
            ],
            Blending::Additive,
        ),
    );
    scene.add_to(
        car,
        Node::light(Light::spot(0xffe2b8, 3.0e4, 0.25, 0.6).pointing(vec3(1.0, -0.05, 0.0)))
            .at(2.6, 0.8, 0.0),
    );

    // the afterglow of the sun, low over the far end of the canyon
    scene.add(Node::light(
        Light::sun(0xffa070, 0.8, vec3(0.75, 0.08, -1.0)).with_shadow(Shadow {
            map_size: 2048,
            softness: 2.5,
            strength: 0.9,
            bias: 2.0,
            bounds: Some(Aabb {
                min: vec3(-330.0, -210.0, -260.0),
                max: vec3(330.0, 115.0, 120.0),
            }),
        }),
    ));
    // the sky light: blue from above, so the ledges stand out from the cliffs
    scene.add(Node::light(Light::hemisphere(0x6a88c8, 0x1a1612, 1.1)));
    scene.atmosphere = Some(Atmosphere {
        density: 0.0016,
        color: Rgb::WHITE,
        base: -200.0,
        falloff: 0.007,
        anisotropy: 0.45,
        ambient: Rgb::hex(0x2c4c92) * 0.24,
        noise: 0.5,
        noise_size: 70.0,
        offset: Vec3::ZERO,
        distance: 2600.0,
        steps: 32,
    });
    let ids = Ids {
        lifts: lift_ids,
        deck: deck_ids,
        lamps,
        stays: stay_node,
        stay_glow,
        pilot,
        strip,
        sparks,
        cranes,
        crane_lights,
        travellers,
        tip_lights,
        windows: window_ids,
        village_lights,
        deck_lights,
        car,
        car_lights,
    };
    (scene, ids, windows)
}

impl Bridge {
    pub fn new(assets: &Assets) -> Self {
        let (scene, ids, windows) = build(assets, (-4200.0, 1400.0), true);

        // the deck segments: a pier table at each pylon, then pairs that grow outwards in
        // balance, then the segment that closes the gap in the middle
        let mut pieces = Vec::new();
        for (i, x) in [-TOWER_X, TOWER_X].into_iter().enumerate() {
            let out = x.signum();
            pieces.push(Piece {
                x,
                dir: -out,
                start: DECK_START,
                front: None,
            });
            for k in 1..=14 {
                let start = DECK_START + k as f32 * SEGMENT_TIME;
                let d = k as f32 * SEGMENT;
                if k < 14 {
                    pieces.push(Piece {
                        x: x - out * d,
                        dir: -out,
                        start,
                        front: Some(2 * i),
                    });
                }
                pieces.push(Piece {
                    x: x + out * d,
                    dir: out,
                    start,
                    front: Some(2 * i + 1),
                });
            }
        }
        pieces.push(Piece {
            x: 0.0,
            dir: 1.0,
            start: CLOSURE - SEGMENT_TIME,
            front: None,
        });

        // the stays in two planes, from anchors on the shaft to the edges of the deck
        let mut stays = Vec::new();
        for x in [-TOWER_X, TOWER_X] {
            for k in 1..=STAYS {
                let start = DECK_START + (k as f32 + 0.7) * SEGMENT_TIME;
                let height = 65.0 + (k - 1) as f32 * (TOWER_TOP - 4.0 - 65.0) / (STAYS - 1) as f32;
                for dir in [-1.0f32, 1.0] {
                    for z in [-1.0f32, 1.0] {
                        stays.push(Stay {
                            top: vec3(x + dir * 2.2, height, z * 1.4),
                            bottom: vec3(
                                x + dir * (k as f32 * SEGMENT + 4.0),
                                DECK_Y + 0.4,
                                z * STAY_Z,
                            ),
                            start,
                        });
                    }
                }
            }
        }

        let mut bridge = Self {
            scene,
            ids,
            lifts: [pylon(-TOWER_X), pylon(TOWER_X)],
            pieces,
            stays,
            windows,
            landmark: Scene::new(),
        };
        // shot 9 sees the finished bridge from kilometers away: the canyon is short, and
        // the shot has its own ground
        let (mut landmark, ids, _) = build(assets, (-450.0, 450.0), false);
        let far = camera(50.0, vec3(0.0, 100.0, 3000.0), Vec3::ZERO);
        bridge.pose(&mut landmark, 4.9, &far);
        landmark[ids.car].visible = false;
        // the lights stay out, so they do not count against the lights of shot 9
        let lights: Vec<NodeId> = landmark
            .nodes()
            .filter(|(_, node)| matches!(node.object, Object::Light(_)))
            .map(|(id, _)| id)
            .collect();
        for id in lights {
            landmark[id].visible = false;
        }
        bridge.landmark = landmark;
        bridge
    }

    /// The scene and the camera at `t` seconds, `0..5`.
    pub fn moment(&self, t: f32) -> (Scene, Camera) {
        let camera = camera_at(t);
        let mut scene = self.scene.clone();
        self.pose(&mut scene, t, &camera);
        if let Some(air) = &mut scene.atmosphere {
            air.offset = vec3(t * 9.0, 0.0, t * 4.0);
        }
        (scene, camera)
    }

    /// The finished bridge and the village with their lit windows for shot 9, see
    /// [`super::Landmarks`].
    pub fn landmark(&self) -> Scene {
        self.landmark.clone()
    }

    /// Sets the parts of `scene` to the state at `t`. `camera` sets the width of the pilot
    /// cable.
    fn pose(&self, scene: &mut Scene, t: f32, camera: &Camera) {
        let ids = &self.ids;
        let mut sparks = Vec::new();
        let set = |scene: &mut Scene, id: NodeId, instances: Instances| {
            if let Some(mesh) = scene[id].mesh_mut() {
                mesh.instances = Some(Arc::new(instances));
            }
        };

        // the pylons rise lift by lift, with a crane and cold lights at the top
        let rise = progress(t, TOWERS_START, TOWERS_END);
        for (i, lifts) in self.lifts.iter().enumerate() {
            let bottom = lifts[0].1;
            let mut transforms = Vec::new();
            let mut top = TOWER_FOOT;
            for (m, y) in lifts {
                let start = TOWERS_START
                    + (y - bottom) / (TOWER_TOP - bottom) * (TOWERS_END - TOWERS_START - 0.12);
                let g = progress(t, start, start + 0.12);
                if g > 0.0 {
                    let part = *m * Mat4::from_scale(vec3(1.0, g.max(0.02), 1.0));
                    top = top.max(part.transform_point3(Vec3::Y).y);
                    transforms.push(part);
                }
            }
            set(scene, ids.lifts[i], Instances::new(transforms));
            let x = if i == 0 { -TOWER_X } else { TOWER_X };
            let crane = &mut scene[ids.cranes[i]];
            let retract = smoothstep(CLOSURE - 0.1, CLOSURE + 0.35, t);
            crane.visible = top > TOWER_FOOT + 2.0 && retract < 1.0;
            crane.position.y = top - 20.0 * ease(retract);
            crane.rotation = Quat::from_rotation_y(
                if i == 0 { 0.4 } else { 2.7 } + 0.6 * (t * 1.1 + i as f32).sin(),
            );
            let working = smoothstep(TOWERS_START - 0.2, TOWERS_START, t)
                * (1.0 - smoothstep(CLOSURE, CLOSURE + 0.4, t));
            scene[ids.crane_lights[i]].visible = working > 0.0;
            if let Some(light) = scene[ids.crane_lights[i]].light_mut() {
                *light.intensity_mut() = 5000.0 * working;
            }
            if rise > 0.0 && rise < 1.0 {
                for j in 0..6u32 {
                    let seed = i as u32 * 31 + j;
                    let a = t * 7.0 + hash(seed) * 6.3;
                    let p = vec3(
                        x + 6.0 * a.cos(),
                        top + 1.0 + 2.0 * hash(seed + 5),
                        5.0 * a.sin(),
                    );
                    let blink = if (t * 9.0 + hash(seed + 9) * 3.0).fract() < 0.5 {
                        30.0
                    } else {
                        6.0
                    };
                    sparks.push(Particle::new(p, 0.35, Rgb::hex(COLD_BLUE) * blink));
                }
            }
        }

        // the deck segments, the lamps on them, and the form travellers at the tips
        let mut deck = Vec::new();
        let mut lamp_colors = Vec::new();
        let mut tips: [Option<(f32, f32)>; 4] = [None; 4];
        for piece in &self.pieces {
            let g = progress(t, piece.start, piece.start + SEGMENT_TIME * 0.9);
            if g <= 0.0 {
                continue;
            }
            let near = piece.x - piece.dir * SEGMENT * 0.5;
            deck.push(
                Mat4::from_translation(vec3(near, DECK_Y, 0.0))
                    * Mat4::from_scale(vec3(ease_out(g).max(0.02), 1.0, 1.0))
                    * Mat4::from_translation(vec3(piece.dir * SEGMENT * 0.5, 0.0, 0.0)),
            );
            if let Some(front) = piece.front {
                tips[front] = Some((near + piece.dir * SEGMENT * ease_out(g), piece.dir));
            }
            let on = CLOSURE + 0.15 + piece.x.abs() / DECK_END * 0.45;
            lamp_colors
                .push(Rgb::hex(0x0c0d10).lerp(Rgb::hex(AMBER) * 9.0, smoothstep(on, on + 0.1, t)));
        }
        for id in ids.deck {
            set(scene, id, Instances::new(deck.clone()));
        }
        set(scene, ids.lamps, Instances::new(deck).colors(lamp_colors));

        // a traveller rides each front until the front is done
        let tips: Vec<(f32, f32)> = tips
            .into_iter()
            .flatten()
            .filter(|(x, _)| t < CLOSURE && x.abs() < DECK_END - 4.0)
            .collect();
        set(
            scene,
            ids.travellers,
            Instances::new(tips.iter().map(|&(x, dir)| {
                Mat4::from_translation(vec3(x - dir * 3.0, DECK_Y, 0.0))
                    * Mat4::from_rotation_y(if dir < 0.0 { PI } else { 0.0 })
            })),
        );
        for (i, id) in ids.tip_lights.into_iter().enumerate() {
            let tip = tips.get(i);
            scene[id].visible = tip.is_some();
            if let Some(&(x, dir)) = tip {
                scene[id].position = vec3(x - dir * 2.0, DECK_Y + 7.0, 0.0);
            }
        }

        // the stays spin from the pylon down to the deck, with a cold crawler at the end
        let mut cables = Vec::new();
        let mut glow = Vec::new();
        let mut glow_colors = Vec::new();
        for (n, stay) in self.stays.iter().enumerate() {
            let g = progress(t, stay.start, stay.start + 0.22);
            if g <= 0.0 {
                continue;
            }
            let end = stay.top.lerp(stay.bottom, ease_out(g));
            cables.push(span(stay.top, end, 0.17));
            if g < 1.0 {
                sparks.push(Particle::new(end, 0.5, Rgb::hex(COLD) * 40.0));
            }
            let k = (n / 4) % STAYS;
            let up = progress(
                t,
                CLOSURE + 0.3 + k as f32 * 0.025,
                CLOSURE + 0.75 + k as f32 * 0.025,
            );
            if up > 0.0 {
                glow.push(span(
                    stay.bottom,
                    stay.bottom.lerp(stay.top, ease(up)),
                    0.28,
                ));
                glow_colors.push(Rgb::hex(AMBER) * 0.65);
            }
        }
        set(scene, ids.stays, Instances::new(cables));
        set(
            scene,
            ids.stay_glow,
            Instances::new(glow).colors(glow_colors),
        );

        // the pilot cable keeps a width of about one pixel when the camera pulls back, and
        // sinks into the road when the deck closes. `view` is the height of the picture
        // at the distance of the cable, in meters.
        let to_line = vec2(camera.position.y - PILOT_Y, camera.position.z).length();
        let view = 2.0 * to_line * (camera.fov.to_radians() * 0.5).tan();
        let radius = (0.0008 * view).max(0.016);
        let sink = smoothstep(CLOSURE, CLOSURE + 0.3, t);
        let pilot = &mut scene[ids.pilot];
        pilot.scale = vec3(1.0, radius, radius);
        pilot.position.y = PILOT_Y - 0.55 * sink;
        pilot.visible = sink < 1.0;
        let strip_on = smoothstep(CLOSURE + 0.05, CLOSURE + 0.4, t);
        let strip = &mut scene[ids.strip];
        strip.visible = strip_on > 0.0;
        strip.scale = vec3(strip_on.max(0.01), 1.0, 1.0);

        // the windows of the village come on one by one
        let mut windows = Vec::new();
        let mut colors = Vec::new();
        for w in &self.windows {
            windows.push(w.transform);
            let on = smoothstep(w.on, w.on + 0.06, t);
            let warm = Rgb::hex(AMBER).lerp(Rgb::hex(0xffd9a0), w.warmth) * 7.0;
            colors.push(Rgb::hex(0x06080c).lerp(warm, on));
        }
        set(scene, ids.windows, Instances::new(windows).colors(colors));
        for (i, id) in ids.village_lights.into_iter().enumerate() {
            let on = smoothstep(
                CLOSURE + 0.45 + i as f32 * 0.18,
                CLOSURE + 0.8 + i as f32 * 0.18,
                t,
            );
            scene[id].visible = on > 0.0;
            if let Some(light) = scene[id].light_mut() {
                *light.intensity_mut() = 4500.0 * on;
            }
        }
        for (i, id) in ids.deck_lights.into_iter().enumerate() {
            let on = smoothstep(CLOSURE + 0.35 + i as f32 * 0.1, CLOSURE + 0.7, t);
            scene[id].visible = on > 0.0;
            if let Some(light) = scene[id].light_mut() {
                *light.intensity_mut() = 6000.0 * on;
            }
        }

        // the first car
        let car = &mut scene[ids.car];
        car.visible = t > CAR_START;
        car.position = car_position(t);
        // the head lights flare up before the cut: the flare becomes the pen of the
        // greenhouse
        let flare = super::ease_in(progress(t, 4.62, 5.0));
        // the glare is a lens effect: it sits 25 m nearer to the camera on the same ray,
        // so the deck does not hide it
        let to_camera = (camera.position - car_position(t)).normalize();
        if let Object::Particles(p) = &mut scene[ids.car_lights].object {
            p.items[0].position = vec3(2.4, 0.8, 0.0) + to_camera * 25.0;
            p.items[0].size = 3.0 + 8.0 * flare;
            p.items[0].color = Rgb::hex(AMBER) * (3.0 + 32.0 * flare);
        }

        // red aviation lights on the tops of the finished pylons
        if t > TOWERS_END && (t * 1.2).fract() < 0.5 {
            for x in [-TOWER_X, TOWER_X] {
                sparks.push(Particle::new(
                    vec3(x, TOWER_TOP + 2.6, 0.0),
                    0.6,
                    Rgb::hex(0xff2010) * 25.0,
                ));
            }
        }

        if let Some(node) = scene.get_mut(ids.sparks)
            && let Object::Particles(p) = &mut node.object
        {
            p.items = sparks;
        }
    }
}

/// The position of the first car at `t`: it crosses at a time-lapse speed on the lane
/// nearer to the camera.
fn car_position(t: f32) -> Vec3 {
    let x = -DECK_END - 60.0 + CAR_SPEED * (t - CAR_START);
    vec3(x, DECK_Y + 0.05, 5.4)
}

/// The camera at `t`. It starts 10 m from the pilot cable with a long lens, pulls back
/// 480 m over the canyon and rises, drifts while the bridge grows, and at the end leans
/// after the headlights of the first car.
fn camera_at(t: f32) -> Camera {
    let line = vec3(0.0, PILOT_Y, 0.0);
    let wide = vec3(25.0, 26.0, 480.0);
    let wide_target = vec3(10.0, -16.0, -100.0);
    let settled = vec3(16.0, 21.0, 440.0);
    let settled_target = vec3(14.0, -8.0, -100.0);

    // the pull back: the distance grows by a factor of 48, evenly in log space. The
    // camera looks up at the cable first, against the sky, then rises above it.
    let pull = ease(progress(t, 0.12, 2.1));
    let distance = (10.0f32.ln() + (wide.z.ln() - 10.0f32.ln()) * pull).exp();
    let slope = mix(
        -0.34,
        (wide.y - PILOT_Y) / wide.z,
        ease(progress(t, 0.15, 1.6)),
    );
    let mut position = vec3(wide.x * pull, PILOT_Y + slope * distance, distance);
    let tilt = ease(progress(t, 0.9, 2.3));
    let mut target = line.lerp(wide_target, tilt);
    let mut mm = mix(85.0, 24.0, ease(progress(t, 0.1, 1.9)));

    // the slow drift while the bridge grows
    let drift = ease(progress(t, 2.1, 5.4));
    position += (settled - wide) * drift;
    target += (settled_target - wide_target) * drift;
    mm = mix(mm, 26.0, ease(progress(t, 3.6, 5.0)));

    // the camera leans a little after the first car's headlights
    let follow = ease(progress(t, CAR_START - 0.2, 5.3)) * 0.2;
    target = target.lerp(car_position(t), follow);

    let focus = line.lerp(target, tilt);
    let blur = mix(9.0, 3.0, ease(progress(t, 0.1, 1.2)));
    camera(mm, position, target)
        .focus_on(focus, blur)
        .shake(t, 0.0005, 5)
}

/// The height of the plateau `out` meters beyond the rim on `side` at `z`.
fn ground_height(side: f32, z: f32, out: f32) -> f32 {
    let seed = if side > 0.0 { 204 } else { 310 };
    let x = out + PLATEAU[0].0;
    let i = PLATEAU
        .iter()
        .rposition(|p| p.0 <= x)
        .unwrap_or(0)
        .min(PLATEAU.len() - 2);
    let (a, b) = (PLATEAU[i], PLATEAU[i + 1]);
    let base = mix(a.1, b.1, ((x - a.0) / (b.0 - a.0)).clamp(0.0, 1.0));
    base + 1.6 * fbm(seed, z / 90.0, out / 90.0, 2) * smoothstep(5.0, 60.0, out)
}

/// The X of the rim on `side` at `z`.
fn rim_x(side: f32, z: f32) -> f32 {
    wall_world(side, z, profile_point(side, z, WALL.len() - 1)).x
}

/// The canyon walls from `extent.0` to `extent.1` along Z, the river, the boulders on the
/// ledges, and the plateau with its shrubs when `plateau` is set. The nodes are the same
/// in both cases, so the ids of later nodes do not change.
fn add_canyon(scene: &mut Scene, assets: &Assets, extent: (f32, f32), plateau: bool) {
    let zs = columns(extent.0, extent.1);
    let rock = assets.rock.material(0xffffff, 1.0, 0.0).normal_scale(1.4);
    let dirt = assets.dirt.material(0xffffff, 1.0, 0.0);
    for side in [-1.0f32, 1.0] {
        let (wall, ground) = canyon_side(side, &zs);
        scene.add(Node::mesh(wall, rock.clone()));
        let ground = if plateau { ground } else { Geometry::default() };
        scene.add(Node::mesh(ground, dirt.clone()).shadows(false, true));
    }

    // the river: dark water that shows the sky
    let mut water = Vec::new();
    let mut uvs = Vec::new();
    for &z in &zs {
        for dx in [-15.0, 15.0] {
            water.push(vec3(river_x(z) + dx, -199.7, z));
            uvs.push(vec2(dx, z));
        }
    }
    let colors = vec![Vec4::ONE; water.len()];
    scene.add(
        Node::mesh(
            grid(1.0, 2, water, uvs, colors),
            Material::standard(0x0b161c).roughness(0.06),
        )
        .shadows(false, true),
    );

    // boulders on the talus slopes and the ledges, fewer far away
    let ledges = [1, 2, 2, 5, 8, 10, 10];
    let mut transforms = Vec::new();
    let mut tints = Vec::new();
    for i in 0..2400u32 {
        let side = if hash(i * 7 + 1) < 0.5 { -1.0 } else { 1.0 };
        let z = mix(
            extent.0.max(-1400.0),
            extent.1.min(500.0),
            hash(i * 7 + 2).powf(0.8),
        );
        let k = ledges[(hash(i * 7 + 3) * ledges.len() as f32) as usize % ledges.len()];
        if k == 10 && z.abs() < 30.0 {
            continue;
        }
        let p = on_wall(side, z, k, hash(i * 7 + 4));
        let size = 0.7 + 5.0 * hash(i * 7 + 5).powi(3);
        let rotation = euler(hash(i * 7 + 6) * 6.3, hash(i) * 6.3, 0.3);
        transforms.push(Mat4::from_scale_rotation_translation(
            vec3(size * 1.2, size * 0.7, size),
            rotation,
            p + vec3(0.0, size * 0.2, 0.0),
        ));
        tints.push(Rgb(strata(p.y, z)));
    }
    scene.add(Node::instanced(
        Geometry::icosahedron(1.0, 1).flat(),
        assets.rock.material(0xffffff, 1.0, 0.0),
        Instances::new(transforms).colors(tints),
    ));

    // desert shrubs on both rims
    if plateau {
        let mut transforms = Vec::new();
        for i in 0..1600u32 {
            let side = if hash(i * 5 + 1) < 0.5 { -1.0 } else { 1.0 };
            let z = mix(-900.0, 500.0, hash(i * 5 + 2));
            let out = 3.0 + 500.0 * hash(i * 5 + 3).powi(2);
            let x = rim_x(side, z) + side * out;
            let size = 0.5 + 0.9 * hash(i * 5 + 4);
            transforms.push(Mat4::from_scale_rotation_translation(
                vec3(size * 1.3, size * 0.8, size * 1.2),
                Quat::from_rotation_y(hash(i) * 6.3),
                vec3(x, ground_height(side, z, out) + size * 0.3, z),
            ));
        }
        scene.add(
            Node::instanced(
                Geometry::icosahedron(1.0, 0).flat(),
                Material::standard(0x4a4a30).roughness(0.95),
                Instances::new(transforms),
            )
            .shadows(false, true),
        );
    } else {
        // an empty node keeps the ids of the later nodes the same
        scene.add(Node::group());
    }
}

/// One deck segment along X, centered on X = 0 with the road at Y = 0: the girder, the
/// road, the steel trim, and the lamp heads.
fn deck_segment() -> (Geometry, Geometry, Geometry, Geometry) {
    let section = [
        vec2(-7.0, -3.2),
        vec2(7.0, -3.2),
        vec2(11.7, -1.0),
        vec2(11.7, 0.0),
        vec2(-11.7, 0.0),
        vec2(-11.7, -1.0),
    ];
    let girder = Geometry::extrude(&section, SEGMENT - 0.15, 0.05).rotate_y(FRAC_PI_2);
    let road = Geometry::floor(SEGMENT, 20.6).translate(0.0, 0.03, 0.0);
    let mut trim = vec![
        Geometry::cuboid(SEGMENT, 1.1, 0.3).translate(0.0, 0.55, -11.3),
        Geometry::cuboid(SEGMENT, 1.1, 0.3).translate(0.0, 0.55, 11.3),
        Geometry::cuboid(SEGMENT, 0.8, 0.6).translate(0.0, 0.4, 0.0),
    ];
    for z in [-1.0f32, 1.0] {
        trim.push(Geometry::cylinder(0.11, 0.15, 9.0, 6).translate(0.0, 4.5, z * 11.0));
        trim.push(Geometry::cuboid(0.16, 0.16, 1.8).translate(0.0, 9.0, z * 10.2));
    }
    let lamps = Geometry::merge(
        [-1.0f32, 1.0].map(|z| Geometry::cuboid(0.9, 0.22, 0.45).translate(0.0, 8.85, z * 9.4)),
    );
    (girder, road, Geometry::merge(trim), lamps)
}

/// The approach roads on both rims, the construction yard on the -X rim, and the adobe
/// village on the +X rim. Returns the windows and the street lamps that light up, and the
/// lights of the village.
fn add_village(scene: &mut Scene, assets: &Assets) -> (Vec<Window>, [NodeId; 3]) {
    // the approach roads: an embankment that comes down from the deck to the plateau
    let bank = [
        vec2(DECK_END, -3.0),
        vec2(720.0, -3.0),
        vec2(720.0, 3.0),
        vec2(420.0, 3.0),
        vec2(DECK_END, DECK_Y),
    ];
    let ramp = (DECK_Y - 3.0).atan2(420.0 - DECK_END);
    let length = (420.0 - DECK_END) / ramp.cos();
    let road = Geometry::merge([
        Geometry::floor(length, 15.0).rotate_z(-ramp).translate(
            f32::midpoint(DECK_END, 420.0),
            f32::midpoint(DECK_Y, 3.0) + 0.05,
            0.0,
        ),
        Geometry::floor(300.0, 15.0).translate(570.0, 3.05, 0.0),
    ]);
    let earth = assets.dirt.material(0xb89a7c, 1.0, 0.0);
    let asphalt = assets.asphalt.material(0x7a7a7a, 0.9, 0.0);
    for turn in [0.0, PI] {
        scene.add(Node::mesh(Geometry::extrude(&bank, 17.0, 0.4), earth.clone()).rotate_y(turn));
        scene.add(Node::mesh(road.clone(), asphalt.clone()).rotate_y(turn));
    }

    let adobe = assets.dirt.material(0xffffff, 1.0, 0.0);
    let unit = Geometry::rounded_box(1.0, 1.0, 1.0, 0.08, 2).translate(0.0, 0.5, 0.0);
    let mut blocks: Vec<Mat4> = Vec::new();
    let mut tints: Vec<Rgb> = Vec::new();
    let mut beams: Vec<Mat4> = Vec::new();
    let mut windows: Vec<Window> = Vec::new();
    let bridge_end = vec3(DECK_END, 0.0, 0.0);
    let mut window = |transform: Mat4, lit: bool, seed: u32| {
        let p = transform.w_axis.truncate();
        let on = if lit {
            CLOSURE + 0.35 + p.distance(bridge_end) / 300.0 * 0.7 + 0.3 * hash(seed)
        } else {
            1.0e6
        };
        windows.push(Window {
            transform,
            on,
            warmth: hash(seed + 1),
        });
    };

    // the houses: random plots on the rim that do not overlap
    let mut houses: Vec<(Vec3, Vec3)> = Vec::new();
    for i in 0..400u32 {
        if houses.len() >= 46 {
            break;
        }
        let x = mix(190.0, 330.0, hash(i * 9 + 1));
        let z = mix(-140.0, 230.0, hash(i * 9 + 2));
        if z.abs() < 15.0 || x < rim_x(1.0, z) + 18.0 || (x < DECK_END + 8.0 && z.abs() < 22.0) {
            continue;
        }
        let size = vec3(
            mix(7.0, 13.0, hash(i * 9 + 3)),
            mix(3.2, 4.4, hash(i * 9 + 4)),
            mix(6.0, 11.0, hash(i * 9 + 5)),
        );
        let clear = houses.iter().all(|(p, s)| {
            2.0 * (p.x - x).abs() > s.x + size.x + 6.0 || 2.0 * (p.z - z).abs() > s.z + size.z + 6.0
        });
        if !clear {
            continue;
        }
        let ground = ground_height(1.0, z, x - rim_x(1.0, z));
        houses.push((vec3(x, ground, z), size));
    }
    // the church: a long nave with two towers at its front
    let church = vec3(250.0, 0.0, 80.0);
    houses.retain(|(p, s)| {
        (p.x - church.x).abs() > s.x * 0.5 + 12.0 || (p.z - church.z).abs() > s.z * 0.5 + 18.0
    });
    houses.push((
        vec3(
            church.x,
            ground_height(1.0, church.z, church.x - rim_x(1.0, church.z)),
            church.z,
        ),
        vec3(13.0, 8.0, 24.0),
    ));

    let colors = [0xc9a17a, 0xd4ae88, 0xbd8f6a, 0xd9b99a, 0xb58466];
    for (n, &(base, size)) in houses.iter().enumerate() {
        let seed = n as u32 * 17;
        let tint = Rgb::hex(colors[n % colors.len()]);
        let body = Mat4::from_translation(base - vec3(0.0, 0.6, 0.0))
            * Mat4::from_scale(size + vec3(0.0, 0.6, 0.0));
        blocks.push(body);
        tints.push(tint);
        let church = n + 1 == houses.len();
        let mut roof = base.y + size.y;
        if church {
            for dx in [-1.0f32, 1.0] {
                blocks.push(
                    Mat4::from_translation(base + vec3(dx * 5.5, -0.5, size.z * 0.5 - 2.5))
                        * Mat4::from_scale(vec3(5.0, 14.0, 5.0)),
                );
                tints.push(tint);
            }
        } else if hash(seed) < 0.4 {
            // a second story at the back
            let upper = vec3(size.x * 0.55, 3.0, size.z * 0.55);
            blocks.push(
                Mat4::from_translation(base + vec3(-size.x * 0.2, size.y - 0.1, -size.z * 0.18))
                    * Mat4::from_scale(upper),
            );
            tints.push(tint);
            window(
                Mat4::from_translation(
                    base + vec3(
                        -size.x * 0.2,
                        size.y + 1.6,
                        -size.z * 0.18 + upper.z * 0.5 + 0.05,
                    ),
                ) * Mat4::from_scale(vec3(0.9, 1.0, 1.0)),
                true,
                seed + 3,
            );
            roof += 3.0;
        }
        // a parapet edge and the ends of the roof beams on the front
        blocks.push(
            Mat4::from_translation(base + vec3(0.0, size.y - 0.1, 0.0))
                * Mat4::from_scale(vec3(size.x + 0.3, 0.45, size.z + 0.3)),
        );
        tints.push(tint * 0.92);
        let count = (size.x / 1.6) as usize;
        for b in 0..count {
            let x = (b as f32 + 0.5) / count as f32 - 0.5;
            beams.push(
                Mat4::from_translation(
                    base + vec3(x * (size.x - 1.0), size.y - 0.55, size.z * 0.5 + 0.3),
                ) * Mat4::from_rotation_x(FRAC_PI_2)
                    * Mat4::from_scale(vec3(0.13, 0.7, 0.13)),
            );
        }
        if hash(seed + 5) < 0.3 {
            blocks.push(
                Mat4::from_translation(
                    base + vec3(size.x * 0.3, roof - base.y - 0.1, -size.z * 0.25),
                ) * Mat4::from_scale(vec3(0.8, 1.4, 0.8)),
            );
            tints.push(tint * 0.85);
        }
        // a door and windows on the front, and windows on the side that faces the bridge
        let front = size.z * 0.5 + 0.04;
        window(
            Mat4::from_translation(base + vec3(size.x * 0.22, 1.05, front))
                * Mat4::from_scale(vec3(1.1, 2.1, 1.0)),
            false,
            seed + 7,
        );
        let lit_front = if church {
            3
        } else {
            1 + (hash(seed + 8) * 2.0) as usize
        };
        for w in 0..lit_front {
            let x = -0.3 + w as f32 * 0.22 - if church { 0.0 } else { 0.05 };
            window(
                Mat4::from_translation(base + vec3(x * size.x, 1.6, front))
                    * Mat4::from_scale(vec3(0.9, 1.1, 1.0)),
                true,
                seed + 9 + w as u32,
            );
        }
        if hash(seed + 12) < 0.7 {
            window(
                Mat4::from_translation(base + vec3(-size.x * 0.5 - 0.04, 1.6, size.z * 0.1))
                    * Mat4::from_rotation_y(-FRAC_PI_2)
                    * Mat4::from_scale(vec3(0.9, 1.1, 1.0)),
                true,
                seed + 13,
            );
        }
    }
    scene.add(Node::instanced(
        unit,
        adobe,
        Instances::new(blocks).colors(tints),
    ));
    scene.add(Node::instanced(
        Geometry::cylinder(1.0, 1.0, 1.0, 5).translate(0.0, 0.5, 0.0),
        Material::standard(0x4a3424).roughness(0.9),
        Instances::new(beams),
    ));

    // street lamps along the road into the village: they light up after the bridge
    let mut poles = Vec::new();
    for i in 0..9 {
        let x = 330.0 + i as f32 * 38.0;
        for z in [-9.0f32, 9.0] {
            let y = if x < 420.0 {
                mix(DECK_Y, 3.0, (x - DECK_END) / (420.0 - DECK_END))
            } else {
                3.0
            };
            poles.push(
                Mat4::from_translation(vec3(x, y, z)) * Mat4::from_scale(vec3(0.12, 7.0, 0.12)),
            );
            let seed = 900 + i * 2 + u32::from(z > 0.0);
            windows.push(Window {
                transform: Mat4::from_translation(vec3(x, y + 7.0, z - z.signum() * 0.8))
                    * Mat4::from_rotation_x(-FRAC_PI_2)
                    * Mat4::from_scale(vec3(0.8, 0.5, 1.0)),
                on: CLOSURE + 0.62 + i as f32 * 0.05,
                warmth: hash(seed),
            });
        }
    }
    scene.add(Node::instanced(
        Geometry::cylinder(1.0, 1.0, 1.0, 5).translate(0.0, 0.5, 0.0),
        Material::metal(0x30343a, 0.5),
        Instances::new(poles),
    ));

    // cottonwoods between the houses
    let mut crowns = Vec::new();
    let mut trunks = Vec::new();
    for i in 0..26u32 {
        let x = mix(190.0, 340.0, hash(i * 3 + 500));
        let z = mix(-140.0, 230.0, hash(i * 3 + 501));
        if x < rim_x(1.0, z) + 15.0 {
            continue;
        }
        if z.abs() < 12.0
            || houses.iter().any(|(p, s)| {
                (p.x - x).abs() < s.x * 0.5 + 3.0 && (p.z - z).abs() < s.z * 0.5 + 3.0
            })
        {
            continue;
        }
        let y = ground_height(1.0, z, x - rim_x(1.0, z));
        let h = mix(7.0, 12.0, hash(i * 3 + 502));
        trunks.push(
            Mat4::from_translation(vec3(x, y, z)) * Mat4::from_scale(vec3(0.35, h * 0.6, 0.35)),
        );
        for c in 0..3u32 {
            let offset = vec3(
                hash(i * 11 + c) - 0.5,
                hash(i * 13 + c),
                hash(i * 17 + c) - 0.5,
            ) * vec3(4.0, 2.5, 4.0);
            crowns.push(Mat4::from_scale_rotation_translation(
                Vec3::splat(h * 0.32),
                Quat::from_rotation_y(c as f32),
                vec3(x, y + h * 0.72, z) + offset,
            ));
        }
    }
    scene.add(Node::instanced(
        Geometry::cylinder(1.0, 1.0, 1.0, 5).translate(0.0, 0.5, 0.0),
        Material::standard(0x3a2c22).roughness(0.9),
        Instances::new(trunks),
    ));
    scene.add(Node::instanced(
        Geometry::icosahedron(1.0, 1).flat(),
        Material::standard(0x3c4a2a).roughness(0.85),
        Instances::new(crowns),
    ));

    // the construction yard on the -X rim: containers and cold flood light towers
    let mut boxes = Vec::new();
    let mut box_colors = Vec::new();
    let container = [0x7a4a30, 0x3a5470, 0x8a8a84, 0x6a3a2a];
    for i in 0..22u32 {
        let x = -mix(330.0, 430.0, hash(i * 5 + 700));
        let z = mix(28.0, 130.0, hash(i * 5 + 701));
        let y = ground_height(-1.0, z, rim_x(-1.0, z) - x);
        let stack = if hash(i * 5 + 702) < 0.3 { 2.6 } else { 0.0 };
        boxes.push(
            Mat4::from_translation(vec3(x, y + stack, z))
                * Mat4::from_rotation_y(if hash(i * 5 + 703) < 0.5 {
                    0.0
                } else {
                    FRAC_PI_2
                })
                * Mat4::from_scale(vec3(12.2, 2.6, 2.45)),
        );
        box_colors.push(Rgb::hex(container[i as usize % container.len()]));
    }
    let mut floods = Vec::new();
    for (x, z) in [(-345.0f32, 25.0f32), (-410.0, 95.0)] {
        let y = ground_height(-1.0, z, rim_x(-1.0, z) - x);
        boxes.push(Mat4::from_translation(vec3(x, y, z)) * Mat4::from_scale(vec3(0.5, 16.0, 0.5)));
        box_colors.push(Rgb::hex(0x50545a));
        floods.push(Geometry::cuboid(3.2, 1.4, 0.5).translate(x, y + 16.7, z));
    }
    let unit_cube = Geometry::cuboid(1.0, 1.0, 1.0).translate(0.0, 0.5, 0.0);
    scene.add(Node::instanced(
        unit_cube,
        Material::standard(0xffffff).roughness(0.6),
        Instances::new(boxes).colors(box_colors),
    ));
    scene.add(Node::mesh(
        Geometry::merge(floods),
        Material::unlit(Rgb::hex(COLD) * 12.0),
    ));

    let lights = [
        vec3(230.0, 9.0, 40.0),
        vec3(285.0, 9.0, 150.0),
        vec3(300.0, 9.0, -70.0),
    ]
    .map(|p| {
        scene.add(
            Node::light(Light::point(AMBER, 0.0, 140.0))
                .with_position(p)
                .hidden(),
        )
    });
    (windows, lights)
}
