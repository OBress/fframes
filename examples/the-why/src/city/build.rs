//! The static meshes of the city, built once from the plan.

use fframes_3d::math::hash;
use fframes_3d::{Geometry, Mat4, Quat, Rgb, Vec2, Vec3, Vec4, vec2, vec3};

use super::layout::{
    BAY, Block, Building, CANAL_WIDTH, CHIP_HALF, FLOOR, HALF, Plan, SCREEN, Site, Street, Style,
    Use, WATER_DEPTH, clip_range,
};
use super::textures::{BAYS, FLOORS, LAMP_SPACING};

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

/// The ground with a gap for the canal in the middle of the central boulevard, the water,
/// and the walls of the canal.
pub fn ground() -> (Geometry, Geometry, Geometry) {
    let half = CANAL_WIDTH * 0.5;
    let mut ground = MeshBuf::default();
    let far = HALF + 3000.0;
    ground.ground_rect(vec2(-far, -far), vec2(-half, far), 0.0);
    ground.ground_rect(vec2(half, -far), vec2(far, far), 0.0);
    let mut water = MeshBuf::default();
    water.ground_rect(vec2(-half, -far), vec2(half, far), -WATER_DEPTH);
    let mut walls = MeshBuf::default();
    for (x, facing) in [(-half, 1.0f32), (half, -1.0)] {
        let a = vec3(x, -WATER_DEPTH - 1.0, far * facing);
        let b = vec3(x, -WATER_DEPTH - 1.0, -far * facing);
        let up = Vec3::Y * (WATER_DEPTH + 1.0);
        let uvs = [a, b, b + up, a + up].map(|p| vec2(p.z, p.y));
        walls.quad([a, b, b + up, a + up], uvs);
    }
    (ground.geometry(), water.geometry(), walls.geometry())
}

/// The strips of a street: one, or two beside its median.
fn carriageways(s: &Street) -> Vec<(Vec2, Vec2, f32)> {
    if s.median <= 0.0 {
        return vec![(s.from, s.to, s.width)];
    }
    let d = s.direction();
    let side = vec2(-d.y, d.x) * (s.width + s.median) * 0.25;
    let width = (s.width - s.median) * 0.5;
    vec![
        (s.from + side, s.to + side, width),
        (s.from - side, s.to - side, width),
    ]
}

/// The street surfaces. Arterials lie a little higher, so they cover the small streets at
/// the crossings.
pub fn streets(plan: &Plan) -> Geometry {
    let mut buf = MeshBuf::default();
    for (i, s) in plan.streets.iter().enumerate() {
        let y = if s.arterial {
            0.06 + (i % 16) as f32 * 0.003
        } else {
            0.04
        };
        for (from, to, width) in carriageways(s) {
            buf.strip(from, to, width, y, None);
        }
    }
    buf.geometry()
}

/// The traces of the chip inside the chip square: a narrow strip along the middle of each
/// street and a square pad at each crossing. Returns the gold traces (the arterials) and
/// the copper traces, scaled by `scale` and at the height `y`.
pub fn traces(plan: &Plan, scale: f32, y: f32) -> [MeshBuf; 2] {
    let mut out = [MeshBuf::default(), MeshBuf::default()];
    let (min, max) = (Vec2::splat(-CHIP_HALF), Vec2::splat(CHIP_HALF));
    for s in &plan.streets {
        let Some((t0, t1)) = clip_range(s.from, s.to, min, max) else {
            continue;
        };
        let (a, b) = (s.from + (s.to - s.from) * t0, s.from + (s.to - s.from) * t1);
        let k = usize::from(!s.arterial);
        let lift = if s.arterial { 0.01 } else { 0.0 };
        out[k].strip(
            a * scale,
            b * scale,
            trace_width(s.width) * scale,
            y + lift * scale,
            None,
        );
    }
    for i in &plan.intersections {
        if i.at.abs().max_element() > CHIP_HALF - 10.0 {
            continue;
        }
        let half = Vec2::splat(trace_width(i.widths.max_element()) * 0.85);
        let k = usize::from(!i.arterial);
        out[k].ground_rect(
            (i.at - half) * scale,
            (i.at + half) * scale,
            y + 0.02 * scale,
        );
    }
    out
}

