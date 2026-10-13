//! Shot 6, "The line": close on the dust in front of his knee. He draws a straight line with
//! the index finger. The groove begins to glow warm. After one still breath, the light of
//! the line runs across the desert into the factory.

use std::f32::consts::PI;
use std::sync::Arc;

use fframes_3d::math::{hash, noise1, smoothstep};
use fframes_3d::*;

use super::terrain::{dirt, playa_tone};
use super::{
    AMBER, AMBER_EDGE, DesertSet, FACTORY_DISTANCE, FAR, LINE_SECONDS, ahead, hide_dark_lights,
    kneel_heading, kneel_point,
};
use crate::assets::Assets;
use crate::figure::{LINE_END, LINE_START, Pose};
use crate::look;

/// Half the width of the groove from lip to lip, in meters.
const HALF_WIDTH: f32 = 0.014;
/// The number of lamps along the line.
const LAMPS: usize = 5;
/// The number of dust grains near the line. The finger pushes aside those in its path.
const GRAINS: u32 = 320;
/// The times when the finger touches the dust and when it stops, in seconds.
const DRAW: (f32, f32) = (0.6, 3.5);
/// The time when the glow begins.
const GLOW: f32 = 2.5;
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
            Geometry::icosahedron(1.0, 1),
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
    smoothstep(DRAW.0, DRAW.1, t)
}

