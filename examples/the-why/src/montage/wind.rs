//! A quick build at night: a wind farm on a ridge above a village. The moon is low behind
//! the ridge, so the turbines stand against the sky with a cold rim. The times are seconds
//! into the part.
//!
//! - 0.0: the flare of the car on the bridge sits on the crest at the middle of the
//!   picture. It splits into two heads that draw the warm line along the crest.
//! - 0.05 to 0.75: behind the heads, cranes raise their booms, the towers rise, and the
//!   cranes lift the nacelles and the rotors over the crest.
//! - 0.5 to 0.95: the blades start to turn, and the red lights on the nacelles blink.
//! - 0.75 to 1.1: power runs down the pylons to the village. The village lights come on
//!   from the ridge to the camera, and then the lamps of the road.
//! - 1.17 to 1.25: the camera trucks right behind a tree on the hill. The tree wipes the
//!   picture for the cut to the school.

use std::f32::consts::{FRAC_PI_2, PI, TAU};
use std::sync::Arc;

use fframes_3d::math::{hash, progress, smoothstep};
use fframes_3d::*;

use super::{
    AMBER, AMBER_EDGE, COLD, camera, ease, ease_out, fbm, mix, random_in_ball, sky, span, stars,
};
use crate::assets::Assets;

/// The length of the part in seconds.
pub const LENGTH: f32 = 1.25;
/// The crest of the ridge runs along X at about this Z.
const CREST_Z: f32 = -1500.0;
/// The X of the point on the crest where the line starts, at the middle of the picture.
const START_X: f32 = 0.0;
/// The half length of the warm line along the crest in meters.
const LINE_HALF: f32 = 1700.0;
/// The time that the heads of the line take to reach their ends.
const LINE_TIME: f32 = 0.32;
/// The height of the hub above the foot of the tower, and the length of a blade: a large
/// modern turbine, so the turbines read against the sky from the far camera.
const HUB: f32 = 128.0;
const BLADE: f32 = 74.0;
/// The heights of the joints of the three tower sections.
const SECTIONS: [f32; 4] = [0.0, 42.0, 86.0, HUB - 2.0];
/// The radius of the tower at the foot and at the top.
const TOWER_RADIUS: (f32, f32) = (3.0, 1.8);
/// The length of the boom of a crane.
const BOOM: f32 = 160.0;
/// The yaw of all rotors: they face the wind, a little to the left of the camera.
const YAW: f32 = -0.22;
/// The turbines along the crest, and their spacing in meters.
const TURBINES: i32 = 6;
const SPACING: f32 = 255.0;
/// The power reaches the village, and the lights come on from the ridge to the camera.
const POWER: (f32, f32) = (0.72, 0.9);
const VILLAGE: (f32, f32) = (0.84, 1.08);
/// The camera at the start and at the end. It trucks right with a growing speed, and
/// passes behind a tree at the end.
const CAMERA_FROM: Vec3 = Vec3::new(-27.0, 44.0, 300.0);
const CAMERA_TO: Vec3 = Vec3::new(4.0, 34.0, 296.0);
/// The middle of the village, on the bench below the crest.
const VILLAGE_Z: f32 = -1060.0;
/// The direction to the moon: behind the ridge, right of the middle of the picture.
const MOON: Vec3 = Vec3::new(0.205, 0.185, -0.961);

/// The Z of the crest at `x`: it bends a little along the ridge.
fn crest_z(x: f32) -> f32 {
    CREST_Z + 140.0 * fbm(31, x / 1900.0 + 3.0, 0.5, 2)
}

/// The height of the land at `x`, `z`: a long round ridge with a gentle apron toward the
/// camera, low hills behind it, and a valley floor.
fn land(x: f32, z: f32) -> f32 {
    let s = z - crest_z(x);
    let crest = 190.0 + 50.0 * fbm(32, x / 1100.0, 0.5, 3);
    // the front falls steeply to a bench for the village, then to the valley floor
    let front = 0.78 * (-(s / 300.0).powi(2)).exp() + 0.22 * (1.0 - smoothstep(420.0, 820.0, s));
    let back = (-(s / 650.0).powi(2)).exp();
    let shape = if s > 0.0 { front } else { back };
    // gullies run down the front of the ridge
    let gully = if s > 0.0 {
        -18.0
            * (1.0 - fbm(37, x / 260.0, 0.3, 3).abs()).powi(3)
            * smoothstep(40.0, 260.0, s)
            * (1.0 - smoothstep(380.0, 620.0, s))
    } else {
        0.0
    };
    let rough = 7.0 * fbm(34, x / 180.0, z / 180.0, 4) * shape.sqrt() + gully;
    crest * shape + rough + 6.0 * fbm(33, x / 500.0, z / 500.0, 3)
}

/// The point on the crest at `x`.
fn crest(x: f32) -> Vec3 {
    let z = crest_z(x);
    vec3(x, land(x, z), z)
}

/// The distance in meters that each head of the line has run from the start at `t`.
fn reach(t: f32) -> f32 {
    LINE_HALF * ease_out(progress(t, 0.02, 0.02 + LINE_TIME))
}

/// The time at which the heads reach `distance` meters from the start: the inverse of
/// [`reach`].
fn reach_time(distance: f32) -> f32 {
    let p = 1.0 - (1.0 - (distance / LINE_HALF).min(1.0)).cbrt();
    0.02 + LINE_TIME * p
}

/// A turbine: the foot of its tower, the time the line passes it, and the angle of its
/// blades at rest.
struct Turbine {
    foot: Vec3,
    start: f32,
    phase: f32,
}

impl Turbine {
    /// The crane stands on the camera side of the tower, a little to the right.
    fn crane(&self) -> Vec3 {
        let p = self.foot + vec3(22.0, 0.0, 19.0);
        vec3(p.x, land(p.x, p.z), p.z)
    }

