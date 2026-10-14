//! Triangle meshes: extracted from signed distance models (surface nets with the vertices
//! projected onto the true surface), skinned to the skeleton, or built every frame from the
//! simulation (hair locks, ties, robes).

use std::collections::HashMap;

use glam::{Mat3, Vec3};

use crate::rig::Posed;
use crate::sdf::Model;

/// A point on a mesh edge: `lerp(v[a], v[b], t)` with `a < b`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EdgePoint {
    pub a: u32,
    pub b: u32,
    pub t: f32,
}

impl EdgePoint {
    pub fn new(a: u32, b: u32, t: f32) -> Self {
        if a < b {
            EdgePoint { a, b, t }
        } else {
            EdgePoint {
                a: b,
                b: a,
                t: 1.0 - t,
            }
        }
    }

    fn key(&self) -> (u32, u32) {
        (self.a, self.b)
    }

    pub fn eval(&self, v: &[Vec3]) -> Vec3 {
        v[self.a as usize].lerp(v[self.b as usize], self.t)
    }
}

/// A skinned mesh in the rest pose.
#[derive(Debug, Clone, Default)]
pub struct Mesh {
    pub rest: Vec<Vec3>,
    pub normal: Vec<Vec3>,
    pub tris: Vec<[u32; 3]>,
    /// Bone index and weight, up to four per vertex.
    pub skin: Vec<[(u8, f32); 4]>,
    /// The nearest material groups and their distances.
    pub near: Vec<[(u8, f32); 3]>,
    /// Curves where two material groups meet.
    pub borders: Vec<Border>,
}

/// A curve where two material groups meet.
#[derive(Debug, Clone)]
pub struct Border {
    pub pts: Vec<EdgePoint>,
    pub groups: (u8, u8),
}

/// Coarse block size of the surface nets pass (cells).
const S: usize = 4;

/// The 12 edges of a cell as pairs of corner indices (bit 0 = x, bit 1 = y, bit 2 = z).
const EDGES: [(usize, usize); 12] = [
    (0, 1),
    (2, 3),
    (4, 5),
    (6, 7),
    (0, 2),
    (1, 3),
    (4, 6),
    (5, 7),
    (0, 4),
    (1, 5),
    (2, 6),
    (3, 7),
];

