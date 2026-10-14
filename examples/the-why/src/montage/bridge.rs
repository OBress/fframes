//! Part 1, 0 to 5 s: the Golden Gate Bridge at blue hour, built by machines in time-lapse
//! in the real strait between San Francisco and the Marin Headlands. A bank of fog rolls in
//! from the ocean under the deck. The machines carry cold work lights; what they build
//! lights up warm at the end.
//!
//! - 0.0: a close shot of the warm pilot line between the future towers. It crosses the
//!   picture at its middle, over the dark hills of the bay and under the stars of the
//!   east. Beads of light run along it.
//! - 0.0 to 2.2: the camera pulls back 1.2 km from the first frame on and swings to the
//!   hill of Battery Spencer on the Marin side: the view down the bridge to the city.
//! - 0.8 to 2.15: the towers rise lift by lift, the north tower from 1.3. A creeper
//!   derrick on each leg swings the pieces up, a scaffold with work lamps climbs with it,
//!   and hot rivets fall.
//! - 2.15 to 2.4: the scaffolds come down, level by level.
//! - 2.1 to 2.35: the catwalks unroll from the towers, with a string of lamps.
//! - 2.15 to 2.85: the spinning wheels run along the catwalks, and the main cables grow
//!   around the pilot line.
//! - 2.6 to 3.25: carts clamp the cable bands, and the suspenders drop behind them, from
//!   the towers outward.
//! - 2.81 to 4.0: robotic barges bring the deck segments, gantries on the cables lift
//!   them (2.95 to 4.0), and welders join them, from the towers outward.
//! - 3.3 to 3.55: the catwalks come down, from the middle of each span.
//! - 3.75 to 4.1: the creeper derricks fold their booms and climb down the towers.
//! - 4.0 to 4.8: the lamps run on warm, the towers glow in their flood lights, and the
//!   lights of the San Francisco waterfront and of Sausalito come on outward from the
//!   bridge.
//! - 4.3 to 5.0: the first car crosses to Marin. Its headlights flare up, and the flare is
//!   the match cut to the pen of the greenhouse.

mod machines;

use std::sync::Arc;

use fframes_3d::math::{hash, progress, smoothstep};
use fframes_3d::*;

use super::bay::{self, Bay, from_bridge, geo};
use super::golden_gate::{
    self, ANCHOR, CABLE_RADIUS, CABLE_Z, HALF_SPAN, Lift, Paints, SIDE_SPAN, TOWER_TOP,
    TRUSS_DEPTH, cable_at, road_y,
};
use super::{AMBER, COLD, COLD_BLUE, camera, ease, ease_in, ease_out, mix, sky, span, stars};
use crate::assets::Assets;
use machines::Work;

const TOWERS: (f32, f32) = (0.8, 2.15);
/// The far (south) tower tops out at `FAR_TOWER_END`. The near (north) tower starts at
/// `NEAR_TOWER_START` and tops out at `TOWERS.1`: the camera sees it climb.
const FAR_TOWER_END: f32 = 2.0;
const NEAR_TOWER_START: f32 = 1.3;
/// The scaffolds come down level by level after the towers top out.
const SCAFFOLDS_OFF: (f32, f32) = (2.15, 2.4);
/// The catwalks unroll from the towers. They come down after the suspenders hang.
const CATWALKS: f32 = 2.1;
const CATWALKS_OFF: f32 = 3.3;
/// The time that the catwalks take to unroll from a tower to the middle of the span, and
/// to come down.
const SWEEP: f32 = 0.25;
const SPIN: (f32, f32) = (2.15, 2.85);
const SUSPENDERS: (f32, f32) = (2.6, 3.25);
const DECK: f32 = 2.95;
/// The deck closes in the middle of the main span.
const CLOSURE: f32 = 4.0;
/// The time of a deck segment from the barge to its place.
const LIFT: f32 = 0.22;
/// The time of a barge from its approach to its place under the deck, and away again.
const SAIL: f32 = 0.14;
/// The time that the welders join a segment after it arrives.
const WELD: f32 = 0.22;
/// The creeper derricks fold their booms, then climb down the towers.
const CRANES_OFF: (f32, f32) = (3.75, 4.1);
/// The first car starts at the south end.
const CAR_START: f32 = 4.3;
/// The time-lapse speed of the car in m/s.
const CAR_SPEED: f32 = 1900.0;
/// The bands of the city lights, by distance from the ends of the bridge.
const BANDS: usize = 6;
/// The cable segments of each main cable.
const CABLE_STEPS: usize = 240;
/// The height of the road of a segment that waits on its barge.
const ON_BARGE: f32 = 3.0 + TRUSS_DEPTH;

/// The scale of the bridge in shot 9: the main span is 256 m, so the towers stand on the
/// plain beside the canyon of the desert.
pub const LANDMARK_SCALE: f32 = 0.2;

/// The node ids that the frames animate. The bridge parts hang below a group node in the
/// space of the bridge.
struct Ids {
    lifts: Vec<NodeId>,
    machines: NodeId,
    tower_lights: [NodeId; 2],
    front_lights: [NodeId; 2],
    pilot: NodeId,
    cables: NodeId,
    catwalks: NodeId,
    suspenders: NodeId,
    deck: [NodeId; 2],
    lamps: NodeId,
    barges: [NodeId; 2],
    floods: [NodeId; 4],
    crown: NodeId,
    sparks: NodeId,
    car: NodeId,
    car_lights: NodeId,
    towers_mesh: NodeId,
    city: Vec<NodeId>,
    city_glow: [NodeId; 3],
    sutro_lights: NodeId,
    /// The sea, which mirrors the bridge.
    sea: NodeId,
    /// The stars, which ride with the camera.
    stars: NodeId,
    /// Wisps of fog on the horizon of the opening frames.
    wisps: NodeId,
}