    /// The part of the tower that stands at `t`, `0..1`.
    fn tower(&self, t: f32) -> f32 {
        ease(progress(t, self.start + 0.07, self.start + 0.3))
    }

    /// The height of the nacelle and of the rotor on the hook, `0..1` each. They rise from
    /// behind the crest.
    fn lifts(&self, t: f32) -> (f32, f32) {
        (
            ease(progress(t, self.start + 0.24, self.start + 0.36)),
            ease(progress(t, self.start + 0.33, self.start + 0.47)),
        )
    }

    /// The time at which the rotor is in place.
    fn running(&self) -> f32 {
        self.start + 0.47
    }

    /// The angle of the boom above the horizontal, in radians: up while the crane lifts,
    /// down when the turbine runs.
    fn boom(&self, t: f32) -> f32 {
        let up = ease(progress(t, self.start, self.start + 0.16));
        let down = ease(progress(t, self.running() + 0.05, self.running() + 0.3));
        mix(0.18, 1.32, up * (1.0 - down))
    }

    /// The angle of the rotor at `t`. The rotor speeds up to 6 rad/s in 0.3 s: a
    /// time-lapse of a real rotor.
    fn spin(&self, t: f32) -> f32 {
        let (speed, ramp) = (6.0, 0.3);
        let u = (t - self.running()).max(0.0);
        let turn = if u < ramp {
            speed * u * u / (2.0 * ramp)
        } else {
            speed * (u - ramp * 0.5)
        };
        self.phase + turn
    }
}

/// A house of the village: its transform and the time its lights come on. A dark house
/// has no light.
struct House {
    place: Mat4,
    on: f32,
    color: Rgb,
}

/// The node ids that the frames animate.
struct Ids {
    line: NodeId,
    glows: NodeId,
    flare: NodeId,
    sections: [NodeId; 3],
    nacelles: NodeId,
    rotors: NodeId,
    cranes: NodeId,
    booms: NodeId,
    hooks: NodeId,
    windows: NodeId,
    wires: NodeId,
    village_glow: [NodeId; 3],
}

/// The ridge, the turbines, the village and the moon.
pub struct Wind {
    scene: Scene,
    ids: Ids,
    turbines: Vec<Turbine>,
    /// The points of the warm line along the crest, from west to east.
    line: Vec<Vec3>,
    houses: Vec<House>,
    /// The street lamps and the lamps of the road, with the time each comes on.
    lamps: Vec<(Vec3, f32)>,
    /// The wires from the ridge to the village: the two ends of each span and the time the
    /// power reaches it.
    wires: Vec<(Vec3, Vec3, f32)>,
    /// The access road from the village up to the crest, a point every few meters.
    road: Vec<Vec3>,
}

