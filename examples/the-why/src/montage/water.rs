//! A quick build before the dawn: a canal brings water to a dry field, and the field turns
//! green in rows. The sky glows low ahead, so the water shines where it runs. The times
//! are seconds into the part.
//!
//! - 0.0 to 0.5: the camera tilts down out of the stars of the school. The warm line runs
//!   across a dry plain, in front of a field of bare furrows.
//! - 0.22 to 0.8: a canal paver runs along the line with its cold lights. Behind it, the
//!   concrete channel of the canal stands on the line.
//! - 0.45 to 1.25: the water runs down the canal behind a warm head of light. Behind the
//!   head, the water turns into the furrows, which shine like lines of light, and the rows
//!   of the crop sprout green. The camera pans after the head.
//! - 1.25: the head is in the middle of the picture, moving right: the match cut to the pen
//!   of the greenhouse.

use std::f32::consts::TAU;
use std::sync::Arc;

use fframes_3d::math::{hash, progress, smoothstep};
use fframes_3d::*;

use super::{
    AMBER, AMBER_EDGE, BUILD_RATE, COLD, SKY_RATE, SKY_TILT, camera, ease, ease_out, fbm, mix,
    polar, ridges, sky, span, stars, streak_stars, yaw_pitch,
};
use crate::assets::Assets;
use crate::look::SHUTTER;

/// The length of the part in seconds.
pub const LENGTH: f32 = 1.25;
/// The field: furrows from the canal to `FIELD_END` (-Z), between ±`FIELD_HALF` along X.
const FIELD_HALF: f32 = 330.0;
const FIELD_END: f32 = -340.0;
/// The distance between two furrows.
const ROW: f32 = 2.4;
/// The far furrows blend into a flat field: from `FADE.0` to `FADE.1` meters from the
/// canal the water of a furrow widens to the whole row and the ridges between the furrows
/// flatten under it. So no thin lines are left far away to make moiré when the camera
/// pans.
const FADE: (f32, f32) = (25.0, 200.0);
/// The width of the water of a furrow near the canal.
const FURROW: f32 = 0.75;
/// The pieces of a furrow along its length: a furrow fills piece by piece, so the shape
/// of each piece stays true while the water runs.
const PIECES: usize = 8;
/// The half width of the canal between its walls, and the height of the walls.
const CANAL: f32 = 1.6;
const CANAL_WALL: f32 = 1.2;
/// The paver runs along the line from `PAVER.0` to `PAVER.1`.
const PAVER: (f32, f32) = (0.22, 0.8);
/// The water starts to run at `WATER`.
const WATER: f32 = 0.45;
/// The X of the paver at its start and at its end. The canal stands from `CANAL_START`
/// to the paver.
const PAVER_X: (f32, f32) = (-230.0, 200.0);
const CANAL_START: f32 = -900.0;
/// The X of the head of the water at its start and at the end of the part.
const HEAD_X: (f32, f32) = (-330.0, 40.0);
/// The camera: a drone 45 m over the plain, 150 m from the canal.
const CAMERA: Vec3 = Vec3::new(-50.0, 45.0, 150.0);
/// The pitch of the view of the field after the tilt, in radians.
const FIELD_PITCH: f32 = -0.13;

/// The head of the water slows down until `HEAD_END`, after the end of the part, so it
/// still moves at the cut.
const HEAD_END: f32 = LENGTH + 0.3;

/// The distance that the head would run until `HEAD_END`.
fn head_run() -> f32 {
    (HEAD_X.1 - HEAD_X.0) / ease_out((LENGTH - WATER) / (HEAD_END - WATER))
}

/// The X of the head of the water at `t`. It runs fast and slows a little at the end.
fn head_x(t: f32) -> f32 {
    HEAD_X.0 + head_run() * ease_out(progress(t, WATER, HEAD_END))
}

