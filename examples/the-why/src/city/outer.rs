//! The world around Midtown: the buildings of the other boroughs and of New Jersey, the
//! carpet of street lights that covers the far city, the bridges with the lights on their
//! cables, and the lamps of the park paths.

use std::sync::Arc;

use fframes_3d::math::hash;
use fframes_3d::{Rgb, Texture, Vec2, Vec3, vec2, vec3};

use super::build::{JERSEY, MeshBuf, walls};
use super::data::{Block, LineKind, Manhattan};
use super::textures::Style;

/// The half size of the square that the light carpet covers, in meters, around the
/// Empire State Building.
pub const CARPET_HALF: f32 = 16000.0;
/// The texels across the light carpet: 10.4 m each.
const CARPET_SIZE: usize = 3072;
/// The box of Midtown with the real buildings, streets and lamps. The carpet stays dark
/// there. The same box as `NEAR` in `nyc.py`.
const NEAR_BOX: (Vec2, Vec2) = (Vec2::new(-1500.0, -1300.0), Vec2::new(2200.0, 3000.0));
/// The distance between two street lights, in meters.
const LAMP_STEP: f32 = 30.0;

/// The distance from `p` to the line of the flight over Midtown, roughly.
fn from_flight(p: Vec2) -> f32 {
    let (a, b) = (vec2(100.0, -330.0), vec2(650.0, 1510.0));
    let d = b - a;
    let k = ((p - a).dot(d) / d.length_squared()).clamp(0.0, 1.0);
    p.distance(a + d * k)
}

/// Whether a building outside Midtown is a box in the picture. The others are only light
/// in the carpet.
fn boxed(b: &Block) -> bool {
    let d = from_flight(b.center);
    b.height >= 30.0 || (b.height >= 15.0 && d < 5000.0) || (b.height >= 9.0 && d < 2500.0)
}

/// The style of the facade of a building outside Midtown.
fn block_style(b: &Block, seed: u32) -> Style {
    if b.height >= 60.0 {
        if hash(seed) < 0.5 {
            Style::Glass
        } else {
            Style::Fins
        }
    } else if b.height >= 25.0 {
        Style::Office
    } else if hash(seed) < 0.6 {
        Style::Prewar
    } else {
        Style::Low
    }
}

/// The boxes of the buildings outside Midtown: the facades of each style, in the order of
/// `build::STYLES`, and the roofs.
pub fn blocks(city: &Manhattan, styles: &[Style; 5]) -> ([MeshBuf; 5], MeshBuf) {
    let mut out: [MeshBuf; 5] = Default::default();
    let mut roofs = MeshBuf::default();
    for (i, b) in city.blocks.iter().enumerate().filter(|(_, b)| boxed(b)) {
        let seed = i as u32 * 7 + 3;
        let (c, s) = (b.angle.cos(), b.angle.sin());
        let (u, v) = (vec2(c, s) * b.half.x, vec2(-s, c) * b.half.y);
        // The corners go counterclockwise seen from above (+Y), so the walls face out.
        let mut corners = [
            b.center - u - v,
            b.center + u - v,
            b.center + u + v,
            b.center - u + v,
        ];
        let area = (corners[1] - corners[0]).perp_dot(corners[2] - corners[0]);
        if area > 0.0 {
            corners.reverse();
        }
        let h = b.height;
        let mut tris = Vec::with_capacity(8);
        for k in 0..4 {
            let (p, q) = (corners[k], corners[(k + 1) % 4]);
            let [p0, q0, q1, p1] = [
                vec3(p.x, 0.0, p.y),
                vec3(q.x, 0.0, q.y),
                vec3(q.x, h, q.y),
                vec3(p.x, h, p.y),
            ];
            tris.extend([[p0, q0, q1], [p0, q1, p1]]);
        }
        let style = block_style(b, seed);
        let k = styles.iter().position(|s| *s == style).unwrap_or(0);
        walls(&mut out[k], &tris, seed, Rgb::WHITE);
        let top = corners.map(|p| vec3(p.x, h, p.y));
        roofs.triangle([top[0], top[2], top[1]], [Vec2::ZERO; 3]);
        roofs.triangle([top[0], top[3], top[2]], [Vec2::ZERO; 3]);
    }
    (out, roofs)
}

/// A map of the land of the five boroughs and of New Jersey on a grid of 40 m cells.
pub struct LandMap {
    cells: Vec<bool>,
}

const LAND_CELL: f32 = 40.0;
const LAND_SIZE: usize = (2.0 * CARPET_HALF / LAND_CELL) as usize;