impl Wind {
    /// Builds the part. It takes a fraction of a second.
    pub fn new(assets: &Assets) -> Self {
        let turbines: Vec<Turbine> = (-TURBINES..=TURBINES)
            .map(|k| {
                let seed = (k + 40) as u32;
                let x = START_X + k as f32 * SPACING + 60.0 * (hash(seed) - 0.5);
                let foot = crest(x) - Vec3::Y;
                Turbine {
                    foot,
                    start: reach_time((x - START_X).abs()),
                    phase: hash(seed * 7) * TAU,
                }
            })
            .collect();
        let line = (0..=(2.0 * LINE_HALF / 6.0) as usize)
            // 6 m above the crest: the bumps of the land in front of it do not cut the line
            .map(|i| crest(START_X - LINE_HALF + i as f32 * 6.0) + Vec3::Y * 6.0)
            .collect();
        let (houses, mut lamps) = village();
        let (scene, ids, wires) = build(assets, &houses);
        lamps.extend(road_lamps());
        let road = access_road();
        Self {
            scene,
            ids,
            turbines,
            line,
            houses,
            lamps,
            wires,
            road,
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
            // the wind moves the haze along the ridge
            air.offset = vec3(t * 40.0, 0.0, t * 6.0);
            // the tree fills the picture: fewer haze samples leave time for the motion blur
            air.steps = if t > LENGTH - 0.12 { 12 } else { 32 };
        }
        let mut glows = Vec::new();

        // the flare of the car fades into the start of the line
        let flare = (-t / 0.07).exp();
        let start = crest(START_X) + Vec3::Y * 6.0;
        glows.push(Particle::new(
            start,
            5.0 + 26.0 * flare,
            Rgb::hex(AMBER) * (3.0 + 30.0 * flare),
        ));
        if let Some(light) = scene[ids.flare].light_mut() {
            *light.intensity_mut() = 4.0e4 * flare + 4.0e3;
        }

        // the warm line and its two heads
        let half = reach(t);
        let dim = mix(1.0, 0.45, smoothstep(0.7, 1.1, t));
        let segments: Vec<Mat4> = self
            .line
            .windows(2)
            .filter(|w| (w[0].x - START_X).abs() <= half && (w[1].x - START_X).abs() <= half)
            .map(|w| span(w[0], w[1], 1.3))
            .collect();
        set(&mut scene, ids.line, Instances::new(segments));
        if let Some(m) = scene[ids.line].material_mut() {
            m.color = Rgb::hex(AMBER_EDGE) * (9.0 * dim);
        }
        if half < LINE_HALF - 1.0 {
            for side in [-1.0, 1.0] {
                let head = crest(START_X + side * half) + Vec3::Y * 6.0;
                glows.push(Particle::new(head, 4.0, Rgb::hex(0xfff0d8) * 80.0));
                glows.push(Particle::new(head, 14.0, Rgb::hex(AMBER) * 4.0));
            }
        }

        // the trucks on the access road leave trails of light in the time-lapse: red tail
        // lights up to the crest, white headlights down to the village
        for k in 0..14 {
            let up = k % 2 == 0;
            let start = -0.1 + k as f32 * 0.05;
            let u = progress(t, start, start + 0.3);
            let fade = smoothstep(0.0, 0.05, u) * (1.0 - smoothstep(0.92, 1.0, u));
            if fade <= 0.0 || t > 0.85 {
                continue;
            }
            let along = |u: f32| {
                let u = if up { u } else { 1.0 - u }.clamp(0.0, 1.0);
                let i = u * (self.road.len() - 1) as f32;
                let (a, b) = (
                    self.road[i as usize],
                    self.road[(i as usize + 1).min(self.road.len() - 1)],
                );
                a.lerp(b, i.fract())
            };
            let color = if up {
                Rgb::hex(0xff2a10) * 14.0
            } else {
                Rgb::hex(0xfff2dc) * 22.0
            };
            for j in 0..5 {
                let (a, b) = (along(u - j as f32 * 0.02), along(u - (j + 1) as f32 * 0.02));
                let strength = fade * (1.0 - j as f32 / 5.0);
                glows.push(Particle::new(a, 0.9, color * strength).tail(b - a));
            }
        }

        // the turbines and their cranes
        let mut sections = [Vec::new(), Vec::new(), Vec::new()];
        let (mut nacelles, mut rotors, mut cranes, mut booms, mut hooks) =
            (Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new());
        let yaw = Quat::from_rotation_y(YAW);
        for turbine in &self.turbines {
            if t < turbine.start {
                continue;
            }
            let height = turbine.tower(t) * SECTIONS[3];
            for k in 0..3 {
                let s = ((height - SECTIONS[k]) / (SECTIONS[k + 1] - SECTIONS[k])).clamp(0.0, 1.0);
                if s > 0.002 {
                    sections[k].push(Mat4::from_scale_rotation_translation(
                        vec3(1.0, s, 1.0),
                        Quat::IDENTITY,
                        turbine.foot + Vec3::Y * SECTIONS[k],
                    ));
                }
            }
            let hub = turbine.foot + Vec3::Y * HUB;
            // the loads rise from behind the crest, where the trucks wait
            let below = turbine.foot + vec3(0.0, -70.0, -60.0);
            let (nacelle, rotor) = turbine.lifts(t);
            let nacelle_at = below.lerp(hub, nacelle);
            let rotor_at = (below - Vec3::Y * 40.0).lerp(hub, rotor);
            if nacelle > 0.0 {
                nacelles.push(Mat4::from_rotation_translation(yaw, nacelle_at));
            }
            if rotor > 0.0 {
                let front = yaw * vec3(0.0, 0.0, 5.2);
                let spin = Quat::from_rotation_z(turbine.spin(t));
                rotors.push(Mat4::from_rotation_translation(
                    yaw * spin,
                    rotor_at + front * rotor,
                ));
            }
            // the crane: its body, the boom toward the tower, the hook and its lights
            let base = turbine.crane();
            let to_tower = (turbine.foot - base).with_y(0.0).normalize();
            let angle = turbine.boom(t);
            let pivot = base + Vec3::Y * 3.5;
            let tip = pivot + (to_tower * angle.cos() + Vec3::Y * angle.sin()) * BOOM;
            let facing = Quat::from_rotation_y(to_tower.x.atan2(to_tower.z));
            cranes.push(Mat4::from_rotation_translation(facing, base));
            booms.push(span(pivot, tip, 1.0));
            let load = if nacelle < 1.0 {
                Some(nacelle_at + Vec3::Y * 2.5)
            } else if rotor < 1.0 && rotor > 0.0 {
                Some(rotor_at + Vec3::Y * 2.0)
            } else {
                None
            };
            if let Some(load) = load.filter(|l| l.y < tip.y - 2.0) {
                hooks.push(span(load, tip, 0.15));
            }
            let working = 1.0 - smoothstep(turbine.running() + 0.1, turbine.running() + 0.35, t);
            if working > 0.01 {
                glows.push(Particle::new(tip, 0.9, Rgb::hex(COLD) * (40.0 * working)));
                glows.push(Particle::new(tip, 6.0, Rgb::hex(COLD) * (0.5 * working)));
                glows.push(Particle::new(
                    base + vec3(0.0, 6.0, 0.0),
                    5.0,
                    Rgb::hex(COLD) * (6.0 * working),
                ));
            }
            // the red light on the nacelle blinks with the lights of the other turbines
            if t > turbine.running() + 0.05 && (t * 2.2).fract() < 0.5 {
                let top = hub + yaw * vec3(0.0, 2.6, -9.0);
                glows.push(Particle::new(top, 1.6, Rgb::hex(0xff2010) * 60.0));
                glows.push(Particle::new(top, 12.0, Rgb::hex(0xff2010) * 0.9));
            }
        }
        for (k, list) in sections.into_iter().enumerate() {
            set(&mut scene, ids.sections[k], Instances::new(list));
        }
        set(&mut scene, ids.nacelles, Instances::new(nacelles));
        set(&mut scene, ids.rotors, Instances::new(rotors));
        set(&mut scene, ids.cranes, Instances::new(cranes));
        set(&mut scene, ids.booms, Instances::new(booms));
        set(&mut scene, ids.hooks, Instances::new(hooks));

        // the power runs down the wires to the village: a warm pulse and a glow behind it
        let pulse = progress(t, POWER.0, POWER.1);
        let colors: Vec<Rgb> = self
            .wires
            .iter()
            .map(|&(_, _, at)| {
                let since = pulse - at;
                if since < 0.0 || t < POWER.0 {
                    Rgb::hex(0x0a0c10)
                } else {
                    Rgb::hex(AMBER) * (0.6 + 9.0 * (-since / 0.08).exp())
                }
            })
            .collect();
        let transforms = self.wires.iter().map(|&(a, b, _)| span(a, b, 0.12));
        set(
            &mut scene,
            ids.wires,
            Instances::new(transforms).colors(colors),
        );
        if (POWER.0..POWER.1 + 0.02).contains(&t)
            && let Some(&(a, b, _)) = self.wires.iter().find(|w| w.2 >= pulse)
        {
            glows.push(Particle::new(
                a.lerp(b, 0.5),
                2.0,
                Rgb::hex(0xfff0d8) * 60.0,
            ));
            glows.push(Particle::new(a.lerp(b, 0.5), 18.0, Rgb::hex(AMBER) * 2.0));
        }

        // the village: windows, street lamps, and the warm glow over the roofs
        let windows: Vec<Mat4> = self.houses.iter().map(|h| h.place).collect();
        let lit: Vec<Rgb> = self
            .houses
            .iter()
            .map(|h| {
                let on = smoothstep(h.on, h.on + 0.04, t);
                Rgb::hex(0x06080c).lerp(h.color, on)
            })
            .collect();
        set(&mut scene, ids.windows, Instances::new(windows).colors(lit));
        let mut lamps_on = 0.0;
        for &(p, on) in &self.lamps {
            let on = smoothstep(on, on + 0.03, t);
            if on > 0.0 {
                lamps_on += on;
                glows.push(Particle::new(p, 0.9, Rgb::hex(0xffb060) * (30.0 * on)));
                glows.push(Particle::new(p, 9.0, Rgb::hex(0xff9a40) * (0.35 * on)));
            }
        }
        let village = smoothstep(VILLAGE.0, VILLAGE.1, t) * 0.6 + 0.004 * lamps_on;
        for id in ids.village_glow {
            if let Some(light) = scene[id].light_mut() {
                *light.intensity_mut() = 2.5e4 * village;
            }
        }
        if let Object::Particles(p) = &mut scene[ids.glows].object {
            p.items = glows;
        }
        (scene, camera_at(t))
    }

