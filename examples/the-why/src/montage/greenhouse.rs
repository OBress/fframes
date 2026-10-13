//! Part 2 of the shot: the greenhouse at first light. The sun is still below the horizon.
//! The blue sky, the warm glow of the dawn and the lights of the build light the desert.
//!
//! The dome stands at the origin: a geodesic hemisphere of [`RADIUS`] m on a concrete ring
//! beam. The warm line builds it: the light of the ring climbs the struts, and machines
//! print them. The times below are seconds into the part.
//!
//! - 0.0 to 1.3: a warm point of light (the headlights of the bridge, matched on the cut)
//!   draws the ring on the sand, and lamps along the ring light the sand behind it.
//! - 0.1 to 1.9: the concrete ring beam rises out of the sand behind the pen, with the
//!   anchors of the frame on it.
//! - 0.55 to 1.35: five strut printers drive in from the dark to the sides and the back of
//!   the ring.
//! - 1.3: the ring closes with a flash. The printers wake up and lift their booms.
//! - 1.3 to 2.6: the light of the ring climbs the frame tier by tier. Each strut is printed
//!   from its lower hub upward, hot and warm, and cools to white steel. Each hub lights up
//!   as it connects. Cold work lights burn on each tier while it is built and go out when
//!   it is done. The printer booms follow the top of the build.
//! - 1.6 to 2.6: the soil and the raised beds rise inside, from the middle outward.
//! - 2.5 to 4.0: the printers fold their booms and drive away.
//! - 1.2 to 3.8: drones bring the glass and set it tier by tier from the bottom.
//! - 2.4: bloom. The grow lights come on around their ring, and the plants sprout and
//!   unfold, from the middle outward.
//! - 2.6 to 3.6: the dome turns green. 3.2: the mist of the irrigation.
//! - 4.4: a spark leaves the ring. It flies to the left, low over the sand.
//! - 4.75 to 5.0: the camera whips after it into the dawn of the rocket part.

use std::f32::consts::{FRAC_PI_2, PI, TAU};
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
/// The height of the top of the ring beam above the sand. The frame stands on it.
const BEAM_TOP: f32 = 0.35;
/// The number of segments of the ring beam. Each one rises on its own, behind the pen.
const BEAM_SEGMENTS: usize = 120;
/// The radius of the warm line on the sand, just outside the ring beam.
const LINE_RADIUS: f32 = RADIUS + 0.75;
/// The number of segments of the warm line.
const LINE_SEGMENTS: usize = 240;
/// The pen draws the ring until `RING_END`.
const RING_END: f32 = 1.3;
/// The angle of the pen on the ring at the start: in front of the dome, seen from the
/// camera, moving to the right.
const PEN_START: f32 = -0.35;
/// A segment of the ring beam starts to rise `BEAM_DELAY` after the pen passes, and
/// stands `BEAM_RISE` later.
const BEAM_DELAY: f32 = 0.1;
const BEAM_RISE: f32 = 0.45;
/// The light starts to climb the frame when the ring closes.
const FRAME_START: f32 = 1.3;
/// The speed of the light that prints the struts, in m/s.
const CLIMB: f32 = 30.0;
/// The distance of the strut printers from the axis of the dome when they work.
const TRACK: f32 = RADIUS + 4.5;
const PRINTERS: usize = 5;
/// The soil and the raised beds rise inside.
const SOIL: (f32, f32) = (1.6, 2.2);
/// The first glass panel is set.
const GLASS: f32 = 2.0;
/// The flight of a drone with its glass to the dome, and its flight away, in seconds.
const CARRY: f32 = 0.8;
const LEAVE: f32 = 0.7;
/// The grow lights come on and the plants sprout.
const BLOOM: f32 = 2.4;
/// The glass glows green.
const GREEN: (f32, f32) = (2.6, 3.6);
/// The irrigation starts to mist.
const MIST: f32 = 3.2;
/// The spark leaves the ring.
const SPARK: f32 = 4.4;
/// The length of the part. The cut to the rocket is at its end.
const END: f32 = 5.0;
/// The ring of the grow lights inside the dome: its radius and its height.
const LIGHT_RING: (f32, f32) = (15.0, 7.8);
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

/// The part of the ring that the pen has drawn at `t`. It starts at about the speed of the
/// car on the bridge, speeds up, and slows down to close the ring.
fn ring_progress(t: f32) -> f32 {
    let x = progress(t, 0.0, RING_END);
    let start = 0.45;
    start * (x - 2.0 * x * x + x * x * x) + (3.0 - 2.0 * x) * x * x
}

/// The time at which the pen passes the part `u` of the ring, the inverse of
/// [`ring_progress`].
fn ring_time(u: f32) -> f32 {
    let (mut low, mut high) = (0.0, RING_END);
    for _ in 0..24 {
        let mid = f32::midpoint(low, high);
        if ring_progress(mid) < u {
            low = mid;
        } else {
            high = mid;
        }
    }
    f32::midpoint(low, high)
}

/// The angle around the dome of the part `u` of the ring, `atan2(x, z)` in radians.
fn ring_angle(u: f32) -> f32 {
    PEN_START + u * TAU
}

/// The part of the ring at the direction of `p` from the axis of the dome, `0..1`.
fn ring_part(p: Vec3) -> f32 {
    ((p.x.atan2(p.z) - PEN_START) / TAU).rem_euclid(1.0)
}

/// The horizontal unit vector at the angle `a` around the dome.
fn around(a: f32) -> Vec3 {
    vec3(a.sin(), 0.0, a.cos())
}

/// The point of the warm line at the part `u` of the ring.
fn ring_point(u: f32) -> Vec3 {
    around(ring_angle(u)) * LINE_RADIUS + vec3(0.0, 0.06, 0.0)
}

/// How far a segment of the ring beam has risen at `t`, `0..1`. `u` is its part of the ring.
fn beam_rise(u: f32, t: f32) -> f32 {
    let at = ring_time(u) + BEAM_DELAY;
    ease_out(progress(t, at, at + BEAM_RISE))
}

