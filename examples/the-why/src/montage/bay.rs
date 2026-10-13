//! The place of the Golden Gate: the strait, the Marin Headlands, the Presidio and the city
//! of San Francisco, from the USGS 3DEP elevation and the building footprints of `DataSF`. Run
//! `./fetch-sf.sh` once to get the data. Without it, the strait is open water.
//!
//! World space: X to the east, Y up, Z to the south, in meters. The origin is the middle of
//! the main span of the bridge at sea level.

use std::f32::consts::TAU;
use std::sync::Arc;

use fframes_3d::math::{hash, smoothstep};
use fframes_3d::*;

use super::{fbm, mix};

/// The folder that `fetch-sf.sh` fills.
const DATA: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/dynamic_media/sf");

/// The direction of the bridge toward Marin (north), from OpenStreetMap.
pub const NORTHWARD: Vec3 = Vec3::new(-0.0925, 0.0, -0.9957);

/// The turn from the space of the bridge (X toward Marin, Z toward the bay) to the world.
pub fn bridge_turn() -> Quat {
    Quat::from_rotation_arc(Vec3::X, NORTHWARD)
}

/// A point in the space of the bridge, in the world.
pub fn from_bridge(p: Vec3) -> Vec3 {
    bridge_turn() * p
}

/// A world position of the latitude and the longitude `lat`, `lon` at the height `y`.
pub fn geo(lat: f32, lon: f32, y: f32) -> Vec3 {
    const LAT0: f32 = 37.819_72;
    const LON0: f32 = -122.478_61;
    let east = 111_320.0 * LAT0.to_radians().cos();
    vec3((lon - LON0) * east, y, -(lat - LAT0) * 110_540.0)
}

/// The land: heights on a regular grid. The sea is below 0.
pub struct Land {
    width: usize,
    height: usize,
    /// The world position of the first sample (north-west) and the step to the next.
    origin: Vec2,
    step: Vec2,
    heights: Vec<f32>,
}

impl Land {
    /// The height at `x`, `z`, interpolated. Outside the grid it is the sea floor.
    pub fn at(&self, x: f32, z: f32) -> f32 {
        let fx = (x - self.origin.x) / self.step.x;
        let fz = (z - self.origin.y) / self.step.y;
        if fx < 0.0 || fz < 0.0 || fx >= (self.width - 1) as f32 || fz >= (self.height - 1) as f32 {
            return -8.0;
        }
        let (ix, iz) = (fx as usize, fz as usize);
        let (u, v) = (fx.fract(), fz.fract());
        let h = |x: usize, z: usize| self.heights[z * self.width + x];
        mix(
            mix(h(ix, iz), h(ix + 1, iz), u),
            mix(h(ix, iz + 1), h(ix + 1, iz + 1), u),
            v,
        )
    }

    /// The world rectangle of the grid: the north-west corner and the size.
    fn extent(&self) -> (Vec2, Vec2) {
        let size = self.step * vec2(self.width as f32, self.height as f32);
        (self.origin, size)
    }
}

/// The San Francisco data: the land, the tall buildings and the centers of all buildings.
pub struct Bay {
    pub land: Land,
    /// Each building taller than 12 m: the center X and Z, the half width and the half
    /// depth, the turn around Y, the ground height, the height, and the scale of the roof
    /// (1 for a box, 0 for a pyramid).
    towers: Vec<[f32; 8]>,
    /// Each building: the center X and Z, the ground height and the height.
    lights: Vec<[f32; 4]>,
}

/// The values of a data file after its four byte `magic`, a count and the records of
/// `N` floats. `None` when the file is missing or wrong.
fn records<const N: usize>(file: &str, magic: &[u8; 4]) -> Option<Vec<[f32; N]>> {
    let bytes = std::fs::read(format!("{DATA}/{file}")).ok()?;
    if bytes.get(..4)? != magic {
        return None;
    }
    let count = u32::from_le_bytes(bytes.get(4..8)?.try_into().ok()?) as usize;
    let floats: Vec<f32> = bytes
        .get(8..8 + count * N * 4)?
        .as_chunks::<4>()
        .0
        .iter()
        .map(|b| f32::from_le_bytes(*b))
        .collect();
    Some(floats.as_chunks::<N>().0.to_vec())
}

