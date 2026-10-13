//! The factory on the horizon: a hall 1 km long and 60 m high with a sawtooth roof. Cold
//! light leaks from its window bands and roof lights and makes a dome of light in the haze.

use std::f32::consts::TAU;
use std::sync::Arc;

use fframes_3d::math::{hash, smoothstep};
use fframes_3d::*;

use super::{COLD, COLD_WHITE};
use crate::assets::Assets;

/// The length of the hall along its front, in meters.
pub const FACTORY_LENGTH: f32 = 1000.0;
/// The depth of the hall in meters.
pub const FACTORY_DEPTH: f32 = 320.0;
/// The height of the walls in meters. The teeth of the roof go up to about 60 m.
const WALL: f32 = 46.0;
/// The width of one tooth of the sawtooth roof in meters.
const TOOTH: f32 = 25.0;
/// The height of a tooth above the walls.
const TOOTH_HEIGHT: f32 = 14.0;

/// The nodes of the factory that the shots animate.
pub struct Factory {
    /// The root of the factory. Its origin is the middle of the front wall on the ground.
    pub root: NodeId,
    /// The red lights on the tops of the stacks. They blink.
    pub beacons: NodeId,
    /// The warm flash where the light of the line arrives at the end of shot 6. It is a
    /// glow mesh with the intensity 0.
    pub flash: NodeId,
    /// The warm light of the flash, intensity 0.
    pub flash_light: NodeId,
}