/// The time at which the light of the ring reaches each hub. The base hubs start when the
/// beam under them stands and the ring is closed. From there the light runs along the
/// struts at [`CLIMB`] m/s and takes the shortest way to each hub.
fn arrivals(points: &[Vec3], edges: &[[usize; 2]]) -> Vec<f32> {
    let n = points.len();
    let base = |p: Vec3| (p.y - BEAM_TOP).abs() < 1e-3;
    let mut arrival: Vec<f32> = points
        .iter()
        .enumerate()
        .map(|(i, p)| {
            if base(*p) {
                let stands = ring_time(ring_part(*p)) + BEAM_DELAY + BEAM_RISE;
                stands.max(FRAME_START) + 0.04 * hash(i as u32)
            } else {
                f32::MAX
            }
        })
        .collect();
    let mut neighbors = vec![Vec::new(); n];
    for &[a, b] in edges {
        neighbors[a].push(b);
        neighbors[b].push(a);
    }
    let mut done = vec![false; n];
    while let Some(a) = (0..n)
        .filter(|&i| !done[i])
        .min_by(|&i, &j| arrival[i].total_cmp(&arrival[j]))
    {
        done[a] = true;
        for &b in &neighbors[a] {
            let reach = arrival[a] + points[a].distance(points[b]) / CLIMB;
            if !base(points[b]) && reach < arrival[b] {
                arrival[b] = reach;
            }
        }
    }
    arrival
}

/// A part that the light prints: a straight bar from `from` to `to`, printed from `start`
/// to `end`. Struts of the frame carry a gasket for the glass on their outer side.
struct Strut {
    from: Vec3,
    to: Vec3,
    start: f32,
    end: f32,
    thickness: f32,
    frame: bool,
}

/// A hub of the frame: where the struts meet. `arrival` is the time the light reaches it.
struct Hub {
    position: Vec3,
    normal: Vec3,
    arrival: f32,
}

/// A glass panel that a drone carries to the frame. Two neighbor panels share a drone; the
/// first of the two flies it.
struct Panel {
    /// The panel in place: the affine map of the unit triangle.
    place: Mat4,
    /// The outward direction of the panel.
    normal: Vec3,
    /// The time the panel is set.
    set: f32,
    drone: bool,
}

/// A plant that sprouts in the dome: it grows up first and then unfolds its leaves.
struct Plant {
    transform: Mat4,
    color: Rgb,
    start: f32,
    time: f32,
}

/// The node ids that the frames animate.
struct Ids {
    line: NodeId,
    pen: NodeId,
    pen_light: NodeId,
    ring_lights: [NodeId; 4],
    beam: NodeId,
    anchors: NodeId,
    soil: NodeId,
    beds: [NodeId; 5],
    struts: NodeId,
    hot: NodeId,
    gaskets: NodeId,
    hubs: NodeId,
    front_light: NodeId,
    carriages: NodeId,
    booms: NodeId,
    heads: NodeId,
    grow_bars: NodeId,
    nozzles: NodeId,
    panels: NodeId,
    drones: NodeId,
    leds: NodeId,
    mist: NodeId,
    crops: NodeId,
    stalks: NodeId,
    trunks: NodeId,
    crowns: NodeId,
    inside: [NodeId; 4],
    shell: NodeId,
    spark: NodeId,
    glow: NodeId,
    dawn: NodeId,
}

/// The desert, the dome and the machines that build it at first light.
pub struct Greenhouse {
    scene: Scene,
    ids: Ids,
    /// The time the pen passes the start of each segment of the warm line.
    line_times: Vec<f32>,
    hubs: Vec<Hub>,
    struts: Vec<Strut>,
    panels: Vec<Panel>,
    /// The time the light reaches the top of the dome.
    top: f32,
    /// The time each of the [`GROW_BARS`] pieces of the light ring is printed.
    piece_ends: Vec<f32>,
    crops: Vec<Plant>,
    stalks: Vec<Plant>,
    trunks: Vec<Plant>,
    crowns: Vec<Plant>,
    landmark: Scene,
}

