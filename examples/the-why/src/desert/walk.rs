//! Shot 5, "The walk": the camera tilts down from the stars to the plateau. The engineer
//! walks away from the factory glow toward the camera, stops and kneels. Time slows.

use fframes_3d::math::{hash, noise1, smoothstep};
use fframes_3d::*;

use super::{DesertSet, FAR, ahead, hide_dark_lights, kneel_heading, right};
use crate::figure::{FigureStyle, Pose};
use crate::look;

/// The forward direction of the camera at the first frame. Shot 4 ends with this view of
/// the stars.
const FIRST_FORWARD: Vec3 = vec3(0.145, 0.906, 0.397);
/// The walking speed in meters per second.
const SPEED: f32 = 1.1;
/// The story time at which he stops and begins to kneel, in seconds.
const STOP: f32 = 8.5;
/// The story time that the kneel takes, in seconds.
const KNEEL: f32 = 1.1;
/// The wind over the plateau in meters per second. It blows from his right to his left.
pub(super) fn wind() -> Vec3 {
    -right() * 7.5 - ahead() * 2.0
}

/// Adds the light of the night before dawn: a deep blue sky, a low moon behind the factory
/// and haze over the plateau. Returns the node of the moon.
pub(super) fn light_night(scene: &mut Scene) -> NodeId {
    scene.environment = Environment::Gradient {
        sky: Rgb::hex(0x060c1e),
        horizon: Rgb::hex(0x1c2c4c),
        ground: Rgb::hex(0x06080c),
        intensity: 1.0,
    };
    scene.background = Background::Environment { blur: 0.0 };
    scene.atmosphere = Some(Atmosphere {
        density: 0.0006,
        color: Rgb::hex(0xc8d8ff),
        base: 0.0,
        falloff: 0.01,
        anisotropy: 0.55,
        ambient: Rgb::hex(0x101a30),
        noise: 0.6,
        noise_size: 18.0,
        offset: Vec3::ZERO,
        distance: 3200.0,
        steps: 32,
    });
    // The moon is low behind the factory, to the left. It gives the rocks and the figure a
    // cold rim and long shadows toward the camera.
    let moon = (ahead() * 0.5 - right() * 1.0 + Vec3::Y * 0.5).normalize();
    let moon = scene.add(Node::light(
        Light::sun(0x9db6e8, 0.95, moon * 100.0).with_shadow(Shadow {
            map_size: 2048,
            softness: 2.5,
            strength: 0.85,
            bias: 1.5,
            bounds: Some(Aabb {
                min: vec3(-30.0, -2.0, -30.0),
                max: vec3(30.0, 6.0, 30.0),
            }),
        }),
    ));
    scene.add(Node::light(Light::hemisphere(0x2a3e66, 0x0a0c10, 0.25)));
    moon
}

/// Dark clothes with a faint cold rim: a silhouette against the factory glow.
pub(super) fn figure_style() -> FigureStyle {
    let rim = Rgb::hex(0x9fb4d8) * 0.05;
    let cloth =
        |color: u32, roughness: f32| Material::standard(color).roughness(roughness).rim(rim, 3.0);
    FigureStyle {
        skin: cloth(0x2a1d17, 0.55),
        jacket: cloth(0x1d2128, 0.85),
        trousers: cloth(0x15171a, 0.9),
        boots: cloth(0x1c1612, 0.6),
        hair: cloth(0x0d0b0a, 0.75),
    }
}

/// The story time at the shot time `t`: from 8.5 s on, time slows to a third.
fn story_time(t: f32) -> f32 {
    // the integral of the rate 1 - 2/3 * smoothstep(STOP, STOP + SLOW, t)
    const SLOW: f32 = 0.7;
    if t <= STOP {
        return t;
    }
    let u = ((t - STOP) / SLOW).min(1.0);
    // integral of smoothstep from 0 to u, in units of SLOW
    let ramp = u * u * u - 0.5 * u * u * u * u;
    let slowed = SLOW * (u - 2.0 / 3.0 * ramp);
    let after = (t - STOP - SLOW).max(0.0) / 3.0;
    STOP + slowed + after
}

/// The pose of the engineer at the story time `tau`: he walks to the origin and kneels.
fn pose(tau: f32) -> Pose {
    let heading = kneel_heading();
    let facing = vec3(heading.sin(), 0.0, heading.cos());
    let start = -facing * SPEED * STOP;
    let walk_at =
        |tau: f32| Pose::walk(tau * SPEED / Pose::stride(SPEED), SPEED).placed(start, heading);
    if tau <= STOP {
        return walk_at(tau);
    }
    // The last step blends into the kneel.
    let blend = smoothstep(STOP, STOP + 0.45, tau);
    let kneel = Pose::kneel((tau - STOP) / KNEEL).placed(Vec3::ZERO, heading);
    walk_at(STOP + 0.25 * blend).lerp(&kneel, blend)
}

