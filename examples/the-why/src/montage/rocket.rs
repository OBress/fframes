//! Part 3, 10 to 15 s of the shot: the rocket. Dawn in the desert, the sun just below a
//! warm horizon behind the pad.
//!
//! The rocket stands on its pad at the origin, the service tower on its -X side. The camera
//! is about 260 m away on +Z. The times below are seconds into the part.
//!
//! - 0.0 to 0.6: the whip pan from the greenhouse slows down and lands on the pad.
//! - 0.35 to 1.45: warm lights run up the tower like a line.
//! - 1.2 to 1.8: the crew access arm swings away. Cold vapor vents from the rocket.
//! - 2.0: ignition. Smoke and steam roll out of the flame trench, lit by the flare.
//! - 2.45: lift off. The camera tilts up after the rocket.
//! - 4.3 to 5.0: the lens zooms into the flare until it fills the frame with warm white:
//!   the cut to the sunrise of shot 9.

use std::f32::consts::{FRAC_PI_2, TAU};
use std::sync::Arc;

use fframes_3d::math::{hash, progress, smoothstep};
use fframes_3d::*;

use super::greenhouse::LEVEL;
use super::{
    AMBER, AMBER_EDGE, COLD_BLUE, camera, ease, ease_in, ease_out, mix, ridges, sky, span,
};
use crate::assets::Assets;

/// The height of the deck of the pad.
const PAD_Y: f32 = 6.0;
/// The bottom of the first stage on the launch mount.
const BASE_Y: f32 = PAD_Y + 3.2;
/// The length of the rocket, from the engines to the tip.
const LENGTH: f32 = 68.5;
/// The center of the service tower.
const TOWER_X: f32 = -13.5;
const TOWER_TOP: f32 = 86.0;
/// The half width of the square tower.
const TOWER_HALF: f32 = 4.2;
/// The tower lights come on from the bottom to the top.
const LIGHTS: (f32, f32) = (0.35, 1.45);
/// The crew access arm swings away.
const ARM: (f32, f32) = (1.2, 1.9);
const IGNITION: f32 = 2.0;
const LIFTOFF: f32 = 2.45;
/// The whip pan ends.
const ARRIVAL: f32 = 0.6;
/// The lens starts to zoom into the flare.
const FLARE: f32 = 4.25;
/// The position of the camera at rest.
const CAMERA: Vec3 = Vec3::new(46.0, 3.0, 262.0);

/// The opacity of the warm white over the picture at `t` seconds into the part. The 3D
/// flare makes most of the white. This makes the last frames nearly white for the cut.
pub fn final_white(t: f32) -> f32 {
    0.9 * ease_in(progress(t, 4.6, 5.0))
}

/// The height of the rocket above its mount at `t`: it accelerates.
fn altitude(t: f32) -> f32 {
    let s = (t - LIFTOFF).max(0.0);
    8.0 * s * s + 6.0 * s * s * s
}

/// The brightness of the engines: 0 before the ignition, 1 at full thrust, and more when
/// the flare fills the frame.
fn thrust(t: f32) -> f32 {
    smoothstep(IGNITION - 0.05, IGNITION + 0.3, t) * (1.0 + 5.0 * ease_in(progress(t, FLARE, 5.0)))
}

/// One puff of smoke, steam or vapor. It starts at `origin` at `start` seconds, slows down
/// from `velocity` by drag, and rises faster as it warms the air.
struct Puff {
    start: f32,
    origin: Vec3,
    velocity: Vec3,
    /// The upward acceleration in m/s².
    lift: f32,
    /// The radius in meters at the start. It grows with the age.
    size: f32,
    /// The age in seconds when the puff is gone.
    life: f32,
    /// The gray of the puff: steam is white, smoke is darker.
    albedo: f32,
    /// Exhaust that leaves the trench glows for its first half second.
    hot: bool,
}

impl Puff {
    /// The particle of the puff at `t`, lit by the `flame` at `flame_at` and the sky.
    fn at(&self, t: f32, flame_at: Vec3, flame: f32) -> Option<Particle> {
        let age = t - self.start;
        if age <= 0.0 || age >= self.life {
            return None;
        }
        let drag = 1.1;
        let p = self.origin
            + self.velocity * drag * (1.0 - (-age / drag).exp())
            + vec3(0.0, 0.5 * self.lift * age * age, 0.0);
        let d2 = p.distance_squared(flame_at);
        // the sky lights the tops; the flame lights the clouds near it from below
        let sky = Rgb::hex(0x9aa0c8).0 * (0.32 + 0.004 * (p.y - PAD_Y).max(0.0));
        let fire = Rgb::hex(0xffa860).0 * flame.min(1.6) * 6000.0 / (d2 + 1600.0);
        let mut color = (sky + fire) * self.albedo;
        if self.hot {
            color += Rgb::hex(0xff9a40).0 * 6.0 * (1.0 - age / 0.5).max(0.0) * flame.min(1.0);
        }
        let opacity =
            0.6 * smoothstep(0.0, 0.12, age) * (1.0 - smoothstep(self.life * 0.55, self.life, age));
        Some(Particle::new(p, self.size * (1.0 + 2.5 * age.sqrt()), Rgb(color)).opacity(opacity))
    }
}