impl Greenhouse {
    /// Builds the part. `glow` is the direction of the dawn glow in the sky, as
    /// `atan2(z, x)` in radians.
    pub fn new(assets: &Assets, glow: f32) -> Self {
        let (mut points, edges, triangles) = geodesic(RADIUS, FREQUENCY);
        for p in &mut points {
            p.y += BEAM_TOP;
        }
        let center = Vec3::Y * BEAM_TOP;
        let arrival = arrivals(&points, &edges);
        // the vertices of the lower half of the icosahedron are on no strut: the light
        // never reaches them
        let reached = |a: &&f32| **a < f32::MAX;
        let top = arrival.iter().filter(reached).copied().fold(0.0, f32::max);
        let hubs: Vec<Hub> = points
            .iter()
            .zip(&arrival)
            .filter(|(_, a)| reached(a))
            .map(|(p, a)| Hub {
                position: *p,
                normal: (*p - center).normalize(),
                arrival: *a,
            })
            .collect();
        // each strut is printed from the end that the light reaches first
        let mut struts: Vec<Strut> = edges
            .iter()
            .map(|&[a, b]| {
                let (from, to) = if arrival[a] <= arrival[b] {
                    (a, b)
                } else {
                    (b, a)
                };
                let start = arrival[from];
                Strut {
                    from: points[from],
                    to: points[to],
                    start,
                    end: start + points[a].distance(points[b]) / CLIMB,
                    thickness: 0.11,
                    frame: true,
                }
            })
            .collect();
        // the glass of a panel comes when its three struts stand, from the bottom up
        let strut_end = |a: usize, b: usize| {
            edges
                .binary_search(&[a.min(b), a.max(b)])
                .map_or(top, |i| struts[i].end)
        };
        let mut panels: Vec<Panel> = triangles
            .iter()
            .enumerate()
            .map(|(i, t)| {
                let [a, b, c] = t.map(|i| points[i]);
                let middle = (a + b + c) / 3.0;
                let ready = strut_end(t[0], t[1])
                    .max(strut_end(t[1], t[2]))
                    .max(strut_end(t[2], t[0]));
                let wanted = GLASS
                    + 1.0 * ((middle.y - BEAM_TOP) / RADIUS).powf(0.9)
                    + 0.1 * hash(i as u32 + 9000);
                // inset the glass by the radius of the struts
                let inset = |p: Vec3| middle + (p - middle) * (1.0 - 0.16 / (p - middle).length());
                let (a, mut b, mut c) = (inset(a), inset(b), inset(c));
                if (b - a).cross(c - a).dot(middle - center) < 0.0 {
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
                    set: wanted.max(ready + 0.15),
                    drone: i % 2 == 0,
                }
            })
            .collect();
        for pair in panels.chunks_mut(2) {
            let set = pair.iter().map(|p| p.set).fold(0.0, f32::max);
            for p in pair {
                p.set = set;
            }
        }
        let (ring_parts, piece_ends) = light_ring(&hubs);
        struts.extend(ring_parts);

        let (crops, stalks, trunks, crowns) = plants();
        let (scene, ids) = build(assets, glow);
        let line_times = (0..LINE_SEGMENTS)
            .map(|i| ring_time(i as f32 / LINE_SEGMENTS as f32))
            .collect();
        let mut greenhouse = Self {
            scene,
            ids,
            line_times,
            hubs,
            struts,
            panels,
            top,
            piece_ends,
            crops,
            stalks,
            trunks,
            crowns,
            landmark: Scene::new(),
        };
        // the finished dome for shot 9, without the desert, the machines and the lights.
        // The warm light inside is a glow that shows from far away.
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
        let mut glows = Vec::new();

        // the warm line: hot where the pen has just passed, and a flash around the whole
        // ring when it closes
        let drawn = ring_progress(t);
        let closed =
            (-(t - RING_END).max(0.0) / 0.3).exp() * smoothstep(RING_END - 0.05, RING_END, t);
        let mut line = Vec::new();
        let mut line_colors = Vec::new();
        for (i, &passed) in self.line_times.iter().enumerate() {
            let u0 = i as f32 / LINE_SEGMENTS as f32;
            if u0 >= drawn {
                break;
            }
            let u1 = ((i + 1) as f32 / LINE_SEGMENTS as f32).min(drawn);
            line.push(span(ring_point(u0), ring_point(u1), 0.09));
            let heat = (-(t - passed).max(0.0) / 0.25).exp() + 0.8 * closed;
            line_colors.push(
                Rgb::hex(AMBER_EDGE).lerp(Rgb::hex(0xfff0d8), 0.5 * heat.min(1.0))
                    * (8.0 + 14.0 * heat),
            );
        }
        set(scene, ids.line, Instances::new(line).colors(line_colors));

        // the pen: a hot head with a trail along the ring. The flare of the car on the
        // bridge settles into it.
        let drawing = t < RING_END + 0.02;
        let pen = ring_point(drawn);
        scene[ids.pen].visible = drawing;
        scene[ids.pen_light].visible = drawing;
        scene[ids.pen_light].position = pen + vec3(0.0, 0.6, 0.0);
        let flare = 1.0 - ease_out(progress(t, 0.0, 0.5));
        if let Some(light) = scene[ids.pen_light].light_mut() {
            *light.intensity_mut() = 600.0 + 2500.0 * flare;
        }
        if let Object::Particles(p) = &mut scene[ids.pen].object {
            let at = |s: f32| ring_point(ring_progress(s));
            p.items = vec![
                Particle::new(pen, 0.35, Rgb::hex(0xfff0d8) * 60.0).tail(at(t - 1.0 / 120.0) - pen),
                Particle::new(pen, 1.5, Rgb::hex(AMBER) * (2.5 + 30.0 * flare)),
            ];
            for k in 0..6 {
                let fade = 1.0 - k as f32 / 6.0;
                let (a, b) = (at(t - k as f32 * 0.012), at(t - (k + 1) as f32 * 0.012));
                p.items.push(
                    Particle::new(a, 0.2 * fade + 0.06, Rgb::hex(AMBER) * (24.0 * fade * fade))
                        .tail(b - a),
                );
            }
        }
        // lamps along the ring light the sand behind the pen, and go out when the grow
        // lights take over
        for (k, id) in ids.ring_lights.into_iter().enumerate() {
            let passed = ring_time((k as f32 + 0.5) / 4.0);
            let on = smoothstep(passed - 0.05, passed + 0.4, t)
                * (1.0 - smoothstep(BLOOM, BLOOM + 0.6, t));
            scene[id].visible = on > 0.0;
            if let Some(light) = scene[id].light_mut() {
                *light.intensity_mut() = 160.0 * on;
            }
        }

        // the ring beam rises out of the sand behind the pen, segment by segment, and
        // throws up a little dust lit by the line
        let mut beam = Vec::new();
        for k in 0..BEAM_SEGMENTS {
            let u = (k as f32 + 0.5) / BEAM_SEGMENTS as f32;
            let g = beam_rise(u, t);
            if g <= 0.0 {
                continue;
            }
            let a = ring_angle(u);
            beam.push(Mat4::from_rotation_translation(
                Quat::from_rotation_y(a - FRAC_PI_2),
                around(a) * RADIUS + Vec3::Y * (-1.3 * (1.0 - g)),
            ));
            if g < 1.0 {
                for j in 0..2u32 {
                    let seed = k as u32 * 2 + j;
                    let side = around(a + 0.02 * (hash(seed) - 0.5));
                    let p =
                        side * (RADIUS + 0.6 + 0.8 * hash(seed + 7)) + Vec3::Y * (0.3 + 0.9 * g);
                    glows.push(Particle::new(
                        p,
                        0.5 + 1.0 * g,
                        Rgb::hex(AMBER) * (0.35 * (PI * g).sin()),
                    ));
                }
            }
        }
        set(scene, ids.beam, Instances::new(beam));
        // the anchors of the frame ride on the beam
        let anchors: Vec<Mat4> = self
            .hubs
            .iter()
            .filter(|h| (h.position.y - BEAM_TOP).abs() < 1e-3)
            .filter_map(|h| {
                let g = beam_rise(ring_part(h.position), t);
                (g > 0.0).then(|| {
                    let a = h.position.x.atan2(h.position.z);
                    Mat4::from_rotation_translation(
                        Quat::from_rotation_y(a - FRAC_PI_2),
                        h.position.with_y(BEAM_TOP - 1.3 * (1.0 - g)),
                    )
                })
            })
            .collect();
        set(scene, ids.anchors, Instances::new(anchors));

        // the soil comes up inside from the middle outward, then the raised beds
        scene[ids.soil].position.y = -0.6 * (1.0 - ease(progress(t, SOIL.0, SOIL.1)));
        for (k, id) in ids.beds.into_iter().enumerate() {
            let at = SOIL.0 + 0.25 + 0.1 * k as f32;
            scene[id].position.y = -0.9 * (1.0 - ease(progress(t, at, at + 0.5)));
        }

        // the light climbs the frame: each strut is printed from its lower hub, hot at its
        // tip, and cools from amber to steel. A gasket for the glass follows on its outer
        // side.
        let mut struts = Vec::new();
        let mut hot = Vec::new();
        let mut hot_colors = Vec::new();
        let mut gaskets = Vec::new();
        for s in &self.struts {
            let g = progress(t, s.start, s.end);
            if g <= 0.0 {
                continue;
            }
            let tip = s.from.lerp(s.to, g);
            struts.push(span(s.from, tip, s.thickness));
            let cool = (-(t - s.end).max(0.0) / 0.2).exp();
            if cool > 0.02 {
                // the fresh half is hotter than the half behind it
                let middle = s.from.lerp(tip, 0.5);
                for (a, b, heat) in [(s.from, middle, 0.45 * cool), (middle, tip, cool)] {
                    hot.push(span(a, b, s.thickness * 1.25));
                    hot_colors.push(
                        Rgb::hex(AMBER_EDGE).lerp(Rgb::hex(0xfff0d8), heat * 0.4) * (3.5 * heat),
                    );
                }
            }
            if g < 1.0 {
                glows.push(Particle::new(tip, 0.18, Rgb::hex(0xfff0d8) * 18.0));
            }
            if s.frame {
                let seal = progress(t, s.start + 0.3, s.end + 0.3);
                if seal > 0.0 {
                    gaskets.push(gasket(s.from, s.from.lerp(s.to, seal)));
                }
            }
        }
        set(scene, ids.struts, Instances::new(struts));
        set(scene, ids.hot, Instances::new(hot).colors(hot_colors));
        set(scene, ids.gaskets, Instances::new(gaskets));

        // each hub snaps in when the light reaches it, with a warm flash. Cold work lights
        // burn on the hubs of the tier that is built and go out when it is done.
        let mut hubs = Vec::new();
        let mut front: f32 = 0.0;
        for (i, h) in self.hubs.iter().enumerate() {
            let g = progress(t, h.arrival - 0.04, h.arrival + 0.12);
            if g <= 0.0 {
                continue;
            }
            front = front.max(h.position.y);
            let size = ease_out(g) * (1.0 + 0.2 * (PI * g).sin());
            hubs.push(Mat4::from_scale_rotation_translation(
                Vec3::splat(size),
                Quat::from_rotation_arc(Vec3::Y, h.normal),
                h.position,
            ));
            let flash = (-(t - h.arrival).max(0.0) / 0.15).exp();
            if flash > 0.02 {
                glows.push(Particle::new(
                    h.position,
                    0.55,
                    Rgb::hex(AMBER) * (6.0 * flash),
                ));
            }
            let work = smoothstep(h.arrival - 0.25, h.arrival - 0.1, t)
                * (1.0 - smoothstep(h.arrival + 0.3, h.arrival + 0.4, t));
            if i % 2 == 0 && work > 0.0 {
                glows.push(Particle::new(
                    h.position + h.normal * 0.35,
                    0.12,
                    Rgb::hex(COLD) * (28.0 * work),
                ));
            }
        }
        set(scene, ids.hubs, Instances::new(hubs));
        // a warm light inside at the height of the build lights the new steel from within
        let climbing = smoothstep(FRAME_START, FRAME_START + 0.2, t)
            * (1.0 - smoothstep(self.top, self.top + 0.4, t));
        scene[ids.front_light].visible = climbing > 0.0;
        scene[ids.front_light].position = vec3(0.0, front.max(1.0), 0.0);
        if let Some(light) = scene[ids.front_light].light_mut() {
            *light.intensity_mut() = 500.0 * climbing;
        }

        self.pose_printers(scene, t, &mut glows);

        // the grow light bars and the nozzles snap on under the light ring when the piece
        // above them is printed; the grow lights come on around the ring at the bloom
        let (radius, height) = LIGHT_RING;
        let mut bars = Vec::new();
        let mut bar_colors = Vec::new();
        for (j, &end) in self.piece_ends.iter().enumerate() {
            let g = ease_out(progress(t, end, end + 0.15));
            if g <= 0.0 {
                continue;
            }
            let a = (j as f32 + 0.5) / GROW_BARS as f32 * TAU;
            bars.push(Mat4::from_scale_rotation_translation(
                vec3(0.24, 0.07, 1.7 * g),
                Quat::from_rotation_y(a + FRAC_PI_2),
                around(a) * radius + Vec3::Y * (height + BEAM_TOP - 0.4),
            ));
            let on = smoothstep(0.0, 0.08, t - BLOOM - 0.35 * j as f32 / GROW_BARS as f32);
            bar_colors.push(Rgb::hex(0x141414).lerp(Rgb::hex(0xfff0d0) * 6.0, on));
        }
        set(
            scene,
            ids.grow_bars,
            Instances::new(bars).colors(bar_colors),
        );
        let nozzles: Vec<Mat4> = (0..16)
            .filter_map(|k| {
                let a = k as f32 / 16.0 * TAU + 0.1;
                let end = self.piece_ends[(a / TAU * GROW_BARS as f32) as usize % GROW_BARS];
                let g = ease_out(progress(t, end, end + 0.15));
                (g > 0.0).then(|| {
                    Mat4::from_scale_rotation_translation(
                        Vec3::splat(g),
                        Quat::IDENTITY,
                        around(a) * radius + Vec3::Y * (height + BEAM_TOP - 0.3),
                    )
                })
            })
            .collect();
        set(scene, ids.nozzles, Instances::new(nozzles));

        // the drones bring the glass after the frame stands
        let mut leds = Vec::new();
        let (panels, drones) = self.pose_glass(t, &mut leds);
        set(scene, ids.panels, Instances::new(panels));
        set(scene, ids.drones, Instances::new(drones));
        if let Object::Particles(p) = &mut scene[ids.leds].object {
            p.items = leds;
        }

        // the plants sprout and unfold from the middle outward
        let grow = |plants: &[Plant]| {
            let mut transforms = Vec::new();
            let mut colors = Vec::new();
            for p in plants {
                let g = progress(t, p.start, p.start + p.time);
                if g <= 0.0 {
                    continue;
                }
                let up = ease_out(g);
                let open = ease(progress(g, 0.25, 1.0));
                let width = 0.15 * up + 0.85 * open;
                transforms.push(
                    p.transform * Mat4::from_scale(vec3(width, up, width).max(Vec3::splat(0.02))),
                );
                colors.push(p.color);
            }
            Instances::new(transforms).colors(colors)
        };
        set(scene, ids.crops, grow(&self.crops));
        set(scene, ids.stalks, grow(&self.stalks));
        set(scene, ids.trunks, grow(&self.trunks));
        set(scene, ids.crowns, grow(&self.crowns));

        // the grow lights fill the dome, and the glass glows green from the light and the
        // damp air inside
        let lit = smoothstep(BLOOM, BLOOM + 0.5, t);
        for id in ids.inside {
            scene[id].visible = lit > 0.0;
            if let Some(light) = scene[id].light_mut() {
                *light.intensity_mut() = 500.0 * lit;
            }
        }
        let green = ease(progress(t, GREEN.0, GREEN.1));
        scene[ids.shell].visible = green > 0.0;
        if let Some(m) = scene[ids.shell].material_mut() {
            m.color = Rgb::hex(0x1f8a48) * (0.03 * green);
            m.rim = Rgb::hex(0x52ff86) * (1.0 * green);
        }
        if let Some(m) = scene[ids.panels].material_mut() {
            m.emissive_intensity = 0.012 + 0.025 * green;
        }

        // the mist of the irrigation drifts down from the nozzles on the light ring
        if let Object::Particles(p) = &mut scene[ids.mist].object {
            p.items = mist(t);
        }
        if let Object::Particles(p) = &mut scene[ids.leds].object {
            p.items.extend(glows);
        }
    }

