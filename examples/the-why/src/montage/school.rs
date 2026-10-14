//! A quick build at night: a gantry printer prints a village school in concrete. The cold
//! light towers of the site go out when the warm windows come on. The times are seconds
//! into the part.
//!
//! - 0.0 to 0.12: the camera trucks right past a tree of the school yard, which leaves the
//!   picture to the left: the end of the wipe from the wind farm. The warm line of the
//!   footprint glows on the ground under the gantry.
//! - 0.06 to 0.62: the nozzle runs twice along the line, seven layers at a time in a ramp,
//!   and the walls rise. The fresh layers glow warm and cool to grey concrete.
//! - 0.6 to 0.76: the roof slabs come down on the walls, and the windows go in.
//! - 0.72: the light towers go out. 0.74 to 0.92: the windows light up warm, from the door
//!   outward.
//! - 0.84 to 1.06: a flag runs up the pole in front of the school.
//! - 1.0 to 1.25: the camera tilts up after the flag into the stars: the cut to the water.

use std::f32::consts::{FRAC_PI_2, PI, TAU};
use std::sync::Arc;

use fframes_3d::math::{hash, progress, smoothstep};
use fframes_3d::*;

use super::{
    AMBER, AMBER_EDGE, BUILD_RATE, COLD, COLD_BLUE, SKY_RATE, SKY_TILT, camera, ease, ease_out,
    fbm, mix, polar, random_in_ball, sky, span, stars, streak_stars,
};
use crate::assets::Assets;
use crate::look::SHUTTER;

/// The length of the part in seconds.
pub const LENGTH: f32 = 1.25;
/// Half the size of the footprint, at the middle of the walls, in meters.
const HALF: Vec2 = Vec2::new(16.0, 6.0);
/// The thickness of a wall and the height of a printed layer.
const WALL: f32 = 0.42;
const LAYER: f32 = 0.3;
const LAYERS: usize = 14;
/// The height of the walls.
const WALL_HEIGHT: f32 = LAYER * LAYERS as f32;
/// The perimeter that the nozzle runs for each layer.
const PERIMETER: f32 = 4.0 * (HALF.x + HALF.y);
const PRINT: (f32, f32) = (0.06, 0.62);
/// The roof slabs come down one after the other.
const ROOF: (f32, f32) = (0.6, 0.76);
/// The light towers go out, and the windows light up from the door outward.
const TOWERS_OUT: f32 = 0.72;
const LIGHTS_ON: (f32, f32) = (0.74, 0.92);
const FLAG: (f32, f32) = (0.84, 1.06);
/// The height of the beams of the gantry.
const GANTRY: f32 = 7.6;
/// The gantry stands on six columns at these X, on both sides at Z = ±`GANTRY_Z`.
const GANTRY_X: [f32; 3] = [-18.0, 0.0, 18.0];
const GANTRY_Z: f32 = 8.5;
/// The foot of the flag pole and its height.
const POLE: Vec3 = Vec3::new(9.5, 0.0, 13.0);
const POLE_HEIGHT: f32 = 9.0;
/// The camera when the truck ends, and the length of the truck. The tree of the yard
/// stands in front of the camera at the start.
const CAMERA_REST: Vec3 = Vec3::new(-17.0, 5.5, 50.0);
const TRUCK: f32 = 20.0;
const TRUCK_TIME: f32 = 0.5;
/// The point that the camera looks at before the tilt.
const VIEW: Vec3 = Vec3::new(1.5, 3.0, 0.0);

/// The four walls: the corner where each starts, counterclockwise from the front left
/// when seen from above. The front faces the camera (+Z).
const CORNERS: [Vec2; 4] = [
    Vec2::new(-HALF.x, HALF.y),
    Vec2::new(HALF.x, HALF.y),
    Vec2::new(HALF.x, -HALF.y),
    Vec2::new(-HALF.x, -HALF.y),
];

/// The openings: the wall, the middle along the wall from its corner, the width, and the
/// bottom and top heights. The door is the middle of the front.
const OPENINGS: [(usize, f32, f32, f32, f32); 14] = [
    (0, 3.0, 2.2, 0.9, 2.7),
    (0, 7.0, 2.2, 0.9, 2.7),
    (0, 11.0, 2.2, 0.9, 2.7),
    (0, 16.0, 2.0, 0.0, 2.7),
    (0, 21.0, 2.2, 0.9, 2.7),
    (0, 25.0, 2.2, 0.9, 2.7),
    (0, 29.0, 2.2, 0.9, 2.7),
    (1, 6.0, 2.4, 0.9, 2.7),
    (2, 5.0, 2.2, 0.9, 2.7),
    (2, 11.0, 2.2, 0.9, 2.7),
    (2, 21.0, 2.2, 0.9, 2.7),
    (2, 27.0, 2.2, 0.9, 2.7),
    (3, 4.0, 2.4, 0.9, 2.7),
    (3, 8.0, 2.4, 0.9, 2.7),
];

/// The start of wall `w`, its direction and its length.
fn wall(w: usize) -> (Vec3, Vec3, f32) {
    let (a, b) = (CORNERS[w], CORNERS[(w + 1) % 4]);
    let d = b - a;
    (
        vec3(a.x, 0.0, a.y),
        vec3(d.x, 0.0, d.y).normalize(),
        d.length(),
    )
}

/// The point at `s` meters along the perimeter from the front left corner, on the ground.
fn along(s: f32) -> Vec3 {
    let mut s = s.rem_euclid(PERIMETER);
    for w in 0..4 {
        let (start, dir, length) = wall(w);
        if s <= length {
            return start + dir * s;
        }
        s -= length;
    }
    vec3(CORNERS[0].x, 0.0, CORNERS[0].y)
}