/// A deck segment of the suspended spans or the approaches.
struct Segment {
    /// The X of its middle.
    x: f32,
    /// The time it starts to rise. The approaches stand from the start, with a time below
    /// zero.
    start: f32,
    /// The height of the ground below it. A segment over the water comes on a barge.
    ground: f32,
}

/// The Golden Gate in the strait at blue hour.
pub struct Bridge {
    scene: Scene,
    ids: Ids,
    lifts: Vec<Lift>,
    segments: Vec<Segment>,
    /// The top and the bottom of each suspender, and the time it starts to drop.
    suspenders: Vec<(Vec3, Vec3, f32)>,
    /// Over Battery Spencer: the camera of the wide shot, in the world.
    battery: Vec3,
    landmark: Scene,
}

/// The distance from the nearest tower to `x` in parts of [`HALF_SPAN`]: 0 at a tower, 1
/// in the middle of the main span, 0.54 at an anchorage.
fn from_tower(x: f32) -> f32 {
    (x.abs() - HALF_SPAN).abs() / HALF_SPAN
}

/// The time at which the deck segment at `x` starts to rise: from the towers outward.
fn segment_start(x: f32) -> f32 {
    DECK + from_tower(x) * (CLOSURE - DECK - LIFT)
}

/// The distance from the towers, in meters, to which the deck is in place at `t`.
fn deck_front(t: f32) -> f32 {
    progress(t, DECK + LIFT, CLOSURE) * HALF_SPAN
}

impl Bridge {
    pub fn new(assets: &Assets) -> Self {
        let bay = Bay::load();
        let towers = golden_gate::towers();
        let (scene, ids) = build(assets, &bay, &towers.shapes, towers.crown);
        let segments = golden_gate::segment_centers()
            .into_iter()
            .map(|x| {
                let p = from_bridge(vec3(x, 0.0, 0.0));
                Segment {
                    x,
                    start: if x.abs() < ANCHOR {
                        segment_start(x)
                    } else {
                        -1.0
                    },
                    ground: bay.land.at(p.x, p.z),
                }
            })
            .collect();
        let suspenders = golden_gate::suspenders()
            .into_iter()
            .map(|(top, bottom)| {
                (
                    top,
                    bottom,
                    mix(SUSPENDERS.0, SUSPENDERS.1 - 0.2, from_tower(top.x)),
                )
            })
            .collect();
        let battery = geo(37.8282, -122.4842, 0.0);
        let battery = battery + Vec3::Y * (bay.land.at(battery.x, battery.z) + 170.0);
        Self {
            scene,
            ids,
            lifts: towers.lifts,
            segments,
            suspenders,
            battery,
            landmark: golden_gate::bridge(&Paints::plain(), LANDMARK_SCALE, &|_, _| 0.0),
        }
    }

    /// The scene and the camera at `t` seconds, `0..5`.
    pub fn moment(&self, t: f32) -> (Scene, Camera) {
        let camera = self.camera_at(t);
        let mut scene = self.scene.clone();
        self.pose(&mut scene, t, &camera);
        if let Some(air) = &mut scene.atmosphere {
            // the fog comes in from the ocean through the Gate
            air.offset = vec3(-60.0 * t, 0.0, -8.0 * t);
        }
        // The fog bank lies on the water during the pull back. The haze does not dim the
        // mirror image of the bright pilot line and its pulses enough, so the sea mirrors
        // little until the camera is wide and the lamps of the bridge come.
        if let Some(m) = scene[self.ids.sea].material_mut() {
            m.reflection = mix(0.03, 0.8, smoothstep(2.4, 3.1, t));
        }
        // the wisps on the fog bank fade while the camera pulls back, and the fog takes
        // a little of the glow of the sky, so the bank and the sky do not meet in a hard
        // edge in the close shot
        let close = 1.0 - smoothstep(0.5, 1.3, t);
        if let Object::Particles(p) = &mut scene[self.ids.wisps].object {
            for item in &mut p.items {
                item.opacity = close;
            }
        }
        if let Some(air) = &mut scene.atmosphere {
            air.ambient = Rgb::hex(0x3a5490).lerp(Rgb::hex(0x8a6478), 0.9 * close) * 0.28;
            // thicker air in the close shot hides the far hills, so the sea fades into the
            // glow of the sky instead of meeting it at the straight top of the hills
            air.density *= mix(1.0, 3.5, close);
            // the haze reaches the far water in the close shot, so the water fades into
            // the sky instead of ending at the edge of the haze
            air.distance = mix(9000.0, 30000.0, close);
            air.steps = 36 + (28.0 * close) as u32;
        }
        scene[self.ids.stars].position = camera.position;
        if let Object::Particles(p) = &mut scene[self.ids.stars].object {
            let dt = crate::look::SHUTTER / self.shutter(t) as f32;
            super::streak_stars(&mut p.items, |t| self.camera_at(t), t, dt);
        }
        (scene, camera)
    }

    /// The motion blur samples at `t`: more on the pull back from the line, and under the
    /// glare of the car, which is wider than the gap between two samples.
    pub fn shutter(&self, t: f32) -> usize {
        match t {
            t if t < 1.8 => 3,
            t if t > 4.4 => 2,
            _ => 1,
        }
    }

    /// The finished bridge for shot 9, see [`super::Landmarks`].
    pub fn landmark(&self) -> Scene {
        self.landmark.clone()
    }