    /// The motion blur samples at `t`: more when the tree wipes the picture.
    pub fn shutter(&self, t: f32) -> usize {
        if t > LENGTH - 0.12 { 5 } else { 2 }
    }
}

/// The houses of the village and its street lamps, with the times that their lights come
/// on. The village lies on the apron of the ridge, in streets that run across the slope.
fn village() -> (Vec<House>, Vec<(Vec3, f32)>) {
    let center = vec3(-150.0, 0.0, VILLAGE_Z);
    let turn = Quat::from_rotation_y(0.32);
    let mut houses = Vec::new();
    let mut lamps = Vec::new();
    let colors = [0xffb46a, 0xffc890, 0xffa458, 0xffe0b0, 0xc8d8ff];
    for row in -3..=3i32 {
        for col in -9..=9i32 {
            let seed = ((row + 10) * 40 + col + 20) as u32;
            // the edge of the village is ragged
            let r = vec2(col as f32 / 9.0, row as f32 / 3.0).length();
            if r > 0.75 + 0.35 * hash(seed * 3) {
                continue;
            }
            let local = vec3(
                col as f32 * 34.0 + 8.0 * (hash(seed * 5) - 0.5),
                0.0,
                row as f32 * 48.0 + if col % 2 == 0 { 9.0 } else { -9.0 },
            );
            let p = center + turn * local;
            let size = vec3(
                8.0 + 6.0 * hash(seed * 7),
                5.0 + 3.5 * hash(seed * 11),
                8.0 + 4.0 * hash(seed * 13),
            );
            let ground = land(p.x, p.z);
            let place = Mat4::from_scale_rotation_translation(
                size,
                turn * Quat::from_rotation_y(0.15 * (hash(seed * 17) - 0.5)),
                vec3(p.x, ground - 0.5, p.z),
            );
            // the light reaches the far houses first: the power comes from the ridge
            let wave = progress(p.z, VILLAGE_Z - 160.0, VILLAGE_Z + 160.0);
            let dark = hash(seed * 19) < 0.18;
            houses.push(House {
                place,
                on: if dark {
                    99.0
                } else {
                    mix(VILLAGE.0, VILLAGE.1 - 0.06, wave) + 0.05 * hash(seed * 23)
                },
                color: Rgb::hex(colors[(hash(seed * 29) * colors.len() as f32) as usize % 5])
                    * (2.2 + 1.6 * hash(seed * 31)),
            });
            // a lamp at each crossing of the streets
            if row.rem_euclid(2) == 0 && col.rem_euclid(3) == 0 {
                let q = center + turn * (local + vec3(17.0, 0.0, 24.0));
                lamps.push((
                    vec3(q.x, land(q.x, q.z) + 7.0, q.z),
                    mix(VILLAGE.0, VILLAGE.1, wave) + 0.03,
                ));
            }
        }
    }
    (houses, lamps)
}

/// The road from the village down the valley toward the camera, with a lamp every 45 m.
/// The lamps come on one after the other, toward the camera.
fn road_lamps() -> Vec<(Vec3, f32)> {
    (0..20)
        .map(|i| {
            let u = i as f32 / 19.0;
            let p = vec3(
                mix(-120.0, -520.0, u) + 40.0 * (u * 4.0).sin(),
                0.0,
                mix(VILLAGE_Z + 170.0, -300.0, u),
            );
            (
                vec3(p.x, land(p.x, p.z) + 8.0, p.z),
                VILLAGE.1 - 0.04 + 0.16 * u,
            )
        })
        .collect()
}