    /// The strut printers: crawlers with a telescopic boom. They drive in while the pen
    /// draws, wake when the ring closes, lift their print heads to the top of the build and
    /// follow it up, then fold their booms and drive away.
    fn pose_printers(&self, scene: &mut Scene, t: f32, glows: &mut Vec<Particle>) {
        let ids = &self.ids;
        // they drive in from the dark while the pen draws, and away when the frame stands
        let arrive = ease(progress(t, 0.55, 1.35));
        let away = ease_in(progress(t, self.top + 0.55, self.top + 1.6));
        let wake = ease(progress(t, RING_END, RING_END + 0.35));
        let fold = ease(progress(t, self.top + 0.1, self.top + 0.6));
        let printing = wake * (1.0 - fold);
        let climb = ease(progress(t, FRAME_START + 0.1, self.top));
        let mut carriages = Vec::new();
        let mut booms = Vec::new();
        let mut heads = Vec::new();
        for k in 0..PRINTERS {
            // the printers work at the sides and the back of the ring, out of the view of
            // the camera, which stays in front (+Z)
            let spread = k as f32 / (PRINTERS - 1) as f32;
            let a = mix(1.25, TAU - 1.25, spread);
            if arrive <= 0.0 || away >= 1.0 {
                continue;
            }
            let out = around(a);
            let base = out * (TRACK + 40.0 * (1.0 - arrive) + 50.0 * away);
            carriages.push(Mat4::from_rotation_translation(
                Quat::from_rotation_y(a - FRAC_PI_2),
                base,
            ));
            // the head at rest lies along the track; at work it touches the build
            let pivot = base + Vec3::Y * 2.6;
            let rest = pivot + around(a + FRAC_PI_2) * 7.0 + Vec3::Y * 0.6;
            let height = BEAM_TOP
                + 0.5
                + (RADIUS - 0.8) * climb
                + 0.6 * (k as f32 * 1.7 + t * 5.0).sin() * (1.0 - climb);
            let level = (height - BEAM_TOP).min(RADIUS - 0.5);
            let reach = (RADIUS * RADIUS - level * level).max(0.0).sqrt() + 1.0;
            // the boom swings along the frame to print the struts beside it
            let work = around(a + 0.15 * (t * 2.0 + k as f32).sin()) * reach + Vec3::Y * height;
            let tip = rest.lerp(work, printing);
            let dir = tip - pivot;
            let length = dir.length();
            let section = (length * 0.55).min(12.0);
            booms.push(span(pivot, pivot + dir * (section / length), 0.6));
            booms.push(span(pivot + dir * (section * 0.8 / length), tip, 0.38));
            // the hydraulic ram from the carriage to the boom
            booms.push(span(
                base + out * 1.2 + Vec3::Y * 1.2,
                pivot + dir * (section * 0.45 / length),
                0.22,
            ));
            heads.push(Mat4::from_rotation_translation(
                Quat::from_rotation_arc(Vec3::Y, dir / length),
                tip,
            ));
            if printing > 0.05 {
                let nozzle = tip + dir / length * 0.5;
                glows.push(Particle::new(
                    nozzle,
                    0.25,
                    Rgb::hex(0xfff0d8) * (25.0 * printing),
                ));
                glows.push(Particle::new(
                    nozzle,
                    1.0,
                    Rgb::hex(AMBER) * (2.0 * printing),
                ));
                glows.push(Particle::new(
                    tip - out * 0.4,
                    0.14,
                    Rgb::hex(COLD) * (25.0 * printing),
                ));
                // sparks fall from the print head
                for j in 0..6u32 {
                    let seed = k as u32 * 31 + j;
                    let age = (t * 2.0 + hash(seed)).fract() * 0.5;
                    let v = vec3(hash(seed + 3) - 0.5, -0.2, hash(seed + 5) - 0.5) * 6.0;
                    let p = nozzle + v * age - Vec3::Y * (4.9 * age * age);
                    glows.push(
                        Particle::new(
                            p,
                            0.05,
                            Rgb::hex(0xffc070) * (30.0 * printing * (1.0 - 2.0 * age)),
                        )
                        .tail(-(v - Vec3::Y * 9.8 * age) * 0.02),
                    );
                }
            }
            let phase = (t * 1.5 + k as f32 * 0.37).fract();
            let blink = 6.0 + 24.0 * (-((phase - 0.5) / 0.15).powi(2)).exp();
            glows.push(Particle::new(
                base + Vec3::Y * 3.3 + out * 0.8,
                0.1,
                Rgb::hex(COLD_BLUE) * blink,
            ));
        }
        let set = |scene: &mut Scene, id: NodeId, instances: Vec<Mat4>| {
            if let Some(mesh) = scene[id].mesh_mut() {
                mesh.instances = Some(Arc::new(Instances::new(instances)));
            }
        };
        set(scene, ids.carriages, carriages);
        set(scene, ids.booms, booms);
        set(scene, ids.heads, heads);
    }