    /// Sets the parts of `scene` to the state at `t`. `camera` sets the width of the thin
    /// lines, so they stay visible far away.
    fn pose(&self, scene: &mut Scene, t: f32, camera: &Camera) {
        let eye = bay::bridge_turn().inverse() * camera.position;
        let mut work = Work::new(eye, 2.0 * (camera.fov.to_radians() * 0.5).tan() / 804.0);
        self.pose_towers(scene, t, &mut work);
        self.pose_cables(scene, t, &mut work);
        self.pose_deck(scene, t, &mut work);
        self.pose_lights(scene, t, eye, &mut work);
        let ids = &self.ids;
        if let Some(mesh) = scene[ids.machines].mesh_mut() {
            let instances = Instances::new(work.steel).colors(work.paint);
            mesh.instances = Some(Arc::new(instances));
        }
        if let Object::Particles(p) = &mut scene[ids.sparks].object {
            p.items = work.glows;
        }
    }

    /// The towers rise lift by lift. A creeper derrick rides on each leg, in a scaffold
    /// with work lamps, and hot rivets fall from the top. When the towers top out, the
    /// scaffolds come down. The derricks stay for the cables and climb down at the end.
    fn pose_towers(&self, scene: &mut Scene, t: f32, work: &mut Work) {
        let ids = &self.ids;
        let mut shapes: Vec<Vec<Mat4>> = vec![Vec::new(); ids.lifts.len()];
        // the top of each leg: [tower][leg], south tower and bay side first
        let mut tops = [[13.0f32; 2]; 2];
        let mut growing = [[false; 2]; 2];
        for lift in &self.lifts {
            // the north tower, which the camera sees from near, starts later and rises
            // faster, so that the camera sees more of its rise
            let (first, last) = if lift.transform.w_axis.x > 0.0 {
                (NEAR_TOWER_START, TOWERS.1)
            } else {
                (TOWERS.0, FAR_TOWER_END)
            };
            let start = mix(first, last - 0.1, lift.bottom / TOWER_TOP);
            let g = progress(t, start, start + 0.1);
            if g <= 0.0 {
                continue;
            }
            let grown = lift.transform * Mat4::from_scale(vec3(1.0, g.max(0.02), 1.0));
            shapes[lift.shape].push(grown);
            let at = lift.transform.w_axis;
            if (at.z.abs() - CABLE_Z).abs() < 0.01 {
                let (i, j) = (usize::from(at.x > 0.0), usize::from(at.z > 0.0));
                tops[i][j] = tops[i][j].max(mix(lift.bottom, lift.top, g));
                growing[i][j] |= g < 1.0;
            }
        }
        for (id, transforms) in ids.lifts.iter().zip(shapes) {
            if let Some(mesh) = scene[*id].mesh_mut() {
                mesh.instances = Some(Arc::new(Instances::new(transforms)));
            }
        }

        // the derricks fold their booms and climb down the legs, then they are gone
        let fold = ease(progress(t, CRANES_OFF.0, CRANES_OFF.0 + 0.12));
        let down = ease_in(progress(t, CRANES_OFF.0 + 0.1, CRANES_OFF.1));
        let working = t > TOWERS.0 - 0.1 && down < 1.0;
        let levels = 3 - (3.0 * progress(t, SCAFFOLDS_OFF.0, SCAFFOLDS_OFF.1)).round() as usize;
        for (i, x) in [-HALF_SPAN, HALF_SPAN].into_iter().enumerate() {
            for (j, z) in [-CABLE_Z, CABLE_Z].into_iter().enumerate() {
                let top = vec3(x, tops[i][j], z);
                let leg = golden_gate::leg_size(top.y - 0.5);
                let phase = i as f32 * 1.7 + j as f32 * 2.9;
                if working {
                    // the boom swings out over the water, picks a piece and swings it in
                    let climbing = t < TOWERS.1;
                    let swing = (t * 5.0 + phase).sin();
                    let slew = mix(0.2, 0.9 * swing, if climbing { 1.0 } else { 0.4 });
                    let luff = mix(1.05 + 0.2 * (t * 1.7 + phase).sin(), 1.48, fold);
                    let hook = mix(9.0 + 26.0 * (0.5 + 0.5 * swing), 4.0, fold);
                    let lowered = mix(top.y, road_y(x) + 2.0, down);
                    let out = z.signum();
                    machines::creeper(
                        work,
                        top.with_y(lowered),
                        leg,
                        out,
                        slew,
                        luff,
                        hook,
                        climbing,
                        t,
                    );
                }
                if t > TOWERS.0 - 0.1 {
                    machines::scaffold(work, top, leg, levels);
                }
                if growing[i][j] {
                    // hot rivets that the riveters drop from the top of the leg
                    for (k, corner) in [(-1.0, 1.0), (1.0, -1.0)].into_iter().enumerate() {
                        let p = top + vec3(corner.0 * leg.x * 0.5, -1.0, corner.1 * leg.y * 0.5);
                        work.sparks(p, t, (i * 4 + j * 2 + k) as u32, 14, 9.0, 0.45);
                    }
                }
            }
            let light = &mut scene[ids.tower_lights[i]];
            light.visible = working && fold < 1.0;
            light.position = vec3(x, tops[i][0].max(tops[i][1]) + 12.0, 0.0);
            if let Some(light) = light.light_mut() {
                *light.intensity_mut() =
                    1.8e4 * smoothstep(TOWERS.0 - 0.1, TOWERS.0, t) * (1.0 - fold);
            }
        }
    }