/// Extracts the surface of `model` inside the box `min..max` with cells of `cell` metres.
/// Only quads whose centre passes `keep` are emitted (to split a model into parts meshed at
/// different resolutions). `spread` is the skinning blend distance.
pub fn surface_nets(
    model: &Model,
    min: Vec3,
    max: Vec3,
    cell: f32,
    spread: f32,
    keep: &dyn Fn(Vec3) -> bool,
) -> Mesh {
    let n = ((max - min) / cell).ceil().as_uvec3() + glam::UVec3::ONE;
    let (nx, ny, nz) = (n.x as usize, n.y as usize, n.z as usize);
    let idx = |i: usize, j: usize, k: usize| (k * ny + j) * nx + i;
    let corner =
        |i: usize, j: usize, k: usize| min + Vec3::new(i as f32, j as f32, k as f32) * cell;

    // Coarse pass: blocks far from the surface keep a sign only.
    let mut values = vec![f32::NAN; nx * ny * nz];
    let bx = (nx - 1).div_ceil(S);
    let by = (ny - 1).div_ceil(S);
    let bz = (nz - 1).div_ceil(S);
    let diag = S as f32 * cell * 3_f32.sqrt() * 1.5;
    for bk in 0..bz {
        for bj in 0..by {
            for bi in 0..bx {
                let (i0, j0, k0) = (bi * S, bj * S, bk * S);
                let (i1, j1, k1) = (
                    (i0 + S).min(nx - 1),
                    (j0 + S).min(ny - 1),
                    (k0 + S).min(nz - 1),
                );
                let center = (corner(i0, j0, k0) + corner(i1, j1, k1)) * 0.5;
                let dc = model.distance(center);
                if dc.abs() > diag {
                    let v = dc.signum() * diag;
                    for k in k0..=k1 {
                        for j in j0..=j1 {
                            for i in i0..=i1 {
                                let id = idx(i, j, k);
                                if values[id].is_nan() {
                                    values[id] = v;
                                }
                            }
                        }
                    }
                } else {
                    for k in k0..=k1 {
                        for j in j0..=j1 {
                            for i in i0..=i1 {
                                let id = idx(i, j, k);
                                // a far block may have filled the shared face: refine it
                                if values[id].is_nan() || values[id].abs() >= diag {
                                    values[id] = model.distance(corner(i, j, k));
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // One vertex per cell the surface passes through.
    let cells = (nx - 1) * (ny - 1) * (nz - 1);
    let cidx = |i: usize, j: usize, k: usize| (k * (ny - 1) + j) * (nx - 1) + i;
    let mut cell_vertex = vec![u32::MAX; cells];
    let mut verts: Vec<Vec3> = Vec::new();
    for k in 0..nz - 1 {
        for j in 0..ny - 1 {
            for i in 0..nx - 1 {
                let mut v = [0.0_f32; 8];
                let mut inside = 0;
                for (c, val) in v.iter_mut().enumerate() {
                    *val = values[idx(i + (c & 1), j + ((c >> 1) & 1), k + ((c >> 2) & 1))];
                    if *val < 0.0 {
                        inside += 1;
                    }
                }
                if inside == 0 || inside == 8 {
                    continue;
                }
                let mut sum = Vec3::ZERO;
                let mut count = 0.0;
                for (a, b) in EDGES {
                    if (v[a] < 0.0) != (v[b] < 0.0) {
                        let t = v[a] / (v[a] - v[b]);
                        let pa =
                            Vec3::new((a & 1) as f32, ((a >> 1) & 1) as f32, ((a >> 2) & 1) as f32);
                        let pb =
                            Vec3::new((b & 1) as f32, ((b >> 1) & 1) as f32, ((b >> 2) & 1) as f32);
                        sum += pa.lerp(pb, t);
                        count += 1.0;
                    }
                }
                cell_vertex[cidx(i, j, k)] = verts.len() as u32;
                verts.push(corner(i, j, k) + sum / count * cell);
            }
        }
    }

    // A quad around every grid edge with a sign change.
    let mut quads: Vec<[u32; 4]> = Vec::new();
    for k in 0..nz {
        for j in 0..ny {
            for i in 0..nx {
                let v0 = values[idx(i, j, k)];
                let mut emit = |cs: [(usize, usize, usize); 4], mid: Vec3| {
                    if !keep(mid) {
                        return;
                    }
                    let q = cs.map(|(a, b, c)| cell_vertex[cidx(a, b, c)]);
                    if q.iter().all(|v| *v != u32::MAX) {
                        quads.push(q);
                    }
                };
                if i + 1 < nx && j > 0 && k > 0 && j < ny - 1 && k < nz - 1 {
                    let v1 = values[idx(i + 1, j, k)];
                    if (v0 < 0.0) != (v1 < 0.0) {
                        emit(
                            [(i, j - 1, k - 1), (i, j, k - 1), (i, j, k), (i, j - 1, k)],
                            corner(i, j, k) + Vec3::X * cell * 0.5,
                        );
                    }
                }
                if j + 1 < ny && i > 0 && k > 0 && i < nx - 1 && k < nz - 1 {
                    let v1 = values[idx(i, j + 1, k)];
                    if (v0 < 0.0) != (v1 < 0.0) {
                        emit(
                            [(i - 1, j, k - 1), (i, j, k - 1), (i, j, k), (i - 1, j, k)],
                            corner(i, j, k) + Vec3::Y * cell * 0.5,
                        );
                    }
                }
                if k + 1 < nz && i > 0 && j > 0 && i < nx - 1 && j < ny - 1 {
                    let v1 = values[idx(i, j, k + 1)];
                    if (v0 < 0.0) != (v1 < 0.0) {
                        emit(
                            [(i - 1, j - 1, k), (i, j - 1, k), (i, j, k), (i - 1, j, k)],
                            corner(i, j, k) + Vec3::Z * cell * 0.5,
                        );
                    }
                }
            }
        }
    }

    // Keep the used vertices, projected onto the surface.
    let mut remap = vec![u32::MAX; verts.len()];
    let mut rest = Vec::new();
    for q in &quads {
        for &v in q {
            if remap[v as usize] == u32::MAX {
                remap[v as usize] = rest.len() as u32;
                rest.push(verts[v as usize]);
            }
        }
    }
    let eps = cell * 0.25;
    let mut normal = Vec::with_capacity(rest.len());
    for p in &mut rest {
        for _ in 0..3 {
            let d = model.distance(*p);
            let g = model.gradient(*p, eps);
            let g2 = g.length_squared().max(1e-8);
            let step = (g * (d / g2)).clamp_length_max(cell);
            *p -= step;
        }
        normal.push(model.gradient(*p, eps).normalize_or(Vec3::Y));
    }

    // Triangles, split along the shorter diagonal and wound outwards.
    let mut tris = Vec::with_capacity(quads.len() * 2);
    for q in &quads {
        let q = q.map(|v| remap[v as usize]);
        let p = q.map(|v| rest[v as usize]);
        let pair = if (p[0] - p[2]).length_squared() < (p[1] - p[3]).length_squared() {
            [[q[0], q[1], q[2]], [q[0], q[2], q[3]]]
        } else {
            [[q[0], q[1], q[3]], [q[1], q[2], q[3]]]
        };
        for t in pair {
            let [a, b, c] = t.map(|v| rest[v as usize]);
            let face = (b - a).cross(c - a);
            let n = normal[t[0] as usize] + normal[t[1] as usize] + normal[t[2] as usize];
            tris.push(if face.dot(n) >= 0.0 {
                t
            } else {
                [t[0], t[2], t[1]]
            });
        }
    }

    let near: Vec<[(u8, f32); 3]> = rest
        .iter()
        .map(|p| model.groups(*p).nearest::<3>())
        .collect();
    let skin = rest
        .iter()
        .map(|p| model.bone_weights(*p, spread))
        .collect();
    let borders = group_borders(&tris, &near);
    Mesh {
        rest,
        normal,
        tris,
        skin,
        near,
        borders,
    }
}

fn group_distance(near: &[(u8, f32); 3], g: u8) -> f32 {
    near.iter()
        .find(|(ng, _)| *ng == g)
        .map_or(near[2].1.max(near[0].1 + 0.01), |(_, d)| *d)
}

/// Curves where the nearest material group changes, where the distances of the two groups are
/// equal.
fn group_borders(tris: &[[u32; 3]], near: &[[(u8, f32); 3]]) -> Vec<Border> {
    let mut by_pair: std::collections::BTreeMap<(u8, u8), Vec<[EdgePoint; 2]>> =
        std::collections::BTreeMap::default();
    for t in tris {
        let g = t.map(|v| near[v as usize][0].0);
        if g[0] == g[1] && g[1] == g[2] {
            continue;
        }
        let (ga, gb) = if g[0] == g[1] {
            (g[0], g[2])
        } else {
            (g[0], g[1])
        };
        if g.iter().any(|x| *x != ga && *x != gb) {
            continue;
        }
        let f = t.map(|v| {
            let n = &near[v as usize];
            group_distance(n, ga) - group_distance(n, gb)
        });
        let mut pts = Vec::with_capacity(2);
        for (i, j) in [(0, 1), (1, 2), (2, 0)] {
            if (f[i] < 0.0) != (f[j] < 0.0) {
                let s = f[i] / (f[i] - f[j]);
                pts.push(EdgePoint::new(t[i], t[j], s));
            }
        }
        if pts.len() == 2 {
            by_pair
                .entry((ga.min(gb), ga.max(gb)))
                .or_default()
                .push([pts[0], pts[1]]);
        }
    }
    by_pair
        .into_iter()
        .flat_map(|(groups, segments)| {
            chain(segments)
                .into_iter()
                .map(move |pts| Border { pts, groups })
        })
        .collect()
}

/// Joins segments that share edge points into polylines.
pub fn chain(segments: Vec<[EdgePoint; 2]>) -> Vec<Vec<EdgePoint>> {
    let mut by_key: HashMap<(u32, u32), Vec<usize>> = HashMap::with_capacity(segments.len() * 2);
    for (i, s) in segments.iter().enumerate() {
        by_key.entry(s[0].key()).or_default().push(i);
        by_key.entry(s[1].key()).or_default().push(i);
    }
    let mut used = vec![false; segments.len()];
    let mut out = Vec::new();
    for start in 0..segments.len() {
        if used[start] {
            continue;
        }
        used[start] = true;
        let mut line: std::collections::VecDeque<EdgePoint> =
            [segments[start][0], segments[start][1]]
                .into_iter()
                .collect();
        // forward from the back, then backward from the front
        for forward in [true, false] {
            loop {
                let end = if forward {
                    *line.back().unwrap()
                } else {
                    *line.front().unwrap()
                };
                let next = by_key
                    .get(&end.key())
                    .and_then(|list| list.iter().copied().find(|s| !used[*s]));
                let Some(s) = next else { break };
                used[s] = true;
                let seg = segments[s];
                let other = if seg[0].key() == end.key() {
                    seg[1]
                } else {
                    seg[0]
                };
                if forward {
                    line.push_back(other);
                } else {
                    line.push_front(other);
                }
            }
        }
        out.push(line.into_iter().collect());
    }
    out
}

/// First skinning slot of the extra transforms (spring chain links) after the skeleton's bones.
pub const EXTRA_SLOTS: usize = 32;

/// Bone transforms from the rest pose to a posed skeleton, plus extra transforms (spring chain
/// links that hair is skinned to, like VRM spring bones) from slot [`EXTRA_SLOTS`] on.
#[derive(Debug, Clone)]
pub struct Skinning {
    rot: Vec<Mat3>,
    pos: Vec<Vec3>,
    rest: Vec<Vec3>,
}

impl Skinning {
    pub fn new(rest: &Posed, posed: &Posed) -> Self {
        let mut s = Skinning {
            rot: posed.r.map(Mat3::from_quat).to_vec(),
            pos: posed.p.to_vec(),
            rest: rest.p.to_vec(),
        };
        s.rot.resize(EXTRA_SLOTS, Mat3::IDENTITY);
        s.pos.resize(EXTRA_SLOTS, Vec3::ZERO);
        s.rest.resize(EXTRA_SLOTS, Vec3::ZERO);
        s
    }

    /// Sets an extra slot: `rest` moves to `pos`, offsets turn by `rot`.
    pub fn set_extra(&mut self, slot: usize, rest: Vec3, pos: Vec3, rot: Mat3) {
        let i = EXTRA_SLOTS + slot;
        if self.rot.len() <= i {
            self.rot.resize(i + 1, Mat3::IDENTITY);
            self.pos.resize(i + 1, Vec3::ZERO);
            self.rest.resize(i + 1, Vec3::ZERO);
        }
        self.rot[i] = rot;
        self.pos[i] = pos;
        self.rest[i] = rest;
    }

    /// A rigid transform: everything follows one bone.
    pub fn bone(&self, bone: usize, p: Vec3) -> Vec3 {
        self.pos[bone] + self.rot[bone] * (p - self.rest[bone])
    }

    pub fn apply(&self, mesh: &Mesh, pos: &mut Vec<Vec3>, nrm: &mut Vec<Vec3>) {
        pos.clear();
        nrm.clear();
        pos.reserve(mesh.rest.len());
        nrm.reserve(mesh.rest.len());
        for ((p, n), w) in mesh.rest.iter().zip(&mesh.normal).zip(&mesh.skin) {
            let mut sp = Vec3::ZERO;
            let mut sn = Vec3::ZERO;
            for &(b, wt) in w {
                if wt <= 0.0 {
                    continue;
                }
                let b = (b as usize).min(self.rot.len() - 1);
                sp += (self.pos[b] + self.rot[b] * (*p - self.rest[b])) * wt;
                sn += (self.rot[b] * *n) * wt;
            }
            pos.push(sp);
            nrm.push(sn.normalize_or(*n));
        }
    }

    /// Everything rigidly attached to `bone`.
    pub fn apply_rigid(&self, mesh: &Mesh, bone: usize, pos: &mut Vec<Vec3>, nrm: &mut Vec<Vec3>) {
        pos.clear();
        nrm.clear();
        pos.extend(mesh.rest.iter().map(|p| self.bone(bone, *p)));
        nrm.extend(mesh.normal.iter().map(|n| self.rot[bone] * *n));
    }
}

/// Smooth curve through the points (centripetal-ish Catmull-Rom), `per` samples per span.
pub fn spline(points: &[Vec3], per: usize) -> Vec<Vec3> {
    let n = points.len();
    if n < 3 || per <= 1 {
        return points.to_vec();
    }
    let mut out = Vec::with_capacity((n - 1) * per + 1);
    for i in 0..n - 1 {
        let p0 = if i == 0 {
            points[0] * 2.0 - points[1]
        } else {
            points[i - 1]
        };
        let p1 = points[i];
        let p2 = points[i + 1];
        let p3 = if i + 2 < n {
            points[i + 2]
        } else {
            p2 * 2.0 - p1
        };
        for s in 0..per {
            let t = s as f32 / per as f32;
            out.push(catmull(p0, p1, p2, p3, t));
        }
    }
    out.push(points[n - 1]);
    out
}

pub fn catmull(p0: Vec3, p1: Vec3, p2: Vec3, p3: Vec3, t: f32) -> Vec3 {
    let t2 = t * t;
    let t3 = t2 * t;
    ((p1 * 2.0)
        + (p2 - p0) * t
        + (p0 * 2.0 - p1 * 5.0 + p2 * 4.0 - p3) * t2
        + (p1 * 3.0 - p0 - p2 * 3.0 + p3) * t3)
        * 0.5
}

/// A mesh built for one frame (no skinning).
#[derive(Debug, Clone, Default)]
pub struct Geo {
    pub pos: Vec<Vec3>,
    pub nrm: Vec<Vec3>,
    pub tris: Vec<[u32; 3]>,
}

impl Geo {
    /// Edges used by one triangle only, chained into vertex polylines (hems, ribbon edges).
    pub fn open_edges(&self) -> Vec<Vec<EdgePoint>> {
        let mut count: HashMap<(u32, u32), u32> = HashMap::new();
        for t in &self.tris {
            for (i, j) in [(0, 1), (1, 2), (2, 0)] {
                let k = (t[i].min(t[j]), t[i].max(t[j]));
                *count.entry(k).or_default() += 1;
            }
        }
        let mut segments: Vec<[EdgePoint; 2]> = count
            .into_iter()
            .filter(|(_, c)| *c == 1)
            .map(|((a, b), _)| [vertex_point(a), vertex_point(b)])
            .collect();
        // deterministic chains (the map iterates in random order)
        segments.sort_by_key(|s| (s[0].a, s[1].a));
        chain(segments)
    }

    /// Recomputes smooth normals from the triangles.
    pub fn smooth_normals(&mut self) {
        self.nrm = vec![Vec3::ZERO; self.pos.len()];
        for t in &self.tris {
            let [a, b, c] = t.map(|v| self.pos[v as usize]);
            let n = (b - a).cross(c - a);
            for v in t {
                self.nrm[*v as usize] += n;
            }
        }
        for n in &mut self.nrm {
            *n = n.normalize_or(Vec3::Y);
        }
    }

    fn push(&mut self, p: Vec3, n: Vec3) -> u32 {
        self.pos.push(p);
        self.nrm.push(n);
        (self.pos.len() - 1) as u32
    }

    /// A closed tube along `centre` with an elliptic cross section: `width` along `side`
    /// (per point), `thickness` times that across. The end tapers to a point.
    pub fn tube(
        &mut self,
        centre: &[Vec3],
        width: &[f32],
        thickness: f32,
        side: Vec3,
        around: usize,
    ) {
        let n = centre.len();
        if n < 2 {
            return;
        }
        let base = self.pos.len() as u32;
        let mut prev_side = side;
        for i in 0..n {
            let tangent = if i + 1 < n {
                centre[i + 1] - centre[i]
            } else {
                centre[i] - centre[i - 1]
            }
            .normalize_or(Vec3::Y);
            // parallel transport of the side vector
            let s = (prev_side - tangent * prev_side.dot(tangent))
                .normalize_or(tangent.any_orthonormal_vector());
            prev_side = s;
            let up = tangent.cross(s);
            let w = width[i] * 0.5;
            for k in 0..around {
                let a = k as f32 / around as f32 * std::f32::consts::TAU;
                let (sa, ca) = a.sin_cos();
                let offset = s * (ca * w) + up * (sa * w * thickness);
                let normal = (s * (ca / w.max(1e-5)) + up * (sa / (w * thickness).max(1e-5)))
                    .normalize_or(up);
                self.push(centre[i] + offset, normal);
            }
        }
        let ring = |i: usize, k: usize| base + (i * around + k % around) as u32;
        for i in 0..n - 1 {
            for k in 0..around {
                let (a, b, c, d) = (
                    ring(i, k),
                    ring(i, k + 1),
                    ring(i + 1, k + 1),
                    ring(i + 1, k),
                );
                self.tris.push([a, c, b]);
                self.tris.push([a, d, c]);
            }
        }
        // caps
        let first = centre[0];
        let t0 = (centre[0] - centre[1]).normalize_or(Vec3::NEG_Y);
        let c0 = self.push(first, t0);
        for k in 0..around {
            self.tris.push([c0, ring(0, k), ring(0, k + 1)]);
        }
        let last = centre[n - 1];
        let t1 = (centre[n - 1] - centre[n - 2]).normalize_or(Vec3::Y);
        let c1 = self.push(last + t1 * width[n - 1] * 0.25, t1);
        for k in 0..around {
            self.tris.push([c1, ring(n - 1, k + 1), ring(n - 1, k)]);
        }
    }

    /// A flat two-sided strip along `centre`, `width` wide along `side`, facing `normal`.
    pub fn ribbon(&mut self, centre: &[Vec3], width: &[f32], normal: Vec3) {
        let n = centre.len();
        if n < 2 {
            return;
        }
        let base = self.pos.len() as u32;
        for i in 0..n {
            let tangent = if i + 1 < n {
                centre[i + 1] - centre[i]
            } else {
                centre[i] - centre[i - 1]
            }
            .normalize_or(Vec3::NEG_Y);
            let side = tangent.cross(normal).normalize_or(Vec3::X);
            let nrm = side.cross(tangent).normalize_or(normal);
            self.push(centre[i] - side * width[i] * 0.5, nrm);
            self.push(centre[i] + side * width[i] * 0.5, nrm);
        }
        for i in 0..n as u32 - 1 {
            let (a, b, c, d) = (
                base + i * 2,
                base + i * 2 + 1,
                base + i * 2 + 3,
                base + i * 2 + 2,
            );
            self.tris.push([a, b, c]);
            self.tris.push([a, c, d]);
        }
    }

    /// A smooth sheet through a grid of points (`rows` x `cols`, row major), `per` samples per
    /// span in both directions.
    pub fn sheet(&mut self, grid: &[Vec3], rows: usize, cols: usize, per: usize, wrap: bool) {
        // rows first, then columns
        let mut along: Vec<Vec<Vec3>> = Vec::with_capacity(rows);
        for r in 0..rows {
            let row = &grid[r * cols..(r + 1) * cols];
            along.push(spline(row, per));
        }
        let w = along[0].len();
        let mut cols_pts: Vec<Vec<Vec3>> = Vec::with_capacity(w);
        for c in 0..w {
            let col: Vec<Vec3> = along.iter().map(|row| row[c]).collect();
            cols_pts.push(spline(&col, per));
        }
        let h = cols_pts[0].len();
        let base = self.pos.len() as u32;
        for r in 0..h {
            for col in &cols_pts {
                self.push(col[r], Vec3::ZERO);
            }
        }
        let id = |r: usize, c: usize| base + (r * w + c) as u32;
        let spans = if wrap { w } else { w - 1 };
        for r in 0..h - 1 {
            for c in 0..spans {
                let c1 = (c + 1) % w;
                self.tris.push([id(r, c), id(r + 1, c), id(r + 1, c1)]);
                self.tris.push([id(r, c), id(r + 1, c1), id(r, c1)]);
            }
        }
        // normals of this sheet only
        let start = base as usize;
        let mut acc = vec![Vec3::ZERO; self.pos.len() - start];
        let first_tri = self.tris.len() - (h - 1) * spans * 2;
        for t in &self.tris[first_tri..] {
            let [a, b, c] = t.map(|v| self.pos[v as usize]);
            let n = (b - a).cross(c - a);
            for v in t {
                acc[*v as usize - start] += n;
            }
        }
        for (i, n) in acc.into_iter().enumerate() {
            self.nrm[start + i] = n.normalize_or(Vec3::Z);
        }
    }
}

fn vertex_point(v: u32) -> EdgePoint {
    EdgePoint { a: v, b: v, t: 0.0 }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rig::Bone;
    use crate::sdf::{Prim, Shape};

    #[test]
    fn sphere_mesh_is_closed_and_round() {
        let mut m = Model::default();
        m.push(Prim::new(Shape::Sphere(0.1), 0, Bone::Head));
        let mesh = surface_nets(
            &m,
            Vec3::splat(-0.15),
            Vec3::splat(0.15),
            0.01,
            0.02,
            &|_| true,
        );
        assert!(mesh.tris.len() > 500);
        for p in &mesh.rest {
            assert!((p.length() - 0.1).abs() < 1e-3, "{p}");
        }
        // outward winding
        for t in &mesh.tris {
            let [a, b, c] = t.map(|v| mesh.rest[v as usize]);
            assert!((b - a).cross(c - a).dot(a) > 0.0);
        }
        // closed: every edge shared by two triangles
        let geo = Geo {
            pos: mesh.rest.clone(),
            nrm: mesh.normal.clone(),
            tris: mesh.tris.clone(),
        };
        assert_eq!(geo.open_edges().len(), 0);
    }

    #[test]
    fn borders_follow_group_seams() {
        let mut m = Model::default();
        m.push(Prim::new(Shape::Sphere(0.1), 0, Bone::Head));
        m.push(Prim::new(Shape::Sphere(0.1), 1, Bone::Head).at(Vec3::new(0.1, 0.0, 0.0)));
        let mesh = surface_nets(
            &m,
            Vec3::new(-0.15, -0.15, -0.15),
            Vec3::new(0.25, 0.15, 0.15),
            0.008,
            0.02,
            &|_| true,
        );
        assert!(!mesh.borders.is_empty());
        for line in &mesh.borders {
            for p in &line.pts {
                assert!((p.eval(&mesh.rest).x - 0.05).abs() < 0.01);
            }
        }
    }
}