/// A printed bead: a part of a layer on one wall, from `from` to `to` meters along the
/// perimeter.
struct Bead {
    layer: usize,
    from: f32,
    to: f32,
}

impl Bead {
    /// The transform of the part of the bead from its start to `end` meters along the
    /// perimeter. A unit bead along X becomes this part.
    fn place(&self, end: f32, thickness: f32) -> Mat4 {
        let end = end.min(self.to);
        let (a, b) = (along(self.from), along(end));
        let dir = (b - a).normalize_or(Vec3::X);
        // the beads overlap at the corners, so the corners are solid
        let (a, b) = (a - dir * WALL * 0.5, b + dir * WALL * 0.5);
        Mat4::from_scale_rotation_translation(
            vec3(a.distance(b), LAYER * 1.12, thickness),
            Quat::from_rotation_y((-dir.z).atan2(dir.x)),
            a.lerp(b, 0.5) + Vec3::Y * (LAYER * (self.layer as f32 + 0.5)),
        )
    }
}

/// The beads of all layers. A layer is solid where no opening crosses its height.
fn beads() -> Vec<Bead> {
    let mut out = Vec::new();
    for layer in 0..LAYERS {
        let y = LAYER * (layer as f32 + 0.5);
        let mut offset = 0.0;
        for w in 0..4 {
            let (_, _, length) = wall(w);
            let mut gaps: Vec<(f32, f32)> = OPENINGS
                .iter()
                .filter(|o| o.0 == w && y > o.3 && y < o.4)
                .map(|o| (o.1 - o.2 * 0.5, o.1 + o.2 * 0.5))
                .collect();
            gaps.sort_by(|a, b| a.0.total_cmp(&b.0));
            let mut from = 0.0;
            for (a, b) in gaps.into_iter().chain([(length, length)]) {
                if a > from + 0.05 {
                    out.push(Bead {
                        layer,
                        from: offset + from,
                        to: offset + a,
                    });
                }
                from = b;
            }
            offset += length;
        }
    }
    out
}

/// The nozzle prints all layers in one run around the perimeter, as a ramp: each layer
/// starts `LAG` meters behind the one below it. A run per layer would move the nozzle half
/// way around the school in one frame, so it would jump from frame to frame. One run moves
/// it about 2 m in a frame, and its light glides along the walls.
const LAG: f32 = 0.9;
/// The length of the ramp behind the lowest layer.
const RAMP: f32 = (LAYERS - 1) as f32 * LAG;
/// The distance that the lowest layer runs: the whole perimeter, and the ramp behind it.
const RUN: f32 = PERIMETER + RAMP;

/// How far the lowest layer has run along the perimeter at `t`, `0..RUN`, or `None` before
/// and after the print.
fn nozzle(t: f32) -> Option<f32> {
    (PRINT.0..PRINT.1)
        .contains(&t)
        .then(|| progress(t, PRINT.0, PRINT.1) * RUN)
}

/// The end of the printed part of `layer` when the lowest layer has run `front` meters.
fn printed_to(layer: usize, front: f32) -> f32 {
    (front - layer as f32 * LAG).clamp(0.0, PERIMETER)
}

/// The place of the gantry over the footprint at `t`, and the layer of the nozzle. The
/// gantry follows the nozzle at the top of the ramp. It waits at the front left corner,
/// where the print starts and ends, and moves back after the print.
fn gantry_at(t: f32) -> (Vec3, f32) {
    let top = (LAYERS - 1) as f32;
    match nozzle(t) {
        Some(front) => (along((front - RAMP).max(0.0)), top),
        None if t < PRINT.0 => (along(0.0), top),
        None => (
            along(0.0).lerp(
                vec3(0.0, 0.0, -GANTRY_Z + 1.0),
                ease(progress(t, PRINT.1, PRINT.1 + 0.08)),
            ),
            LAYERS as f32,
        ),
    }
}

/// The node ids that the frames animate.
struct Ids {
    line: NodeId,
    beads: NodeId,
    hot: NodeId,
    bridge: NodeId,
    carriage: NodeId,
    mast: NodeId,
    leds: NodeId,
    slabs: [NodeId; 4],
    panes: NodeId,
    frames: NodeId,
    inside: [NodeId; 3],
    floods: [NodeId; 2],
    flood_heads: NodeId,
    nozzle_light: NodeId,
    flag: NodeId,
    flag_light: NodeId,
    glows: NodeId,
    drum: NodeId,
    stars: NodeId,
}

/// The site at night, the gantry and the school.
pub struct School {
    scene: Scene,
    ids: Ids,
    beads: Vec<Bead>,
    /// The panes of the windows and the door: their transform and the time they light up.
    panes: Vec<(Mat4, f32)>,
}

impl School {
    /// Builds the part. It takes a fraction of a second.
    pub fn new(assets: &Assets) -> Self {
        let panes = OPENINGS
            .iter()
            .map(|&(w, middle, width, bottom, top)| {
                let (start, dir, _) = wall(w);
                let center = start + dir * middle + Vec3::Y * f32::midpoint(bottom, top);
                let outward = vec3(dir.z, 0.0, -dir.x);
                let place = Mat4::from_scale_rotation_translation(
                    vec3(width, top - bottom, 1.0),
                    Quat::from_rotation_y(outward.x.atan2(outward.z)),
                    center + outward * 0.05,
                );
                // the light runs from the door outward, the far side last
                let from_door = (center.x.abs() / HALF.x) * 0.7 + if w == 0 { 0.0 } else { 0.3 };
                (place, mix(LIGHTS_ON.0, LIGHTS_ON.1 - 0.04, from_door))
            })
            .collect();
        let (scene, ids) = build(assets);
        Self {
            scene,
            ids,
            beads: beads(),
            panes,
        }
    }