    /// The glass panels at `t` and the drones that carry them. A drone carries two panels
    /// down from above the frame in an arc, brakes nose up at their place, then lifts away
    /// and leaves the frame. Returns the panels and the drones, and adds the drone lights to
    /// `leds`. The drones grow in from nothing high above the frame and shrink to nothing
    /// as they leave, so none of them pops in or out between two frames. They move at most
    /// about 1 m per frame.
    fn pose_glass(&self, t: f32, leds: &mut Vec<Particle>) -> (Vec<Mat4>, Vec<Mat4>) {
        let mut panels = Vec::new();
        let mut drones = Vec::new();
        for (i, p) in self.panels.iter().enumerate() {
            let g = progress(t, p.set - CARRY, p.set);
            if g <= 0.0 {
                continue;
            }
            let center = p.place.transform_point3(vec3(1.0 / 3.0, 1.0 / 3.0, 0.0));
            let pair = i - i % 2;
            let side =
                p.normal.cross(Vec3::Y).normalize_or(Vec3::X) * (6.0 * (hash(pair as u32) - 0.5));
            let from = p.normal * 10.0 + Vec3::Y * (28.0 - center.y).max(10.0) + side;
            // slow at both ends: the drone starts while it is still small, and brakes at
            // the place of the panel
            let left = 1.0 - ease(g);
            let away = from * left + side * (0.6 * (PI * left).sin());
            let arrive = smoothstep(0.0, 0.3, g);
            panels.push(
                Mat4::from_translation(center + away)
                    * Mat4::from_scale(Vec3::splat(arrive))
                    * Mat4::from_translation(-center)
                    * p.place,
            );
            let leave = progress(t, p.set, p.set + LEAVE);
            let size = arrive * (1.0 - smoothstep(0.55, 1.0, leave));
            if !p.drone || size < 0.01 {
                continue;
            }
            let up = p.normal.with_y(0.0) * 5.0 + Vec3::Y * 14.0;
            let drone = center + away + p.normal * 0.5 + Vec3::Y * 0.9 + up * (leave * leave);
            let brake = Quat::from_axis_angle(
                Vec3::Y.cross(from.with_y(0.0)).normalize_or(Vec3::X),
                -2.0 * left * (1.0 - left),
            );
            let climb = Quat::from_axis_angle(
                Vec3::Y.cross(p.normal.with_y(0.0)).normalize_or(Vec3::X),
                0.35 * smoothstep(0.0, 0.5, leave),
            );
            let heading = Quat::from_rotation_y((i as f32 * 2.4 + t * 3.0).sin() * 0.3);
            drones.push(Mat4::from_scale_rotation_translation(
                Vec3::splat(size),
                climb * brake * heading,
                drone,
            ));
            // the beacon pulses three times a second; it ramps over a few frames, so a
            // hundred of them do not strobe
            let phase = (t * 3.0 + hash(i as u32)).fract();
            let blink = 8.0 + 32.0 * (-((phase - 0.5) / 0.12).powi(2)).exp();
            leds.push(Particle::new(
                drone - vec3(0.0, 0.18, 0.0) * size,
                0.08 * size,
                Rgb::hex(COLD_BLUE) * (blink * size),
            ));
            leds.push(Particle::new(
                drone + vec3(0.0, 0.1, 0.0) * size,
                0.05 * size,
                Rgb::hex(COLD) * (20.0 * size),
            ));
        }
        (panels, drones)
    }
}