/// The width of the trace of a street of `width` meters.
fn trace_width(width: f32) -> f32 {
    width * 0.42
}

/// The light pools of the street lamps, one strip over each street (see
/// [`super::textures::lamp_pools`]).
pub fn lamp_light(plan: &Plan) -> Geometry {
    let mut buf = MeshBuf::default();
    for s in &plan.streets {
        for (from, to, width) in carriageways(s) {
            buf.strip(from, to, width + 6.0, 0.3, Some(LAMP_SPACING));
        }
    }
    buf.geometry()
}

/// The lamps along all streets in the inner city: the transforms of the poles and the
/// positions of the lamp heads. The pole geometry stands at the origin with its arm along
/// +X.
pub fn lamps(plan: &Plan) -> (Vec<Mat4>, Vec<Vec3>) {
    let mut poles = Vec::new();
    let mut heads = Vec::new();
    let limit = 1900.0;
    for s in &plan.streets {
        let dir = s.direction();
        let side = vec2(-dir.y, dir.x);
        let length = s.from.distance(s.to);
        for (k, sign) in [(0.0, 1.0f32), (0.5, -1.0)] {
            let mut along = k * LAMP_SPACING;
            while along < length {
                let base = s.from + dir * along + side * sign * (s.width * 0.5 + 1.2);
                along += LAMP_SPACING;
                if base.abs().max_element() > limit || in_crossing(plan, base, s) {
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
    (poles, heads)
}

/// Whether `p` lies on another street than `s`.
fn in_crossing(plan: &Plan, p: Vec2, s: &Street) -> bool {
    let d = s.direction();
    plan.streets
        .iter()
        .any(|o| o.direction().dot(d).abs() < 0.9 && o.distance(p) < o.width * 0.5 + 3.0)
}

/// A street lamp: a pole of 9 m with an arm along +X.
pub fn lamp_pole() -> Geometry {
    let mut buf = MeshBuf::default();
    buf.block(vec3(-0.12, 0.0, -0.12), vec3(0.12, 9.0, 0.12));
    buf.block(vec3(0.0, 8.75, -0.07), vec3(2.4, 8.9, 0.07));
    buf.block(vec3(1.9, 8.55, -0.18), vec3(2.6, 8.75, 0.18));
    buf.geometry()
}

/// The painted lines of the arterials and the zebra crossings, in the inner city.
pub fn markings(plan: &Plan) -> Geometry {
    let mut buf = MeshBuf::default();
    let limit = 1500.0;
    let clip = |p: Vec2| p.clamp(Vec2::splat(-limit), Vec2::splat(limit));
    for s in &plan.streets {
        let (from, to) = (clip(s.from), clip(s.to));
        if from.distance(to) < 1.0 {
            continue;
        }
        let dir = (to - from).normalize();
        let side = vec2(-dir.y, dir.x);
        let y = 0.15;
        // A double line in the middle.
        if s.median <= 0.0 {
            for o in [-0.25, 0.25] {
                buf.strip(from + side * o, to + side * o, 0.16, y, Some(1.0));
            }
        }
        if !s.arterial {
            continue;
        }
        // Dashed lines between the lanes.
        let lanes = ((s.width * 0.5 - s.median * 0.5 - 1.0) / 3.4).floor() as i32;
        let length = from.distance(to);
        for lane in 1..lanes {
            for sign in [-1.0, 1.0] {
                let offset = side * sign * (s.median * 0.5 + lane as f32 * 3.4 + 0.5);
                let mut along = 0.0;
                while along + 3.0 < length {
                    let a = from + dir * along + offset;
                    if !in_crossing(plan, a, s) {
                        buf.strip(a, a + dir * 3.0, 0.15, y, Some(1.0));
                    }
                    along += 9.0;
                }
            }
        }
    }
    // Zebra crossings on the four sides of each crossing near the flight.
    for i in &plan.intersections {
        if i.at.distance(vec2(0.0, 500.0)) > 1100.0 {
            continue;
        }
        let half = i.widths * 0.5;
        for (axis, width, depth) in [(Vec2::X, half.y, half.x), (Vec2::Y, half.x, half.y)] {
            let across = vec2(axis.y, axis.x);
            for sign in [-1.0, 1.0] {
                // The crossing lies across the street that runs along `across`.
                let center = i.at + across * sign * (width + 3.5);
                let mut o = -depth + 1.0;
                while o < depth - 1.0 {
                    let a = center + axis * o - across * 2.0;
                    buf.strip(a, a + across * 4.0, 0.55, 0.16, Some(1.0));
                    o += 1.2;
                }
            }
        }
    }
    buf.geometry()
}

/// The bridges of the streets over the canal, the railings of the canal between them, and
/// the small lamps on the railings.
pub fn bridges(plan: &Plan) -> (Geometry, Vec<Vec3>) {
    let mut buf = MeshBuf::default();
    let mut lights = Vec::new();
    let x = CANAL_WIDTH * 0.5 + 0.5;
    // The streets that cross the canal, as Z ranges.
    let mut spans: Vec<(f32, f32)> = plan
        .streets
        .iter()
        .filter(|s| s.direction().x.abs() > 0.99 && s.from.x < -x && s.to.x > x)
        .map(|s| {
            (
                s.from.y - s.width * 0.5 - 1.0,
                s.from.y + s.width * 0.5 + 1.0,
            )
        })
        .collect();
    spans.sort_by(|a, b| a.0.total_cmp(&b.0));
    for &(z0, z1) in &spans {
        buf.block(vec3(-x, -1.4, z0), vec3(x, 0.03, z1));
        for z in [z0 + 0.15, z1 - 0.15] {
            buf.block(vec3(-x, 0.0, z - 0.15), vec3(x, 1.1, z + 0.15));
        }
    }
    // The railings between the bridges, with a lamp every 14 m.
    for pair in spans.windows(2) {
        let (z0, z1) = (pair[0].1, pair[1].0);
        for side in [-1.0, 1.0] {
            buf.block(
                vec3(side * x - 0.15, 0.0, z0),
                vec3(side * x + 0.15, 1.1, z1),
            );
            if z0.abs() < 1600.0 {
                let mut z = z0 + 7.0;
                while z < z1 - 3.0 {
                    lights.push(vec3(side * x, 1.3, z));
                    z += 14.0;
                }
            }
        }
    }
    (buf.geometry(), lights)
}

/// The facades, roofs and roof details of the buildings that rise together.
#[derive(Default)]
pub struct RiseGroup {
    /// The facade meshes, in the order of [`STYLES`].
    pub walls: [MeshBuf; 4],
    /// The roofs, in the order of [`RoofKind`].
    pub roofs: [MeshBuf; 3],
    pub details: MeshBuf,
    /// The quads of the video screens.
    pub screens: Vec<MeshBuf>,
    /// The light bands on the crowns of some towers.
    pub crowns: MeshBuf,
    /// The small lights on the roofs, at full height: red aviation lights and the lights
    /// around the helipads.
    pub lights: Vec<RoofLight>,
    /// The second at which the buildings start to rise.
    pub start: f32,
}

/// The look of a roof on the chip.
#[derive(Debug, Clone, Copy)]
pub enum RoofKind {
    /// Inside the chip square: a component.
    Component = 0,
    /// Outside the chip: black at first.
    Outside = 1,
    /// A pin of the chip: silver.
    Pin = 2,
}

#[derive(Debug, Clone, Copy)]
pub struct RoofLight {
    pub at: Vec3,
    /// Red and blinking, or a steady helipad light.
    pub aviation: bool,
}

pub const STYLES: [Style; 4] = [Style::Glass, Style::Fins, Style::Office, Style::Low];
/// The number of rise groups.
pub const GROUPS: usize = 12;
/// The seconds that one building takes to rise.
pub const RISE_TIME: f32 = 1.2;
/// The first and the last start of a rise, in seconds.
const RISE_FIRST: f32 = 0.55;
const RISE_LAST: f32 = 3.3;

/// The second at which a building at `p` starts to rise. The buildings below the dive rise
/// first, and the wave runs outward.
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

/// Sorts the buildings into rise groups and builds their meshes.
pub fn rise_groups(plan: &Plan) -> Vec<RiseGroup> {
    let mut groups: Vec<RiseGroup> = (0..GROUPS)
        .map(|g| RiseGroup {
            start: group_start(g),
            ..Default::default()
        })
        .collect();
    for b in &plan.buildings {
        let g = &mut groups[group_of(rise_start(b.center, b.seed))];
        building(g, b);
    }
    for screen in &plan.screens {
        let Some(b) = plan.buildings.get(screen.building) else {
            continue;
        };
        let g = &mut groups[group_of(rise_start(b.center, b.seed))];
        let right = vec2(-screen.facing.y, screen.facing.x) * -1.0;
        let half = SCREEN * 0.5;
        let p = |side: f32, up: f32| {
            let q = screen.at + right * side;
            vec3(q.x, screen.height + up, q.y)
        };
        let mut quad = MeshBuf::default();
        quad.quad(
            [
                p(-half.x, -half.y),
                p(half.x, -half.y),
                p(half.x, half.y),
                p(-half.x, half.y),
            ],
            UNIT_UV,
        );
        g.screens.push(quad);
        // A dark frame between the screen and the wall.
        let corner = |side: f32, up: f32, depth: f32| {
            let q = screen.at - screen.facing * depth + right * side;
            vec3(q.x, screen.height + up, q.y)
        };
        let a = corner(-half.x - 0.8, -half.y - 0.8, 0.9);
        let b = corner(half.x + 0.8, half.y + 0.8, 0.1);
        g.details.block(a.min(b), a.max(b));
    }
    groups
}

/// The tints of the facades: the glass and the frames of each building differ a little.
const TINTS: [u32; 6] = [0xffffff, 0xc8d4e6, 0xb4ccd2, 0xd2d2d2, 0x9aa4b0, 0xc0d6c8];

/// Whether a building is near enough to the flight for small details.
fn near(p: Vec2) -> bool {
    p.distance(vec2(0.0, 450.0)) < 1300.0
}

/// Adds the walls, the roofs and the roof details of one building.
fn building(group: &mut RiseGroup, b: &Building) {
    let style = STYLES.iter().position(|s| *s == b.style).unwrap_or(0);
    let tint = Rgb::hex(TINTS[(hash(b.seed + 50) * TINTS.len() as f32) as usize % TINTS.len()]);
    let mut bottom = 0.0;
    for (i, tier) in b.tiers.iter().enumerate() {
        let half = tier.size * 0.5;
        let min = b.center - half;
        let max = b.center + half;
        let first = group.walls[style].len();
        walls(
            &mut group.walls[style],
            min,
            max,
            bottom,
            tier.top,
            b.seed + i as u32 * 13,
        );
        group.walls[style].paint(first, tint);
        let roof = [
            vec3(min.x, tier.top, max.y),
            vec3(max.x, tier.top, max.y),
            vec3(max.x, tier.top, min.y),
            vec3(min.x, tier.top, min.y),
        ];
        let kind = if b.pin {
            RoofKind::Pin
        } else if b.center.abs().max_element() < CHIP_HALF {
            RoofKind::Component
        } else {
            RoofKind::Outside
        };
        group.roofs[kind as usize].quad(roof, UNIT_UV);
        if near(b.center) {
            parapet(&mut group.details, min, max, tier.top);
        }
        bottom = tier.top;
    }
    let Some(top) = b.tiers.last() else { return };
    roof_details(group, b, top.size, top.top);
}

/// A low wall around the edge of a roof.
fn parapet(buf: &mut MeshBuf, min: Vec2, max: Vec2, y: f32) {
    let (t, h) = (0.35, 1.1);
    buf.block(vec3(min.x, y, min.y), vec3(max.x, y + h, min.y + t));
    buf.block(vec3(min.x, y, max.y - t), vec3(max.x, y + h, max.y));
    buf.block(vec3(min.x, y, min.y + t), vec3(min.x + t, y + h, max.y - t));
    buf.block(vec3(max.x - t, y, min.y + t), vec3(max.x, y + h, max.y - t));
}

/// The four walls of a box from `min` to `max` on the ground and from `y0` to `y1` in
/// height. The UVs count window bays and floors, so the windows keep their size on every
/// wall. Each wall starts at a random place in the texture.
fn walls(buf: &mut MeshBuf, min: Vec2, max: Vec2, y0: f32, y1: f32, seed: u32) {
    let corners = [
        vec2(min.x, min.y),
        vec2(max.x, min.y),
        vec2(max.x, max.y),
        vec2(min.x, max.y),
    ];
    for k in 0..4 {
        // Seen from outside, `b` is on the left and `a` on the right.
        let (a, b) = (corners[k], corners[(k + 1) % 4]);
        let width = a.distance(b);
        let u0 = (hash(seed + k as u32 * 3) * BAYS).floor() / BAYS;
        let v0 = (hash(seed + k as u32 * 3 + 1) * FLOORS).floor() / FLOORS;
        let u1 = u0 + width / (BAY * BAYS);
        let v = |y: f32| v0 - y / (FLOOR * FLOORS);
        let p = |q: Vec2, y: f32| vec3(q.x, y, q.y);
        buf.quad(
            [p(b, y0), p(a, y0), p(a, y1), p(b, y1)],
            [
                vec2(u0, v(y0)),
                vec2(u1, v(y0)),
                vec2(u1, v(y1)),
                vec2(u0, v(y1)),
            ],
        );
    }
}

/// Plant rooms, cooling units, antennas, helipads, light bands on the crowns and red
/// aviation lights on the top roof.
fn roof_details(group: &mut RiseGroup, b: &Building, size: Vec2, top: f32) {
    let s = b.seed;
    let half = size * 0.5;
    let c = b.center;
    let height = b.height();
    // The tall towers carry red lights at two corners, and some a band of light.
    if height > 110.0 {
        let corner = |sx: f32, sz: f32| {
            vec3(
                c.x + sx * (half.x - 0.6),
                top + 1.6,
                c.y + sz * (half.y - 0.6),
            )
        };
        for at in [corner(-1.0, -1.0), corner(1.0, 1.0)] {
            group.lights.push(RoofLight { at, aviation: true });
        }
        if hash(s + 60) < 0.45 {
            let crown = &mut group.crowns;
            let first = crown.len();
            let (min, max) = (c - half - Vec2::splat(0.15), c + half + Vec2::splat(0.15));
            let (y0, y1) = (top - 2.2, top - 1.5);
            crown.block(vec3(min.x, y0, min.y), vec3(max.x, y1, min.y + 0.2));
            crown.block(vec3(min.x, y0, max.y - 0.2), vec3(max.x, y1, max.y));
            crown.block(vec3(min.x, y0, min.y), vec3(min.x + 0.2, y1, max.y));
            crown.block(vec3(max.x - 0.2, y0, min.y), vec3(max.x, y1, max.y));
            // White light bands, and some in cyan.
            let color = if hash(s + 61) < 0.35 {
                0x5fe6ff
            } else {
                0xcfe4ff
            };
            crown.paint(first, Rgb::hex(color));
        }
    }
    if !near(c) {
        return;
    }
    // A helipad on some towers, with green lights around it.
    let helipad = height > 130.0 && size.min_element() >= 30.0 && hash(s + 70) < 0.35;
    if helipad {
        let r = size.min_element() * 0.36;
        group.details.block(
            vec3(c.x - r, top, c.y - r),
            vec3(c.x + r, top + 1.4, c.y + r),
        );
        for k in 0..12 {
            let a = k as f32 / 12.0 * std::f32::consts::TAU;
            let at = vec3(
                c.x + a.cos() * r * 0.92,
                top + 1.6,
                c.y + a.sin() * r * 0.92,
            );
            group.lights.push(RoofLight {
                at,
                aviation: false,
            });
        }
    } else if size.min_element() > 14.0 && hash(s + 21) < 0.75 {
        // A plant room.
        let room = (size * (0.3 + 0.25 * hash(s + 22))).max(Vec2::splat(6.0));
        let h = 3.5 + 3.0 * hash(s + 23);
        let offset = (vec2(hash(s + 24), hash(s + 25)) - 0.5) * (size - room) * 0.6;
        let min = c + offset - room * 0.5;
        group.details.block(
            vec3(min.x, top, min.y),
            vec3(min.x + room.x, top + h, min.y + room.y),
        );
    }
    // Cooling units.
    let units = (size.x * size.y / 220.0).min(12.0) as u32;
    for u in 0..units {
        let p = c
            + (vec2(hash(s + 30 + u * 2), hash(s + 31 + u * 2)) - 0.5) * (size - Vec2::splat(6.0));
        if helipad && (p - c).abs().max_element() < size.min_element() * 0.38 {
            continue;
        }
        group.details.block(
            vec3(p.x - 1.2, top, p.y - 0.9),
            vec3(p.x + 1.2, top + 1.6, p.y + 0.9),
        );
    }
    // An antenna mast on some towers.
    if height > 120.0 && !helipad && hash(s + 40) < 0.6 {
        let mast = 12.0 + 30.0 * hash(s + 41);
        let base = vec3(c.x, top, c.y);
        group
            .details
            .block(base - vec3(0.6, 0.0, 0.6), base + vec3(0.6, 4.0, 0.6));
        group.details.bar(base, base + Vec3::Y * mast, 0.4);
        group.lights.push(RoofLight {
            at: base + Vec3::Y * (mast + 0.3),
            aviation: true,
        });
    }
}

/// The trees of the parks, as transforms of a tree of 1 m height.
pub fn trees(plan: &Plan) -> Vec<Mat4> {
    let mut out = Vec::new();
    for (bi, block) in plan.blocks.iter().enumerate() {
        if block.use_ != Use::Park {
            continue;
        }
        let size = block.max - block.min;
        let count = (size.x * size.y / 70.0) as u32;
        for i in 0..count {
            let seed = bi as u32 * 1543 + i * 7;
            let p = block.min + vec2(hash(seed), hash(seed + 1)) * size;
            // Paths cross the park: no trees on them.
            let local = (p - block.min) / size;
            if (local.x - 0.5).abs() < 0.04
                || (local.y - 0.5).abs() < 0.05
                || (local.x - local.y).abs() < 0.04
            {
                continue;
            }
            let h = 7.0 + 8.0 * hash(seed + 2);
            out.push(Mat4::from_scale_rotation_translation(
                Vec3::splat(h),
                Quat::from_rotation_y(hash(seed + 3) * std::f32::consts::TAU),
                vec3(p.x, 0.0, p.y),
            ));
        }
    }
    out
}

/// The paths of the parks with their lamps.
pub fn park_paths(plan: &Plan) -> (Geometry, Vec<Vec3>) {
    let mut buf = MeshBuf::default();
    let mut lights = Vec::new();
    for block in plan.blocks.iter().filter(|b| b.use_ == Use::Park) {
        let Block { min, max, .. } = *block;
        let mid = (min + max) * 0.5;
        let paths = [
            (vec2(mid.x, min.y), vec2(mid.x, max.y)),
            (vec2(min.x, mid.y), vec2(max.x, mid.y)),
            (min, max),
        ];
        for (a, b) in paths {
            buf.strip(a, b, 4.0, 0.08, None);
            let length = a.distance(b);
            let dir = (b - a) / length;
            let mut along = 6.0;
            while along < length {
                let p = a + dir * along + vec2(-dir.y, dir.x) * 2.6;
                lights.push(vec3(p.x, 3.5, p.y));
                along += 18.0;
            }
        }
    }
    (buf.geometry(), lights)
}

/// The steel frame of a tower under construction: columns, beams and floor slabs, and the
/// glass on the lower floors. Returns the steel, the concrete slabs, the glass walls and
/// the work lights.
pub fn site_frame(site: &Site) -> (Geometry, Geometry, Geometry, Vec<Vec3>) {
    let mut steel = MeshBuf::default();
    let mut slabs = MeshBuf::default();
    let mut glass = MeshBuf::default();
    let mut lights = Vec::new();
    let half = site.size * 0.5;
    let min = site.center - half;
    let max = site.center + half;
    let span = 4.0 * BAY;
    let nx = (site.size.x / span).round() as i32;
    let nz = (site.size.y / span).round() as i32;
    let floors = site.floors as i32;
    let clad = (floors as f32 * 0.55) as i32;
    let at = |i: i32, j: i32| {
        vec2(
            min.x + site.size.x * i as f32 / nx as f32,
            min.y + site.size.y * j as f32 / nz as f32,
        )
    };
    for f in 0..floors {
        let y = f as f32 * FLOOR;
        let top = y + FLOOR;
        // The columns of the top floor are not all up yet.
        for i in 0..=nx {
            for j in 0..=nz {
                let edge = i == 0 || j == 0 || i == nx || j == nz;
                if f == floors - 1 && hash(site.seed + (i * 31 + j) as u32) < 0.45 {
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
                    (hash(site.seed + f as u32 * 5 + k) * nx as f32) as i32,
                    (hash(site.seed + f as u32 * 7 + k) * nz as f32) as i32,
                );
                lights.push(vec3(p.x, y + 2.8, p.y));
            }
        }
    }
    // Glass on the finished lower floors.
    let h = clad as f32 * FLOOR;
    walls(
        &mut glass,
        min - Vec2::splat(0.4),
        max + Vec2::splat(0.4),
        0.0,
        h,
        site.seed,
    );
    (steel.geometry(), slabs.geometry(), glass.geometry(), lights)
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

/// The poles and housings of the traffic lights at the empty crossings near the flight,
/// and at the key crossing of the boulevard.
pub fn signal_poles(plan: &Plan) -> (Geometry, Vec<Signal>) {
    let mut buf = MeshBuf::default();
    let mut signals = Vec::new();
    for (k, i) in plan.intersections.iter().enumerate() {
        let key = i.at.distance(plan.key_crossing) < 1.0;
        if !key && (i.arterial || i.at.distance(vec2(0.0, 500.0)) > 1300.0) {
            continue;
        }
        let half = i.widths * 0.5 + Vec2::splat(1.5);
        for (n, corner) in [
            vec2(-1.0, -1.0),
            vec2(1.0, -1.0),
            vec2(1.0, 1.0),
            vec2(-1.0, 1.0),
        ]
        .into_iter()
        .enumerate()
        {
            let p = i.at + corner * half;
            let arm = if key { 7.0 } else { 0.0 };
            buf.block(
                vec3(p.x - 0.14, 0.0, p.y - 0.14),
                vec3(p.x + 0.14, 6.5, p.y + 0.14),
            );
            // A mast arm over the street at the key crossing.
            let tip = p - corner * vec2(arm, 0.0);
            buf.block(
                vec3(p.x.min(tip.x) - 0.1, 6.2, p.y - 0.1),
                vec3(p.x.max(tip.x) + 0.1, 6.4, p.y + 0.1),
            );
            buf.block(
                vec3(tip.x - 0.35, 5.2, tip.y - 0.35),
                vec3(tip.x + 0.35, 6.6, tip.y + 0.35),
            );
            // Corners 0 and 2 face the traffic on the avenue, 1 and 3 on the street.
            signals.push(Signal {
                head: vec3(tip.x, 5.9, tip.y),
                pool: i.at + corner * (half - Vec2::splat(5.0)),
                avenue: n % 2 == 0,
                phase: if key {
                    super::traffic::key_phase()
                } else {
                    hash(k as u32 * 17 + 5) * 20.0
                },
                key,
            });
        }
    }
    (buf.geometry(), signals)
}

/// One traffic light.
#[derive(Debug, Clone, Copy)]
pub struct Signal {
    /// The lit lens.
    pub head: Vec3,
    /// The center of its glow on the street.
    pub pool: Vec2,
    /// Whether it controls the avenue (along Z) or the street (along X).
    pub avenue: bool,
    /// The offset of its cycle in seconds.
    pub phase: f32,
    /// Whether it stands at the key crossing of the boulevard.
    pub key: bool,
}
