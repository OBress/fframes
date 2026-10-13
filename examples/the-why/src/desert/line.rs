//! Shot 6, "The line": low on the dust in front of the kneeling engineer. His knee comes
//! down, and he draws one line toward himself with the index finger. Light seeps from the
//! groove, warm. After one still breath, the light of the line runs across the desert into
//! the factory.

use std::f32::consts::PI;
use std::sync::Arc;

use fframes_3d::math::{hash, noise1, smoothstep};
use fframes_3d::*;

use super::terrain::{dirt, playa_tone};
use super::walk::{KNEEL, kneel_at_cut};
use super::{
    AMBER, AMBER_EDGE, DesertSet, FACTORY_DISTANCE, FAR, ahead, hide_dark_lights, kneel_heading,
    kneel_point,
};
use crate::assets::Assets;
use crate::figure::{Joint, Pose, line_point};
use crate::look;

/// Half the width of the groove from lip to lip, in meters.
const HALF_WIDTH: f32 = 0.014;
/// The number of lamps along the line.
const LAMPS: usize = 6;
/// The number of dust grains near the line. The finger pushes aside those in its path.
const GRAINS: u32 = 520;
/// The finger touches the dust at this time, in seconds.
pub const TOUCH: f32 = 0.95;
/// The finger reaches the end of the line and stops.
const DRAW_END: f32 = 3.5;
/// The finger lifts from the dust.
pub const LIFT: f32 = 3.75;
/// The glow begins at the start of the line and follows the finger.
pub const IGNITE: f32 = 2.3;
/// The still moment: the glow pulses once at this time.
const BREATH: f32 = 4.0;
/// The light leaves the end of the line at this time.
const RUN: f32 = 4.5;
/// The light reaches the factory at this time.
const ARRIVAL: f32 = 6.15;

/// Across the groove: the offset in half widths and the height above the mud in meters.
/// The channel stays just above the mud, so the mud does not cover it. The lips of pushed
/// dust rise on both sides.
const PROFILE: [(f32, f32); 11] = [
    (-1.0, 0.0003),
    (-0.72, 0.0034),
    (-0.5, 0.0052),
    (-0.32, 0.0032),
    (-0.17, 0.0008),
    (0.0, 0.0005),
    (0.17, 0.0008),
    (0.32, 0.0032),
    (0.5, 0.0052),
    (0.72, 0.0034),
    (1.0, 0.0003),
];

/// The nodes of the line in the night scene. Shot 6 shows them.
pub(super) struct LineNodes {
    groove: NodeId,
    glow: NodeId,
    grains: NodeId,
    /// Dust, embers and sparks near the line.
    motes: NodeId,
    lamps: [NodeId; LAMPS],
    head: NodeId,
    head_light: NodeId,
}

impl LineNodes {
    pub fn build(scene: &mut Scene, assets: &Assets) -> Self {
        // Shot 6 sets the meshes of the groove and the glow in each frame. Until then they
        // hold a small hidden box.
        let empty = Arc::new(Geometry::cuboid(0.01, 0.01, 0.01));
        let groove = Node::mesh(empty.clone(), dirt(assets)).shadows(false, true);
        let glow = Node::mesh(empty, Material::glow(AMBER, 1.0)).shadows(false, false);
        let grains = Node::instanced(
            super::terrain::rock(77, 1),
            assets.dirt.material(0xc0b098, 1.0, 0.0),
            Instances::new([Mat4::IDENTITY]),
        );
        Self {
            groove: scene.add(groove.hidden()),
            glow: scene.add(glow.hidden()),
            grains: scene.add(grains.shadows(false, false).hidden()),
            motes: scene.add(Node::particles(Vec::new(), Blending::Additive)),
            lamps: std::array::from_fn(|_| scene.add(Node::light(Light::point(AMBER, 0.0, 1.5)))),
            head: scene.add(Node::particles(Vec::new(), Blending::Additive)),
            head_light: scene.add(Node::light(Light::point(AMBER, 0.0, 0.0))),
        }
    }
}

