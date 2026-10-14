//! Shot 5, "The walk": the camera tilts down from the stars to the plateau. The engineer
//! walks away from the factory glow toward the camera, stops and kneels. Time slows.

use fframes_3d::math::{hash, noise1, smoothstep};
use fframes_3d::*;

use super::{DesertSet, FAR, WALK_SECONDS, ahead, hide_dark_lights, kneel_heading, right};
use crate::figure::{FigureStyle, KNEEL_CONTACTS, Pose, RIGHT};
use crate::look;

/// The forward direction of the camera at the first frame. Shot 4 ends with this view of
/// the stars.
const FIRST_FORWARD: Vec3 = vec3(0.145, 0.906, 0.397);
/// The walking speed in meters per second: slow and tired.
const SPEED: f32 = 1.0;
/// The story time at which he stops and begins to kneel, in seconds.
const STOP: f32 = 8.5;
/// The story time that the kneel takes, in seconds.
pub(super) const KNEEL: f32 = 1.1;
/// The wind over the plateau in meters per second. It blows from his right to his left.
pub(super) fn wind() -> Vec3 {
    -right() * 7.5 - ahead() * 2.0
}

/// Adds the light of the night before dawn: a deep blue sky, a low moon behind the factory
/// and haze over the plateau. Returns the node of the moon.
pub(super) fn light_night(scene: &mut Scene) -> NodeId {
    scene.environment = Environment::Gradient {
        // the zenith is as dark as the end of shot 4, which tilts up into the same stars
        sky: Rgb::hex(0x040a1a),
        horizon: Rgb::hex(0x1c2c4c),
        ground: Rgb::hex(0x06080c),
        intensity: 1.0,
    };
    scene.background = Background::Environment { blur: 0.0 };
    // Haze lies low over the ground: dense at the ground and thin at 1 m. Far away it
    // hides the foot of the factory and glows in its light.
    scene.atmosphere = Some(Atmosphere {
        density: 0.03,
        color: Rgb::hex(0xc8d8ff),
        base: 0.0,
        falloff: 2.0,
        anisotropy: 0.6,
        ambient: Rgb::hex(0x16243f),
        noise: 1.0,
        noise_size: 8.0,
        offset: Vec3::ZERO,
        distance: 3200.0,
        steps: 32,
    });
    // The air between the plateau and the horizon: the mesas and the far land fade into
    // the sky, and the factory loses some of its contrast at 2.5 km.
    scene.fog = Some(Fog::Exp2 {
        color: Rgb::hex(0x1c2c4c) * 0.9,
        density: 1.0 / 6500.0,
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
        // Little light of the sky reaches the face, so it stays dark. The skin is matte:
        // a gloss makes the hand of shot 6 look like plastic under the lamps.
        skin: cloth(0x5c4a42, 0.82).env_intensity(0.3),
        // Cloth is matte: little of the sky reflects from it at grazing angles.
        jacket: cloth(0x2a3038, 0.85).env_intensity(0.5),
        // The legs have no rim: in the close views of shot 6 it outlines them.
        trousers: Material::standard(0x2c2924)
            .roughness(0.9)
            .env_intensity(0.5),
        boots: Material::standard(0x2a1e16)
            .roughness(0.75)
            .env_intensity(0.5),
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
    let walk = |tau: f32| Pose::walk(walk_phase(tau), SPEED);
    // The walk ends with the right foot planted where the kneel starts, so the foot does
    // not slide in the blend.
    let last = walk(STOP);
    let offset = Pose::kneel(0.0).heel(RIGHT) - last.heel(RIGHT) - last.position;
    let start = Quat::from_rotation_y(heading) * offset;
    let walk_at = |tau: f32| walk(tau).placed(start, heading);
    if tau <= STOP {
        return walk_at(tau);
    }
    // The last step blends into the kneel. The blend ends before the left foot of the
    // kneel touches down.
    let blend = smoothstep(STOP, STOP + 0.3 * KNEEL, tau);
    let kneel = Pose::kneel((tau - STOP) / KNEEL).placed(Vec3::ZERO, heading);
    walk_at(STOP + 0.2 * blend).lerp(&kneel, blend)
}

/// The part of [`Pose::kneel`] at the end of the shot. Shot 6 goes on from it.
pub(super) fn kneel_at_cut() -> f32 {
    (story_time(WALK_SECONDS) - STOP) / KNEEL
}

/// The gait cycles of the walk at the story time `tau`, see [`Pose::walk`]. The pace
/// drifts a little, so the steps do not come like clockwork. At [`STOP`] the right heel
/// has just struck: the kneel starts on that foot.
fn walk_phase(tau: f32) -> f32 {
    let drift = |tau: f32| 0.04 * noise1(77, 0.45 * tau);
    let end = 7.16;
    let rate = (end - drift(STOP) + drift(0.0)) / STOP;
    tau * rate + drift(tau) - drift(0.0)
}

/// The time at which `f`, a function that grows with time, reaches `value`. It searches
/// from 0 to `high` seconds.
fn solve(f: impl Fn(f32) -> f32, value: f32, high: f32) -> f32 {
    let (mut low, mut high) = (0.0, high);
    for _ in 0..40 {
        let mid = f32::midpoint(low, high);
        if f(mid) < value {
            low = mid;
        } else {
            high = mid;
        }
    }
    high
}

/// The shot times in seconds when a foot of the engineer touches the ground in shot 5, for
/// the sounds of the steps: the heel strikes of the walk, then the left foot, the right foot
/// and the right knee of the kneel. The times after the end of the shot are left out.
pub fn footfalls() -> Vec<f32> {
    // The right heel strikes at each whole cycle, the left heel half a cycle later.
    let first = (2.0 * walk_phase(0.0)).floor() as i32 + 1;
    let last = (2.0 * walk_phase(STOP)).floor() as i32;
    let walk = (first..=last).map(|k| solve(walk_phase, 0.5 * k as f32, STOP));
    let kneel = KNEEL_CONTACTS.map(|k| STOP + k * KNEEL);
    walk.chain(kneel)
        .map(|tau| solve(story_time, tau, WALK_SECONDS + 10.0))
        .filter(|&t| t < WALK_SECONDS)
        .collect()
}

/// The meteor: a slow one that crosses the stars in 0.9 s, from 0.35 s on. `eye` is the
/// camera position.
fn meteor_at(eye: Vec3, t: f32) -> Vec<Particle> {
    const START: f32 = 0.35;
    const LENGTH: f32 = 0.9;
    let u = (t - START) / LENGTH;
    if !(0.0..1.0).contains(&u) {
        return Vec::new();
    }
    // It crosses the middle of the view of its time, from the upper right to the left.
    let view = walk_camera(START + 0.5 * LENGTH, Vec3::ZERO);
    let (forward, side, up) = (view.forward(), view.right(), view.up_vector());
    let from = (forward + side * 0.34 + up * 0.12).normalize();
    let to = (forward - side * 0.02 - up * 0.1).normalize();
    let at = |u: f32| eye + from.slerp(to, u) * 6000.0;
    // It flares up near its end and burns out.
    let light = smoothstep(0.0, 0.15, u)
        * (1.0 - smoothstep(0.8, 1.0, u))
        * (1.0 + 1.5 * smoothstep(0.55, 0.75, u));
    let mut items = vec![
        Particle::new(at(u), 2.4, Rgb::hex(0xe8fff0) * (12.0 * light)).tail(at(u - 0.012) - at(u)),
        Particle::new(at(u), 10.0, Rgb::hex(0xb8ffd8) * (0.35 * light)),
    ];
    // The glowing trail fades behind the head.
    for k in 1..=30 {
        let back = u - 0.008 * k as f32;
        if back < 0.0 {
            break;
        }
        let fade = (1.0 - k as f32 / 30.0).powi(2);
        items.push(
            Particle::new(at(back), 1.4, Rgb::hex(0xb8ffd8) * (1.2 * light * fade))
                .tail(at(back - 0.008) - at(back)),
        );
    }
    items
}

/// The rotation from the space of `camera` to the world.
fn orientation(camera: &Camera) -> Quat {
    Quat::from_mat3(&Mat3::from_cols(
        camera.right(),
        camera.up_vector(),
        -camera.forward(),
    ))
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
        let mut pose = pose(tau);
        pose.position.y = self.terrain.playa(pose.position.x, pose.position.z);
        self.night_figure.apply(&mut scene, &pose);
        let camera = walk_camera(t, pose.position);
        scene[self.stars].position = camera.position;
        // At the cut the stars are as bright as at the end of shot 4: that shot has 2.2 times
        // less exposure and brightens its stars by 2.2 in the tilt. In the tilt each sample
        // of the motion blur draws the stars as streaks, so they do not break into dots.
        let samples = self.shutter_walk(t);
        let next = walk_camera(t + look::SHUTTER / samples as f32, pose.position);
        let turn = orientation(&camera) * orientation(&next).inverse();
        if let Object::Particles(sky) = &mut scene[self.stars].object {
            for p in &mut sky.items {
                if samples > 1 {
                    p.tail = turn * p.position - p.position;
                }
            }
        }
        if let Object::Particles(meteor) = &mut scene[self.meteor].object {
            meteor.items = meteor_at(camera.position, t);
        }
        // The glow of the factory behind him rims his outline: the hair, the shoulders and
        // the arms. The close views of shot 6 keep the thin rim of the style.
        self.night_figure
            .set_rim(&mut scene, Rgb::hex(0xb8ccf0) * 0.32, 4.0);
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
                let light = (0.03 + 0.5 * toward.powi(12)) * gust * edge;
                let color = Rgb::hex(0xa8c0e8) * (light * (0.4 + 0.6 * k(6)));
                items.push(
                    Particle::new(position, 0.002 + 0.003 * k(7), color)
                        .tail(-wind * (pace * rate) * look::SHUTTER),
                );
            }
        }
        // Thin ribbons of fine sand snake over the ground. Each ribbon is a chain of
        // streaks that moves with the wind. Its shape changes slowly, and gusts run along
        // it.
        for r in 0..RIBBONS {
            let h = |n: u32| hash(r * 53 + n * 7919 + 17);
            let offset = (h(1) * 2.0 - 1.0) * half;
            let length = 2.5 + 6.0 * h(2);
            let head = (h(3) * 2.0 * half + speed * (0.8 + 0.2 * h(4)) * tau)
                .rem_euclid(2.0 * half + length)
                - half;
            let lift = 0.008 + 0.03 * h(5);
            let points: Vec<Vec3> = (0..=SEGMENTS)
                .map(|k| {
                    let x = head - length * k as f32 / SEGMENTS as f32;
                    let meander = 0.45 * noise1(r + 3, x * 0.3 - tau * 0.5)
                        + 0.1 * noise1(r + 9, x * 1.4 + tau * 1.5);
                    let p = center + along * x + across * (offset + meander);
                    let flutter = 0.012 * noise1(r + 11, x * 2.5 - tau * 4.0).abs();
                    p.with_y(self.terrain.height(p.x, p.z) + lift + flutter)
                })
                .collect();
            for (k, pair) in points.windows(2).enumerate() {
                let u = (k as f32 + 0.5) / SEGMENTS as f32;
                let x = head - length * u;
                let taper = (std::f32::consts::PI * u).sin();
                let gust = (0.5 + 0.5 * noise1(r + 101, x * 0.4 - tau * 1.2)).powi(2);
                let edge = 1.0 - smoothstep(half * 0.6, half, x.abs().max(offset.abs()));
                let near = smoothstep(1.5, 4.0, pair[0].distance(eye));
                let toward = (pair[0] - eye).normalize_or_zero().dot(ahead()).max(0.0);
                let light = (0.05 + 0.5 * toward.powi(10)) * taper * gust * edge * near;
                items.push(
                    Particle::new(pair[0], 0.003 + 0.004 * taper, Rgb::hex(0xa8c0e8) * light)
                        .tail(pair[1] - pair[0]),
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
/// The number of ribbons of fine sand.
const RIBBONS: u32 = 70;
/// The streaks of a ribbon.
const SEGMENTS: u32 = 24;

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

    #[test]
    fn planted_feet_do_not_slide() {
        let mut scene = Scene::new();
        let figure = crate::figure::Figure::build(&mut scene, None, &figure_style());
        let soles = |t: f32| {
            let mut scene = scene.clone();
            figure.apply(&mut scene, &pose(story_time(t)));
            figure.feet(&scene)
        };
        let mut worst = (0.0f32, 0.0);
        let mut last = soles(0.0);
        for frame in 1..(WALK_SECONDS * 60.0) as u32 {
            let t = frame as f32 / 60.0;
            let now = soles(t);
            for (a, b) in last.iter().flatten().zip(now.iter().flatten()) {
                // a point of a sole on the ground in both frames must stay in place
                if a.y < 0.001 && b.y < 0.001 {
                    let slide = (*a - *b).with_y(0.0).length();
                    if slide > worst.0 {
                        worst = (slide, t);
                    }
                }
            }
            last = now;
        }
        eprintln!("worst slide {:.1} mm at {:.2} s", worst.0 * 1000.0, worst.1);
        eprintln!("footfalls {:?}", footfalls());
        assert!(worst.0 < 0.004, "{worst:?}");
    }
}