/// The time at which the head of the water passes `x`, or a time after the part.
fn water_time(x: f32) -> f32 {
    let p = ((x - HEAD_X.0) / head_run()).clamp(0.0, 1.0);
    // the inverse of the cubic ease out
    WATER + (1.0 - (1.0 - p).cbrt()) * (HEAD_END - WATER)
}

/// The X of the paver at `t`: it runs at an even speed through the picture.
fn paver_x(t: f32) -> f32 {
    mix(PAVER_X.0, PAVER_X.1, progress(t, PAVER.0, PAVER.1))
}

/// The node ids that the frames animate.
struct Ids {
    walls: NodeId,
    rims: NodeId,
    canal_water: NodeId,
    glow: NodeId,
    furrows: [NodeId; PIECES],
    shine: [NodeId; PIECES],
    crops: NodeId,
    paver: NodeId,
    paver_light: NodeId,
    head_light: NodeId,
    glows: NodeId,
    line: NodeId,
    stars: NodeId,
}

/// The plain, the field and the canal before the dawn.
pub struct Water {
    scene: Scene,
    ids: Ids,
    /// The turn of the world: the field lies in its own space, turned so that the first
    /// view looks where the school part ends.
    turn: Quat,
    /// The X of each furrow.
    furrows: Vec<f32>,
}