/// The part of the line that the finger has drawn at `t`, `0..1`.
fn drawn(t: f32) -> f32 {
    smoothstep(TOUCH, DRAW_END, t)
}

/// The part of the line that glows at `t`, `0..1`. The glow starts at the start of the line
/// and catches up with the finger when it stops.
fn lit(t: f32) -> f32 {
    let u = ((t - IGNITE) / (DRAW_END + 0.1 - IGNITE)).clamp(0.0, 1.0);
    (1.0 - (1.0 - u).powi(2)).min(drawn(t))
}

/// The brightness of the glow at `t`: it grows from the ignition, pulses once in the
/// breath, and stays when the light runs away.
fn glow_level(t: f32) -> f32 {
    let grow = smoothstep(IGNITE, IGNITE + 0.8, t);
    let pulse = (-((t - BREATH) / 0.22).powi(2)).exp();
    grow * (1.0 + 0.9 * pulse)
}

/// How much light seeps from the groove `s` meters along the line at `t`. It is uneven:
/// hot spots, dim places where dust covers the groove, and a slow flicker.
fn seep(s: f32, t: f32) -> f32 {
    let spots = smoothstep(-0.3, 0.7, noise1(31, s * 26.0));
    let grain = 0.6 + 0.4 * noise1(32, s * 90.0 - t * 0.6);
    let flicker = 0.85 + 0.15 * noise1(33, s * 18.0 - t * 2.5);
    (0.2 + 0.8 * spots * grain) * flicker
}

/// The distance in meters that the light has run along the ground at `t`, of `length`.
fn run_distance(t: f32, length: f32) -> f32 {
    let u = ((t - RUN) / (ARRIVAL - RUN)).clamp(0.0, 1.0);
    length * u.powf(2.6)
}

/// The speed of the time of the sand at `t`: the wind calms while he draws and stops in
/// the breath. When the light runs, the wind comes back.
fn sand_rate(t: f32) -> f32 {
    0.35 - 0.3 * smoothstep(3.2, 3.7, t) + 0.5 * smoothstep(4.5, 5.2, t)
}

/// The time of the sand at `t`, the integral of [`sand_rate`].
fn sand_time(t: f32) -> f32 {
    let steps = 64;
    let dt = t / steps as f32;
    (0..steps)
        .map(|i| sand_rate((i as f32 + 0.5) * dt) * dt)
        .sum()
}

/// The time when the right knee touches the ground: the kneel of shot 5 goes on at full
/// speed.
fn knee_down() -> f32 {
    (1.0 - kneel_at_cut()) * KNEEL
}

impl DesertSet {
    /// The renderer of shot 6 for pictures of `width` x `height` pixels.
    pub fn renderer_line(width: u32, height: u32) -> Renderer {
        Renderer::new(width, height).exposure(2.6).bloom(Bloom {
            strength: 0.85,
            radius: 0.75,
            threshold: 1.0,
        })
    }

    /// Motion blur samples of shot 6: one. The camera moves slowly, and the running light
    /// draws its own streak (the tails of its particles).
    pub fn shutter_line(&self, _t: f32) -> usize {
        1
    }

    /// The pose at `t`: the knee comes down, the hand comes down, draws, and lifts a little.
    /// The root stands on the mud at the origin, and the finger follows the mud of the line.
    fn line_pose(&self, t: f32) -> Pose {
        let root = self.terrain.playa(0.0, 0.0);
        let mud = |u: f32| self.on_line(u).y - root - 0.002;
        let mut pose = if t < TOUCH {
            let kneel = Pose::kneel(kneel_at_cut() + t / KNEEL);
            let reach = smoothstep(knee_down() - 0.15, TOUCH, t);
            kneel.lerp(&Pose::draw(0.0, mud(0.0) + 0.07 * (1.0 - reach), t), reach)
        } else {
            let u = drawn(t);
            Pose::draw(u, mud(u) + 0.035 * smoothstep(LIFT, LIFT + 0.6, t), t)
        }
        .placed(Vec3::ZERO, kneel_heading());
        pose.position.y = root;
        pose
    }