    /// The catwalks unroll from the towers, the spinning wheels run along them while the
    /// main cables grow, the suspenders drop behind the carts that clamp the cable bands,
    /// and the catwalks come down.
    fn pose_cables(&self, scene: &mut Scene, t: f32, work: &mut Work) {
        let ids = &self.ids;
        let spun = ease(progress(t, SPIN.0, SPIN.1));
        let x_at = |k: usize| -ANCHOR + 2.0 * ANCHOR * k as f32 / CABLE_STEPS as f32;
        let step = |k: usize, z: f32| (cable_at(x_at(k), z), cable_at(x_at(k + 1), z));
        let mut cables = Vec::new();
        let mut walks = Vec::new();
        let tramway = t > SPIN.0 - 0.1 && t < SPIN.1 + 0.1;
        for z in [-CABLE_Z, CABLE_Z] {
            for k in 0..CABLE_STEPS {
                let (a, b) = step(k, z);
                if spun > 0.0 {
                    let r = (CABLE_RADIUS * spun).max(0.6 * work.pixel_at(a) * spun);
                    cables.push(span(a, b, r));
                }
                if tramway {
                    let up = Vec3::Y * 3.5;
                    walks.push(span(a + up, b + up, 0.05_f32.max(0.35 * work.pixel_at(a))));
                }
                // the catwalk: a deck of mesh under the cable with two hand ropes, from the
                // towers out and back
                let x = a.x.midpoint(b.x);
                let near = from_tower(x);
                let shown = t > CATWALKS + SWEEP * near && t < CATWALKS_OFF + SWEEP * (1.0 - near);
                if !shown {
                    continue;
                }
                let down = Vec3::Y * 1.6;
                let thick = 0.15_f32.max(0.5 * work.pixel_at(a));
                walks.push(band(a - down, b - down, thick, 2.6));
                for side in [-1.3, 1.3] {
                    let rope = vec3(0.0, -0.5, side);
                    walks.push(span(
                        a + rope,
                        b + rope,
                        0.05_f32.max(0.35 * work.pixel_at(a)),
                    ));
                }
                if k % 10 == 5 && z < 0.0 {
                    // a cross bridge to the other catwalk
                    let c = vec3(a.x, a.y - 1.6, -z);
                    walks.push(band(a - down, c, thick, 1.4));
                }
                if k % 3 == 0 {
                    let lamp = a - down + vec3(0.0, 1.3, 1.3 * z.signum());
                    work.glow(lamp, 0.35, 0.9, Rgb::hex(COLD) * 14.0);
                }
            }
        }
        if let Some(mesh) = scene[ids.cables].mesh_mut() {
            mesh.instances = Some(Arc::new(Instances::new(cables)));
        }
        if let Some(mesh) = scene[ids.catwalks].mesh_mut() {
            mesh.instances = Some(Arc::new(Instances::new(walks)));
        }
        // the pilot line: warm and about one pixel wide at any distance
        let pilot: Vec<Mat4> = (0..CABLE_STEPS)
            .map(|k| {
                let (a, b) = step(k, -CABLE_Z);
                span(a, b, (0.7 * work.pixel_at(a)).max(0.012))
            })
            .collect();
        if let Some(mesh) = scene[ids.pilot].mesh_mut() {
            mesh.instances = Some(Arc::new(Instances::new(pilot)));
        }
        scene[ids.pilot].visible = spun < 0.9;
        // pulses of light run along the pilot line while the camera is near it: the warm
        // light on its way, 2 m apart, each a dot wider than the line with a short streak
        // that joins the motion blur samples
        let near = 1.0 - smoothstep(0.5, 0.9, t);
        if near > 0.0 {
            for k in 0..1000u32 {
                let x =
                    (t * 5.0 + k as f32 * 2.0 + 0.9 * hash(k)).rem_euclid(2.0 * ANCHOR) - ANCHOR;
                let p = cable_at(x, -CABLE_Z);
                let size = 0.05_f32.max(2.0 * work.pixel_at(p));
                work.glows.push(
                    Particle::new(p, size, Rgb::hex(0xfff0d8) * (40.0 * near))
                        .tail(cable_at(x + 0.08, -CABLE_Z) - p),
                );
            }
        }

        // the spinning wheels: three on each cable, from anchorage to anchorage and back.
        // Each pulls a wire that glints behind it.
        if tramway {
            for (i, z) in [-CABLE_Z, CABLE_Z].into_iter().enumerate() {
                for w in 0..3 {
                    let phase = (t - SPIN.0) * 1.1 + w as f32 / 3.0 + i as f32 * 0.17;
                    let s = 1.0 - (2.0 * phase.rem_euclid(1.0) - 1.0).abs();
                    let back = if phase.rem_euclid(1.0) < 0.5 {
                        -1.0
                    } else {
                        1.0
                    };
                    let x = ANCHOR * 0.98 * (2.0 * s - 1.0);
                    let head = cable_at(x, z) + Vec3::Y * 2.2;
                    work.glow(head, 0.8, 2.2, Rgb::hex(COLD) * 50.0);
                    for k in 0..6 {
                        let fade = 1.0 - k as f32 / 6.0;
                        let (p, q) = (
                            cable_at(x + back * 12.0 * k as f32, z),
                            cable_at(x + back * 12.0 * (k + 1) as f32, z),
                        );
                        let size = (0.3 * fade).max(0.8 * work.pixel_at(p));
                        work.glows.push(
                            Particle::new(p + Vec3::Y * 2.2, size, Rgb::hex(COLD) * (25.0 * fade))
                                .tail(q - p),
                        );
                    }
                }
            }
        }

        // the suspenders drop from the cables behind the carts that clamp their bands
        let ropes: Vec<Mat4> = self
            .suspenders
            .iter()
            .filter_map(|&(top, bottom, start)| {
                let g = ease_out(progress(t, start, start + 0.25));
                (g > 0.0).then(|| {
                    span(
                        top,
                        top.lerp(bottom, g),
                        0.07_f32.max(0.45 * work.pixel_at(top)),
                    )
                })
            })
            .collect();
        if let Some(mesh) = scene[ids.suspenders].mesh_mut() {
            mesh.instances = Some(Arc::new(Instances::new(ropes)));
        }
        let reach = progress(t, SUSPENDERS.0, SUSPENDERS.1 - 0.2) * HALF_SPAN;
        if reach > 0.0 && reach < HALF_SPAN {
            for tower in [-HALF_SPAN, HALF_SPAN] {
                for (dir, limit) in [(-tower.signum(), HALF_SPAN), (tower.signum(), SIDE_SPAN)] {
                    if reach > limit - 10.0 {
                        continue;
                    }
                    for z in [-CABLE_Z, CABLE_Z] {
                        let p = cable_at(tower + dir * reach, z) + Vec3::Y * 0.8;
                        work.block(p - Vec3::Y * 0.6, vec3(4.0, 1.4, 1.8), 0xd8a62c);
                        work.glow(p + Vec3::Y * 1.2, 0.5, 1.6, Rgb::hex(COLD) * 40.0);
                    }
                }
            }
        }
    }

