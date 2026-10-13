//! Part 2, 5 to 10 s of the shot: the greenhouse at first light. The sun is still below the
//! horizon. The blue sky, the warm glow of the dawn and the lights of the build light the
//! desert.
//!
//! The dome stands at the origin: a geodesic hemisphere of [`RADIUS`] m on the sand. The
//! times below are seconds into the part.
//!
//! - 0.0 to 1.3: a warm point of light (the headlights of the bridge, matched on the cut)
//!   draws the base ring on the sand.
//! - 0.7 to 1.4: the foundation rises out of the sand.
//! - 0.9 to 2.4: the frame of struts grows from the ring to the top.
//! - 1.5 to 4.3: drones carry the triangular glass panels down from above the frame, set
//!   them from the ground up (2.1 to 3.6), and fly away.
//! - 2.4 to 4.0: inside, the crops, the corn and the trees grow.
//! - 2.9 to 3.9: the grow lights come on, and the dome turns green.
//! - 4.4: a spark leaves the ring. It flies to the left, low over the sand.
//! - 4.75 to 5.0: the camera whips after it into the dawn of the rocket part.

use std::f32::consts::{PI, TAU};
use std::sync::Arc;

use fframes_3d::math::{hash, progress, smoothstep};
use fframes_3d::*;

use super::{
    AMBER, AMBER_EDGE, COLD, COLD_BLUE, WHIP_HALF, camera, ease, ease_in, ease_out, fbm,
    haze_steps, mix, polar, ridges, sky, span, spark, whip, yaw_pitch,
};
use crate::assets::Assets;

/// The radius of the dome in meters.
const RADIUS: f32 = 20.0;
/// The subdivisions of each icosahedron edge.
const FREQUENCY: usize = 6;
/// The pen draws the ring from `RING_START` to `RING_END`.
const RING_START: f32 = 0.0;
const RING_END: f32 = 1.3;
/// The angle of the pen on the ring at the start: in front of the dome, seen from the
/// camera, moving to the right.
const PEN_START: f32 = -0.35;
const STRUTS: (f32, f32) = (0.9, 2.4);
const PANELS: (f32, f32) = (1.5, 3.2);
/// The flight of a drone with its panel to the dome, and its flight away, in seconds.
const CARRY: f32 = 0.6;
const LEAVE: f32 = 0.5;
const GROWTH: (f32, f32) = (2.4, 4.0);
/// The grow lights come on and the glass glows green.
const GREEN: (f32, f32) = (2.9, 3.9);
/// The spark leaves the ring.
const SPARK: f32 = 4.4;
/// The length of the part. The cut to the rocket is at its end.
const END: f32 = 5.0;
/// The slope of the view at the cut to the rocket: a little above the horizon. The rocket
/// part starts with the same slope, so the horizon stays in place over the cut.
pub const LEVEL: f32 = 0.08;
/// The spark at the cut, seen from the camera: the pitch above the horizontal plane in
/// radians and the distance in meters. The rocket part starts the spark at the same place
/// in the picture.
pub const SPARK_AT_CUT: (f32, f32) = (-0.09, 36.0);
/// The time that the spark leads the camera in the whip pan, in seconds. It keeps the spark
/// a little left of the center of the picture.
pub const SPARK_LEAD: f32 = 0.006;

/// The vertices, the edges and the triangles of a geodesic hemisphere of `radius`, cut at
/// the equator. It is an icosahedron with a vertex at the top, each face divided into
/// `frequency`² triangles and pushed onto the sphere. An even frequency puts a row of
/// vertices on the equator.
fn geodesic(radius: f32, frequency: usize) -> (Vec<Vec3>, Vec<[usize; 2]>, Vec<[usize; 3]>) {
    // the golden ratio
    let phi = 1.618_034;
    let corners = [
        vec3(-1.0, phi, 0.0),
        vec3(1.0, phi, 0.0),
        vec3(-1.0, -phi, 0.0),
        vec3(1.0, -phi, 0.0),
        vec3(0.0, -1.0, phi),
        vec3(0.0, 1.0, phi),
        vec3(0.0, -1.0, -phi),
        vec3(0.0, 1.0, -phi),
        vec3(phi, 0.0, -1.0),
        vec3(phi, 0.0, 1.0),
        vec3(-phi, 0.0, -1.0),
        vec3(-phi, 0.0, 1.0),
    ];
    let faces = [
        [0, 11, 5],
        [0, 5, 1],
        [0, 1, 7],
        [0, 7, 10],
        [0, 10, 11],
        [1, 5, 9],
        [5, 11, 4],
        [11, 10, 2],
        [10, 7, 6],
        [7, 1, 8],
        [3, 9, 4],
        [3, 4, 2],
        [3, 2, 6],
        [3, 6, 8],
        [3, 8, 9],
        [4, 9, 5],
        [2, 4, 11],
        [6, 2, 10],
        [8, 6, 7],
        [9, 8, 1],
    ];
    // turn the corner (-1, phi, 0) to the top
    let top = Quat::from_rotation_arc(corners[0].normalize(), Vec3::Y);
    let mut points: Vec<Vec3> = Vec::new();
    let mut index = |p: Vec3| {
        let p = (top * p).normalize() * radius;
        points
            .iter()
            .position(|q| q.distance(p) < 1e-3)
            .unwrap_or_else(|| {
                points.push(p);
                points.len() - 1
            })
    };
    let n = frequency;
    let mut triangles = Vec::new();
    for [a, b, c] in faces {
        let (a, b, c) = (corners[a], corners[b], corners[c]);
        let at = |i: usize, j: usize| {
            a + (b - a) * (i as f32 / n as f32) + (c - a) * (j as f32 / n as f32)
        };
        for i in 0..n {
            for j in 0..n - i {
                triangles.push([index(at(i, j)), index(at(i + 1, j)), index(at(i, j + 1))]);
                if i + j + 1 < n {
                    triangles.push([
                        index(at(i + 1, j)),
                        index(at(i + 1, j + 1)),
                        index(at(i, j + 1)),
                    ]);
                }
            }
        }
    }
    triangles.retain(|t| t.iter().all(|&i| points[i].y > -1e-3));
    let mut edges: Vec<[usize; 2]> = triangles
        .iter()
        .flat_map(|t| [[t[0], t[1]], [t[1], t[2]], [t[2], t[0]]])
        .map(|[a, b]| [a.min(b), a.max(b)])
        .collect();
    edges.sort_unstable();
    edges.dedup();
    (points, edges, triangles)
}