/// The smoke and the steam of the launch, and the vapor before it.
fn puffs() -> Vec<Puff> {
    let mut puffs = Vec::new();
    // the exhaust out of both ends of the flame trench
    for i in 0..2400u32 {
        let h = |k: u32| hash(i * 11 + k);
        let side = if i % 2 == 0 { 1.0 } else { -1.0 };
        puffs.push(Puff {
            start: IGNITION + 0.08 + 2.9 * (i as f32 / 2400.0) + 0.02 * h(1),
            origin: vec3(side * 37.0, 2.5 + 2.0 * h(2), (h(3) - 0.5) * 16.0),
            velocity: vec3(
                side * (60.0 + 60.0 * h(4)),
                2.0 + 8.0 * h(5),
                (h(6) - 0.5) * 40.0,
            ),
            lift: 1.5 + 5.0 * h(7),
            size: 2.5 + 4.5 * h(8) * h(8),
            life: 3.0 + 1.5 * h(9),
            albedo: 0.55 + 0.4 * h(10),
            hot: true,
        });
    }
    // steam of the deluge water around the base of the pad
    for i in 0..500u32 {
        let h = |k: u32| hash(i * 13 + 50_000 + k);
        let a = h(1) * TAU;
        let out = vec3(a.cos(), 0.0, a.sin());
        puffs.push(Puff {
            start: IGNITION + 0.12 + 2.8 * (i as f32 / 500.0),
            origin: out * 9.0 + vec3(0.0, PAD_Y + 1.0, 0.0),
            velocity: out * (14.0 + 16.0 * h(2)) + vec3(0.0, 5.0 + 5.0 * h(3), 0.0),
            lift: 3.0 + 3.0 * h(4),
            size: 3.0 + 2.5 * h(5),
            life: 2.8 + 1.4 * h(6),
            albedo: 0.9,
            hot: false,
        });
    }
    // the column of exhaust that the rocket leaves behind it
    for i in 0..700u32 {
        let h = |k: u32| hash(i * 17 + 90_000 + k);
        let start = LIFTOFF + 0.1 + 2.5 * (i as f32 / 700.0);
        puffs.push(Puff {
            start,
            origin: vec3(
                (h(1) - 0.5) * 3.0,
                BASE_Y + altitude(start) - 8.0,
                (h(2) - 0.5) * 3.0,
            ),
            velocity: vec3((h(3) - 0.5) * 8.0, -12.0 - 10.0 * h(4), (h(5) - 0.5) * 8.0),
            lift: 1.0,
            size: 2.0 + 2.0 * h(6),
            life: 3.5,
            albedo: 0.85,
            hot: false,
        });
    }
    // cold vapor that vents from the second stage and streams down the side before the
    // launch
    for i in 0..260u32 {
        let h = |k: u32| hash(i * 19 + 130_000 + k);
        puffs.push(Puff {
            start: 0.3 + 2.0 * (i as f32 / 260.0),
            origin: vec3(1.9, BASE_Y + 50.0 + 3.0 * h(5), 0.4 + 0.8 * h(6)),
            velocity: vec3(1.0 + 1.5 * h(1), -5.0 - 3.0 * h(2), 0.5 + 1.5 * h(3)),
            lift: -2.0,
            size: 0.18 + 0.15 * h(4),
            life: 1.1,
            albedo: 3.0,
            hot: false,
        });
    }
    puffs
}

/// The node ids that the frames animate.
struct Ids {
    rocket: NodeId,
    flame: [NodeId; 2],
    flame_light: NodeId,
    trench: NodeId,
    arm: NodeId,
    tower_lights: NodeId,
    line_light: NodeId,
    smoke: NodeId,
    sparks: NodeId,
}

/// The launch pad, the rocket and its tower at dawn.
pub struct Rocket {
    scene: Scene,
    ids: Ids,
    /// The positions of the tower lights, from the bottom to the top.
    lights: Vec<Vec3>,
    puffs: Vec<Puff>,
    landmark: Scene,
}