/// The camera at `t`. It looks at the start of the line, then tilts up a little, so the
/// rotors stay in the picture. It trucks right faster and faster and passes behind the tree at the end.
fn camera_at(t: f32) -> Camera {
    let x = progress(t, 0.0, LENGTH);
    let position = CAMERA_FROM.lerp(CAMERA_TO, x * x);
    let start = crest(START_X) + Vec3::Y * 2.0;
    let tilt = ease(progress(t, 0.35, LENGTH));
    // the start of the line is where the flare of the car was in the picture: a little
    // right of and above the middle
    let target =
        start + vec3(-36.0, -35.0, 0.0) + (position - CAMERA_FROM) * 0.6 + Vec3::Y * (28.0 * tilt);
    camera(55.0, position, target)
        .focus_on(start, mix(3.0, 14.0, smoothstep(LENGTH - 0.2, LENGTH, t)))
        .shake(t, 0.0004, 13)
}

/// A blade of the rotor along +Y from the hub: a thin twisted wing, widest near the root.
fn blade() -> Geometry {
    let stations = 14;
    let around = 8;
    let mut positions = Vec::new();
    let mut indices = Vec::new();
    for j in 0..=stations {
        let u = j as f32 / stations as f32;
        let r = 1.8 + u * BLADE;
        let chord = if u < 0.18 {
            mix(2.6, 5.2, smoothstep(0.0, 0.18, u))
        } else {
            mix(5.2, 0.9, (u - 0.18) / 0.82)
        };
        let thick = (chord * 0.18).max(0.3) * if u < 0.08 { 2.0 } else { 1.0 };
        let twist = Quat::from_rotation_y(0.35 * (1.0 - u) + 0.05);
        for i in 0..around {
            let a = i as f32 / around as f32 * TAU;
            // the leading edge is round and the trailing edge is thin
            let x = chord * (0.5 * a.cos() - 0.15);
            let z = 0.5 * thick * a.sin() * (0.6 + 0.4 * a.cos());
            positions.push(twist * vec3(x, 0.0, z) + Vec3::Y * r);
        }
    }
    for j in 0..stations {
        for i in 0..around {
            let a = (j * around + i) as u32;
            let b = (j * around + (i + 1) % around) as u32;
            let (c, d) = (a + around as u32, b + around as u32);
            indices.extend([a, c, b, b, c, d]);
        }
    }
    // the tip
    let tip = positions.len() as u32;
    positions.push(Vec3::Y * (1.8 + BLADE + 0.3));
    let last = (stations * around) as u32;
    for i in 0..around as u32 {
        indices.extend([last + i, tip, last + (i + 1) % around as u32]);
    }
    Geometry::new(positions, Vec::new(), Vec::new(), indices)
}

/// The surface of the moon, for the UVs of [`Geometry::sphere`]: bright highlands, dark
/// maria, and craters with bright rims and rays. The noise runs on the direction from the
/// center, so the map has no seam.
fn moon_map() -> Arc<Texture> {
    let (width, height) = (512u32, 256u32);
    let mut rgba = Vec::with_capacity((width * height * 4) as usize);
    let craters: Vec<(Vec3, f32)> = (0..60u32)
        .map(|i| {
            let size = 0.03 + 0.12 * hash(i * 3 + 501).powi(3);
            (math::random_direction(i * 3 + 500), size)
        })
        .collect();
    for y in 0..height {
        for x in 0..width {
            let around = (x as f32 + 0.5) / width as f32 * TAU;
            let polar = (y as f32 + 0.5) / height as f32 * PI;
            let d = vec3(
                polar.sin() * around.cos(),
                polar.cos(),
                polar.sin() * around.sin(),
            );
            let (a, b) = (d.x * 1.7 + d.z * 0.9, d.y * 1.7 - d.z * 0.6);
            let maria = smoothstep(0.05, 0.3, fbm(91, a + 4.0, b + 4.0, 4));
            let fine = fbm(92, a * 9.0, b * 9.0, 4);
            let mut gray = mix(0.92, 0.5, maria) + 0.07 * fine;
            for &(c, size) in &craters {
                let r = d.angle_between(c) / size;
                if r < 3.0 {
                    // a dark floor, a bright rim, and faint rays around young craters
                    gray += if r < 0.8 {
                        -0.08
                    } else {
                        0.18 * (-(r - 1.0).powi(2) * 8.0).exp()
                    };
                    gray += 0.06 * (1.0 - r / 3.0) * (around * 40.0 + polar * 23.0).sin().max(0.0);
                }
            }
            let v = (gray.clamp(0.0, 1.0) * 255.0) as u8;
            rgba.extend([v, v, v, 255]);
        }
    }
    Arc::new(Texture::from_rgba8(width, height, &rgba, true))
}