/// The gasket on the outer side of a strut from `a` to `b`: a flat strip of black rubber
/// that seals the glass. It places a unit cube that stands on Y = 0..1.
fn gasket(a: Vec3, b: Vec3) -> Mat4 {
    let center = Vec3::Y * BEAM_TOP;
    let out = (a.lerp(b, 0.5) - center).normalize();
    let along = b - a;
    let side = along.cross(out).normalize_or(Vec3::X);
    Mat4::from_cols(
        (side * 0.1).extend(0.0),
        along.extend(0.0),
        (out * 0.035).extend(0.0),
        (a + out * 0.12).extend(1.0),
    )
}

/// The number of grow light bars under the light ring.
const GROW_BARS: usize = 32;

/// The ring that carries the grow lights and the mist nozzles: rods hang it from the frame,
/// and the light prints it after the frame above it stands. The rods print down from the
/// dome, then the ring prints from the foot of each rod both ways. Returns the parts, and
/// the time each of the [`GROW_BARS`] pieces of the ring is printed.
fn light_ring(hubs: &[Hub]) -> (Vec<Strut>, Vec<f32>) {
    let (radius, height) = LIGHT_RING;
    let height = height + BEAM_TOP;
    let roof = BEAM_TOP + (RADIUS * RADIUS - radius * radius).sqrt();
    let rods = 8;
    let mut parts = Vec::new();
    let mut feet = Vec::new();
    for k in 0..rods {
        let a = k as f32 / rods as f32 * TAU + 0.2;
        let top = around(a) * radius + Vec3::Y * roof;
        let foot = around(a) * radius + Vec3::Y * height;
        let reached = hubs
            .iter()
            .min_by(|p, q| {
                p.position
                    .distance(top)
                    .total_cmp(&q.position.distance(top))
            })
            .map_or(BLOOM, |h| h.arrival);
        let end = reached + (roof - height) / CLIMB;
        parts.push(Strut {
            from: top,
            to: foot,
            start: reached,
            end,
            thickness: 0.05,
            frame: false,
        });
        feet.push((a, end));
    }
    let pieces = GROW_BARS;
    let mut ends = Vec::new();
    for j in 0..pieces {
        let (a0, a1) = (
            j as f32 / pieces as f32 * TAU,
            (j + 1) as f32 / pieces as f32 * TAU,
        );
        // the piece starts when the print from the nearest rod reaches it
        let mid = f32::midpoint(a0, a1);
        let (rod, end) = feet
            .iter()
            .copied()
            .min_by(|p, q| angle_gap(p.0, mid).total_cmp(&angle_gap(q.0, mid)))
            .unwrap_or((0.0, BLOOM));
        let start = end + angle_gap(rod, mid) * radius / CLIMB;
        let (from, to) = if angle_gap(rod, a0) < angle_gap(rod, a1) {
            (a0, a1)
        } else {
            (a1, a0)
        };
        let end = start + TAU / pieces as f32 * radius / CLIMB;
        parts.push(Strut {
            from: around(from) * radius + Vec3::Y * height,
            to: around(to) * radius + Vec3::Y * height,
            start,
            end,
            thickness: 0.08,
            frame: false,
        });
        ends.push(end);
    }
    (parts, ends)
}

/// The angle between the directions `a` and `b`, `0..π`.
fn angle_gap(a: f32, b: f32) -> f32 {
    ((a - b + PI).rem_euclid(TAU) - PI).abs()
}

/// The mist of the irrigation at `t`: puffs that drift down from the nozzles of the light
/// ring and spread, lit by the grow lights.
fn mist(t: f32) -> Vec<Particle> {
    let on = smoothstep(MIST, MIST + 0.6, t);
    if on <= 0.0 {
        return Vec::new();
    }
    let (radius, height) = LIGHT_RING;
    (0..320u32)
        .filter_map(|i| {
            let life = 1.8;
            let age = (t - MIST + life * hash(i)).rem_euclid(life);
            if t - MIST < age {
                return None;
            }
            let nozzle = i % 16;
            let a = nozzle as f32 / 16.0 * TAU + 0.1;
            let drift = vec3(
                hash(i + 3) - 0.5,
                -0.6 - 0.4 * hash(i + 5),
                hash(i + 7) - 0.5,
            ) * 2.2;
            let p = around(a) * radius + Vec3::Y * (height + BEAM_TOP - 0.3) + drift * age;
            let fade = (PI * age / life).sin();
            Some(Particle::new(
                p,
                0.3 + 0.9 * age,
                Rgb::hex(0xe0f4e4) * (0.09 * fade * on),
            ))
        })
        .collect()
}