impl Rocket {
    pub fn new(assets: &Assets) -> Self {
        let (scene, ids, lights) = build(assets);
        let landmark = contrail(&scene, &ids);
        Self {
            scene,
            ids,
            lights,
            puffs: puffs(),
            landmark,
        }
    }

    /// The scene and the camera at `t` seconds into the part, `0..5`.
    pub fn moment(&self, t: f32) -> (Scene, Camera) {
        let ids = &self.ids;
        let camera = camera_at(t);
        let mut scene = self.scene.clone();
        if let Some(air) = &mut scene.atmosphere {
            air.offset = vec3(t * 4.0, 0.0, t * 1.5);
        }

        // the line of warm lights runs up the tower, a bright head first
        let head = mix(0.0, TOWER_TOP, ease(progress(t, LIGHTS.0, LIGHTS.1)));
        let lit: Vec<Mat4> = self
            .lights
            .iter()
            .filter(|p| p.y <= head)
            .map(|p| Mat4::from_translation(*p))
            .collect();
        if let Some(mesh) = scene[ids.tower_lights].mesh_mut() {
            mesh.instances = Some(Arc::new(Instances::new(lit)));
        }
        let running = t > LIGHTS.0 && t < LIGHTS.1 + 0.3;
        scene[ids.line_light].visible = running;
        scene[ids.line_light].position = vec3(TOWER_X + TOWER_HALF + 0.5, head, TOWER_HALF + 0.5);
        let mut sparks = Vec::new();
        if running {
            let p = vec3(TOWER_X + TOWER_HALF + 0.4, head, TOWER_HALF + 0.4);
            sparks.push(Particle::new(p, 0.9, Rgb::hex(AMBER) * 30.0));
        }

        // the crew access arm swings away from the rocket
        scene[ids.arm].rotation = Quat::from_rotation_y(-1.3 * ease(progress(t, ARM.0, ARM.1)));

        // the rocket and its flame
        let rocket_y = BASE_Y + altitude(t);
        scene[ids.rocket].position.y = rocket_y;
        let power = thrust(t);
        // on the pad the flame goes down into the trench; in the air it grows long
        let reach = (rocket_y - PAD_Y) / 46.0;
        let length = reach.clamp(0.12, 1.0)
            * (1.0 + 0.6 * smoothstep(1.0, 3.0, reach))
            * (1.0 + ease_in(progress(t, FLARE, 5.0)));
        for (i, id) in ids.flame.into_iter().enumerate() {
            let flame = &mut scene[id];
            flame.visible = power > 0.0;
            flame.position.y = rocket_y;
            let flicker = 1.0 + 0.06 * (t * 47.0 + i as f32).sin();
            flame.scale = vec3(flicker, length * power.clamp(0.05, 1.0), flicker);
        }
        let flame_at = vec3(0.0, rocket_y - 10.0 * length, 0.0);
        scene[ids.flame_light].visible = power > 0.0;
        scene[ids.flame_light].position = flame_at;
        if let Some(light) = scene[ids.flame_light].light_mut() {
            *light.intensity_mut() = 1.6e4 * power;
        }
        // the mouths of the trench glow while the rocket is low
        let trench = power.min(1.0) * (1.0 - smoothstep(30.0, 90.0, rocket_y));
        if let Some(m) = scene[ids.trench].material_mut() {
            m.color = Rgb::hex(0x050505).lerp(Rgb::hex(0xffa860) * 12.0, trench);
        }

        // the flame as soft light along its length, the flash of the ignition, and the
        // flare that fills the frame at the end
        let zoom = ease_in(progress(t, FLARE, 5.0));
        if power > 0.0 {
            for k in 0..24 {
                let s = k as f32 / 24.0;
                let p = vec3(0.0, rocket_y - 2.0 - s * 40.0 * length, 0.0);
                let color = Rgb::hex(0xfff0d8).lerp(Rgb::hex(AMBER_EDGE), s)
                    * (6.0 * (1.0 - s).powf(1.5) * power.min(1.5));
                sparks.push(Particle::new(p, 2.4 + 4.0 * s, color));
            }
            // a glow of three layers, wide and dim to small and hot, so it fades out softly
            let mut glow = |at: Vec3, radius: f32, strength: f32| {
                for (scale, gain, color) in [
                    (1.0, 1.0, 0xffd2a0),
                    (0.4, 4.0, 0xffe6c4),
                    (0.12, 12.0, 0xfff6ea),
                ] {
                    sparks.push(Particle::new(
                        at,
                        radius * scale,
                        Rgb::hex(color) * (strength * gain),
                    ));
                }
            };
            let flash = (-(t - IGNITION).max(0.0) / 0.35).exp()
                * smoothstep(IGNITION - 0.05, IGNITION + 0.05, t);
            glow(vec3(0.0, PAD_Y + 2.0, 0.0), 45.0, 1.6 * flash);
            glow(
                vec3(0.0, rocket_y - 4.0, 0.0),
                10.0 + 120.0 * zoom,
                0.5 + 5.0 * zoom,
            );
        }
        // the igniters throw sparks just before the engines start
        if t > IGNITION - 0.25 && t < IGNITION + 0.35 {
            for i in 0..60u32 {
                let age = (t - IGNITION + 0.25 - 0.6 * hash(i * 3)).rem_euclid(0.6);
                let dir = vec3(
                    hash(i * 3 + 1) - 0.5,
                    0.15 * hash(i * 5),
                    hash(i * 3 + 2) - 0.5,
                )
                .normalize();
                let p = vec3(0.0, PAD_Y + 0.5, 0.0) + dir * age * 60.0;
                sparks.push(Particle::new(p, 0.15, Rgb::hex(0xffc070) * 40.0).tail(-dir * 3.0));
            }
        }
        if t < ARM.1 + 0.2 {
            let blink = if (t * 4.0).fract() < 0.5 { 25.0 } else { 4.0 };
            let arm = scene.local_to_world(ids.arm, vec3(6.5, 1.8, 1.4));
            sparks.push(Particle::new(arm, 0.18, Rgb::hex(COLD_BLUE) * blink));
            sparks.push(Particle::new(
                vec3(TOWER_X + TOWER_HALF, 24.0, 2.0),
                0.18,
                Rgb::hex(COLD_BLUE) * blink,
            ));
        }
        if let Object::Particles(p) = &mut scene[ids.sparks].object {
            p.items = sparks;
        }

        // smoke and steam, lit by the flame, drawn from back to front
        let mut smoke: Vec<Particle> = self
            .puffs
            .iter()
            .filter_map(|p| p.at(t, flame_at, power))
            .collect();
        smoke.sort_by(|a, b| {
            b.position
                .distance_squared(camera.position)
                .total_cmp(&a.position.distance_squared(camera.position))
        });
        if let Object::Particles(p) = &mut scene[ids.smoke].object {
            p.items = smoke;
        }
        (scene, camera)
    }

