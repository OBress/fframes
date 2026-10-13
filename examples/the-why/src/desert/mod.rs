//! The desert set: shots 5 ("The walk"), 6 ("The line") and 9 ("Sunrise").
//!
//! The world is a high desert plateau. The origin is the place where the engineer kneels
//! and draws the line. The factory stands 2.5 km away in the direction [`ahead`], on the
//! horizon. Shots 5 and 6 share the night scene. Shot 9 has its own scene with the low sun
//! and the landmarks that the machines built.

mod factory;
mod line;
mod sunrise;
mod terrain;
mod walk;

pub use factory::{Factory, factory};

use fframes_3d::math::hash;
use fframes_3d::*;

use crate::assets::Assets;
use crate::figure::{Figure, LINE_END, LINE_START};

/// The length of shot 5 in seconds.
pub const WALK_SECONDS: f32 = 10.0;
/// The length of shot 6 in seconds.
pub const LINE_SECONDS: f32 = 6.5;
/// The length of shot 9 in seconds.
pub const SUNRISE_SECONDS: f32 = 10.0;

/// The distance from the origin to the front of the factory, in meters.
pub const FACTORY_DISTANCE: f32 = 2500.0;

/// The light of the machines, cold blue.
const COLD: u32 = 0x7fb4ff;
/// The cold white of the machine light.
const COLD_WHITE: u32 = 0xcfe4ff;
/// The warm core of human intent.
const AMBER: u32 = 0xffb35c;
/// The warm edge of human intent.
const AMBER_EDGE: u32 = 0xff8a2a;

/// The direction from the origin to the factory, horizontal. Shot 4 ends looking at the
/// stars in a direction with this heading, so the tilt down of shot 5 finds the factory.
pub fn ahead() -> Vec3 {
    vec3(0.145, 0.0, 0.397).normalize()
}

/// The direction to the right of [`ahead`] when you look along it.
fn right() -> Vec3 {
    ahead().cross(Vec3::Y)
}

/// The heading of the kneeling engineer, in radians. With it the line that he draws
/// points at the factory.
fn kneel_heading() -> f32 {
    let line = LINE_END - LINE_START;
    let target = ahead();
    target.x.atan2(target.z) - line.x.atan2(line.z)
}

/// The world position of a point in the space of the kneeling figure.
fn kneel_point(local: Vec3) -> Vec3 {
    Quat::from_rotation_y(kneel_heading()) * local
}

/// The sets of the desert, built once.
pub struct DesertSet {
    /// The night plateau of shots 5 and 6.
    night: Scene,
    night_figure: Figure,
    /// The star field. It follows the camera.
    stars: NodeId,
    /// The blowing sand.
    dust: NodeId,
    /// The moon of the night scene.
    moon: NodeId,
    factory: Factory,
    line: line::LineNodes,
    /// The plateau at sunrise, shot 9.
    dawn: Scene,
    dawn_figure: Figure,
    dawn_dust: NodeId,
    landmarks: sunrise::Landmarks,
    /// The heights of the ground, for the paths of the dust and the light.
    terrain: terrain::Terrain,
}

impl std::fmt::Debug for DesertSet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DesertSet").finish_non_exhaustive()
    }
}

impl DesertSet {
    /// Builds the terrain, the factory, the landmarks, the figures and the lights of the
    /// three shots. It takes less than a second.
    pub fn new(assets: &Assets) -> Self {
        let terrain = terrain::Terrain::new();
        let ground = terrain.meshes(assets);

        let mut night = Scene::new();
        ground.add_to(&mut night, Rgb::WHITE);
        let factory = factory(&mut night, assets, ahead() * FACTORY_DISTANCE, -ahead());
        let night_figure = Figure::build(&mut night, None, &walk::figure_style());
        let stars = night.add(crate::stars::stars().scaled(STAR_SCALE));
        let dust = night.add(Node::particles(Vec::new(), Blending::Additive));
        let line = line::LineNodes::build(&mut night, assets);
        let moon = walk::light_night(&mut night);

        let mut dawn = Scene::new();
        // the sand at sunrise is seen against the sun, so it looks darker
        ground.add_to(&mut dawn, Rgb::linear(0.2, 0.17, 0.14));
        let landmarks = sunrise::landmarks(&mut dawn, &terrain);
        let dawn_figure = Figure::build(&mut dawn, None, &sunrise::figure_style());
        let dawn_dust = dawn.add(Node::particles(Vec::new(), Blending::Additive));
        sunrise::dawn_set(&mut dawn, assets, &terrain);

        Self {
            night,
            night_figure,
            stars,
            dust,
            moon,
            factory,
            line,
            dawn,
            dawn_figure,
            dawn_dust,
            landmarks,
            terrain,
        }
    }
}