/// The height of the desert at `x`, `z`: flat sand around the dome and dunes beyond 150 m.
fn dunes(x: f32, z: f32) -> f32 {
    let r = vec2(x, z).length();
    let field = smoothstep(150.0, 420.0, r);
    // long crests across the wind, bent by noise
    let warp = 60.0 * fbm(5, x / 400.0, z / 400.0, 2);
    let crest = ((x * 0.8 + z * 0.6 + warp) / 70.0 * TAU).sin() * 0.5 + 0.5;
    let height = 4.0 + 16.0 * (fbm(9, x / 700.0, z / 700.0, 2) * 0.5 + 0.5);
    field * (height * crest.powf(1.6) + 3.0 * fbm(13, x / 120.0, z / 120.0, 3))
        - (1.0 - field) * 0.4
}

/// A strut that grows from its lower hub to its upper hub.
struct Strut {
    from: Vec3,
    to: Vec3,
    start: f32,
}

/// A glass panel that a drone flies into the frame.
struct Panel {
    /// The panel in place: the affine map of the unit triangle.
    place: Mat4,
    /// The outward direction of the panel.
    normal: Vec3,
    start: f32,
}

/// A plant that grows inside the dome.
struct Plant {
    transform: Mat4,
    color: Rgb,
    start: f32,
}

/// The node ids that the frames animate.
struct Ids {
    ring: NodeId,
    pen: NodeId,
    pen_light: NodeId,
    struts: NodeId,
    hubs: NodeId,
    panels: NodeId,
    drones: NodeId,
    leds: NodeId,
    crops: NodeId,
    stalks: NodeId,
    trunks: NodeId,
    crowns: NodeId,
    inside: [NodeId; 4],
    shell: NodeId,
    spark: NodeId,
    glow: NodeId,
    dawn: NodeId,
    foundation: NodeId,
}

/// The desert, the dome and its machines at first light.
pub struct Greenhouse {
    scene: Scene,
    ids: Ids,
    struts: Vec<Strut>,
    hubs: Vec<(Vec3, f32)>,
    panels: Vec<Panel>,
    crops: Vec<Plant>,
    stalks: Vec<Plant>,
    trunks: Vec<Plant>,
    crowns: Vec<Plant>,
    landmark: Scene,
}

/// The number of segments of the glowing ring.
const RING_SEGMENTS: usize = 240;

/// The position of the pen at the part `u` of the circle, `0..1`.
fn ring_point(u: f32) -> Vec3 {
    let a = PEN_START + u * TAU;
    vec3((RADIUS + 0.45) * a.sin(), 0.08, (RADIUS + 0.45) * a.cos())
}

/// The part of the ring that the pen has drawn at `t`. It starts slowly, at about the speed
/// of the car on the bridge, and speeds up.
fn ring_progress(t: f32) -> f32 {
    let x = progress(t, RING_START, RING_END);
    0.45 * x + 0.55 * x * x
}