    /// The pad with its tower and the trail of the rocket for shot 9, see
    /// [`super::Landmarks`].
    pub fn landmark(&self) -> Scene {
        self.landmark.clone()
    }
}

/// The scene template. Returns the positions of the tower lights too.
fn build(assets: &Assets) -> (Scene, Ids, Vec<Vec3>) {
    let mut scene = Scene::new()
        .environment(Environment::map(
            sky(
                Rgb::hex(0x172a52),
                Rgb::hex(0x8c90b4) * 0.55,
                Rgb::hex(0xff9a48) * 1.6,
                -1.32,
            ),
            1.0,
        ))
        .background(Background::Environment { blur: 0.0 });

    // the dry lake bed, the concrete apron, and the mountains far away
    scene.add(
        Node::mesh(
            Geometry::floor(9000.0, 9000.0),
            assets.dirt.material(0xd0c0b0, 1.0, 0.0),
        )
        .named("ground"),
    );
    scene.add(
        Node::mesh(
            Geometry::floor(180.0, 150.0),
            assets.concrete.material(0xc8c4bc, 0.9, 0.0),
        )
        .at(0.0, 0.03, 10.0)
        .named("ground"),
    );
    scene.add(
        Node::mesh(
            ridges(5200.0, 90.0, 420.0),
            assets.rock.material(0x8a7468, 1.0, 0.0),
        )
        .named("ground")
        .shadows(false, false),
    );

    let concrete = assets.concrete.material(0xbab6ae, 0.9, 0.0);
    let steel = assets.steel.material(0x6a6e74, 0.7, 0.6);
    let dark = Material::standard(0x1b1c1f).roughness(0.55);
    let unit = Arc::new(Geometry::cuboid(1.0, 1.0, 1.0).translate(0.0, 0.5, 0.0));

    // the pad: a concrete block with the flame trench through it, and the ramp up to it
    scene.add(Node::mesh(
        Geometry::cuboid(74.0, PAD_Y, 52.0).translate(0.0, PAD_Y * 0.5, 0.0),
        concrete.clone(),
    ));
    let ramp = Geometry::extrude(
        &[vec2(0.0, 0.0), vec2(60.0, 0.0), vec2(0.0, PAD_Y)],
        18.0,
        0.2,
    )
    .rotate_y(-FRAC_PI_2)
    .translate(0.0, 0.0, 26.0);
    scene.add(Node::mesh(ramp, concrete.clone()));
    let mouths = Geometry::merge([-1.0f32, 1.0].map(|side| {
        Geometry::plane(22.0, 4.6)
            .rotate_y(side * FRAC_PI_2)
            .translate(side * 37.05, 2.5, 0.0)
    }));
    let trench = scene.add(Node::mesh(mouths, Material::unlit(0x050505)).shadows(false, false));
    // the launch mount: four legs and a ring that holds the rocket
    for (x, z) in [(-4.5f32, -4.5f32), (4.5, -4.5), (-4.5, 4.5), (4.5, 4.5)] {
        scene.add(Node::mesh(
            Geometry::cuboid(1.6, 3.2, 1.6).translate(x, PAD_Y + 1.6, z),
            steel.clone(),
        ));
    }
    scene.add(Node::mesh(
        Geometry::tube_ring(5.6, 3.4, 0.9, 32).translate(0.0, BASE_Y - 0.45, 0.0),
        steel.clone(),
    ));
    // the rails of the transporter to the hangar
    for z in [-3.0f32, 3.0] {
        scene.add(Node::mesh(
            Geometry::cuboid(900.0, 0.35, 0.5).translate(487.0, 0.18, z),
            steel.clone(),
        ));
    }
    scene.add(Node::mesh(
        Geometry::rounded_box(110.0, 46.0, 70.0, 1.0, 2).translate(980.0, 23.0, -20.0),
        concrete.clone(),
    ));

    // the propellant farm: a sphere of liquid oxygen, two kerosene tanks, and the pipes to
    // the pad
    let tank = Material::standard(0xe2e0da).roughness(0.45);
    scene.add(Node::mesh(
        Geometry::sphere(8.0, 32, 20).translate(-62.0, 11.0, -42.0),
        tank.clone(),
    ));
    for (x, z) in [
        (-67.0f32, -47.0f32),
        (-57.0, -47.0),
        (-67.0, -37.0),
        (-57.0, -37.0),
    ] {
        scene.add(Node::mesh(
            Geometry::cuboid(0.8, 6.0, 0.8).translate(x, 3.0, z),
            steel.clone(),
        ));
    }
    for z in [-30.0f32, -38.0] {
        scene.add(Node::mesh(
            Geometry::capsule(3.2, 22.0, 8, 24)
                .rotate_z(FRAC_PI_2)
                .translate(60.0, 4.6, z),
            tank.clone(),
        ));
        for x in [52.0f32, 68.0] {
            scene.add(Node::mesh(
                Geometry::cuboid(1.2, 2.0, 6.0).translate(x, 1.0, z),
                concrete.clone(),
            ));
        }
    }
    let pipes = Geometry::merge([
        Geometry::tube(
            &[
                vec3(-54.0, 1.0, -40.0),
                vec3(-36.0, 1.0, -20.0),
                vec3(-8.0, PAD_Y + 0.6, -6.0),
            ],
            0.5,
            8,
        ),
        Geometry::tube(
            &[
                vec3(47.0, 1.0, -34.0),
                vec3(36.0, 1.0, -22.0),
                vec3(8.0, PAD_Y + 0.6, -6.0),
            ],
            0.4,
            8,
        ),
    ]);
    scene.add(Node::mesh(pipes, steel.clone()));
    // the road to the pad, and a low blockhouse with lit windows
    scene.add(
        Node::mesh(
            Geometry::floor(10.0, 190.0),
            assets.asphalt.material(0x8a8a8a, 0.9, 0.0),
        )
        .at(24.0, 0.05, 160.0),
    );
    scene.add(Node::mesh(
        Geometry::rounded_box(18.0, 4.5, 10.0, 0.4, 2).translate(-48.0, 2.25, 66.0),
        concrete.clone(),
    ));
    scene.add(Node::mesh(
        Geometry::cuboid(12.0, 0.8, 0.1).translate(-48.0, 2.6, 71.06),
        Material::unlit(Rgb::hex(AMBER) * 5.0),
    ));

    // the service tower: four columns, a beam at each level, and cross braces
    let mut beams = Vec::new();
    let corners = [(-1.0f32, -1.0f32), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)]
        .map(|(x, z)| vec3(TOWER_X + x * TOWER_HALF, 0.0, z * TOWER_HALF));
    for c in corners {
        beams.push(span(c, c + vec3(0.0, TOWER_TOP, 0.0), 0.75));
    }
    let levels = 20;
    for l in 0..levels {
        let (y0, y1) = (
            l as f32 * TOWER_TOP / levels as f32,
            (l + 1) as f32 * TOWER_TOP / levels as f32,
        );
        for k in 0..4 {
            let (a, b) = (corners[k], corners[(k + 1) % 4]);
            beams.push(span(a + vec3(0.0, y1, 0.0), b + vec3(0.0, y1, 0.0), 0.4));
            beams.push(span(a + vec3(0.0, y0, 0.0), b + vec3(0.0, y1, 0.0), 0.22));
            beams.push(span(b + vec3(0.0, y0, 0.0), a + vec3(0.0, y1, 0.0), 0.22));
        }
    }
    scene.add(Node::instanced(
        unit.clone(),
        steel.clone(),
        Instances::new(beams),
    ));
    // the elevator core and the lightning rod on the top
    scene.add(Node::mesh(
        Geometry::cuboid(3.2, TOWER_TOP, 3.2).translate(TOWER_X - 1.0, TOWER_TOP * 0.5, -1.0),
        dark.clone(),
    ));
    scene.add(Node::mesh(
        Geometry::cylinder(0.12, 0.3, 14.0, 6).translate(TOWER_X, TOWER_TOP + 7.0, 0.0),
        steel.clone(),
    ));
    // the umbilical arms, and the crew access arm that swings away
    for y in [24.0f32, 44.0] {
        scene.add(Node::mesh(
            Geometry::cuboid(7.2, 1.2, 1.6).translate(TOWER_X + TOWER_HALF + 3.6, y, 0.0),
            steel.clone(),
        ));
    }
    let arm = scene.add(Node::group().at(TOWER_X + TOWER_HALF, BASE_Y + 50.0, 0.0));
    scene.add_to(
        arm,
        Node::mesh(
            Geometry::merge([
                Geometry::cuboid(6.4, 0.5, 2.6).translate(3.2, 0.0, 0.0),
                Geometry::cuboid(6.4, 0.3, 0.3).translate(3.2, 2.6, 1.15),
                Geometry::cuboid(6.4, 0.3, 0.3).translate(3.2, 2.6, -1.15),
                Geometry::cuboid(2.4, 3.2, 3.0).translate(6.2, 1.6, 0.0),
            ]),
            steel.clone(),
        ),
    );
    // the lights of the tower: a line up its corner that faces the camera
    let lights: Vec<Vec3> = (0..30)
        .map(|i| {
            vec3(
                TOWER_X + TOWER_HALF + 0.45,
                2.0 + i as f32 * 2.85,
                TOWER_HALF + 0.45,
            )
        })
        .collect();
    let tower_lights = scene.add(
        Node::instanced(
            Geometry::cuboid(0.5, 0.35, 0.5),
            Material::unlit(Rgb::hex(AMBER) * 14.0),
            Instances::default(),
        )
        .shadows(false, false),
    );
    let line_light = scene.add(Node::light(Light::point(AMBER, 2500.0, 60.0)).hidden());

    // the rocket: two white stages, a black interstage, the fairing, the engines
    let rocket = scene.add(Node::group().at(0.0, BASE_Y, 0.0));
    let white = Material::standard(0xecebe6).roughness(0.35).clearcoat(0.5);
    let mut fairing = vec![
        vec2(1.85, 46.0),
        vec2(1.85, 53.2),
        vec2(2.3, 54.9),
        vec2(2.3, 60.5),
    ];
    for i in 1..=10 {
        let s = i as f32 / 10.0;
        fairing.push(vec2(
            2.3 * (1.0 - s * s).max(0.0).powf(0.7),
            60.5 + (LENGTH - 60.5) * s,
        ));
    }
    let body = Geometry::merge([
        Geometry::lathe(&[vec2(1.55, 0.0), vec2(1.85, 0.3), vec2(1.85, 41.6)], 48),
        Geometry::lathe(&fairing, 48),
    ]);
    scene.add_to(rocket, Node::mesh(body, white));
    let mut marks = vec![
        Geometry::lathe(&[vec2(1.87, 41.6), vec2(1.87, 46.0)], 48),
        Geometry::circle(1.56, 32).rotate_x(FRAC_PI_2),
        // a cable raceway along the first stage
        Geometry::cuboid(0.3, 38.0, 0.2).translate(0.0, 21.0, 1.9),
    ];
    for k in 0..4 {
        let a = k as f32 * FRAC_PI_2 + 0.4;
        let out = vec3(a.cos(), 0.0, a.sin());
        // grid fins and the folded landing legs
        marks.push(Geometry::cuboid(1.1, 0.12, 1.3).rotate_y(-a).translate(
            out.x * 2.3,
            44.5,
            out.z * 2.3,
        ));
        marks.push(
            Geometry::cuboid(0.6, 11.0, 0.3)
                .rotate_y(-a + FRAC_PI_2)
                .translate(out.x * 1.95, 6.0, out.z * 1.95),
        );
    }
    scene.add_to(rocket, Node::mesh(Geometry::merge(marks), dark.clone()));
    let bell = Geometry::lathe(
        &[
            vec2(0.3, 0.0),
            vec2(0.36, -0.3),
            vec2(0.62, -1.2),
            vec2(0.78, -1.9),
        ],
        20,
    );
    let mut engines = vec![Mat4::IDENTITY];
    engines.extend((0..8).map(|k| {
        let a = k as f32 / 8.0 * TAU;
        Mat4::from_translation(vec3(1.15 * a.cos(), 0.0, 1.15 * a.sin()))
    }));
    scene.add_to(
        rocket,
        Node::instanced(
            bell,
            Material::metal(0x5a524c, 0.4).double_sided(),
            Instances::new(engines),
        ),
    );

    // the flame: a white core and a wider orange glow, scaled along Y by the frames
    let flame_shape = |r: f32| {
        Geometry::lathe(
            &[
                vec2(0.0, 0.5),
                vec2(1.7 * r, -1.0),
                vec2(2.6 * r, -6.0),
                vec2(2.2 * r, -18.0),
                vec2(1.2 * r, -32.0),
                vec2(0.0, -46.0),
            ],
            24,
        )
    };
    let flame = [
        (flame_shape(1.0), Material::glow(0xfff1d6, 9.0)),
        (flame_shape(1.8), Material::glow(AMBER_EDGE, 1.6)),
    ]
    .map(|(g, m)| {
        scene.add(
            Node::mesh(g, m.double_sided())
                .shadows(false, false)
                .hidden(),
        )
    });
    let flame_light = scene.add(Node::light(Light::point(0xffb070, 0.0, 450.0)).hidden());

    // flood light towers that light the rocket warm, lightning masts, a water tower
    for (x, z) in [
        (-78.0f32, 52.0f32),
        (78.0, 52.0),
        (-80.0, -62.0),
        (80.0, -62.0),
    ] {
        scene.add(Node::mesh(
            Geometry::cuboid(0.9, 42.0, 0.9).translate(x, 21.0, z),
            steel.clone(),
        ));
        let facing = (-x).atan2(-z);
        scene.add(
            Node::mesh(
                Geometry::cuboid(6.0, 3.0, 0.6),
                Material::unlit(Rgb::hex(0xffe0b0) * 9.0),
            )
            .at(x, 42.0, z)
            .with_rotation(Quat::from_rotation_y(facing)),
        );
    }
    for (x, z) in [(-62.0f32, 52.0f32), (62.0, 52.0)] {
        let target = vec3(0.0, 40.0, 0.0);
        let from = vec3(x, 42.0, z);
        scene.add(
            Node::light(Light::spot(0xffcf98, 2.6e4, 0.32, 0.5).pointing(target - from))
                .with_position(from),
        );
    }
    for (x, z) in [(-84.0f32, -38.0f32), (76.0, -58.0), (36.0, 84.0)] {
        scene.add(Node::mesh(
            Geometry::cuboid(1.3, 118.0, 1.3).translate(x, 59.0, z),
            steel.clone(),
        ));
        scene.add(Node::mesh(
            Geometry::cylinder(0.05, 0.3, 10.0, 5).translate(x, 123.0, z),
            steel.clone(),
        ));
    }
    scene.add(Node::mesh(
        Geometry::cylinder(1.6, 2.0, 34.0, 12).translate(130.0, 17.0, -120.0),
        concrete.clone(),
    ));
    scene.add(Node::mesh(
        Geometry::sphere(7.5, 24, 16).translate(130.0, 38.0, -120.0),
        Material::standard(0xdcdad4).roughness(0.5),
    ));

    let smoke = scene.add(Node::particles(Vec::new(), Blending::Normal));
    let sparks = scene.add(Node::particles(Vec::new(), Blending::Additive));

    // the glow of the sun below the horizon behind the pad, and the sky light
    scene.add(Node::light(
        Light::sun(0xff9f6a, 0.9, vec3(0.3, 0.05, -1.0)).with_shadow(Shadow {
            map_size: 2048,
            softness: 2.0,
            strength: 0.8,
            bias: 2.0,
            bounds: Some(Aabb {
                min: vec3(-140.0, -2.0, -140.0),
                max: vec3(140.0, 130.0, 140.0),
            }),
        }),
    ));
    scene.add(Node::light(Light::hemisphere(0x6a7eb8, 0x3a3028, 1.2)));
    scene.atmosphere = Some(Atmosphere {
        density: 0.0012,
        color: Rgb::WHITE,
        base: 0.0,
        falloff: 0.02,
        anisotropy: 0.5,
        ambient: Rgb::hex(0x6a6a98) * 0.14,
        noise: 0.45,
        noise_size: 40.0,
        offset: Vec3::ZERO,
        distance: 1500.0,
        steps: 32,
    });

    let ids = Ids {
        rocket,
        flame,
        flame_light,
        trench,
        arm,
        tower_lights,
        line_light,
        smoke,
        sparks,
    };
    (scene, ids, lights)
}