impl LandMap {
    pub fn new(city: &Manhattan) -> Self {
        let mut cells = vec![false; LAND_SIZE * LAND_SIZE];
        for t in &city.land {
            let (min, max) = t.iter().fold((Vec2::MAX, Vec2::MIN), |(lo, hi), &p| {
                (lo.min(p), hi.max(p))
            });
            let cell = |p: Vec2| ((p + CARPET_HALF) / LAND_CELL).floor();
            let (a, b) = (cell(min).max(Vec2::ZERO), cell(max));
            for j in a.y as usize..=(b.y as usize).min(LAND_SIZE - 1) {
                for i in a.x as usize..=(b.x as usize).min(LAND_SIZE - 1) {
                    let q = (vec2(i as f32, j as f32) + 0.5) * LAND_CELL - CARPET_HALF;
                    let s = [0, 1, 2].map(|k| (t[(k + 1) % 3] - t[k]).perp_dot(q - t[k]));
                    if s.iter().all(|&v| v >= 0.0) || s.iter().all(|&v| v <= 0.0) {
                        cells[j * LAND_SIZE + i] = true;
                    }
                }
            }
        }
        Self { cells }
    }

    /// Whether `p` is on land: in a borough, or in New Jersey west of the Hudson.
    pub fn land(&self, p: Vec2) -> bool {
        if p.x > jersey_shore(p.y) {
            return true;
        }
        let q = ((p + CARPET_HALF) / LAND_CELL).floor();
        if q.x < 0.0 || q.y < 0.0 || q.x >= LAND_SIZE as f32 || q.y >= LAND_SIZE as f32 {
            return false;
        }
        self.cells[q.y as usize * LAND_SIZE + q.x as usize]
    }
}

/// The x of the New Jersey shore at `z`.
pub fn jersey_shore(z: f32) -> f32 {
    let i = JERSEY
        .partition_point(|p| p.y < z)
        .clamp(1, JERSEY.len() - 1);
    let (a, b) = (JERSEY[i - 1], JERSEY[i]);
    let k = ((z - a.y) / (b.y - a.y)).clamp(0.0, 1.0);
    a.x + (b.x - a.x) * k
}

/// The light carpet: the street lights and the windows of the far city seen from above, as
/// an emissive texture over the square of [`CARPET_HALF`]. U runs along +X, V along +Z.
/// Red, green and blue hold the light; the material color sets the brightness.
pub fn carpet(city: &Manhattan, land: &LandMap) -> Arc<Texture> {
    let n = CARPET_SIZE;
    let texel = 2.0 * CARPET_HALF / n as f32;
    // Two layers of light: the street lights and the windows.
    let mut lamps = vec![0.0f32; n * n];
    let mut windows = vec![0.0f32; n * n];
    let in_near = |p: Vec2| p.cmpge(NEAR_BOX.0).all() && p.cmple(NEAR_BOX.1).all();
    // Adds `light` at `p`, shared by the four nearest texels.
    let splat = |layer: &mut Vec<f32>, p: Vec2, light: f32| {
        if in_near(p) {
            return;
        }
        let q = (p + CARPET_HALF) / texel - 0.5;
        let (i, j) = (q.x.floor(), q.y.floor());
        let (fx, fy) = (q.x - i, q.y - j);
        for (di, dj, w) in [
            (0, 0, (1.0 - fx) * (1.0 - fy)),
            (1, 0, fx * (1.0 - fy)),
            (0, 1, (1.0 - fx) * fy),
            (1, 1, fx * fy),
        ] {
            let (x, y) = (i as i64 + di, j as i64 + dj);
            if x >= 0 && y >= 0 && (x as usize) < n && (y as usize) < n {
                layer[y as usize * n + x as usize] += light * w;
            }
        }
    };
    for (kind, points) in &city.lines {
        let (light, step, side) = match kind {
            LineKind::Street => (1.0, LAMP_STEP, 6.0),
            LineKind::Highway | LineKind::Ramp => (1.6, LAMP_STEP * 1.2, 9.0),
            LineKind::Bridge => continue,
            LineKind::Path => (0.45, LAMP_STEP, 0.0),
        };
        let mut rest = 0.0;
        let mut flip = false;
        for w in points.windows(2) {
            let d = w[1] - w[0];
            let l = d.length();
            if l < 1e-3 {
                continue;
            }
            let across = vec2(-d.y, d.x) / l * side;
            let mut s = rest;
            while s < l {
                let p = w[0] + d * (s / l);
                flip = !flip;
                splat(
                    &mut lamps,
                    if flip { p + across } else { p - across },
                    light,
                );
                s += step;
            }
            rest = s - l;
        }
    }
    // The windows of the buildings: more light from bigger and taller buildings.
    for (i, b) in city.blocks.iter().enumerate() {
        let floors = (b.height / 3.3).max(1.0);
        let area = 4.0 * b.half.x * b.half.y;
        let lit = 0.15 + 0.3 * hash(i as u32 * 13 + 1);
        splat(&mut windows, b.center, (area * floors).sqrt() * 0.05 * lit);
    }
    // New Jersey has no streets in the data: a grid of street lights on its land.
    for k in 0..60000u32 {
        let z = -7000.0 + 21000.0 * hash(k * 5 + 1);
        let shore = jersey_shore(z);
        let x = shore + 60.0 + 7000.0 * hash(k * 5 + 2).powf(1.4);
        // Snap to streets 80 m apart that run at the angle of the shore.
        let p = vec2((x / 80.0).round() * 80.0 + 20.0 * hash(k * 5 + 3), z);
        if land.land(p) {
            splat(&mut lamps, p, 1.0);
        }
    }
    // From the flight the lamps of a street are one line and a block of windows one glow:
    // a soft blur over 3 texels (30 m) joins the dots. The windows get it twice.
    blur(&mut lamps, n);
    blur(&mut windows, n);
    blur(&mut windows, n);
    // The blur spreads a lamp over about 7 texels: the lines keep their brightness.
    for v in &mut lamps {
        *v *= 3.0;
    }
    for v in &mut windows {
        *v *= 2.0;
    }
    let lamp = Rgb::hex(0xffd7a0).0;
    let window = Rgb::hex(0xdfe4f0).0;
    // Fog adds its color to an additive surface, so the carpet has no fog. It fades with
    // the distance from Midtown instead: the haze hides the far city.
    let rgba: Vec<u8> = (0..n * n)
        .flat_map(|k| {
            let p = (vec2((k % n) as f32, (k / n) as f32) + 0.5) * texel - CARPET_HALF;
            let fade = (-(p.length() / 9000.0).powi(2)).exp();
            let light = (lamp * lamps[k] + window * windows[k]) * fade;
            let c = Rgb::from(light.min(Vec3::ONE)).to_color();
            [c.r, c.g, c.b, 255]
        })
        .collect();
    Arc::new(Texture::from_rgba8(n as u32, n as u32, &rgba, true))
}

