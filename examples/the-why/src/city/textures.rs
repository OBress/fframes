//! Procedural textures of the city: the facades with their lit windows, the roofs, the
//! pools of light that the street lamps make on the asphalt, and the placeholder of the
//! video screens.

use std::sync::Arc;

use fframes_3d::math::hash;
use fframes_3d::{Rgb, Texture, Vec3};

/// The facade of a building.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Style {
    /// A glass curtain wall, for the towers after 1960.
    Glass,
    /// Dark glass between light vertical fins, for the towers after 1960.
    Fins,
    /// Concrete with punched windows, for the middle heights.
    Office,
    /// Stone and brick with punched windows, for the buildings before 1945.
    Prewar,
    /// Dark low buildings with few lit windows.
    Low,
}

/// The window bays across one facade texture.
pub const BAYS: f32 = 32.0;
/// The floors down one facade texture.
pub const FLOORS: f32 = 48.0;
/// Texels across one window bay and down one floor.
const CELL: u32 = 32;

/// The size of a facade texture in window bays and floors, and its texels per bay.
#[derive(Debug, Clone, Copy)]
pub struct Layout {
    pub bays: f32,
    pub floors: f32,
    pub cell: u32,
    /// Far away: small lit windows that each light on their own, on a dark facade. Seen
    /// from far, the texture keeps the same mean light at every size, so it does not
    /// shimmer, and no floor is a bright band.
    pub sparse: bool,
}

/// The facades of the buildings near the flight.
pub const FAR: Layout = Layout {
    bays: BAYS,
    floors: FLOORS,
    cell: CELL,
    sparse: false,
};
/// The facades along the flight through Times Square, which the camera passes at 20 m:
/// two times the texels in each direction, on a texture of the same size.
pub const NEAR: Layout = Layout {
    bays: 16.0,
    floors: 24.0,
    cell: 64,
    sparse: false,
};
/// The facades of the buildings that the flight sees only from far away, and of the
/// other boroughs and New Jersey.
pub const DISTANT: Layout = Layout {
    bays: BAYS,
    floors: FLOORS,
    cell: 16,
    sparse: true,
};
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

/// The colors of light in offices, sRGB: cold LED panels (5000 K), neutral white
/// (4000 K), and the green white of old fluorescent tubes.
const OFFICE: [u32; 8] = [
    0xeef3ff, 0xdce8ff, 0xcfe0ff, 0xfff0dc, 0xfde9d2, 0xf0f4ff, 0xe4f0ff, 0xf2f9e4,
];
/// The colors of light in homes and hotels, sRGB: warm lamps (2700 K to 3500 K).
const HOME: [u32; 4] = [0xffb46b, 0xffc485, 0xffd8a8, 0xffbf7a];
/// A television in a dark room.
const TV: u32 = 0x5a86ff;

/// A tone of light for the hash value `h`: a home light with the chance `warm`, else an
/// office light.
fn tone(h: f32, warm: f32) -> Vec3 {
    if h < warm {
        Rgb::hex(HOME[((h / warm) * HOME.len() as f32) as usize % HOME.len()]).0
    } else {
        let k = (h - warm) / (1.0 - warm);
        Rgb::hex(OFFICE[(k * OFFICE.len() as f32) as usize % OFFICE.len()]).0
    }
}

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
    /// The parts of the tenants that are dark and that are open floors lit from end to end.
    /// The rest have rooms.
    floors: [f32; 2],
    /// The chance that a tenant is a home or a hotel with warm light.
    warm: f32,
    /// The color of the spandrel panels between the floors.
    spandrel: u32,
}