    /// The scene and the camera of shot 6 at `t` seconds, `0..6.5`.
    pub fn moment_line(&self, t: f32) -> (Scene, Camera) {
        let mut scene = self.night.clone();
        self.night_figure.apply(&mut scene, &self.line_pose(t));
        // the sky is a little lighter: the dawn comes
        scene.environment = scene.environment.clone().intensity(1.25);

        let glow = glow_level(t);
        self.place_groove(&mut scene, drawn(t), glow, t);
        self.place_grains(&mut scene, t);
        let run = run_distance(t, self.run_length());
        self.place_run(&mut scene, run, t);

        let tip = self.night_figure.fingertip(&scene);
        let knee = scene.world_position(self.night_figure.joint(Joint::KneeR));
        let camera = self.line_camera(t, tip, knee, run);
        scene[self.stars].position = camera.position;
        self.night_air(
            &mut scene,
            40.0 + sand_time(t),
            sand_rate(t),
            camera.position,
        );
        // The moon is the key light of the close views. It rims the hand and the dust. Its
        // shadow map covers only the figure and the line, so the shadows of the fingers
        // are sharp.
        if let Some(Light::Directional(moon)) = scene[self.moon].light_mut() {
            moon.intensity *= 2.2;
            if let Some(shadow) = &mut moon.shadow {
                shadow.bounds = Some(Aabb {
                    min: vec3(-1.3, -0.3, -1.3),
                    max: vec3(1.3, 1.6, 1.3),
                });
                shadow.softness = 1.5;
            }
        }
        // The glow of the factory on the horizon gives the hand and the dust a cold rim.
        scene.add(Node::light(Light::directional(
            super::COLD,
            0.5,
            ahead() + Vec3::Y * 0.1,
        )));
        if let Object::Particles(motes) = &mut scene[self.line.motes].object {
            motes.items = self.motes(t, glow, knee);
        }
        hide_dark_lights(&mut scene);
        (scene, camera)
    }

    /// The point at the part `u` of the line on the mud, in world space.
    fn on_line(&self, u: f32) -> Vec3 {
        let p = kneel_point(line_point(u));
        p.with_y(self.terrain.playa(p.x, p.z))
    }

    /// The unit direction across the line at the part `u`, horizontal.
    fn across(&self, u: f32) -> Vec3 {
        let along = self.on_line(u + 0.003) - self.on_line(u - 0.003);
        along.with_y(0.0).normalize().cross(Vec3::Y)
    }

    /// The start and the end of the line on the mud, in world space.
    fn line_ends(&self) -> (Vec3, Vec3) {
        (self.on_line(0.0), self.on_line(1.0))
    }

    /// The length of the line in meters.
    fn line_length(&self) -> f32 {
        (0..16)
            .map(|i| {
                let u = i as f32 / 16.0;
                self.on_line(u).distance(self.on_line(u + 1.0 / 16.0))
            })
            .sum()
    }

    /// The distance from the end of the line to the front of the factory, in meters.
    fn run_length(&self) -> f32 {
        let (_, end) = self.line_ends();
        FACTORY_DISTANCE - end.dot(ahead()) - 10.0
    }