    /// The deck: robotic barges bring the segments under their places, gantries on the
    /// cables lift them, and welders join each one to the deck. The lamps on the deck
    /// light up from the middle outward at the end.
    fn pose_deck(&self, scene: &mut Scene, t: f32, work: &mut Work) {
        let ids = &self.ids;
        let frame = (t * 60.0) as u32;
        let mut deck = Vec::new();
        let mut lamp_colors = Vec::new();
        let mut barges = Vec::new();
        for (n, s) in self.segments.iter().enumerate() {
            let x = s.x;
            let road = road_y(x);
            let water = s.ground < -1.0;
            let arrive = s.start - if water { SAIL } else { 0.05 };
            if s.start >= 0.0 && t < arrive {
                continue;
            }
            let g = progress(t, s.start, s.start + LIFT);
            // the barge comes in from the bay and leaves to the ocean, each on its own path
            let seed = n as u32 * 3;
            let sail = (50.0 + 50.0 * hash(seed)) * (1.0 - ease_out(progress(t, arrive, s.start)))
                - (50.0 + 50.0 * hash(seed + 1))
                    * ease_in(progress(t, s.start + LIFT, s.start + LIFT + SAIL));
            let rest = if water {
                ON_BARGE
            } else {
                s.ground + TRUSS_DEPTH + 1.0
            };
            let y = mix(rest, road, ease(g));
            let on_barge = if g > 0.0 { 0.0 } else { sail };
            let segment =
                Mat4::from_translation(vec3(0.0, y - road, on_barge)) * golden_gate::segment_at(x);
            deck.push(segment);
            let on = CLOSURE + 0.05 + x.abs() / ANCHOR * 0.5;
            lamp_colors.push(
                Rgb::hex(0x0c0d10).lerp(Rgb::hex(0xffa850) * 10.0, smoothstep(on, on + 0.08, t)),
            );
            if s.start < 0.0 {
                continue;
            }
            let after = t - (s.start + LIFT);
            if water && after < SAIL {
                // it turns a little on its way in and out
                let turn = Quat::from_rotation_y((hash(seed + 2) - 0.5) * 0.004 * sail);
                let p = vec3(x, 0.0, sail);
                barges.push(Mat4::from_rotation_translation(turn, p));
                machines::barge_lights(work, p, turn, t + n as f32 * 0.3);
            }
            if g > 0.0 && g < 1.0 {
                let cables = [-CABLE_Z, CABLE_Z].map(|z| cable_at(x, z));
                machines::gantry(work, cables, y + 0.3, t + n as f32 * 0.11);
            }
            if after > 0.0 && after < WELD {
                // the joint toward the tower, at the top and the bottom chord of each truss
                let tower = HALF_SPAN * x.signum();
                let joint = x + (tower - x).signum() * golden_gate::SEGMENT * 0.5;
                for (k, z) in [-CABLE_Z, CABLE_Z].into_iter().enumerate() {
                    for (m, dy) in [-0.6, -TRUSS_DEPTH + 0.4].into_iter().enumerate() {
                        let p = vec3(joint, road_y(joint) + dy, z);
                        work.weld(p, after, (n * 4 + k * 2 + m) as u32, frame);
                    }
                }
            }
        }
        for id in ids.deck {
            if let Some(mesh) = scene[id].mesh_mut() {
                mesh.instances = Some(Arc::new(Instances::new(deck.clone())));
            }
        }
        if let Some(mesh) = scene[ids.lamps].mesh_mut() {
            mesh.instances = Some(Arc::new(Instances::new(deck).colors(lamp_colors)));
        }
        let barges = Arc::new(Instances::new(barges));
        for id in ids.barges {
            if let Some(mesh) = scene[id].mesh_mut() {
                mesh.instances = Some(barges.clone());
            }
        }
        // cold lights on the two fronts of the deck that grow from the near tower: they
        // shine down on the segments that rise, and through the fog on the barges
        let front = deck_front(t);
        for (i, (dir, limit)) in [(-1.0f32, HALF_SPAN), (1.0, SIDE_SPAN)]
            .into_iter()
            .enumerate()
        {
            let x = HALF_SPAN + dir * (front.min(limit - 20.0) + 14.0);
            let node = &mut scene[ids.front_lights[i]];
            node.visible = t > DECK + LIFT && front < limit - 5.0;
            node.position = vec3(x, road_y(x) + 9.0, 0.0);
        }
    }

