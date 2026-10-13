//! The field of screens: thousands of monitors on stands, in rows on a dark floor. They all
//! write code at once.

use std::f32::consts::FRAC_PI_2;
use std::sync::Arc;

use fframes_3d::math::{hash, smoothstep};
use fframes_3d::*;

use super::terminal::{self, PAGE_LINES, ROWS, SCREEN};

/// The distance between the monitors of a row, in meters.
const COLUMN_PITCH: f32 = 2.0;
/// The monitors stand in blocks of this many, with an aisle between the blocks.
const BLOCK: i32 = 6;
/// The extra width of an aisle, in meters.
const AISLE: f32 = 1.8;
/// The distance between the rows, in meters.
const ROW_PITCH: f32 = 3.2;
/// The height of the center of the screens above the floor, in meters.
pub const SCREEN_Y: f32 = 1.3;
/// The monitors on each side of the main monitor.
const SIDE: i32 = 34;
/// The rows in front of the main monitor.
const FRONT: i32 = 7;
/// The rows behind the main monitor.
const BACK: i32 = 46;
/// The scroll steps for each line. The screens far away show the page through one of
/// `PAGE_LINES * STEPS` windows.
const STEPS: usize = 4;
/// The width of the frame around the screen, in meters.
const BEZEL: f32 = 0.014;
/// The height of the frame below the screen, in meters.
const CHIN: f32 = 0.03;
/// The size of the light of a screen on the floor in front of it, in meters.
const POOL: f32 = 3.0;

/// The code on the screens (sRGB) and its light.
pub const CODE: u32 = 0x9cc4ff;
pub const CODE_LIGHT: f32 = 2.2;
/// The light that the screen panel lets through where it is black.
pub const BLEED: u32 = 0x0a1424;
pub const BLEED_LIGHT: f32 = 0.35;
/// The light of a screen on the floor below it, at the brightest point.
const POOL_LIGHT: f32 = 0.07;

/// One monitor of the field.
struct Monitor {
    /// The center of the screen, in meters.
    position: Vec3,
    /// The distance from the main monitor, `0..1` of the farthest.
    distance: f32,
    /// How many seconds the terminal of the monitor is ahead of the main terminal.
    ahead: f32,
    /// The page line at the top of the screen at the start.
    phase: f32,
    /// The light of the screen relative to the others.
    light: f32,
}

/// The monitors and the meshes of their screens.
pub struct Field {
    /// All monitors except the main monitor.
    monitors: Vec<Monitor>,
    /// The windows on the page, see [`STEPS`].
    windows: Vec<Arc<Geometry>>,
    /// The black panel of a screen.
    panel: Arc<Geometry>,
    /// The light of a screen on the floor.
    pool: Arc<Geometry>,
    content: Material,
    bleed: Material,
    pool_light: Material,
}

impl Field {
    pub fn new(page: Arc<Texture>) -> Self {
        let mut monitors = Vec::new();
        for row in -FRONT..=BACK {
            for column in -SIDE..=SIDE {
                if row == 0 && column == 0 {
                    continue;
                }
                let seed = ((row + FRONT) * (2 * SIDE + 1) + column + SIDE) as u32;
                let x =
                    column as f32 * COLUMN_PITCH + (column + 3).div_euclid(BLOCK) as f32 * AISLE;
                monitors.push(Monitor {
                    position: vec3(x, SCREEN_Y, -(row as f32) * ROW_PITCH),
                    distance: (x.hypot(row as f32 * ROW_PITCH) / 120.0).min(1.0),
                    ahead: hash(seed * 13) * 2.0,
                    phase: (hash(seed * 7 + 3) * PAGE_LINES as f32).floor(),
                    light: 0.75 + 0.5 * hash(seed * 5 + 1),
                });
            }
        }
        let (center, size) = terminal::text_area();
        let count = PAGE_LINES * STEPS;
        let windows = (0..count)
            .map(|k| {
                let v0 = k as f32 / count as f32;
                let v1 = v0 + ROWS as f32 / PAGE_LINES as f32;
                let mut quad = Geometry::plane(size.x, size.y);
                quad.uvs = vec![vec2(0.0, v1), vec2(1.0, v1), vec2(1.0, v0), vec2(0.0, v0)];
                Arc::new(quad.translate(center.x, center.y, 0.0006))
            })
            .collect();
        Self {
            monitors,
            windows,
            panel: Arc::new(Geometry::plane(SCREEN.x, SCREEN.y)),
            pool: Arc::new(Geometry::plane(POOL, POOL).rotate_x(-FRAC_PI_2).translate(
                0.0,
                -SCREEN_Y + 0.004,
                POOL * 0.5,
            )),
            content: Material::unlit(Rgb::hex(CODE) * CODE_LIGHT).map(page),
            bleed: Material::unlit(Rgb::hex(BLEED) * BLEED_LIGHT),
            pool_light: Material::glow(CODE, POOL_LIGHT).map(pool()),
        }
    }