    /// Puts the groove of the drawn part `drawn` of the line into `scene`, with the light
    /// that seeps from it. `glow` is the brightness of the glow.
    fn place_groove(&self, scene: &mut Scene, drawn: f32, glow: f32, t: f32) {
        let length = self.line_length();
        let drawn_length = length * drawn;
        if drawn_length < 0.004 {
            return;
        }
        let steps = ((drawn_length / 0.003) as usize).max(2);
        let mut positions = Vec::new();
        let mut uvs = Vec::new();
        let mut colors = Vec::new();
        for i in 0..=steps {
            let u = drawn * i as f32 / steps as f32;
            let s = u * length;
            let (center, side) = (self.on_line(u), self.across(u));
            // shallow where the finger touched down and where it is now
            let taper = smoothstep(0.0, 0.015, s) * smoothstep(0.0, 0.01, drawn_length - s + 0.004);
            for (k, &(across, height)) in PROFILE.iter().enumerate() {
                // loose dust: lumps along the lips and crumbs on them
                let lip = 0.7
                    + 0.45 * noise1(k as u32 * 3 + 1, s * 70.0)
                    + 0.35 * noise1(k as u32 * 3 + 2, s * 380.0);
                let wobble = 0.12 * noise1(k as u32 + 40, s * 35.0);
                let a = across * (0.6 + 0.4 * taper) + wobble * across.abs();
                let p = center + side * (a * HALF_WIDTH);
                let h = 0.0003 + (height * lip - 0.0003) * taper;
                positions.push(p.with_y(self.terrain.playa(p.x, p.z) + h));
                uvs.push(vec2(p.x, p.z));
                // the pressed channel is darker, the loose dust of the lips lighter
                let fresh = if across.abs() < 0.3 {
                    0.62
                } else {
                    1.0 + 0.25 * height / 0.0052
                };
                let tone = playa_tone(vec2(p.x, p.z)) * (1.0 + (fresh - 1.0) * taper);
                colors.push(vec4(tone, tone, tone, 1.0));
            }
        }
        let columns = PROFILE.len() as u32;
        let mut indices = Vec::new();
        for i in 0..steps as u32 {
            for k in 0..columns - 1 {
                let a = i * columns + k;
                let (b, c, d) = (a + 1, a + columns, a + columns + 1);
                indices.extend([a, b, c, b, d, c]);
            }
        }
        let mut geometry = Geometry::new(positions, Vec::new(), uvs, indices);
        geometry.colors = colors;
        set_mesh(scene, self.line.groove, geometry);

        let glowing = length * lit(t);
        if glow <= 0.0 || glowing < 0.004 {
            return;
        }
        // The light in the channel: a hot core at the bottom and deep orange on the inner
        // walls. The newest part flares up where the glow catches the dust.
        let steps = ((glowing / 0.003) as usize).max(2);
        let mut positions = Vec::new();
        let mut colors = Vec::new();
        for i in 0..=steps {
            let s = glowing * i as f32 / steps as f32;
            let u = s / length;
            let (center, side) = (self.on_line(u), self.across(u));
            let front = 1.0 + 1.6 * (-((glowing - s) / 0.03).powi(2)).exp();
            let ends = smoothstep(0.0, 0.012, glowing - s) * smoothstep(0.0, 0.01, s);
            let b = 14.0 * glow * seep(s, t) * front * ends;
            for (across, height, core) in [
                (-0.42, 0.0037, 0.0),
                (-0.3, 0.0030, 0.5),
                (-0.12, 0.0010, 1.0),
                (0.12, 0.0010, 1.0),
                (0.3, 0.0030, 0.5),
                (0.42, 0.0037, 0.0),
            ] {
                let p = center + side * (across * HALF_WIDTH);
                positions.push(p.with_y(self.terrain.playa(p.x, p.z) + height));
                // the core is hot and pale, the walls deep orange
                let hue = vec3(1.0, 0.62, 0.38).lerp(vec3(1.25, 1.15, 1.05), core);
                colors.push((hue * (b * (0.15 + 0.85 * core))).extend(1.0));
            }
        }
        let mut indices = Vec::new();
        for i in 0..steps as u32 {
            for k in 0..5u32 {
                let a = i * 6 + k;
                let (b, c, d) = (a + 1, a + 6, a + 7);
                indices.extend([a, b, c, b, d, c]);
            }
        }
        let normals = vec![Vec3::Y; positions.len()];
        let mut geometry = Geometry::new(positions, normals, Vec::new(), indices);
        geometry.colors = colors;
        set_mesh(scene, self.line.glow, geometry);

        // Lamps along the glowing part light the hand, the sleeve, the knee and the dust.
        // The last lamp is at the front of the glow, near the finger.
        for (i, &id) in self.line.lamps.iter().enumerate() {
            let k = (i as f32 + 0.5) / (LAMPS - 1) as f32;
            // the lamps stay a little behind the front, so the finger does not burn out
            let s = (glowing - 0.03).max(0.0) * k.min(1.0);
            scene[id].position = self.on_line(s / length) + Vec3::Y * 0.025;
            if let Some(Light::Point(light)) = scene[id].light_mut() {
                light.color = Rgb::hex(if i % 2 == 0 { AMBER } else { AMBER_EDGE });
                light.intensity = 0.05 * glow * (0.4 + 0.6 * seep(s, t));
            }
        }
    }