/// The scene: the sky, the land, the village, the turbines and the lights.
fn build(assets: &Assets, houses: &[House]) -> (Scene, Ids, Vec<(Vec3, Vec3, f32)>) {
    let moon = MOON.normalize();
    let mut scene = Scene::new()
        .environment(Environment::map(
            sky(
                Rgb::hex(0x0a1430) * 0.55,
                Rgb::hex(0x2a3a64) * 0.42,
                Rgb::hex(0x8aa0d0) * 0.5,
                moon.z.atan2(moon.x),
                0.0,
            ),
            1.0,
        ))
        .background(Background::Environment { blur: 0.0 });

    // the land: grass on the slopes, rock on the crest, dark trees, fields in the valley
    let mut ground =
        Geometry::floor_segments(7000.0, 5200.0, 350, 260).translate(0.0, 0.0, -1700.0);
    ground.colors = ground
        .positions
        .iter()
        .map(|p| field_color(p.x, p.z).0.extend(1.0))
        .collect();
    for p in &mut ground.positions {
        p.y = land(p.x, p.z);
    }
    ground.compute_vertex_normals();
    ground.update_bounds();
    scene.add(Node::mesh(ground, assets.dirt.material(0xffffff, 1.0, 0.0)));
    let mut rocks = Vec::new();
    let mut trees = Vec::new();
    for i in 0..30000u32 {
        let x = (hash(i * 3 + 1) - 0.5) * 4400.0;
        let z = -2300.0 + hash(i * 3 + 2) * 2600.0;
        let y = land(x, z);
        let s = z - crest_z(x);
        if vec2(x + 150.0, (z - VILLAGE_Z) * 1.8).length() < 360.0 {
            continue;
        }
        let size = 0.6 + 2.4 * hash(i * 3 + 3).powi(2);
        let turn = Quat::from_rotation_y(hash(i) * TAU);
        let (_, cell) = field(x, z);
        // hedgerows on the edges of the fields, groves on the slopes
        let hedge = s > 650.0 && (cell.x < 0.035 || cell.y < 0.05) && i % 3 != 0;
        let grove = s > 140.0 && s < 650.0 && fbm(35, x / 220.0, z / 220.0, 3) > -0.05;
        if s.abs() < 220.0 && i % 9 == 0 {
            rocks.push(Mat4::from_scale_rotation_translation(
                vec3(size * 1.4, size * 0.7, size) * 2.0,
                turn,
                vec3(x, y + size * 0.3, z),
            ));
        } else if hedge || (grove && i % 2 == 0) {
            let height = 8.0 + 7.0 * hash(i * 5);
            trees.push(Mat4::from_scale_rotation_translation(
                vec3(height, height * (0.9 + 0.3 * hash(i * 11)), height),
                turn,
                vec3(x, y - 0.3, z),
            ));
        }
    }
    scene.add(Node::instanced(
        Geometry::icosahedron(1.0, 1).flat(),
        assets.rock.material(0x7a7066, 1.0, 0.0),
        Instances::new(rocks),
    ));
    let leaves = Material::standard(0x16200f)
        .roughness(0.95)
        .env_intensity(0.5);
    scene.add(Node::instanced(tree(), leaves, Instances::new(trees)));

    // the tree that wipes the picture at the end: a trunk and a crown of leafy clumps
    let tree = CAMERA_TO + vec3(0.0, -1.0, -9.0);
    let trunk_foot = vec3(tree.x + 0.6, land(tree.x, tree.z) - 1.0, tree.z - 1.0);
    scene.add(
        Node::mesh(
            Geometry::cylinder(0.35, 0.6, tree.y - trunk_foot.y, 10).translate(
                0.0,
                (tree.y - trunk_foot.y) * 0.5,
                0.0,
            ),
            assets.rock.material(0x3a3028, 1.0, 0.0),
        )
        .with_position(trunk_foot),
    );
    let clumps: Vec<Mat4> = (0..420u32)
        .map(|i| {
            let d = random_in_ball(i * 4 + 900);
            let r = 0.35 + 0.55 * hash(i * 4 + 903);
            Mat4::from_scale_rotation_translation(
                Vec3::splat(r),
                Quat::from_rotation_y(hash(i) * TAU) * Quat::from_rotation_x(hash(i + 7) * PI),
                tree + d * vec3(4.8, 6.0, 3.0),
            )
        })
        .collect();
    scene.add(Node::instanced(
        Geometry::icosahedron(1.0, 1).flat(),
        Material::standard(0x0a1008)
            .roughness(1.0)
            .env_intensity(0.2),
        Instances::new(clumps),
    ));

    // the village: plastered houses with roofs, and their windows. The walls are dark,
    // so the houses sit in the night until their windows come on.
    let house = Geometry::merge([
        Geometry::cuboid(1.0, 1.0, 1.0)
            .translate(0.0, 0.5, 0.0)
            .with_color(0x5a5048),
        Geometry::extrude(
            &[vec2(-0.56, 0.0), vec2(0.56, 0.0), vec2(0.0, 0.42)],
            1.08,
            0.0,
        )
        .translate(0.0, 1.0, 0.0)
        .with_color(0x3a2824),
    ]);
    let places: Vec<Mat4> = houses.iter().map(|h| h.place).collect();
    scene.add(Node::instanced(
        house,
        Material::standard(0xffffff).roughness(0.85),
        Instances::new(places),
    ));
    let mut panes = Vec::new();
    for (x, y) in [(-0.25, 0.32), (0.25, 0.32), (-0.25, 0.68), (0.25, 0.68)] {
        panes.push(Geometry::plane(0.16, 0.18).translate(x, y, 0.505));
        panes.push(
            Geometry::plane(0.16, 0.18)
                .rotate_y(FRAC_PI_2)
                .translate(0.505, y, x),
        );
    }
    let windows = scene.add(
        Node::instanced(
            Geometry::merge(panes),
            Material::unlit(Rgb::WHITE),
            Instances::default(),
        )
        .shadows(false, false),
    );
    let village_glow = [
        vec3(-150.0, 0.0, VILLAGE_Z),
        vec3(-360.0, 0.0, VILLAGE_Z + 60.0),
        vec3(60.0, 0.0, VILLAGE_Z - 40.0),
    ]
    .map(|p| {
        scene.add(
            Node::light(Light::point(0xffa860, 0.0, 400.0)).with_position(vec3(
                p.x,
                land(p.x, p.z) + 25.0,
                p.z,
            )),
        )
    });

    // the pylons from the crest down to the village, and their wires
    let route: Vec<Vec3> = (0..8)
        .map(|i| {
            let u = i as f32 / 7.0;
            let x = mix(START_X + 130.0, -60.0, u);
            let z = mix(crest_z(START_X + 130.0) + 40.0, -900.0, u);
            vec3(x, land(x, z), z)
        })
        .collect();
    let steel = assets.steel.material(0x6a6e74, 0.7, 0.6);
    let mut pylon = Vec::new();
    for k in 0..4 {
        let a = k as f32 * FRAC_PI_2 + PI / 4.0;
        let foot = vec3(a.cos(), 0.0, a.sin()) * 4.5;
        let top = vec3(a.cos(), 0.0, a.sin()) * 0.8 + Vec3::Y * 34.0;
        pylon.push(
            Geometry::cylinder(1.0, 1.0, 1.0, 4)
                .translate(0.0, 0.5, 0.0)
                .transform(span(foot, top, 0.25)),
        );
    }
    for y in [26.0, 31.0] {
        pylon.push(Geometry::cuboid(16.0, 0.6, 0.8).translate(0.0, y, 0.0));
    }
    let pylon = Geometry::merge(pylon);
    let pylons: Vec<Mat4> = route
        .iter()
        .map(|p| Mat4::from_translation(*p - Vec3::Y))
        .collect();
    scene.add(Node::instanced(
        pylon,
        steel.clone(),
        Instances::new(pylons),
    ));
    let mut wires = Vec::new();
    for (i, pair) in route.windows(2).enumerate() {
        let side = (pair[1] - pair[0]).cross(Vec3::Y).normalize();
        for (k, offset) in [-7.5f32, 0.0, 7.5].into_iter().enumerate() {
            let y = if k == 1 { 31.0 } else { 26.0 };
            let a = pair[0] + side * offset + Vec3::Y * (y - 1.5);
            let b = pair[1] + side * offset + Vec3::Y * (y - 1.5);
            let steps = 8;
            for j in 0..steps {
                let at = |u: f32| a.lerp(b, u) - Vec3::Y * (9.0 * 4.0 * u * (1.0 - u));
                let (u0, u1) = (j as f32 / steps as f32, (j + 1) as f32 / steps as f32);
                let time = (i as f32 + u0) / (route.len() - 1) as f32;
                wires.push((at(u0), at(u1), time));
            }
        }
    }
    let wire_nodes = scene.add(
        Node::instanced(
            Arc::new(Geometry::cylinder(1.0, 1.0, 1.0, 4).translate(0.0, 0.5, 0.0)),
            Material::unlit(Rgb::WHITE),
            Instances::default(),
        )
        .shadows(false, false),
    );

    // the warm line, the glows, and the light of the flare on the crest
    let unit = Arc::new(Geometry::cylinder(1.0, 1.0, 1.0, 6).translate(0.0, 0.5, 0.0));
    let line = scene.add(
        Node::instanced(
            unit.clone(),
            Material::unlit(Rgb::hex(AMBER_EDGE) * 9.0),
            Instances::default(),
        )
        .shadows(false, false),
    );
    let glows = scene.add(Node::particles(Vec::new(), Blending::Additive));
    let flare = scene.add(
        Node::light(Light::point(AMBER, 0.0, 500.0))
            .with_position(crest(START_X) + vec3(0.0, 30.0, 60.0)),
    );

    // the turbines: white towers in three sections, nacelles and rotors
    // white paint, with a rim of moonlight: the moon is behind the turbines
    let white = Material::standard(0xe4e6e8)
        .roughness(0.38)
        .clearcoat(0.3)
        .rim(Rgb::hex(0xb8c8ff) * 1.4, 1.8);
    let sections = [0, 1, 2].map(|k| {
        let (y0, y1) = (SECTIONS[k], SECTIONS[k + 1]);
        let r = |y: f32| mix(TOWER_RADIUS.0, TOWER_RADIUS.1, y / SECTIONS[3]);
        let mut parts = vec![Geometry::cylinder(r(y1), r(y0), y1 - y0, 28).translate(
            0.0,
            (y1 - y0) * 0.5,
            0.0,
        )];
        // a flange at the top of the section
        parts.push(
            Geometry::cylinder(r(y1) + 0.12, r(y1) + 0.12, 0.4, 28).translate(
                0.0,
                y1 - y0 - 0.2,
                0.0,
            ),
        );
        if k == 0 {
            // the door and the transformer box at the foot
            parts.push(Geometry::cuboid(1.2, 2.4, 0.5).translate(0.0, 1.6, r(0.0)));
            parts.push(Geometry::cuboid(2.6, 2.6, 2.0).translate(4.5, 1.3, 2.0));
        }
        scene.add(Node::instanced(
            Geometry::merge(parts),
            white.clone(),
            Instances::default(),
        ))
    });
    let nacelle = Geometry::merge([
        Geometry::rounded_box(3.8, 3.9, 11.5, 0.7, 3).translate(0.0, 0.6, -3.6),
        // the cooler and the wind vane on the roof
        Geometry::cuboid(2.6, 0.6, 2.4).translate(0.0, 2.85, -8.0),
        Geometry::cylinder(0.05, 0.05, 1.6, 4).translate(0.8, 3.4, -9.0),
    ])
    .scale(1.35, 1.35, 1.35);
    let nacelles = scene.add(Node::instanced(
        nacelle,
        white.clone(),
        Instances::default(),
    ));
    let spinner = Geometry::lathe(
        &[
            vec2(0.0, -1.6),
            vec2(1.7, -1.4),
            vec2(1.75, 0.0),
            vec2(1.2, 1.6),
            vec2(0.0, 2.4),
        ],
        24,
    )
    .scale(1.35, 1.35, 1.35)
    .rotate_x(FRAC_PI_2);
    let blade = blade();
    let rotor = Geometry::merge([
        spinner,
        blade.clone(),
        blade.clone().rotate_z(TAU / 3.0),
        blade.rotate_z(2.0 * TAU / 3.0),
    ]);
    let rotors = scene.add(Node::instanced(rotor, white, Instances::default()));

    // the crawler cranes: a yellow body, a lattice boom and the hook rope
    let crane = Geometry::merge([
        Geometry::cuboid(6.0, 2.0, 9.0)
            .translate(0.0, 1.0, 0.0)
            .with_color(0x2a2a2a),
        Geometry::cuboid(5.0, 3.0, 6.5)
            .translate(0.0, 3.4, -0.8)
            .with_color(0xd8a020),
        Geometry::cuboid(1.8, 2.2, 2.0)
            .translate(1.8, 4.0, 3.0)
            .with_color(0xd8a020),
    ]);
    let cranes = scene.add(Node::instanced(
        crane,
        Material::standard(0xffffff).roughness(0.6),
        Instances::default(),
    ));
    let booms = scene.add(Node::instanced(
        unit.clone(),
        Material::standard(0xc89020).roughness(0.6),
        Instances::default(),
    ));
    let hooks = scene.add(
        Node::instanced(unit, Material::standard(0x202020), Instances::default())
            .shadows(false, false),
    );

    // the moon: a disc with the texture of rock, and a soft halo
    let moon_at = moon * 4600.0;
    scene.add(
        Node::mesh(
            Geometry::sphere(70.0, 48, 24).rotate_y(2.3),
            Material::unlit(Rgb::hex(0xfff6e6) * 2.4)
                .map(moon_map())
                .no_fog(),
        )
        .with_position(moon_at)
        .shadows(false, false),
    );
    scene.add(Node::particles(
        vec![
            Particle::new(moon_at, 260.0, Rgb::hex(0xa8b8e0) * 0.12),
            Particle::new(moon_at, 700.0, Rgb::hex(0x8098c8) * 0.035),
        ],
        Blending::Additive,
    ));
    scene.add(Node::particles(stars(CAMERA_FROM, 1.0), Blending::Additive));

    // the moonlight from behind the ridge, and the faint light of the sky
    scene.add(Node::light(Light::sun(0xb4c4f0, 0.32, moon).with_shadow(
        Shadow {
            map_size: 4096,
            softness: 2.0,
            strength: 0.9,
            bias: 2.0,
            bounds: Some(Aabb {
                min: vec3(-1800.0, -20.0, -2000.0),
                max: vec3(1800.0, 420.0, 400.0),
            }),
        },
    )));
    scene.add(Node::light(Light::hemisphere(0x2a3a64, 0x0a0c10, 0.8)));
    // the light of the open sky behind the camera: it shows the folds, the gullies and
    // the groves of the slope, which the moon behind the ridge does not reach
    scene.add(Node::light(Light::directional(
        0x6a80b8,
        1.2,
        vec3(-0.6, 0.35, 0.7),
    )));
    scene.atmosphere = Some(Atmosphere {
        density: 0.00035,
        color: Rgb::WHITE,
        base: 0.0,
        falloff: 0.01,
        anisotropy: 0.7,
        ambient: Rgb::hex(0x2a3a60) * 0.02,
        noise: 0.5,
        noise_size: 160.0,
        offset: Vec3::ZERO,
        distance: 2600.0,
        steps: 32,
    });

    let ids = Ids {
        line,
        glows,
        flare,
        sections,
        nacelles,
        rotors,
        cranes,
        booms,
        hooks,
        windows,
        wires: wire_nodes,
        village_glow,
    };
    (scene, ids, wires)
}

