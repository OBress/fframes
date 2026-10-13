//! The meshes of Manhattan, built once from the data of the city.

use std::collections::{HashMap, HashSet};

use fframes_3d::math::hash;
use fframes_3d::{Geometry, Mat4, Quat, Rgb, Vec2, Vec3, Vec4, vec2, vec3};

use super::data::{Building, Class, Manhattan, Street};
use super::textures::{BAYS, FLOORS, LAMP_SPACING, Style};

/// The height of one floor in meters.
pub const FLOOR: f32 = 3.6;
/// The width of one window bay in meters.
pub const BAY: f32 = 1.8;
/// The sizes of the video screens in meters, 4:3 like the film that they play.
pub const SCREENS: [Vec2; 3] = [
    Vec2::new(40.0, 30.0),
    Vec2::new(32.0, 24.0),
    Vec2::new(24.0, 18.0),
];
/// The Building Identification Numbers of the two crowns that the city lights.
pub const EMPIRE_STATE: u32 = 1015862;
pub const CHRYSLER: u32 = 1036156;

/// Vertices and triangles that grow into one [`Geometry`].
#[derive(Default)]
pub struct MeshBuf {
    positions: Vec<Vec3>,
    normals: Vec<Vec3>,
    uvs: Vec<Vec2>,
    /// Empty, or one color for each vertex.
    colors: Vec<Vec4>,
    indices: Vec<u32>,
}

/// The faces of a unit cube: the normal and the up direction of each face.
const FACES: [(Vec3, Vec3); 6] = [
    (Vec3::X, Vec3::Y),
    (Vec3::NEG_X, Vec3::Y),
    (Vec3::Z, Vec3::Y),
    (Vec3::NEG_Z, Vec3::Y),
    (Vec3::Y, Vec3::NEG_Z),
    (Vec3::NEG_Y, Vec3::Z),
];