/// The radii of the rings of raised beds inside the dome.
const BEDS: [f32; 5] = [6.5, 9.0, 11.5, 14.0, 16.5];
/// The height of the crest of a raised bed, where the plants stand.
const BED_TOP: f32 = 0.52;

/// The crops in the rows of the beds, the corn, and the small trees. Each plant has its
/// transform in place, its color, and the time it starts to sprout. They sprout from the
/// middle outward after the bloom.
fn plants() -> (Vec<Plant>, Vec<Plant>, Vec<Plant>, Vec<Plant>) {
    let start = |r: f32, seed: u32| BLOOM + 0.05 + r / RADIUS * 0.9 + 0.2 * hash(seed);
    let greens = [0x4f8a2e, 0x6aa03a, 0x3d7a34, 0x86b04a, 0x5c9440];
    let mut crops = Vec::new();
    let mut stalks = Vec::new();
    // three rows in each bed, with a gap for the path to the door at +Z
    for (ring, r) in BEDS.into_iter().enumerate() {
        let count = (TAU * r / 0.55) as usize;
        for i in 0..count {
            let a = i as f32 / count as f32 * TAU;
            if !(0.12..=TAU - 0.12).contains(&a) {
                continue;
            }
            for (side, row) in [-0.5f32, 0.5, 0.0].into_iter().enumerate() {
                let seed = ((ring * 1000 + i) * 3 + side) as u32;
                let rr = r + row + 0.12 * (hash(seed * 3 + 1) - 0.5);
                let p = around(a) * rr + Vec3::Y * (BED_TOP - 0.06);
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
                        time: 0.8,
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
                        time: 0.6,
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
        let base = around(a) * r + Vec3::Y * 0.3;
        let h = match i {
            0 => 6.5,
            1..=6 => 3.6 + 1.2 * hash(i + 40),
            _ => 2.6 + 1.0 * hash(i + 40),
        };
        let begin = start(r, i + 300) - 0.2;
        trunks.push(Plant {
            transform: Mat4::from_translation(base) * Mat4::from_scale(vec3(0.16, h * 0.7, 0.16)),
            color: Rgb::hex(0x5a4030),
            start: begin,
            time: 0.7,
        });
        // a crown of many small clumps of leaves that unfold when the trunk stands
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
                start: begin + 0.12 + 0.15 * hash(i * 37 + c),
                time: 0.6,
            });
        }
    }
    (crops, stalks, trunks, crowns)
}