    /// Adds the bodies of all monitors, the main monitor too, to `scene`. The monitors
    /// near the main monitor have rounded edges, the others are plain boxes. Only the main
    /// monitor has glass: the lights cast no shadows, so the glass of the others would
    /// reflect the cursor through the main monitor.
    pub fn add_monitors(&self, scene: &mut Scene) {
        let material = Material::standard(Rgb::WHITE)
            .metalness(0.6)
            .roughness(0.38);
        let (near, far): (Vec<Vec3>, Vec<Vec3>) = std::iter::once(vec3(0.0, SCREEN_Y, 0.0))
            .chain(self.monitors.iter().map(|m| m.position))
            .partition(|p| p.x.hypot(p.z) < 14.0);
        for (bodies, rounded) in [(near, true), (far, false)] {
            scene.add(Node::instanced(
                body(rounded),
                material.clone(),
                Instances::new(bodies.into_iter().map(Mat4::from_translation)),
            ));
        }
    }

    /// Adds the screens of the monitors at `t` seconds: the panels, the code and the light
    /// on the floor. The main monitor shows the page from line `main_lines` with the light
    /// `main_code`, and lights the floor with `main_pool`. `adapt` multiplies the light of
    /// the code.
    pub fn add_screens(
        &self,
        scene: &mut Scene,
        t: f32,
        main_lines: f32,
        main_code: f32,
        main_pool: f32,
        adapt: f32,
    ) {
        let count = self.windows.len();
        let window = |lines: f32| (lines * STEPS as f32).floor() as i64;
        let mut windows: Vec<(Vec<Mat4>, Vec<Rgb>)> = vec![(Vec::new(), Vec::new()); count];
        let mut panels = (Vec::new(), Vec::new());
        let mut pools = (Vec::new(), Vec::new());
        let main = Mat4::from_translation(vec3(0.0, SCREEN_Y, 0.0));
        if main_code > 0.0 {
            let k = window(main_lines).rem_euclid(count as i64) as usize;
            windows[k].0.push(main);
            windows[k].1.push(Rgb::WHITE * (main_code * adapt));
        }
        if main_pool > 0.0 {
            pools.0.push(main);
            pools.1.push(Rgb::WHITE * main_pool);
        }
        for m in &self.monitors {
            let light = screen_light(t, m.distance) * m.light;
            let at = Mat4::from_translation(m.position);
            // a dark panel stays in front of the frame of the monitor
            panels.0.push(at);
            panels.1.push(Rgb::WHITE * light);
            if light <= 0.0 {
                continue;
            }
            let lines = super::written(t.min(super::ANSWER_END) + m.ahead) + m.phase;
            let k = window(lines).rem_euclid(count as i64) as usize;
            windows[k].0.push(at);
            windows[k].1.push(Rgb::WHITE * (light * adapt));
            pools.0.push(at);
            pools.1.push(Rgb::WHITE * light);
        }
        let mut add =
            |geometry: &Arc<Geometry>, material: &Material, (at, colors): (Vec<Mat4>, Vec<Rgb>)| {
                if !at.is_empty() {
                    scene.add(
                        Node::instanced(
                            geometry.clone(),
                            material.clone(),
                            Instances::new(at).colors(colors),
                        )
                        .shadows(false, false),
                    );
                }
            };
        add(&self.panel, &self.bleed, panels);
        add(&self.pool, &self.pool_light, pools);
        for (k, list) in windows.into_iter().enumerate() {
            add(&self.windows[k], &self.content, list);
        }
    }
}