impl Bay {
    /// Reads the data of `fetch-sf.sh`. Without it, the land is flat sea floor and the city
    /// is empty.
    pub fn load() -> Self {
        let land = Self::land().unwrap_or_else(|| {
            eprintln!("{DATA} is missing: run examples/the-why/fetch-sf.sh");
            Land {
                width: 2,
                height: 2,
                origin: Vec2::splat(-1.0),
                step: Vec2::ONE,
                heights: vec![-8.0; 4],
            }
        });
        Self {
            land,
            towers: records::<8>("towers.bin", b"SFB1").unwrap_or_default(),
            lights: records::<4>("lights.bin", b"SFL1").unwrap_or_default(),
        }
    }

    fn land() -> Option<Land> {
        let bytes = std::fs::read(format!("{DATA}/terrain.bin")).ok()?;
        if bytes.get(..4)? != b"SFT1" {
            return None;
        }
        let word = |i: usize| {
            bytes
                .get(4 + i * 4..8 + i * 4)
                .map(|b| b.try_into().unwrap())
        };
        let width = u32::from_le_bytes(word(0)?) as usize;
        let height = u32::from_le_bytes(word(1)?) as usize;
        let float = |i: usize| word(i).map(f32::from_le_bytes);
        let heights = bytes
            .get(28..28 + width * height * 2)?
            .as_chunks::<2>()
            .0
            .iter()
            // decimeters; the sea is near 0 and goes down a few meters off the shore
            .map(|b| {
                let h = f32::from(i16::from_le_bytes(*b)) * 0.1;
                if h < 0.4 { -6.0 } else { h }
            })
            .collect();
        Some(Land {
            width,
            height,
            origin: vec2(float(2)?, float(3)?),
            step: vec2(float(4)?, float(5)?),
            heights,
        })
    }
}

/// The spacing of the land mesh at the distance `d` from the dense middle: 8 m near the
/// bridge, up to 90 m far away.
fn spacing(d: f32) -> f32 {
    (8.0 + (d - 1400.0).max(0.0) * 0.016).min(90.0)
}

/// The grid lines from `from` to `to`, dense near `middle`.
fn lines(from: f32, to: f32, middle: f32) -> Vec<f32> {
    let mut out = vec![from];
    let mut v = from;
    while v < to {
        v = (v + spacing((v - middle).abs())).min(to);
        out.push(v);
    }
    out
}

/// The land as a mesh. It is dense near the bridge and coarse far away. Quads that lie
/// under the sea are left out. The colors are the dry grass of the hills, the dark
/// scrub, and the gray of the city where the buildings stand close.
pub fn land_mesh(bay: &Bay) -> Geometry {
    let land = &bay.land;
    let (origin, size) = land.extent();
    let xs = lines(origin.x, origin.x + size.x, -250.0);
    let zs = lines(origin.y, origin.y + size.y, -500.0);
    let city = density(bay, 96);
    let mut positions = Vec::with_capacity(xs.len() * zs.len());
    let mut uvs = Vec::with_capacity(xs.len() * zs.len());
    let mut colors = Vec::with_capacity(xs.len() * zs.len());
    for &z in &zs {
        for &x in &xs {
            let y = land.at(x, z);
            positions.push(vec3(x, y, z));
            uvs.push(vec2(x, z) / 4.0);
            let grass = Rgb::hex(0xa8946a).0.lerp(
                Rgb::hex(0x6e7650).0,
                0.5 + 0.5 * fbm(31, x / 300.0, z / 300.0, 3),
            );
            let scrub = smoothstep(0.1, 0.5, fbm(37, x / 90.0, z / 90.0, 3));
            let ground = grass.lerp(Rgb::hex(0x343a26).0, scrub * 0.8);
            let urban = (city.at(x, z) / 12.0).min(1.0);
            let color = ground.lerp(Rgb::hex(0x5c5a58).0, urban);
            // the shore: wet dark rock
            colors.push(
                color
                    .lerp(Rgb::hex(0x2a2826).0, 1.0 - smoothstep(0.0, 4.0, y))
                    .extend(1.0),
            );
        }
    }
    let columns = xs.len() as u32;
    let mut indices = Vec::new();
    for j in 0..zs.len() as u32 - 1 {
        for i in 0..columns - 1 {
            let (a, b, c, d) = (
                j * columns + i,
                j * columns + i + 1,
                (j + 1) * columns + i + 1,
                (j + 1) * columns + i,
            );
            if [a, b, c, d].iter().all(|&k| positions[k as usize].y < -1.0) {
                continue;
            }
            // counterclockwise when seen from above
            indices.extend([a, d, b, b, d, c]);
        }
    }
    let mut g = Geometry::new(positions, Vec::new(), uvs, indices);
    g.colors = colors;
    g
}