fn cell(style: Style) -> Cell {
    match style {
        Style::Glass => Cell {
            glass: [0.05, 0.95, 0.05, 0.78],
            wall: 0x262c34,
            glass_color: 0x4a5a70,
            glass_finish: [0.14, 0.75],
            wall_finish: [0.35, 0.8],
            floors: [0.33, 0.17],
            warm: 0.15,
            spandrel: 0x1a2028,
        },
        Style::Fins => Cell {
            glass: [0.12, 0.88, 0.03, 0.9],
            wall: 0x8c929a,
            glass_color: 0x0a0f16,
            glass_finish: [0.05, 0.0],
            wall_finish: [0.3, 0.9],
            floors: [0.38, 0.14],
            warm: 0.15,
            spandrel: 0x2a3038,
        },
        Style::Office => Cell {
            glass: [0.2, 0.8, 0.16, 0.72],
            wall: 0x6c6f74,
            glass_color: 0x111720,
            glass_finish: [0.04, 0.0],
            wall_finish: [0.85, 0.0],
            floors: [0.3, 0.14],
            warm: 0.25,
            spandrel: 0x3a3d42,
        },
        Style::Prewar => Cell {
            glass: [0.26, 0.74, 0.18, 0.7],
            wall: 0x6e665e,
            glass_color: 0x10151c,
            glass_finish: [0.05, 0.0],
            wall_finish: [0.9, 0.0],
            floors: [0.35, 0.08],
            warm: 0.55,
            spandrel: 0x4a4038,
        },
        Style::Low => Cell {
            glass: [0.24, 0.76, 0.2, 0.68],
            wall: 0x4b4a4c,
            glass_color: 0x10151c,
            glass_finish: [0.05, 0.0],
            wall_finish: [0.9, 0.0],
            floors: [0.5, 0.05],
            warm: 0.6,
            spandrel: 0x353436,
        },
    }
}

fn srgb(c: u32) -> [u8; 4] {
    [(c >> 16) as u8, (c >> 8) as u8, c as u8, 255]
}