/// The glass in front of a screen: it adds the reflections of the lights and of the room.
pub fn glass() -> Material {
    Material::standard(Rgb::BLACK).roughness(0.1).additive()
}

/// The body of a monitor without its screen: the frame, the back and the stand on its
/// foot. The screen is at Z = 0 with its center at the origin, and the floor is at
/// Y = -`SCREEN_Y`. A body that is not `rounded` has sharp edges and few triangles, for the
/// monitors far away.
fn body(rounded: bool) -> Geometry {
    let part = |width: f32, height: f32, depth: f32, radius: f32| {
        if rounded {
            Geometry::rounded_box(width, height, depth, radius, 2)
        } else {
            Geometry::cuboid(width, height, depth)
        }
    };
    let frame_height = SCREEN.y + BEZEL + CHIN;
    let depth = 0.03;
    let frame = part(SCREEN.x + 2.0 * BEZEL, frame_height, depth, 0.005)
        .translate(0.0, (BEZEL - CHIN) * 0.5, -depth * 0.5 - 0.0004)
        .with_color(0x3a3e45);
    let back = part(SCREEN.x * 0.62, SCREEN.y * 0.55, 0.05, 0.012)
        .translate(0.0, -0.04, -depth - 0.024)
        .with_color(0x2a2d33);
    let neck_top = -0.06;
    let neck_bottom = -SCREEN_Y + 0.012;
    let neck = part(0.075, neck_top - neck_bottom, 0.032, 0.01)
        .translate(0.0, f32::midpoint(neck_top, neck_bottom), -depth - 0.065)
        .with_color(0x9aa0a8);
    let foot = part(0.46, 0.018, 0.28, 0.008)
        .translate(0.0, -SCREEN_Y + 0.009, -depth - 0.065)
        .with_color(0x9aa0a8);
    Geometry::merge([frame, back, neck, foot])
}

/// The light of a screen on the floor in front of it, for a square of [`POOL`] meters
/// that starts below the screen. The screen is a flat emitter at the height `SCREEN_Y`.
/// The light that falls on the floor at a distance `d` is `z h / d⁴` (cosine at the
/// screen, cosine at the floor, inverse square). The brightest point is 1.
fn pool() -> Texture {
    let h = SCREEN_Y;
    let light = |x: f32, z: f32| z.max(0.0) * h / (x * x + h * h + z * z).powi(2);
    let peak = light(0.0, h / 3f32.sqrt());
    Texture::from_fn(64, 64, |u, v| {
        let (x, z) = ((u - 0.5) * POOL, v * POOL);
        // the square fades out at its sides and its far end, so its edges do not show
        let edge =
            smoothstep(0.0, 0.2, 1.0 - (u - 0.5).abs() * 2.0) * (1.0 - smoothstep(0.75, 1.0, v));
        let l = (light(x, z) / peak).powf(1.5) * edge;
        Rgb::linear(l, l, l).to_color()
    })
}

/// The light of a screen at `distance` (`0..1`) from the main monitor at `t` seconds, `0..1`.
/// The screens come on in a wave from the main monitor when the machine answers. They go
/// dark in a wave from the far ones in, and the main monitor goes dark last.
pub fn screen_light(t: f32, distance: f32) -> f32 {
    let on = super::ANSWER_START + 0.05 + 0.5 * distance;
    let off = super::DARK_START + 0.32 * (1.0 - distance);
    smoothstep(on, on + 0.12, t) * (1.0 - smoothstep(off, off + 0.08, t))
}