    /// The scene and the camera at `t` seconds into the part, `0..LENGTH`.
    pub fn moment(&self, t: f32) -> (Scene, Camera) {
        let ids = &self.ids;
        let mut scene = self.scene.clone();
        let set = |scene: &mut Scene, id: NodeId, instances: Instances| {
            if let Some(mesh) = scene[id].mesh_mut() {
                mesh.instances = Some(Arc::new(instances));
            }
        };
        if let Some(air) = &mut scene.atmosphere {
            air.offset = vec3(t * 3.0, 0.0, t * 1.0);
            // the tree fills the picture: fewer haze samples leave time for the motion blur
            air.steps = if t < 0.14 { 12 } else { 32 };
        }
        let mut glows = Vec::new();

        // the printed beads, and the warm glow of the fresh layers
        let at = nozzle(t);
        let (mut beads, mut hot, mut heat) = (Vec::new(), Vec::new(), Vec::new());
        for bead in &self.beads {
            let end = match at {
                Some(front) => printed_to(bead.layer, front),
                None if t < PRINT.0 => continue,
                None => PERIMETER,
            };
            if end <= bead.from {
                continue;
            }
            beads.push(bead.place(end, WALL));
            let lag = bead.layer as f32 * LAG;
            let printed = mix(PRINT.0, PRINT.1, (bead.to.min(end) + lag) / RUN);
            let warm = (-(t - printed).max(0.0) / 0.07).exp();
            if warm > 0.02 {
                hot.push(bead.place(end, WALL * 1.04));
                heat.push(Rgb::hex(AMBER_EDGE) * (2.4 * warm));
            }
        }
        set(&mut scene, ids.beads, Instances::new(beads));
        set(&mut scene, ids.hot, Instances::new(hot).colors(heat));

        // the line of the footprint: bright before the print, then under the walls
        if let Some(m) = scene[ids.line].material_mut() {
            m.color = Rgb::hex(AMBER_EDGE) * mix(9.0, 2.0, smoothstep(0.05, 0.4, t));
        }

        let (head, layer) = gantry_at(t);
        let tip = head + Vec3::Y * (LAYER * layer + 0.25);
        scene[ids.bridge].position = vec3(head.x, GANTRY, 0.0);
        scene[ids.carriage].position = vec3(head.x, GANTRY - 0.2, head.z);
        set(
            &mut scene,
            ids.mast,
            Instances::new([span(
                tip + Vec3::Y * 0.35,
                vec3(head.x, GANTRY + 0.4, head.z),
                0.22,
            )]),
        );
        if at.is_some() {
            // The nozzle runs several meters in a frame. Its glow is a streak over the time
            // between two motion blur samples, so the samples join into one line of light.
            let dt = 0.5 * SHUTTER * BUILD_RATE / self.shutter(t) as f32;
            let tip_at = |t: f32| {
                let (head, layer) = gantry_at(t);
                head + Vec3::Y * (LAYER * layer + 0.25)
            };
            let (a, b) = (tip_at(t - dt), tip_at(t + dt));
            glows.push(Particle::new(a, 0.18, Rgb::hex(0xfff0d8) * 50.0).tail(b - a));
            glows.push(Particle::new(a, 0.9, Rgb::hex(AMBER) * 3.0).tail(b - a));
        }
        // high above the nozzle, so its pool of light is wide and soft and does not flash on
        // the walls as the nozzle runs past them
        scene[ids.nozzle_light].position = tip + vec3(0.0, 2.6, 1.2);
        if let Some(light) = scene[ids.nozzle_light].light_mut() {
            *light.intensity_mut() = if at.is_some() { 150.0 } else { 0.0 };
        }

        // the roof slabs come down on the walls, 0.11 s each: a faster drop steps 3 m a
        // frame and pops
        for (k, id) in ids.slabs.into_iter().enumerate() {
            let start = mix(ROOF.0, ROOF.1 - 0.11, k as f32 / 3.0);
            let down = ease_out(progress(t, start, start + 0.11));
            scene[id].visible = t > start;
            scene[id].position.y = WALL_HEIGHT + 14.0 * (1.0 - down);
        }

        // the windows: in with the roof, then warm from the door outward
        let installed = t > ROOF.0 + 0.05;
        scene[ids.frames].visible = installed;
        let (places, colors): (Vec<Mat4>, Vec<Rgb>) = self
            .panes
            .iter()
            .filter(|_| installed)
            .map(|&(place, on)| {
                let on = smoothstep(on, on + 0.05, t);
                (place, Rgb::hex(0x0c1016).lerp(Rgb::hex(0xffc47a) * 3.2, on))
            })
            .unzip();
        set(&mut scene, ids.panes, Instances::new(places).colors(colors));
        let inside = smoothstep(LIGHTS_ON.0, LIGHTS_ON.1, t);
        for id in ids.inside {
            if let Some(light) = scene[id].light_mut() {
                *light.intensity_mut() = 900.0 * inside;
            }
        }

        // the cold light of the machines goes out
        let cold = 1.0 - smoothstep(TOWERS_OUT, TOWERS_OUT + 0.06, t);
        for id in ids.floods {
            if let Some(light) = scene[id].light_mut() {
                *light.intensity_mut() = 2200.0 * cold;
            }
        }
        if let Some(m) = scene[ids.flood_heads].material_mut() {
            m.color = Rgb::hex(COLD) * (0.3 + 30.0 * cold);
        }
        if let Some(m) = scene[ids.leds].material_mut() {
            m.color = Rgb::hex(COLD_BLUE) * (0.4 + 5.0 * cold);
        }
        // the drum of the mixer truck turns
        scene[ids.drum].rotation = Quat::from_rotation_z(0.35) * Quat::from_rotation_y(t * 9.0);

        // the flag runs up the pole and waves
        let up = ease(progress(t, FLAG.0, FLAG.1));
        let flag_top = mix(1.6, POLE_HEIGHT - 0.2, up);
        scene[ids.flag].visible = t > FLAG.0 - 0.05;
        if let Some(mesh) = scene[ids.flag].mesh_mut() {
            mesh.geometry = Arc::new(flag(t).translate(POLE.x + 0.08, flag_top, POLE.z));
        }
        if let Some(light) = scene[ids.flag_light].light_mut() {
            *light.intensity_mut() = 500.0 * smoothstep(FLAG.0 - 0.06, FLAG.0, t);
        }
        if let Object::Particles(p) = &mut scene[ids.glows].object {
            p.items.extend(glows);
        }
        if let Object::Particles(p) = &mut scene[ids.stars].object {
            let dt = SHUTTER * BUILD_RATE / self.shutter(t) as f32;
            streak_stars(&mut p.items, camera_at, t, dt);
        }
        (scene, camera_at(t))
    }