/// A coarse map of how many buildings stand in each cell, `cells` on a side.
struct Density {
    cells: usize,
    origin: Vec2,
    size: Vec2,
    counts: Vec<f32>,
}

impl Density {
    /// The number of buildings in the cell at `x`, `z`.
    fn at(&self, x: f32, z: f32) -> f32 {
        let u = ((x - self.origin.x) / self.size.x * self.cells as f32) as isize;
        let v = ((z - self.origin.y) / self.size.y * self.cells as f32) as isize;
        if u < 0 || v < 0 || u >= self.cells as isize || v >= self.cells as isize {
            return 0.0;
        }
        self.counts[v as usize * self.cells + u as usize]
    }
}

/// Counts the buildings of `bay` in a grid of `cells` x `cells` over the land.
fn density(bay: &Bay, cells: usize) -> Density {
    let (origin, size) = bay.land.extent();
    let mut counts = vec![0.0; cells * cells];
    for l in &bay.lights {
        let u = ((l[0] - origin.x) / size.x * cells as f32) as usize;
        let v = ((l[1] - origin.y) / size.y * cells as f32) as usize;
        if u < cells && v < cells {
            counts[v * cells + u] += 1.0;
        }
    }
    Density {
        cells,
        origin,
        size,
        counts,
    }
}

/// The sea: a wide plane with small waves from a normal map. It mirrors the scene in a
/// [`Reflector`] at Y = 0.
pub fn water() -> Node {
    // a tileable height field of waves: a few sines with whole numbers of periods
    let size = 256;
    let waves = [
        (3.0f32, 1.0f32, 1.0f32, 0.0f32),
        (-2.0, 5.0, 0.6, 1.3),
        (7.0, -3.0, 0.35, 2.1),
        (11.0, 6.0, 0.2, 0.7),
        (-13.0, 9.0, 0.12, 4.0),
    ];
    let slope = |u: f32, v: f32| {
        let mut d = Vec2::ZERO;
        for (a, b, amp, phase) in waves {
            let k = TAU * (a * u + b * v) + phase;
            d += vec2(a, b) * amp * k.cos();
        }
        d
    };
    let mut rgba = Vec::with_capacity(size * size * 4);
    for y in 0..size {
        for x in 0..size {
            let d = slope(x as f32 / size as f32, y as f32 / size as f32) * 0.012;
            let n = vec3(-d.x, -d.y, 1.0).normalize() * 0.5 + 0.5;
            rgba.extend([
                (n.x * 255.0) as u8,
                (n.y * 255.0) as u8,
                (n.z * 255.0) as u8,
                255,
            ]);
        }
    }
    let normals = Texture::from_rgba8(size as u32, size as u32, &rgba, false)
        .repeat(1.0 / 140.0, 1.0 / 110.0);
    Node::mesh(
        Geometry::floor(60_000.0, 60_000.0),
        Material::standard(0x0b1a24)
            .roughness(0.1)
            .reflection(0.8)
            .normal_map(Arc::new(normals))
            .normal_scale(0.5),
    )
    .shadows(false, false)
}

/// A texture of windows for the towers: 16 x 16 windows in a tile, some lit warm, some
/// lit white, some dark. The texel (0, 0) is a dark frame, for the roofs.
fn windows() -> Arc<Texture> {
    let size = 256u32;
    let rgba: Vec<u8> = (0..size * size)
        .flat_map(|i| {
            let (x, y) = (i % size, i / size);
            let (cx, cy) = (x / 16, y / 16);
            let inside = x % 16 >= 3 && x % 16 < 14 && y % 16 >= 4 && y % 16 < 13;
            let lit = hash(cx * 31 + cy * 977) < 0.42;
            if !inside || !lit {
                return [0, 0, 0, 255];
            }
            let warm = hash(cx * 7 + cy * 13 + 5) < 0.7;
            let level = 0.5 + 0.5 * hash(cx * 3 + cy * 101);
            let color = if warm {
                [255.0, 196.0, 128.0]
            } else {
                [210.0, 225.0, 255.0]
            };
            let [r, g, b] = color.map(|c| (c * level) as u8);
            [r, g, b, 255]
        })
        .collect();
    // one tile is 16 windows of 3 m across and 16 floors of 3.6 m
    Arc::new(Texture::from_rgba8(size, size, &rgba, true).repeat(1.0 / 48.0, 1.0 / 57.6))
}