    /// The lights of the finished bridge, the city, and the first car.
    fn pose_lights(&self, scene: &mut Scene, t: f32, eye: Vec3, work: &mut Work) {
        let ids = &self.ids;
        // the flood lights of the towers and the red lights on their tops
        let floods = smoothstep(CLOSURE + 0.15, CLOSURE + 0.5, t);
        for id in ids.floods {
            scene[id].visible = floods > 0.0;
            if let Some(light) = scene[id].light_mut() {
                *light.intensity_mut() = 9.0e5 * floods;
            }
        }
        if let Some(m) = scene[ids.crown].material_mut() {
            m.emissive = Rgb::hex(0xffa060);
            m.emissive_intensity = 0.6 * floods;
        }
        if t > TOWERS.1 && (t * 1.1).fract() < 0.55 {
            for x in [-HALF_SPAN, HALF_SPAN] {
                for z in [-CABLE_Z, CABLE_Z] {
                    let p = vec3(x, TOWER_TOP + 3.0, z);
                    work.glows
                        .push(Particle::new(p, 1.6, Rgb::hex(0xff2010) * 30.0));
                }
            }
        }

        // the city lights up outward from the ends of the bridge, Sausalito too
        for (band, id) in ids.city.iter().enumerate() {
            let at = CLOSURE + 0.2 + band as f32 * 0.12;
            let on = smoothstep(at, at + 0.25, t);
            if let Some(m) = scene[*id].material_mut() {
                m.color = Rgb::WHITE * mix(0.25, 1.6, on);
            }
        }
        let city = smoothstep(CLOSURE + 0.4, CLOSURE + 0.9, t);
        if let Some(m) = scene[ids.towers_mesh].material_mut() {
            m.emissive_intensity = mix(0.4, 2.2, city);
        }
        for id in ids.city_glow {
            if let Some(light) = scene[id].light_mut() {
                *light.intensity_mut() = mix(2.0e5, 9.0e5, city);
            }
        }
        if let Object::Particles(p) = &mut scene[ids.sutro_lights].object {
            let blink = if (t * 0.9).fract() < 0.5 { 30.0 } else { 3.0 };
            for item in &mut p.items {
                item.color = Rgb::hex(0xff2010) * blink;
            }
        }

        // the first car
        let car = &mut scene[ids.car];
        car.visible = t > CAR_START;
        car.position = car_position(t);
        // its head lights flare up before the cut; the glare is a lens effect that sits
        // 30 m nearer to the camera on the same ray, so the deck does not hide it
        let flare = ease_in(progress(t, 4.62, 5.0));
        let to_camera = (eye - car_position(t)).normalize();
        if let Object::Particles(p) = &mut scene[ids.car_lights].object {
            p.items[0].position = vec3(2.4, 0.8, 0.0) + to_camera * 30.0;
            p.items[0].size = 4.0 + 12.0 * flare;
            p.items[0].color = Rgb::hex(AMBER) * (3.0 + 32.0 * flare);
        }
    }

    /// The camera at `t`. It starts 10 m from the pilot line on the ocean side with a long
    /// lens, at its height, so the line lies on the horizon: the fog of the bay under it,
    /// the stars over it. It pulls back and swings up over Battery Spencer, drifts while the bridge
    /// grows, and at the end turns to the headlights of the first car.
    fn camera_at(&self, t: f32) -> Camera {
        let line = from_bridge(cable_at(0.0, -CABLE_Z));
        let start = from_bridge(cable_at(0.0, -CABLE_Z) + vec3(0.0, 0.4, -10.0));
        let wide_target = from_bridge(vec3(260.0, 70.0, 0.0));
        let car = from_bridge(car_position(5.0));

        // The pull back: the distance grows evenly in log space, and the direction turns
        // from the west to the Marin hill. It moves from the first frame on, so the strait
        // opens up under the line within the first second, and settles at the end.
        let x = progress(t, 0.0, 2.2);
        let pull = -1.3 * x * x * x + 1.6 * x * x + 0.7 * x;
        let turn = ease(progress(t, 0.35, 2.2));
        let d0 = start.distance(line);
        let d1 = self.battery.distance(wide_target);
        let distance = (d0.ln() + (d1.ln() - d0.ln()) * pull).exp();
        let dir = ((start - line) / d0)
            .lerp((self.battery - wide_target) / d1, turn)
            .normalize();
        let tilt = ease(progress(t, 0.9, 2.3));
        let mut target = line.lerp(wide_target, tilt);
        let mut position = target + dir * distance;
        let mut mm = mix(85.0, 24.0, ease(progress(t, 0.1, 1.9)));

        // the slow push while the bridge grows, then the turn to the car
        let drift = ease(progress(t, 2.2, 5.4));
        position += (wide_target - self.battery) * 0.08 * drift;
        let follow = ease(progress(t, CAR_START - 0.4, 5.0));
        target = target.lerp(car, follow);
        mm = mix(mm, 28.0, follow);

        let focus = line.lerp(target, tilt);
        let blur = mix(9.0, 2.5, ease(progress(t, 0.1, 1.2)));
        camera(mm, position, target)
            .focus_on(focus, blur)
            .clip(0.5, 30_000.0)
            .shake(t, 0.0004, 5)
    }
}

