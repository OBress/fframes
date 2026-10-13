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
/// The colors of the code on the screens (sRGB) and the part of the screens with each
/// color. All are cold: warm light is for human intent, later in the film.
const TINTS: [(u32, f32); 5] = [
    (0x9cc4ff, 0.45),
    (0xbcd6ff, 0.2),
    (0x7fb4ff, 0.15),
    (0xa8e2ff, 0.12),
    (0xe2ecff, 0.08),
];
/// The code of the main screen in the wide views: a cold white, brighter than the others.
const HERO: u32 = 0xeef4ff;
const HERO_LIGHT: f32 = 7.0;
/// The light of the panel of the main screen in the wide views.
const HERO_PANEL: f32 = 0.04;
/// The light of the small lamps on the floor along the aisles.
const MARKER_LIGHT: f32 = 2.5;

/// The state of the main screen in the field at one moment.
pub struct MainScreen {
    /// The page line at the top of the screen.
    pub lines: f32,
    /// The light of the page on the screen. It is 0 where meshes show the text.
    pub code: f32,
    /// The light that the screen throws on the floor.
    pub pool: f32,
    /// How much the screen stands out of the field in the wide views, `0..1`: white code
    /// on a glowing panel, so the eye finds it among the thousands.
    pub hero: f32,
}

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
    /// The color of the code, linear.
    tint: Rgb,
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
    /// A small lamp on the floor.
    marker_lamp: Arc<Geometry>,
    /// The small lamps on the floor along the aisles, and their distances from the main
    /// monitor (`0..1`).
    markers: Vec<(Mat4, f32)>,
    content: Material,
    /// The code of the main screen in the wide views.
    hero: Material,
    bleed: Material,
    pool_light: Material,
    marker: Material,
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
                let x = column_x(column);
                monitors.push(Monitor {
                    position: vec3(x, SCREEN_Y, -(row as f32) * ROW_PITCH),
                    distance: distance(x, row as f32 * ROW_PITCH),
                    ahead: hash(seed * 13) * 2.0,
                    phase: (hash(seed * 7 + 3) * PAGE_LINES as f32).floor(),
                    light: 0.55 + 0.8 * hash(seed * 5 + 1).powi(2),
                    tint: tint(hash(seed * 11 + 2)),
                });
            }
        }
        // two lines of lamps along each aisle, one lamp every half row
        let mut markers = Vec::new();
        for column in -SIDE..SIDE {
            if (column + 3 + 1).rem_euclid(BLOCK) != 0 {
                continue;
            }
            let left = column_x(column) + 1.0;
            let right = column_x(column + 1) - 1.0;
            for half_row in -2 * FRONT..=2 * BACK {
                let z = -(half_row as f32) * ROW_PITCH * 0.5 - ROW_PITCH * 0.25;
                for x in [left, right] {
                    markers.push((Mat4::from_translation(vec3(x, 0.006, z)), distance(x, z)));
                }
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
            marker_lamp: Arc::new(Geometry::cuboid(0.05, 0.012, 0.3)),
            markers,
            content: Material::unlit(Rgb::WHITE * CODE_LIGHT).map(page.clone()),
            hero: Material::unlit(Rgb::hex(HERO) * HERO_LIGHT)
                .map(page)
                .additive(),
            bleed: Material::unlit(Rgb::hex(BLEED) * BLEED_LIGHT),
            pool_light: Material::glow(Rgb::WHITE, POOL_LIGHT).map(pool()),
            marker: Material::glow(0xd8e6ff, MARKER_LIGHT),
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

    /// Adds the screens of the monitors at `t` seconds: the panels, the code, the light on
    /// the floor and the lamps along the aisles. `adapt` multiplies the light of the code.
    pub fn add_screens(&self, scene: &mut Scene, t: f32, main: &MainScreen, adapt: f32) {
        let count = self.windows.len();
        let window = |lines: f32| (lines * STEPS as f32).floor().rem_euclid(count as f32) as usize;
        let mut windows: Vec<(Vec<Mat4>, Vec<Rgb>)> = vec![(Vec::new(), Vec::new()); count];
        let mut panels = (Vec::new(), Vec::new());
        let mut pools = (Vec::new(), Vec::new());
        let at = Mat4::from_translation(vec3(0.0, SCREEN_Y, 0.0));
        if main.code > 0.0 {
            // the main screen fades from the code of the field to the white of the hero
            let k = window(main.lines);
            windows[k].0.push(at);
            windows[k]
                .1
                .push(Rgb::hex(CODE) * (main.code * adapt * (1.0 - main.hero)));
            if main.hero > 0.0 {
                // white code and a glow over the whole screen, added to the light of the
                // screen
                let light = main.code * adapt * main.hero;
                let code = self
                    .hero
                    .clone()
                    .color(Rgb::hex(HERO) * (HERO_LIGHT * light));
                let panel = Material::glow(HERO, HERO_PANEL * light);
                for (geometry, material, z) in [
                    (&self.windows[k], code, 0.0003),
                    (&self.panel, panel, 0.001),
                ] {
                    scene.add(
                        Node::mesh(geometry.clone(), material)
                            .at(0.0, SCREEN_Y, z)
                            .shadows(false, false),
                    );
                }
            }
        }
        if main.pool > 0.0 {
            pools.0.push(at);
            pools
                .1
                .push(Rgb::hex(CODE) * (main.pool * (1.0 + 2.0 * main.hero)));
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
            let k = window(lines);
            windows[k].0.push(at);
            windows[k].1.push(m.tint * (light * adapt));
            pools.0.push(at);
            pools.1.push(m.tint * light);
        }
        let mut markers = (Vec::new(), Vec::new());
        for (at, distance) in &self.markers {
            let light = screen_light(t, *distance);
            if light > 0.0 {
                markers.0.push(*at);
                markers.1.push(Rgb::WHITE * light);
            }
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
        add(&self.marker_lamp, &self.marker, markers);
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

/// The X position of the monitors in `column`, in meters. Blocks of [`BLOCK`] monitors
/// have aisles between them. The main monitor is in column 0.
fn column_x(column: i32) -> f32 {
    column as f32 * COLUMN_PITCH + (column + 3).div_euclid(BLOCK) as f32 * AISLE
}

/// The distance of `x`, `z` from the main monitor, `0..1` of the farthest monitors.
fn distance(x: f32, z: f32) -> f32 {
    (x.hypot(z) / 120.0).min(1.0)
}

/// The color of the code of a screen (linear) for the random value `pick` in `0..1`.
fn tint(pick: f32) -> Rgb {
    let mut sum = 0.0;
    for (hex, part) in TINTS {
        sum += part;
        if pick < sum {
            return Rgb::hex(hex);
        }
    }
    Rgb::hex(TINTS[0].0)
}