/// The brightness of the glow at `t`: it grows from 2.5 s, pulses once at 4 s, and stays
/// when the light runs away.
fn glow_level(t: f32) -> f32 {
    let grow = smoothstep(GLOW, BREATH - 0.3, t);
    let pulse = (-((t - BREATH) / 0.22).powi(2)).exp();
    grow * (1.0 + 0.9 * pulse)
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

/// The pose at `t`: he kneels, the hand comes down, draws and stays on the ground.
fn pose(t: f32) -> Pose {
    let pose = if t < DRAW.0 {
        Pose::kneel(1.0).lerp(&Pose::draw(0.0), smoothstep(0.0, DRAW.0, t))
    } else {
        Pose::draw(drawn(t))
    };
    pose.placed(Vec3::ZERO, kneel_heading())
}

impl DesertSet {
    /// The renderer of shot 6 for pictures of `width` x `height` pixels.
    pub fn renderer_line(width: u32, height: u32) -> Renderer {
        Renderer::new(width, height).exposure(2.4).bloom(Bloom {
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

    /// The scene and the camera of shot 6 at `t` seconds, `0..6.5`.
    pub fn moment_line(&self, t: f32) -> (Scene, Camera) {
        let mut scene = self.night.clone();
        self.night_figure.apply(&mut scene, &pose(t));
        // the sky is a little lighter: the dawn comes
        scene.environment = scene.environment.clone().intensity(1.25);

        let glow = glow_level(t);
        self.place_groove(
            &mut scene,
            drawn(t),
            glow,
            smoothstep(GLOW, GLOW + 0.9, t),
            t,
        );
        self.place_grains(&mut scene, t);
        let run = run_distance(t, self.run_length());
        self.place_run(&mut scene, run, t);

        let tip = self.night_figure.fingertip(&scene);
        let camera = self.line_camera(t, tip, run);
        scene[self.stars].position = camera.position;
        self.night_air(
            &mut scene,
            40.0 + sand_time(t),
            sand_rate(t),
            camera.position,
        );
        // the moon gives the dust more shape in the close views
        if let Some(moon) = scene[self.moon].light_mut() {
            *moon.intensity_mut() *= 1.6;
        }
        if let Object::Particles(motes) = &mut scene[self.line.motes].object {
            motes.items = self.motes(t, glow);
        }
        hide_dark_lights(&mut scene);
        (scene, camera)
    }

    /// The start and the end of the line on the mud, in world space.
    fn line_ends(&self) -> (Vec3, Vec3) {
        let on_mud = |p: Vec3| p.with_y(self.terrain.playa(p.x, p.z));
        (
            on_mud(kneel_point(LINE_START)),
            on_mud(kneel_point(LINE_END)),
        )
    }

    /// The distance from the end of the line to the front of the factory, in meters.
    fn run_length(&self) -> f32 {
        let (_, end) = self.line_ends();
        FACTORY_DISTANCE - end.dot(ahead()) - 10.0
    }

    /// Puts the groove of the drawn part `drawn` of the line into `scene`, with its glow.
    /// `glow` is the brightness of the glow, `lit` the part of the groove that glows.
    fn place_groove(&self, scene: &mut Scene, drawn: f32, glow: f32, lit: f32, t: f32) {
        let (start, end) = self.line_ends();
        let length = start.distance(end) * drawn;
        if length < 0.004 {
            return;
        }
        let dir = (end - start).normalize();
        let side = dir.cross(Vec3::Y).normalize();
        let steps = ((length / 0.003) as usize).max(2);
        let mut positions = Vec::new();
        let mut uvs = Vec::new();
        let mut colors = Vec::new();
        for i in 0..=steps {
            let s = length * i as f32 / steps as f32;
            // shallow where the finger touched down and where it is now
            let taper = smoothstep(0.0, 0.015, s) * smoothstep(0.0, 0.01, length - s + 0.004);
            for (k, &(across, height)) in PROFILE.iter().enumerate() {
                let lip = 0.7 + 0.6 * noise1(k as u32 * 3 + 1, s * 70.0);
                let wobble = 0.12 * noise1(k as u32 + 40, s * 35.0);
                let a = across * (0.6 + 0.4 * taper) + wobble * across.abs();
                let p = start + dir * s + side * (a * HALF_WIDTH);
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

        if glow <= 0.0 {
            return;
        }
        // The glow runs from the start of the line toward the finger: a hot core in the
        // channel that fades out over the lips.
        let glowing = length * lit;
        let steps = ((glowing / 0.004) as usize).max(2);
        let mut positions = Vec::new();
        let mut colors = Vec::new();
        for i in 0..=steps {
            let s = glowing * i as f32 / steps as f32;
            let flicker = 0.85 + 0.15 * noise1(7, s * 30.0 - t * 2.5);
            let ends = smoothstep(0.0, 0.02, glowing - s) * smoothstep(0.0, 0.01, s);
            // the glow follows the inner walls of the groove, so it shows from low views
            for (across, height, core) in [
                (-0.42, 0.0037, 0.0),
                (-0.3, 0.0030, 0.55),
                (-0.15, 0.0011, 1.0),
                (0.15, 0.0011, 1.0),
                (0.3, 0.0030, 0.55),
                (0.42, 0.0037, 0.0),
            ] {
                let p = start + dir * s + side * (across * HALF_WIDTH);
                positions.push(p.with_y(self.terrain.playa(p.x, p.z) + height));
                let b = 16.0 * core * glow * flicker * ends;
                colors.push(vec4(b, b, b, 1.0));
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

        // Lamps along the glowing part light the hand, the lips and the dust.
        for (i, &id) in self.line.lamps.iter().enumerate() {
            let s = glowing * (i as f32 + 0.5) / LAMPS as f32;
            scene[id].position = start + dir * s + Vec3::Y * 0.04;
            if let Some(light) = scene[id].light_mut() {
                *light.intensity_mut() = 0.016 * glow * lit;
            }
        }
    }

    /// Puts the dust grains near the line into `scene`. The grains in the path of the finger
    /// hop to the sides when it passes.
    fn place_grains(&self, scene: &mut Scene, t: f32) {
        let (start, end) = self.line_ends();
        let length = start.distance(end);
        let dir = (end - start).normalize();
        let side = dir.cross(Vec3::Y).normalize();
        let tip = drawn(t);
        let transforms = (0..GRAINS).map(|i| {
            let h = |n: u32| hash(i * 13 + n * 7919 + 3);
            let s = h(1) * 1.1 - 0.05;
            let rest = (h(2) * 2.0 - 1.0) * 0.07;
            // only the grains under the finger move
            let pushed = rest.signum() * (0.014 + 0.03 * h(3).powi(2));
            let (offset, hop) = if tip > s && rest.abs() < 0.012 && s >= 0.0 {
                // the time since the finger passed, in seconds
                let passed = (tip - s) * (DRAW.1 - DRAW.0);
                let k = 1.0 - (-passed * 14.0).exp();
                let hop = 0.005 * (PI * (passed / 0.18).min(1.0)).sin();
                (rest + (pushed - rest) * k, hop)
            } else {
                (rest, 0.0)
            };
            let p = start + dir * (s * length) + side * offset;
            let size = 0.0008 + 0.0018 * h(4).powi(3);
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

    /// Fine dust that the finger lifts. It floats in the light of the glow. Soft sprites
    /// over the groove make the glow of the air.
    fn motes(&self, t: f32, glow: f32) -> Vec<Particle> {
        let (start, end) = self.line_ends();
        let dir = (end - start).normalize();
        let length = start.distance(end);
        let tip = drawn(t);
        let lit = smoothstep(GLOW, GLOW + 0.9, t) * tip;
        let wind = super::walk::wind().normalize();
        let halo = (0..12).map(|i| {
            let s = length * lit * (i as f32 + 0.5) / 12.0;
            let p = start + dir * s + Vec3::Y * 0.012;
            Particle::new(p, 0.05, Rgb::hex(AMBER_EDGE) * (0.05 * glow))
        });
        (0..160u32)
            .filter_map(|i| {
                let h = |n: u32| hash(i * 29 + n * 104_729 + 11);
                let s = h(1);
                if tip <= s {
                    return None;
                }
                let age = (tip - s) * (DRAW.1 - DRAW.0) + (t - DRAW.1).max(0.0);
                let rise = 0.004 + 0.06 * h(2) * (1.0 - (-age * 1.5).exp());
                let drift = vec3(noise1(i, age * 0.6), 0.0, noise1(i + 300, age * 0.6)) * 0.02
                    + wind * 0.012 * age;
                let p = start + dir * (s * length) + drift + Vec3::Y * rise;
                let fade = (-age * 0.5).exp();
                let cold = Rgb::hex(0x8fa8d0) * 0.03;
                let warm = Rgb::hex(AMBER) * (0.25 * glow * (-(rise / 0.03)).exp());
                Some(Particle::new(
                    p,
                    0.0006 + 0.0009 * h(3),
                    (cold + warm) * fade,
                ))
            })
            .chain(halo)
            .collect()
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

    /// The camera of shot 6. It orbits low and close around the start of the line on the
    /// side away from his body and follows the finger. Then it stays behind the line and
    /// looks along it to the factory.
    fn line_camera(&self, t: f32, tip: Vec3, run: f32) -> Camera {
        let (start, end) = self.line_ends();
        let dir = (end - start).normalize();
        // the side of the line away from his body
        let open = Vec3::Y.cross(dir);
        // The angle from behind the line toward the open side. The orbit keeps the speed
        // that it had at the end of shot 5 and stops when the finger stops. The camera stays
        // still in the breath.
        let u = (t / DRAW.1).min(1.0);
        let settle = u * (2.0 - u);
        let angle = 1.35 - 1.02 * settle - 0.03 * smoothstep(RUN, LINE_SECONDS, t);
        let distance = 1.05 + 0.75 * settle;
        let height = 0.17 + 0.07 * settle;
        let position =
            start + (-dir * angle.cos() + open * angle.sin()) * distance + Vec3::Y * height;

        // first the dust in front of his knee, then the finger, then the line and the
        // horizon beyond it
        let dust = start + (end - start) * 0.15;
        let finger = dust.lerp(tip.with_y(start.y + 0.015), smoothstep(0.3, 1.2, t));
        let far = end + ahead() * 4.0 + Vec3::Y * 0.12;
        let target = finger.lerp(far, smoothstep(2.5, DRAW.1, t));

        let mm = 100.0 - 40.0 * smoothstep(0.4, DRAW.1, t);
        // The focus follows the finger, then the end of the line, then the running light.
        let near = position.distance(finger.lerp(end, smoothstep(2.6, DRAW.1, t)));
        let follow = position.distance(end + ahead() * run).max(near);
        let focus = near + (follow - near) * smoothstep(RUN, RUN + 0.6, t);
        let blur = 14.0 - 7.0 * smoothstep(2.6, DRAW.1, t);
        look::lens(mm)
            .position(position)
            .look_at(target)
            .clip(0.01, FAR)
            .focus(focus, blur)
            .shake(t * 0.3, 0.001 * (1.0 - smoothstep(3.3, 3.6, t)), 9)
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