impl Water {
    /// Builds the part. `heading` is the direction of the first view, `atan2(z, x)` in
    /// radians.
    pub fn new(assets: &Assets, heading: f32) -> Self {
        // the turn maps -Z of the field to the heading
        let turn = Quat::from_rotation_y((-heading.cos()).atan2(-heading.sin()));
        let (scene, ids) = build(assets, turn);
        let count = (2.0 * FIELD_HALF / ROW) as usize;
        let furrows = (0..count)
            .map(|i| -FIELD_HALF + (i as f32 + 0.5) * ROW)
            .collect();
        Self {
            scene,
            ids,
            turn,
            furrows,
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
            air.offset = vec3(t * 6.0, 0.0, t * 2.0);
        }
        // the sky starts as dark as the night sky of the school and grows to the light
        // before the dawn while the camera comes down
        scene.environment =
            scene
                .environment
                .clone()
                .intensity(mix(0.42, 1.0, smoothstep(0.0, 0.5, t)));
        let mut glows = Vec::new();

        // the paver and the channel behind it
        let paver = paver_x(t);
        scene[ids.paver].position = vec3(paver, 0.0, 0.0);
        scene[ids.paver].visible = t > PAVER.0 - 0.05 && t < PAVER.1 + 0.05;
        let working =
            smoothstep(PAVER.0 - 0.05, PAVER.0, t) * (1.0 - smoothstep(PAVER.1 - 0.05, PAVER.1, t));
        if let Some(light) = scene[ids.paver_light].light_mut() {
            *light.intensity_mut() = 6000.0 * working;
        }
        scene[ids.paver_light].position = vec3(paver, 9.0, 4.0);
        if working > 0.01 {
            for (x, z) in [(-3.0, 4.6), (3.0, 4.6), (-3.0, -4.6), (3.0, -4.6)] {
                let p = vec3(paver + x, 5.2, z);
                glows.push(Particle::new(p, 0.35, Rgb::hex(COLD) * (25.0 * working)));
                glows.push(Particle::new(p, 3.0, Rgb::hex(COLD) * (0.6 * working)));
            }
        }
        let built = paver - CANAL_START - 4.0;
        let walls = if built > 0.1 {
            vec![Mat4::from_scale_rotation_translation(
                vec3(built, 1.0, 1.0),
                Quat::IDENTITY,
                vec3(CANAL_START + built * 0.5, 0.0, 0.0),
            )]
        } else {
            Vec::new()
        };
        set(&mut scene, ids.walls, Instances::new(walls.clone()));
        set(&mut scene, ids.rims, Instances::new(walls));
        // the line glows ahead of the paver, and under the water
        if let Some(m) = scene[ids.line].material_mut() {
            m.color = Rgb::hex(AMBER_EDGE) * mix(8.0, 3.0, smoothstep(0.3, 0.8, t));
        }
        // the walls hide the line in the canal, so the line runs on along their tops
        if let Some(m) = scene[ids.rims].material_mut() {
            m.color = Rgb::hex(AMBER_EDGE) * mix(5.0, 2.0, smoothstep(0.5, 1.1, t));
        }

        // the water in the canal, and the warm head of light
        let head = head_x(t);
        let running = t > WATER;
        let (mut water, mut glow, mut heat) = (Vec::new(), Vec::new(), Vec::new());
        if running {
            let from = CANAL_START;
            let steps = 40;
            for k in 0..steps {
                let (a, b) = (
                    mix(from, head, k as f32 / steps as f32),
                    mix(from, head, (k + 1) as f32 / steps as f32),
                );
                let place = Mat4::from_scale_rotation_translation(
                    vec3(b - a, 1.0, 1.0),
                    Quat::IDENTITY,
                    vec3(f32::midpoint(a, b), 0.0, 0.0),
                );
                water.push(place);
                let behind = (head - b).max(0.0);
                let warm = (-behind / 60.0).exp();
                if warm > 0.02 {
                    glow.push(place);
                    heat.push(Rgb::hex(AMBER) * (1.6 * warm));
                }
            }
            let p = vec3(head, 0.75, 0.0);
            glows.push(Particle::new(p, 0.9, Rgb::hex(0xfff0d8) * 60.0));
            glows.push(Particle::new(p, 7.0, Rgb::hex(AMBER) * 3.0));
            glows.push(Particle::new(p, 22.0, Rgb::hex(AMBER) * 0.35));
            // spray thrown up by the head
            for i in 0..24u32 {
                let a = hash(i * 3 + 1) * TAU;
                let r = 1.0 + 4.0 * hash(i * 3 + 2);
                let s = vec3(
                    head - r * 0.8 * a.cos().abs(),
                    0.9 + 1.4 * hash(i * 3 + 3) * (1.0 - r / 6.0),
                    r * a.sin() * 0.3,
                );
                glows.push(Particle::new(s, 0.12, Rgb::hex(0xffd8a8) * 6.0));
            }
        }
        set(&mut scene, ids.canal_water, Instances::new(water));
        set(&mut scene, ids.glow, Instances::new(glow).colors(heat));
        scene[ids.head_light].position = vec3(head, 2.0, 3.0);
        scene[ids.head_light].visible = running;

        // the furrows fill behind the head, and the rows of the crop sprout
        let (mut crops, mut colors) = (Vec::new(), Vec::new());
        let mut furrows = vec![Vec::new(); PIECES];
        let mut shine = vec![(Vec::new(), Vec::new()); PIECES];
        let piece = FURROW_LENGTH / PIECES as f32;
        for (i, &x) in self.furrows.iter().enumerate() {
            let at = water_time(x) + 0.02 * hash(i as u32 * 7 + 1);
            let fill = ease_out(progress(t, at, at + 0.28));
            if fill > 0.0 {
                let length = FURROW_LENGTH * fill;
                // the water carries the warm light into the furrow, brightest at its front
                let warm = (-(t - at).max(0.0) / 0.16).exp();
                for k in 0..PIECES {
                    let start = k as f32 * piece;
                    let part = ((length - start) / piece).min(1.0);
                    if part <= 0.0 {
                        break;
                    }
                    let place = Mat4::from_scale_rotation_translation(
                        vec3(1.0, 1.0, part),
                        Quat::IDENTITY,
                        vec3(x, 0.0, -CANAL - 0.6 - start),
                    );
                    furrows[k].push(place);
                    if warm > 0.03 {
                        shine[k].0.push(place);
                        shine[k].1.push(Rgb::hex(AMBER_EDGE) * (2.2 * warm));
                    }
                }
                if warm > 0.03 && fill < 1.0 {
                    // The front runs tens of meters in a frame. A streak over the time
                    // between two motion blur samples keeps it from sparkling.
                    let front = vec3(x, 0.15, -CANAL - 0.6 - length);
                    let u = progress(t, at, at + 0.28);
                    let speed = FURROW_LENGTH * 3.0 * (1.0 - u).powi(2) / 0.28;
                    let dt = SHUTTER * BUILD_RATE / self.shutter(t) as f32;
                    glows.push(
                        Particle::new(front, 0.5, Rgb::hex(0xffd8a0) * (20.0 * warm))
                            .tail(Vec3::Z * (speed * dt)),
                    );
                }
            }
            let grow = ease_out(progress(t, at + 0.12, at + 0.45));
            if grow > 0.0 {
                // the row beside the furrow, grown as far as the water ran. Each row is
                // shifted along its length by a random part of the step of its tufts, so
                // the tufts of the rows do not line up into a grid that makes moiré.
                let shift = 0.5 + 2.4 * hash(i as u32 * 17 + 3);
                crops.push(Mat4::from_scale_rotation_translation(
                    vec3(1.0, grow, fill.max(0.02)),
                    Quat::IDENTITY,
                    vec3(x + ROW * 0.5, 0.0, -CANAL - 0.6 - shift),
                ));
                let tint = 0.85 + 0.3 * hash(i as u32 * 13 + 5);
                colors.push(Rgb::hex(0x62b034) * tint);
            }
        }
        for (k, (furrows, (shine, warmth))) in furrows.into_iter().zip(shine).enumerate() {
            set(&mut scene, ids.furrows[k], Instances::new(furrows));
            set(
                &mut scene,
                ids.shine[k],
                Instances::new(shine).colors(warmth),
            );
        }
        set(&mut scene, ids.crops, Instances::new(crops).colors(colors));

        if let Object::Particles(p) = &mut scene[ids.glows].object {
            p.items = glows;
        }
        if let Object::Particles(p) = &mut scene[ids.stars].object {
            let dt = SHUTTER * BUILD_RATE / self.shutter(t) as f32;
            streak_stars(&mut p.items, |t| self.camera_at(t), t, dt);
        }
        (scene, self.camera_at(t))
    }