impl Greenhouse {
    /// Builds the part. `glow` is the direction of the dawn glow in the sky, as
    /// `atan2(z, x)` in radians.
    pub fn new(assets: &Assets, glow: f32) -> Self {
        let (points, edges, triangles) = geodesic(RADIUS, FREQUENCY);
        // the frame grows from the ground up: each part starts by the height of its lowest
        // point, with a little noise
        let when = |y: f32, seed: u32, (from, to): (f32, f32)| {
            from + (y / RADIUS).powf(0.8) * (to - from - 0.35) + 0.12 * hash(seed)
        };
        let struts: Vec<Strut> = edges
            .iter()
            .enumerate()
            .map(|(i, &[a, b])| {
                let (from, to) = if points[a].y <= points[b].y {
                    (points[a], points[b])
                } else {
                    (points[b], points[a])
                };
                Strut {
                    from,
                    to,
                    start: when(from.y, i as u32, STRUTS),
                }
            })
            .collect();
        let hubs: Vec<(Vec3, f32)> = points
            .iter()
            .enumerate()
            .map(|(i, p)| (*p, when(p.y, i as u32 + 7000, STRUTS) - 0.05))
            .collect();
        let panels: Vec<Panel> = triangles
            .iter()
            .enumerate()
            .map(|(i, t)| {
                let [a, b, c] = t.map(|i| points[i]);
                let center = (a + b + c) / 3.0;
                // inset the glass by the radius of the struts
                let inset = |p: Vec3| center + (p - center) * (1.0 - 0.16 / (p - center).length());
                let (a, mut b, mut c) = (inset(a), inset(b), inset(c));
                if (b - a).cross(c - a).dot(center) < 0.0 {
                    std::mem::swap(&mut b, &mut c);
                }
                let normal = (b - a).cross(c - a).normalize();
                Panel {
                    place: Mat4::from_cols(
                        (b - a).extend(0.0),
                        (c - a).extend(0.0),
                        normal.extend(0.0),
                        a.extend(1.0),
                    ),
                    normal,
                    start: when(center.y, i as u32 + 9000, PANELS),
                }
            })
            .collect();

        let (crops, stalks, trunks, crowns) = plants();
        let (scene, ids) = build(assets, glow);
        let mut greenhouse = Self {
            scene,
            ids,
            struts,
            hubs,
            panels,
            crops,
            stalks,
            trunks,
            crowns,
            landmark: Scene::new(),
        };
        // the finished dome for shot 9, without the desert and the lights. The warm light
        // inside is a glow that shows from far away.
        let mut landmark = greenhouse.scene.clone();
        greenhouse.pose(&mut landmark, 4.9);
        let hidden: Vec<NodeId> = landmark
            .nodes()
            .filter(|(_, node)| {
                node.name.as_deref() == Some("desert") || matches!(node.object, Object::Light(_))
            })
            .map(|(id, _)| id)
            .collect();
        for id in hidden {
            landmark[id].visible = false;
        }
        landmark.add(Node::particles(
            vec![Particle::new(
                vec3(0.0, 7.0, 0.0),
                14.0,
                Rgb::hex(0xffc27a) * 0.6,
            )],
            Blending::Additive,
        ));
        greenhouse.landmark = landmark;
        greenhouse
    }

    /// The scene and the camera at `t` seconds into the part, `0..5`.
    pub fn moment(&self, t: f32) -> (Scene, Camera) {
        let mut scene = self.scene.clone();
        self.pose(&mut scene, t);
        let ids = &self.ids;
        // the light of the dawn grows: a time-lapse of the minutes before the sunrise
        let dawn = ease(progress(t, 0.0, END));
        scene.environment = scene.environment.clone().intensity(mix(1.0, 1.3, dawn));
        if let Some(light) = scene[ids.dawn].light_mut() {
            *light.intensity_mut() = mix(1.9, 1.7, dawn);
        }
        if let Some(light) = scene[ids.glow].light_mut() {
            *light.intensity_mut() = mix(0.15, 0.55, dawn);
        }
        if let Some(air) = &mut scene.atmosphere {
            air.offset = vec3(t * 3.0, 0.0, t * 1.2);
            air.steps = haze_steps(t - END);
            air.ambient = Rgb::hex(0x6a84c0) * mix(0.035, 0.07, dawn);
        }

        // the spark: it lifts off the ring, then darts to the left, and the camera whips
        // after it
        let camera = camera_at(t);
        if t > SPARK
            && let Object::Particles(p) = &mut scene[ids.spark].object
        {
            p.items = spark(spark_at, t, smoothstep(SPARK, SPARK + 0.25, t));
        }
        (scene, camera)
    }

    /// The finished dome with its warm light for shot 9, see [`super::Landmarks`].
    pub fn landmark(&self) -> Scene {
        self.landmark.clone()
    }