impl MeshBuf {
    /// Adds a flat quad. The corners go counterclockwise as seen from the front: bottom
    /// left, bottom right, top right, top left.
    pub fn quad(&mut self, corners: [Vec3; 4], uvs: [Vec2; 4]) {
        let n = (corners[1] - corners[0])
            .cross(corners[2] - corners[0])
            .normalize_or(Vec3::Y);
        let base = self.positions.len() as u32;
        self.positions.extend(corners);
        self.normals.extend([n; 4]);
        self.uvs.extend(uvs);
        self.indices
            .extend([base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    /// Adds a flat triangle. The corners go counterclockwise as seen from the front.
    pub fn triangle(&mut self, corners: [Vec3; 3], uvs: [Vec2; 3]) {
        let n = (corners[1] - corners[0])
            .cross(corners[2] - corners[0])
            .normalize_or(Vec3::Y);
        let base = self.positions.len() as u32;
        self.positions.extend(corners);
        self.normals.extend([n; 3]);
        self.uvs.extend(uvs);
        self.indices.extend([base, base + 1, base + 2]);
    }

    /// Adds the unit cube around the origin, transformed by `m`. Each face has UVs from 0 to
    /// 1. `m` must not mirror.
    pub fn cube(&mut self, m: Mat4) {
        for (n, up) in FACES {
            let right = up.cross(n);
            let c = n * 0.5;
            let corners = [
                c - right * 0.5 - up * 0.5,
                c + right * 0.5 - up * 0.5,
                c + right * 0.5 + up * 0.5,
                c - right * 0.5 + up * 0.5,
            ]
            .map(|p| m.transform_point3(p));
            self.quad(corners, UNIT_UV);
        }
    }

    /// Adds an axis aligned box from `min` to `max`.
    pub fn block(&mut self, min: Vec3, max: Vec3) {
        self.cube(Mat4::from_scale_rotation_translation(
            max - min,
            Quat::IDENTITY,
            (min + max) * 0.5,
        ));
    }

    /// Adds a square bar of `thickness` meters from `a` to `b`.
    pub fn bar(&mut self, a: Vec3, b: Vec3, thickness: f32) {
        let d = b - a;
        let length = d.length();
        if length < 1e-4 {
            return;
        }
        let dir = d / length;
        let side = if dir.y.abs() < 0.9 { Vec3::Y } else { Vec3::X };
        let s1 = dir.cross(side).normalize();
        let s2 = s1.cross(dir);
        self.cube(Mat4::from_cols(
            (s1 * thickness).extend(0.0),
            (dir * length).extend(0.0),
            (s2 * thickness).extend(0.0),
            ((a + b) * 0.5).extend(1.0),
        ));
    }

    /// Adds a flat rectangle on the ground at height `y`, with UVs in meters.
    pub fn ground_rect(&mut self, min: Vec2, max: Vec2, y: f32) {
        let corners = [
            vec3(min.x, y, max.y),
            vec3(max.x, y, max.y),
            vec3(max.x, y, min.y),
            vec3(min.x, y, min.y),
        ];
        self.quad(corners, corners.map(|p| vec2(p.x, p.z)));
    }

    /// Adds a flat strip along a street at height `y`. The UVs are in meters, or, with
    /// `along_scale`, U runs along the street in units of `along_scale` meters and V runs
    /// across it from 0 to 1.
    pub fn strip(&mut self, from: Vec2, to: Vec2, width: f32, y: f32, along_scale: Option<f32>) {
        let dir = (to - from).normalize();
        let side = vec2(-dir.y, dir.x) * width * 0.5;
        let p = |q: Vec2| vec3(q.x, y, q.y);
        let corners = [p(from + side), p(to + side), p(to - side), p(from - side)];
        let uvs = match along_scale {
            Some(scale) => {
                let length = from.distance(to) / scale;
                [
                    vec2(0.0, 0.0),
                    vec2(length, 0.0),
                    vec2(length, 1.0),
                    vec2(0.0, 1.0),
                ]
            }
            None => corners.map(|c| vec2(c.x, c.z)),
        };
        // The corners must go counterclockwise seen from above.
        if (corners[1] - corners[0]).cross(corners[2] - corners[0]).y < 0.0 {
            let [a, b, c, d] = corners;
            let [ua, ub, uc, ud] = uvs;
            self.quad([b, a, d, c], [ub, ua, ud, uc]);
        } else {
            self.quad(corners, uvs);
        }
    }

    /// Adds all vertices and triangles of `other`.
    pub fn append(&mut self, other: MeshBuf) {
        let base = self.positions.len() as u32;
        if !other.colors.is_empty() {
            self.paint(base as usize, Rgb::WHITE);
            self.colors.extend(other.colors);
        }
        self.positions.extend(other.positions);
        self.normals.extend(other.normals);
        self.uvs.extend(other.uvs);
        self.indices.extend(other.indices.iter().map(|i| i + base));
    }

    /// The number of vertices.
    pub fn len(&self) -> usize {
        self.positions.len()
    }

    pub fn set_uv(&mut self, vertex: usize, uv: Vec2) {
        self.uvs[vertex] = uv;
    }

    /// Gives the vertices from `first` on the color `color`. The vertices before them get
    /// white when they have no color yet.
    pub fn paint(&mut self, first: usize, color: Rgb) {
        self.colors.resize(first, Vec4::ONE);
        self.colors
            .resize(self.positions.len(), color.0.extend(1.0));
    }

    /// Gives vertex `vertex` the color `color`. The vertices without a color get white.
    pub fn paint_vertex(&mut self, vertex: usize, color: Rgb) {
        self.colors.resize(self.positions.len(), Vec4::ONE);
        self.colors[vertex] = color.0.extend(1.0);
    }

    pub fn geometry(self) -> Geometry {
        let mut colors = self.colors;
        if !colors.is_empty() {
            colors.resize(self.positions.len(), Vec4::ONE);
        }
        let mut geometry = Geometry::new(self.positions, self.normals, self.uvs, self.indices);
        geometry.colors = colors;
        geometry
    }
}

const UNIT_UV: [Vec2; 4] = [
    Vec2::new(0.0, 1.0),
    Vec2::new(1.0, 1.0),
    Vec2::new(1.0, 0.0),
    Vec2::new(0.0, 0.0),
];

/// The facades, roofs and roof details of the buildings that rise together.
#[derive(Default)]
pub struct RiseGroup {
    /// The facade meshes, in the order of [`STYLES`].
    pub walls: [MeshBuf; 5],
    /// The roofs inside the chip square, then outside it.
    pub roofs: [MeshBuf; 2],
    pub details: MeshBuf,
    /// The light bands on the crowns of some towers.
    pub crowns: MeshBuf,
    /// The light of the flood lights on the top of the Empire State Building, added to the
    /// facade: the stone takes the light, the glass stays dark. The vertex colors hold the
    /// brightness.
    pub floodlit: MeshBuf,
    /// The steel crown of the Chrysler Building with its lit windows. The vertex colors hold
    /// the brightness.
    pub chrome: MeshBuf,
    /// The water towers on the old roofs, as transforms of [`water_tower`].
    pub water_towers: Vec<Mat4>,
    /// The small lights on the roofs, at full height: red aviation lights and the lights
    /// around the helipads.
    pub lights: Vec<RoofLight>,
    /// The quads of the video screens.
    pub screens: Vec<MeshBuf>,
    /// The second at which the buildings start to rise.
    pub start: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct RoofLight {
    pub at: Vec3,
    /// Red and blinking, or a steady white light.
    pub aviation: bool,
}

pub const STYLES: [Style; 5] = [
    Style::Glass,
    Style::Fins,
    Style::Office,
    Style::Prewar,
    Style::Low,
];
/// The number of rise groups.
pub const GROUPS: usize = 12;
/// The seconds that one building takes to rise.
pub const RISE_TIME: f32 = 1.2;
/// The first and the last start of a rise, in seconds.
const RISE_FIRST: f32 = 0.55;
const RISE_LAST: f32 = 3.3;

/// The second at which a building `p` meters from the point of the dive starts to rise.
/// The buildings below the dive rise first, and the wave runs outward.
pub fn rise_start(p: Vec2, seed: u32) -> f32 {
    (RISE_FIRST + 1.7 * (p.length() / 800.0).powf(0.8) + 0.45 * hash(seed)).min(RISE_LAST)
}

/// The rise group of a start time.
pub fn group_of(start: f32) -> usize {
    let k = (start - RISE_FIRST) / (RISE_LAST - RISE_FIRST);
    (k * GROUPS as f32).clamp(0.0, GROUPS as f32 - 1.0) as usize
}

/// The start time of a rise group.
pub fn group_start(group: usize) -> f32 {
    RISE_FIRST + (RISE_LAST - RISE_FIRST) * (group as f32 + 0.5) / GROUPS as f32
}

/// The facade of a building: stone before 1946, glass on the towers after it.
pub fn style(b: &Building) -> Style {
    if b.height < 22.0 {
        Style::Low
    } else if b.year != 0 && b.year < 1946 {
        Style::Prewar
    } else if b.height > 90.0 && (b.year == 0 || b.year >= 1958) {
        if hash(b.bin) < 0.5 {
            Style::Glass
        } else {
            Style::Fins
        }
    } else {
        Style::Office
    }
}

/// The tints of the facades: the glass and the frames of each building differ a little.
const TINTS: [u32; 6] = [0xffffff, 0xc8d4e6, 0xb4ccd2, 0xd2d2d2, 0x9aa4b0, 0xc0d6c8];

/// The roof of a building at its highest level.
struct Top {
    height: f32,
    min: Vec2,
    max: Vec2,
    /// A point inside the top roof, the center of its largest triangle.
    inner: Vec2,
    area: f32,
}

fn top(b: &Building) -> Option<Top> {
    let height = b
        .roofs
        .iter()
        .flat_map(|t| t.iter().map(|p| p.y))
        .fold(f32::MIN, f32::max);
    let tris: Vec<&[Vec3; 3]> = b
        .roofs
        .iter()
        .filter(|t| t.iter().all(|p| p.y > height - 0.5))
        .collect();
    let area = |t: &[Vec3; 3]| (t[1] - t[0]).cross(t[2] - t[0]).length() * 0.5;
    let largest = tris.iter().max_by(|a, b| area(a).total_cmp(&area(b)))?;
    let (mut min, mut max) = (Vec2::MAX, Vec2::MIN);
    for p in tris.iter().flat_map(|t| t.iter()) {
        min = min.min(vec2(p.x, p.z));
        max = max.max(vec2(p.x, p.z));
    }
    let c = (largest[0] + largest[1] + largest[2]) / 3.0;
    Some(Top {
        height,
        min,
        max,
        inner: vec2(c.x, c.z),
        area: tris.iter().map(|t| area(t)).sum(),
    })
}

/// The walls of a building with UVs in window bays and floors: U runs along the wall, V up.
/// Each building starts at a random place in the texture.
fn walls(buf: &mut MeshBuf, tris: &[[Vec3; 3]], seed: u32, tint: Rgb) {
    let first = buf.len();
    let u0 = (hash(seed) * BAYS).floor() / BAYS;
    let v0 = (hash(seed + 1) * FLOORS).floor() / FLOORS;
    for t in tris {
        let n = (t[1] - t[0]).cross(t[2] - t[0]);
        // The direction to the right of a viewer in front of the wall.
        let along = vec2(n.z, -n.x).normalize_or(Vec2::X);
        let uv = t.map(|p| {
            vec2(
                u0 + (p.x * along.x + p.z * along.y) / (BAY * BAYS),
                v0 - p.y / (FLOOR * FLOORS),
            )
        });
        buf.triangle(*t, uv);
    }
    buf.paint(first, tint);
}

/// Roof triangles with UVs from 0 to 1 over the box of the building, so the roof picture
/// has its rim at the edges of the building.
fn roofs(buf: &mut MeshBuf, tris: &[[Vec3; 3]]) {
    let (mut min, mut max) = (Vec2::MAX, Vec2::MIN);
    for p in tris.iter().flat_map(|t| t.iter()) {
        min = min.min(vec2(p.x, p.z));
        max = max.max(vec2(p.x, p.z));
    }
    let size = (max - min).max(Vec2::splat(1.0));
    for t in tris {
        buf.triangle(*t, t.map(|p| (vec2(p.x, p.z) - min) / size));
    }
}

/// Walls with a brightness in their vertex colors, for a facade lit from outside.
/// `brightness` gets the corner and its triangle.
fn lit(
    buf: &mut MeshBuf,
    tris: &[[Vec3; 3]],
    seed: u32,
    brightness: impl Fn(Vec3, &[Vec3; 3]) -> f32,
) {
    let first = buf.len();
    walls(buf, tris, seed, Rgb::WHITE);
    for (k, t) in tris.iter().enumerate() {
        for (c, p) in t.iter().enumerate() {
            buf.paint_vertex(first + 3 * k + c, Rgb::WHITE * brightness(*p, t));
        }
    }
}

/// The ledges of the setbacks of the Empire State Building from the 72nd floor up, and
/// the foot of the mast, in meters.
const LEDGES: [f32; 4] = [265.5, 299.0, 323.0, 383.0];

/// The Empire State Building has dark stone and lit windows like the other towers. From
/// the 72nd floor up, flood lights on the ledges of the setbacks wash the stone in a soft
/// color, and the mast glows.
fn empire_state(group: &mut RiseGroup, b: &Building, k: usize, tint: Rgb, inside: bool) {
    let above = |t: &[Vec3; 3], y: f32| t.iter().all(|p| p.y > y);
    let mast = LEDGES[3];
    let body: Vec<[Vec3; 3]> = b
        .walls
        .iter()
        .filter(|t| !above(t, mast))
        .copied()
        .collect();
    walls(&mut group.walls[k], &body, b.bin, tint);
    let roof_tris: Vec<[Vec3; 3]> = b
        .roofs
        .iter()
        .filter(|t| !above(t, mast))
        .copied()
        .collect();
    roofs(&mut group.roofs[usize::from(!inside)], &roof_tris);
    // The light of the flood lights is a second skin 0.3 m in front of the walls. Its
    // picture is the facade, so the stone takes the light and the glass stays dark.
    let washed: Vec<[Vec3; 3]> = body
        .iter()
        .flat_map(|t| {
            let n = (t[1] - t[0]).cross(t[2] - t[0]).normalize_or_zero();
            cut_above(t, LEDGES[0])
                .into_iter()
                .map(move |c| c.map(|p| p + n * 0.3))
        })
        .collect();
    lit(&mut group.floodlit, &washed, b.bin, flood_light);
    // The mast glows evenly: all of it shows a point of the stone between the windows.
    let first = group.floodlit.len();
    let top: Vec<[Vec3; 3]> = b
        .walls
        .iter()
        .chain(&b.roofs)
        .filter(|t| above(t, mast))
        .copied()
        .collect();
    lit(&mut group.floodlit, &top, b.bin, |p, _| {
        2.6 - 1.2 * ((p.y - mast) / 60.0).clamp(0.0, 1.0)
    });
    for i in first..group.floodlit.len() {
        group.floodlit.set_uv(i, vec2(0.1 / BAYS, 0.05 / FLOORS));
    }
}

/// The part of a triangle above the height `y`, as triangles.
fn cut_above(t: &[Vec3; 3], y: f32) -> Vec<[Vec3; 3]> {
    let mut ring = Vec::with_capacity(4);
    for (i, &a) in t.iter().enumerate() {
        let b = t[(i + 1) % 3];
        if a.y >= y {
            ring.push(a);
        }
        if (a.y < y) != (b.y < y) {
            ring.push(a.lerp(b, (y - a.y) / (b.y - a.y)));
        }
    }
    (2..ring.len())
        .map(|i| [ring[0], ring[i - 1], ring[i]])
        .collect()
}

/// The light of the flood lights on the ledges of the setbacks of the Empire State
/// Building. Each face is brightest at its foot. The faces toward the avenues get more
/// light than the faces toward the streets, the higher setbacks more than the lower.
fn flood_light(p: Vec3, t: &[Vec3; 3]) -> f32 {
    // The setback of the triangle, from its middle: a corner on a ledge belongs to the
    // face above or below it.
    let mid = (t[0].y + t[1].y + t[2].y) / 3.0;
    let n = (t[1] - t[0]).cross(t[2] - t[0]).normalize_or(Vec3::Y);
    let i = LEDGES.partition_point(|&l| l <= mid).clamp(1, 3);
    let k = ((p.y - LEDGES[i - 1]) / (LEDGES[i] - LEDGES[i - 1])).clamp(0.0, 1.0);
    let face = 0.55 + 0.35 * n.x.abs();
    let high = [0.2, 0.55, 1.0][i - 1];
    (0.2 + 2.4 * (1.0 - k) * (1.0 - k)) * face * high
}

/// The meshes of all buildings, sorted into rise groups. `dive` is the point below the
/// first frame. `skip` holds the buildings that construction sites replace, `screens`
/// the video screens.
pub fn rise_groups(
    city: &Manhattan,
    dive: Vec2,
    chip_half: f32,
    skip: &HashSet<usize>,
    screens: &[Screen],
) -> Vec<RiseGroup> {
    let mut groups: Vec<RiseGroup> = (0..GROUPS)
        .map(|g| RiseGroup {
            start: group_start(g),
            ..Default::default()
        })
        .collect();
    for (i, b) in city.buildings.iter().enumerate() {
        if skip.contains(&i) {
            continue;
        }
        let g = &mut groups[group_of(rise_start(b.center - dive, b.bin))];
        building(g, b, (b.center - dive).abs().max_element() < chip_half);
    }
    for screen in screens {
        let b = &city.buildings[screen.building];
        let g = &mut groups[group_of(rise_start(b.center - dive, b.bin))];
        screen_quad(g, screen);
    }
    groups
}

/// Adds the walls, the roofs and the roof details of one building.
fn building(group: &mut RiseGroup, b: &Building, inside: bool) {
    let style = style(b);
    let tint = Rgb::hex(TINTS[(hash(b.bin + 50) * TINTS.len() as f32) as usize % TINTS.len()]);
    let k = STYLES.iter().position(|s| *s == style).unwrap_or(0);
    if b.bin == EMPIRE_STATE {
        empire_state(group, b, k, tint, inside);
    } else {
        // The crown of the Chrysler Building keeps its light above 232 m.
        let crown = if b.bin == CHRYSLER { 232.0 } else { f32::MAX };
        let (high, low): (Vec<[Vec3; 3]>, Vec<[Vec3; 3]>) = b
            .walls
            .iter()
            .chain(b.roofs.iter().filter(|t| t.iter().all(|p| p.y > crown)))
            .partition(|t| t.iter().all(|p| p.y > crown - 0.1));
        walls(&mut group.walls[k], &low, b.bin, tint);
        let roof_tris: Vec<[Vec3; 3]> = b
            .roofs
            .iter()
            .filter(|t| t.iter().any(|p| p.y <= crown))
            .copied()
            .collect();
        roofs(&mut group.roofs[usize::from(!inside)], &roof_tris);
        // The arches of the Chrysler crown: rows of lit windows between steel bands.
        lit(&mut group.chrome, &high, b.bin, |p, _| {
            if (p.y / 5.5).fract() < 0.55 {
                1.4
            } else {
                0.15
            }
        });
    }
    if b.far {
        return;
    }
    if let Some(top) = top(b) {
        roof_details(group, b, style, &top);
    }
}

/// Water towers on the old roofs, plant rooms on the new ones, light bands and red
/// aviation lights on the towers.
fn roof_details(group: &mut RiseGroup, b: &Building, style: Style, top: &Top) {
    let s = b.bin;
    let size = top.max - top.min;
    let old = b.year != 0 && b.year < 1946;
    if old && b.height < 160.0 && top.area > 120.0 && hash(s + 20) < 0.7 {
        // Manhattan's wooden water tanks on steel legs.
        let turn = hash(s + 21) * std::f32::consts::TAU;
        group.water_towers.push(Mat4::from_rotation_translation(
            Quat::from_rotation_y(turn),
            vec3(top.inner.x, top.height, top.inner.y),
        ));
    } else if !old && top.area > 300.0 && hash(s + 22) < 0.8 {
        let room = (size * (0.3 + 0.2 * hash(s + 23))).clamp(Vec2::splat(5.0), Vec2::splat(22.0));
        let h = 3.5 + 3.0 * hash(s + 24);
        let c = top.inner;
        group.details.block(
            vec3(c.x - room.x * 0.5, top.height, c.y - room.y * 0.5),
            vec3(c.x + room.x * 0.5, top.height + h, c.y + room.y * 0.5),
        );
    }
    if b.height > 160.0 {
        let corner = |sx: f32, sz: f32| {
            let c = (top.min + top.max) * 0.5;
            vec3(
                c.x + sx * (size.x * 0.5 - 0.6),
                top.height + 1.4,
                c.y + sz * (size.y * 0.5 - 0.6),
            )
        };
        group.lights.push(RoofLight {
            at: corner(-1.0, -1.0),
            aviation: true,
        });
        // A light at the tip of a mast or a spire.
        let tip = b
            .walls
            .iter()
            .flatten()
            .max_by(|a, b| a.y.total_cmp(&b.y))
            .copied()
            .unwrap_or(Vec3::ZERO);
        if tip.y > top.height + 8.0 {
            group.lights.push(RoofLight {
                at: tip + Vec3::Y * 0.5,
                aviation: true,
            });
        }
        if matches!(style, Style::Glass | Style::Fins) && hash(s + 60) < 0.45 {
            // A band of light around the edge of the top roof, a little below it.
            let crown = &mut group.crowns;
            let first = crown.len();
            for (from, to, inside) in roof_edges(b, top.height) {
                let out = vec2(to.z - from.z, from.x - to.x).normalize_or_zero();
                let out = if out.dot(inside - vec2(from.x, from.z)) > 0.0 {
                    -out
                } else {
                    out
                };
                let shift = vec3(out.x, 0.0, out.y) * 0.35 - Vec3::Y * 1.85;
                crown.bar(from + shift, to + shift, 0.6);
            }
            // White light bands, and some in cyan.
            let color = if hash(s + 61) < 0.35 {
                0x5fe6ff
            } else {
                0xcfe4ff
            };
            crown.paint(first, Rgb::hex(color));
        }
    }
}

/// The outer edges of the roof at `height`: the edges of only one of its triangles, each
/// with a point inside the roof next to it.
fn roof_edges(b: &Building, height: f32) -> Vec<(Vec3, Vec3, Vec2)> {
    let key = |p: Vec3| [(p.x * 10.0).round() as i32, (p.z * 10.0).round() as i32];
    // The number of triangles at each edge, with its corners and a point inside.
    let mut edges: HashMap<[[i32; 2]; 2], (u32, Vec3, Vec3, Vec2)> = HashMap::new();
    for t in b
        .roofs
        .iter()
        .filter(|t| t.iter().all(|p| p.y > height - 0.5))
    {
        let c = (t[0] + t[1] + t[2]) / 3.0;
        for i in 0..3 {
            let (p, q) = (t[i], t[(i + 1) % 3]);
            let k = if key(p) < key(q) {
                [key(p), key(q)]
            } else {
                [key(q), key(p)]
            };
            edges.entry(k).or_insert((0, p, q, vec2(c.x, c.z))).0 += 1;
        }
    }
    edges
        .into_values()
        .filter(|e| e.0 == 1)
        .map(|(_, a, b, inside)| (a, b, inside))
        .collect()
}

/// A wooden water tank on four steel legs, 5.5 m wide and 11 m high, standing on the
/// origin.
pub fn water_tower() -> Geometry {
    let mut buf = MeshBuf::default();
    for (x, z) in [(-1.9, -1.9), (1.9, -1.9), (1.9, 1.9), (-1.9, 1.9)] {
        buf.bar(vec3(x, 0.0, z), vec3(x * 0.8, 4.2, z * 0.8), 0.25);
    }
    buf.block(vec3(-2.4, 4.0, -2.4), vec3(2.4, 4.3, 2.4));
    let mut geometry = buf.geometry();
    geometry = Geometry::merge([
        geometry,
        Geometry::cylinder(2.75, 2.75, 5.4, 14).translate(0.0, 7.0, 0.0),
        Geometry::cone(3.0, 1.8, 14).translate(0.0, 10.6, 0.0),
    ]);
    geometry
}

/// A video screen on the face of a building.
#[derive(Debug, Clone, Copy)]
pub struct Screen {
    /// The building that carries it.
    pub building: usize,
    /// The center of the screen in front of the wall.
    pub center: Vec3,
    /// The direction that the screen faces, on the ground plane.
    pub facing: Vec2,
    /// The width and the height.
    pub size: Vec2,
}

/// The places for a video screen on the walls of Times Square that face the camera on
/// Broadway: on each wall in one plane, for each size, a grid of places 8 m apart.
pub fn screen_walls(city: &Manhattan, area: (Vec2, Vec2), toward: Vec2) -> Vec<Screen> {
    let mut out = Vec::new();
    for (i, b) in city.buildings.iter().enumerate() {
        if b.far || !(b.center.cmpge(area.0).all() && b.center.cmple(area.1).all()) {
            continue;
        }
        // The walls of the building in one plane: the normal, and the extent along the wall
        // (x) and up (y).
        let mut planes: HashMap<[i32; 3], (Vec2, Vec2, Vec2)> = HashMap::new();
        for t in &b.walls {
            let n = (t[1] - t[0]).cross(t[2] - t[0]);
            let nh = vec2(n.x, n.z);
            if n.y.abs() > 0.1 * n.length() || nh.length() < 1e-6 {
                continue;
            }
            let nh = nh.normalize();
            if nh.dot(-toward) < 0.3 {
                continue;
            }
            let along = vec2(nh.y, -nh.x);
            let offset = vec2(t[0].x, t[0].z).dot(nh);
            let key = [
                (nh.x * 50.0) as i32,
                (nh.y * 50.0) as i32,
                offset.round() as i32,
            ];
            let e = planes.entry(key).or_insert((nh, Vec2::MAX, Vec2::MIN));
            for p in t {
                let q = vec2(vec2(p.x, p.z).dot(along), p.y);
                e.1 = e.1.min(q);
                e.2 = e.2.max(q);
            }
        }
        for ([_, _, offset], (nh, min, max)) in planes {
            let (u0, u1, y0, y1) = (min.x, max.x, min.y, max.y);
            let along = vec2(nh.y, -nh.x);
            for size in SCREENS {
                let half = size * 0.5;
                // Times Square has screens from the third floor up to 150 m.
                let mut y = y0.max(0.0) + half.y + 10.0;
                while y <= (y1 - half.y - 4.0).min(150.0) {
                    let mut u = u0 + half.x + 1.0;
                    while u <= u1 - half.x - 1.0 {
                        let mid = along * u + nh * (offset as f32 + 0.8);
                        out.push(Screen {
                            building: i,
                            center: vec3(mid.x, y, mid.y),
                            facing: nh,
                            size,
                        });
                        u += 8.0;
                    }
                    y += 8.0;
                }
            }
        }
    }
    out
}

/// Adds the quad of a screen and its dark frame.
fn screen_quad(g: &mut RiseGroup, screen: &Screen) {
    let right = vec2(screen.facing.y, -screen.facing.x);
    let half = screen.size * 0.5;
    let c = screen.center;
    let p = |side: f32, up: f32, depth: f32| {
        let q = vec2(c.x, c.z) + right * side - screen.facing * depth;
        vec3(q.x, c.y + up, q.y)
    };
    let mut quad = MeshBuf::default();
    quad.quad(
        [
            p(-half.x, -half.y, 0.0),
            p(half.x, -half.y, 0.0),
            p(half.x, half.y, 0.0),
            p(-half.x, half.y, 0.0),
        ],
        UNIT_UV,
    );
    g.screens.push(quad);
    let (a, b) = (
        p(-half.x - 0.8, -half.y - 0.8, 0.8),
        p(half.x + 0.8, half.y + 0.8, 0.1),
    );
    g.details.block(a.min(b), a.max(b));
}

/// The nodes of the street net: the points where three or more segments end.
pub fn crossings(streets: &[Street]) -> Vec<Vec2> {
    let mut count: HashMap<(i32, i32), (u32, Vec2)> = HashMap::new();
    for s in streets {
        for p in [s.points[0], s.points[s.points.len() - 1]] {
            let e = count
                .entry(((p.x * 0.5).round() as i32, (p.y * 0.5).round() as i32))
                .or_insert((0, p));
            e.0 += 1;
        }
    }
    count
        .into_values()
        .filter(|(n, _)| *n >= 3)
        .map(|(_, p)| p)
        .collect()
}

/// The segments of a street, each one as a strip that reaches half its width past both
/// ends, so the strips close the corners.
fn segments(s: &Street) -> impl Iterator<Item = (Vec2, Vec2)> + '_ {
    s.points.windows(2).filter_map(move |w| {
        let d = w[1] - w[0];
        let length = d.length();
        (length > 0.5).then(|| {
            let d = d / length;
            (w[0] - d * s.width * 0.5, w[1] + d * s.width * 0.5)
        })
    })
}

/// The street surfaces. The avenues lie a little higher, so they cover the streets at the
/// crossings.
pub fn streets(city: &Manhattan) -> Geometry {
    let mut buf = MeshBuf::default();
    for s in &city.streets {
        let y = if s.class == Class::Street { 0.04 } else { 0.07 };
        for (a, b) in segments(s) {
            buf.strip(a, b, s.width, y, None);
        }
    }
    buf.geometry()
}

/// The traces of the chip inside the chip square around `dive`: a narrow strip along the
/// middle of each street and a square pad at each crossing. Returns the gold traces (the
/// avenues) and the copper traces, moved by `-origin`, scaled by `scale` and at the height
/// `y`.
pub fn traces(
    city: &Manhattan,
    crossings: &[Vec2],
    (dive, half): (Vec2, f32),
    origin: Vec2,
    scale: f32,
    y: f32,
) -> [MeshBuf; 2] {
    let mut out = [MeshBuf::default(), MeshBuf::default()];
    let inside = |p: Vec2| (p - dive).abs().max_element() < half;
    for s in &city.streets {
        if s.class == Class::Highway {
            continue;
        }
        let k = usize::from(s.class == Class::Street);
        let lift = if k == 0 { 0.01 } else { 0.0 };
        for (a, b) in segments(s) {
            if inside(a) && inside(b) {
                out[k].strip(
                    (a - origin) * scale,
                    (b - origin) * scale,
                    s.width * 0.42 * scale,
                    y + lift * scale,
                    None,
                );
            }
        }
    }
    for &p in crossings {
        if inside(p) {
            let pad = Vec2::splat(6.5);
            out[0].ground_rect(
                (p - origin - pad) * scale,
                (p - origin + pad) * scale,
                y + 0.02 * scale,
            );
        }
    }
    out
}

/// The light pools of the street lamps, one strip over each street (see
/// [`super::textures::lamp_pools`]).
pub fn lamp_light(city: &Manhattan, near: impl Fn(Vec2) -> bool) -> Geometry {
    let mut buf = MeshBuf::default();
    for s in &city.streets {
        for (a, b) in segments(s) {
            if near(a) {
                buf.strip(a, b, s.width + 6.0, 0.3, Some(LAMP_SPACING));
            }
        }
    }
    buf.geometry()
}

/// The lamps along the streets near the flight: the transforms of the poles and the
/// positions of the lamp heads. The pole geometry stands at the origin with its arm along
/// +X.
pub fn lamps(
    city: &Manhattan,
    crossings: &[Vec2],
    near: impl Fn(Vec2) -> bool,
) -> (Vec<Mat4>, Vec<Vec3>) {
    // The crossings on a grid of 40 m cells, to find the ones near a point fast.
    let mut cells: HashMap<(i32, i32), Vec<Vec2>> = HashMap::new();
    let cell = |p: Vec2| ((p.x / 40.0).floor() as i32, (p.y / 40.0).floor() as i32);
    for &c in crossings {
        cells.entry(cell(c)).or_default().push(c);
    }
    let at_crossing = |p: Vec2| {
        let (cx, cz) = cell(p);
        (-1..=1).any(|i| {
            (-1..=1).any(|j| {
                cells
                    .get(&(cx + i, cz + j))
                    .is_some_and(|v| v.iter().any(|c| c.distance(p) < 16.0))
            })
        })
    };
    let mut poles = Vec::new();
    let mut heads = Vec::new();
    for s in &city.streets {
        if s.class == Class::Highway {
            continue;
        }
        for w in s.points.windows(2) {
            let length = w[0].distance(w[1]);
            if length < 1.0 || !near(w[0]) {
                continue;
            }
            let dir = (w[1] - w[0]) / length;
            let side = vec2(-dir.y, dir.x);
            for (k, sign) in [(0.0, 1.0f32), (0.5, -1.0)] {
                let mut along = k * LAMP_SPACING;
                while along < length {
                    let base = w[0] + dir * along + side * sign * (s.width * 0.5 + 1.2);
                    along += LAMP_SPACING;
                    if at_crossing(base) {
                        continue;
                    }
                    // The arm reaches over the street.
                    let inward = -side * sign;
                    let angle = inward.y.atan2(inward.x);
                    poles.push(Mat4::from_rotation_translation(
                        Quat::from_rotation_y(-angle),
                        vec3(base.x, 0.0, base.y),
                    ));
                    let head = base + inward * 2.2;
                    heads.push(vec3(head.x, 8.6, head.y));
                }
            }
        }
    }
    (poles, heads)
}

/// A street lamp: a pole of 9 m with an arm along +X.
pub fn lamp_pole() -> Geometry {
    let mut buf = MeshBuf::default();
    buf.block(vec3(-0.12, 0.0, -0.12), vec3(0.12, 9.0, 0.12));
    buf.block(vec3(0.0, 8.75, -0.07), vec3(2.4, 8.9, 0.07));
    buf.block(vec3(1.9, 8.55, -0.18), vec3(2.6, 8.75, 0.18));
    buf.geometry()
}

/// The painted lines of the avenues and the zebra crossings near the flight.
pub fn markings(city: &Manhattan, crossings: &[Vec2], near: impl Fn(Vec2) -> bool) -> Geometry {
    let mut buf = MeshBuf::default();
    let y = 0.15;
    for s in &city.streets {
        if s.class != Class::Avenue && s.class != Class::Broadway {
            continue;
        }
        let lanes = (s.width / 3.3).floor() as i32;
        for w in s.points.windows(2) {
            let length = w[0].distance(w[1]);
            if length < 1.0 || !near(w[0]) {
                continue;
            }
            let dir = (w[1] - w[0]) / length;
            let side = vec2(-dir.y, dir.x);
            for lane in 1..lanes {
                let offset = side * (lane as f32 * s.width / lanes as f32 - s.width * 0.5);
                let mut along = 12.0;
                while along + 3.0 < length - 12.0 {
                    let a = w[0] + dir * along + offset;
                    buf.strip(a, a + dir * 3.0, 0.15, y, Some(1.0));
                    along += 9.0;
                }
            }
        }
    }
    // Zebra crossings on the four sides of each crossing near the flight.
    for &c in crossings {
        if !near(c) {
            continue;
        }
        for axis in [Vec2::X, Vec2::Y] {
            let across = vec2(axis.y, axis.x);
            for sign in [-1.0, 1.0] {
                let center = c + across * sign * 13.0;
                let mut o = -7.0;
                while o < 7.0 {
                    let a = center + axis * o - across * 2.0;
                    buf.strip(a, a + across * 4.0, 0.55, 0.16, Some(1.0));
                    o += 1.2;
                }
            }
        }
    }
    buf.geometry()
}

/// Triangles on the ground at height `y`, facing up.
pub fn area(tris: &[[Vec2; 3]], y: f32) -> Geometry {
    let mut buf = MeshBuf::default();
    for t in tris {
        let mut p = t.map(|q| vec3(q.x, y, q.y));
        if (p[1] - p[0]).cross(p[2] - p[0]).y < 0.0 {
            p.swap(1, 2);
        }
        buf.triangle(p, t.map(|q| q));
    }
    buf.geometry()
}

/// Trees in the parks within `radius` meters of `center`, as transforms of a tree of 1 m.
pub fn trees(city: &Manhattan, center: Vec2, radius: f32) -> Vec<Mat4> {
    let mut out = Vec::new();
    for (i, t) in city.parks.iter().enumerate() {
        if t[0].distance(center) > radius {
            continue;
        }
        let area = (t[1] - t[0]).perp_dot(t[2] - t[0]).abs() * 0.5;
        let count = (area / 90.0) as u32 + u32::from(hash(i as u32) < (area / 90.0).fract());
        for k in 0..count {
            let seed = i as u32 * 7919 + k * 13;
            let (mut a, mut b) = (hash(seed), hash(seed + 1));
            if a + b > 1.0 {
                (a, b) = (1.0 - a, 1.0 - b);
            }
            let p = t[0] + (t[1] - t[0]) * a + (t[2] - t[0]) * b;
            let h = 8.0 + 9.0 * hash(seed + 2);
            out.push(Mat4::from_scale_rotation_translation(
                Vec3::splat(h),
                Quat::from_rotation_y(hash(seed + 3) * std::f32::consts::TAU),
                vec3(p.x, 0.0, p.y),
            ));
        }
    }
    out
}

/// The lights along the bridges: two rows on the deck, which rises to 40 m over the water.
pub fn bridge_lights(city: &Manhattan) -> Vec<Vec3> {
    let mut out = Vec::new();
    for points in &city.bridges {
        let length: f32 = points.windows(2).map(|w| w[0].distance(w[1])).sum();
        if length < 300.0 {
            continue;
        }
        let mut s = 0.0;
        for w in points.windows(2) {
            let d = w[1] - w[0];
            let l = d.length().max(1e-3);
            let side = vec2(-d.y, d.x) / l * 9.0;
            let mut k = 0.0;
            while k < l {
                let along = (s + k) / length;
                let h = 10.0 + 32.0 * (along * std::f32::consts::PI).sin();
                let p = w[0] + d * (k / l);
                for q in [p + side, p - side] {
                    out.push(vec3(q.x, h, q.y));
                }
                k += 18.0;
            }
            s += l;
        }
    }
    out
}

/// The parts of a tower crane. The mast stands on the origin. The jib points along +X at
/// the height `height`.
pub struct CraneParts {
    pub mast: Geometry,
    /// The jib, the counter jib, the cab and the tie bars, around the slewing point.
    pub top: Geometry,
    pub trolley: Geometry,
    /// A cable of 1 m that hangs down from the origin. Scale it on Y for the length.
    pub cable: Geometry,
    pub hook: Geometry,
    /// A steel beam of 12 m along X that hangs below the origin.
    pub load: Geometry,
    /// The length of the jib in meters.
    pub reach: f32,
}

impl CraneParts {
    pub fn new(height: f32) -> Self {
        let mut mast = MeshBuf::default();
        let w = 1.1;
        let chord = 0.22;
        let corners = [vec2(-w, -w), vec2(w, -w), vec2(w, w), vec2(-w, w)];
        for c in corners {
            mast.bar(vec3(c.x, 0.0, c.y), vec3(c.x, height, c.y), chord);
        }
        // Zigzag braces on each face, one section per 3 m.
        let sections = (height / 3.0) as i32;
        for s in 0..sections {
            let (y0, y1) = (s as f32 * 3.0, s as f32 * 3.0 + 3.0);
            for k in 0..4 {
                let (a, b) = (corners[k], corners[(k + 1) % 4]);
                let (p, q) = if s % 2 == 0 { (a, b) } else { (b, a) };
                mast.bar(vec3(p.x, y0, p.y), vec3(q.x, y1, q.y), 0.1);
            }
        }
        mast.block(vec3(-2.5, 0.0, -2.5), vec3(2.5, 1.2, 2.5));

        let reach = 58.0;
        let back = 18.0;
        let mut top = MeshBuf::default();
        // The slewing unit and the cab.
        top.block(vec3(-1.6, -1.0, -1.6), vec3(1.6, 0.6, 1.6));
        top.block(vec3(0.4, -2.6, 1.3), vec3(2.6, -0.2, 3.4));
        // The jib: a triangular lattice, 1.6 m wide and 1.8 m high.
        let (jw, jh) = (0.8, 1.8);
        for z in [-jw, jw] {
            top.bar(vec3(0.0, 0.6, z), vec3(reach, 0.6, z), 0.16);
        }
        top.bar(
            vec3(0.0, 0.6 + jh, 0.0),
            vec3(reach - 2.0, 0.6 + jh, 0.0),
            0.14,
        );
        let mut x = 0.0;
        while x < reach - 2.0 {
            for z in [-jw, jw] {
                top.bar(vec3(x, 0.6, z), vec3(x + 1.5, 0.6 + jh, 0.0), 0.07);
                top.bar(vec3(x + 1.5, 0.6 + jh, 0.0), vec3(x + 3.0, 0.6, z), 0.07);
            }
            x += 3.0;
        }
        // The counter jib with its concrete weights.
        top.block(vec3(-back, 0.3, -1.0), vec3(0.0, 0.9, 1.0));
        top.block(vec3(-back, 0.9, -1.2), vec3(-back + 5.0, 3.4, 1.2));
        // The apex and the tie bars to both jibs.
        let apex = vec3(0.0, 9.0, 0.0);
        top.bar(vec3(-0.8, 0.6, 0.0), apex, 0.4);
        top.bar(vec3(0.8, 0.6, 0.0), apex, 0.4);
        top.bar(apex, vec3(reach * 0.62, 0.6 + jh, 0.0), 0.08);
        top.bar(apex, vec3(-back + 1.0, 0.9, 0.0), 0.08);

        let mut trolley = MeshBuf::default();
        trolley.block(vec3(-1.0, 0.0, -1.0), vec3(1.0, 0.7, 1.0));
        let mut cable = MeshBuf::default();
        cable.block(vec3(-0.03, -1.0, -0.2), vec3(0.03, 0.0, -0.14));
        cable.block(vec3(-0.03, -1.0, 0.14), vec3(0.03, 0.0, 0.2));
        let mut hook = MeshBuf::default();
        hook.block(vec3(-0.5, -1.2, -0.4), vec3(0.5, 0.0, 0.4));
        hook.bar(vec3(0.0, -1.2, 0.0), vec3(-3.0, -3.5, 0.0), 0.05);
        hook.bar(vec3(0.0, -1.2, 0.0), vec3(3.0, -3.5, 0.0), 0.05);
        let mut load = MeshBuf::default();
        // An I beam: two flanges and a web.
        let y = -3.5;
        load.block(vec3(-6.0, y - 0.06, -0.25), vec3(6.0, y, 0.25));
        load.block(vec3(-6.0, y - 0.6, -0.25), vec3(6.0, y - 0.54, 0.25));
        load.block(vec3(-6.0, y - 0.54, -0.05), vec3(6.0, y - 0.06, 0.05));

        Self {
            mast: mast.geometry(),
            top: top.geometry(),
            trolley: trolley.geometry(),
            cable: cable.geometry(),
            hook: hook.geometry(),
            load: load.geometry(),
            reach,
        }
    }
}

/// The steel frame of a tower under construction on the lot `min`..`max`: columns, beams
/// and floor slabs, with glass on the lower floors. Returns the steel, the concrete slabs,
/// the glass walls and the work lights.
pub fn site_frame(
    min: Vec2,
    max: Vec2,
    floors: u32,
    seed: u32,
) -> (Geometry, Geometry, Geometry, Vec<Vec3>) {
    let mut steel = MeshBuf::default();
    let mut slabs = MeshBuf::default();
    let mut glass = MeshBuf::default();
    let mut lights = Vec::new();
    let size = max - min;
    let span = 4.0 * BAY;
    let nx = (size.x / span).round().max(1.0) as i32;
    let nz = (size.y / span).round().max(1.0) as i32;
    let floors = floors as i32;
    let clad = (floors as f32 * 0.55) as i32;
    let at = |i: i32, j: i32| {
        vec2(
            min.x + size.x * i as f32 / nx as f32,
            min.y + size.y * j as f32 / nz as f32,
        )
    };
    for f in 0..floors {
        let y = f as f32 * FLOOR;
        let top = y + FLOOR;
        // The columns of the top floor are not all up yet.
        for i in 0..=nx {
            for j in 0..=nz {
                let edge = i == 0 || j == 0 || i == nx || j == nz;
                if f == floors - 1 && hash(seed + (i * 31 + j) as u32) < 0.45 {
                    continue;
                }
                if edge || (i % 2 == 0 && j % 2 == 0) {
                    let p = at(i, j);
                    steel.block(
                        vec3(p.x - 0.35, y, p.y - 0.35),
                        vec3(p.x + 0.35, top, p.y + 0.35),
                    );
                }
            }
        }
        if f >= floors - 1 {
            continue;
        }
        // The beams around the floor and the slab.
        let (y0, y1) = (top - 0.6, top);
        steel.block(vec3(min.x, y0, min.y - 0.2), vec3(max.x, y1, min.y + 0.2));
        steel.block(vec3(min.x, y0, max.y - 0.2), vec3(max.x, y1, max.y + 0.2));
        steel.block(vec3(min.x - 0.2, y0, min.y), vec3(min.x + 0.2, y1, max.y));
        steel.block(vec3(max.x - 0.2, y0, min.y), vec3(max.x + 0.2, y1, max.y));
        for i in 1..nx {
            let x = at(i, 0).x;
            steel.block(vec3(x - 0.15, y0 + 0.2, min.y), vec3(x + 0.15, y1, max.y));
        }
        if f < floors - 3 {
            slabs.block(vec3(min.x, top - 0.3, min.y), vec3(max.x, top, max.y));
        }
        // Work lights on the open floors.
        if f >= clad && f % 2 == 0 {
            for k in 0..3 {
                let p = at(
                    (hash(seed + f as u32 * 5 + k) * nx as f32) as i32,
                    (hash(seed + f as u32 * 7 + k) * nz as f32) as i32,
                );
                lights.push(vec3(p.x, y + 2.8, p.y));
            }
        }
    }
    // Glass on the finished lower floors.
    let h = clad as f32 * FLOOR;
    let (a, b) = (min - Vec2::splat(0.4), max + Vec2::splat(0.4));
    let c = [
        vec2(a.x, a.y),
        vec2(b.x, a.y),
        vec2(b.x, b.y),
        vec2(a.x, b.y),
    ];
    let mut tris = Vec::new();
    for k in 0..4 {
        let (p, q) = (c[k], c[(k + 1) % 4]);
        let [p0, q0, q1, p1] = [
            vec3(q.x, 0.0, q.y),
            vec3(p.x, 0.0, p.y),
            vec3(p.x, h, p.y),
            vec3(q.x, h, q.y),
        ];
        tris.extend([[p0, q0, q1], [p0, q1, p1]]);
    }
    walls(&mut glass, &tris, seed, Rgb::WHITE);
    (steel.geometry(), slabs.geometry(), glass.geometry(), lights)
}

/// The west shore of the Hudson in New Jersey, from south to north. The model of the city
/// ends at the river, so this shore is drawn after a map.
const JERSEY: [Vec2; 10] = [
    vec2(1412.0, -6929.0),
    vec2(1814.0, -5084.0),
    vec2(2417.0, -2930.0),
    vec2(3080.0, -999.0),
    vec2(3293.0, 660.0),
    vec2(3368.0, 2014.0),
    vec2(3316.0, 4709.0),
    vec2(3514.0, 8027.0),
    vec2(3773.0, 10930.0),
    vec2(4080.0, 14442.0),
];

/// New Jersey across the river: the land, and low buildings along the shore with a few
/// towers. They are only seen far away in the haze.
pub fn new_jersey() -> (Geometry, MeshBuf) {
    let mut land = MeshBuf::default();
    let far = 12000.0;
    for w in JERSEY.windows(2) {
        let (a, b) = (w[0], w[1]);
        let corners = [
            vec3(a.x, 0.0, a.y),
            vec3(far, 0.0, a.y),
            vec3(far, 0.0, b.y),
            vec3(b.x, 0.0, b.y),
        ];
        let uvs = corners.map(|p| vec2(p.x, p.z));
        land.quad(
            [corners[0], corners[3], corners[2], corners[1]],
            [uvs[0], uvs[3], uvs[2], uvs[1]],
        );
    }
    let mut walls_buf = MeshBuf::default();
    for k in 0..900u32 {
        let t = hash(k * 3 + 1) * (JERSEY.len() - 1) as f32;
        let i = t as usize;
        let shore = JERSEY[i].lerp(JERSEY[i + 1], t.fract());
        let p = shore + vec2(40.0 + 1400.0 * hash(k * 3 + 2).powi(2), 0.0);
        let tall = hash(k * 3 + 3) < 0.05;
        let h = if tall {
            60.0 + 90.0 * hash(k + 7)
        } else {
            8.0 + 22.0 * hash(k + 7)
        };
        let size = vec2(12.0 + 30.0 * hash(k + 11), 12.0 + 30.0 * hash(k + 13));
        let (a, b) = (p - size * 0.5, p + size * 0.5);
        let c = [
            vec2(a.x, a.y),
            vec2(b.x, a.y),
            vec2(b.x, b.y),
            vec2(a.x, b.y),
        ];
        let mut tris = Vec::new();
        for j in 0..4 {
            let (p, q) = (c[j], c[(j + 1) % 4]);
            let [p0, q0, q1, p1] = [
                vec3(q.x, 0.0, q.y),
                vec3(p.x, 0.0, p.y),
                vec3(p.x, h, p.y),
                vec3(q.x, h, q.y),
            ];
            tris.extend([[p0, q0, q1], [p0, q1, p1]]);
        }
        walls(&mut walls_buf, &tris, k * 17, Rgb::WHITE);
    }
    (land.geometry(), walls_buf)
}

/// The pins of the chip and its gold seal ring, on the city around the point of the dive.
/// They show only in the first frames, while the city is still a chip.
pub fn pins(dive: Vec2, half: f32, pitch: f32, reach: f32, count: usize) -> (Geometry, Geometry) {
    let mut pins = MeshBuf::default();
    let width = 8.0 * BAY;
    for side in [-1.0f32, 1.0] {
        for k in 0..count {
            let z = dive.y + (k as f32 - (count - 1) as f32 * 0.5) * pitch;
            let x0 = dive.x + side * half;
            let x1 = dive.x + side * (half + reach);
            pins.block(
                vec3(x0.min(x1), 1.2, z - width * 0.5),
                vec3(x0.max(x1), 1.6, z + width * 0.5),
            );
        }
    }
    let mut ring = MeshBuf::default();
    let w = 18.0;
    let (min, max) = (dive - Vec2::splat(half), dive + Vec2::splat(half));
    ring.ground_rect(vec2(min.x, min.y), vec2(min.x + w, max.y), 1.0);
    ring.ground_rect(vec2(max.x - w, min.y), vec2(max.x, max.y), 1.0);
    ring.ground_rect(vec2(min.x, min.y), vec2(max.x, min.y + w), 1.0);
    ring.ground_rect(vec2(min.x, max.y - w), vec2(max.x, max.y), 1.0);
    (pins.geometry(), ring.geometry())
}