    /// Puts the dust grains near the line into `scene`. The grains in the path of the finger
    /// hop to the sides when it passes.
    fn place_grains(&self, scene: &mut Scene, t: f32) {
        let tip = drawn(t);
        let transforms = (0..GRAINS).map(|i| {
            let h = |n: u32| hash(i * 13 + n * 7919 + 3);
            let u = h(1);
            let rest = (h(2) * 2.0 - 1.0).powi(3) * 0.07;
            // only the grains under the finger move
            let pushed = rest.signum() * (0.014 + 0.03 * h(3).powi(2));
            let (offset, hop) = if tip > u && rest.abs() < 0.012 {
                // the time since the finger passed, in seconds
                let passed = (tip - u) * (DRAW_END - TOUCH);
                let k = 1.0 - (-passed * 14.0).exp();
                let hop = 0.005 * (PI * (passed / 0.18).min(1.0)).sin();
                (rest + (pushed - rest) * k, hop)
            } else {
                (rest, 0.0)
            };
            let p = self.on_line(u) + self.across(u) * offset;
            let size = 0.0005 + 0.0016 * h(4).powi(3);
            let y = self.terrain.playa(p.x, p.z) + size * 0.4 + hop;
            Mat4::from_scale_rotation_translation(
                Vec3::splat(size) * vec3(1.0, 0.7, 1.2),
                Quat::from_rotation_y(h(5) * 6.3) * Quat::from_rotation_x(h(6) * 2.0),
                p.with_y(y),
            )
        });
        let tones = (0..GRAINS).map(|i| {
            let v = 0.9 + 0.35 * hash(i * 5 + 1);
            Rgb::linear(v, v, v)
        });
        let node = &mut scene[self.line.grains];
        node.visible = true;
        if let Some(mesh) = node.mesh_mut() {
            mesh.instances = Some(Arc::new(Instances::new(transforms).colors(tones)));
        }
    }