    /// The motion blur samples at `t`: more on the fast tilt at the start, and on the pan
    /// after the head, where the thin lines of the far furrows would crawl.
    pub fn shutter(&self, t: f32) -> usize {
        // the thin rows of water and crop crawl with fewer samples while the camera pans
        if t > 0.5 {
            4
        } else if t < 0.2 {
            3
        } else {
            2
        }
    }

    /// The camera at `t`, in the world. It comes down out of the stars, still turning at
    /// the rate of the school tilt, settles on the field, and pans after the head of the
    /// water until the head is in the middle of the picture.
    fn camera_at(&self, t: f32) -> Camera {
        let position = CAMERA + vec3(10.0 * ease(progress(t, 0.0, LENGTH)), 0.0, 0.0);
        let down = 2.0 * (SKY_TILT - FIELD_PITCH) / SKY_RATE;
        let x = progress(t, 0.0, down);
        let mut pitch = mix(SKY_TILT, FIELD_PITCH, 1.0 - (1.0 - x) * (1.0 - x));
        let head = vec3(head_x(t), 0.8, 0.0);
        let (head_yaw, head_pitch) = yaw_pitch(Vec3::NEG_Z, head - position);
        let follow = ease(progress(t, 0.55, 1.15));
        let yaw = mix(0.0, head_yaw, follow);
        pitch = mix(pitch, head_pitch, follow);
        let target = polar(position, Vec3::NEG_Z, yaw, pitch, 200.0);
        let focus = vec3(head_x(t).max(-120.0), 0.0, 0.0);
        let turn = |p: Vec3| self.turn * p;
        camera(32.0, turn(position), turn(target))
            .focus_on(turn(focus), 3.0)
            .shake(t, 0.0005, 19)
    }
}