impl Facade {
    pub fn new(style: Style, seed: u32, layout: Layout) -> Self {
        let (w, h) = (
            layout.bays as u32 * layout.cell,
            layout.floors as u32 * layout.cell,
        );
        let mut c = cell(style);
        if layout.sparse {
            // Far away the facades are dark, so the lit windows carry the picture.
            c.wall = 0x16191e;
            c.spandrel = 0x101318;
            c.glass_color = 0x0a0d12;
        }
        let [x0, x1, y0, y1] = c.glass;
        // The cell, the position in the cell, and whether it is glass.
        let at = |u: f32, v: f32| {
            let (x, y) = (u * layout.bays, v * layout.floors);
            let (fx, fy) = (x.fract(), y.fract());
            let glass = fx > x0 && fx < x1 && fy > y0 && fy < y1;
            (x as u32, y as u32, fx, fy, glass)
        };
        // The spandrel panel: the band of the wall at the slab between two floors.
        let spandrel = |fy: f32| fy > y1 + 0.03 || fy < y0 - 0.03;
        let color = Texture::from_fn_data(w, h, true, |u, v| {
            let (cx, cy, _, fy, glass) = at(u, v);
            if glass {
                return srgb(c.glass_color);
            }
            if spandrel(fy) {
                // The panels differ a little from bay to bay.
                let k = 0.85 + 0.3 * hash(cx * 17 + cy * 131 + seed);
                let p = (Rgb::hex(c.spandrel).0 * k).min(Vec3::ONE);
                let out = Rgb::from(p).to_color();
                return [out.r, out.g, out.b, 255];
            }
            srgb(c.wall)
        });
        let metal_rough = Texture::from_fn_data(w, h, false, |u, v| {
            let (_, _, _, fy, glass) = at(u, v);
            let [rough, metal] = if glass {
                c.glass_finish
            } else if spandrel(fy) {
                [0.3, 0.6]
            } else {
                c.wall_finish
            };
            [255, (rough * 255.0) as u8, (metal * 255.0) as u8, 255]
        });
        let emissive = Texture::from_fn_data(w, h, true, |u, v| {
            let (cx, cy, fx, fy, glass) = at(u, v);
            if !glass {
                return [0, 0, 0, 255];
            }
            let (px, py) = ((fx - x0) / (x1 - x0), (fy - y0) / (y1 - y0));
            let light = if layout.sparse {
                window_light(seed, &c, cx, cy, px, py)
            } else {
                room_light(seed, &c, cx, cy, px, py)
            };
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
/// `cy`. Tenants take runs of two to six floors with one kind of light. Their floors are
/// dark, lit from end to end, or divided into rooms that are lit or dark. Homes and
/// hotels have warm light, curtains and televisions, offices cold or green white light,
/// ceiling panels and blinds.
fn room_light(seed: u32, c: &Cell, cx: u32, cy: u32, px: f32, py: f32) -> Vec3 {
    // The tenant of the floor: runs of floors that start where the hash is low.
    let mut first = cy;
    while first > 0 && hash(seed.wrapping_mul(613).wrapping_add(first * 29)) >= 0.3 {
        first -= 1;
    }
    let tenant = seed.wrapping_mul(977).wrapping_add(first * 131);
    let floor = tenant.wrapping_add(cy * 7);
    let window = floor.wrapping_add(cx * 31);
    let home = hash(tenant + 4) < c.warm;
    let light = tone(hash(tenant + 2), if home { 1.0 } else { 0.0 });
    let class = hash(tenant);
    let dark = Rgb::hex(0x1a2a40).0 * 0.012;
    if class < c.floors[0] {
        // A dark tenant. A few rooms keep a desk lamp, a monitor or a television on.
        return match hash(window + 21) {
            h if h < 0.03 => monitor(window, px, py),
            h if h < 0.05 && home => television(window, px, py),
            _ => dark,
        };
    }
    if !home && class > 1.0 - c.floors[1] {
        // An open office floor, lit from end to end with the same ceiling panels.
        let level = 0.16 + 0.2 * hash(tenant + 3) + 0.05 * hash(window + 8);
        return light * level * ceiling(window, px, py);
    }
    // A floor of rooms: a room starts at a bay where the hash is low and runs to the right.
    let mut start = cx;
    while start > 0 && hash(floor + start * 7) >= if home { 0.4 } else { 0.2 } {
        start -= 1;
    }
    let room = floor.wrapping_add(start * 7919);
    let lit = 0.2 + 0.5 * hash(tenant + 5);
    if hash(room + 1) > lit {
        return match hash(window + 21) {
            h if h < 0.06 => monitor(window, px, py),
            h if h < 0.12 && home => television(window, px, py),
            _ => dark,
        };
    }
    // Some rooms of a tenant have another light.
    let light = if hash(room + 9) < 0.2 {
        tone(hash(room + 2), c.warm)
    } else {
        light
    };
    let level = 0.16 + 0.6 * hash(room + 3).powf(1.4) * (0.8 + 0.4 * hash(window + 8));
    if home {
        // Curtains or a lamp in the room: an even, warm light, darker at the sides.
        let fold = ((px * 9.0 + hash(window + 10) * 3.0).sin() * 0.5 + 0.5) * 0.25;
        let lamp = 1.0 - 0.6 * (px - 0.2 - 0.6 * hash(room + 12)).abs();
        // Curtains drawn to the sides of the window.
        let open = 0.25 + 0.2 * hash(window + 17);
        if (px - 0.5).abs() > open && hash(window + 18) < 0.6 {
            return light * level * (0.35 + fold);
        }
        if hash(window + 6) < 0.35 {
            return light * level * (0.45 + fold);
        }
        // A sofa or a shelf in front of the lamp.
        let shade = if py > 0.7 && hash(window + 19 + (px * 3.0) as u32) < 0.5 {
            0.3
        } else {
            1.0
        };
        return light * level * lamp * (1.15 - 0.5 * py) * shade;
    }
    let mut l = level * ceiling(window, px, py);
    // Blinds, part way down, with their slats.
    let blind = 0.1 + 0.8 * hash(window + 7);
    if hash(window + 6) < 0.35 && py < blind {
        let slat = ((py * 40.0).fract() - 0.5).abs() * 2.0;
        l = level * (0.3 + 0.18 * slat) * (0.7 + 0.3 * py / blind);
    }
    // A column or a cabinet in front of part of the window.
    if hash(window + 13) < 0.15 && (px - hash(window + 14)).abs() < 0.12 {
        l *= 0.25;
    }
    // A screen on a desk, cold and brighter than the room.
    let desk = 0.15 + 0.6 * hash(window + 15);
    if hash(window + 16) < 0.45 && py > 0.56 && py < 0.66 && (px - desk).abs() < 0.07 {
        return Rgb::hex(0xbcd4ff).0 * (level * 1.6 + 0.08);
    }
    light * l
}

/// The light of a far window at `px`, `py`: each window lights on its own, a little more
/// often on some floors. The light fills the middle of the glass, so the texture stays the
/// same at every mip level.
fn window_light(seed: u32, c: &Cell, cx: u32, cy: u32, px: f32, py: f32) -> Vec3 {
    let window = seed
        .wrapping_mul(977)
        .wrapping_add(cy * 131)
        .wrapping_add(cx * 31);
    let floor = 0.6 + 0.8 * hash(seed.wrapping_add(cy * 53));
    if hash(window + 1) > 0.26 * floor || (px - 0.5).abs() > 0.38 || (py - 0.5).abs() > 0.4 {
        return Vec3::ZERO;
    }
    let level = 0.15 + 0.45 * hash(window + 3).powf(1.5);
    tone(hash(window + 2), c.warm) * level
}

/// A dark room with a television: a cold blue glow that fills the room.
fn television(window: u32, px: f32, py: f32) -> Vec3 {
    let x = 0.2 + 0.6 * hash(window + 23);
    let glow = (1.0 - (px - x).abs() * 1.5).max(0.2) * (0.6 + 0.4 * py);
    Rgb::hex(TV).0 * (0.12 * glow)
}

/// The light of a room from its ceiling: brightest at the top, where the lights hang, and
/// darker behind the desks and the partitions at the bottom.
fn ceiling(window: u32, px: f32, py: f32) -> f32 {
    // The ceiling: two light panels in each bay, and the dark ceiling between them. Deeper
    // in the room the ceiling is darker.
    if py < 0.1 {
        let panel = ((px * 2.0).fract() - 0.5).abs() < 0.3;
        return if panel { 2.8 } else { 0.55 };
    }
    if py < 0.16 {
        return 0.7;
    }
    // The back of the room, lit from above.
    let mut l = 1.0 - 0.6 * (py - 0.16);
    // Desks and their chairs at the bottom, and partitions.
    if py > 0.64 {
        let block = hash(window + 11 + (px * 4.0) as u32);
        l *= if block < 0.35 {
            0.12
        } else if block < 0.7 {
            0.35
        } else {
            0.7
        };
    }
    // A mullion between the two panes of the window.
    if (px - 0.5).abs() < 0.015 {
        l *= 0.2;
    }
    l
}

/// A dark room with one monitor left on: a small cold rectangle low in the window.
fn monitor(window: u32, px: f32, py: f32) -> Vec3 {
    let x = 0.2 + 0.5 * hash(window + 22);
    let on = px > x && px < x + 0.18 && py > 0.6 && py < 0.78;
    let glow = (1.0 - ((px - x - 0.09).abs() * 3.0).min(1.0)) * (py - 0.4).max(0.0) * 0.12;
    Rgb::hex(0x7fb4ff).0 * if on { 0.55 } else { glow }
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

/// The shop fronts and lobbies at street level, one per repeat of U: a lit sign band at
/// the top, glass with a bright interior, dark frames, and a door in some of them.
pub fn storefront() -> Arc<Texture> {
    Arc::new(Texture::from_fn(256, 64, |u, v| {
        // Four shops across the texture, each with its own light.
        let shop = (u * 4.0) as u32;
        let x = (u * 4.0).fract();
        let level = 0.35 + 0.65 * hash(shop + 3);
        let light = if x < 0.04 || x > 0.96 || v > 0.94 {
            0.0
        } else if v < 0.16 {
            // The sign band.
            if hash(shop + 5) < 0.6 { 1.0 } else { 0.15 }
        } else if v < 0.2 {
            0.0
        } else {
            // The glass with mullions, brighter at the top where the ceiling lights are,
            // and a door in the middle of some shops.
            let mullion = if (x * 3.0).fract() < 0.03 { 1.0 } else { 0.0 };
            let door = hash(shop + 7) < 0.5 && (x - 0.5).abs() < 0.08 && v > 0.45;
            let inside = (1.0 - 0.5 * v) * (1.0 - 0.8 * mullion);
            if door { 0.35 } else { inside * level }
        };
        let c = (light * 255.0) as u8;
        fframes::Color::rgb(c, c, c)
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