    /// The small things in the air near the line at `t`: dust that the knee and the finger
    /// lift, grains that glow in the groove, embers that rise from it, and the warm air
    /// above it. `knee` is the right knee joint.
    fn motes(&self, t: f32, glow: f32, knee: Vec3) -> Vec<Particle> {
        let length = self.line_length();
        let wind = super::walk::wind().normalize();
        let mut items = Vec::new();
        // Dust from the knee: a ring of fine dust that spreads and settles.
        let since_knee = t - knee_down();
        if (0.0..2.0).contains(&since_knee) {
            let ground = knee.with_y(self.terrain.playa(knee.x, knee.z));
            for i in 0..90u32 {
                let h = |n: u32| hash(i * 37 + n * 7919 + 21);
                let angle = h(1) * 2.0 * PI;
                let out = vec3(angle.cos(), 0.0, angle.sin());
                let spread = (0.02 + 0.06 * h(2)) * (1.0 - (-since_knee * 4.0).exp());
                let rise = (0.01 + 0.05 * h(3)) * (1.0 - (-since_knee * 3.0).exp());
                let settle = 0.02 * since_knee * since_knee;
                let p = ground
                    + out * (0.04 + spread)
                    + wind * (0.03 * since_knee)
                    + Vec3::Y * (rise - settle).max(0.001);
                let fade = (1.0 - since_knee / 2.0).powi(2);
                // fine grains, and every fifth one a soft puff of dust
                let (size, light) = if i % 5 == 0 {
                    (0.006 + 0.01 * h(4), 0.004)
                } else {
                    (0.0004 + 0.0006 * h(4), 0.12)
                };
                items.push(Particle::new(p, size, Rgb::hex(0x9fb2d6) * (light * fade)));
            }
        }
        // Dust that the finger kicks up as it passes, and that drifts in the wind.
        for i in 0..160u32 {
            let h = |n: u32| hash(i * 29 + n * 104_729 + 11);
            let u = h(1);
            let age = t - (TOUCH + u * (DRAW_END - TOUCH));
            if !(0.0..2.5).contains(&age) {
                continue;
            }
            let kick = (h(2) * 2.0 - 1.0) * (0.01 + 0.03 * (1.0 - (-age * 5.0).exp()));
            let rise = (0.004 + 0.05 * h(3)) * (1.0 - (-age * 2.5).exp());
            let drift = wind * (0.02 * age)
                + vec3(noise1(i, age * 0.8), 0.0, noise1(i + 300, age * 0.8)) * 0.01;
            let p = self.on_line(u) + self.across(u) * kick + drift + Vec3::Y * rise;
            let fade = (1.0 - age / 2.5).powi(2) * smoothstep(0.0, 0.08, age);
            // the moon lights the dust, and the glow warms it near the line
            let warm = Rgb::hex(AMBER) * (0.5 * glow * (-(rise / 0.025)).exp());
            let color = (Rgb::hex(0x9fb2d6) * 0.1 + warm) * fade;
            // fine grains, and every fourth one a soft puff of dust
            let (size, light) = if i % 4 == 0 {
                (0.004 + 0.006 * h(4), 0.04)
            } else {
                (0.0004 + 0.0006 * h(4), 1.0)
            };
            items.push(Particle::new(p, size, color * light));
        }
        let glowing = length * lit(t);
        if glow <= 0.0 || glowing < 0.004 {
            return items;
        }
        // Grains in the groove that glow like embers, and flicker.
        for i in 0..260u32 {
            let h = |n: u32| hash(i * 43 + n * 7919 + 77);
            let s = h(1) * glowing;
            let u = s / length;
            let across = (h(2) * 2.0 - 1.0).powi(3) * HALF_WIDTH * 0.8;
            let p = self.on_line(u) + self.across(u) * across + Vec3::Y * (0.0008 + 0.003 * h(3));
            let flicker = 0.5 + 0.5 * noise1(i + 500, t * (2.0 + 3.0 * h(4)));
            let heat = glow * seep(s, t) * flicker * (1.0 - across.abs() / HALF_WIDTH);
            let color = Rgb::hex(0xff7a1c).lerp(Rgb::hex(0xffd59a), h(5)) * (3.0 * heat);
            items.push(Particle::new(p, 0.0005 + 0.0006 * h(6), color));
        }
        // Embers rise from the groove, cool from yellow to red and go out. Each ember
        // slot starts again after its life.
        for i in 0..140u32 {
            let h = |n: u32| hash(i * 59 + n * 7919 + 131);
            let life = 0.7 + 1.1 * h(1);
            let cycle = (t - IGNITE) / life + h(2);
            let age = cycle.fract() * life;
            // where the ember starts: a place that glowed when it was born
            let s = hash(i * 59 + cycle.floor() as u32 * 977 + 5) * glowing;
            let born_glow = glow_level(t - age) * seep(s, t);
            if t - age < IGNITE || born_glow <= 0.05 {
                continue;
            }
            let u = s / length;
            let speed = 0.04 + 0.08 * h(3);
            let sway = vec3(noise1(i + 700, t * 1.3), 0.0, noise1(i + 900, t * 1.3)) * 0.012;
            let p = self.on_line(u)
                + self.across(u) * ((h(4) * 2.0 - 1.0) * HALF_WIDTH * 0.5)
                + wind * (0.02 * age * age)
                + sway * age
                + Vec3::Y * (0.002 + speed * age);
            let k = age / life;
            let temperature = Rgb::hex(0xffe0a0).lerp(Rgb::hex(0xff5a10), k.sqrt());
            let light = born_glow * (1.0 - k).powi(2) * smoothstep(0.0, 0.05, age);
            let velocity = Vec3::Y * speed + wind * (0.04 * age);
            items.push(
                Particle::new(p, 0.0004 + 0.0004 * h(5), temperature * (4.0 * light))
                    .tail(-velocity * look::SHUTTER),
            );
        }
        // Warm air over the line: soft glows that rise and waver.
        for i in 0..24u32 {
            let h = |n: u32| hash(i * 71 + n * 7919 + 191);
            let life = 1.5 + h(1);
            let age = ((t - IGNITE) / life + h(2)).fract() * life;
            let s = h(3) * glowing;
            let p = self.on_line(s / length)
                + Vec3::Y * (0.01 + 0.05 * age)
                + vec3(noise1(i + 40, t * 2.0), 0.0, noise1(i + 60, t * 2.0)) * 0.006;
            let k = age / life;
            let light = 0.06 * glow * seep(s, t) * (PI * k).sin();
            items.push(Particle::new(
                p,
                0.012 + 0.012 * k,
                Rgb::hex(AMBER_EDGE) * light,
            ));
        }
        items
    }