    fn pose(&self, scene: &mut Scene, t: f32) {
        let ids = &self.ids;
        let set = |scene: &mut Scene, id: NodeId, instances: Instances| {
            if let Some(mesh) = scene[id].mesh_mut() {
                mesh.instances = Some(Arc::new(instances));
            }
        };
        let mut leds = Vec::new();

        // the ring and the pen that draws it
        let drawn = ring_progress(t);
        let count = (drawn * RING_SEGMENTS as f32).ceil() as usize;
        set(
            scene,
            ids.ring,
            Instances::new((0..count).map(|i| {
                let u0 = i as f32 / RING_SEGMENTS as f32;
                let u1 = ((i + 1) as f32 / RING_SEGMENTS as f32).min(drawn);
                span(ring_point(u0), ring_point(u1), 0.09)
            })),
        );
        let pen = ring_point(drawn);
        let drawing = t < RING_END + 0.05;
        scene[ids.pen].visible = drawing;
        scene[ids.pen].position = pen;
        scene[ids.pen_light].visible = drawing;
        scene[ids.pen_light].position = pen + vec3(0.0, 0.6, 0.0);
        // the flare of the car on the bridge settles into the pen
        let flare = 1.0 - ease_out(progress(t, 0.0, 0.5));
        if let Object::Particles(p) = &mut scene[ids.pen].object {
            p.items[1].color = Rgb::hex(AMBER) * (2.5 + 30.0 * flare);
        }
        if let Some(light) = scene[ids.pen_light].light_mut() {
            *light.intensity_mut() = 600.0 + 2500.0 * flare;
        }

        // the foundation rises out of the sand behind the pen
        let rise = ease_out(progress(t, 0.7, 1.4));
        let foundation = &mut scene[ids.foundation];
        foundation.visible = rise > 0.0;
        foundation.scale = vec3(1.0, rise.max(0.01), 1.0);

        // the struts grow from their lower hub, with a cold welding light at the end
        let mut struts = Vec::new();
        for s in &self.struts {
            let g = progress(t, s.start, s.start + 0.22);
            if g > 0.0 {
                let end = s.from.lerp(s.to, ease_out(g));
                struts.push(span(s.from, end, 0.11));
                if g < 1.0 {
                    leds.push(Particle::new(end, 0.12, Rgb::hex(COLD) * 30.0));
                }
            }
        }
        set(scene, ids.struts, Instances::new(struts));
        set(
            scene,
            ids.hubs,
            Instances::new(
                self.hubs
                    .iter()
                    .filter(|(_, start)| t > *start)
                    .map(|(p, _)| Mat4::from_translation(*p)),
            ),
        );

        // a drone carries each panel down from above the frame in an arc and brakes, nose
        // up, at its place; then it lifts away and leaves the frame
        let mut panels = Vec::new();
        let mut drones = Vec::new();
        for (i, p) in self.panels.iter().enumerate() {
            let g = progress(t, p.start, p.start + CARRY);
            if g <= 0.0 {
                continue;
            }
            let center = p.place.transform_point3(vec3(1.0 / 3.0, 1.0 / 3.0, 0.0));
            let side =
                p.normal.cross(Vec3::Y).normalize_or(Vec3::X) * (8.0 * (hash(i as u32) - 0.5));
            let from = p.normal * 14.0 + Vec3::Y * (34.0 - center.y).max(14.0) + side;
            let left = 1.0 - ease_out(g);
            let away = from * left + side * (0.6 * (PI * left).sin());
            panels.push(Mat4::from_translation(away) * p.place);
            let leave = progress(t, p.start + CARRY, p.start + CARRY + LEAVE);
            if leave >= 1.0 {
                continue;
            }
            let up = p.normal.with_y(0.0) * 8.0 + Vec3::Y * 30.0;
            let drone = center + away + p.normal * 0.5 + Vec3::Y * 0.9 + up * ease_in(leave);
            let brake = Quat::from_axis_angle(
                Vec3::Y.cross(from.with_y(0.0)).normalize_or(Vec3::X),
                -2.0 * left * (1.0 - left),
            );
            let climb = Quat::from_axis_angle(
                Vec3::Y.cross(p.normal.with_y(0.0)).normalize_or(Vec3::X),
                0.35 * smoothstep(0.0, 0.5, leave),
            );
            let heading = Quat::from_rotation_y((i as f32 * 2.4 + t * 3.0).sin() * 0.3);
            drones.push(Mat4::from_rotation_translation(
                climb * brake * heading,
                drone,
            ));
            let blink = if (t * 6.0 + hash(i as u32) * 2.0).fract() < 0.5 {
                40.0
            } else {
                8.0
            };
            leds.push(Particle::new(
                drone - vec3(0.0, 0.18, 0.0),
                0.08,
                Rgb::hex(COLD_BLUE) * blink,
            ));
            leds.push(Particle::new(
                drone + vec3(0.0, 0.1, 0.0),
                0.05,
                Rgb::hex(COLD) * 20.0,
            ));
        }
        set(scene, ids.panels, Instances::new(panels));
        set(scene, ids.drones, Instances::new(drones));
        if let Object::Particles(p) = &mut scene[ids.leds].object {
            p.items = leds;
        }

        // the plants grow from the center outwards
        let grow = |plants: &[Plant], time: f32| {
            let mut transforms = Vec::new();
            let mut colors = Vec::new();
            for p in plants {
                let g = ease_out(progress(t, p.start, p.start + time));
                if g > 0.0 {
                    transforms.push(p.transform * Mat4::from_scale(Vec3::splat(g.max(0.02))));
                    colors.push(p.color);
                }
            }
            Instances::new(transforms).colors(colors)
        };
        set(scene, ids.crops, grow(&self.crops, 0.5));
        set(scene, ids.stalks, grow(&self.stalks, 0.7));
        set(scene, ids.trunks, grow(&self.trunks, 0.8));
        set(scene, ids.crowns, grow(&self.crowns, 0.8));

        // the grow lights come on: the plants show, and the glass glows green from the
        // light and the damp air inside
        let green = ease(progress(t, GREEN.0, GREEN.1));
        for id in ids.inside {
            scene[id].visible = green > 0.0;
            if let Some(light) = scene[id].light_mut() {
                *light.intensity_mut() = 500.0 * green;
            }
        }
        scene[ids.shell].visible = green > 0.0;
        if let Some(m) = scene[ids.shell].material_mut() {
            m.color = Rgb::hex(0x1f8a48) * (0.03 * green);
            m.rim = Rgb::hex(0x52ff86) * (1.0 * green);
        }
        if let Some(m) = scene[ids.panels].material_mut() {
            m.emissive_intensity = 0.03 * green;
        }
    }
}