/// The landmark of shot 9: the pad, the tower and the lit lights from the template, and
/// the trail that the rocket left: a column of smoke that rises 2.5 km and bends with the
/// wind to +X. Its top catches the first sun.
fn contrail(template: &Scene, ids: &Ids) -> Scene {
    let mut scene = template.clone();
    let remove: Vec<NodeId> = scene
        .nodes()
        .filter(|(id, node)| {
            node.name.as_deref() == Some("ground")
                || matches!(node.object, Object::Light(_))
                || *id == ids.rocket
        })
        .map(|(id, _)| id)
        .collect();
    for id in remove {
        scene[id].visible = false;
    }
    let mut trail = Vec::new();
    for i in 0..1800u32 {
        let s = i as f32 / 1800.0;
        let y = 2500.0 * s.powf(1.3);
        // the wind at each height pushes the trail to +X and shears it into waves
        let drift = vec3(600.0 * s * s, 0.0, 180.0 * s * (y / 380.0).sin());
        let spread = 6.0 + 140.0 * s;
        let wobble = vec3(
            (hash(i * 3) - 0.5) * spread,
            (hash(i * 3 + 1) - 0.5) * 10.0,
            (hash(i * 3 + 2) - 0.5) * spread,
        );
        let sun = smoothstep(0.35, 0.8, s);
        let color = Rgb::hex(0xc8c8d6).lerp(Rgb::hex(0xffd8b0) * 2.6, sun);
        trail.push(
            Particle::new(
                drift + vec3(0.0, BASE_Y + y, 0.0) + wobble,
                8.0 + 70.0 * s,
                color,
            )
            .opacity(0.3),
        );
    }
    scene.add(Node::particles(trail, Blending::Normal));
    if let Some(mesh) = scene[ids.tower_lights].mesh_mut() {
        let lit: Vec<Mat4> = (0..30)
            .map(|i| {
                Mat4::from_translation(vec3(
                    TOWER_X + TOWER_HALF + 0.45,
                    2.0 + i as f32 * 2.85,
                    TOWER_HALF + 0.45,
                ))
            })
            .collect();
        mesh.instances = Some(Arc::new(Instances::new(lit)));
    }
    scene
}