/// Blurs the square `layer` of `n` texels on a side with the binomial kernel 1 4 6 4 1,
/// along the rows and then along the columns. The sum of the light stays the same.
fn blur(layer: &mut [f32], n: usize) {
    const KERNEL: [f32; 5] = [1.0 / 16.0, 4.0 / 16.0, 6.0 / 16.0, 4.0 / 16.0, 1.0 / 16.0];
    let source = layer.to_vec();
    for y in 0..n {
        for x in 0..n {
            layer[y * n + x] = KERNEL
                .iter()
                .enumerate()
                .map(|(k, w)| {
                    let sx = (x + k).wrapping_sub(2);
                    if sx < n { w * source[y * n + sx] } else { 0.0 }
                })
                .sum();
        }
    }
    let source = layer.to_vec();
    for y in 0..n {
        for x in 0..n {
            layer[y * n + x] = KERNEL
                .iter()
                .enumerate()
                .map(|(k, w)| {
                    let sy = (y + k).wrapping_sub(2);
                    if sy < n { w * source[sy * n + x] } else { 0.0 }
                })
                .sum();
        }
    }
}

/// The ground under the light carpet: the land of the boroughs and of New Jersey, with UVs
/// on the carpet.
pub fn carpet_ground(city: &Manhattan, y: f32) -> MeshBuf {
    let uv = |p: Vec2| (p + CARPET_HALF) / (2.0 * CARPET_HALF);
    let mut buf = MeshBuf::default();
    for t in &city.land {
        let mut p = t.map(|q| vec3(q.x, y, q.y));
        let mut u = t.map(uv);
        if (p[1] - p[0]).cross(p[2] - p[0]).y < 0.0 {
            p.swap(1, 2);
            u.swap(1, 2);
        }
        buf.triangle(p, u);
    }
    // New Jersey, from the shore to the edge of the carpet.
    for w in JERSEY.windows(2) {
        let (a, b) = (w[0], w[1]);
        let far = CARPET_HALF;
        let q = [a, vec2(far, a.y), vec2(far, b.y), b];
        let p = q.map(|q| vec3(q.x, y, q.y));
        buf.quad(
            [p[0], p[3], p[2], p[1]],
            [uv(q[0]), uv(q[3]), uv(q[2]), uv(q[1])],
        );
    }
    buf
}