impl DesertSet {
    /// The renderer of shot 5 for pictures of `width` x `height` pixels.
    pub fn renderer_walk(width: u32, height: u32) -> Renderer {
        Renderer::new(width, height).exposure(2.2).bloom(Bloom {
            strength: 0.7,
            radius: 0.7,
            threshold: 1.0,
        })
    }

    /// Motion blur samples of shot 5: the tilt is fast in its middle.
    pub fn shutter_walk(&self, t: f32) -> usize {
        if (0.6..2.5).contains(&t) { 3 } else { 1 }
    }

    /// The scene and the camera of shot 5 at `t` seconds, `0..10`.
    pub fn moment_walk(&self, t: f32) -> (Scene, Camera) {
        let tau = story_time(t);
        let mut scene = self.night.clone();
        let pose = pose(tau);
        self.night_figure.apply(&mut scene, &pose);
        let camera = walk_camera(t, pose.position);
        scene[self.stars].position = camera.position;
        let rate = (story_time(t + 0.01) - story_time(t)) / 0.01;
        self.night_air(&mut scene, tau, rate, camera.position);
        hide_dark_lights(&mut scene);
        (scene, camera)
    }

    /// The wind, the sand and the blinking lights of the night at the story time `tau`.
    /// `rate` is the speed of the story time: below 1 in slow motion.
    pub(super) fn night_air(&self, scene: &mut Scene, tau: f32, rate: f32, eye: Vec3) {
        let wind = wind();
        if let Some(air) = &mut scene.atmosphere {
            air.offset = -wind * tau * 0.6;
        }
        if let Object::Particles(sand) = &mut scene[self.dust].object {
            sand.items = self.blowing_sand(eye.with_y(0.0) + ahead() * 10.0, eye, tau, rate);
        }
        // the beacons on the stacks blink once in 2 s
        let blink = smoothstep(0.0, 0.08, (tau * 0.5).fract())
            * (1.0 - smoothstep(0.25, 0.4, (tau * 0.5).fract()));
        if let Object::Particles(beacons) = &mut scene[self.factory.beacons].object {
            for p in &mut beacons.items {
                p.opacity = 0.15 + 0.85 * blink;
            }
        }
    }

    /// Sand that the wind blows over the ground near `center` at the story time `tau`.
    /// The grains run in narrow streams that wind over the ground, and gusts make the
    /// streams brighter and darker. `eye` is the camera position, `rate` the speed of the
    /// story time.
    pub(super) fn blowing_sand(
        &self,
        center: Vec3,
        eye: Vec3,
        tau: f32,
        rate: f32,
    ) -> Vec<Particle> {
        let wind = wind();
        let speed = wind.length();
        let along = wind / speed;
        let across = along.cross(Vec3::Y);
        let half = 18.0;
        let mut items = Vec::with_capacity(STREAMS as usize * GRAINS as usize);
        for s in 0..STREAMS {
            let h = |n: u32| hash(s * 31 + n * 7919 + 5);
            let offset = (h(1) * 2.0 - 1.0) * half;
            let pace = 0.7 + 0.5 * h(2);
            for g in 0..GRAINS {
                let k = |n: u32| hash(s * 977 + g * 13 + n * 104_729);
                // the distance along the stream, moving with the wind and wrapped in the box
                let x = (k(1) * 2.0 * half + speed * pace * (0.85 + 0.3 * k(2)) * tau)
                    .rem_euclid(2.0 * half)
                    - half;
                let meander =
                    0.6 * noise1(s + 7, x * 0.18 + h(3) * 40.0) + 0.04 * (k(3) * 2.0 - 1.0);
                let p = center + along * x + across * (offset + meander);
                let lift =
                    0.015 + 0.07 * k(4).powi(3) + 0.03 * (tau * 9.0 + k(5) * 30.0).sin().abs();
                let position = vec3(p.x, self.terrain.height(p.x, p.z) + lift, p.z);
                // gusts run along the stream, and the box edges fade out
                let gust = (0.5 + 0.5 * noise1(s + 101, x * 0.25 - tau * 2.0)).powi(2);
                let edge = 1.0 - smoothstep(half * 0.6, half, x.abs().max(offset.abs()));
                // sand in front of the factory glow scatters its light toward the camera
                let toward = (position - eye).normalize_or_zero().dot(ahead()).max(0.0);
                let light = (0.06 + 0.7 * toward.powi(12)) * gust * edge;
                let color = Rgb::hex(0xa8c0e8) * (light * (0.4 + 0.6 * k(6)));
                items.push(
                    Particle::new(position, 0.002 + 0.003 * k(7), color)
                        .tail(-wind * (pace * rate) * look::SHUTTER),
                );
            }
            // Fine sand drifts in soft bands over the first streams: small puffs, close
            // together, so they blend into one flowing veil.
            if s >= VEILS {
                continue;
            }
            for g in 0..PUFFS {
                let k = |n: u32| hash(s * 541 + g * 17 + n * 15_485_863);
                let x = (g as f32 / PUFFS as f32 * 2.0 * half + speed * pace * 0.8 * tau)
                    .rem_euclid(2.0 * half)
                    - half;
                let meander =
                    0.6 * noise1(s + 7, x * 0.18 + h(3) * 40.0) + 0.25 * (k(1) * 2.0 - 1.0);
                let p = center + along * x + across * (offset + meander);
                let lift = 0.04 + 0.22 * k(2).powi(2);
                let position = vec3(p.x, self.terrain.height(p.x, p.z) + lift, p.z);
                let gust = (0.5 + 0.5 * noise1(s + 101, x * 0.25 - tau * 2.0)).powi(3);
                let edge = 1.0 - smoothstep(half * 0.5, half, x.abs().max(offset.abs()));
                // puffs right in front of the lens would blur into large blobs
                let near = smoothstep(2.0, 6.0, position.distance(eye));
                let toward = (position - eye).normalize_or_zero().dot(ahead()).max(0.0);
                let light = (0.05 + 0.6 * toward.powi(12)) * gust * edge * near;
                let color = Rgb::hex(0x9ab4e0) * (0.06 * light * (0.5 + k(3)));
                items.push(
                    Particle::new(position, 0.12 + 0.16 * k(4), color)
                        .tail(-wind * (0.012 * (0.4 + 0.6 * rate))),
                );
            }
        }
        items
    }
}