    /// The motion blur samples at `t`: more while the tree leaves the picture and when the
    /// camera tilts up.
    pub fn shutter(&self, t: f32) -> usize {
        // The nozzle runs about 3 m in a frame while it prints, and its light and the
        // hot layers move with it. Fewer samples show it as separate copies that flicker.
        if t < 0.14 || (PRINT.0..PRINT.1 + 0.02).contains(&t) {
            5
        } else if (ROOF.0..ROOF.1).contains(&t) {
            // The roof slabs drop 14 m in 0.11 s, 2 m a frame. The slabs are 0.35 m thick:
            // with fewer samples, each slab shows as a stack of separate copies.
            7
        } else if t > 0.85 {
            3
        } else {
            2
        }
    }
}

/// The flag at `t`: 2.4 x 1.6 m, white with a warm stripe, with its top left corner at the
/// origin. Waves run along it in the wind.
fn flag(t: f32) -> Geometry {
    let (columns, rows) = (14u32, 8u32);
    let mut positions = Vec::new();
    let mut colors = Vec::new();
    for j in 0..=rows {
        for i in 0..=columns {
            let (u, v) = (i as f32 / columns as f32, j as f32 / rows as f32);
            let wave = (u * 7.0 - t * 14.0 + v * 0.8).sin() * 0.16 * u;
            positions.push(vec3(u * 2.4, -v * 1.6 - 0.12 * u * u, wave));
            let stripe = (v - 0.5).abs() < 0.09;
            let c = if stripe {
                Rgb::hex(AMBER_EDGE)
            } else {
                Rgb::hex(0xf2efe8)
            };
            colors.push(c.0.extend(1.0));
        }
    }
    let mut indices = Vec::new();
    for j in 0..rows {
        for i in 0..columns {
            let a = j * (columns + 1) + i;
            let b = a + columns + 1;
            indices.extend([a, b, a + 1, a + 1, b, b + 1]);
        }
    }
    let mut g = Geometry::new(positions, Vec::new(), Vec::new(), indices);
    g.colors = colors;
    g
}

/// The place of the camera at `t` and the point that it looks at, before the tilt into the
/// sky. It trucks right past the tree of the yard and slows to a stop, pushes in a little,
/// and follows the flag up.
fn rest(t: f32) -> (Vec3, Vec3) {
    let view = (VIEW - CAMERA_REST).normalize();
    let right = view.cross(Vec3::Y).normalize();
    let u = progress(t, 0.0, TRUCK_TIME);
    let truck = TRUCK * (1.0 - (1.0 - u) * (1.0 - u)) - TRUCK;
    let push = view * (3.0 * ease(progress(t, 0.3, LENGTH)));
    let position = CAMERA_REST + right * truck + push;
    let follow = ease(progress(t, 0.86, 1.06));
    (position, VIEW.lerp(POLE + Vec3::Y * POLE_HEIGHT, follow))
}

/// The direction of the view at the end of the part, `atan2(z, x)` in radians. The water
/// part starts with the same view, so the stars stay in place over the cut.
pub fn heading() -> f32 {
    let (position, look) = rest(LENGTH);
    let f = look - position;
    f.z.atan2(f.x)
}

/// The camera at `t`: the view of [`rest`], and at the end a tilt up past the flag into
/// the stars. The tilt ends at the pitch `SKY_TILT` and turns at `SKY_RATE` there; the
/// water part goes on with the same turn.
fn camera_at(t: f32) -> Camera {
    let (position, look) = rest(t);
    let forward = (look - position).normalize();
    let (end, end_look) = rest(LENGTH);
    let from = (end_look - end).normalize().y.asin();
    let length = 2.0 * (SKY_TILT - from) / SKY_RATE;
    let x = progress(t, LENGTH - length, LENGTH);
    let pitch = mix(forward.y.asin(), SKY_TILT, x * x);
    let target = polar(position, forward, 0.0, pitch, 60.0);
    let follow = ease(progress(t, 0.86, 1.06));
    camera(mix(35.0, 32.0, follow), position, target)
        .focus_on(look, mix(9.0, 4.0, smoothstep(0.0, 0.14, t)))
        .shake(t, 0.0006 * (1.0 - x), 17)
}