/// The radii of the rings of raised beds inside the dome.
const BEDS: [f32; 5] = [6.5, 9.0, 11.5, 14.0, 16.5];

/// The crops in the rings of beds, the corn, and the small trees in the middle. Each plant
/// has its transform in place, its color and the time it starts to grow.
fn plants() -> (Vec<Plant>, Vec<Plant>, Vec<Plant>, Vec<Plant>) {
    let start =
        |r: f32, seed: u32| GROWTH.0 + r / RADIUS * (GROWTH.1 - GROWTH.0 - 0.8) + 0.25 * hash(seed);
    let greens = [0x4f8a2e, 0x6aa03a, 0x3d7a34, 0x86b04a, 0x5c9440];
    let mut crops = Vec::new();
    let mut stalks = Vec::new();
    // three rows in each bed, with a gap for the path to the door at +Z
    for (ring, r) in BEDS.into_iter().enumerate() {
        let count = (TAU * r / 0.55) as usize;
        for i in 0..count {
            let a = i as f32 / count as f32 * TAU;
            if a < 0.12 || a > TAU - 0.12 {
                continue;
            }
            for (side, row) in [-0.5f32, 0.5, 0.0].into_iter().enumerate() {
                let seed = ((ring * 1000 + i) * 3 + side) as u32;
                let rr = r + row + 0.12 * (hash(seed * 3 + 1) - 0.5);
                let p = vec3(rr * a.sin(), 0.45, rr * a.cos());
                let color = Rgb::hex(greens[(seed as usize + ring) % greens.len()])
                    * (0.8 + 0.4 * hash(seed * 7));
                if (ring == 1 || ring == 3) && side == 1 {
                    let lean = Quat::from_rotation_x(0.12 * (hash(seed * 13) - 0.5));
                    stalks.push(Plant {
                        transform: Mat4::from_scale_rotation_translation(
                            vec3(1.0, 0.6 + 0.6 * hash(seed * 17), 1.0),
                            Quat::from_rotation_y(hash(seed) * TAU) * lean,
                            p,
                        ),
                        color,
                        start: start(rr, seed + 5),
                    });
                } else {
                    let size = 0.42 + 0.2 * hash(seed * 5 + 2);
                    crops.push(Plant {
                        transform: Mat4::from_scale_rotation_translation(
                            vec3(size, size * 0.75, size),
                            Quat::from_rotation_y(hash(seed * 11) * TAU),
                            p,
                        ),
                        color,
                        start: start(rr, seed),
                    });
                }
            }
        }
    }
    // small fruit trees: one in the middle, six around it, and a ring along the wall
    let mut trunks = Vec::new();
    let mut crowns = Vec::new();
    for i in 0..19u32 {
        let (r, a) = match i {
            0 => (0.0, 0.0),
            1..=6 => (3.3, i as f32 / 6.0 * TAU + 0.3),
            _ => (18.6, (i - 7) as f32 / 12.0 * TAU + 0.26),
        };
        let base = vec3(r * a.sin(), 0.3, r * a.cos());
        let h = match i {
            0 => 6.5,
            1..=6 => 3.6 + 1.2 * hash(i + 40),
            _ => 2.6 + 1.0 * hash(i + 40),
        };
        let begin = start(r, i + 300) - 0.3;
        trunks.push(Plant {
            transform: Mat4::from_translation(base) * Mat4::from_scale(vec3(0.16, h * 0.7, 0.16)),
            color: Rgb::hex(0x5a4030),
            start: begin,
        });
        // a crown of many small clumps of leaves
        for c in 0..14u32 {
            let offset = vec3(
                (hash(i * 17 + c) - 0.5) * 0.9,
                (hash(i * 19 + c) - 0.3) * 0.35,
                (hash(i * 23 + c) - 0.5) * 0.9,
            ) * h;
            crowns.push(Plant {
                transform: Mat4::from_scale_rotation_translation(
                    Vec3::splat(h * (0.09 + 0.05 * hash(i * 29 + c))),
                    Quat::from_rotation_y(c as f32 * 1.7),
                    base + vec3(0.0, h * 0.78, 0.0) + offset,
                ),
                color: Rgb::hex(greens[(i + c) as usize % greens.len()])
                    * (0.35 + 0.25 * hash(i * 31 + c)),
                start: begin + 0.1 + 0.2 * hash(i * 37 + c),
            });
        }
    }
    (crops, stalks, trunks, crowns)
}

