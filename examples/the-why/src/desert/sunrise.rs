//! Shot 9, "Sunrise": the cut comes from the flare of the rocket, so the shot starts with
//! the low sun overexposed in the middle of the frame. The light comes down to the plateau.
//! The engineer stands at the place of the line, a silhouette against the sun. In the
//! distance: a bridge over the canyon to a village, a greenhouse dome, a rocket contrail.

use std::f32::consts::PI;

use fframes_3d::math::{hash, noise1, smoothstep};
use fframes_3d::*;

use super::terrain::{EDGE, Terrain, canyon_center, canyon_width, rock};
use super::{AMBER, DesertSet, ahead, expose, hide_dark_lights, segment};
use crate::assets::Assets;
use crate::figure::{FigureStyle, Pose};
use crate::look;

/// The direction to the sun in the panorama `sky_sunrise.hdr` without rotation: the
/// brightest texel, 12.7 degrees above the horizon.
const PANORAMA_SUN: Vec3 = vec3(0.7888, 0.2206, 0.5739);

/// The turn of the sky panorama around Y, in radians. It puts the sun opposite the factory:
/// the night cameras look at the factory, so they do not show the land of the sunrise.
fn sky_rotation() -> f32 {
    let a = ahead();
    a.x.atan2(a.z) + PI - PANORAMA_SUN.x.atan2(PANORAMA_SUN.z)
}

/// The unit direction to the sun of shot 9. It is the sun of the panorama.
pub(super) fn sun_direction() -> Vec3 {
    Quat::from_rotation_y(sky_rotation()) * PANORAMA_SUN.normalize()
}

/// The horizontal direction to the sun.
fn sunward() -> Vec3 {
    sun_direction().with_y(0.0).normalize()
}

/// Dark clothes with a thin warm rim of the sun behind him.
pub(super) fn figure_style() -> FigureStyle {
    let rim = Rgb::hex(0xffc078) * 0.5;
    // little light from the sky behind the camera, so he stays a silhouette
    let cloth = |color: u32, roughness: f32| {
        Material::standard(color)
            .roughness(roughness)
            .rim(rim, 4.0)
            .env_intensity(0.15)
    };
    FigureStyle {
        skin: cloth(0x3a2a20, 0.55),
        jacket: cloth(0x24262b, 0.85),
        trousers: cloth(0x1c1d20, 0.9),
        boots: cloth(0x221a14, 0.6),
        hair: cloth(0x100c0a, 0.75),
    }
}

/// The place where the engineer stands at sunrise: on the plateau, a few meters from its
/// edge, at the place of the line.
fn stand_point(terrain: &Terrain) -> Vec3 {
    let p = terrain.sunward_point(EDGE - 12.0, 4.0);
    vec3(p.x, terrain.height(p.x, p.y), p.y)
}