/// The field at `x`, `z`: a number for the field, and the place in it, `0..1` on each
/// side. The fields are 150 x 95 m and turn with the streets of the village.
fn field(x: f32, z: f32) -> (u32, Vec2) {
    let (sin, cos) = 0.32f32.sin_cos();
    let u = (x * cos - z * sin) / 150.0;
    let v = (x * sin + z * cos) / 95.0;
    let id = (u.floor() as i32 as u32).wrapping_mul(7919) ^ (v.floor() as i32 as u32);
    (id, vec2(u.fract().abs(), v.fract().abs()))
}

/// The color of the land at `x`, `z`: fields of grass, crops and stubble in the valley,
/// grass on the slopes.
fn field_color(x: f32, z: f32) -> Rgb {
    let s = z - crest_z(x);
    let grass = Rgb::hex(0x56603e).lerp(
        Rgb::hex(0x6a6448),
        fbm(36, x / 400.0, z / 400.0, 3) * 0.5 + 0.5,
    );
    let (id, _) = field(x, z);
    let crops = [0x4a5a30, 0x6e6a40, 0x8a7a50, 0x3e4a2a, 0x7a7048];
    let crop = Rgb::hex(crops[(hash(id) * crops.len() as f32) as usize % crops.len()]);
    grass.lerp(crop, smoothstep(600.0, 760.0, s))
}

/// A tree of about 1 m: a short trunk and a crown of four leafy clumps. Scale it to its
/// height.
fn tree() -> Geometry {
    let clump = |x: f32, y: f32, z: f32, r: f32| Geometry::icosahedron(r, 1).translate(x, y, z);
    Geometry::merge([
        Geometry::cylinder(0.035, 0.06, 0.45, 5).translate(0.0, 0.225, 0.0),
        clump(0.0, 0.62, 0.0, 0.32),
        clump(0.2, 0.5, 0.08, 0.22),
        clump(-0.18, 0.52, -0.1, 0.24),
        clump(0.04, 0.86, 0.04, 0.2),
    ])
}

/// The access road from the village up to the crest: switchbacks on the slope, 2 m above
/// the land so that the lights of the trucks clear it.
fn access_road() -> Vec<Vec3> {
    let top = START_X + 190.0;
    (0..=240)
        .map(|i| {
            let u = i as f32 / 240.0;
            let z = mix(VILLAGE_Z - 110.0, crest_z(top) + 35.0, u);
            let x = mix(-40.0, top, u) + 120.0 * (u * 3.5 * PI).sin() * (1.0 - u);
            vec3(x, land(x, z) + 2.0, z)
        })
        .collect()
}