/// Adds the factory to `scene`. `front` is the middle of the front wall on the ground, and
/// the front faces `facing` (horizontal). The interior of shot 7 matches this building: a
/// glass front with three bands of windows, a sawtooth roof of 40 teeth with glass faces,
/// five stacks behind the hall and tanks on the left side.
pub fn factory(scene: &mut Scene, assets: &Assets, front: Vec3, facing: Vec3) -> Factory {
    let turn = Quat::from_rotation_arc(Vec3::Z, facing.with_y(0.0).normalize());
    let root = scene.add(Node::group().with_position(front).with_rotation(turn));
    let (half, depth) = (FACTORY_LENGTH * 0.5, FACTORY_DEPTH);

    // The walls and the roof: one dark mass of corrugated metal.
    let mut shell = vec![Geometry::cuboid(FACTORY_LENGTH, WALL, depth).translate(
        0.0,
        WALL * 0.5,
        -depth * 0.5,
    )];
    let teeth = (FACTORY_LENGTH / TOOTH) as usize;
    for i in 0..teeth {
        let x0 = -half + i as f32 * TOOTH;
        // a prism: the slope rises along +X to the glass face
        let shape = [
            vec2(0.0, 0.0),
            vec2(TOOTH, 0.0),
            vec2(TOOTH, TOOTH_HEIGHT),
            vec2(TOOTH - 1.2, TOOTH_HEIGHT),
        ];
        let prism = Geometry::extrude(&shape, depth - 6.0, 0.0).translate(x0, WALL, -depth * 0.5);
        shell.push(prism);
    }
    // the annex in front of the left end, and the low wing on the right
    shell.push(Geometry::cuboid(160.0, 24.0, 90.0).translate(-half + 110.0, 12.0, 45.0));
    shell.push(Geometry::cuboid(220.0, 18.0, 140.0).translate(half + 110.0, 9.0, -60.0));
    let mut metal = assets.corrugated.material(0x5a6270, 0.7, 0.6);
    metal.env_intensity = 0.6;
    scene.add_to(
        root,
        Node::mesh(Geometry::merge(shell), metal.clone()).shadows(false, false),
    );

    // The glass faces of the teeth glow. They face +X, away from the slopes.
    let mut roof_glass = Vec::new();
    for i in 0..teeth {
        let x = -half + (i + 1) as f32 * TOOTH - 0.05;
        let pane = Geometry::plane(depth - 10.0, TOOTH_HEIGHT - 2.0)
            .rotate_y(TAU * 0.25)
            .translate(x, WALL + TOOTH_HEIGHT * 0.5, -depth * 0.5);
        roof_glass.push(pane.with_color(Rgb::WHITE * (0.4 + 0.8 * hash(i as u32 * 7 + 1))));
    }
    scene.add_to(
        root,
        Node::mesh(
            Geometry::merge(roof_glass),
            Material::standard(0x0a0c10)
                .emissive(COLD_WHITE, 9.0)
                .double_sided(),
        )
        .shadows(false, false),
    );

    // Three bands of windows on the front. Some parts are dark, so the light reads as
    // rows of leaks and not as one strip.
    let panes = Arc::new(window_texture());
    let mut bands = Vec::new();
    for (y, h) in [(9.0, 6.0), (21.0, 6.0), (33.0, 7.0)] {
        bands.push(Geometry::plane(FACTORY_LENGTH - 20.0, h).translate(0.0, y, 0.3));
    }
    let mut glass = Material::standard(0x06080c).emissive(COLD_WHITE, 4.0);
    glass.emissive_map = Some(panes);
    scene.add_to(
        root,
        Node::mesh(Geometry::merge(bands), glass).shadows(false, false),
    );
    // the big doors of the loading bays: tall slots of light at the ground
    let doors: Vec<Geometry> = (0..9)
        .map(|i| {
            let x = -360.0 + i as f32 * 90.0 + 20.0 * hash(i * 3 + 5);
            Geometry::plane(14.0, 16.0).translate(x, 8.0, 0.35)
        })
        .collect();
    scene.add_to(
        root,
        Node::mesh(Geometry::merge(doors), Material::glow(COLD, 3.5)).shadows(false, false),
    );

    // Five stacks behind the hall and tanks on the left side.
    let mut stacks = Vec::new();
    let mut beacon_points = Vec::new();
    for i in 0..5 {
        let x = -300.0 + i as f32 * 140.0 + 30.0 * hash(i * 11);
        let height = 95.0 + 40.0 * hash(i * 11 + 1);
        stacks.push(Geometry::cylinder(4.5, 6.0, height, 16).translate(
            x,
            height * 0.5,
            -depth + 40.0,
        ));
        beacon_points.push(vec3(x, height + 1.5, -depth + 40.0));
    }
    for i in 0..4 {
        let x = -half - 40.0 - 36.0 * i as f32;
        stacks.push(Geometry::cylinder(15.0, 15.0, 30.0, 24).translate(
            x,
            15.0,
            -70.0 - 30.0 * (i % 2) as f32,
        ));
    }
    // a control tower at the right end
    stacks.push(Geometry::cuboid(22.0, 82.0, 22.0).translate(half - 60.0, 41.0, -30.0));
    scene.add_to(
        root,
        Node::mesh(Geometry::merge(stacks), metal).shadows(false, false),
    );
    scene.add_to(
        root,
        Node::mesh(
            Geometry::cuboid(26.0, 6.0, 26.0).translate(half - 60.0, 85.0, -30.0),
            Material::standard(0x06080c).emissive(COLD_WHITE, 5.0),
        )
        .shadows(false, false),
    );
    let beacons = scene.add_to(
        root,
        Node::particles(
            beacon_points
                .iter()
                .map(|&p| Particle::new(p, 2.2, Rgb::hex(0xff3020) * 30.0))
                .collect(),
            Blending::Additive,
        ),
    );

    // The dome of cold light in the haze over the hall: a soft glow behind the roof, and
    // lamps over the roof that light the air and the ground around the factory.
    let dome = glow_card(2800.0, 1300.0, 0.0);
    scene.add_to(
        root,
        Node::mesh(dome, Material::glow(COLD, 0.3))
            .at(0.0, 0.0, -depth - 60.0)
            .shadows(false, false),
    );
    let low = glow_card(1500.0, 160.0, 0.0);
    scene.add_to(
        root,
        Node::mesh(low, Material::glow(COLD_WHITE, 0.12))
            .at(0.0, 0.0, 30.0)
            .shadows(false, false),
    );
    for x in [-330.0, 0.0, 330.0] {
        scene.add_to(
            root,
            Node::light(Light::point(COLD_WHITE, 5.5e5, 900.0)).at(x, 38.0, -depth * 0.4),
        );
    }

    // Light leaks from vents on the roof and rises into the haze as pale beams.
    let mut beams = Vec::new();
    for i in 0..14u32 {
        let x = -half + 40.0 + (FACTORY_LENGTH - 80.0) * (i as f32 + 0.5 * hash(i * 5 + 2)) / 14.0;
        let z = -depth * (0.25 + 0.5 * hash(i * 5 + 3));
        let height = 220.0 + 180.0 * hash(i * 5 + 4);
        // a cone that widens upward, with rings along its height for the fade
        let profile: Vec<Vec2> = (0..=12)
            .map(|k| {
                let u = k as f32 / 12.0;
                vec2(5.0 + 13.0 * u, height * u)
            })
            .collect();
        let mut beam = Geometry::lathe(&profile, 12).translate(x, WALL + TOOTH_HEIGHT, z);
        let bottom = WALL + TOOTH_HEIGHT;
        let strength = 0.5 + 0.5 * hash(i * 5 + 5);
        beam.colors = beam
            .positions
            .iter()
            .map(|p| {
                let u = ((p.y - bottom) / height).clamp(0.0, 1.0);
                vec4(
                    1.0,
                    1.0,
                    1.0,
                    strength * (1.0 - u).powi(2) * smoothstep(0.0, 0.05, u),
                )
            })
            .collect();
        beams.push(beam);
    }
    scene.add_to(
        root,
        Node::mesh(
            Geometry::merge(beams),
            Material::glow(COLD, 0.22).double_sided(),
        )
        .shadows(false, false),
    );

    // the flash of the warm light at the end of shot 6
    let flash = scene.add_to(
        root,
        Node::mesh(
            glow_card(700.0, 260.0, 0.0),
            Material::glow(super::AMBER, 0.0),
        )
        .at(0.0, 0.0, 40.0)
        .shadows(false, false),
    );
    let flash_light = scene.add_to(
        root,
        Node::light(Light::point(super::AMBER, 0.0, 3000.0)).at(0.0, 30.0, 60.0),
    );

    Factory {
        root,
        beacons,
        flash,
        flash_light,
    }
}