/// The scene template: the sky, the desert, the empty frame and the machine parts.
fn build(assets: &Assets, glow: f32) -> (Scene, Ids) {
    // first light: a deep blue sky and a warm band low over the horizon where the sun
    // comes up
    let mut scene = Scene::new()
        .environment(Environment::map(
            sky(
                Rgb::hex(0x102252),
                Rgb::hex(0x7a7ea8) * 0.5,
                Rgb::hex(0xff8a48) * 1.1,
                glow,
            ),
            1.0,
        ))
        .background(Background::Environment { blur: 0.0 });

    // the desert: flat sand around the dome and dunes beyond
    let sand = assets.sand.material(0xf0d2b0, 1.0, 0.0);
    scene.add(Node::mesh(Geometry::floor(420.0, 420.0), sand.clone()).named("desert"));
    let mut field = Geometry::floor_segments(5000.0, 5000.0, 250, 250);
    for p in &mut field.positions {
        p.y = dunes(p.x, p.z);
    }
    field.compute_vertex_normals();
    field.update_bounds();
    scene.add(Node::mesh(field, sand).named("desert").shadows(false, true));
    // far ridges all around: they hide the ground of the panorama photo
    scene.add(
        Node::mesh(
            ridges(3400.0, 140.0, 700.0),
            assets.rock.material(0xa08670, 1.0, 0.0),
        )
        .named("desert")
        .shadows(false, false),
    );
    // stones and dry bushes on the sand
    let mut stones = Vec::new();
    let mut bushes = Vec::new();
    for i in 0..900u32 {
        let r = 30.0 + 600.0 * hash(i * 3 + 1).powi(2);
        let a = hash(i * 3 + 2) * TAU;
        let (x, z) = (r * a.sin(), r * a.cos());
        let size = 0.15 + 0.6 * hash(i * 3 + 3).powi(3);
        let m = Mat4::from_scale_rotation_translation(
            vec3(size * 1.3, size * 0.6, size),
            Quat::from_rotation_y(hash(i) * TAU),
            vec3(x, dunes(x, z).max(0.0) + size * 0.15, z),
        );
        if i % 3 == 0 {
            bushes.push(m);
        } else {
            stones.push(m);
        }
    }
    scene.add(
        Node::instanced(
            Geometry::icosahedron(1.0, 1).flat(),
            assets.rock.material(0xb89a80, 1.0, 0.0),
            Instances::new(stones),
        )
        .named("desert"),
    );
    scene.add(
        Node::instanced(
            Geometry::icosahedron(1.0, 0).flat(),
            Material::standard(0x6a6440).roughness(0.95),
            Instances::new(bushes),
        )
        .named("desert"),
    );

    // the foundation: a low concrete ring wall, the soil, and raised beds with timber sides
    let foundation = scene.add(Node::group());
    let wall = Geometry::lathe(
        &[
            vec2(RADIUS - 0.35, -0.2),
            vec2(RADIUS + 0.35, -0.2),
            vec2(RADIUS + 0.35, 0.3),
            vec2(RADIUS - 0.35, 0.3),
        ],
        96,
    );
    scene.add_to(
        foundation,
        Node::mesh(wall, assets.concrete.material(0xd8d4cc, 0.9, 0.0)),
    );
    let soil = assets.dirt.material(0x8a6a50, 1.0, 0.0);
    scene.add_to(
        foundation,
        Node::mesh(
            Geometry::circle(RADIUS - 0.3, 64).rotate_x(-PI / 2.0),
            soil.clone(),
        )
        .at(0.0, 0.28, 0.0),
    );
    let mut timber = Vec::new();
    let mut beds = Vec::new();
    for r in BEDS {
        for edge in [r - 0.8, r + 0.8] {
            timber.push(
                Geometry::tube_ring(edge + 0.06, edge - 0.06, 0.3, 96).translate(0.0, 0.4, 0.0),
            );
        }
        beds.push(
            Geometry::ring(r - 0.74, r + 0.74, 96)
                .rotate_x(-PI / 2.0)
                .translate(0.0, 0.45, 0.0),
        );
    }
    scene.add_to(
        foundation,
        Node::mesh(
            Geometry::merge(timber),
            Material::standard(0x6a4a30).roughness(0.8),
        ),
    );
    scene.add_to(foundation, Node::mesh(Geometry::merge(beds), soil));

    // the warm ring that the pen draws, and the pen
    let cylinder = Arc::new(Geometry::cylinder(1.0, 1.0, 1.0, 6).translate(0.0, 0.5, 0.0));
    let ring = scene.add(
        Node::instanced(
            cylinder.clone(),
            Material::unlit(Rgb::hex(AMBER_EDGE) * 8.0),
            Instances::default(),
        )
        .shadows(false, false),
    );
    let pen = scene.add(Node::particles(
        vec![
            Particle::new(Vec3::ZERO, 0.35, Rgb::hex(0xfff0d8) * 60.0),
            Particle::new(Vec3::ZERO, 1.5, Rgb::hex(AMBER) * 2.5),
        ],
        Blending::Additive,
    ));
    let pen_light = scene.add(Node::light(Light::point(AMBER, 600.0, 40.0)));

    // the frame: struts and hubs of white steel
    let steel = Material::standard(0xd6dade).roughness(0.35).metalness(0.2);
    let struts = scene.add(Node::instanced(
        cylinder.clone(),
        steel.clone(),
        Instances::default(),
    ));
    let hubs = scene.add(Node::instanced(
        Geometry::sphere(0.2, 10, 6),
        steel,
        Instances::default(),
    ));

    // the glass: it adds the reflection of the sky and the sun and hides nothing
    let unit_triangle = Geometry::new(
        vec![Vec3::ZERO, Vec3::X, Vec3::Y],
        vec![Vec3::Z; 3],
        vec![Vec2::ZERO, Vec2::X, Vec2::Y],
        vec![0, 1, 2],
    );
    let glass = Material::standard(0x000000)
        .roughness(0.05)
        .env_intensity(1.8)
        .emissive(0x52ff86, 0.0)
        .additive()
        .double_sided();
    let panels = scene
        .add(Node::instanced(unit_triangle, glass, Instances::default()).shadows(false, false));

    // the drones: a body, four arms and four rotors
    let mut drone = vec![Geometry::rounded_box(0.5, 0.18, 0.5, 0.06, 2)];
    for k in 0..4 {
        let a = k as f32 * PI / 2.0 + PI / 4.0;
        let tip = vec3(a.cos(), 0.0, a.sin()) * 0.55;
        drone.push(
            Geometry::cylinder(1.0, 1.0, 1.0, 5)
                .translate(0.0, 0.5, 0.0)
                .transform(span(Vec3::ZERO, tip, 0.03)),
        );
        drone.push(Geometry::cylinder(0.22, 0.22, 0.02, 12).translate(tip.x, 0.08, tip.z));
    }
    let drones = scene.add(Node::instanced(
        Geometry::merge(drone),
        Material::standard(0x24272c).roughness(0.4),
        Instances::default(),
    ));
    let leds = scene.add(Node::particles(Vec::new(), Blending::Additive));

    // the plants: rosettes of leaves, corn with long drooping leaves, and small trees
    let leaf = Material::standard(0xffffff).roughness(0.7).double_sided();
    // a leaf: a diamond of `width` x `length` along +Z, folded along its middle
    let blade = |width: f32, length: f32| {
        Geometry::new(
            vec![
                Vec3::ZERO,
                vec3(-width, 0.0, length * 0.45),
                vec3(0.0, width * 0.3, length * 0.45),
                vec3(width, 0.0, length * 0.45),
                vec3(0.0, 0.0, length),
            ],
            Vec::new(),
            Vec::new(),
            vec![0, 2, 1, 0, 3, 2, 1, 2, 4, 2, 3, 4],
        )
    };
    let rosette = Geometry::merge((0..10).map(|k| {
        let inner = k >= 7;
        let around = k as f32 * if inner { 2.1 } else { TAU / 7.0 } + 0.3 * hash(k);
        blade(
            if inner { 0.22 } else { 0.32 },
            if inner { 0.7 } else { 1.0 },
        )
        .rotate_x(if inner { -0.9 } else { -0.35 })
        .rotate_y(around)
    }))
    .flat();
    let crops = scene.add(Node::instanced(rosette, leaf.clone(), Instances::default()));
    let mut corn = vec![Geometry::cylinder(0.025, 0.04, 2.1, 5).translate(0.0, 1.05, 0.0)];
    for k in 0..7 {
        corn.push(
            blade(0.07, 0.75)
                .rotate_x(0.2 + 0.15 * (k % 3) as f32)
                .rotate_y(k as f32 * 2.4)
                .translate(0.0, 0.5 + k as f32 * 0.22, 0.0),
        );
    }
    corn.push(Geometry::cylinder(0.0, 0.05, 0.35, 5).translate(0.0, 2.25, 0.0));
    let stalks = scene.add(Node::instanced(
        Geometry::merge(corn).flat(),
        leaf.clone(),
        Instances::default(),
    ));
    let trunks = scene.add(Node::instanced(
        cylinder,
        leaf.clone(),
        Instances::default(),
    ));
    let crowns = scene.add(Node::instanced(
        Geometry::icosahedron(1.0, 1).flat(),
        leaf,
        Instances::default(),
    ));
    // the grow lights, and a thin shell inside the glass that glows green at its edges:
    // the light in the damp air under the glass
    let inside = [
        vec3(0.0, 4.0, 9.0),
        vec3(-9.0, 3.5, -4.0),
        vec3(9.0, 3.5, -3.0),
        vec3(0.0, 6.0, -2.0),
    ]
    .map(|p| {
        scene.add(
            Node::light(Light::point(0xfff0c8, 0.0, 30.0))
                .with_position(p)
                .hidden(),
        )
    });
    let shell = scene.add(
        Node::mesh(
            Geometry::sphere_partial(RADIUS - 0.4, 48, 16, 0.0, PI / 2.0),
            Material::unlit(0x000000).rim(0x000000, 2.2).additive(),
        )
        .shadows(false, false)
        .hidden(),
    );
    let spark = scene.add(Node::particles(Vec::new(), Blending::Additive));

    // the blue light of the sky, and the warm light of the glow low over the horizon. The
    // sun is below the horizon, so the shadows are faint and long.
    let dawn = scene.add(Node::light(Light::hemisphere(0x6a88d0, 0x2a2420, 0.0)));
    let from = vec3(glow.cos(), 0.06, glow.sin());
    let glow = scene.add(Node::light(Light::sun(0xff9a60, 0.0, from).with_shadow(
        Shadow {
            map_size: 2048,
            softness: 4.0,
            strength: 0.6,
            bias: 1.5,
            bounds: Some(Aabb {
                min: vec3(-60.0, -1.0, -60.0),
                max: vec3(60.0, 25.0, 60.0),
            }),
        },
    )));
    scene.atmosphere = Some(Atmosphere {
        density: 0.0035,
        color: Rgb::WHITE,
        base: 0.0,
        falloff: 0.06,
        anisotropy: 0.45,
        ambient: Rgb::hex(0x6a84c0) * 0.05,
        noise: 0.6,
        noise_size: 25.0,
        offset: Vec3::ZERO,
        distance: 900.0,
        steps: 32,
    });

    let ids = Ids {
        ring,
        pen,
        pen_light,
        struts,
        hubs,
        panels,
        drones,
        leds,
        crops,
        stalks,
        trunks,
        crowns,
        inside,
        shell,
        spark,
        glow,
        dawn,
        foundation,
    };
    (scene, ids)
}