/// The position of the first car at `t`, in the space of the bridge: on the lanes to
/// Marin, nearer to the bay.
fn car_position(t: f32) -> Vec3 {
    let x = -ANCHOR + CAR_SPEED * (t - CAR_START);
    vec3(x, road_y(x) + 0.05, 5.0)
}

/// The transform of a flat bar from `a` to `b`: `thickness` across the bar in the plane of
/// the cable, and `width` along Z. It places the unit box of [`span`].
fn band(a: Vec3, b: Vec3, thickness: f32, width: f32) -> Mat4 {
    let d = b - a;
    let rotation = Quat::from_rotation_arc(Vec3::Y, d.normalize_or(Vec3::Y));
    Mat4::from_scale_rotation_translation(vec3(thickness, d.length(), width), rotation, a)
}

/// Builds the scene: the sky, the place, the bridge parts and the lights.
fn build(assets: &Assets, bay: &Bay, shapes: &[Geometry], crown: usize) -> (Scene, Ids) {
    // the last light of the day low in the west-south-west, the deep blue sky of the east
    let west = vec3(-0.94, 0.0, 0.34);
    let mut scene = Scene::new()
        .environment(Environment::map(
            sky(
                Rgb::hex(0x0b1838) * 0.9,
                Rgb::hex(0x5a78b0) * 0.4,
                Rgb::hex(0xff8a50) * 0.7,
                west.z.atan2(west.x),
                0.0,
            ),
            1.0,
        ))
        .background(Background::Environment { blur: 0.0 })
        .reflector(Reflector::new(Vec3::ZERO, Vec3::Y).scale(0.4));

    // the place
    let land = bay::land_mesh(bay);
    scene.add(Node::mesh(land, assets.dirt.material(0xffffff, 1.0, 0.0)).shadows(false, false));
    let sea = scene.add(bay::water());
    scene.add(bay::presidio(bay));
    let towers_mesh = scene.add(bay::towers(bay));
    let (sutro, sutro_spots) = bay::sutro(bay);
    scene.add(sutro);
    let sutro_lights = scene.add(Node::particles(
        sutro_spots
            .into_iter()
            .map(|p| Particle::new(p, 3.0, Rgb::hex(0xff2010) * 20.0))
            .collect(),
        Blending::Additive,
    ));
    let ends = [
        from_bridge(vec3(-ANCHOR, 0.0, 0.0)),
        from_bridge(vec3(ANCHOR, 0.0, 0.0)),
    ];
    let (texture, bands) = bay::city_lights(bay, ends, BANDS);
    let city = bands
        .into_iter()
        .map(|g| {
            let lights = Material::unlit(Rgb::WHITE).map(texture.clone()).additive();
            scene.add(Node::mesh(g, lights).shadows(false, false))
        })
        .collect();
    // the glow of the city in the haze: high, so it is a wide dome and not a spot
    let city_glow = [
        geo(37.800, -122.440, 450.0),
        geo(37.790, -122.405, 450.0),
        geo(37.857, -122.485, 350.0),
    ]
    .map(|p| scene.add(Node::light(Light::point(0xffb070, 0.0, 4000.0)).with_position(p)));

    // the bridge, in its own space
    let paints = Paints {
        // International Orange with the dents and the roughness of worn steel plate
        steel: Material::standard(golden_gate::ORANGE)
            .roughness(0.7)
            .normal_map(assets.steel.normal.clone())
            .ao_map(assets.steel.orm.clone()),
        concrete: assets.concrete.material(0xc4bcb0, 0.9, 0.0),
        road: assets.asphalt.material(0x707070, 0.9, 0.0),
    };
    let gate = scene.add(Node::group().with_rotation(bay::bridge_turn()));
    let ground = |x: f32, z: f32| {
        let p = from_bridge(vec3(x, 0.0, z));
        bay.land.at(p.x, p.z)
    };
    let lifts: Vec<NodeId> = shapes
        .iter()
        .map(|g| {
            let node = Node::instanced(g.clone(), paints.steel.clone(), Instances::default());
            scene.add_to(gate, node)
        })
        .collect();
    let crown = lifts[crown];
    let concrete = golden_gate::concrete(&ground);
    scene.add_to(gate, Node::mesh(concrete, paints.concrete.clone()));
    let fort = ground(-ANCHOR - 70.0, -20.0);
    scene.add_to(
        gate,
        Node::mesh(golden_gate::fort_point_arch(fort), paints.steel.clone()),
    );
    scene.add_to(
        gate,
        Node::mesh(
            golden_gate::fort_point(fort),
            Material::standard(0x7a4a3a).roughness(0.9),
        ),
    );
    let unit = Arc::new(Geometry::cylinder(1.0, 1.0, 1.0, 8).translate(0.0, 0.5, 0.0));
    let instanced =
        |material: Material| Node::instanced(unit.clone(), material, Instances::default());
    let cables = scene.add_to(gate, instanced(paints.steel.clone()));
    let pilot = scene.add_to(
        gate,
        instanced(Material::unlit(Rgb::hex(AMBER) * 9.0)).shadows(false, false),
    );
    let block = Arc::new(Geometry::cuboid(1.0, 1.0, 1.0).translate(0.0, 0.5, 0.0));
    let catwalks = scene.add_to(
        gate,
        Node::instanced(
            block.clone(),
            Material::standard(0x4a4640).roughness(0.9),
            Instances::default(),
        )
        .shadows(false, false),
    );
    let suspenders = scene.add_to(gate, instanced(paints.steel.clone()).shadows(false, false));
    let (steel, road, lamp_heads) = golden_gate::deck_segment();
    let deck = [(steel, paints.steel.clone()), (road, paints.road.clone())]
        .map(|(g, m)| scene.add_to(gate, Node::instanced(g, m, Instances::default())));
    let lamps = scene.add_to(
        gate,
        Node::instanced(
            lamp_heads,
            Material::unlit(Rgb::WHITE),
            Instances::default(),
        )
        .shadows(false, false),
    );

    // the machines: their steel is one list of boxes with a paint each, which the frames
    // fill, see [`Work`]
    let machines = scene.add_to(
        gate,
        Node::instanced(
            block,
            Material::standard(0xffffff).roughness(0.6).metalness(0.3),
            Instances::default(),
        )
        .shadows(false, false),
    );
    let (hull, strips) = machines::barge();
    let barges = [
        (hull, Material::standard(0x2a2c30).roughness(0.7)),
        (strips, Material::unlit(Rgb::hex(COLD_BLUE) * 6.0)),
    ]
    .map(|(g, m)| {
        scene.add_to(
            gate,
            Node::instanced(g, m, Instances::default()).shadows(false, false),
        )
    });
    // the work lights over the tops of the towers, and on the fronts of the deck that grow
    // from the near tower
    let tower_lights =
        [0; 2].map(|_| scene.add_to(gate, Node::light(Light::point(COLD, 0.0, 300.0)).hidden()));
    let front_lights = [-0.5, 0.5].map(|x| {
        scene.add_to(
            gate,
            Node::light(Light::spot(COLD, 3.0e4, 0.45, 0.7).pointing(vec3(x, -1.0, 0.1))).hidden(),
        )
    });
    // flood lights on the deck that shine up the north faces of the legs, the faces that
    // Marin sees
    let floods = [(-1.0f32, -1.0f32), (-1.0, 1.0), (1.0, -1.0), (1.0, 1.0)].map(|(side, z)| {
        let x = side * HALF_SPAN;
        let at = vec3(x + 45.0, road_y(x) - 4.0, z * CABLE_Z);
        scene.add_to(
            gate,
            Node::light(Light::spot(0xffc890, 0.0, 0.3, 0.9).pointing(vec3(-0.22, 1.0, 0.0)))
                .with_position(at)
                .hidden(),
        )
    });
    let sparks = scene.add_to(gate, Node::particles(Vec::new(), Blending::Additive));

    // the first car: a dark body with head and tail lights
    let car = scene.add_to(gate, Node::group().hidden());
    scene.add_to(
        car,
        Node::mesh(
            Geometry::rounded_box(4.6, 1.4, 1.9, 0.3, 2).translate(0.0, 0.9, 0.0),
            Material::standard(0x1a1c20).roughness(0.3).clearcoat(1.0),
        ),
    );
    let car_lights = scene.add_to(
        car,
        Node::particles(
            vec![
                Particle::new(vec3(3.2, 0.8, 0.0), 4.0, Rgb::hex(AMBER) * 3.0),
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

    // the afterglow in the west-south-west and the light of the blue sky
    scene.add(Node::light(Light::directional(
        0xff9a70,
        0.5,
        west + Vec3::Y * 0.1,
    )));
    scene.add(Node::light(Light::hemisphere(0x5a78c0, 0x14161c, 1.6)));
    // the stars of the east, faint in the blue hour. The frames move them with the camera.
    // The haze over 4.5 km dims them to a tenth, so they are bright.
    let sky_stars = scene.add(Node::particles(stars(Vec3::ZERO, 3.5), Blending::Additive));
    // wisps on the top of the fog bank, 300 m to 1.8 km ahead of the first camera at the
    // height of its horizon: soft, uneven, in the colors of the glow over them
    let eye = cable_at(0.0, -CABLE_Z) + vec3(0.0, 0.4, -10.0);
    let wisps = (0..90u32)
        .map(|i| {
            let d = 300.0 + 1500.0 * hash(i * 5 + 1).powf(1.5);
            let x = (hash(i * 5 + 2) - 0.5) * 1.4 * d;
            // the far water meets the sky about 0.7 degrees under the horizon
            let y = eye.y - d * (0.012 + (hash(i * 5 + 3) - 0.5) * 0.018);
            let size = d * (0.025 + 0.06 * hash(i * 5 + 4));
            let tint = Rgb::hex(0xb08090).lerp(Rgb::hex(0x7080b0), hash(i * 5 + 5));
            Particle::new(vec3(eye.x + x, y, eye.z + d), size, tint * 0.14)
        })
        .collect();
    let wisps = scene.add_to(gate, Node::particles(wisps, Blending::Additive));
    // the fog: thick on the water, thin above the deck, in banks that drift in
    scene.atmosphere = Some(Atmosphere {
        density: 0.014,
        color: Rgb::WHITE,
        base: 0.0,
        falloff: 0.045,
        anisotropy: 0.35,
        ambient: Rgb::hex(0x3a5490) * 0.28,
        noise: 0.95,
        noise_size: 110.0,
        offset: Vec3::ZERO,
        distance: 9000.0,
        steps: 36,
    });

    let ids = Ids {
        lifts,
        machines,
        tower_lights,
        front_lights,
        pilot,
        cables,
        catwalks,
        suspenders,
        deck,
        lamps,
        barges,
        floods,
        crown,
        sparks,
        car,
        car_lights,
        towers_mesh,
        city,
        city_glow,
        sutro_lights,
        sea,
        stars: sky_stars,
        wisps,
    };
    (scene, ids)
}