/// A grid in the XY plane that faces +Z with a soft glow: the vertex alpha falls from 1 at
/// the bottom middle to 0 at the edges. `width` and `height` in meters, `lift` moves the
/// brightest point up.
fn glow_card(width: f32, height: f32, lift: f32) -> Geometry {
    let (columns, rows) = (32u32, 16u32);
    let mut positions = Vec::new();
    let mut colors = Vec::new();
    for j in 0..=rows {
        for i in 0..=columns {
            let u = i as f32 / columns as f32 * 2.0 - 1.0;
            let v = j as f32 / rows as f32;
            positions.push(vec3(u * width * 0.5, v * height, 0.0));
            let d = (u * u + (v - lift).powi(2) * 1.0).sqrt().min(1.0);
            let a = (1.0 - d).powi(2) * (1.0 - d * 0.5);
            colors.push(vec4(1.0, 1.0, 1.0, a));
        }
    }
    let mut indices = Vec::new();
    for j in 0..rows {
        for i in 0..columns {
            let a = j * (columns + 1) + i;
            let (b, c, d) = (a + 1, a + columns + 1, a + columns + 2);
            indices.extend([a, b, d, a, d, c]);
        }
    }
    let normals = vec![Vec3::Z; positions.len()];
    let mut geometry = Geometry::new(positions, normals, Vec::new(), indices);
    geometry.colors = colors;
    geometry
}

/// The panes of the window bands: sections of lit glass of different brightness and some
/// dark sections. White, the emissive color of the material tints it.
fn window_texture() -> Texture {
    let sections = 48.0;
    let mut texture = Texture::from_fn(512, 16, |u, v| {
        let section = (u * sections) as u32;
        let lit = hash(section * 13 + 1);
        let level = if lit < 0.35 {
            0.04
        } else {
            0.25 + 0.75 * hash(section * 13 + 2).powi(2)
        };
        // mullions every few meters and a frame at the top and the bottom
        let mullion = ((u * sections * 6.0).fract() < 0.12) || !(0.12..0.88).contains(&v);
        let c = if mullion { 0 } else { (level * 255.0) as u8 };
        fframes::Color::rgb(c, c, c)
    });
    texture.repeat = vec2(1.0, 1.0);
    texture
}
