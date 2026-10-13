//! Part 3, 10 to 15 s of the shot: the launch at dawn. The sun is just below the horizon
//! behind the pad, and the haze glows warm there.
//!
//! The rocket stands on its launch mount at the origin, the service tower on its -X side.
//! The flame trench runs along X under the pad and opens on both sides. The camera stands
//! about 210 m away on +Z. The times below are seconds into the part.
//!
//! - 0.0 to 0.3: the whip pan from the greenhouse lands on the pad. The spark of the
//!   greenhouse flies on to the foot of the tower. The pad is bare: the tower is one
//!   level high.
//! - 0.0 to 1.05: the transporter erector rolls the rocket out of the hangar on the right,
//!   down the ramp and over the trench to the mount. It comes in fast and slows down.
//! - 0.45 to 1.45: the spark runs up the corner of the tower, and a crawler crane builds the
//!   tower level by level just above it. The lamps of each level come on behind the
//!   spark: a warm line from the ground to the top. The hammerhead crane and the machine
//!   room come last.
//! - 1.05 to 1.45: the transporter erector raises the rocket.
//! - 1.45 to 1.6: the arms swing to the rocket. 1.45 to 1.95: the crawler crane lowers its
//!   boom and drives away.
//! - 1.4 to 1.85: the tanks fill: frost grows up the oxygen tank, and cold vapor vents
//!   from it and from the sphere of oxygen, until 2.3.
//! - 1.6 to 1.85: the strongback tilts back. 1.65 to 1.9: the crew access arm swings away.
//!   1.7 to 1.95: the vent hood lifts off the nose and swings away.
//! - 1.55: the water deluge starts: arcs of water from the four corners of the deck.
//! - 1.8 to 2.3: the igniters throw sparks, with a short green flash.
//! - 2.0: ignition. Fire leaves both ends of the trench, then steam billows out in big
//!   clouds, lit warm by the flame from below.
//! - 2.45: lift off. The umbilical arms swing away. The sound reaches the camera, which
//!   shakes, and it tilts up after the climbing rocket.
//! - 4.2: the rocket clears the tower.
//! - 4.25 to 4.9: the lens zooms into the flame: the shock diamonds under the engines.
//! - 4.6 to 5.0: the flare grows until it fills the frame with warm white: the cut to the
//!   sunrise of shot 9.

mod machines;

use std::f32::consts::{FRAC_PI_2, PI, TAU};
use std::sync::Arc;

use fframes_3d::math::{hash, noise1, progress, random_direction, smoothstep};
use fframes_3d::*;

use super::greenhouse::{LEVEL, SPARK_AT_CUT, SPARK_LEAD};
use super::{
    AMBER, AMBER_EDGE, COLD, COLD_BLUE, WHIP_HALF, camera, ease, ease_in, ease_out, haze_steps,
    mix, polar, ridges, sky, span, spark, whip, yaw_pitch,
};
use crate::assets::Assets;

/// The height of the deck of the pad.
const PAD_Y: f32 = 6.0;
/// The bottom of the first stage on the launch mount.
const BASE_Y: f32 = PAD_Y + 4.0;
/// The bottom and the top of the frost on the oxygen tank, from the engines.
const FROST: (f32, f32) = (26.0, 40.6);
/// The length of the rocket, from the engines to the tip.
const LENGTH: f32 = 68.5;
/// The radius of the stages.
const BODY: f32 = 1.85;
/// The center of the service tower.
const TOWER_X: f32 = -14.0;
/// The half width of the square tower.
const TOWER_HALF: f32 = 4.5;
/// The height of one level of the tower.
const LEVEL_HEIGHT: f32 = 4.4;
const LEVELS: usize = 19;
/// The top of the steel of the tower.
const TOWER_TOP: f32 = PAD_Y + LEVELS as f32 * LEVEL_HEIGHT;
/// The face of the tower toward the rocket.
const FACE_X: f32 = TOWER_X + TOWER_HALF;
/// The mouths of the flame trench at X = ±`MOUTH_X`.
const MOUTH_X: f32 = 44.0;
/// The spark of the greenhouse reaches the foot of the tower.
const SPARK_ARRIVES: f32 = 0.45;
/// The spark runs up the tower and the lights come on.
const LIGHTS: (f32, f32) = (0.45, 1.45);
/// The transporter erector rolls the rocket from the hangar to the pad.
const ROLLOUT: (f32, f32) = (0.0, 1.05);
/// It raises the rocket.
const RAISE: (f32, f32) = (1.05, 1.45);
/// The arms swing to the rocket.
const ARMS_IN: (f32, f32) = (1.45, 1.6);
/// The oxygen tank fills, and frost grows up on it.
const FUELING: (f32, f32) = (1.4, 1.85);
/// The strongback of the transporter erector tilts back from the rocket.
const STRONGBACK: (f32, f32) = (1.6, 1.85);
/// The crew access arm swings away.
const ARM: (f32, f32) = (1.65, 1.9);
/// The vent hood lifts off the nose and swings away.
const HOOD: (f32, f32) = (1.7, 1.95);
/// The crawler crane lowers its boom and drives away.
const CRANE_AWAY: (f32, f32) = (1.45, 1.95);
/// The pivot of the transporter erector: the rocket and its strongback turn around it.
const PIVOT: Vec3 = Vec3::new(BODY + 1.6, PAD_Y + 2.6, 0.0);
/// The length of the strongback.
const STRONGBACK_LENGTH: f32 = 68.0;
/// The length of the chassis of the transporter erector from the pivot.
const CHASSIS: f32 = 78.0;
/// The door of the hangar, and the X of the pivot when the rollout starts.
const HANGAR: f32 = 192.0;
const HANGAR_X: f32 = HANGAR + 4.0;
/// The ramp of the rails from the bridge over the trench down to the ground.
const RAMP: (f32, f32) = (84.0, 184.0);
/// The crawler crane that builds the tower, and the length of its boom.
const CRANE_AT: Vec3 = Vec3::new(-52.0, 0.0, -42.0);
const BOOM: f32 = 120.0;
/// The paints of the machines.
const YELLOW: u32 = 0xd8b030;
const DARK: u32 = 0x2a2c30;
/// The water deluge starts.
const DELUGE: f32 = 1.55;
const IGNITION: f32 = 2.0;
pub(super) const LIFTOFF: f32 = 2.45;
/// The lens zooms into the flame from `ZOOM.0`; it reaches its longest focal length at
/// `ZOOM.1`.
const ZOOM: (f32, f32) = (4.25, 4.9);
/// The flare grows from `FLARE` until it fills the frame at the end.
const FLARE: f32 = 4.6;
/// The direction of the dawn glow in the sky, `atan2(z, x)` in radians: behind the pad, a
/// little to the right of the rocket.
pub(super) const GLOW: f32 = -1.5;
/// The position of the camera when the whip pan lands, and its push in to the pad.
const CAMERA: Vec3 = Vec3::new(40.0, 4.5, 214.0);
const PUSH: Vec3 = Vec3::new(-5.0, 0.0, -22.0);
/// The point that the camera looks at when it rests.
const VIEW: Vec3 = Vec3::new(-5.0, 46.0, 0.0);

/// The opacity of the warm white over the picture at `t` seconds into the part. The 3D
/// flare makes most of the white. This makes the last frames nearly white for the cut.
pub fn final_white(t: f32) -> f32 {
    0.9 * ease_in(progress(t, 4.6, 5.0))
}

/// The height of the rocket above its mount at `t`: it starts slowly, heavy, and
/// accelerates.
fn altitude(t: f32) -> f32 {
    let s = (t - LIFTOFF).max(0.0);
    6.0 * s * s + 12.0 * s * s * s
}

/// The brightness of the engines: 0 before the ignition, 1 at full thrust, and more when
/// the flare fills the frame. It flickers a little.
fn thrust(t: f32) -> f32 {
    let on = smoothstep(IGNITION - 0.04, IGNITION + 0.25, t);
    on * (1.0 + 0.08 * noise1(41, t * 30.0)) * (1.0 + 5.0 * ease_in(progress(t, FLARE, 5.0)))
}

/// The lights that color the steam: the fires and the sky.
struct Lighting {
    /// The places of the fires and their light: the color times the power.
    fires: Vec<(Vec3, Vec3)>,
    /// The direction to the dawn glow.
    glow: Vec3,
}

impl Lighting {
    /// The light that steam at `p` sends to the camera. `n` is the direction from the
    /// center of its cloud: the top of a cloud gets the sky, the side to a fire gets the
    /// fire.
    fn steam(&self, p: Vec3, n: Vec3) -> Vec3 {
        let sky = Rgb::hex(0xc4c4dc).0 * (0.3 + 1.2 * (0.5 + 0.5 * n.y).powf(1.5));
        let dawn = Rgb::hex(0xffa070).0 * (0.3 * n.dot(self.glow).max(0.0));
        let mut color = sky + dawn;
        for &(at, light) in &self.fires {
            let to = at - p;
            let d2 = to.length_squared();
            let d = d2.sqrt().max(0.1);
            // light that wraps around the cloud, strongest on the side to the fire. The
            // steam between absorbs it, so far clouds get little.
            let facing = 0.3 + 0.7 * (0.5 + 0.5 * n.dot(to / d));
            color += light * (facing * (-d / 35.0).exp() / (d2 + 300.0));
        }
        color
    }
}

/// A mass of steam, smoke or vapor. It leaves `origin` at `start`, slows down from
/// `velocity` by drag, rises as it warms the air, and grows. It rolls as it moves, so its
/// puffs turn around its center.
///
/// The puffs are opaque: the renderer darkens a normal particle with an opacity below 1
/// and does not make it thinner. A cloud that goes away shrinks.
struct Cloud {
    start: f32,
    origin: Vec3,
    velocity: Vec3,
    /// The time constant of the drag in seconds.
    drag: f32,
    /// The upward acceleration in m/s². Cold vapor sinks.
    lift: f32,
    /// The radius in meters at the start, and its growth in m/√s.
    radius: f32,
    growth: f32,
    puffs: u32,
    /// The gray of the steam: 1 is white.
    albedo: f32,
    /// The age in seconds at which it is gone.
    life: f32,
    seed: u32,
}