/// The number of streams of blowing sand.
const STREAMS: u32 = 72;
/// The grains in each stream.
const GRAINS: u32 = 56;
/// The streams that also carry a veil of fine sand.
const VEILS: u32 = 34;
/// The puffs of a veil.
const PUFFS: u32 = 110;

/// The camera of shot 5 at `t`. `body` is the root of the figure.
fn walk_camera(t: f32, body: Vec3) -> Camera {
    let (a, r) = (ahead(), right());
    let first = FIRST_FORWARD.normalize();
    // 0 to 3 s: the camera stands low and tilts from the stars to the horizon.
    let start = -a * 8.5 + r * 3.6 + Vec3::Y * 1.0;
    let horizon = (a + Vec3::Y * 0.035).normalize();
    let tilt = smoothstep(0.0, 3.0, t);
    // The tilt turns the view in the vertical plane of the first view, so the stars and
    // the horizon line stay level.
    let elevation = |d: Vec3| d.y.asin();
    let e = elevation(first) + (elevation(horizon) - elevation(first)) * tilt;
    let flat = vec3(first.x, 0.0, first.z).normalize();
    let tilted = flat * e.cos() + Vec3::Y * e.sin();

    // 3 to 8.5 s: a slow track to the left keeps him in front of the factory glow.
    let track = smoothstep(2.4, 9.0, t);
    let along = start.lerp(-a * 6.8 + r * 0.9 + Vec3::Y * 0.8, track);
    let chest = body + Vec3::Y * 1.05;
    let follow = smoothstep(2.6, 5.5, t);

    // 8.5 to 10 s: a low orbit around him begins.
    let orbit = smoothstep(8.3, 10.6, t);
    let angle = (-a).x.atan2((-a).z) + 0.45 * orbit;
    let around =
        vec3(angle.sin(), 0.0, angle.cos()) * (6.8 - 1.0 * orbit) + Vec3::Y * (0.8 - 0.3 * orbit);
    let position = along.lerp(around, smoothstep(8.3, 9.4, t));
    let kneel_target = Vec3::Y * 0.9;
    let target = chest.lerp(kneel_target, smoothstep(8.4, 9.8, t));

    let view = tilted
        .lerp((target - position).normalize(), follow)
        .normalize();
    // The lens goes from 35 mm to 48 mm while he comes closer.
    let mm = 35.0 + 13.0 * smoothstep(3.0, 9.0, t);
    let focus = position.distance(target).max(1.0);
    let blur = 2.0 + 3.0 * smoothstep(3.0, 9.0, t);
    let camera = look::lens(mm)
        .position(position)
        .look_at(position + view)
        .clip(0.1, FAR)
        .focus(focus, blur);
    // a little hand-held drift after the tilt, none on the first frame
    let drift = 0.0025 * smoothstep(1.0, 3.0, t);
    camera.shake(t * 0.4, drift, 5)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn time_slows_to_a_third() {
        assert_eq!(story_time(4.0), 4.0);
        let rate = (story_time(9.95) - story_time(9.9)) / 0.05;
        assert!((rate - 1.0 / 3.0).abs() < 0.01, "{rate}");
        let near = (story_time(8.51) - story_time(8.5)) / 0.01;
        assert!((near - 1.0).abs() < 0.02, "{near}");
    }

    #[test]
    fn first_frame_looks_at_the_stars_of_shot_4() {
        let camera = walk_camera(0.0, Vec3::ZERO);
        let expected = FIRST_FORWARD.normalize();
        assert!(
            camera.forward().distance(expected) < 1e-4,
            "{:?}",
            camera.forward()
        );
    }
}