/// Adds the light of the sunrise: the sky panorama, the low sun with long shadows and warm
/// dusty air over the plain. Also adds boulders on the edge of the plateau: their shadows
/// make shafts of light in the dust.
pub(super) fn dawn_set(scene: &mut Scene, assets: &Assets, terrain: &Terrain) {
    let sun = sun_direction();
    scene.environment = Environment::Map {
        map: assets.sunrise_sky.clone(),
        intensity: 0.8,
        rotation: sky_rotation(),
    };
    scene.background = Background::Environment { blur: 0.0 };
    scene.add(Node::light(
        Light::sun(0xffc684, 3.4, sun * 100.0).with_shadow(Shadow {
            map_size: 4096,
            softness: 1.5,
            strength: 0.95,
            bias: 1.5,
            bounds: Some(Aabb {
                min: vec3(-90.0, -60.0, -90.0),
                max: vec3(90.0, 14.0, 90.0),
            }),
        }),
    ));
    // Dust lies thick in the plain below the plateau and thin over the plateau.
    scene.atmosphere = Some(Atmosphere {
        density: 0.0016,
        color: Rgb::hex(0xffe4c4),
        base: -45.0,
        falloff: 0.035,
        anisotropy: 0.72,
        ambient: Rgb::hex(0x1c2030),
        noise: 0.55,
        noise_size: 9.0,
        offset: Vec3::ZERO,
        distance: 2600.0,
        steps: 32,
    });

    let material = assets.rock.material(0xb89a80, 0.95, 0.0);
    let boulders = [
        (EDGE - 5.0, -14.0, 3.2),
        (EDGE - 2.0, -7.0, 1.4),
        (EDGE - 8.0, 13.0, 2.6),
        (EDGE - 1.0, 21.0, 4.5),
        (EDGE - 15.0, -22.0, 1.8),
        (EDGE + 1.0, 32.0, 3.0),
        (EDGE - 3.0, 9.0, 0.9),
    ];
    let instances = boulders.iter().enumerate().map(|(i, &(s, a, size))| {
        let p = terrain.sunward_point(s, a);
        let ground = terrain.height(p.x, p.y);
        Mat4::from_scale_rotation_translation(
            vec3(size * 1.2, size * 0.75, size),
            Quat::from_rotation_y(hash(i as u32 * 3 + 1) * 6.3),
            vec3(p.x, ground - 0.3 * size, p.y),
        )
    });
    scene.add(Node::instanced(
        rock(901, 3),
        material,
        Instances::new(instances),
    ));
}

/// The nodes of the landmarks that the shot animates.
pub struct Landmarks {
    /// The green light in the greenhouse dome.
    pub dome_light: NodeId,
}