    /// Puts the running light into `scene`: a bright head `run` meters past the end of the
    /// line, a warm trail of sparks behind it on the ground, and the flash at the factory.
    fn place_run(&self, scene: &mut Scene, run: f32, t: f32) {
        if t < RUN {
            return;
        }
        let (_, end) = self.line_ends();
        let dir = ahead();
        let side = dir.cross(Vec3::Y);
        // The path rises a little with the distance, so the light stays above the bumps of
        // the ground far away.
        let ground = |s: f32| {
            let p = end + dir * s;
            p.with_y(self.terrain.playa(p.x, p.z) + 0.002 + 0.0006 * s)
        };
        // The head: a hot core and a halo, smeared along the path by the motion.
        let arrived = smoothstep(ARRIVAL - 0.08, ARRIVAL, t);
        let speed = (run_distance(t + 0.01, self.run_length()) - run) / 0.01;
        let head = ground(run) + Vec3::Y * (0.004 + 0.004 * run);
        let smear = -dir * speed * look::SHUTTER;
        let fade = 1.0 - arrived;
        let mut items = vec![
            Particle::new(
                head,
                0.004 + 0.0025 * run,
                Rgb::hex(0xffe2b0) * (40.0 * fade),
            )
            .tail(smear),
            Particle::new(head, 0.016 + 0.012 * run, Rgb::hex(AMBER) * (2.5 * fade)).tail(smear),
        ];
        // The trail: a chain of sparks on the ground that fades behind the head. The sparks
        // are dense near the line and sparse far away, and they grow with the distance, so
        // the trail stays a line to the horizon.
        let tail = 0.4 + 0.2 * run;
        for i in 0..=320u32 {
            let s = (run + 1.0).powf(i as f32 / 320.0) - 1.0;
            let b = 0.1 + 0.9 * (-(run - s) / tail).exp();
            let color = Rgb::hex(AMBER) * (6.0 * b * fade.max(0.35));
            items.push(Particle::new(
                ground(s) + Vec3::Y * 0.003,
                0.0018 + 0.0011 * s,
                color,
            ));
        }
        // dust that the light lifts as it passes, warm and fading
        for i in 0..240u32 {
            let h = |n: u32| hash(i * 41 + n * 7919 + 99);
            let s = 0.05 * (run.max(0.06) / 0.05).powf(h(1));
            if s > run {
                continue;
            }
            let age = (run - s) / speed.max(1.0) + 0.05 * h(2);
            let lift = (0.004 + 0.004 * s) * (1.0 + 3.0 * h(4) * (1.0 - (-age * 3.0).exp()));
            let p = ground(s) + side * ((h(3) * 2.0 - 1.0) * (0.02 + 0.01 * s)) + Vec3::Y * lift;
            let warm = Rgb::hex(AMBER_EDGE) * (0.8 * (-age * 2.0).exp());
            items.push(Particle::new(p, 0.001 + 0.0012 * s, warm));
        }
        if let Object::Particles(particles) = &mut scene[self.line.head].object {
            particles.items = items;
        }
        // The light of the head makes a pool of light that keeps its brightness.
        let lift = 0.15 + 0.01 * run;
        scene[self.line.head_light].position = head + Vec3::Y * lift;
        if let Some(Light::Point(light)) = scene[self.line.head_light].light_mut() {
            light.intensity = 3.0 * lift * lift * fade;
            light.distance = 2.0 + 0.3 * run;
        }

        // the flash where the light reaches the factory
        let flash = smoothstep(ARRIVAL - 0.1, ARRIVAL + 0.12, t);
        let bloom = flash * (1.0 + 0.6 * (-(t - ARRIVAL - 0.12).max(0.0) * 4.0).exp());
        if let Some(material) = scene[self.factory.flash].material_mut() {
            // gold rather than orange: over the blue haze orange turns pink
            material.color = Rgb::hex(0xffc060) * (3.2 * bloom);
        }
        if let Some(light) = scene[self.factory.flash_light].light_mut() {
            *light.intensity_mut() = 2.5e5 * bloom;
        }
    }

