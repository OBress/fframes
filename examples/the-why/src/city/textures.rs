//! Procedural textures of the city: the facades with their lit windows, the roofs, and the
//! pools of light that the street lamps make on the asphalt.

use std::sync::Arc;

use fframes_3d::math::hash;
use fframes_3d::{Rgb, Texture, Vec3};

use super::layout::Style;

/// The window bays across one facade texture.
pub const BAYS: f32 = 32.0;
/// The floors down one facade texture.
pub const FLOORS: f32 = 48.0;
/// Texels across one window bay and down one floor.
const CELL: u32 = 32;
/// The emitted light of the brightest texel of a window. The texels keep half of the
/// light, so the ceiling lights can be two times brighter than a lit room.
pub const WINDOW_LIGHT: f32 = 2.0;

/// The maps of one facade style. One texture covers `BAYS` bays and `FLOORS` floors.
pub struct Facade {
    pub color: Arc<Texture>,
    /// The lit windows.
    pub emissive: Arc<Texture>,
    /// Roughness in green and metalness in blue, as in glTF.
    pub metal_rough: Arc<Texture>,
}

/// The cold colors of office light, sRGB.
const TONES: [u32; 8] = [
    0xe4eeff, 0xf2f5ff, 0xc2d8ff, 0x9cc0ff, 0xd2f4ff, 0xdde6f2, 0xe4eeff, 0xfff1e0,
];

/// How a style divides a cell of one bay and one floor.
struct Cell {
    /// The vision glass: `x0, x1, y0, y1` in `0..1` from the left and from the top.
    glass: [f32; 4],
    wall: u32,
    glass_color: u32,
    /// The roughness and the metalness of the glass. Coated glass reflects like metal.
    glass_finish: [f32; 2],
    /// The roughness and the metalness of the wall.
    wall_finish: [f32; 2],
    /// The parts of the floors that are dark and that are partly lit. The rest is lit.
    floors: [f32; 2],
}

fn cell(style: Style) -> Cell {
    match style {
        Style::Glass => Cell {
            glass: [0.05, 0.95, 0.05, 0.78],
            wall: 0x262c34,
            glass_color: 0x4a5a70,
            glass_finish: [0.14, 0.75],
            wall_finish: [0.35, 0.8],
            floors: [0.35, 0.35],
        },
        Style::Fins => Cell {
            glass: [0.12, 0.88, 0.03, 0.9],
            wall: 0x8c929a,
            glass_color: 0x0a0f16,
            glass_finish: [0.05, 0.0],
            wall_finish: [0.3, 0.9],
            floors: [0.42, 0.3],
        },
        Style::Office => Cell {
            glass: [0.2, 0.8, 0.16, 0.72],
            wall: 0x6c6f74,
            glass_color: 0x111720,
            glass_finish: [0.05, 0.0],
            wall_finish: [0.85, 0.0],
            floors: [0.3, 0.4],
        },
        Style::Low => Cell {
            glass: [0.24, 0.76, 0.2, 0.68],
            wall: 0x4b4a4c,
            glass_color: 0x10151c,
            glass_finish: [0.05, 0.0],
            wall_finish: [0.9, 0.0],
            floors: [0.5, 0.35],
        },
    }
}

fn srgb(c: u32) -> [u8; 4] {
    [(c >> 16) as u8, (c >> 8) as u8, c as u8, 255]
}

impl Facade {
    pub fn new(style: Style, seed: u32) -> Self {
        let (w, h) = (BAYS as u32 * CELL, FLOORS as u32 * CELL);
        let c = cell(style);
        let [x0, x1, y0, y1] = c.glass;
        // The cell, the position in the cell, and whether it is glass.
        let at = |u: f32, v: f32| {
            let (x, y) = (u * BAYS, v * FLOORS);
            let (fx, fy) = (x.fract(), y.fract());
            let glass = fx > x0 && fx < x1 && fy > y0 && fy < y1;
            (x as u32, y as u32, fx, fy, glass)
        };
        let color = Texture::from_fn_data(w, h, true, |u, v| {
            let (_, _, _, _, glass) = at(u, v);
            srgb(if glass { c.glass_color } else { c.wall })
        });
        let metal_rough = Texture::from_fn_data(w, h, false, |u, v| {
            let (_, _, _, _, glass) = at(u, v);
            let [rough, metal] = if glass { c.glass_finish } else { c.wall_finish };
            [255, (rough * 255.0) as u8, (metal * 255.0) as u8, 255]
        });
        let emissive = Texture::from_fn_data(w, h, true, |u, v| {
            let (cx, cy, fx, fy, glass) = at(u, v);
            if !glass {
                return [0, 0, 0, 255];
            }
            let light = room_light(
                seed,
                &c,
                cx,
                cy,
                (fx - x0) / (x1 - x0),
                (fy - y0) / (y1 - y0),
            );
            let out = Rgb::from(light / WINDOW_LIGHT).to_color();
            [out.r, out.g, out.b, 255]
        });
        Self {
            color: Arc::new(color),
            emissive: Arc::new(emissive),
            metal_rough: Arc::new(metal_rough),
        }
    }
}