/// The towers of San Francisco as boxes that taper to their roofs, with windows. The
/// windows glow by the emissive intensity of the material.
pub fn towers(bay: &Bay) -> Node {
    let mut positions = Vec::new();
    let mut uvs = Vec::new();
    let mut indices: Vec<u32> = Vec::new();
    for t in &bay.towers {
        let [cx, cz, hw, hd, angle, ground, height, taper] = *t;
        let (sin, cos) = angle.sin_cos();
        let corner = |sx: f32, sz: f32, scale: f32, y: f32| {
            let (u, v) = (sx * hw * scale, sz * hd * scale);
            vec3(cx + u * cos - v * sin, y, cz + u * sin + v * cos)
        };
        let roof = ground + height;
        let top = taper.max(0.06);
        let ring = [(1.0, 1.0), (-1.0, 1.0), (-1.0, -1.0), (1.0, -1.0)];
        // the walls, each with its own vertices, so the faces are flat
        let mut along = 0.0;
        for k in 0..4 {
            let (a, b) = (ring[k], ring[(k + 1) % 4]);
            let base = positions.len() as u32;
            let p = [
                corner(a.0, a.1, 1.0, ground),
                corner(b.0, b.1, 1.0, ground),
                corner(b.0, b.1, top, roof),
                corner(a.0, a.1, top, roof),
            ];
            let width = p[0].distance(p[1]);
            positions.extend(p);
            uvs.extend([
                vec2(along, height),
                vec2(along + width, height),
                vec2(along + width, 0.0),
                vec2(along, 0.0),
            ]);
            along += width;
            indices.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
        }
        let base = positions.len() as u32;
        positions.extend(ring.map(|(sx, sz)| corner(sx, sz, top, roof)));
        uvs.extend([Vec2::ZERO; 4]);
        indices.extend([base, base + 2, base + 1, base, base + 3, base + 2]);
    }
    let windows = windows();
    Node::mesh(
        Geometry::new(positions, Vec::new(), uvs, indices),
        Material::standard(0x1c1e22)
            .roughness(0.55)
            .emissive(Rgb::WHITE, 0.0)
            .emissive_map(windows)
            .double_sided(),
    )
    .shadows(false, false)
}

/// The lights of the low city and of the Marin shore, as a texture over the land: one dot
/// for each building, and dots on the hills of Sausalito and Fort Baker. Returns the meshes
/// that carry it, in bands by the distance from the ends of the bridge, nearest first, and
/// the texture. The frames light the bands one after the other.
pub fn city_lights(bay: &Bay, ends: [Vec3; 2], bands: usize) -> (Arc<Texture>, Vec<Geometry>) {
    let land = &bay.land;
    let (origin, size) = land.extent();
    let pixels = 2048usize;
    let mut rgba = vec![0u8; pixels * pixels * 4];
    let mut dot = |x: f32, z: f32, color: [f32; 3], level: f32| {
        let u = ((x - origin.x) / size.x * pixels as f32) as usize;
        let v = ((z - origin.y) / size.y * pixels as f32) as usize;
        if u < pixels && v < pixels {
            let i = (v * pixels + u) * 4;
            for c in 0..3 {
                rgba[i + c] = rgba[i + c].saturating_add((color[c] * level) as u8);
            }
            rgba[i + 3] = 255;
        }
    };
    for (n, l) in bay.lights.iter().enumerate() {
        let seed = n as u32;
        if hash(seed) < 0.12 {
            continue;
        }
        let color = if hash(seed + 1) < 0.75 {
            [255.0, 180.0, 100.0]
        } else {
            [220.0, 230.0, 255.0]
        };
        dot(l[0], l[1], color, 0.4 + 0.6 * hash(seed + 2));
    }
    // Sausalito and Fort Baker on the Marin shore: houses on the hills over the water
    let towns = [
        (geo(37.857, -122.486, 0.0), 900.0, 2600u32),
        (geo(37.834, -122.476, 0.0), 260.0, 300),
    ];
    for (center, radius, count) in towns {
        for i in 0..count {
            let a = hash(i * 3 + 7) * TAU;
            let r = radius * hash(i * 3 + 8).sqrt();
            let (x, z) = (center.x + r * a.cos(), center.z + r * a.sin());
            let y = land.at(x, z);
            if y > 2.0 && y < 160.0 {
                dot(x, z, [255.0, 190.0, 120.0], 0.5 + 0.5 * hash(i * 3 + 9));
            }
        }
    }
    let texture = Arc::new(Texture::from_rgba8(
        pixels as u32,
        pixels as u32,
        &rgba,
        true,
    ));

    // the cells of a coarse grid that hold lights, each in the band of its distance
    let cells = 160usize;
    let lit = |i: usize, j: usize| {
        let (x0, y0) = (i * pixels / cells, j * pixels / cells);
        (y0..y0 + pixels / cells)
            .any(|y| (x0..x0 + pixels / cells).any(|x| rgba[(y * pixels + x) * 4 + 3] > 0))
    };
    let mut parts: Vec<Vec<Geometry>> = (0..bands).map(|_| Vec::new()).collect();
    for j in 0..cells {
        for i in 0..cells {
            if !lit(i, j) {
                continue;
            }
            let corner = |di: usize, dj: usize| {
                let (u, v) = (
                    (i + di) as f32 / cells as f32,
                    (j + dj) as f32 / cells as f32,
                );
                let (x, z) = (origin.x + u * size.x, origin.y + v * size.y);
                (vec3(x, land.at(x, z).max(0.0) + 3.0, z), vec2(u, v))
            };
            let quad = [corner(0, 0), corner(1, 0), corner(1, 1), corner(0, 1)];
            let middle = (quad[0].0 + quad[2].0) * 0.5;
            let d = ends
                .iter()
                .map(|e| e.distance(middle))
                .fold(f32::MAX, f32::min);
            let band = ((d / 1500.0) as usize).min(bands - 1);
            parts[band].push(Geometry::new(
                quad.map(|q| q.0).to_vec(),
                vec![Vec3::Y; 4],
                quad.map(|q| q.1).to_vec(),
                vec![0, 3, 1, 1, 3, 2],
            ));
        }
    }
    let meshes = parts.into_iter().map(Geometry::merge).collect();
    (texture, meshes)
}