    /// The camera of shot 6. It starts wide on the dust where the finger will touch, with
    /// the knee that comes down behind. It moves in to a three quarter view of the finger
    /// and follows it along the line. When the line glows, it swings low behind the line
    /// and looks along it to the factory, after the running light.
    fn line_camera(&self, t: f32, tip: Vec3, knee: Vec3, run: f32) -> Camera {
        let (start, end) = self.line_ends();
        let dir = (end - start).with_y(0.0).normalize();
        // the side of the line away from his body
        let open = Vec3::Y.cross(dir);
        let reach = smoothstep(0.2, TOUCH + 0.3, t);
        let swing = smoothstep(DRAW_END - 0.4, RUN + 0.2, t);
        // The camera is on a circle around the place of the finger on the line: an angle
        // from behind the line toward the open side, a distance and a height.
        let pivot = start.lerp(end, drawn(t));
        let angle = 1.13 - 0.21 * reach - 0.77 * swing;
        let distance = 0.83 - 0.08 * reach + 0.85 * swing;
        let height = 0.15 + 0.05 * reach + 0.1 * swing;
        let position =
            pivot + (-dir * angle.cos() + open * angle.sin()) * distance + Vec3::Y * height;

        let dust = knee.with_y(start.y + 0.05).lerp(start + dir * 0.2, 0.35);
        let finger = pivot + dir * 0.04 + Vec3::Y * 0.025;
        // Behind the line the view goes along it, so the line leads to the factory. Then
        // it rises a little with the running light.
        let along = end + ahead() * 1.5;
        let far = end + ahead() * 4.0 + Vec3::Y * 0.12;
        let target = dust
            .lerp(finger, reach)
            .lerp(along, smoothstep(LIFT - 0.4, RUN, t))
            .lerp(far, 0.6 * smoothstep(RUN, ARRIVAL, t));
        let mm = 45.0 + 30.0 * reach - 41.0 * swing;

        // The focus: the knee, then the finger, then the end of the line, then the running
        // light.
        let on_knee = position.distance(knee.with_y(start.y + 0.04));
        let on_finger = position.distance(tip);
        let near = on_knee + (on_finger - on_knee) * smoothstep(knee_down(), TOUCH, t);
        let on_end = position.distance(end);
        let near = near + (on_end - near) * smoothstep(DRAW_END, LIFT + 0.3, t);
        let follow = position.distance(end + ahead() * run).max(near);
        let focus = near + (follow - near) * smoothstep(RUN, RUN + 0.6, t);
        let blur = 8.0 - 3.5 * swing;
        look::lens(mm)
            .position(position)
            .look_at(target)
            .clip(0.01, FAR)
            .focus(focus, blur)
            .shake(t * 0.3, 0.0012 * (1.0 - 0.6 * smoothstep(3.3, 3.8, t)), 9)
    }
}

/// Shows the mesh node `id` with a new geometry.
fn set_mesh(scene: &mut Scene, id: NodeId, geometry: Geometry) {
    let node = &mut scene[id];
    node.visible = true;
    if let Some(mesh) = node.mesh_mut() {
        mesh.geometry = Arc::new(geometry);
    }
}