/// The linear light of a window at `px`, `py` (`0..1` over the glass) in bay `cx` of floor
/// `cy`. The rooms of a floor are runs of bays with one light. Whole floors are dark,
/// partly lit or lit.
fn room_light(seed: u32, c: &Cell, cx: u32, cy: u32, px: f32, py: f32) -> Vec3 {
    let floor = seed.wrapping_mul(977).wrapping_add(cy * 131);
    // Large dark and bright patches over the facade.
    let patch = hash(seed + (cx / 5) * 71 + (cy / 6) * 13);
    let class = hash(floor);
    let mut lit = if class < c.floors[0] {
        0.03
    } else if class < c.floors[0] + c.floors[1] {
        0.45
    } else {
        0.92
    };
    lit *= 0.55 + 0.9 * patch;
    // A room starts at a bay where the hash is low, and runs to the right.
    let mut start = cx;
    while start > 0 && hash(floor + start * 7) >= 0.2 {
        start -= 1;
    }
    let room = floor.wrapping_add(start * 7919);
    let window = floor.wrapping_add(cx * 31);
    if hash(room + 1) > lit {
        return Rgb::hex(0x1a2a40).0 * 0.015;
    }
    let tone = Rgb::hex(TONES[(hash(room + 2) * TONES.len() as f32) as usize % TONES.len()]).0;
    let level = 0.16 + 0.6 * hash(room + 3).powf(1.8) * (0.8 + 0.4 * hash(window + 8));
    // Curtains: an even, soft light with folds.
    if hash(window + 6) < 0.12 {
        let fold = ((px * 9.0 + hash(window + 10) * 3.0).sin() * 0.5 + 0.5) * 0.25;
        return tone * level * (0.5 + fold);
    }
    // The light falls off from the ceiling. The ceiling lights shine at the top.
    let mut l = level * (1.0 - 0.45 * py);
    if py < 0.08 {
        l *= 2.0;
    }
    // Desks and partitions in front of the lower part.
    if py > 0.66 {
        let block = hash(window + 11 + (px * 3.0) as u32);
        l *= if block < 0.5 { 0.3 } else { 0.62 };
    }
    // Blinds, part way down, with their slats.
    if hash(window + 6) < 0.32 && py < 0.15 + 0.75 * hash(window + 7) {
        let slat = ((py * 40.0).fract() - 0.5).abs() * 2.0;
        l = level * (0.35 + 0.15 * slat);
    }
    tone * l
}

/// A flat roof: a dark membrane with seams and a light parapet at the edge. On the chip the
/// same picture is a component with a metal rim.
pub fn roof() -> Arc<Texture> {
    let size = 256;
    Arc::new(Texture::from_fn(size, size, |u, v| {
        let edge = u.min(v).min(1.0 - u).min(1.0 - v);
        let x = (u * size as f32) as u32;
        let y = (v * size as f32) as u32;
        let grain = hash(x * 263 + y * 71) * 10.0;
        let (r, g, b) = if edge < 0.035 {
            (138.0, 142.0, 150.0)
        } else if edge < 0.05 {
            // The shadow of the parapet on the membrane.
            (24.0, 25.0, 28.0)
        } else if x % 32 < 2 || y % 48 < 2 {
            (34.0, 36.0, 40.0)
        } else {
            (44.0 + grain, 46.0 + grain, 51.0 + grain)
        };
        fframes::Color::rgb(r as u8, g as u8, b as u8)
    }))
}

/// The length of street between two lamps on the same side, in meters.
pub const LAMP_SPACING: f32 = 32.0;

/// The light of the street lamps on the asphalt. U runs along the street, one lamp
/// spacing per repeat. V runs across the street from 0 to 1. The lamps on the two sides
/// are half a spacing apart.
pub fn lamp_pools() -> Arc<Texture> {
    Arc::new(Texture::from_fn(128, 64, |u, v| {
        let pool = |center_u: f32, center_v: f32| {
            let du = (u - center_u).abs().min(1.0 - (u - center_u).abs()) * 3.6;
            let dv = (v - center_v) * 4.0;
            (-(du * du + dv * dv) * 2.5).exp()
        };
        let light = (pool(0.0, 0.16) + pool(0.5, 0.84)).min(1.0);
        let c = (light * 255.0) as u8;
        fframes::Color::rgb(c, c, c)
    }))
}

/// The picture of a video screen before the film owner sets a video: a cold dark blue
/// field with a faint grid.
pub fn screen() -> Arc<Texture> {
    Arc::new(Texture::from_fn(256, 192, |u, v| {
        let grid = if (u * 16.0).fract() < 0.04 || (v * 12.0).fract() < 0.05 {
            0.15
        } else {
            0.0
        };
        let glow = 0.35 + 0.4 * (1.0 - ((u - 0.5).powi(2) + (v - 0.45).powi(2)).sqrt() * 1.6);
        Rgb::from(Rgb::hex(0x6f9cff).0 * (glow + grid)).to_color()
    }))
}

/// A round soft spot of light, for the glow of traffic lights on the street.
pub fn spot() -> Arc<Texture> {
    Arc::new(Texture::from_fn(64, 64, |u, v| {
        let d = ((u - 0.5).powi(2) + (v - 0.5).powi(2)).sqrt() * 2.0;
        let c = ((1.0 - d).max(0.0).powi(2) * 255.0) as u8;
        fframes::Color::rgb(c, c, c)
    }))
}