/// A column of the gantry: a square lattice of four chords with braces, from the ground to
/// the beams.
fn column() -> Geometry {
    let unit = Geometry::cylinder(1.0, 1.0, 1.0, 5).translate(0.0, 0.5, 0.0);
    let half = 0.32;
    let corners = [
        vec3(-half, 0.0, -half),
        vec3(half, 0.0, -half),
        vec3(half, 0.0, half),
        vec3(-half, 0.0, half),
    ];
    let mut parts = Vec::new();
    for c in corners {
        parts.push(unit.clone().transform(span(c, c + Vec3::Y * GANTRY, 0.06)));
    }
    let levels = 8;
    for level in 0..levels {
        let (y0, y1) = (
            GANTRY * level as f32 / levels as f32,
            GANTRY * (level + 1) as f32 / levels as f32,
        );
        for k in 0..4 {
            let (a, b) = (corners[k], corners[(k + 1) % 4]);
            parts.push(
                unit.clone()
                    .transform(span(a + Vec3::Y * y1, b + Vec3::Y * y1, 0.03)),
            );
            let (low, high) = if level % 2 == 0 { (a, b) } else { (b, a) };
            parts.push(unit.clone().transform(span(
                low + Vec3::Y * y0,
                high + Vec3::Y * y1,
                0.025,
            )));
        }
    }
    // the foot plate
    parts.push(Geometry::cuboid(1.2, 0.08, 1.2).translate(0.0, 0.04, 0.0));
    Geometry::merge(parts)
}

/// A beam of the gantry along X at the top of the columns: two chords with a zigzag web.
fn beam() -> Geometry {
    let unit = Geometry::cylinder(1.0, 1.0, 1.0, 5).translate(0.0, 0.5, 0.0);
    let (from, to) = (GANTRY_X[0] - 0.6, GANTRY_X[2] + 0.6);
    let (low, high) = (GANTRY - 0.7, GANTRY + 0.1);
    let mut parts = Vec::new();
    for y in [low, high] {
        for z in [-0.25, 0.25] {
            parts.push(
                unit.clone()
                    .transform(span(vec3(from, y, z), vec3(to, y, z), 0.07)),
            );
        }
    }
    let steps = 24;
    for i in 0..steps {
        let (x0, x1) = (
            mix(from, to, i as f32 / steps as f32),
            mix(from, to, (i + 1) as f32 / steps as f32),
        );
        let (y0, y1) = if i % 2 == 0 { (low, high) } else { (high, low) };
        for z in [-0.25, 0.25] {
            parts.push(
                unit.clone()
                    .transform(span(vec3(x0, y0, z), vec3(x1, y1, z), 0.035)),
            );
        }
    }
    // the rail of the bridge on top
    parts.push(Geometry::cuboid(to - from, 0.12, 0.2).translate(0.0, high + 0.1, 0.0));
    Geometry::merge(parts)
}

/// The height of the land at `x`, `z`: the flat site, and hills that rise far away.
fn land(x: f32, z: f32) -> f32 {
    let r = vec2(x, z).length();
    let hills = 90.0 + 140.0 * (fbm(51, x / 900.0, z / 900.0, 4) * 0.5 + 0.5);
    smoothstep(500.0, 2200.0, r) * hills + 0.3 * fbm(52, x / 30.0, z / 30.0, 2)
}