/// The scene template: the sky, the desert, the machines, and the parts of the dome that
/// the frames pose.
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
    // stones and dry bushes on the sand, out of the way of the track
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

    // the struts are thin: a rough finish keeps their highlights wide, so they do not
    // sparkle when the camera moves
    let steel = Material::standard(0xd6dade).roughness(0.6).metalness(0.2);
    let galvanized = Material::standard(0xa8acb0).roughness(0.45).metalness(0.6);
    let concrete = assets.concrete.material(0xd8d4cc, 0.9, 0.0);
    let cylinder = Arc::new(Geometry::cylinder(1.0, 1.0, 1.0, 8).translate(0.0, 0.5, 0.0));
    let unit = Arc::new(Geometry::cuboid(1.0, 1.0, 1.0).translate(0.0, 0.5, 0.0));

    // the warm line that the pen draws, the pen, its light, and the lamps along the ring
    let line = scene.add(
        Node::instanced(
            cylinder.clone(),
            Material::unlit(Rgb::WHITE),
            Instances::default(),
        )
        .shadows(false, false),
    );
    let pen = scene.add(Node::particles(Vec::new(), Blending::Additive));
    let pen_light = scene.add(Node::light(Light::point(AMBER, 600.0, 40.0)));
    let ring_lights = [0, 1, 2, 3].map(|k| {
        let p = ring_point((k as f32 + 0.5) / 4.0) * (1.0 + 1.2 / LINE_RADIUS) + Vec3::Y;
        scene.add(
            Node::light(Light::point(AMBER, 0.0, 22.0))
                .with_position(p)
                .hidden(),
        )
    });

    // the ring beam: segments of concrete with a rounded top, so each one comes out of the
    // sand as a thin line that widens. The anchors of the frame sit on it.
    let mut profile = vec![
        vec2(-0.45, -1.4),
        vec2(0.45, -1.4),
        vec2(0.45, BEAM_TOP - 0.15),
    ];
    for k in 1..8 {
        let a = k as f32 / 8.0 * PI;
        profile.push(vec2(0.45 * a.cos(), BEAM_TOP - 0.15 + 0.15 * a.sin()));
    }
    profile.push(vec2(-0.45, BEAM_TOP - 0.15));
    let chord = 2.0 * RADIUS * (PI / BEAM_SEGMENTS as f32).sin() + 0.03;
    let beam = scene.add(Node::instanced(
        Geometry::extrude_segments(&profile, chord, 0.0, 1),
        concrete.clone(),
        Instances::default(),
    ));
    let mut bracket = vec![Geometry::cuboid(0.8, 0.06, 0.55).translate(0.0, 0.03, 0.0)];
    for z in [-0.18f32, 0.18] {
        bracket.push(Geometry::cuboid(0.5, 0.42, 0.05).translate(0.0, 0.24, z));
    }
    for (x, z) in [(-0.3f32, -0.2f32), (0.3, -0.2), (-0.3, 0.2), (0.3, 0.2)] {
        bracket.push(Geometry::cylinder(0.04, 0.04, 0.1, 6).translate(x, 0.1, z));
    }
    let anchors = scene.add(Node::instanced(
        Geometry::merge(bracket),
        galvanized.clone(),
        Instances::default(),
    ));

    // the soil: a flat mound that comes up from the middle outward, and the raised beds
    // with two drip lines each
    let soil_material = assets.dirt.material(0x8a6a50, 1.0, 0.0);
    let mut lens = vec![vec2(RADIUS - 0.45, -0.7)];
    for k in 0..=12 {
        let r = (RADIUS - 0.5) * (1.0 - k as f32 / 12.0);
        lens.push(vec2(r, 0.3 - 0.2 * (r / (RADIUS - 0.5)).powi(2)));
    }
    let soil = scene.add(Node::mesh(
        Geometry::lathe(&lens, 96),
        soil_material.clone(),
    ));
    let beds = BEDS.map(|r| {
        let mut mound = vec![vec2(r + 0.8, -0.5)];
        for k in 0..=10 {
            let a = k as f32 / 10.0 * PI;
            mound.push(vec2(r + 0.8 * a.cos(), 0.12 + (BED_TOP - 0.12) * a.sin()));
        }
        mound.push(vec2(r - 0.8, -0.5));
        let drips = [-0.3f32, 0.3].map(|d| {
            Geometry::tube_ring(r + d + 0.025, r + d - 0.025, 0.05, 96)
                .translate(0.0, BED_TOP - 0.03, 0.0)
                .with_color(0x101010)
        });
        let geometry = Geometry::merge([
            Geometry::lathe(&mound, 96).with_color(0xffffff),
            drips[0].clone(),
            drips[1].clone(),
        ]);
        scene.add(Node::mesh(geometry, soil_material.clone()))
    });

    // the frame: struts of white steel, their hot glow while they are printed, the gaskets,
    // and the hub connectors: a plate with a cap and six bolts
    let struts = scene.add(Node::instanced(
        cylinder.clone(),
        steel.clone(),
        Instances::default(),
    ));
    let hot = scene.add(
        Node::instanced(
            cylinder.clone(),
            Material::unlit(Rgb::WHITE).additive(),
            Instances::default(),
        )
        .shadows(false, false),
    );
    let gaskets = scene.add(Node::instanced(
        unit.clone(),
        Material::standard(0x26282a).roughness(0.6),
        Instances::default(),
    ));
    let mut hub = vec![
        Geometry::cylinder(0.3, 0.3, 0.1, 16),
        Geometry::cylinder(0.17, 0.2, 0.12, 12).translate(0.0, 0.1, 0.0),
    ];
    for k in 0..6 {
        let a = k as f32 / 6.0 * TAU;
        hub.push(Geometry::cylinder(0.035, 0.035, 0.06, 6).translate(
            0.23 * a.cos(),
            0.07,
            0.23 * a.sin(),
        ));
    }
    let hubs_node = scene.add(Node::instanced(
        Geometry::merge(hub),
        galvanized.clone(),
        Instances::default(),
    ));
    let front_light = scene.add(Node::light(Light::point(AMBER, 0.0, 40.0)).hidden());

    // the strut printers: a crawler with a turret, a counterweight, a telescopic boom and
    // a print head. Local X points away from the dome.
    let mut carriage = vec![
        Geometry::rounded_box(3.4, 1.2, 2.0, 0.15, 2).translate(0.0, 1.3, 0.0),
        Geometry::cylinder(0.9, 1.0, 0.7, 20).translate(0.0, 2.25, 0.0),
        Geometry::cuboid(1.2, 1.0, 1.6).translate(1.3, 2.3, 0.0),
    ];
    for z in [-1.0f32, 1.0] {
        carriage.push(Geometry::rounded_box(3.9, 0.8, 0.6, 0.3, 3).translate(0.0, 0.4, z));
    }
    let machine = Material::standard(0x3a3e44).roughness(0.5).metalness(0.3);
    let carriages = scene.add(Node::instanced(
        Geometry::merge(carriage),
        machine.clone(),
        Instances::default(),
    ));
    let booms = scene.add(Node::instanced(
        unit.clone(),
        Material::standard(0xd8b020).roughness(0.5),
        Instances::default(),
    ));
    let heads = scene.add(Node::instanced(
        Geometry::merge([
            Geometry::cuboid(0.7, 0.9, 0.7).translate(0.0, 0.1, 0.0),
            Geometry::cylinder(0.08, 0.2, 0.35, 10).translate(0.0, 0.7, 0.0),
        ]),
        machine,
        Instances::default(),
    ));

    // the grow light bars under the light ring, and the mist nozzles
    let grow_bars = scene.add(
        Node::instanced(
            unit.clone(),
            Material::unlit(Rgb::WHITE),
            Instances::default(),
        )
        .shadows(false, false),
    );
    let nozzles = scene.add(Node::instanced(
        Geometry::cylinder(0.05, 0.09, 0.25, 8),
        galvanized,
        Instances::default(),
    ));

    // the glass: it adds the reflection of the sky, a bright edge where the view grazes it,
    // and a slight green tint
    let unit_triangle = Geometry::new(
        vec![Vec3::ZERO, Vec3::X, Vec3::Y],
        vec![Vec3::Z; 3],
        vec![Vec2::ZERO, Vec2::X, Vec2::Y],
        vec![0, 1, 2],
    );
    // rough enough that the glint of a ring light spreads over a panel instead of
    // flashing on the panels that fly past it
    let glass = Material::standard(0x000000)
        .roughness(0.14)
        .env_intensity(1.8)
        .emissive(0x52ff86, 0.0)
        .rim(Rgb::hex(0xa8d8e0) * 0.05, 4.0)
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
    let mist = scene.add(Node::particles(Vec::new(), Blending::Additive));

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
        let turn = k as f32 * if inner { 2.1 } else { TAU / 7.0 } + 0.3 * hash(k);
        blade(
            if inner { 0.22 } else { 0.32 },
            if inner { 0.7 } else { 1.0 },
        )
        .rotate_x(if inner { -0.9 } else { -0.35 })
        .rotate_y(turn)
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
    // the light of the grow lights, and a thin shell inside the glass that glows green at
    // its edges: the light in the damp air under the glass
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
        .at(0.0, BEAM_TOP, 0.0)
        .shadows(false, false)
        .hidden(),
    );
    let spark = scene.add(Node::particles(Vec::new(), Blending::Additive));

    // the blue light of the sky, and the warm light of the glow low over the horizon. The
    // sun is below the horizon, so it casts no sharp shadows.
    let dawn = scene.add(Node::light(Light::hemisphere(0x6a88d0, 0x2a2420, 0.0)));
    let from = vec3(glow.cos(), 0.06, glow.sin());
    let glow = scene.add(Node::light(Light::directional(0xff9a60, 0.0, from)));
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
        line,
        pen,
        pen_light,
        ring_lights,
        beam,
        anchors,
        soil,
        beds,
        struts,
        hot,
        gaskets,
        hubs: hubs_node,
        front_light,
        carriages,
        booms,
        heads,
        grow_bars,
        nozzles,
        panels,
        drones,
        leds,
        mist,
        crops,
        stalks,
        trunks,
        crowns,
        inside,
        shell,
        spark,
        glow,
        dawn,
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
    let front = rest(SPARK).0.with_y(0.0).normalize() * LINE_RADIUS + vec3(0.0, 0.1, 0.0);
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