/// The lamps along the paths of the parks within `radius` meters of the flight.
pub fn park_lamps(city: &Manhattan, radius: f32) -> Vec<Vec3> {
    let mut out = Vec::new();
    for (kind, points) in &city.lines {
        if *kind != LineKind::Path || points.iter().all(|&p| from_flight(p) > radius) {
            continue;
        }
        let mut rest = 0.0;
        for w in points.windows(2) {
            let d = w[1] - w[0];
            let l = d.length();
            let mut s = rest;
            while s < l {
                let p = w[0] + d * (s / l.max(1e-3));
                out.push(vec3(p.x, 3.6, p.y));
                s += 28.0;
            }
            rest = s - l;
        }
    }
    out
}

/// A bridge over the water: its line, the part over the water and the height of its deck.
struct Span {
    points: Vec<Vec2>,
    /// The distances along the line where the water starts and ends.
    water: (f32, f32),
    deck: f32,
}

impl Span {
    fn length(&self) -> f32 {
        self.points.windows(2).map(|w| w[0].distance(w[1])).sum()
    }

    fn at(&self, s: f32) -> (Vec2, Vec2) {
        let mut rest = s.max(0.0);
        for w in self.points.windows(2) {
            let l = w[0].distance(w[1]);
            if rest <= l {
                let d = (w[1] - w[0]) / l.max(1e-3);
                return (w[0] + d * rest, d);
            }
            rest -= l;
        }
        let n = self.points.len();
        let d = (self.points[n - 1] - self.points[n - 2]).normalize_or(Vec2::Y);
        (self.points[n - 1], d)
    }

    /// The height of the deck at `s`: up over the water, down to the street on ramps of
    /// 350 m.
    fn height(&self, s: f32) -> f32 {
        let out = (self.water.0 - s).max(s - self.water.1).max(0.0);
        self.deck * (1.0 - out / 350.0).max(0.0)
    }
}

/// The bridges over the rivers: the decks and the towers, and the lights on the decks and
/// along the main cables of the suspension bridges.
pub fn bridges(city: &Manhattan, land: &LandMap) -> (MeshBuf, Vec<Vec3>) {
    let mut spans: Vec<Span> = Vec::new();
    for points in &city.bridges {
        let total: f32 = points.windows(2).map(|w| w[0].distance(w[1])).sum();
        if total < 300.0 {
            continue;
        }
        let span = Span {
            points: points.clone(),
            water: (0.0, 0.0),
            deck: 0.0,
        };
        // The longest part of the line over water, in steps of 10 m.
        let (mut best, mut start) = ((0.0f32, 0.0f32), None);
        let mut s = 0.0;
        while s <= total {
            let wet = !land.land(span.at(s).0);
            match (wet, start) {
                (true, None) => start = Some(s),
                (false, Some(a)) => {
                    if s - a > best.1 - best.0 {
                        best = (a, s);
                    }
                    start = None;
                }
                _ => {}
            }
            s += 10.0;
        }
        if let Some(a) = start
            && total - a > best.1 - best.0
        {
            best = (a, total);
        }
        let width = best.1 - best.0;
        if width < 200.0 {
            continue;
        }
        let middle = span.at(f32::midpoint(best.0, best.1)).0;
        // The roadways of one bridge are several lines: keep one.
        if spans
            .iter()
            .any(|o| o.at(f32::midpoint(o.water.0, o.water.1)).0.distance(middle) < 250.0)
        {
            continue;
        }
        let deck = (0.05 * width + 22.0).clamp(30.0, 65.0);
        spans.push(Span {
            water: best,
            deck,
            ..span
        });
    }
    let mut mesh = MeshBuf::default();
    let mut lights = Vec::new();
    for b in &spans {
        let total = b.length();
        let half = 12.0;
        // The deck: a band of steel along the line, and two rows of lights.
        let mut s = 0.0;
        while s < total {
            let (p, d) = b.at(s);
            let (q, _) = b.at((s + 12.0).min(total));
            let (h0, h1) = (b.height(s), b.height(s + 12.0));
            if h0.max(h1) > 2.0 {
                let side = vec2(-d.y, d.x) * half;
                let corners = [p - side, p + side, q + side, q - side];
                let hs = [h0, h0, h1, h1];
                let top: [Vec3; 4] =
                    std::array::from_fn(|k| vec3(corners[k].x, hs[k], corners[k].y));
                let low = top.map(|v| v - Vec3::Y * 3.0);
                mesh.quad([top[0], top[3], top[2], top[1]], [Vec2::ZERO; 4]);
                mesh.quad([low[0], low[1], low[2], low[3]], [Vec2::ZERO; 4]);
                for k in [0usize, 1] {
                    let e = corners[k] + (corners[k] - p).normalize_or_zero() * 0.5;
                    lights.push(vec3(e.x, h0 + 1.2, e.y));
                }
            }
            s += 12.0;
        }
        let main = b.water.1 - b.water.0;
        if main < 400.0 {
            continue;
        }
        // A suspension bridge: two towers near the shores and the main cables between
        // them, with a string of lights along each cable.
        let towers = [b.water.0 + 0.07 * main, b.water.1 - 0.07 * main];
        let top = b.deck + (0.12 * main).clamp(45.0, 125.0);
        let mut ends = [Vec3::ZERO; 4];
        for (t, &s) in towers.iter().enumerate() {
            let (p, d) = b.at(s);
            let side = vec2(-d.y, d.x) * (half + 1.5);
            for (k, leg) in [p - side, p + side].into_iter().enumerate() {
                mesh.block(
                    vec3(leg.x - 3.0, 0.0, leg.y - 3.0),
                    vec3(leg.x + 3.0, top, leg.y + 3.0),
                );
                ends[t * 2 + k] = vec3(leg.x, top, leg.y);
                lights.push(vec3(leg.x, top + 1.5, leg.y));
            }
            for y in [b.deck + 5.0, top - 4.0] {
                mesh.bar(
                    vec3((p - side).x, y, (p - side).y),
                    vec3((p + side).x, y, (p + side).y),
                    3.0,
                );
            }
        }
        for k in 0..2 {
            let (a, c) = (ends[k], ends[2 + k]);
            let n = (main / 12.0) as usize;
            for i in 0..=n {
                let u = i as f32 / n as f32;
                let sag = (2.0 * u - 1.0).powi(2);
                let mut p = a.lerp(c, u);
                p.y = b.deck + 4.0 + (top - b.deck - 4.0) * sag;
                lights.push(p);
                if i > 0 {
                    let mut q = a.lerp(c, (i - 1) as f32 / n as f32);
                    q.y = b.deck
                        + 4.0
                        + (top - b.deck - 4.0) * (2.0 * u - 2.0 / n as f32 - 1.0).powi(2);
                    mesh.bar(q, p, 0.7);
                }
            }
        }
    }
    (mesh, lights)
}