/// The trees of the Presidio: cypress, pine and eucalyptus on the hills between the bridge
/// and the city, where few buildings stand.
pub fn presidio(bay: &Bay) -> Node {
    let city = density(bay, 96);
    let (a, b) = (geo(37.808, -122.483, 0.0), geo(37.787, -122.447, 0.0));
    let mut trees = Vec::new();
    let mut colors = Vec::new();
    for i in 0..9000u32 {
        let x = mix(a.x, b.x, hash(i * 5 + 1));
        let z = mix(a.z, b.z, hash(i * 5 + 2));
        let y = bay.land.at(x, z);
        if y < 6.0 || city.at(x, z) > 3.0 || fbm(41, x / 160.0, z / 160.0, 2) < -0.25 {
            continue;
        }
        let size = 3.5 + 3.0 * hash(i * 5 + 3);
        trees.push(Mat4::from_scale_rotation_translation(
            vec3(size, size * 1.8, size),
            Quat::from_rotation_y(hash(i) * TAU),
            vec3(x, y + size * 1.2, z),
        ));
        colors.push(Rgb::hex(0x24331f) * (0.7 + 0.5 * hash(i * 5 + 4)));
    }
    Node::instanced(
        Geometry::icosahedron(1.0, 1).flat(),
        Material::standard(0xffffff).roughness(0.9),
        Instances::new(trees).colors(colors),
    )
    .shadows(false, false)
}

/// Sutro Tower on Mount Sutro: three legs 298 m tall, braced at two levels, painted red and
/// white. Returns the steel and the positions of its red lights.
pub fn sutro(bay: &Bay) -> (Node, Vec<Vec3>) {
    let base = geo(37.755_23, -122.452_84, 0.0);
    let ground = bay.land.at(base.x, base.z);
    let leg = |k: usize, y: f32| {
        let a = k as f32 / 3.0 * TAU;
        let r = mix(28.0, 14.0, (y / 298.0).min(1.0));
        vec3(base.x + r * a.cos(), ground + y, base.z + r * a.sin())
    };
    let unit = Geometry::cylinder(1.0, 1.0, 1.0, 6).translate(0.0, 0.5, 0.0);
    let mut parts = Vec::new();
    for k in 0..3 {
        parts.push(
            unit.clone()
                .transform(super::span(leg(k, 0.0), leg(k, 298.0), 2.2)),
        );
        for y in [150.0f32, 230.0] {
            parts.push(
                unit.clone()
                    .transform(super::span(leg(k, y), leg((k + 1) % 3, y), 1.4)),
            );
        }
        parts.push(
            unit.clone()
                .transform(super::span(leg(k, 298.0), leg(k, 330.0), 0.8)),
        );
    }
    let lights = (0..3)
        .flat_map(|k| [leg(k, 332.0), leg(k, 230.0)])
        .collect();
    (
        Node::mesh(
            Geometry::merge(parts),
            Material::standard(0xb84a3a).roughness(0.6),
        )
        .shadows(false, false),
        lights,
    )
}