/// The scene: the sky, the site, the gantry, the school parts and the lights.
fn build(assets: &Assets) -> (Scene, Ids) {
    // the moon is behind the camera on the left, out of the picture
    let moon = vec3(-0.55, 0.32, 0.77).normalize();
    let mut scene = Scene::new()
        .environment(Environment::map(
            sky(
                Rgb::hex(0x0a1430) * 0.5,
                Rgb::hex(0x2a3a64) * 0.4,
                Rgb::hex(0x8aa0d0) * 0.35,
                moon.z.atan2(moon.x),
                0.0,
            ),
            1.0,
        ))
        .background(Background::Environment { blur: 0.0 });
    let sky_stars = scene.add(Node::particles(stars(CAMERA_REST, 1.0), Blending::Additive));

    // the land: dirt of the site, a gravel pad, the path and the street, hills around
    let mut ground = Geometry::floor_segments(6000.0, 6000.0, 300, 300);
    for p in &mut ground.positions {
        p.y = land(p.x, p.z);
    }
    ground.compute_vertex_normals();
    ground.update_bounds();
    scene.add(Node::mesh(ground, assets.dirt.material(0x8a7a66, 1.0, 0.0)));
    scene.add(
        Node::mesh(
            Geometry::floor(48.0, 26.0),
            assets.concrete.material(0x9a968e, 1.0, 0.0),
        )
        .at(0.0, 0.02, 0.0),
    );
    let asphalt = assets.asphalt.material(0x5a5a5a, 0.9, 0.0);
    scene.add(Node::mesh(Geometry::floor(400.0, 9.0), asphalt.clone()).at(0.0, 0.03, 24.0));
    scene.add(Node::mesh(Geometry::floor(2.6, 13.0), asphalt).at(0.0, 0.03, 13.0));
    // the warm line of the footprint under the walls
    let unit = Arc::new(Geometry::cylinder(1.0, 1.0, 1.0, 6).translate(0.0, 0.5, 0.0));
    let footprint: Vec<Mat4> = (0..4)
        .map(|w| {
            let (start, dir, length) = wall(w);
            span(
                start - dir * 0.1 + Vec3::Y * 0.06,
                start + dir * (length + 0.1) + Vec3::Y * 0.06,
                0.06,
            )
        })
        .collect();
    let line = scene.add(
        Node::instanced(
            unit.clone(),
            Material::unlit(Rgb::hex(AMBER_EDGE) * 9.0),
            Instances::new(footprint),
        )
        .shadows(false, false),
    );

    // stones, tufts of dry grass, and the trees of the yard and the street
    let mut stones = Vec::new();
    for i in 0..5000u32 {
        let r = 12.0 + 200.0 * hash(i * 3 + 1).powi(2);
        let a = hash(i * 3 + 2) * TAU;
        let (x, z) = (r * a.sin(), r * a.cos());
        if x.abs() < 25.0 && z.abs() < 14.0 {
            continue;
        }
        // pebbles: big stones read as lumps on the gravel
        let size = 0.05 + 0.16 * hash(i * 3 + 3).powi(3);
        stones.push(Mat4::from_scale_rotation_translation(
            vec3(size * 1.4, size * 0.6, size),
            Quat::from_rotation_y(hash(i) * TAU),
            vec3(x, land(x, z) + size * 0.1, z),
        ));
    }
    scene.add(Node::instanced(
        Geometry::icosahedron(1.0, 1).flat(),
        assets.rock.material(0xb0a494, 1.0, 0.0),
        Instances::new(stones),
    ));
    let leaves = Material::standard(0x0e160a)
        .roughness(1.0)
        .env_intensity(0.3);
    let bark = assets.rock.material(0x3a3028, 1.0, 0.0);
    let view = (VIEW - CAMERA_REST).normalize();
    let right = view.cross(Vec3::Y).normalize();
    let start = CAMERA_REST - right * TRUCK;
    let yard_tree = start + (VIEW - start).normalize() * 9.0;
    let mut crowns = Vec::new();
    let mut trunks = Vec::new();
    for (k, (center, size)) in [
        (yard_tree, vec3(7.0, 4.2, 3.0)),
        (vec3(-30.0, 5.5, 18.0), vec3(3.0, 3.6, 3.0)),
        (vec3(28.0, 5.0, 19.0), vec3(2.6, 3.0, 2.6)),
        (vec3(-46.0, 5.0, 30.0), vec3(3.0, 3.2, 3.0)),
    ]
    .into_iter()
    .enumerate()
    {
        trunks.push(span(
            vec3(center.x, 0.0, center.z),
            center - Vec3::Y * 1.0,
            0.25,
        ));
        for i in 0..380u32 {
            let seed = k as u32 * 1000 + i * 4;
            crowns.push(Mat4::from_scale_rotation_translation(
                Vec3::splat(0.3 + 0.45 * hash(seed + 3)),
                Quat::from_rotation_y(hash(seed) * TAU)
                    * Quat::from_rotation_x(hash(seed + 7) * PI),
                center + random_in_ball(seed + 11) * size,
            ));
        }
    }
    scene.add(Node::instanced(unit.clone(), bark, Instances::new(trunks)));
    scene.add(Node::instanced(
        Geometry::icosahedron(1.0, 1).flat(),
        leaves,
        Instances::new(crowns),
    ));

    // the street lamps and the fence of the yard
    let steel = assets.steel.material(0x8a8e94, 0.6, 0.6);
    let mut fence = Vec::new();
    for i in 0..40 {
        let x = -60.0 + i as f32 * 3.0;
        fence.push(span(vec3(x, 0.0, 19.5), vec3(x, 1.4, 19.5), 0.04));
    }
    fence.push(span(vec3(-60.0, 1.3, 19.5), vec3(57.0, 1.3, 19.5), 0.03));
    fence.push(span(vec3(-60.0, 0.4, 19.5), vec3(57.0, 0.4, 19.5), 0.03));
    scene.add(Node::instanced(
        unit.clone(),
        steel.clone(),
        Instances::new(fence),
    ));
    let mut poles = Vec::new();
    let mut glows = Vec::new();
    // the lamps stand left and right of the view, not in front of the school
    for x in [-96.0, -72.0, -46.0, 18.0, 44.0, 70.0] {
        poles.push(span(vec3(x, 0.0, 29.5), vec3(x, 7.5, 29.5), 0.09));
        poles.push(span(vec3(x, 7.5, 29.5), vec3(x, 7.6, 27.8), 0.06));
        glows.push(Particle::new(
            vec3(x, 7.4, 27.8),
            0.35,
            Rgb::hex(0xffb070) * 22.0,
        ));
        glows.push(Particle::new(
            vec3(x, 7.4, 27.8),
            2.0,
            Rgb::hex(0xff9a50) * 0.25,
        ));
    }
    scene.add(Node::instanced(
        unit.clone(),
        steel.clone(),
        Instances::new(poles),
    ));
    // the village and the wind farm on the hills: warm windows and blinking red lights
    for i in 0..500u32 {
        let a = -2.35 + 1.1 * hash(i * 5 + 1);
        let r = 1300.0 + 500.0 * hash(i * 5 + 2);
        let (x, z) = (r * a.cos(), r * a.sin());
        if fbm(53, x / 300.0, z / 300.0, 2) < 0.05 {
            continue;
        }
        glows.push(Particle::new(
            vec3(x, land(x, z) + 3.0, z),
            1.6,
            Rgb::hex(0xffb46a) * (4.0 + 8.0 * hash(i * 5 + 3)),
        ));
    }
    let glows = scene.add(Node::particles(glows, Blending::Additive));

    // the gantry: six lattice columns, two beams, the moving bridge, carriage and mast
    let gantry_paint = Material::standard(0x3c4046)
        .roughness(0.55)
        .metalness(0.4)
        .normal_map(assets.painted_metal.normal.clone());
    let columns: Vec<Mat4> = GANTRY_X
        .iter()
        .flat_map(|&x| [-GANTRY_Z, GANTRY_Z].map(|z| Mat4::from_translation(vec3(x, 0.0, z))))
        .collect();
    scene.add(Node::instanced(
        column(),
        gantry_paint.clone(),
        Instances::new(columns),
    ));
    let beam = beam();
    for z in [-GANTRY_Z, GANTRY_Z] {
        scene.add(Node::mesh(beam.clone(), gantry_paint.clone()).at(0.0, 0.0, z));
    }
    let bridge = scene.add(Node::mesh(
        Geometry::merge([
            Geometry::cuboid(0.7, 0.8, 2.0 * GANTRY_Z + 1.2),
            Geometry::cuboid(1.4, 0.6, 1.0).translate(0.0, 0.1, GANTRY_Z),
            Geometry::cuboid(1.4, 0.6, 1.0).translate(0.0, 0.1, -GANTRY_Z),
        ]),
        Material::standard(0xd89a28).roughness(0.5).metalness(0.3),
    ));
    let carriage = scene.add(Node::mesh(
        Geometry::merge([
            Geometry::cuboid(1.3, 1.1, 1.2),
            Geometry::cylinder(0.35, 0.35, 0.9, 12).translate(0.0, 0.9, 0.0),
        ]),
        gantry_paint.clone(),
    ));
    let mast = scene.add(Node::instanced(
        unit.clone(),
        steel.clone(),
        Instances::default(),
    ));
    // a strip of cold LEDs under each beam
    let leds = scene.add(
        Node::mesh(
            Geometry::merge([-GANTRY_Z, GANTRY_Z].map(|z| {
                Geometry::cuboid(2.0 * GANTRY_X[2], 0.06, 0.1).translate(0.0, GANTRY - 0.75, z)
            })),
            Material::unlit(Rgb::hex(COLD_BLUE) * 5.0),
        )
        .shadows(false, false),
    );
    let nozzle_light = scene.add(Node::light(Light::point(AMBER, 0.0, 22.0)));

    // the mixer truck and the pump beside the gantry, with the hose to the carriage
    let truck_at = vec3(-26.0, 0.0, -4.0);
    let truck_paint = Material::standard(0xe8e4dc).roughness(0.45).clearcoat(0.5);
    scene.add(
        Node::mesh(
            Geometry::merge([
                Geometry::cuboid(2.5, 1.1, 9.0)
                    .translate(0.0, 1.1, 0.0)
                    .with_color(0x2a2c30),
                Geometry::rounded_box(2.5, 2.4, 2.2, 0.2, 2)
                    .translate(0.0, 2.3, 4.0)
                    .with_color(0xe8e4dc),
                Geometry::cuboid(2.3, 0.7, 0.05)
                    .translate(0.0, 2.8, 5.12)
                    .with_color(0x101418),
            ]),
            truck_paint.clone(),
        )
        .with_position(truck_at),
    );
    let mut wheels = Vec::new();
    for z in [3.6, -1.2, -2.8] {
        for x in [-1.25, 1.25] {
            wheels.push(Mat4::from_rotation_translation(
                Quat::from_rotation_z(FRAC_PI_2),
                truck_at + vec3(x, 0.55, z),
            ));
        }
    }
    scene.add(Node::instanced(
        Geometry::cylinder(0.55, 0.55, 0.4, 16),
        Material::standard(0x151515).roughness(0.9),
        Instances::new(wheels),
    ));
    let drum = scene.add(
        Node::mesh(
            Geometry::lathe(
                &[
                    vec2(0.0, -2.6),
                    vec2(0.9, -2.4),
                    vec2(1.35, -0.6),
                    vec2(1.25, 1.4),
                    vec2(0.5, 2.6),
                    vec2(0.0, 2.7),
                ],
                24,
            )
            .rotate_x(FRAC_PI_2),
            Material::standard(0xd8622a)
                .roughness(0.5)
                .clearcoat(0.4)
                .map(assets.corrugated.color.clone()),
        )
        .with_position(truck_at + vec3(0.0, 3.0, -1.6)),
    );
    let hose: Vec<Vec3> = (0..=24)
        .map(|i| {
            let u = i as f32 / 24.0;
            let a = truck_at + vec3(1.3, 1.6, -4.0);
            let b = vec3(GANTRY_X[0], GANTRY + 0.3, -GANTRY_Z);
            a.lerp(b, u) + Vec3::Y * (3.0 * (u * PI).sin()) - Vec3::Y * (2.5 * u * (1.0 - u))
        })
        .collect();
    scene.add(Node::mesh(
        Geometry::tube(&hose, 0.09, 8),
        Material::standard(0x1a1a1a).roughness(0.6),
    ));

    // the light towers: masts with four cold floods each, aimed at the gantry
    let towers = [vec3(-30.0, 0.0, 12.0), vec3(26.0, 0.0, -16.0)];
    let mut masts = Vec::new();
    let mut heads = Vec::new();
    let mut floods = [NodeId(0); 2];
    for (k, &foot) in towers.iter().enumerate() {
        let top = foot + Vec3::Y * 9.0;
        masts.push(span(foot, top, 0.12));
        let aim = (vec3(0.0, 1.5, 0.0) - top).normalize();
        let side = aim.cross(Vec3::Y).normalize();
        for (i, j) in [(-1.0, 0.0), (1.0, 0.0), (-1.0, 1.0), (1.0, 1.0)] {
            heads.push(Mat4::from_rotation_translation(
                Quat::from_rotation_arc(Vec3::Z, aim),
                top + side * (0.5 * i) + Vec3::Y * (0.5 * j),
            ));
        }
        floods[k] = scene.add(
            Node::light(Light::spot(COLD, 0.0, 0.62, 0.5).pointing(aim))
                .with_position(top + aim * 0.5),
        );
    }
    scene.add(Node::instanced(
        unit.clone(),
        steel.clone(),
        Instances::new(masts),
    ));
    let flood_heads = scene.add(
        Node::instanced(
            Geometry::cuboid(0.45, 0.35, 0.12),
            Material::unlit(Rgb::hex(COLD) * 30.0),
            Instances::new(heads),
        )
        .shadows(false, false),
    );

    // the school parts: printed beads, the glow of the fresh beads, roof slabs, windows
    let bead = Arc::new(Geometry::cylinder(0.5, 0.5, 1.0, 10).rotate_z(FRAC_PI_2));
    let beads = scene.add(Node::instanced(
        bead.clone(),
        assets.concrete.material(0xd0c8bc, 0.95, 0.0),
        Instances::default(),
    ));
    let hot = scene.add(
        Node::instanced(
            bead,
            Material::unlit(Rgb::WHITE).additive(),
            Instances::default(),
        )
        .shadows(false, false),
    );
    let slab_paint = assets.concrete.material(0xa8a298, 0.9, 0.0);
    let slabs = [-12.0f32, -4.0, 4.0, 12.0].map(|x| {
        // each slab comes with its roof plant: a row of solar panels on stands and a
        // vent unit, so the roof is not a bare lid
        let mut parts = vec![
            Geometry::cuboid(8.05, 0.35, 2.0 * HALF.y + 1.2)
                .translate(0.0, 0.175, 0.0)
                .with_color(0xffffff),
            // the fascia that hides the edge of the roof
            Geometry::cuboid(8.05, 0.5, 0.12)
                .translate(0.0, 0.25, HALF.y + 0.6)
                .with_color(0xffffff),
            Geometry::cuboid(1.3, 0.7, 1.0)
                .translate(2.6, 0.7, HALF.y * 0.45)
                .with_color(0x909090),
        ];
        for px in [-2.7f32, -0.6, 1.5] {
            parts.push(
                Geometry::cuboid(2.0, 0.05, 1.3)
                    .rotate_x(-0.45)
                    .translate(px, 0.75, -HALF.y * 0.35)
                    .with_color(0x202a3c),
            );
            parts.push(
                Geometry::cuboid(1.8, 0.5, 0.06)
                    .translate(px, 0.6, -HALF.y * 0.35 - 0.55)
                    .with_color(0x606060),
            );
        }
        scene.add(
            Node::mesh(Geometry::merge(parts), slab_paint.clone())
                .with_position(vec3(x, WALL_HEIGHT, 0.0))
                .hidden(),
        )
    });
    let panes = scene.add(
        Node::instanced(
            Geometry::plane(1.0, 1.0),
            Material::unlit(Rgb::WHITE),
            Instances::default(),
        )
        .shadows(false, false),
    );
    let mut frame_parts = Vec::new();
    for &(w, middle, width, bottom, top) in &OPENINGS {
        let (start, dir, _) = wall(w);
        let center = start + dir * middle;
        let outward = vec3(dir.z, 0.0, -dir.x) * 0.12;
        let half = dir * (width * 0.5);
        let rail = |a: Vec3, b: Vec3| (*unit).clone().transform(span(a, b, 0.05));
        for y in [bottom, top] {
            frame_parts.push(rail(
                center - half + Vec3::Y * y + outward,
                center + half + Vec3::Y * y + outward,
            ));
        }
        for s in [-1.0, 0.0, 1.0] {
            frame_parts.push(rail(
                center + half * s + Vec3::Y * bottom + outward,
                center + half * s + Vec3::Y * top + outward,
            ));
        }
    }
    let frames = scene.add(
        Node::mesh(
            Geometry::merge(frame_parts),
            Material::standard(0x2a2c30).roughness(0.4).metalness(0.6),
        )
        .shadows(false, false)
        .hidden(),
    );
    let inside = [
        vec3(-9.0, 2.6, 1.0),
        vec3(0.0, 2.6, 1.5),
        vec3(9.0, 2.6, 1.0),
    ]
    .map(|p| scene.add(Node::light(Light::point(0xffc890, 0.0, 26.0)).with_position(p)));

    // the flag pole, the flag, and the warm light at the foot of the pole
    scene.add(
        Node::mesh(
            Geometry::merge([
                Geometry::cylinder(0.05, 0.07, POLE_HEIGHT, 10).translate(
                    0.0,
                    POLE_HEIGHT * 0.5,
                    0.0,
                ),
                Geometry::sphere(0.1, 10, 6).translate(0.0, POLE_HEIGHT + 0.05, 0.0),
            ]),
            Material::standard(0xd8dadc).roughness(0.3).metalness(0.8),
        )
        .with_position(POLE),
    );
    let flag = scene.add(
        Node::mesh(
            flag(0.0),
            Material::standard(0xffffff).roughness(0.8).double_sided(),
        )
        .hidden(),
    );
    let flag_light = scene.add(
        Node::light(Light::spot(0xffd2a0, 0.0, 0.3, 0.6).pointing(vec3(0.05, 1.0, -0.12)))
            .with_position(POLE + vec3(0.6, 0.3, 1.4)),
    );

    // the moonlight from behind the camera, and the faint light of the sky
    scene.add(Node::light(Light::sun(0xb4c4f0, 0.8, moon).with_shadow(
        Shadow {
            map_size: 2048,
            softness: 2.0,
            strength: 0.85,
            bias: 1.5,
            bounds: Some(Aabb {
                min: vec3(-60.0, -1.0, -40.0),
                max: vec3(60.0, 12.0, 40.0),
            }),
        },
    )));
    scene.add(Node::light(Light::hemisphere(0x2a3a64, 0x0c0a08, 0.7)));
    scene.atmosphere = Some(Atmosphere {
        density: 0.0035,
        color: Rgb::WHITE,
        base: 0.0,
        falloff: 0.09,
        anisotropy: 0.6,
        ambient: Rgb::hex(0x2a3a60) * 0.03,
        noise: 0.55,
        noise_size: 14.0,
        offset: Vec3::ZERO,
        distance: 300.0,
        steps: 32,
    });

    let ids = Ids {
        line,
        beads,
        hot,
        bridge,
        carriage,
        mast,
        leds,
        slabs,
        panes,
        frames,
        inside,
        floods,
        flood_heads,
        nozzle_light,
        flag,
        flag_light,
        glows,
        drum,
        stars: sky_stars,
    };
    (scene, ids)
}