/// The radius of the glow of the sky, in meters: behind the edge of the water, inside the
/// far plane of the camera.
pub const SKY_RADIUS: f32 = 240_000.0;

/// The brightness of the glow of the city on the sky at the elevation `e` (radians): the
/// light of the city in the haze, strongest at the horizon, gone above 30 degrees.
pub fn sky_glow(e: f32) -> f32 {
    if e <= 0.0 {
        1.0
    } else {
        (-e / 0.075).exp() * (1.0 - (e / 0.52).min(1.0)).powi(2)
    }
}

/// A dome around the origin with the glow of the city in its vertex colors. Move it with
/// the camera. It adds light to the sky; above 30 degrees it is black, so the stars at the
/// end of the shot stay as they are.
pub fn sky_dome(color: Rgb) -> MeshBuf {
    let mut buf = MeshBuf::default();
    let (rings, segments) = (24, 64);
    // Elevations from 1 degree below the horizon to 32 degrees above it, denser near the
    // horizon. The water, closer, covers the part below the horizon; the dome only closes
    // the gap where the water ends.
    let elevation = |i: usize| {
        if i == 0 {
            return (-1.0f32).to_radians();
        }
        let k = (i - 1) as f32 / (rings - 1) as f32;
        (32.0 * k * k).to_radians()
    };
    for i in 0..rings {
        let (e0, e1) = (elevation(i), elevation(i + 1));
        for j in 0..segments {
            let a0 = j as f32 / segments as f32 * std::f32::consts::TAU;
            let a1 = (j + 1) as f32 / segments as f32 * std::f32::consts::TAU;
            let p =
                |e: f32, a: f32| vec3(e.cos() * a.cos(), e.sin(), e.cos() * a.sin()) * SKY_RADIUS;
            let first = buf.len();
            buf.quad(
                [p(e0, a0), p(e0, a1), p(e1, a1), p(e1, a0)],
                [Vec2::ZERO; 4],
            );
            for (k, e) in [e0, e0, e1, e1].into_iter().enumerate() {
                buf.paint_vertex(first + k, color * sky_glow(e));
            }
        }
    }
    buf
}