impl Cloud {
    /// Adds the puffs of the cloud at `t` to `out`.
    fn puffs(&self, t: f32, light: &Lighting, out: &mut Vec<Particle>) {
        let age = t - self.start;
        if age <= 0.0 || age >= self.life {
            return;
        }
        let radius = (self.radius + self.growth * age.sqrt())
            * (1.0 - smoothstep(self.life * 0.5, self.life, age));
        let travel = self.drag * (1.0 - (-age / self.drag).exp());
        let mut center =
            self.origin + self.velocity * travel + Vec3::Y * (0.5 * self.lift * age * age);
        center.y = center.y.max(radius * 0.5);
        let ahead = self.velocity.with_y(0.0);
        let roll = Quat::from_axis_angle(
            Vec3::Y.cross(ahead).normalize_or(Vec3::X),
            ahead.length() * travel / (2.0 * radius),
        );
        // a dense core and lumps on its surface. The steam near the ground is in the shade
        // of the steam above it.
        let shade = self.albedo * (0.55 + 0.45 * smoothstep(0.0, 40.0, center.y));
        let core = light.steam(center, Vec3::Y) * (shade * 0.8);
        out.push(Particle::new(center, radius * 0.85, Rgb(core)));
        for k in 1..self.puffs {
            let seed = self.seed * 37 + k * 5;
            let n = roll * random_direction(seed);
            let p = center + n * radius * (0.5 + 0.35 * hash(seed + 1));
            let size = radius * (0.3 + 0.25 * hash(seed + 2));
            let color = light.steam(p, n) * (shade * (0.85 + 0.3 * hash(seed + 3)));
            out.push(Particle::new(p, size, Rgb(color)));
        }
    }
}

/// The steam of the launch, the exhaust behind the rocket, and the vapor before it.
fn clouds() -> Vec<Cloud> {
    let mut clouds = Vec::new();
    let steam = |start, origin, velocity, h: &dyn Fn(u32) -> f32, seed| Cloud {
        start,
        origin,
        velocity,
        drag: 0.8 + 0.5 * h(10),
        lift: 1.0 + 3.0 * h(11),
        radius: 3.0 + 2.0 * h(12),
        growth: 6.0 + 6.0 * h(13),
        puffs: 14,
        albedo: 0.85 + 0.15 * h(14),
        life: f32::MAX,
        seed,
    };
    // the steam that rolls out of both mouths of the flame trench
    for i in 0..300u32 {
        let h = |k: u32| hash(i * 23 + k);
        let side = if i % 2 == 0 { 1.0 } else { -1.0 };
        let start = IGNITION + 0.05 + 2.9 * (i as f32 / 300.0).powf(1.3) + 0.02 * h(1);
        clouds.push(steam(
            start,
            vec3(side * MOUTH_X, 2.6, (h(2) - 0.5) * 16.0),
            vec3(
                side * (40.0 + 50.0 * h(3)),
                2.0 + 9.0 * h(4),
                (h(5) - 0.5) * 40.0,
            ),
            &h,
            i,
        ));
    }
    // the steam of the deluge water around the mount, and the exhaust that spreads over the
    // deck while the rocket is low
    for i in 0..160u32 {
        let h = |k: u32| hash(i * 29 + 50_000 + k);
        let a = h(1) * TAU;
        let out = vec3(a.cos(), 0.0, a.sin());
        let start = IGNITION + 0.1 + 2.4 * (i as f32 / 160.0);
        let mut cloud = steam(
            start,
            out * (6.0 + 4.0 * h(2)) + vec3(0.0, PAD_Y + 2.0, 0.0),
            out * (14.0 + 22.0 * h(3)) + vec3(0.0, 6.0 + 10.0 * h(4), 0.0),
            &h,
            i + 10_000,
        );
        cloud.radius *= 0.8;
        clouds.push(cloud);
    }
    // the column of exhaust that the rocket leaves behind it
    for i in 0..380u32 {
        let h = |k: u32| hash(i * 31 + 90_000 + k);
        let start = LIFTOFF + 0.12 + 2.6 * (i as f32 / 380.0);
        clouds.push(Cloud {
            start,
            origin: vec3(
                (h(1) - 0.5) * 4.0,
                (BASE_Y + altitude(start) - 34.0).max(PAD_Y + 3.0),
                (h(2) - 0.5) * 4.0,
            ),
            velocity: vec3(
                (h(3) - 0.5) * 12.0,
                -18.0 - 22.0 * h(4),
                (h(5) - 0.5) * 12.0,
            ),
            drag: 0.45,
            lift: 1.5,
            radius: 2.0 + 1.5 * h(6),
            growth: 4.0 + 3.0 * h(7),
            puffs: 5,
            albedo: 0.8,
            life: f32::MAX,
            seed: i + 20_000,
        });
    }
    // cold vapor that jets from the vents of the tanks to +X, where the wind blows, and
    // sinks along the rocket. It starts when the tanks fill, and the vents close before
    // the lift off.
    for i in 0..480u32 {
        let h = |k: u32| hash(i * 37 + 130_000 + k);
        let port = if i % 3 == 0 { 51.5 } else { 40.8 };
        let a = 1.1 + (h(1) - 0.5) * 0.8;
        let out = vec3(a.sin(), 0.0, a.cos());
        clouds.push(Cloud {
            start: FUELING.0 + 0.1 + 0.55 * (i as f32 / 480.0),
            origin: out * (BODY + 0.1) + vec3(0.0, BASE_Y + port, 0.0),
            velocity: out * (10.0 + 8.0 * h(2)) + vec3(3.0, -1.0 - 2.0 * h(3), 0.0),
            drag: 0.7,
            lift: -7.0,
            radius: 0.2 + 0.15 * h(4),
            growth: 1.5,
            puffs: 6,
            albedo: 1.3,
            life: 1.0,
            seed: i + 40_000,
        });
    }
    // cold vapor from the umbilical at the foot of the rocket rolls over the deck while
    // the tanks fill
    for i in 0..140u32 {
        let h = |k: u32| hash(i * 43 + 190_000 + k);
        let a = (h(1) - 0.5) * 2.4;
        clouds.push(Cloud {
            start: FUELING.0 + (IGNITION - 0.15 - FUELING.0) * (i as f32 / 140.0),
            origin: vec3(BODY + 1.0, PAD_Y + 1.5, (h(2) - 0.5) * 2.0),
            velocity: vec3(a.cos(), 0.0, a.sin()) * (5.0 + 6.0 * h(3)),
            drag: 0.7,
            lift: -0.5,
            radius: 1.0 + 0.6 * h(4),
            growth: 4.0,
            puffs: 6,
            albedo: 1.1,
            life: 1.4,
            seed: i + 80_000,
        });
    }
    // the relief valve on the top of the sphere of oxygen vents while it fills the rocket
    for i in 0..60u32 {
        let h = |k: u32| hash(i * 41 + 170_000 + k);
        clouds.push(Cloud {
            start: FUELING.0 - 0.1 + 0.8 * (i as f32 / 60.0),
            origin: vec3(-66.0, 20.4, -46.0),
            velocity: vec3(1.5 + 2.0 * h(1), 6.0 + 4.0 * h(2), (h(3) - 0.5) * 3.0),
            drag: 0.5,
            lift: -3.0,
            radius: 0.4 + 0.3 * h(4),
            growth: 2.4,
            puffs: 4,
            albedo: 1.6,
            life: 1.1,
            seed: i + 60_000,
        });
    }
    clouds
}

/// The thin moving members of a frame: the hoist lines and the pendants of the crane, its
/// load, and the rams of the transporter erector. They are instances of a unit box that
/// stands on Y = 0..1 (see [`span`]), each with a paint.
#[derive(Default)]
struct Rigging {
    steel: Vec<Mat4>,
    paint: Vec<Rgb>,
}

impl Rigging {
    fn member(&mut self, a: Vec3, b: Vec3, thickness: f32, paint: u32) {
        self.steel.push(span(a, b, thickness));
        self.paint.push(Rgb::hex(paint));
    }
}

/// The height of the rails of the transporter erector at `x`: on the deck of the pad and
/// on the bridge over the trench, then down the ramp to the ground.
fn rail_height(x: f32) -> f32 {
    PAD_Y * (1.0 - progress(x, RAMP.0, RAMP.1))
}

/// The arcs of the water deluge at `t`: four nozzles on the corners of the deck spray onto
/// the mount.
fn deluge(t: f32, out: &mut Vec<Particle>) {
    let age = t - DELUGE;
    if age <= 0.0 {
        return;
    }
    let flight = 0.9;
    for (n, (x, z)) in [
        (-15.0f32, 13.0f32),
        (15.0, 13.0),
        (-15.0, -13.0),
        (15.0, -13.0),
    ]
    .into_iter()
    .enumerate()
    {
        let nozzle = vec3(x, PAD_Y + 2.2, z);
        let to = vec3(x * 0.15, PAD_Y + 2.5, z * 0.15) - nozzle;
        let velocity = to / flight + Vec3::Y * (4.9 * flight);
        for k in 0..48u32 {
            let seed = n as u32 * 100 + k;
            let s = (k as f32 + hash(seed)) / 48.0 * flight;
            // the jet reaches out over the first moments
            if s > age * 2.5 {
                break;
            }
            let spread = (random_direction(seed + 7) + Vec3::Y * 0.3) * (2.0 * s);
            let p = nozzle + velocity * s + spread - Vec3::Y * (4.9 * s * s);
            let tail = -(velocity - Vec3::Y * (9.81 * s)) * 0.04;
            out.push(Particle::new(p, 0.15 + 0.4 * s, Rgb::hex(0xd8e2f0) * 0.6).tail(tail));
        }
    }
}

/// The node ids that the frames animate.
struct Ids {
    rocket: NodeId,
    flame: [NodeId; 2],
    flame_light: NodeId,
    mouth_lights: [NodeId; 2],
    mouths: NodeId,
    tower_lamps: NodeId,
    tower_lights: [NodeId; 2],
    line_light: NodeId,
    smoke: NodeId,
    sparks: NodeId,
    /// The frost on the oxygen tank.
    frost: NodeId,
    /// The groups of the transporter erector: its chassis and its strongback.
    chassis: NodeId,
    strongback: NodeId,
    /// The crawler crane and its boom.
    crane: NodeId,
    boom: NodeId,
    rigging: NodeId,
}

/// The launch pad, the rocket and its tower at dawn.
pub struct Rocket {
    scene: Scene,
    ids: Ids,
    tower: Tower,
    /// The work lamps on the chassis of the transporter erector.
    erector_lamps: Vec<Vec3>,
    clouds: Vec<Cloud>,
    landmark: Scene,
}

