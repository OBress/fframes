//! Shot 9, "Sunrise": the cut comes from the white flare of the rocket, so the shot starts
//! looking into the low sun, overexposed. The light comes down to the plateau. The engineer
//! stands near its edge, a silhouette against the sun. Below him: the Golden Gate over the
//! canyon to a village, a greenhouse dome, the contrail of the rocket. Birds cross the
//! light. For the quote the camera rises slowly behind him and shows the land below.

use std::f32::consts::{PI, TAU};
use std::sync::Arc;

use fframes_3d::math::{hash, noise1, smoothstep};
use fframes_3d::*;

use super::terrain::{EDGE, Terrain, canyon_center, canyon_width, rock};
use super::{AMBER, DesertSet, ahead, expose, fbm, hide_dark_lights, segment};
use crate::assets::Assets;
use crate::figure::{FigureStyle, Pose};
use crate::look;

/// The height of the sun above the horizon, in radians: 4.6 degrees. A low sun gives long
/// shadows and a warm light.
const SUN_HEIGHT: f32 = 0.08;

/// The unit direction to the sun of shot 9. The sun stands opposite the factory: the night
/// cameras look at the factory, so they do not show the land of the sunrise.
pub(super) fn sun_direction() -> Vec3 {
    -ahead() * SUN_HEIGHT.cos() + Vec3::Y * SUN_HEIGHT.sin()
}

/// The horizontal direction to the sun.
fn sunward() -> Vec3 {
    -ahead()
}

/// The horizontal direction to the right when you look at the sun.
fn right() -> Vec3 {
    sunward().cross(Vec3::Y)
}

/// The breeze at sunrise in meters per second: from the left, a little toward the sun. It
/// moves the dust and the contrail.
fn breeze() -> Vec3 {
    right() * 2.5 + sunward() * 0.8
}