/// The height of the plain at `x`, `z`: flat near the field, low hills far away.
fn land(x: f32, z: f32) -> f32 {
    let r = vec2(x, z + 300.0).length();
    smoothstep(700.0, 2000.0, r) * 80.0 * (fbm(61, x / 700.0, z / 700.0, 3) * 0.5 + 0.6)
        + 0.3 * smoothstep(420.0, 600.0, r) * fbm(62, x / 40.0, z / 40.0, 3)
}

/// The length of a furrow from the canal to the end of the field.
const FURROW_LENGTH: f32 = -FIELD_END - CANAL - 0.6;

/// How far the furrows are blended into a flat field `d` meters from the canal, `0..1`,
/// see [`FADE`].
fn flat(d: f32) -> f32 {
    smoothstep(FADE.0, FADE.1, d)
}

/// The piece `k` of the water of a furrow, see [`PIECES`]: a strip that faces up at
/// `height`, from the start of the piece at Z = 0 along -Z. Its width is `width` times the
/// width of the water, which grows from [`FURROW`] to the whole row with the distance.
fn furrow_piece(k: usize, width: f32, height: f32) -> Geometry {
    let piece = FURROW_LENGTH / PIECES as f32;
    let steps = 6;
    let mut positions = Vec::new();
    let mut uvs = Vec::new();
    for j in 0..=steps {
        let z = piece * j as f32 / steps as f32;
        let half = 0.5 * width * mix(FURROW, ROW, flat(k as f32 * piece + z));
        for x in [-half, half] {
            positions.push(vec3(x, height, -z));
            uvs.push(vec2(x, -z));
        }
    }
    let indices = (0..steps as u32)
        .flat_map(|j| {
            let (l0, r0, l1, r1) = (2 * j, 2 * j + 1, 2 * j + 2, 2 * j + 3);
            [l0, r0, l1, r0, r1, l1]
        })
        .collect();
    let normals = vec![Vec3::Y; positions.len()];
    Geometry::new(positions, normals, uvs, indices)
}

/// A ridge of the tilled field, `length` meters long from Z = -2 along -Z. Near the canal it
/// is 1.5 m wide and 0.22 m high. Far away it flattens and widens to the whole row, under
/// the water of the furrows beside it, see [`FADE`].
fn field_ridge(length: f32) -> Geometry {
    let steps = 48;
    let mut positions = Vec::new();
    let mut uvs = Vec::new();
    let mut indices = Vec::new();
    for j in 0..=steps {
        let d = length * j as f32 / steps as f32;
        let f = flat(d);
        // it sinks faster than the water widens, so the water shows wider with the
        // distance and covers the whole row at the end
        let (base, top, height) = (
            mix(0.75, 0.5 * ROW + 0.05, f),
            mix(0.25, 0.5 * ROW - 0.2, f),
            0.22 * (1.0 - f).powi(2) + 0.005,
        );
        // each face of the profile has its own vertices, so the edges stay sharp
        for (x0, y0, x1, y1) in [
            (-base, 0.0, -top, height),
            (-top, height, top, height),
            (top, height, base, 0.0),
        ] {
            for (x, y) in [(x0, y0), (x1, y1)] {
                positions.push(vec3(x, y, -2.0 - d));
                uvs.push(vec2(x, -2.0 - d));
            }
        }
    }
    for j in 0..steps as u32 {
        for face in 0..3u32 {
            let a = j * 6 + face * 2;
            let (a0, a1, b0, b1) = (a, a + 1, a + 6, a + 7);
            indices.extend([a0, a1, b0, a1, b1, b0]);
        }
    }
    Geometry::new(positions, Vec::new(), uvs, indices)
}