/// Adds the bridge over the canyon to the village, the village, the greenhouse dome and the
/// contrail of the rocket. They are simple shapes for the distance. The models of the
/// montage of shot 8 can replace them.
pub(super) fn landmarks(scene: &mut Scene, terrain: &Terrain) -> Landmarks {
    let s_dir = sunward();
    let right = s_dir.cross(Vec3::Y);
    let at = |s: f32, a: f32| {
        let p = terrain.sunward_point(s, a);
        vec3(p.x, terrain.height(p.x, p.y), p.y)
    };
    let moved = |g: Geometry, v: Vec3| g.transform(Mat4::from_translation(v));
    let to_right = Quat::from_rotation_arc(Vec3::X, right);

    // The bridge: a cable-stayed deck from rim to rim with an H-shaped pylon at each rim.
    let s_bridge = 600.0;
    let (center, width) = (canyon_center(s_bridge), canyon_width(s_bridge));
    let (a0, a1) = (center - width * 0.5 - 20.0, center + width * 0.5 + 20.0);
    let deck_y = at(s_bridge, a0).y.max(at(s_bridge, a1).y) + 2.0;
    let deck_at = |a: f32, z: f32| {
        let p = terrain.sunward_point(s_bridge, a);
        vec3(p.x, deck_y, p.y) - s_dir * z
    };
    let deck = Geometry::cuboid(a1 - a0, 2.4, 13.0).rotate(to_right);
    let mut parts = vec![moved(deck, deck_at(f32::midpoint(a0, a1), 0.0))];
    let mut cables = Vec::new();
    for pylon in [a0 + 26.0, a1 - 26.0] {
        let top = 58.0;
        for z in [-7.5f32, 7.5] {
            let foot = deck_at(pylon, z) - Vec3::Y * 40.0;
            let head = deck_at(pylon, z * 0.35) + Vec3::Y * top;
            parts.push(beam(foot, head, 1.6));
        }
        parts.push(beam(
            deck_at(pylon, -3.0) + Vec3::Y * (top - 4.0),
            deck_at(pylon, 3.0) + Vec3::Y * (top - 4.0),
            1.2,
        ));
        // fans of cables to both sides of the pylon
        for k in 1..=11 {
            for side in [-1.0f32, 1.0] {
                let a = pylon + side * k as f32 * 7.5;
                if a < a0 + 2.0 || a > a1 - 2.0 {
                    continue;
                }
                for z in [-6.0f32, 6.0] {
                    let head = deck_at(pylon, z * 0.35) + Vec3::Y * (top - 2.0 - 1.4 * k as f32);
                    cables.push(segment(head, deck_at(a, z), 0.18));
                }
            }
        }
    }
    let stone = Material::standard(0x7a726a).roughness(0.8);
    scene.add(Node::mesh(Geometry::merge(parts), stone).shadows(false, false));
    scene.add(
        Node::instanced(
            Geometry::cylinder(1.0, 1.0, 1.0, 6).translate(0.0, 0.5, 0.0),
            Material::standard(0x8a8680).roughness(0.5),
            Instances::new(cables),
        )
        .shadows(false, false),
    );
    // lamps along the deck
    let lamps: Vec<Particle> = (0..24)
        .map(|i| {
            let a = a0 + (a1 - a0) * (i as f32 + 0.5) / 24.0;
            Particle::new(deck_at(a, 6.8) + Vec3::Y * 2.5, 0.5, Rgb::hex(AMBER) * 6.0)
        })
        .collect();
    scene.add(Node::particles(lamps, Blending::Additive));

    // The village on the far rim: white and ochre houses with warm windows, and trees.
    let mut houses = Vec::new();
    let mut windows = Vec::new();
    for i in 0..70u32 {
        let h = |n: u32| hash(i * 17 + n * 7919 + 5);
        let s = 640.0 + 420.0 * h(1);
        let rim = canyon_center(s) - canyon_width(s) * 0.5;
        let base = at(s, rim - 25.0 - 220.0 * h(2).powf(1.3));
        if h(7) < 0.25 {
            let tree = Geometry::cone(3.0 + 2.0 * h(3), 9.0 + 6.0 * h(4), 8);
            houses.push(moved(tree.with_color(0x2c4a26), base + Vec3::Y * 5.0));
            continue;
        }
        let (w, d, wall) = (7.0 + 7.0 * h(3), 6.0 + 6.0 * h(4), 4.0 + 5.0 * h(5));
        let turn = to_right * Quat::from_rotation_y((h(6) - 0.5) * 0.5);
        let tone = if h(8) < 0.6 { 0xe8dccb } else { 0xd8a878 };
        let body = Geometry::cuboid(w, wall, d).translate(0.0, wall * 0.5, 0.0);
        let ridge = 2.2 + 1.5 * h(9);
        let roof = Geometry::extrude(
            &[vec2(-w * 0.55, 0.0), vec2(w * 0.55, 0.0), vec2(0.0, ridge)],
            d * 1.05,
            0.0,
        )
        .translate(0.0, wall, 0.0);
        let house = Geometry::merge([body.with_color(tone), roof.with_color(0x8a4a32)]);
        houses.push(moved(house.rotate(turn), base));
        // two windows on the side that faces the camera, lit in most houses
        if h(10) < 0.7 {
            for k in [-0.25f32, 0.25] {
                let p = base + turn * vec3(k * w, wall * 0.5, d * 0.5 + 0.1);
                windows.push(moved(Geometry::plane(1.4, 1.6).rotate(turn), p));
            }
        }
    }
    scene.add(
        Node::mesh(
            Geometry::merge(houses),
            Material::standard(0xffffff).roughness(0.85),
        )
        .shadows(false, false),
    );
    scene.add(
        Node::mesh(Geometry::merge(windows), Material::glow(AMBER, 4.0)).shadows(false, false),
    );

    // The greenhouse: a geodesic dome of glass that glows green from the plants inside.
    let dome_center = at(900.0, 230.0);
    let sphere = Geometry::icosahedron(36.0, 2);
    let mut panes = Vec::new();
    let mut struts = Vec::new();
    for tri in sphere.indices.as_chunks::<3>().0 {
        let [a, b, c] = tri.map(|i| sphere.positions[i as usize] + dome_center);
        if a.y.min(b.y).min(c.y) < dome_center.y - 1.0 {
            continue;
        }
        panes.extend([a, b, c]);
        for (p, q) in [(a, b), (b, c), (c, a)] {
            struts.push(segment(p, q, 0.35));
        }
    }
    scene.add(
        Node::mesh(
            Geometry::from_triangles(&panes),
            Material::standard(0x1c3a24)
                .roughness(0.15)
                .emissive(0x5dff86, 1.6)
                .opacity(0.85),
        )
        .shadows(false, false),
    );
    scene.add(
        Node::instanced(
            Geometry::cylinder(1.0, 1.0, 1.0, 5).translate(0.0, 0.5, 0.0),
            Material::standard(0xe8e4dc).roughness(0.4),
            Instances::new(struts),
        )
        .shadows(false, false),
    );
    let dome_light = scene.add(
        Node::light(Light::point(0x5dff86, 4.0e4, 260.0))
            .with_position(dome_center + Vec3::Y * 12.0),
    );

    // The contrail of the rocket: it rises far beyond the plain, to the left of the sun.
    // It goes up straight and bends over to the right high in the sky. The sun behind it
    // lights it, so it glows.
    let launch = at(5200.0, 750.0);
    let path: Vec<Vec3> = (0..=60)
        .map(|i| {
            let u = i as f32 / 60.0;
            let downrange = -right * (5200.0 * u.powf(2.2)) + s_dir * (1500.0 * u * u);
            launch + Vec3::Y * (7000.0 * u) + downrange
        })
        .collect();
    let rings = 13;
    let mut trail = Geometry::tube(&path, 1.0, rings - 1);
    // the trail widens with the height and fades at the ground and at the top
    for (i, p) in trail.positions.iter_mut().enumerate() {
        let k = i / rings as usize;
        let u = k as f32 / 60.0;
        *p = path[k] + (*p - path[k]) * (6.0 + 70.0 * u);
    }
    trail.colors = (0..trail.positions.len())
        .map(|i| {
            let u = (i / rings as usize) as f32 / 60.0;
            let a = smoothstep(0.0, 0.03, u) * (1.0 - smoothstep(0.7, 1.0, u)) * (1.0 - 0.6 * u);
            vec4(1.0, 1.0, 1.0, a)
        })
        .collect();
    trail.update_bounds();
    scene.add(Node::mesh(trail, Material::glow(0xffe4c8, 0.45)).shadows(false, false));
    scene.add(Node::particles(
        vec![Particle::new(path[60], 30.0, Rgb::hex(0xfff2d8) * 40.0)],
        Blending::Additive,
    ));

    Landmarks { dome_light }
}