/// Dark clothes with a thin warm rim of the sun behind him.
pub(super) fn figure_style() -> FigureStyle {
    let rim = Rgb::hex(0xffc078) * 0.22;
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

/// The dawn sky as a panorama: deep blue at the top, a warm band at the horizon that is
/// gold near the sun and rose on the other side, the glow around the sun, and thin
/// clouds that the sun lights from below.
fn dawn_sky() -> Arc<EnvironmentMap> {
    let (width, height) = (1024, 512);
    let sun = sun_direction();
    let data = (0..width * height)
        .map(|i| {
            let (x, y) = (i % width, i / width);
            // the inverse of the panorama mapping of the engine
            let around = ((x as f32 + 0.5) / width as f32 - 0.5) * TAU;
            let elevation = (0.5 - (y as f32 + 0.5) / height as f32) * PI;
            let dir = vec3(
                elevation.cos() * around.cos(),
                elevation.sin(),
                elevation.cos() * around.sin(),
            );
            // 1 toward the sun, 0 away from it, around the horizon
            let side = 0.5 + 0.5 * dir.with_y(0.0).normalize_or_zero().dot(sunward());
            let horizon = vec3(0.2, 0.15, 0.2).lerp(vec3(0.85, 0.42, 0.17), side * side);
            let zenith = vec3(0.022, 0.05, 0.13);
            let e = elevation.max(0.0);
            let mut color = horizon.lerp(zenith, 1.0 - (-e / 0.13).exp());
            // the glow of the sun in the dust of the air
            let angle = dir.dot(sun).clamp(-1.0, 1.0).acos();
            color +=
                vec3(1.0, 0.7, 0.4) * (2.5 * (-angle / 0.035).exp() + 0.35 * (-angle / 0.25).exp());
            if elevation < 0.0 {
                // the ground far away, in the haze of the horizon
                let ground = vec3(0.09, 0.06, 0.045);
                return color.lerp(ground, smoothstep(0.0, -0.06, elevation));
            }
            // Thin clouds on a layer high above: big overhead, small near the horizon.
            let layer = vec2(dir.x, dir.z) / dir.y.max(0.04);
            // The coordinates bend with a slow noise, so the clouds do not follow a grid.
            let warp = vec2(fbm(layer * 0.35, 2, 71), fbm(layer * 0.35 + 5.0, 2, 73));
            let streak = fbm(
                (layer + warp * 1.6) * vec2(0.9, 2.6) + vec2(3.0, 1.0),
                4,
                61,
            );
            let cloud = smoothstep(-0.05, 0.5, streak) * smoothstep(0.03, 0.12, e);
            // Near the sun the clouds glow gold. Away from it they are rose and lavender.
            let lit = vec3(0.3, 0.2, 0.26).lerp(vec3(1.8, 0.95, 0.4), side.powi(3));
            color.lerp(lit * (0.5 + 0.5 * (-e / 0.4).exp()), 0.75 * cloud)
        })
        .collect();
    Arc::new(EnvironmentMap::from_linear(width, height, data))
}

/// Adds the light of the sunrise: the dawn sky, the low sun with long shadows, warm dusty
/// air over the plain, and rocks on the plateau. Their shadows make shafts of light in the
/// dust, and the outcrop on the left keeps the lower left of the picture in shadow for the
/// quote.
pub(super) fn dawn_set(scene: &mut Scene, assets: &Assets, terrain: &Terrain) {
    let sun = sun_direction();
    scene.environment = Environment::map(dawn_sky(), 1.0);
    scene.background = Background::Environment { blur: 0.0 };
    let feet = stand_point(terrain);
    scene.add(Node::light(
        Light::sun(0xffbe7a, 3.6, sun * 100.0).with_shadow(Shadow {
            map_size: 4096,
            softness: 1.5,
            strength: 0.95,
            bias: 1.5,
            bounds: Some(Aabb {
                min: feet - vec3(70.0, 50.0, 70.0),
                max: feet + vec3(70.0, 14.0, 70.0),
            }),
        }),
    ));
    // Dust lies thick in the plain below the plateau and thin over the plateau.
    scene.atmosphere = Some(Atmosphere {
        density: 0.0009,
        color: Rgb::hex(0xffe0bc),
        base: -45.0,
        falloff: 0.045,
        anisotropy: 0.68,
        ambient: Rgb::hex(0x1a2236),
        noise: 0.6,
        noise_size: 9.0,
        offset: Vec3::ZERO,
        distance: 2600.0,
        steps: 32,
    });

    let material = assets.rock.material(0xb89a80, 0.95, 0.0);
    // distance toward the sun, distance to the right, size, in meters
    let boulders = [
        (EDGE - 5.0, -14.0, 3.2),
        (EDGE - 2.0, -7.0, 1.4),
        (EDGE - 8.0, 13.0, 2.6),
        (EDGE - 1.0, 21.0, 4.5),
        (EDGE - 15.0, -22.0, 1.8),
        (EDGE + 1.0, 32.0, 3.0),
        (EDGE - 3.0, 9.0, 0.9),
        // the outcrop on the left of the camera
        (EDGE - 22.0, -15.0, 5.5),
        (EDGE - 26.0, -11.5, 3.0),
        (EDGE - 19.0, -19.0, 4.0),
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
    /// The lit windows of the village. They go out as the day comes.
    pub windows: NodeId,
    /// The contrail of the rocket: soft puffs that the wind moves.
    pub contrail: NodeId,
    /// The birds that cross the light.
    pub birds: NodeId,
}

/// The points of the path of the rocket from the launch pad, 61 of them. It goes up
/// straight and bends over to the right high in the sky.
fn rocket_path(terrain: &Terrain) -> Vec<Vec3> {
    let p = terrain.sunward_point(5200.0, -900.0);
    let launch = vec3(p.x, terrain.height(p.x, p.y), p.y);
    (0..=60)
        .map(|i| {
            let u = i as f32 / 60.0;
            let downrange = right() * (5200.0 * u.powf(2.2)) + sunward() * (1500.0 * u * u);
            launch + Vec3::Y * (7000.0 * u) + downrange
        })
        .collect()
}

/// Adds the Golden Gate over the canyon to the village, the village, the greenhouse dome,
/// the contrail of the rocket and the birds. Except the bridge, they are simple shapes for
/// the distance.
pub(super) fn landmarks(scene: &mut Scene, terrain: &Terrain) -> Landmarks {
    let right = right();
    let at = |s: f32, a: f32| {
        let p = terrain.sunward_point(s, a);
        vec3(p.x, terrain.height(p.x, p.y), p.y)
    };
    let moved = |g: Geometry, v: Vec3| g.transform(Mat4::from_translation(v));
    let to_right = Quat::from_rotation_arc(Vec3::X, right);

    // The Golden Gate of the montage at one fifth of its size, over the canyon to the village.
    let s_bridge = 600.0;
    let center = canyon_center(s_bridge);
    // the plain under the towers, 128 m to each side of the canyon middle
    let foot = at(s_bridge, center - 128.0)
        .y
        .max(at(s_bridge, center + 128.0).y);
    let p = terrain.sunward_point(s_bridge, center);
    let gate = scene.add(
        Node::group()
            .with_position(vec3(p.x, foot, p.y))
            .with_rotation(to_right),
    );
    scene.append(Some(gate), &crate::montage::golden_gate_landmark());

    // The village on the far rim: white, ochre and rose houses with warm windows, garden
    // walls and trees. The houses have flat roofs with a parapet and a small stair house,
    // or tiled roofs with a chimney. Some have two floors.
    let mut houses = Vec::new();
    let mut windows = Vec::new();
    let walls = [0xe8dccb, 0xe2cfb4, 0xd8a878, 0xd9b8a4, 0xf0e8dc];
    let roofs = [0x8a4a32, 0x9a5a3a, 0x7a3e2c];
    for i in 0..90u32 {
        let h = |n: u32| hash(i * 17 + n * 7919 + 5);
        let s = 640.0 + 420.0 * h(1);
        let rim = canyon_center(s) - canyon_width(s) * 0.5;
        let base = at(s, rim - 25.0 - 220.0 * h(2).powf(1.3));
        let turn = to_right * Quat::from_rotation_y((h(6) - 0.5) * 0.5);
        if h(7) < 0.3 {
            // a round tree, or a dark cypress
            let tree = if h(11) < 0.6 {
                let r = 2.5 + 2.0 * h(3);
                Geometry::merge([
                    Geometry::cylinder(0.3, 0.4, 3.0, 5)
                        .translate(0.0, 1.5, 0.0)
                        .with_color(0x3a2a1c),
                    Geometry::icosahedron(r, 1)
                        .scale(1.0, 0.85, 1.0)
                        .translate(0.0, 2.5 + r * 0.8, 0.0)
                        .with_color(0x3c5a2c),
                ])
            } else {
                let height = 9.0 + 6.0 * h(4);
                Geometry::cone(1.3 + 0.6 * h(3), height, 7)
                    .translate(0.0, height * 0.5, 0.0)
                    .with_color(0x23361e)
            };
            houses.push(moved(tree, base));
            continue;
        }
        let (w, d) = (7.0 + 7.0 * h(3), 6.0 + 6.0 * h(4));
        let floors = if h(5) < 0.35 { 2.0 } else { 1.0 };
        let wall = 3.2 * floors + 0.6 + 0.8 * h(12);
        let tone = walls[(h(8) * walls.len() as f32) as usize % walls.len()];
        let mut parts = vec![
            Geometry::cuboid(w, wall, d)
                .translate(0.0, wall * 0.5, 0.0)
                .with_color(tone),
        ];
        if h(9) < 0.55 {
            // a flat roof with a parapet, a stair house and a water tank
            let parapet = Geometry::cuboid(w + 0.3, 0.6, d + 0.3).translate(0.0, wall + 0.3, 0.0);
            let stair = Geometry::cuboid(2.4, 2.4, 2.4).translate(w * 0.25, wall + 1.2, -d * 0.2);
            let tank =
                Geometry::cylinder(0.6, 0.6, 1.2, 8).translate(-w * 0.3, wall + 1.2, d * 0.2);
            parts.extend([
                parapet.with_color(tone),
                stair.with_color(tone),
                tank.with_color(0x6a6460),
            ]);
        } else {
            let ridge = 2.2 + 1.5 * h(10);
            let roof = Geometry::extrude(
                &[vec2(-w * 0.55, 0.0), vec2(w * 0.55, 0.0), vec2(0.0, ridge)],
                d * 1.05,
                0.0,
            )
            .translate(0.0, wall, 0.0);
            let chimney =
                Geometry::cuboid(0.7, 2.0, 0.7).translate(w * 0.28, wall + ridge * 0.6, 0.0);
            let roof_tone = roofs[(h(13) * roofs.len() as f32) as usize % roofs.len()];
            parts.extend([roof.with_color(roof_tone), chimney.with_color(tone)]);
        }
        // a low annex on one side, and a garden wall
        if h(14) < 0.5 {
            let annex = Geometry::cuboid(w * 0.4, 2.6, d * 0.6).translate(-w * 0.7, 1.3, d * 0.1);
            parts.push(annex.with_color(tone));
        }
        if h(15) < 0.6 {
            let garden =
                Geometry::cuboid(w * 1.6, 1.4, 0.35).translate(w * 0.1, 0.7, d * 0.5 + 4.0);
            parts.push(garden.with_color(walls[0]));
        }
        houses.push(moved(Geometry::merge(parts).rotate(turn), base));
        // windows on the two sides that face the camera, lit in most houses
        for floor in 0..floors as u32 {
            let y = 1.7 + 3.2 * floor as f32;
            for k in [-0.3f32, 0.0, 0.3] {
                if hash(i * 31 + floor * 7 + (k * 10.0) as u32 + 99) < 0.35 {
                    continue;
                }
                let p = base + turn * vec3(k * w, y, d * 0.5 + 0.1);
                windows.push(moved(Geometry::plane(1.2, 1.5).rotate(turn), p));
            }
            if h(16) < 0.6 {
                let side = turn * Quat::from_rotation_y(-std::f32::consts::FRAC_PI_2);
                let p = base + turn * vec3(-w * 0.5 - 0.1, y, 0.0);
                windows.push(moved(Geometry::plane(1.2, 1.5).rotate(side), p));
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
    let windows = scene.add(
        Node::mesh(Geometry::merge(windows), Material::glow(AMBER, 6.0)).shadows(false, false),
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
                .emissive(0x3dd870, 1.5)
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
        Node::light(Light::point(0x5dff86, 4.5e4, 260.0))
            .with_position(dome_center + Vec3::Y * 12.0),
    );

    Landmarks {
        dome_light,
        windows,
        contrail: scene.add(Node::particles(Vec::new(), Blending::Normal)),
        birds: scene.add(Node::particles(Vec::new(), Blending::Normal).named("birds")),
    }
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
            air.offset = -breeze() * t;
        }
        if let Object::Particles(dust) = &mut scene[self.dawn_dust].object {
            dust.items = dust_in_sun(camera.position, t);
            // The sun: a hot disc, and a warm glow around it. The glow fills the frame at
            // the cut from the flare of the rocket and settles into a halo.
            let sun = camera.position + sun_direction() * 20000.0;
            let flash = 1.0 - smoothstep(0.0, 1.6, t);
            dust.items
                .push(Particle::new(sun, 95.0, Rgb::hex(0xfff4e0) * 60.0));
            dust.items.push(Particle::new(
                sun,
                900.0 + 2600.0 * flash,
                Rgb::hex(0xffcf98) * (1.0 + 6.0 * flash),
            ));
        }
        if let Object::Particles(contrail) = &mut scene[self.landmarks.contrail].object {
            contrail.items = self.contrail(t, camera.position);
        }
        if let Object::Particles(birds) = &mut scene[self.landmarks.birds].object {
            birds.items = birds_at(t, feet);
        }
        // The village lights go out as the day comes.
        if let Some(material) = scene[self.landmarks.windows].material_mut() {
            material.emissive_intensity *= 1.0 - 0.85 * smoothstep(2.0, 9.5, t);
        }
        if let Some(light) = scene[self.landmarks.dome_light].light_mut() {
            *light.intensity_mut() *= 1.0 + 0.04 * noise1(3, t * 2.0);
        }
        expose(&mut scene, flare(t));
        hide_dark_lights(&mut scene);
        (scene, camera)
    }

    /// The contrail of the rocket at `t`: soft puffs along the path that widen with the
    /// height. The wind moves the old high parts more. Seen from `eye`, the puffs near the
    /// sun are bright.
    fn contrail(&self, t: f32, eye: Vec3) -> Vec<Particle> {
        let path = rocket_path(&self.terrain);
        let sun = sun_direction();
        let mut items = Vec::new();
        for i in 0..900u32 {
            let h = |n: u32| hash(i * 31 + n * 7919 + 401);
            let u = i as f32 / 900.0 * 0.97 + 0.02 * h(1);
            let k = u * 60.0;
            let (a, b) = (path[k as usize], path[(k as usize + 1).min(60)]);
            let center = a.lerp(b, k.fract());
            // the trail is older higher up: it has spread and drifted more
            let age = 6.0 + 30.0 * u + t;
            let spread = 30.0 + 160.0 * u.powf(0.8);
            let jitter = vec3(h(2) - 0.5, h(3) - 0.5, h(4) - 0.5) * spread;
            let sway = vec3(
                noise1(i % 37, u * 9.0 + t * 0.05),
                0.0,
                noise1(i % 41 + 50, u * 9.0 + t * 0.05),
            ) * (15.0 * u);
            let p = center + jitter + sway + breeze() * (age * 2.5 * u);
            let toward = sun.dot((p - eye).normalize()).max(0.0);
            let light = 1.1 + 4.0 * toward.powi(30);
            let fade = smoothstep(0.0, 0.03, u) * (1.0 - smoothstep(0.75, 1.0, u));
            items.push(
                Particle::new(p, spread * (0.6 + 0.5 * h(5)), Rgb::hex(0xffdcb8) * light)
                    .opacity(0.07 * fade * (1.0 - 0.5 * u)),
            );
        }
        items
    }

    /// The camera of shot 9. It looks into the sun at the cut and tilts down to the land.
    /// Then it rises slowly behind the engineer, who stands at `feet`, and pushes in. The
    /// plain and the canyon below the plateau come into view.
    fn sunrise_camera(&self, t: f32, feet: Vec3) -> Camera {
        let (s_dir, right) = (sunward(), right());
        let rise = smoothstep(0.5, 10.5, t);
        // he stands right of the center, so the quote has the left third
        let place = feet - s_dir * (16.0 - 5.0 * rise) - right * (2.6 - 0.6 * rise);
        let ground = self.terrain.height(place.x, place.z);
        let position = place.with_y(ground + 0.9 + 1.5 * rise);
        // The view goes over him to the bridge. When the camera is high it looks down a
        // little more, so the horizon stays in the upper third.
        let landscape = (s_dir - Vec3::Y * (0.028 + 0.01 * rise)).normalize();
        let view = sun_direction()
            .lerp(landscape, smoothstep(0.4, 3.2, t))
            .normalize();
        look::lens(40.0 + 10.0 * rise)
            .position(position)
            .look_at(position + view)
            .clip(0.5, 30000.0)
            .focus_on(feet + Vec3::Y * 1.4, 3.0)
            .shake(t * 0.35, 0.0012, 21)
    }
}

/// Dust in the air near the camera. Motes in front of the sun scatter its light toward the
/// camera and sparkle. `eye` is the camera position.
fn dust_in_sun(eye: Vec3, t: f32) -> Vec<Particle> {
    let sun = sun_direction();
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
                + breeze() * (0.15 * t)
                + wander * 0.4;
            let local = vec3(
                (base.x + half).rem_euclid(2.0 * half) - half,
                base.y.rem_euclid(6.0),
                (base.z + half).rem_euclid(2.0 * half) - half,
            );
            let p = eye + sunward() * (half + 2.0) + local - Vec3::Y;
            let toward = (p - eye).normalize_or_zero().dot(sun).max(0.0);
            let glint = 0.08 + 3.0 * toward.powi(10);
            Particle::new(p, 0.001 + 0.002 * h(4), Rgb::hex(0xffe0b8) * glint)
        })
        .collect()
}

/// The birds take off at this time, in seconds. They enter the picture at about 2.5 s, with
/// the sound of the birds, and leave it at about 7.7 s.
pub const BIRDS: f32 = 1.0;

/// The birds at `t`: a loose flock of seven that crosses the light from right to left over
/// the plain, from [`BIRDS`] on. Each bird is a dark stroke for the body and two for the wings,
/// which beat and glide. `feet` is the place of the engineer.
fn birds_at(t: f32, feet: Vec3) -> Vec<Particle> {
    let (s_dir, right) = (sunward(), right());
    // They fly to the left and a little toward the camera, so the wings show.
    let along = (-right * 0.85 - s_dir * 0.5).normalize();
    let across = along.cross(Vec3::Y);
    let dark = Rgb::hex(0x1a120e);
    let mut items = Vec::new();
    for b in 0..7u32 {
        let h = |n: u32| hash(b * 13 + n * 7919 + 11);
        let flight = t - BIRDS - 0.5 * h(1);
        if !(0.0..8.0).contains(&flight) {
            continue;
        }
        // They fly 55 to 75 m beyond him, 4 to 10 m above the plateau: through the glow
        // of the sun.
        let start = feet
            + s_dir * (55.0 + 20.0 * h(2))
            + right * (45.0 + 10.0 * h(3))
            + Vec3::Y * (4.0 + 6.0 * h(4));
        let speed = 11.0 + 3.0 * h(5);
        let body = start
            + along * (speed * flight)
            + Vec3::Y * (1.5 * noise1(b + 20, flight * 0.4))
            + across * (3.0 * noise1(b + 40, flight * 0.3));
        // Beats, then a short glide with the wings up.
        let beat = (TAU * 3.2 * flight + 6.0 * h(6)).sin();
        let glide = smoothstep(0.3, 0.7, noise1(b + 60, flight * 0.7));
        let angle = 0.75 * beat * (1.0 - glide) + 0.25 * glide;
        let span = 0.38 + 0.08 * h(7);
        for side in [1.0f32, -1.0] {
            let tip = body + across * (side * span * angle.cos()) + Vec3::Y * (span * angle.sin());
            items.push(Particle::new(body, 0.05, dark).tail(tip - body));
        }
        items.push(Particle::new(body + along * 0.12, 0.07, dark).tail(-along * 0.3));
    }
    items
}