/// The scene in the space of the field, below a group with `turn`.
fn build(assets: &Assets, turn: Quat) -> (Scene, Ids) {
    // the dawn is ahead of the first view: -Z of the field
    let ahead = turn * Vec3::NEG_Z;
    let mut scene = Scene::new()
        .environment(Environment::map(
            sky(
                Rgb::hex(0x0e1a3c) * 0.7,
                Rgb::hex(0x5a6890) * 0.55,
                Rgb::hex(0xff9a60) * 3.0,
                ahead.z.atan2(ahead.x),
            ),
            1.0,
        ))
        .background(Background::Environment { blur: 0.0 });
    let sky_stars = scene.add(Node::particles(
        stars(turn * CAMERA, 0.7),
        Blending::Additive,
    ));
    let root = scene.add(Node::group().with_rotation(turn));
    let add = |scene: &mut Scene, node: Node| scene.add_to(root, node);

    // the dry plain: pale cracked dirt, the dark tilled field, hills far away
    let mut ground = Geometry::floor_segments(5000.0, 5000.0, 250, 250);
    for p in &mut ground.positions {
        p.y = land(p.x, p.z) - 0.05;
    }
    ground.compute_vertex_normals();
    ground.update_bounds();
    add(
        &mut scene,
        Node::mesh(ground, assets.dirt.material(0xd8c4a8, 1.0, 0.0)),
    );
    add(
        &mut scene,
        Node::mesh(
            Geometry::floor(2.0 * FIELD_HALF + 20.0, (FIELD_END + 2.0).abs()),
            assets.dirt.material(0x6a5040, 1.0, 0.0),
        )
        .at(0.0, -0.03, (FIELD_END - 2.0) * 0.5),
    );
    add(
        &mut scene,
        Node::mesh(
            ridges(3200.0, 25.0, 110.0),
            assets.rock.material(0x8a7468, 1.0, 0.0),
        )
        .shadows(false, false),
    );
    // the ridges of the tilled field and dry shrubs on the plain
    let count = (2.0 * FIELD_HALF / ROW) as usize;
    let ridges_of_field: Vec<Mat4> = (0..count)
        .map(|i| Mat4::from_translation(vec3(-FIELD_HALF + i as f32 * ROW, 0.0, 0.0)))
        .collect();
    let length = (FIELD_END + 2.0).abs();
    let ridge = field_ridge(length);
    add(
        &mut scene,
        Node::instanced(
            ridge,
            assets.dirt.material(0x7a5e48, 1.0, 0.0),
            Instances::new(ridges_of_field),
        )
        .shadows(false, false),
    );
    let mut shrubs = Vec::new();
    for i in 0..3000u32 {
        let x = (hash(i * 3 + 1) - 0.5) * 1600.0;
        let z = -900.0 + hash(i * 3 + 2) * 1150.0;
        let in_field = x.abs() < FIELD_HALF + 8.0 && z < 4.0 && z > FIELD_END - 4.0;
        if in_field || z.abs() < 6.0 {
            continue;
        }
        let size = 0.3 + 0.9 * hash(i * 3 + 3).powi(2);
        shrubs.push(Mat4::from_scale_rotation_translation(
            vec3(size * 1.3, size * 0.7, size),
            Quat::from_rotation_y(hash(i) * TAU),
            vec3(x, land(x, z) + size * 0.2, z),
        ));
    }
    add(
        &mut scene,
        Node::instanced(
            Geometry::icosahedron(1.0, 1).flat(),
            Material::standard(0x4a4632).roughness(1.0),
            Instances::new(shrubs),
        ),
    );

    // the service road beside the canal, and a line of power poles along it
    add(
        &mut scene,
        Node::mesh(
            Geometry::floor(2400.0, 6.0),
            assets.dirt.material(0xb8a48c, 1.0, 0.0),
        )
        .at(0.0, 0.02, 9.0),
    );
    let poles: Vec<Mat4> = (0..40)
        .map(|i| {
            let x = -800.0 + i as f32 * 45.0;
            span(vec3(x, 0.0, 14.0), vec3(x, 9.0, 14.0), 0.14)
        })
        .chain((0..40).map(|i| {
            let x = -800.0 + i as f32 * 45.0;
            span(vec3(x - 1.2, 8.3, 14.0), vec3(x + 1.2, 8.3, 14.0), 0.08)
        }))
        .collect();
    let unit = Arc::new(Geometry::cylinder(1.0, 1.0, 1.0, 6).translate(0.0, 0.5, 0.0));
    add(
        &mut scene,
        Node::instanced(
            unit.clone(),
            Material::standard(0x3a3028).roughness(0.9),
            Instances::new(poles),
        ),
    );

    // the warm line along the canal
    let line = add(
        &mut scene,
        Node::instanced(
            unit.clone(),
            Material::unlit(Rgb::hex(AMBER_EDGE) * 8.0),
            Instances::new([span(
                vec3(-1400.0, 0.12, 0.0),
                vec3(1400.0, 0.12, 0.0),
                0.16,
            )]),
        )
        .shadows(false, false),
    );

    // the canal: two concrete walls and a floor, a unit long along X, and its water
    let concrete = assets.concrete.material(0xc8c0b4, 0.95, 0.0);
    let walls = add(
        &mut scene,
        Node::instanced(
            Geometry::merge([
                Geometry::cuboid(1.0, CANAL_WALL, 0.3).translate(
                    0.0,
                    CANAL_WALL * 0.5 - 0.3,
                    CANAL + 0.15,
                ),
                Geometry::cuboid(1.0, CANAL_WALL, 0.3).translate(
                    0.0,
                    CANAL_WALL * 0.5 - 0.3,
                    -CANAL - 0.15,
                ),
                Geometry::cuboid(1.0, 0.1, 2.0 * CANAL).translate(0.0, -0.25, 0.0),
            ]),
            concrete,
            Instances::default(),
        ),
    );
    let rims = add(
        &mut scene,
        Node::instanced(
            Geometry::merge([
                Geometry::cuboid(1.0, 0.06, 0.12).translate(0.0, CANAL_WALL - 0.27, CANAL + 0.15),
                Geometry::cuboid(1.0, 0.06, 0.12).translate(0.0, CANAL_WALL - 0.27, -CANAL - 0.15),
            ]),
            Material::unlit(Rgb::hex(AMBER_EDGE) * 5.0),
            Instances::default(),
        )
        .shadows(false, false),
    );
    let water = Material::standard(0x0a1018)
        .roughness(0.04)
        .env_intensity(1.0)
        .clearcoat(1.0);
    let canal_water = add(
        &mut scene,
        Node::instanced(
            Geometry::cuboid(1.0, 0.05, 2.0 * CANAL).translate(0.0, 0.55, 0.0),
            water.clone(),
            Instances::default(),
        )
        .shadows(false, false),
    );
    let glow = add(
        &mut scene,
        Node::instanced(
            Geometry::cuboid(1.0, 0.05, 2.0 * CANAL).translate(0.0, 0.6, 0.0),
            Material::unlit(Rgb::WHITE).additive(),
            Instances::default(),
        )
        .shadows(false, false),
    );
    // the water of a furrow in pieces, see [`PIECES`], and the warm light on it
    let furrow_water = water.roughness(0.12);
    let shine_light = Material::unlit(Rgb::WHITE).additive();
    let mut piece_nodes = |width: f32, height: f32, material: &Material| {
        std::array::from_fn(|k| {
            add(
                &mut scene,
                Node::instanced(
                    furrow_piece(k, width, height),
                    material.clone(),
                    Instances::default(),
                )
                .shadows(false, false),
            )
        })
    };
    let furrows = piece_nodes(1.0, 0.05, &furrow_water);
    let shine = piece_nodes(0.8, 0.07, &shine_light);
    // a row of the crop: tufts of leaves along -Z, as long as the field
    let mut tufts = Vec::new();
    let tuft = Geometry::icosahedron(1.0, 0);
    let steps = 140;
    for k in 0..steps {
        let z = -(k as f32 + 0.5) / steps as f32;
        let w = 0.45 + 0.15 * hash(k * 5 + 1);
        tufts.push(
            tuft.clone()
                .scale(w, 0.45 + 0.2 * hash(k * 5 + 2), w * 0.004)
                .translate(0.0, 0.15, z),
        );
    }
    let crops = add(
        &mut scene,
        Node::instanced(
            Geometry::merge(tufts).scale(1.0, 1.0, length),
            Material::standard(0xffffff)
                .roughness(0.7)
                .rim(0x9ad850, 1.6),
            Instances::default(),
        ),
    );

    // the canal paver: a portal on four tracks with a hopper and cold work lights
    let yellow = Material::standard(0xd8a020).roughness(0.5).metalness(0.2);
    let paver = add(
        &mut scene,
        Node::mesh(
            Geometry::merge([
                Geometry::cuboid(7.0, 0.8, 10.0)
                    .translate(0.0, 4.4, 0.0)
                    .with_color(0xffffff),
                Geometry::cuboid(0.6, 4.0, 0.6)
                    .translate(-3.0, 2.0, 4.6)
                    .with_color(0xffffff),
                Geometry::cuboid(0.6, 4.0, 0.6)
                    .translate(3.0, 2.0, 4.6)
                    .with_color(0xffffff),
                Geometry::cuboid(0.6, 4.0, 0.6)
                    .translate(-3.0, 2.0, -4.6)
                    .with_color(0xffffff),
                Geometry::cuboid(0.6, 4.0, 0.6)
                    .translate(3.0, 2.0, -4.6)
                    .with_color(0xffffff),
                Geometry::cuboid(3.0, 0.9, 1.2)
                    .translate(-3.0, 0.45, 4.6)
                    .with_color(0x202020),
                Geometry::cuboid(3.0, 0.9, 1.2)
                    .translate(3.0, 0.45, 4.6)
                    .with_color(0x202020),
                Geometry::cuboid(3.0, 0.9, 1.2)
                    .translate(-3.0, 0.45, -4.6)
                    .with_color(0x202020),
                Geometry::cuboid(3.0, 0.9, 1.2)
                    .translate(3.0, 0.45, -4.6)
                    .with_color(0x202020),
                Geometry::cuboid(4.0, 1.6, 4.0)
                    .translate(1.0, 5.6, 0.0)
                    .with_color(0x8a8a88),
                Geometry::cuboid(1.6, 1.8, 1.6)
                    .translate(-2.4, 5.7, 3.4)
                    .with_color(0xffffff),
            ]),
            yellow,
        )
        .hidden(),
    );
    let paver_light = add(
        &mut scene,
        Node::light(Light::spot(COLD, 0.0, 0.7, 0.5).pointing(vec3(0.3, -1.0, -0.2))),
    );
    let head_light = add(
        &mut scene,
        Node::light(Light::point(AMBER, 1500.0, 70.0)).hidden(),
    );
    let glows = add(&mut scene, Node::particles(Vec::new(), Blending::Additive));

    // the light of the sky, and the dawn glow low ahead: it rims the crop
    add(
        &mut scene,
        Node::light(Light::hemisphere(0x5a6a98, 0x1a1410, 0.7)),
    );
    add(
        &mut scene,
        Node::light(
            Light::sun(0xffa070, 0.45, vec3(0.15, 0.08, -1.0)).with_shadow(Shadow {
                map_size: 2048,
                softness: 3.0,
                strength: 0.6,
                bias: 1.5,
                bounds: Some(Aabb {
                    min: vec3(-400.0, -1.0, -360.0),
                    max: vec3(400.0, 8.0, 40.0),
                }),
            }),
        ),
    );
    // a thin mist over the plain, warm toward the dawn
    scene.atmosphere = Some(Atmosphere {
        density: 0.004,
        color: Rgb::WHITE,
        base: 0.0,
        falloff: 0.18,
        anisotropy: 0.65,
        ambient: Rgb::hex(0x4a5a88) * 0.05,
        noise: 0.6,
        noise_size: 60.0,
        offset: Vec3::ZERO,
        distance: 1200.0,
        steps: 32,
    });

    let ids = Ids {
        walls,
        rims,
        canal_water,
        glow,
        furrows,
        shine,
        crops,
        paver,
        paver_light,
        head_light,
        glows,
        line,
        stars: sky_stars,
    };
    (scene, ids)
}