/// The camera at `t`: it lands from the whip pan on the pad, pushes in a little, tilts up
/// after the rocket, and at the end zooms into the flare.
fn camera_at(t: f32) -> Camera {
    let position = CAMERA + vec3(-4.0, 0.0, -22.0) * ease(progress(t, ARRIVAL, IGNITION + 0.5));
    // at rest the view looks up at the middle of the tower; at the cut it is level with
    // the view of the greenhouse part
    let rest = vec3(-6.0, 38.0, 0.0);
    let level = vec3(
        -6.0,
        CAMERA.y + LEVEL * vec2(CAMERA.x, CAMERA.z).length(),
        0.0,
    );
    let rest = level.lerp(rest, ease(progress(t, 0.0, ARRIVAL + 0.3)));
    let rocket_y = BASE_Y + altitude(t);
    let follow = ease(progress(t, LIFTOFF + 0.15, FLARE + 0.2));
    let mut target = rest.lerp(vec3(-2.0, rocket_y + 12.0, 0.0), follow);
    // the whip pan ends: the view turns to the left and slows down. It starts at 3.5 rad/s,
    // the speed at the end of the greenhouse part.
    let arrive = ease_out(progress(t, 0.0, ARRIVAL));
    let dir = target - position;
    target = position + Quat::from_rotation_y(-0.7 * (1.0 - arrive)) * dir;
    // the zoom into the flare
    let zoom = ease_in(progress(t, FLARE, 5.0));
    let flame = vec3(0.0, rocket_y - 14.0, 0.0);
    target = target.lerp(flame, ease(progress(t, FLARE - 0.2, 4.8)));
    let mm = mix(mix(35.0, 42.0, follow), 260.0, zoom);
    // the ground shakes the camera from the ignition until the rocket is clear
    camera(mm, position, target)
        .focus_on(vec3(0.0, rocket_y, 0.0), 3.0)
        .shake(
            t,
            0.0006
                + 0.004
                    * smoothstep(IGNITION, LIFTOFF, t)
                    * (1.0 - smoothstep(LIFTOFF + 0.5, 3.6, t)),
            23,
        )
}