/// A square beam from `a` to `b` with the half width `half`.
fn beam(a: Vec3, b: Vec3, half: f32) -> Geometry {
    Geometry::cuboid(1.0, 1.0, 1.0)
        .translate(0.0, 0.5, 0.0)
        .transform(segment(a, b, half * 2.0))
}

/// The multiplier of the scene light at `t`: the shot starts overexposed by the flare of
/// the rocket and comes down to the sunrise.
fn flare(t: f32) -> f32 {
    let fade = 1.0 - smoothstep(0.0, 1.2, t);
    1.0 + 9.0 * fade * fade
}

impl DesertSet {
    /// The renderer of shot 9 for pictures of `width` x `height` pixels.
    pub fn renderer_sunrise(width: u32, height: u32) -> Renderer {
        Renderer::new(width, height).exposure(0.9).bloom(Bloom {
            strength: 0.7,
            radius: 0.8,
            threshold: 1.6,
        })
    }

    /// Motion blur samples of shot 9: none, the camera moves slowly.
    pub fn shutter_sunrise(&self, _t: f32) -> usize {
        1
    }

    /// The scene and the camera of shot 9 at `t` seconds, `0..10`.
    pub fn moment_sunrise(&self, t: f32) -> (Scene, Camera) {
        let mut scene = self.dawn.clone();
        let heading = sunward().x.atan2(sunward().z);
        let feet = stand_point(&self.terrain);
        let stand = Pose::stand(t + 3.0).placed(feet, heading);
        self.dawn_figure.apply(&mut scene, &stand);
        let camera = self.sunrise_camera(t, feet);
        if let Some(air) = &mut scene.atmosphere {
            air.offset = vec3(1.2, 0.05, 0.4) * -t;
        }
        if let Object::Particles(dust) = &mut scene[self.dawn_dust].object {
            dust.items = dust_in_sun(camera.position, t);
            // A warm glow around the sun: it fills the frame at the cut from the flare of
            // the rocket and settles into a halo.
            let sun = camera.position + sun_direction() * 20000.0;
            let flash = 1.0 - smoothstep(0.0, 1.6, t);
            dust.items.push(Particle::new(
                sun,
                900.0 + 2600.0 * flash,
                Rgb::hex(0xffcf98) * (1.2 + 6.0 * flash),
            ));
        }
        if let Some(light) = scene[self.landmarks.dome_light].light_mut() {
            *light.intensity_mut() *= 1.0 + 0.04 * noise1(3, t * 2.0);
        }
        expose(&mut scene, flare(t));
        hide_dark_lights(&mut scene);
        (scene, camera)
    }
}