impl Rocket {
    pub fn new(assets: &Assets) -> Self {
        let (scene, ids, tower, erector_lamps) = build(assets);
        let landmark = contrail(&scene, &ids, &tower.lamps);
        Self {
            scene,
            ids,
            tower,
            erector_lamps,
            clouds: clouds(),
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
            air.steps = haze_steps(t);
        }
        let mut sparks = Vec::new();
        let mut rigging = Rigging::default();

        // the spark of the greenhouse flies to the foot of the tower and runs up its
        // corner. The crane builds the tower one level above it, and the lamps of each
        // level come on behind it.
        let head = mix(
            PAD_Y + 1.0,
            TOWER_TOP,
            ease(progress(t, LIGHTS.0, LIGHTS.1)),
        );
        let built = (head + LEVEL_HEIGHT).min(TOWER_TOP);
        self.pose_tower(&mut scene, t, built);
        if t > LIGHTS.0 - 0.1 && t < LIGHTS.1 {
            // welders join the new level at two corners: cold arcs and falling sparks
            let frame = (t * 60.0) as u32;
            for (k, (x, z)) in [(-1.0f32, 1.0f32), (1.0, -1.0)].into_iter().enumerate() {
                let p = vec3(
                    TOWER_X + x * TOWER_HALF,
                    built - LEVEL_HEIGHT,
                    z * TOWER_HALF,
                );
                let flicker = 0.4 + 0.6 * hash(frame * 7 + k as u32);
                sparks.push(Particle::new(p, 0.3, Rgb::hex(0xe8f0ff) * (50.0 * flicker)));
                sparks.push(Particle::new(p, 1.5, Rgb::hex(COLD_BLUE) * (1.5 * flicker)));
                for i in 0..12u32 {
                    let seed = k as u32 * 100 + i;
                    let age = (t / 0.4 + hash(seed)).fract() * 0.4;
                    let mut dir = random_direction(seed + 7);
                    dir.y = dir.y.abs() * 0.3;
                    let velocity = dir * (6.0 + 6.0 * hash(seed + 3));
                    let at = p + velocity * age - Vec3::Y * (60.0 * age * age);
                    let now = velocity - Vec3::Y * (120.0 * age);
                    let fade = 1.0 - age / 0.4;
                    sparks.push(
                        Particle::new(at, 0.07, Rgb::hex(0xffb060) * (40.0 * fade))
                            .tail(-now * 0.02),
                    );
                }
            }
        }
        self.pose_crane(&mut scene, t, built, &mut rigging, &mut sparks);
        let lit: Vec<Mat4> = self
            .tower
            .lamps
            .iter()
            .filter(|p| t > LIGHTS.0 && p.y <= head + 0.5)
            .map(|p| Mat4::from_translation(*p))
            .collect();
        if let Some(mesh) = scene[ids.tower_lamps].mesh_mut() {
            mesh.instances = Some(Arc::new(Instances::new(lit)));
        }
        let climbing = t < LIGHTS.1 + 0.15;
        if climbing {
            sparks.extend(spark(spark_at, t, 1.0));
            scene[ids.line_light].position = spark_at(t) + vec3(0.8, 0.0, 0.8);
        }
        scene[ids.line_light].visible = climbing && t > 0.1;
        // a flash at the top when the line arrives, then the red lights of the top blink
        let top_flash = (-(t - LIGHTS.1).max(0.0) / 0.25).exp() * smoothstep(1.3, 1.45, t);
        if top_flash > 0.01 {
            sparks.push(Particle::new(
                vec3(FACE_X + 1.0, TOWER_TOP, TOWER_HALF + 1.0),
                3.0,
                Rgb::hex(AMBER) * (8.0 * top_flash),
            ));
        }
        if t > LIGHTS.1 && (t * 1.2).fract() < 0.5 {
            for p in [
                vec3(TOWER_X, TOWER_TOP + 25.0, 0.0),
                vec3(TOWER_X + 22.0, TOWER_TOP + 4.5, 0.0),
                vec3(86.0, 63.0, -64.0),
            ] {
                sparks.push(Particle::new(p, 0.5, Rgb::hex(0xff2010) * 40.0));
            }
        }
        let lamps_on = smoothstep(LIGHTS.0 + 0.2, LIGHTS.1, t);
        for id in ids.tower_lights {
            if let Some(light) = scene[id].light_mut() {
                *light.intensity_mut() *= lamps_on;
            }
        }

        // the arms stand folded on the tower until the rocket stands. They swing to it,
        // and away again: the crew arm and the vent hood before the launch, the umbilicals
        // at the lift off.
        let tower = &self.tower;
        let arms_in = ease(progress(t, ARMS_IN.0, ARMS_IN.1));
        let crew = (1.0 - arms_in).max(ease(progress(t, ARM.0, ARM.1)));
        scene[tower.crew_arm].rotation = Quat::from_rotation_y(-1.4 * crew);
        let hood = (1.0 - arms_in).max(ease(progress(t, HOOD.0, HOOD.1)));
        scene[tower.hood].position.y += 2.5 * smoothstep(0.0, 0.3, hood);
        scene[tower.hood].rotation = Quat::from_rotation_y(-1.5 * smoothstep(0.2, 1.0, hood));
        let release = (1.0 - arms_in).max(ease(progress(t, LIFTOFF - 0.05, LIFTOFF + 0.45)));
        for (i, id) in tower.umbilicals.into_iter().enumerate() {
            scene[id].rotation = Quat::from_rotation_y((1.1 + 0.2 * i as f32) * release);
        }
        for id in [
            tower.crew_arm,
            tower.hood,
            tower.umbilicals[0],
            tower.umbilicals[1],
        ] {
            let node = &mut scene[id];
            node.visible = built > node.position.y - 2.0;
        }
        // the blinking light of the crew arm while it moves
        if t > ARMS_IN.0 - 0.1 && t < ARM.1 + 0.3 {
            let blink = if (t * 4.0).fract() < 0.5 { 30.0 } else { 4.0 };
            let tip = scene.local_to_world(tower.crew_arm, vec3(5.0, 3.4, 1.4));
            sparks.push(Particle::new(tip, 0.18, Rgb::hex(COLD_BLUE) * blink));
        }
        // the tanks fill from the bottom, and the frost on the oxygen tank grows with them
        let fill = ease(progress(t, FUELING.0, FUELING.1));
        let frost = &mut scene[ids.frost];
        frost.visible = fill > 0.01;
        frost.scale.y = fill.max(0.01);
        frost.position.y = FROST.0 * (1.0 - frost.scale.y);

        // the rocket, its flame and the light of the flame
        let rocket_y = BASE_Y + altitude(t);
        scene[ids.rocket].position.y = rocket_y;
        self.pose_erector(&mut scene, t, &mut rigging, &mut sparks);
        if let Some(mesh) = scene[ids.rigging].mesh_mut() {
            let instances = Instances::new(rigging.steel).colors(rigging.paint);
            mesh.instances = Some(Arc::new(instances));
        }
        let power = thrust(t);
        // on the deck the flame goes down into the trench; in the air it grows long
        let reach = ((rocket_y - PAD_Y) / 30.0).clamp(0.15, 1.0)
            * (1.0 + 0.25 * smoothstep(30.0, 120.0, rocket_y))
            * (1.0 + ease_in(progress(t, FLARE, 5.0)));
        for (i, id) in ids.flame.into_iter().enumerate() {
            let flame = &mut scene[id];
            flame.visible = power > 0.0;
            flame.position.y = rocket_y;
            let flicker = 1.0 + 0.07 * noise1(50 + i as u32, t * 40.0);
            flame.scale = vec3(flicker, reach * power.clamp(0.05, 1.0) * flicker, flicker);
        }
        let flame_at = vec3(0.0, (rocket_y - 18.0 * reach).max(PAD_Y + 1.0), 0.0);
        scene[ids.flame_light].visible = power > 0.0;
        scene[ids.flame_light].position = flame_at;
        if let Some(light) = scene[ids.flame_light].light_mut() {
            *light.intensity_mut() = 9.0e3 * power.min(2.0);
        }
        // the fire leaves the trench while the rocket is low
        let trench = power.min(1.0) * (1.0 - smoothstep(25.0, 80.0, rocket_y));
        if let Some(m) = scene[ids.mouths].material_mut() {
            m.color = Rgb::hex(0x050505).lerp(Rgb::hex(0xffa860) * 14.0, trench);
        }
        for id in ids.mouth_lights {
            scene[id].visible = trench > 0.0;
            if let Some(light) = scene[id].light_mut() {
                *light.intensity_mut() *= trench;
            }
        }

        let flare = ease_in(progress(t, FLARE, 5.0));
        if power > 0.0 {
            // the shock diamonds in the exhaust under the engines
            for k in 0..5 {
                let y = rocket_y - 2.4 - k as f32 * 2.9 * reach.min(1.0);
                if y < PAD_Y + 0.5 {
                    break;
                }
                let fade = (1.0 - k as f32 / 5.0) * power.min(1.5);
                sparks.push(
                    Particle::new(vec3(0.0, y, 0.0), 0.6, Rgb::hex(0xfff8ec) * (32.0 * fade))
                        .tail(Vec3::Y * -1.0),
                );
            }
            // the turbulent body of the flame: blobs of light that flicker and stream down
            for k in 0..28u32 {
                let s = (k as f32 + (t * 9.0 + hash(k)).fract()) / 28.0;
                let wobble = vec3(noise1(k, t * 14.0), 0.0, noise1(k + 99, t * 14.0)) * (1.6 * s);
                let p = vec3(0.0, rocket_y - 2.0 - s * 36.0 * reach, 0.0) + wobble;
                if p.y < PAD_Y {
                    continue;
                }
                let color = Rgb::hex(0xffc888).lerp(Rgb::hex(AMBER_EDGE), s)
                    * (0.7 * (1.0 - s).powf(1.2) * power.min(1.5) * (0.6 + 0.8 * hash(k + 7)));
                sparks.push(Particle::new(p, 2.2 + 4.5 * s, color));
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
            glow(vec3(0.0, PAD_Y + 3.0, 0.0), 40.0, 1.4 * flash);
            glow(
                vec3(0.0, rocket_y - 3.0, 0.0),
                7.0 + 130.0 * flare,
                0.3 + 5.0 * flare,
            );
            // fire out of both mouths of the trench before the steam covers it
            if trench > 0.0 {
                for i in 0..80u32 {
                    let side = if i % 2 == 0 { 1.0 } else { -1.0 };
                    let age = (t - IGNITION + 0.4 * hash(i)).rem_euclid(0.4);
                    let dir = vec3(side, 0.15 * hash(i + 3), 0.5 * (hash(i + 5) - 0.5));
                    let p = vec3(side * MOUTH_X, 2.5, 6.0 * (hash(i + 7) - 0.5)) + dir * age * 70.0;
                    let fire = trench * (1.0 - age / 0.4) * smoothstep(IGNITION, IGNITION + 0.1, t);
                    sparks.push(Particle::new(
                        p,
                        2.0 + 8.0 * age,
                        Rgb::hex(0xffa050) * (3.0 * fire),
                    ));
                }
            }
        }
        // the igniters: a short green flash, and sparks just before the engines start
        if t > IGNITION - 0.2 && t < IGNITION + 0.3 {
            let green = (1.0 - ((t - IGNITION + 0.05) / 0.08).abs()).max(0.0);
            if green > 0.0 {
                sparks.push(Particle::new(
                    vec3(0.0, BASE_Y - 1.5, 0.0),
                    6.0,
                    Rgb::hex(0x7dff9a) * (6.0 * green),
                ));
            }
            for i in 0..70u32 {
                let age = (t - IGNITION + 0.2 - 0.5 * hash(i * 3)).rem_euclid(0.5);
                let dir = vec3(
                    hash(i * 3 + 1) - 0.5,
                    0.2 * hash(i * 5),
                    hash(i * 3 + 2) - 0.5,
                )
                .normalize();
                let p = vec3(0.0, BASE_Y - 1.0, 0.0) + dir * age * 50.0;
                sparks.push(Particle::new(p, 0.12, Rgb::hex(0xffc070) * 40.0).tail(-dir * 2.5));
            }
        }
        if let Object::Particles(p) = &mut scene[ids.sparks].object {
            p.items = sparks;
        }

        // steam, smoke, vapor and water, lit by the flame and the sky, drawn from back to
        // front
        let fire = Rgb::hex(0xffb878).0;
        let light = Lighting {
            fires: vec![
                (flame_at, fire * (4000.0 * power.min(1.6))),
                (
                    vec3(0.0, (rocket_y - 4.0).max(PAD_Y + 1.0), 0.0),
                    fire * (4000.0 * power.min(1.6)),
                ),
                (vec3(MOUTH_X + 6.0, 4.0, 0.0), fire * (2500.0 * trench)),
                (vec3(-MOUTH_X - 6.0, 4.0, 0.0), fire * (2500.0 * trench)),
            ],
            glow: vec3(GLOW.cos(), 0.15, GLOW.sin()).normalize(),
        };
        let mut smoke = Vec::new();
        for cloud in &self.clouds {
            cloud.puffs(t, &light, &mut smoke);
        }
        deluge(t, &mut smoke);
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

    /// Builds the tower up to the height `built`: the levels below it stand, the elevator
    /// shaft and the pipes grow with it, and the parts of the top come when it is done.
    fn pose_tower(&self, scene: &mut Scene, t: f32, built: f32) {
        let tower = &self.tower;
        for (id, parts) in
            tower
                .nodes
                .into_iter()
                .zip([&tower.members, &tower.floors, &tower.rails])
        {
            let standing = parts.iter().filter(|p| p.0 <= built + 0.1).map(|p| p.1);
            if let Some(mesh) = scene[id].mesh_mut() {
                mesh.instances = Some(Arc::new(Instances::new(standing)));
            }
        }
        for id in tower.grow {
            scene[id].scale.y = ((built - PAD_Y) / (TOWER_TOP - PAD_Y)).max(0.01);
        }
        scene[tower.top].visible = t > LIGHTS.1 - 0.03;
    }

    /// The crawler crane beside the tower. It swings the levels of the tower up to its
    /// top, then lowers its boom and drives away before the launch.
    fn pose_crane(
        &self,
        scene: &mut Scene,
        t: f32,
        built: f32,
        rigging: &mut Rigging,
        sparks: &mut Vec<Particle>,
    ) {
        let ids = &self.ids;
        let to_tower = vec3(TOWER_X, 0.0, 0.0) - CRANE_AT;
        let building = t < LIGHTS.1;
        // the boom swings between the pick on the ground and the top of the tower
        let swing = if building {
            0.5 - 0.5 * (t * 13.0).cos()
        } else {
            0.0
        };
        // when the tower stands, the boom turns away from the pad before it comes down
        let back = ease(progress(t, CRANE_AWAY.0, CRANE_AWAY.0 + 0.2));
        let slew = (-to_tower.z).atan2(to_tower.x) + 0.35 * swing - 2.0 * back;
        let lower = ease(progress(t, CRANE_AWAY.0 + 0.08, CRANE_AWAY.0 + 0.3));
        let away = ease_in(progress(t, CRANE_AWAY.0 + 0.1, CRANE_AWAY.1));
        let crane = &mut scene[ids.crane];
        crane.visible = away < 1.0;
        crane.position = CRANE_AT + vec3(-230.0, 0.0, 30.0) * away;
        crane.rotation = Quat::from_rotation_y(slew);
        scene[ids.boom].rotation = Quat::from_rotation_z(mix(1.1, 0.45, lower));
        if away >= 1.0 {
            return;
        }
        let tip = scene.local_to_world(ids.boom, vec3(BOOM, -0.8, 0.0));
        for z in [-0.6, 0.6] {
            let gantry =
                scene.local_to_world(ids.crane, machines::CRAWLER_GANTRY + vec3(0.0, 0.0, z));
            rigging.member(gantry, tip, 0.1, DARK);
        }
        // the hook: down at the pick when the boom swings out, and with a level of the
        // tower over the top
        let hook_y = if building {
            mix(built + 9.0, 3.0, swing)
        } else {
            tip.y - 6.0
        };
        let hook = vec3(tip.x, hook_y, tip.z);
        rigging.member(hook, tip, 0.09, DARK);
        if building {
            let (half, top) = (TOWER_HALF - 0.4, hook - Vec3::Y * 1.2);
            let corners = [(-1.0f32, -1.0f32), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)]
                .map(|(x, z)| top + vec3(x * half, 0.0, z * half));
            for k in 0..4 {
                let (a, b) = (corners[k], corners[(k + 1) % 4]);
                rigging.member(a, hook, 0.06, DARK);
                rigging.member(a - Vec3::Y * LEVEL_HEIGHT, a, 0.8, 0x7a7e84);
                rigging.member(a, b, 0.5, 0x7a7e84);
                let low = Vec3::Y * LEVEL_HEIGHT;
                rigging.member(a - low, b - low, 0.4, 0x7a7e84);
            }
        }
        let blink = if (t * 1.4).fract() < 0.5 { 40.0 } else { 5.0 };
        sparks.push(Particle::new(
            tip + Vec3::Y * 1.2,
            0.6,
            Rgb::hex(0xff2010) * blink,
        ));
        sparks.push(Particle::new(tip, 0.5, Rgb::hex(COLD) * 30.0));
        let cab = scene.local_to_world(ids.crane, vec3(6.6, 5.4, 3.2));
        sparks.push(Particle::new(cab, 0.6, Rgb::hex(COLD) * 20.0));
    }

    /// The transporter erector: it rolls the rocket from the hangar down the ramp and
    /// over the trench to the pad, raises it, and tilts its strongback back. Until the
    /// rocket stands, the rocket rides on it.
    fn pose_erector(
        &self,
        scene: &mut Scene,
        t: f32,
        rigging: &mut Rigging,
        sparks: &mut Vec<Particle>,
    ) {
        let ids = &self.ids;
        let x = mix(
            HANGAR_X,
            PIVOT.x,
            ease_out(progress(t, ROLLOUT.0, ROLLOUT.1)),
        );
        let (front, back) = (rail_height(x), rail_height(x + CHASSIS));
        let pitch = Quat::from_rotation_z(((back - front) / CHASSIS).atan());
        let pivot = vec3(x, front + PIVOT.y - PAD_Y, 0.0);
        let raise = -FRAC_PI_2 * (1.0 - ease(progress(t, RAISE.0, RAISE.1)));
        let tilt = if t < RAISE.1 {
            raise
        } else {
            -0.2 * ease(progress(t, STRONGBACK.0, STRONGBACK.1))
        };
        scene[ids.chassis].position = pivot;
        scene[ids.chassis].rotation = pitch;
        scene[ids.strongback].position = pivot;
        scene[ids.strongback].rotation = pitch * Quat::from_rotation_z(tilt);
        if t < RAISE.1 {
            let turn = pitch * Quat::from_rotation_z(raise);
            let rocket = &mut scene[ids.rocket];
            rocket.rotation = turn;
            rocket.position = pivot + turn * (vec3(0.0, BASE_Y, 0.0) - PIVOT);
        }
        // the rods of the two rams from the chassis to the strongback
        for z in [-2.2, 2.2] {
            let from = scene.local_to_world(ids.chassis, vec3(14.0, -1.0, z));
            let to = scene.local_to_world(ids.strongback, vec3(1.0, 16.0, z));
            rigging.member(from, to, 0.45, 0xb8bcc0);
        }
        for p in &self.erector_lamps {
            let at = scene.local_to_world(ids.chassis, *p);
            sparks.push(Particle::new(at, 0.35, Rgb::hex(COLD) * 25.0));
        }
        if t < ROLLOUT.1 {
            let blink = if (t * 3.0).fract() < 0.5 { 30.0 } else { 3.0 };
            let beacon = scene.local_to_world(ids.chassis, vec3(CHASSIS, 2.1, 0.0));
            sparks.push(Particle::new(beacon, 0.4, Rgb::hex(0xffa020) * blink));
        }
    }
}

/// Texture coordinates in meters from the positions: each triangle takes the plane that
/// it faces most, so tiled textures keep their size on large shapes.
fn box_uvs(mut g: Geometry) -> Geometry {
    g.uvs = g
        .positions
        .iter()
        .zip(&g.normals)
        .map(|(p, n)| {
            let a = n.abs();
            if a.y >= a.x && a.y >= a.z {
                vec2(p.x, p.z)
            } else if a.x >= a.z {
                vec2(p.z, p.y)
            } else {
                vec2(p.x, p.y)
            }
        })
        .collect();
    g
}

/// The scene template. Returns the tower and the work lamps of the transporter erector
/// too.
fn build(assets: &Assets) -> (Scene, Ids, Tower, Vec<Vec3>) {
    let mut scene = Scene::new()
        .environment(Environment::map(
            sky(
                Rgb::hex(0x1a3263),
                Rgb::hex(0x9a8eb4) * 0.6,
                Rgb::hex(0xffa050) * 2.0,
                GLOW,
            ),
            1.0,
        ))
        .background(Background::Environment { blur: 0.0 });

    let concrete = assets.concrete.material(0xbab6ae, 0.9, 0.0);
    let steel = assets.steel.material(0x7a7e84, 0.7, 0.6);
    let dark = Material::standard(0x1b1c1f).roughness(0.55);
    let white = Material::standard(0xe8e6e0).roughness(0.5);

    // the dry lake bed, the concrete apron, the road, and the mountains far away
    scene.add(
        Node::mesh(
            Geometry::floor(9000.0, 9000.0),
            assets.dirt.material(0xc8b8a8, 1.0, 0.0),
        )
        .named("ground"),
    );
    scene.add(
        Node::mesh(
            Geometry::floor(200.0, 170.0),
            assets.concrete.material(0xa8a49c, 0.95, 0.0),
        )
        .at(0.0, 0.03, 8.0)
        .named("ground"),
    );
    scene.add(
        Node::mesh(
            Geometry::floor(11.0, 260.0),
            assets.asphalt.material(0x7a7a7a, 0.9, 0.0),
        )
        .at(26.0, 0.05, 210.0)
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
    // stones around the camera
    let mut stones = Vec::new();
    for i in 0..500u32 {
        let r = 10.0 + 140.0 * hash(i * 3 + 1).powi(2);
        let a = hash(i * 3 + 2) * TAU;
        let p = CAMERA + vec3(r * a.sin(), 0.0, r * a.cos());
        if p.z < 100.0 || (p.x - 26.0).abs() < 7.0 {
            continue;
        }
        let size = 0.12 + 0.5 * hash(i * 3 + 3).powi(3);
        stones.push(Mat4::from_scale_rotation_translation(
            vec3(size * 1.4, size * 0.7, size),
            Quat::from_rotation_y(hash(i) * TAU),
            vec3(p.x, size * 0.2, p.z),
        ));
    }
    scene.add(
        Node::instanced(
            Geometry::icosahedron(1.0, 1).flat(),
            assets.rock.material(0x9a8070, 1.0, 0.0),
            Instances::new(stones),
        )
        .named("ground"),
    );

    let mouths = pad(&mut scene, &concrete, &steel);
    let tower = tower(&mut scene, assets, &steel, &dark, &white);
    let tower_lamps = scene.add(
        Node::instanced(
            Geometry::cuboid(0.55, 0.4, 0.55),
            Material::unlit(Rgb::hex(AMBER) * 14.0),
            Instances::default(),
        )
        .shadows(false, false),
    );
    // the lamps light the rocket warm from the tower side
    let tower_lights = [24.0f32, 58.0].map(|y| {
        scene.add(Node::light(Light::point(AMBER, 300.0, 50.0)).at(TOWER_X, y, TOWER_HALF + 3.0))
    });
    let line_light = scene.add(Node::light(Light::point(AMBER, 900.0, 40.0)).hidden());
    site(&mut scene, assets, &concrete, &steel, &white);

    let rocket = scene.add(Node::group().at(0.0, BASE_Y, 0.0));
    let frost = vehicle(&mut scene, rocket);

    // the machines of the time-lapse. The landmark of shot 9 leaves them out.
    let erector = machines::erector(PIVOT.x, BODY, PIVOT.y - PAD_Y, STRONGBACK_LENGTH);
    let chassis = scene.add(Node::group().with_position(PIVOT).named("surroundings"));
    scene.add_to(
        chassis,
        Node::mesh(
            erector.chassis,
            Material::standard(0xc8c6c0).roughness(0.55),
        ),
    );
    scene.add_to(chassis, Node::mesh(erector.wheels, dark.clone()));
    let strongback = scene.add(Node::group().with_position(PIVOT).named("surroundings"));
    scene.add_to(
        strongback,
        Node::mesh(
            erector.strongback,
            Material::standard(0xdedbd4).roughness(0.5),
        ),
    );
    let crawler = machines::crawler(BOOM);
    let yellow = Material::standard(YELLOW).roughness(0.55);
    let crane = scene.add(Node::group().with_position(CRANE_AT).named("surroundings"));
    scene.add_to(crane, Node::mesh(crawler.body, yellow.clone()));
    let boom = scene.add_to(crane, Node::group().with_position(machines::CRAWLER_FOOT));
    scene.add_to(boom, Node::mesh(crawler.boom, yellow));
    let rigging = scene.add(
        Node::instanced(
            Geometry::cuboid(1.0, 1.0, 1.0).translate(0.0, 0.5, 0.0),
            Material::standard(0xffffff).roughness(0.6),
            Instances::default(),
        )
        .shadows(false, false)
        .named("surroundings"),
    );

    // the flame: a white core and a slim orange body inside the soft particles of the
    // frames, scaled along Y by the frames
    let flame_shape = |points: &[(f32, f32)]| {
        let points: Vec<Vec2> = points.iter().map(|&(r, y)| vec2(r, y)).collect();
        Geometry::lathe(&points, 32)
    };
    let flame = [
        (
            flame_shape(&[
                (0.0, 0.2),
                (1.3, -0.4),
                (1.4, -2.5),
                (0.9, -8.0),
                (0.4, -13.0),
                (0.0, -16.0),
            ]),
            Material::glow(0xffe8c4, 0.9),
        ),
        (
            flame_shape(&[
                (0.0, 0.3),
                (1.8, -0.8),
                (2.2, -6.0),
                (1.8, -18.0),
                (1.0, -30.0),
                (0.0, -38.0),
            ]),
            Material::glow(0xffa040, 0.4),
        ),
    ]
    .map(|(g, m)| {
        scene.add(
            Node::mesh(g, m.double_sided())
                .shadows(false, false)
                .hidden(),
        )
    });
    let flame_light = scene.add(Node::light(Light::point(0xffb070, 0.0, 600.0)).hidden());
    let mouth_lights = [-1.0f32, 1.0].map(|side| {
        scene.add(
            Node::light(Light::point(0xffa060, 1.2e4, 160.0))
                .at(side * (MOUTH_X + 10.0), 3.0, 0.0)
                .hidden(),
        )
    });

    let smoke = scene.add(Node::particles(Vec::new(), Blending::Normal));
    let sparks = scene.add(Node::particles(Vec::new(), Blending::Additive));

    // the glow of the sun below the horizon behind the pad, and the light of the sky
    scene.add(Node::light(
        Light::sun(0xff9f6a, 0.9, vec3(GLOW.cos(), 0.06, GLOW.sin())).with_shadow(Shadow {
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
    // The haze stops at 450 m: the steam does not write its depth, so the haze over a
    // cloud in front of the sky is the haze up to this distance.
    scene.atmosphere = Some(Atmosphere {
        density: 0.0014,
        color: Rgb::WHITE,
        base: 0.0,
        falloff: 0.018,
        anisotropy: 0.55,
        ambient: Rgb::hex(0x6a6a98) * 0.12,
        noise: 0.45,
        noise_size: 40.0,
        offset: Vec3::ZERO,
        distance: 450.0,
        steps: 32,
    });

    let ids = Ids {
        rocket,
        flame,
        flame_light,
        mouth_lights,
        mouths,
        tower_lamps,
        tower_lights,
        line_light,
        smoke,
        sparks,
        frost,
        chassis,
        strongback,
        crane,
        boom,
        rigging,
    };
    (scene, ids, tower, erector.lamps)
}

/// The pad: a raised concrete hardstand with the flame trench through it, the launch mount
/// on its deck, and the nozzles of the water deluge. Returns the node of the trench
/// mouths.
fn pad(scene: &mut Scene, concrete: &Material, steel: &Material) -> NodeId {
    // the hardstand: sloped on the front and the back, a wall at each mouth of the trench
    let profile = [
        vec2(-38.0, 0.0),
        vec2(38.0, 0.0),
        vec2(27.0, PAD_Y),
        vec2(-27.0, PAD_Y),
    ];
    let hardstand = Geometry::extrude_segments(&profile, 2.0 * MOUTH_X, 0.3, 1).rotate_y(FRAC_PI_2);
    scene.add(Node::mesh(box_uvs(hardstand), concrete.clone()));
    // the mouths of the trench and their steel frames
    let mouths = Geometry::merge([-1.0f32, 1.0].map(|side| {
        Geometry::plane(22.0, 4.6)
            .rotate_y(side * FRAC_PI_2)
            .translate(side * (MOUTH_X + 0.05), 2.4, 0.0)
    }));
    let mouths = scene.add(Node::mesh(mouths, Material::unlit(0x050505)).shadows(false, false));
    let mut frames = Vec::new();
    for side in [-1.0f32, 1.0] {
        let x = side * (MOUTH_X + 0.2);
        frames.push(Geometry::cuboid(0.5, 0.6, 23.0).translate(x, 4.9, 0.0));
        for z in [-11.3f32, 11.3] {
            frames.push(Geometry::cuboid(0.5, 5.0, 0.6).translate(x, 2.5, z));
        }
    }
    scene.add(Node::mesh(Geometry::merge(frames), steel.clone()));

    // the launch mount: four legs, a deck with a hole for the flame, the hold-down clamps
    let mut mount = Vec::new();
    for (x, z) in [(-5.5f32, -5.5f32), (5.5, -5.5), (-5.5, 5.5), (5.5, 5.5)] {
        mount.push(Geometry::cuboid(2.2, 3.0, 2.2).translate(x, PAD_Y + 1.5, z));
    }
    for (w, d, x, z) in [
        (16.0f32, 4.5f32, 0.0f32, 5.75f32),
        (16.0, 4.5, 0.0, -5.75),
        (4.5, 7.0, 5.75, 0.0),
        (4.5, 7.0, -5.75, 0.0),
    ] {
        mount.push(Geometry::cuboid(w, 1.0, d).translate(x, BASE_Y - 1.5, z));
    }
    for k in 0..4 {
        let a = k as f32 * FRAC_PI_2 + PI / 4.0;
        mount.push(Geometry::cuboid(0.8, 1.6, 0.6).rotate_y(-a).translate(
            2.3 * a.cos(),
            BASE_Y - 0.4,
            2.3 * a.sin(),
        ));
    }
    scene.add(Node::mesh(Geometry::merge(mount), steel.clone()));
    // the nozzles of the deluge on the corners of the deck
    let mut nozzles = Vec::new();
    for (x, z) in [
        (-15.0f32, 13.0f32),
        (15.0, 13.0),
        (-15.0, -13.0),
        (15.0, -13.0),
    ] {
        nozzles.push(Geometry::cylinder(0.25, 0.3, 2.0, 8).translate(x, PAD_Y + 1.0, z));
        nozzles.push(Geometry::cuboid(0.9, 0.5, 0.9).translate(x, PAD_Y + 2.1, z));
    }
    scene.add(Node::mesh(Geometry::merge(nozzles), steel.clone()));
    // a rail along the front and the back edges of the deck
    let mut rail = Vec::new();
    for z in [27.0f32, -27.0] {
        let (a, b) = (
            vec3(-MOUTH_X, PAD_Y + 1.1, z),
            vec3(MOUTH_X, PAD_Y + 1.1, z),
        );
        rail.push(span(a, b, 0.08));
        let posts = 40;
        for i in 0..=posts {
            let p = a.lerp(b, i as f32 / posts as f32);
            rail.push(span(p - Vec3::Y * 1.1, p, 0.08));
        }
    }
    scene.add(
        Node::instanced(
            Geometry::cylinder(1.0, 1.0, 1.0, 5).translate(0.0, 0.5, 0.0),
            Material::standard(0xc8a020).roughness(0.6),
            Instances::new(rail),
        )
        .shadows(false, false),
    );
    mouths
}

/// The service tower. The frames build it up level by level: they keep the parts below
/// the top of the build and hide the rest.
struct Tower {
    /// The steel members, the floors and the rails, each with the height of its top.
    members: Vec<(f32, Mat4)>,
    floors: Vec<(f32, Mat4)>,
    rails: Vec<(f32, Mat4)>,
    /// The instanced nodes of the members, the floors and the rails.
    nodes: [NodeId; 3],
    /// The nodes that grow from the deck with the tower: the elevator shaft and the pipes.
    grow: [NodeId; 2],
    /// The group of the parts that come last: the machine room, the hammerhead crane and
    /// the lightning mast.
    top: NodeId,
    crew_arm: NodeId,
    umbilicals: [NodeId; 2],
    hood: NodeId,
    /// The places of the lamps, from the bottom to the top.
    lamps: Vec<Vec3>,
}

/// The fixed service tower: a square steel lattice with a floor at each level, the
/// elevator, pipes, a hammerhead crane and a lightning mast on the top, and the arms to the
/// rocket.
fn tower(
    scene: &mut Scene,
    assets: &Assets,
    steel: &Material,
    dark: &Material,
    white: &Material,
) -> Tower {
    let unit = Arc::new(Geometry::cuboid(1.0, 1.0, 1.0).translate(0.0, 0.5, 0.0));
    let corners = [(-1.0f32, -1.0f32), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)]
        .map(|(x, z)| vec3(TOWER_X + x * TOWER_HALF, PAD_Y, z * TOWER_HALF));
    let at = |p: Vec3, y: f32| p + Vec3::Y * y;
    let top = TOWER_TOP - PAD_Y;
    let mut members = Vec::new();
    for l in 0..LEVELS {
        let (y0, y1) = (l as f32 * LEVEL_HEIGHT, (l + 1) as f32 * LEVEL_HEIGHT);
        let level = PAD_Y + y1;
        for k in 0..4 {
            let (a, b) = (corners[k], corners[(k + 1) % 4]);
            members.push((level, span(at(a, y0), at(a, y1), 0.9)));
            members.push((level, span(at(a, y1), at(b, y1), 0.5)));
            // K braces: from both lower corners to the middle of the upper beam
            let mid = at(a.lerp(b, 0.5), y1);
            members.push((level, span(at(a, y0), mid, 0.26)));
            members.push((level, span(at(b, y0), mid, 0.26)));
        }
    }
    let members_node = scene.add(Node::instanced(
        unit.clone(),
        steel.clone(),
        Instances::new(members.iter().map(|m| m.1)),
    ));

    // the parts of the top: the hammerhead crane with a mast and a jib over the pad, its
    // cab, the machine room and the lightning mast
    let crown = scene.add(Node::group());
    let crane_y = TOWER_TOP + 4.0;
    let mast = vec3(TOWER_X, TOWER_TOP, 0.0);
    let mut jib = vec![span(mast, mast + Vec3::Y * 5.0, 1.6)];
    for z in [-0.8f32, 0.8] {
        for (a, b) in [
            (
                vec3(TOWER_X - 10.0, crane_y, z),
                vec3(TOWER_X + 22.0, crane_y, z),
            ),
            (
                vec3(TOWER_X - 10.0, crane_y + 1.6, z),
                vec3(TOWER_X + 20.0, crane_y + 1.0, z),
            ),
        ] {
            jib.push(span(a, b, 0.3));
        }
        for i in 0..16 {
            let x = TOWER_X - 10.0 + i as f32 * 2.0;
            jib.push(span(
                vec3(x, crane_y, z),
                vec3(x + 2.0, crane_y + 1.5 - 0.04 * i as f32, z),
                0.14,
            ));
        }
    }
    scene.add_to(
        crown,
        Node::instanced(unit.clone(), steel.clone(), Instances::new(jib)),
    );
    let clad = assets.corrugated.material(0x8a8e94, 0.6, 0.5);
    scene.add_to(
        crown,
        Node::mesh(
            box_uvs(Geometry::cuboid(9.0, 4.0, 9.0).translate(TOWER_X, TOWER_TOP - 2.2, 0.0)),
            clad.clone(),
        ),
    );
    scene.add_to(
        crown,
        Node::mesh(
            Geometry::cylinder(0.12, 0.45, 20.0, 8).translate(TOWER_X, crane_y + 11.0, 0.0),
            Material::standard(0xd8d0b0).roughness(0.5),
        ),
    );
    scene.add_to(
        crown,
        Node::mesh(
            Geometry::cuboid(3.0, 2.2, 2.4).translate(TOWER_X + 2.5, crane_y - 0.9, 0.0),
            Material::standard(0xd8b020).roughness(0.5),
        ),
    );

    // a floor at each level with a walkway around the tower, and its rails
    let mut floors = Vec::new();
    let mut rails = Vec::new();
    let reach = TOWER_HALF + 1.3;
    for l in 1..=LEVELS {
        let y = PAD_Y + l as f32 * LEVEL_HEIGHT;
        floors.push((
            y,
            Mat4::from_scale_rotation_translation(
                vec3(2.0 * reach, 0.25, 2.0 * reach),
                Quat::IDENTITY,
                vec3(TOWER_X, y - 0.25, 0.0),
            ),
        ));
        let edge = [(-1.0f32, -1.0f32), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)]
            .map(|(x, z)| vec3(TOWER_X + x * reach, y + 1.0, z * reach));
        for k in 0..4 {
            rails.push((y, span(edge[k], edge[(k + 1) % 4], 0.07)));
        }
    }
    let floors_node = scene.add(Node::instanced(
        unit.clone(),
        Material::standard(0x3a3c40).roughness(0.8),
        Instances::new(floors.iter().map(|f| f.1)),
    ));
    let rails_node = scene.add(
        Node::instanced(
            Geometry::cylinder(1.0, 1.0, 1.0, 4).translate(0.0, 0.5, 0.0),
            Material::standard(0xc8a020).roughness(0.6),
            Instances::new(rails.iter().map(|r| r.1)),
        )
        .shadows(false, false),
    );
    // the elevator shaft on the back, clad in metal. It stands on the deck, so the frames
    // grow it with the tower.
    let shaft = top + 3.0;
    let shaft = scene.add(
        Node::mesh(
            box_uvs(Geometry::cuboid(3.6, shaft, 3.6).translate(
                TOWER_X - TOWER_HALF - 1.9,
                PAD_Y + shaft * 0.5,
                -1.5,
            ))
            .translate(0.0, -PAD_Y, 0.0),
            clad,
        )
        .at(0.0, PAD_Y, 0.0),
    );
    // pipes and a cable tray up the front of the tower, with clamps at each level
    let mut pipes = Vec::new();
    for (k, r) in [0.42f32, 0.32, 0.26].into_iter().enumerate() {
        let x = FACE_X - 0.8 - k as f32 * 1.1;
        let z = TOWER_HALF + 0.7;
        pipes.push(Geometry::cylinder(r, r, top - 12.0, 12).translate(x, (top - 12.0) * 0.5, z));
        for l in 0..LEVELS - 3 {
            pipes.push(Geometry::cuboid(r * 2.6, 0.3, r * 2.6).translate(
                x,
                2.0 + l as f32 * LEVEL_HEIGHT,
                z,
            ));
        }
    }
    pipes.push(Geometry::cuboid(0.7, top, 0.25).translate(
        TOWER_X - 1.5,
        top * 0.5,
        TOWER_HALF + 0.5,
    ));
    let pipes = scene.add(
        Node::mesh(
            Geometry::merge(pipes),
            Material::standard(0x9a9890).roughness(0.5),
        )
        .at(0.0, PAD_Y, 0.0),
    );

    // the umbilical arms: a lattice beam to the rocket, with hoses that hang to it
    let umbilicals = [BASE_Y + 36.0, BASE_Y + 48.5].map(|y| {
        let arm = scene.add(Node::group().at(FACE_X, y, -0.8));
        let length = -FACE_X - BODY - 0.2;
        let mut parts = Vec::new();
        for (dy, dz) in [(0.0f32, -0.6f32), (0.0, 0.6), (1.4, -0.6), (1.4, 0.6)] {
            parts.push(Geometry::cuboid(length, 0.22, 0.22).translate(length * 0.5, dy, dz));
        }
        for i in 0..6 {
            let x = i as f32 * length / 6.0;
            for dz in [-0.6f32, 0.6] {
                parts.push(Geometry::cuboid(0.14, 1.7, 0.14).rotate_z(-0.6).translate(
                    x + 0.5,
                    0.7,
                    dz,
                ));
            }
        }
        scene.add_to(arm, Node::mesh(Geometry::merge(parts), steel.clone()));
        let hoses = Geometry::merge([0.3f32, -0.3].map(|dz| {
            let path: Vec<Vec3> = (0..=12)
                .map(|i| {
                    let s = i as f32 / 12.0;
                    vec3(length * s, -1.2 * (s * PI).sin() - 0.3, dz)
                })
                .collect();
            Geometry::tube(&path, 0.16, 8)
        }));
        scene.add_to(arm, Node::mesh(hoses, dark.clone()));
        arm
    });

    // the crew access arm: a covered walkway to the white room at the hatch
    let crew_arm = scene.add(Node::group().at(FACE_X, BASE_Y + 54.0, 1.4));
    let length = -FACE_X - BODY - 2.4;
    let mut walk = vec![
        Geometry::cuboid(length, 0.4, 2.4).translate(length * 0.5, 0.0, 0.0),
        Geometry::cuboid(length, 0.25, 2.4).translate(length * 0.5, 3.0, 0.0),
    ];
    for dz in [-1.15f32, 1.15] {
        for dy in [0.2f32, 2.9] {
            walk.push(Geometry::cuboid(length, 0.2, 0.2).translate(length * 0.5, dy, dz));
        }
        for i in 0..4 {
            let x = i as f32 * length / 4.0;
            walk.push(Geometry::cuboid(0.14, 3.6, 0.14).rotate_z(-0.62).translate(
                x + 1.0,
                1.5,
                dz,
            ));
        }
    }
    scene.add_to(crew_arm, Node::mesh(Geometry::merge(walk), steel.clone()));
    scene.add_to(
        crew_arm,
        Node::mesh(
            Geometry::rounded_box(2.6, 3.4, 3.0, 0.15, 2).translate(length + 1.2, 1.5, 0.0),
            white.clone(),
        ),
    );

    // the vent hood on the nose, on an arm from the top of the tower
    let hood = scene.add(Node::group().at(FACE_X, BASE_Y + LENGTH + 1.2, 0.0));
    let reach = -FACE_X;
    scene.add_to(
        hood,
        Node::mesh(
            Geometry::merge([
                Geometry::cuboid(reach, 0.8, 1.0).translate(reach * 0.5, 1.6, 0.0),
                Geometry::cuboid(reach * 0.7, 0.3, 0.3)
                    .rotate_z(0.18)
                    .translate(reach * 0.35, 0.9, 0.0),
            ]),
            steel.clone(),
        ),
    );
    scene.add_to(
        hood,
        Node::mesh(
            Geometry::lathe(
                &[
                    vec2(2.4, -3.2),
                    vec2(2.5, -0.4),
                    vec2(2.0, 0.6),
                    vec2(0.6, 1.2),
                    vec2(0.0, 1.25),
                ],
                24,
            )
            .translate(reach, 0.0, 0.0),
            white.clone().double_sided(),
        ),
    );

    // the lamps: two lines up the front corners, one lamp at each level
    let lamps = (0..LEVELS)
        .flat_map(|l| {
            let y = PAD_Y + 2.6 + l as f32 * LEVEL_HEIGHT;
            [-1.0f32, 1.0]
                .map(|side| vec3(TOWER_X + side * (TOWER_HALF + 1.0), y, TOWER_HALF + 1.0))
        })
        .collect();
    Tower {
        members,
        floors,
        rails,
        nodes: [members_node, floors_node, rails_node],
        grow: [shaft, pipes],
        top: crown,
        crew_arm,
        umbilicals,
        hood,
        lamps,
    }
}

/// The rocket below the group node `rocket`, with its engines at Y = 0: a first stage with
/// frost on its oxygen tank, a black interstage with grid fins, the second stage and the
/// fairing. Returns the node of the frost.
fn vehicle(scene: &mut Scene, rocket: NodeId) -> NodeId {
    let paint = Material::standard(0xecebe6).roughness(0.32).clearcoat(0.6);
    let frost = Material::standard(0xdce8f6).roughness(1.0);
    let black = Material::standard(0x16171a).roughness(0.6);
    let soot = Material::standard(0x3a3430).roughness(0.85);
    let mut fairing = vec![
        vec2(BODY, 46.0),
        vec2(BODY, 53.2),
        vec2(2.3, 54.7),
        vec2(2.3, 60.5),
    ];
    for i in 1..=12 {
        let s = i as f32 / 12.0;
        fairing.push(vec2(
            2.3 * (1.0 - s * s).max(0.0).powf(0.65),
            60.5 + (LENGTH - 60.5) * s,
        ));
    }
    let body = Geometry::merge([
        Geometry::lathe(&[vec2(1.6, 0.0), vec2(BODY, 0.3), vec2(BODY, 41.6)], 64),
        Geometry::lathe(&fairing, 64),
    ]);
    scene.add_to(rocket, Node::mesh(body, paint));
    // frost on the oxygen tank, with a ragged lower edge
    let mut frost_band = Geometry::lathe(
        &[vec2(BODY + 0.012, FROST.0), vec2(BODY + 0.012, FROST.1)],
        64,
    );
    for p in &mut frost_band.positions {
        if p.y < 30.0 {
            let a = p.z.atan2(p.x);
            p.y += 2.5 * (0.5 + 0.5 * (a * 5.0).sin() * (a * 3.0 + 1.0).cos());
        }
    }
    let frost = scene.add_to(rocket, Node::mesh(frost_band, frost));
    // panel seams of the tanks: thin gray rings
    let seams =
        Geometry::merge([3.2f32, 9.0, 14.8, 20.6, 25.8, 33.0, 48.5, 54.7].map(|y| {
            Geometry::tube_ring(BODY + 0.02, BODY - 0.05, 0.08, 64).translate(0.0, y, 0.0)
        }));
    scene.add_to(
        rocket,
        Node::mesh(seams, Material::standard(0x9a9ca0).roughness(0.5)),
    );
    let mut marks = vec![
        // the interstage and the bottom of the engine section
        Geometry::lathe(&[vec2(BODY + 0.02, 41.6), vec2(BODY + 0.02, 46.0)], 64),
        Geometry::circle(1.6, 32).rotate_x(FRAC_PI_2),
        // the seam of the fairing halves
        Geometry::cuboid(0.06, 14.0, 0.06).translate(0.0, 61.0, 2.31),
    ];
    for k in 0..4 {
        let a = k as f32 * FRAC_PI_2 + PI / 4.0;
        let out = vec3(a.cos(), 0.0, a.sin());
        // stowed grid fins on the interstage
        marks.push(Geometry::cuboid(1.2, 0.14, 1.4).rotate_y(-a).translate(
            out.x * 2.3,
            44.6,
            out.z * 2.3,
        ));
        // the folded landing legs
        let leg = Geometry::extrude(
            &[
                vec2(-0.45, 0.0),
                vec2(0.45, 0.0),
                vec2(0.3, 11.5),
                vec2(-0.3, 11.5),
            ],
            0.35,
            0.05,
        );
        marks.push(leg.rotate_y(-a + FRAC_PI_2).translate(
            out.x * (BODY + 0.18),
            0.6,
            out.z * (BODY + 0.18),
        ));
    }
    scene.add_to(rocket, Node::mesh(Geometry::merge(marks), black));
    // the cable raceway on the side to the camera
    scene.add_to(
        rocket,
        Node::mesh(
            Geometry::cuboid(0.34, 38.0, 0.22).translate(0.55, 21.5, BODY + 0.05),
            Material::standard(0xd8d8d4).roughness(0.5),
        ),
    );
    // the engine section, dark from the soot of the test fires
    scene.add_to(
        rocket,
        Node::mesh(
            Geometry::lathe(&[vec2(BODY + 0.015, 0.3), vec2(BODY + 0.015, 2.8)], 64),
            soot,
        ),
    );
    // nine engines: one in the middle, eight around it
    let bell = Geometry::lathe(
        &[
            vec2(0.28, 0.3),
            vec2(0.3, 0.0),
            vec2(0.36, -0.3),
            vec2(0.6, -1.1),
            vec2(0.76, -1.9),
        ],
        24,
    );
    let mut engines = vec![Mat4::IDENTITY];
    engines.extend((0..8).map(|k| {
        let a = k as f32 / 8.0 * TAU;
        Mat4::from_translation(vec3(1.12 * a.cos(), 0.0, 1.12 * a.sin()))
    }));
    scene.add_to(
        rocket,
        Node::instanced(
            bell,
            Material::metal(0x4a3c34, 0.4).double_sided(),
            Instances::new(engines),
        ),
    );
    frost
}

/// The rest of the launch site: the tanks of oxygen and kerosene with their pipes, the
/// water tower of the deluge, the searchlights on the rocket, the blockhouse and the
/// hangar far away.
fn site(
    scene: &mut Scene,
    assets: &Assets,
    concrete: &Material,
    steel: &Material,
    white: &Material,
) {
    let unit = Arc::new(Geometry::cuboid(1.0, 1.0, 1.0).translate(0.0, 0.5, 0.0));
    let tank = Material::standard(0xe6e4de).roughness(0.4);
    // the oxygen sphere on legs
    scene.add(Node::mesh(
        Geometry::sphere(8.0, 40, 24).translate(-66.0, 12.0, -46.0),
        tank.clone(),
    ));
    let mut supports = Vec::new();
    for k in 0..8 {
        let a = k as f32 / 8.0 * TAU;
        let foot = vec3(-66.0 + 8.5 * a.cos(), 0.0, -46.0 + 8.5 * a.sin());
        supports.push(span(
            foot,
            foot + vec3(-0.6 * a.cos(), 12.0, -0.6 * a.sin()),
            0.6,
        ));
    }
    // the kerosene tanks on saddles
    let mut kerosene = Vec::new();
    for z in [-34.0f32, -42.0] {
        kerosene.push(
            Geometry::capsule(3.2, 22.0, 8, 32)
                .rotate_z(FRAC_PI_2)
                .translate(64.0, 4.8, z),
        );
        for x in [56.0f32, 72.0] {
            supports.push(Mat4::from_scale_rotation_translation(
                vec3(1.2, 2.0, 6.0),
                Quat::IDENTITY,
                vec3(x, 0.0, z),
            ));
        }
    }
    scene.add(Node::mesh(Geometry::merge(kerosene), tank));
    scene.add(Node::instanced(
        unit.clone(),
        concrete.clone(),
        Instances::new(supports),
    ));
    // the pipes to the pad on low supports
    let pipes = Geometry::merge([
        Geometry::tube(
            &[
                vec3(-58.0, 1.2, -42.0),
                vec3(-40.0, 1.2, -24.0),
                vec3(-27.0, 1.2, -16.0),
                vec3(-8.0, PAD_Y + 0.8, -6.0),
            ],
            0.55,
            10,
        ),
        Geometry::tube(
            &[
                vec3(52.0, 1.2, -38.0),
                vec3(40.0, 1.2, -24.0),
                vec3(27.0, 1.2, -16.0),
                vec3(8.0, PAD_Y + 0.8, -6.0),
            ],
            0.45,
            10,
        ),
    ]);
    scene.add(Node::mesh(pipes, steel.clone()));

    // the water tower of the deluge: a tank on four braced legs
    let (wx, wz) = (86.0f32, -64.0f32);
    let legs = [(-1.0f32, -1.0f32), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)].map(|(x, z)| {
        (
            vec3(wx + x * 8.0, 0.0, wz + z * 8.0),
            vec3(wx + x * 4.5, 46.0, wz + z * 4.5),
        )
    });
    let mut frame = Vec::new();
    for &(a, b) in &legs {
        frame.push(span(a, b, 0.9));
    }
    for ring in 1..=6 {
        let (s0, s1) = ((ring - 1) as f32 / 6.0, ring as f32 / 6.0);
        for k in 0..4 {
            let (a0, a1) = legs[k];
            let (b0, b1) = legs[(k + 1) % 4];
            let (p, q) = (a0.lerp(a1, s1), b0.lerp(b1, s1));
            frame.push(span(p, q, 0.4));
            frame.push(span(a0.lerp(a1, s0), q, 0.2));
            frame.push(span(b0.lerp(b1, s0), p, 0.2));
        }
    }
    frame.push(span(vec3(wx, 0.0, wz), vec3(wx, 47.0, wz), 1.4));
    scene.add(Node::instanced(
        unit.clone(),
        steel.clone(),
        Instances::new(frame),
    ));
    scene.add(Node::mesh(
        Geometry::lathe(
            &[
                vec2(0.0, 45.5),
                vec2(6.0, 46.5),
                vec2(9.0, 49.0),
                vec2(9.4, 54.0),
                vec2(9.0, 58.0),
                vec2(5.0, 61.5),
                vec2(0.0, 62.5),
            ],
            48,
        )
        .translate(wx, 0.0, wz),
        white.clone(),
    ));

    // xenon searchlights on the ground: beams of cold light on the rocket through the haze
    for (x, z) in [
        (-70.0f32, 64.0f32),
        (78.0, 82.0),
        (-96.0, -58.0),
        (104.0, -36.0),
    ] {
        let from = vec3(x, 2.4, z);
        let dir = (vec3(0.0, 46.0, 0.0) - from).normalize();
        let turn = Quat::from_rotation_arc(Vec3::Z, dir);
        scene.add(Node::mesh(
            Geometry::merge([
                Geometry::cuboid(1.6, 1.6, 2.2)
                    .rotate(turn)
                    .translate(x, 2.4, z),
                Geometry::cuboid(0.5, 1.6, 0.5).translate(x, 0.8, z),
            ]),
            Material::standard(0x2a2c30).roughness(0.5),
        ));
        scene.add(
            Node::mesh(
                Geometry::circle(0.65, 20).rotate(turn),
                Material::unlit(Rgb::hex(0xeef4ff) * 30.0),
            )
            .with_position(from + dir * 1.12)
            .shadows(false, false),
        );
        scene.add(
            Node::light(Light::spot(0xdfe8ff, 2.5e4, 0.3, 1.0).pointing(dir))
                .with_position(from + dir * 1.3),
        );
    }

    // the blockhouse with lit windows, and the hangar of the rockets far away
    scene.add(Node::mesh(
        box_uvs(Geometry::rounded_box(20.0, 5.0, 11.0, 0.4, 2).translate(-56.0, 2.5, 74.0)),
        concrete.clone(),
    ));
    scene.add(Node::mesh(
        Geometry::cuboid(13.0, 0.9, 0.1).translate(-56.0, 3.0, 79.56),
        Material::unlit(Rgb::hex(AMBER) * 5.0),
    ));
    // The landmark of shot 9 leaves out the nodes named "surroundings".
    let (walls, inside) = machines::hangar(90.0, 46.0, 44.0);
    scene.add(
        Node::mesh(
            box_uvs(walls.translate(HANGAR, 0.0, 0.0)),
            assets.corrugated.material(0xb8b8b4, 0.6, 0.4),
        )
        .named("surroundings"),
    );
    scene.add(
        Node::mesh(
            inside.translate(HANGAR, 0.0, 0.0),
            Material::unlit(Rgb::hex(COLD) * 1.3),
        )
        .shadows(false, false)
        .named("surroundings"),
    );
    // a row of work lamps over the door
    let lamps = (0..9)
        .map(|i| {
            let z = -14.0 + 3.5 * i as f32;
            Particle::new(vec3(HANGAR - 1.5, 37.0, z), 0.5, Rgb::hex(COLD) * 25.0)
        })
        .collect();
    scene.add(Node::particles(lamps, Blending::Additive).named("surroundings"));
    // low lamps along both edges of the road: a dotted line from the camera to the pad
    let mut lamps = Vec::new();
    for i in 0..22 {
        let z = 84.0 + i as f32 * 12.0;
        for x in [20.2f32, 31.8] {
            lamps.push(Particle::new(
                vec3(x, 0.45, z),
                0.14,
                Rgb::hex(0xffb060) * 18.0,
            ));
        }
    }
    scene.add(Node::particles(lamps, Blending::Additive).named("surroundings"));

    // another launch tower far away on the left, with red lights on the tower and white
    // work lights on its rocket
    let far = vec3(-1150.0, 0.0, -1500.0);
    let mut distant = vec![
        Geometry::cuboid(10.0, 92.0, 10.0).translate(far.x, 46.0, far.z),
        Geometry::cylinder(0.4, 1.2, 24.0, 6).translate(far.x, 104.0, far.z),
        Geometry::cylinder(1.9, 1.9, 60.0, 16).translate(far.x + 16.0, 30.0, far.z),
        Geometry::cuboid(70.0, 7.0, 50.0).translate(far.x + 10.0, 3.5, far.z),
    ];
    for k in 0..6 {
        distant.push(Geometry::cuboid(14.0, 0.8, 14.0).translate(
            far.x,
            12.0 + k as f32 * 14.0,
            far.z,
        ));
    }
    scene.add(
        Node::mesh(
            Geometry::merge(distant),
            Material::standard(0x3a3a40).roughness(0.7),
        )
        .named("surroundings"),
    );
    scene.add(
        Node::particles(
            [20.0f32, 50.0, 80.0, 115.0]
                .map(|y| Particle::new(far + vec3(5.5, y, 5.5), 1.2, Rgb::hex(0xff2a10) * 30.0))
                .into_iter()
                .chain([30.0f32, 60.0].map(|y| {
                    Particle::new(far + vec3(16.0, y, 2.5), 1.0, Rgb::hex(0xeef4ff) * 20.0)
                }))
                .collect(),
            Blending::Additive,
        )
        .named("surroundings"),
    );
    // the way of the transporter erector from the hangar: rails on the ground, a ramp up
    // to the height of the deck, and a bridge over the mouth of the trench
    let ramp = Geometry::extrude_segments(
        &[vec2(RAMP.0, 0.0), vec2(RAMP.1, 0.0), vec2(RAMP.0, PAD_Y)],
        13.0,
        0.2,
        1,
    );
    let bridge = Geometry::cuboid(RAMP.0 - MOUTH_X, 1.3, 13.0).translate(
        RAMP.0.midpoint(MOUTH_X),
        PAD_Y - 0.65,
        0.0,
    );
    scene.add(
        Node::mesh(box_uvs(Geometry::merge([ramp, bridge])), concrete.clone())
            .named("surroundings"),
    );
    let mut rails = Vec::new();
    for z in [-3.0f32, 3.0] {
        let points = [
            vec3(2.0, PAD_Y, z),
            vec3(RAMP.0, PAD_Y, z),
            vec3(RAMP.1, 0.0, z),
            vec3(HANGAR + 88.0, 0.0, z),
        ];
        for pair in points.windows(2) {
            let (a, b) = (pair[0], pair[1]);
            let d = b - a;
            let turn = Quat::from_rotation_z(d.y.atan2(d.x));
            rails.push(Mat4::from_scale_rotation_translation(
                vec3(d.length(), 0.3, 0.5),
                turn,
                (a + b) * 0.5 + Vec3::Y * 0.15,
            ));
        }
    }
    scene.add(
        Node::instanced(
            Geometry::cuboid(1.0, 1.0, 1.0),
            steel.clone(),
            Instances::new(rails),
        )
        .named("surroundings"),
    );
}

/// The landmark of shot 9: the pad, the tower and its lit lamps from the template, and
/// the trail that the rocket left: a column of smoke that rises 2.5 km and bends with the
/// wind to +X. Its top catches the first sun.
fn contrail(template: &Scene, ids: &Ids, lamps: &[Vec3]) -> Scene {
    let mut scene = template.clone();
    let remove: Vec<NodeId> = scene
        .nodes()
        .filter(|(id, node)| {
            matches!(node.name.as_deref(), Some("ground" | "surroundings"))
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
        let color = Rgb::hex(0xc8c8d6).lerp(Rgb::hex(0xffd8b0) * 2.6, sun) * 0.06;
        trail.push(Particle::new(
            drift + vec3(0.0, BASE_Y + y, 0.0) + wobble,
            8.0 + 70.0 * s,
            color,
        ));
    }
    // additive: a normal particle cannot be thin, see [`Cloud`]
    scene.add(Node::particles(trail, Blending::Additive));
    if let Some(mesh) = scene[ids.tower_lamps].mesh_mut() {
        mesh.instances = Some(Arc::new(Instances::new(
            lamps.iter().map(|p| Mat4::from_translation(*p)),
        )));
    }
    scene
}

/// The place of the camera and the point that it looks at, without the whip pan and the
/// tilt: it pushes in slowly to the pad.
fn rest(t: f32) -> (Vec3, Vec3) {
    let position = CAMERA + PUSH * ease(progress(t, 0.3, IGNITION + 0.6));
    (position, VIEW)
}

/// The spark of the greenhouse at `t`. It enters at its place in the picture of the cut,
/// flies to the foot of the tower, and runs up the corner of the tower.
fn spark_at(t: f32) -> Vec3 {
    let foot = vec3(FACE_X + 1.0, PAD_Y + 1.0, TOWER_HALF + 1.0);
    if t > SPARK_ARRIVES {
        let head = mix(
            PAD_Y + 1.0,
            TOWER_TOP,
            ease(progress(t, LIGHTS.0, LIGHTS.1)),
        );
        return foot.with_y(head);
    }
    let (eye, target) = rest(t);
    let forward = target - eye;
    let (yaw, pitch) = yaw_pitch(forward, foot - eye);
    let (cut_pitch, cut_distance) = SPARK_AT_CUT;
    let arrive = ease(progress(t, 0.0, SPARK_ARRIVES));
    let dart = whip(t + SPARK_LEAD) - WHIP_HALF;
    let distance = mix(cut_distance.ln(), foot.distance(eye).ln(), arrive).exp();
    polar(
        eye,
        forward,
        mix(dart, yaw, arrive),
        mix(cut_pitch, pitch, arrive),
        distance,
    )
}

/// The camera at `t`. It lands from the whip pan level with the horizon, rises to the
/// tower and the rocket, and pushes in slowly. At the lift off the sound shakes it, and it
/// tilts up after the rocket with a little lag. At the end it zooms into the flare.
pub(super) fn camera_at(t: f32) -> Camera {
    let (position, rest_target) = rest(t);
    let forward = (rest_target - position).normalize();
    let level = 1.0 - ease(progress(t, 0.05, 0.9));
    let mut pitch = mix(forward.y.asin(), LEVEL.atan(), level);
    // the tilt after the rocket: it keeps the rocket and the top of its flame in the frame
    let late = BASE_Y + altitude(t - 0.12);
    let follow = smoothstep(LIFTOFF, LIFTOFF + 0.6, t);
    let (_, rocket_pitch) = yaw_pitch(forward, vec3(-3.0, late + 30.0, 0.0) - position);
    pitch = mix(pitch, pitch.max(rocket_pitch), follow);
    let mut target = polar(position, forward, whip(t) - WHIP_HALF, pitch, 120.0);
    // the zoom into the flame
    let rocket_y = BASE_Y + altitude(t);
    let zoom = ease_in(progress(t, ZOOM.0, ZOOM.1));
    target = target.lerp(
        vec3(0.0, rocket_y - 8.0, 0.0),
        ease(progress(t, ZOOM.0 - 0.2, ZOOM.1)),
    );
    // the lens of the greenhouse at the cut, a little wider when the camera has landed
    let rest_mm = mix(32.0, 28.0, ease(progress(t, 0.1, 0.9)));
    let mm = mix(mix(rest_mm, 34.0, follow), 300.0, zoom);
    let focus = if t < SPARK_ARRIVES {
        spark_at(t)
    } else {
        vec3(0.0, rocket_y + 20.0, 0.0)
    };
    // the hand of the operator, and the rumble of the launch when its sound arrives
    let rumble =
        smoothstep(IGNITION + 0.5, IGNITION + 0.75, t) * (1.0 - 0.5 * smoothstep(3.2, 4.6, t));
    let camera = camera(mm, position, target)
        .focus_on(focus, 2.0)
        .shake(t, 0.0005, 23);
    rumble_shake(camera, t, 0.0045 * rumble)
}

/// The rumble of the launch: the view shakes by `amount` times the distance to the target.
/// It is a sum of sines below 11 Hz, so it moves in every frame and each frame shows a
/// small step of it. `Camera::shake` is value noise: it stops at each of its keys, and at a
/// high rate the stops and the jumps between them read as stutter.
fn rumble_shake(mut camera: Camera, t: f32, amount: f32) -> Camera {
    if amount == 0.0 {
        return camera;
    }
    let wave = |rates: [f32; 3], phase: f32| {
        (rates[0] * t + phase).sin() * 0.5
            + (rates[1] * t + phase * 1.7).sin() * 0.3
            + (rates[2] * t + phase * 2.3).sin() * 0.2
    };
    let distance = camera.position.distance(camera.target);
    let (right, up) = (camera.right(), camera.up_vector());
    let dx = wave([23.0, 47.0, 67.0], 0.4);
    let dy = wave([29.0, 53.0, 61.0], 1.9);
    camera.target += (right * dx + up * dy) * (amount * distance);
    camera.roll += wave([19.0, 37.0, 59.0], 3.1) * amount * 0.5;
    camera
}