/// The place of the camera and the point that it looks at, without the whip pan. The
/// camera starts high on the pen, where the car of the bridge was in the picture. It
/// follows the ring down to a low orbit around the dome, and comes closer when the dome
/// turns green.
fn rest(t: f32) -> (Vec3, Vec3) {
    let azimuth = mix(25.0f32, -20.0, ease(progress(t, 0.0, 4.6))).to_radians();
    let height = mix(34.0, 5.0, ease(progress(t, 0.2, 2.6)));
    let distance =
        mix(95.0, 70.0, ease(progress(t, 0.0, 3.0))) - 8.0 * ease(progress(t, GREEN.0, END));
    let position = vec3(distance * azimuth.sin(), height, distance * azimuth.cos());
    let dome = vec3(0.0, mix(4.0, 10.0, ease(progress(t, 0.8, 3.0))), 0.0);
    // the target starts on the pen and settles on the dome
    let settle = ease(progress(t, 0.15, 1.5));
    (position, ring_point(ring_progress(t)).lerp(dome, settle))
}

/// The spark at `t`. It lifts off the ring in front of the camera, comes nearer and darts
/// to the left, a little ahead of the camera.
fn spark_at(t: f32) -> Vec3 {
    let (eye, target) = rest(t);
    let forward = target - eye;
    let front = rest(SPARK).0.with_y(0.0).normalize() * (RADIUS + 0.45) + vec3(0.0, 0.1, 0.0);
    let (yaw, pitch) = yaw_pitch(forward, front - eye);
    let (cut_pitch, cut_distance) = SPARK_AT_CUT;
    let lift = ease(progress(t, SPARK, END));
    let dart = WHIP_HALF + whip(t - END + SPARK_LEAD);
    polar(
        eye,
        forward,
        mix(yaw, 0.0, lift) + dart,
        mix(pitch, cut_pitch, lift),
        mix(front.distance(eye), cut_distance, lift),
    )
}

/// The camera at `t`: the view of [`rest`]. At the end it levels with the horizon and
/// whips to the left after the spark.
pub(super) fn camera_at(t: f32) -> Camera {
    let (position, target) = rest(t);
    let forward = (target - position).normalize();
    let level = ease(progress(t, 4.3, END));
    let pitch = mix(forward.y.asin(), LEVEL.atan(), level);
    let turn = WHIP_HALF + whip(t - END);
    let view = polar(position, forward, turn, pitch, 60.0);
    let settle = ease(progress(t, 0.15, 1.5));
    let mm = mix(40.0, 32.0, settle);
    let focus = ring_point(ring_progress(t)).lerp(Vec3::ZERO, settle);
    let focus = focus.lerp(spark_at(t), ease(progress(t, SPARK, SPARK + 0.4)));
    camera(mm, position, view)
        .focus_on(focus, 4.0)
        .shake(t, 0.0008, 11)
}