/// Dust in the air near the camera. Motes in front of the sun scatter its light toward the
/// camera and sparkle.
fn dust_in_sun(eye: Vec3, t: f32) -> Vec<Particle> {
    let sun = sun_direction();
    let breeze = vec3(0.3, 0.05, -0.2);
    let half = 14.0;
    (0..2500u32)
        .map(|i| {
            let h = |n: u32| hash(i * 23 + n * 7919 + 77);
            let wander = vec3(
                noise1(i, t * 0.3),
                noise1(i + 7, t * 0.25),
                noise1(i + 13, t * 0.3),
            );
            let base = vec3(h(1) * 2.0 - 1.0, h(2), h(3) * 2.0 - 1.0) * vec3(half, 6.0, half)
                + breeze * t
                + wander * 0.4;
            let local = vec3(
                (base.x + half).rem_euclid(2.0 * half) - half,
                base.y.rem_euclid(6.0),
                (base.z + half).rem_euclid(2.0 * half) - half,
            );
            let p = eye + sunward() * (half + 2.0) + local - Vec3::Y;
            let toward = (p - eye).normalize_or_zero().dot(sun).max(0.0);
            let glint = 0.02 + 3.0 * toward.powi(40);
            Particle::new(p, 0.0006 + 0.0014 * h(4), Rgb::hex(0xffe0b8) * glint)
        })
        .collect()
}

impl DesertSet {
    /// The camera of shot 9: it looks into the sun, tilts down to the plain below the
    /// plateau and comes down from a high view to the engineer, who stands at `feet`.
    fn sunrise_camera(&self, t: f32, feet: Vec3) -> Camera {
        let s_dir = sunward();
        let right = s_dir.cross(Vec3::Y);
        let push = smoothstep(0.8, 11.0, t);
        // he stands right of the center, so the quote has the left third
        let place = feet - s_dir * (30.0 - 17.0 * push) + right * (-2.6 + 1.74 * push);
        let ground = self.terrain.height(place.x, place.z);
        let position = place.with_y(ground + 4.5 - 2.0 * push);
        // He stands in the lower third. The view goes over him to the bridge.
        let landscape = (s_dir - Vec3::Y * (0.045 - 0.015 * push)).normalize();
        let view = sun_direction()
            .lerp(landscape, smoothstep(0.4, 3.2, t))
            .normalize();
        look::lens(50.0 + 15.0 * push)
            .position(position)
            .look_at(position + view)
            .clip(0.5, 30000.0)
            .focus_on(feet + Vec3::Y * 1.4, 3.0)
            .shake(t * 0.35, 0.0015, 21)
    }
}