/// The stars of `crate::stars` are 400 m from the camera. This scale puts them beyond
/// the mesas, so the land hides the stars behind it. Their size in the picture stays the
/// same.
const STAR_SCALE: f32 = 16.0;

/// The far clip plane of the cameras in meters. The mesas and the stars are inside it.
const FAR: f32 = 9000.0;

/// Smooth value noise in `0..1` on a grid of 1 m cells.
fn noise2(p: Vec2, seed: u32) -> f32 {
    let cell = p.floor();
    let f = p - cell;
    let u = f * f * (3.0 - 2.0 * f);
    let (x, y) = (cell.x as i32, cell.y as i32);
    let at = |i: i32, j: i32| {
        let key = (x + i) as u32 ^ ((y + j) as u32).wrapping_mul(0x9e37_79b9);
        hash(key.wrapping_mul(0x85eb_ca6b) ^ seed.wrapping_mul(0xc2b2_ae35))
    };
    let a = at(0, 0) + (at(1, 0) - at(0, 0)) * u.x;
    let b = at(0, 1) + (at(1, 1) - at(0, 1)) * u.x;
    a + (b - a) * u.y
}

/// Fractal noise in about `-1..1`: `octaves` layers of [`noise2`], each with twice the
/// frequency and half the amplitude of the layer before.
fn fbm(p: Vec2, octaves: u32, seed: u32) -> f32 {
    let mut sum = 0.0;
    let mut amplitude = 1.0;
    let mut total = 0.0;
    let mut q = p;
    for i in 0..octaves {
        sum += amplitude * (noise2(q, seed + i * 101) * 2.0 - 1.0);
        total += amplitude;
        amplitude *= 0.5;
        // the turn hides the grid of the cells
        q = Vec2::new(q.x * 1.6 - q.y * 1.2, q.x * 1.2 + q.y * 1.6);
    }
    sum / total
}

/// A unit cylinder along +Y from `y = 0` to `y = 1`, scaled and turned to go from `a` to
/// `b` with the radius `radius`. For instanced struts and cables.
fn segment(a: Vec3, b: Vec3, radius: f32) -> Mat4 {
    let d = b - a;
    let length = d.length().max(1e-4);
    Mat4::from_scale_rotation_translation(
        vec3(radius, length, radius),
        Quat::from_rotation_arc(Vec3::Y, d / length),
        a,
    )
}

/// Multiplies all light of `scene` by `k`: the lights, the environment, the haze, the
/// emissive materials and the particles. It works like the exposure of the renderer, but
/// the bloom also grows, because the bloom threshold stays the same.
fn expose(scene: &mut Scene, k: f32) {
    match &mut scene.environment {
        Environment::Map { intensity, .. }
        | Environment::Gradient { intensity, .. }
        | Environment::Studio(Studio { intensity, .. }) => *intensity *= k,
        Environment::None => {}
    }
    if let Some(air) = &mut scene.atmosphere {
        air.ambient = air.ambient * k;
    }
    let ids: Vec<NodeId> = scene.nodes().map(|(id, _)| id).collect();
    for id in ids {
        match &mut scene[id].object {
            Object::Light(light) => *light.intensity_mut() *= k,
            Object::Mesh(mesh) => {
                let m = &mut mesh.material;
                m.emissive_intensity *= k;
                if m.shading == Shading::Unlit {
                    m.color = m.color * k;
                }
            }
            Object::Particles(particles) => {
                for p in &mut particles.items {
                    p.color = p.color * k;
                }
            }
            Object::Group => {}
        }
    }
}

/// Hides the lights with the intensity 0. The renderer and the haze then skip them: each
/// light costs time in every step of the haze.
fn hide_dark_lights(scene: &mut Scene) {
    let ids: Vec<NodeId> = scene.nodes().map(|(id, _)| id).collect();
    for id in ids {
        let node = &mut scene[id];
        if node.light_mut().is_some_and(|l| *l.intensity_mut() <= 0.0) {
            node.visible = false;
        }
    }
}
