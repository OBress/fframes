//! Tools for building organic and procedural shapes in code: skinning a mesh to a chain of
//! bones, smooth 2D outlines (curves and SVG paths) for extrusions and lathes, deformers,
//! box-projected UVs, implicit surfaces (metaballs and signed distance fields, meshed with
//! marching tetrahedra) and Loop subdivision.

use std::collections::HashMap;
use std::sync::Arc;

use glam::{IVec3, Mat4, Quat, Vec2, Vec3};

use crate::geometry::{Geometry, Shape, SkinWeights};
use crate::math::Aabb;
use crate::scene::{NodeId, Scene, Skin};

// --- skinning ---------------------------------------------------------------------------------

impl Geometry {
    /// Skins the geometry to bones given as segments (from, to) in the geometry's space:
    /// each vertex follows the `influences` (1 to 4) nearest bones, weighted by inverse
    /// distance to the segment and normalized. The poor man's automatic weights, good for
    /// tubes, tails, tentacles and limbs built along their bones. Joint `i` of the skin is
    /// bone `i`.
    pub fn skin_to_segments(mut self, bones: &[(Vec3, Vec3)], influences: usize) -> Self {
        let k = influences.clamp(1, 4);
        let distance = |p: Vec3, (a, b): (Vec3, Vec3)| {
            let ab = b - a;
            let t = ((p - a).dot(ab) / ab.length_squared().max(1e-12)).clamp(0.0, 1.0);
            (p - (a + ab * t)).length()
        };
        let mut joints = Vec::with_capacity(self.positions.len());
        let mut weights = Vec::with_capacity(self.positions.len());
        for &p in &self.positions {
            let mut near: Vec<(usize, f32)> = bones.iter().enumerate().map(|(i, &b)| (i, distance(p, b))).collect();
            near.sort_by(|a, b| a.1.total_cmp(&b.1));
            near.truncate(k);
            // Inverse distance, sharpened so a vertex mostly follows its own bone.
            let raw: Vec<f32> = near.iter().map(|(_, d)| 1.0 / (d * d + 1e-6)).collect();
            let sum: f32 = raw.iter().sum();
            let mut j = [0u16; 4];
            let mut w = [0.0f32; 4];
            for (slot, ((bone, _), r)) in near.iter().zip(&raw).enumerate() {
                j[slot] = *bone as u16;
                w[slot] = r / sum;
            }
            joints.push(j);
            weights.push(w);
        }
        self.skin = Some(SkinWeights { joints, weights });
        self
    }
}

impl Skin {
    /// A skin whose rest pose is the scene as it is now: the inverse bind matrix of each
    /// joint takes the mesh from `mesh_world` (where the mesh should appear at rest) into the
    /// joint's space. Build the joints in their rest pose, then call this.
    pub fn from_rest(scene: &Scene, joints: Vec<NodeId>, mesh_world: Mat4) -> Self {
        let inverse_bind_matrices = joints.iter().map(|&j| scene.world_matrix(j).inverse() * mesh_world).collect();
        Skin { joints, inverse_bind_matrices: Arc::new(inverse_bind_matrices) }
    }
}

/// A chain of `bones` joints under `parent`, straight along +Y from its origin, each
/// `length / bones` long. Returns the joints; skin a mesh built along +Y to it with
/// [`Geometry::skin_to_segments`] and [`chain_segments`], then [`Skin::from_rest`].
pub fn add_bone_chain(scene: &mut Scene, parent: NodeId, bones: u32, length: f32) -> Vec<NodeId> {
    let seg = length / bones.max(1) as f32;
    let mut joints = Vec::new();
    let mut prev = parent;
    for i in 0..bones.max(1) {
        let y = if i == 0 { 0.0 } else { seg };
        let j = scene.add_to(prev, crate::Node::group().at(0.0, y, 0.0));
        joints.push(j);
        prev = j;
    }
    joints
}

/// The segments of a straight chain along +Y, as [`add_bone_chain`] builds it.
pub fn chain_segments(bones: u32, length: f32) -> Vec<(Vec3, Vec3)> {
    let seg = length / bones.max(1) as f32;
    (0..bones.max(1)).map(|i| (Vec3::Y * seg * i as f32, Vec3::Y * seg * (i + 1) as f32)).collect()
}

// --- 2D curves --------------------------------------------------------------------------------

/// A smooth curve through `points` (Catmull-Rom), `steps` points per span. With `closed`,
/// the curve joins the last point back to the first. For outlines to extrude and profiles
/// to lathe that must stay round.
pub fn smooth_curve(points: &[Vec2], steps: u32, closed: bool) -> Vec<Vec2> {
    let n = points.len();
    if n < 3 {
        return points.to_vec();
    }
    let steps = steps.max(1);
    let get = |i: isize| {
        if closed {
            points[i.rem_euclid(n as isize) as usize]
        } else {
            points[i.clamp(0, n as isize - 1) as usize]
        }
    };
    let spans = if closed { n } else { n - 1 };
    let mut out = Vec::with_capacity(spans * steps as usize + 1);
    for i in 0..spans as isize {
        let (p0, p1, p2, p3) = (get(i - 1), get(i), get(i + 1), get(i + 2));
        for s in 0..steps {
            let t = s as f32 / steps as f32;
            let (t2, t3) = (t * t, t * t * t);
            out.push(0.5 * ((2.0 * p1) + (-p0 + p2) * t + (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * t2 + (-p0 + 3.0 * p1 - 3.0 * p2 + p3) * t3));
        }
    }
    if !closed {
        out.push(points[n - 1]);
    }
    out
}

/// The reason an SVG path does not parse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathError(pub String);

impl std::fmt::Display for PathError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SVG path: {}", self.0)
    }
}

impl std::error::Error for PathError {}

/// The subpaths of an SVG path `d` as polylines, curves flattened to `steps` points each.
/// It reads `M L H V C S Q T A Z` and their relative forms. SVG's Y goes down; the points
/// keep SVG's coordinates (flip Y yourself, or use [`Shape::from_svg_path`]).
pub fn svg_path(d: &str, steps: u32) -> Result<Vec<Vec<Vec2>>, PathError> {
    let steps = steps.max(1);
    // Tokens: command letters and numbers.
    let mut tokens: Vec<Result<char, f32>> = Vec::new();
    let chars: Vec<char> = d.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c.is_ascii_alphabetic() && !matches!(c, 'e' | 'E') {
            tokens.push(Ok(c));
            i += 1;
        } else if c.is_ascii_digit() || matches!(c, '-' | '+' | '.') {
            let start = i;
            i += 1;
            let mut dot = c == '.';
            while i < chars.len() {
                let ch = chars[i];
                let prev = chars[i - 1];
                if ch.is_ascii_digit() || ((ch == '-' || ch == '+') && matches!(prev, 'e' | 'E')) || matches!(ch, 'e' | 'E') {
                    i += 1;
                } else if ch == '.' && !dot {
                    dot = true;
                    i += 1;
                } else {
                    break;
                }
            }
            let s: String = chars[start..i].iter().collect();
            tokens.push(Err(s.parse().map_err(|_| PathError(format!("bad number {s:?}")))?));
        } else {
            i += 1;
        }
    }
    let mut paths: Vec<Vec<Vec2>> = Vec::new();
    let mut current: Vec<Vec2> = Vec::new();
    let (mut pos, mut start) = (Vec2::ZERO, Vec2::ZERO);
    let mut last_ctrl: Option<(char, Vec2)> = None;
    let mut k = 0;
    let mut cmd = ' ';
    let num = |k: &mut usize| -> Result<f32, PathError> {
        match tokens.get(*k) {
            Some(Err(v)) => {
                *k += 1;
                Ok(*v)
            }
            _ => Err(PathError(format!("expected a number at token {k}"))),
        }
    };
    let flush = |current: &mut Vec<Vec2>, paths: &mut Vec<Vec<Vec2>>| {
        if current.len() > 1 {
            paths.push(std::mem::take(current));
        } else {
            current.clear();
        }
    };
    while k < tokens.len() {
        if let Ok(c) = tokens[k] {
            cmd = c;
            k += 1;
        } else if cmd == ' ' {
            return Err(PathError("a number before the first command".into()));
        }
        let rel = cmd.is_ascii_lowercase();
        let base = if rel { pos } else { Vec2::ZERO };
        match cmd.to_ascii_uppercase() {
            'M' => {
                flush(&mut current, &mut paths);
                pos = base + Vec2::new(num(&mut k)?, num(&mut k)?);
                start = pos;
                current.push(pos);
                // Further pairs are line-tos.
                cmd = if rel { 'l' } else { 'L' };
                last_ctrl = None;
            }
            'L' => {
                pos = base + Vec2::new(num(&mut k)?, num(&mut k)?);
                current.push(pos);
                last_ctrl = None;
            }
            'H' => {
                let x = num(&mut k)?;
                pos.x = if rel { pos.x + x } else { x };
                current.push(pos);
                last_ctrl = None;
            }
            'V' => {
                let y = num(&mut k)?;
                pos.y = if rel { pos.y + y } else { y };
                current.push(pos);
                last_ctrl = None;
            }
            'C' | 'S' => {
                let c1 = if cmd.eq_ignore_ascii_case(&'S') {
                    match last_ctrl {
                        Some(('C', c)) => 2.0 * pos - c,
                        _ => pos,
                    }
                } else {
                    base + Vec2::new(num(&mut k)?, num(&mut k)?)
                };
                let c2 = base + Vec2::new(num(&mut k)?, num(&mut k)?);
                let end = base + Vec2::new(num(&mut k)?, num(&mut k)?);
                let p0 = pos;
                for s in 1..=steps {
                    let t = s as f32 / steps as f32;
                    let u = 1.0 - t;
                    current.push(p0 * u * u * u + c1 * 3.0 * u * u * t + c2 * 3.0 * u * t * t + end * t * t * t);
                }
                pos = end;
                last_ctrl = Some(('C', c2));
            }
            'Q' | 'T' => {
                let c = if cmd.eq_ignore_ascii_case(&'T') {
                    match last_ctrl {
                        Some(('Q', c)) => 2.0 * pos - c,
                        _ => pos,
                    }
                } else {
                    base + Vec2::new(num(&mut k)?, num(&mut k)?)
                };
                let end = base + Vec2::new(num(&mut k)?, num(&mut k)?);
                let p0 = pos;
                for s in 1..=steps {
                    let t = s as f32 / steps as f32;
                    let u = 1.0 - t;
                    current.push(p0 * u * u + c * 2.0 * u * t + end * t * t);
                }
                pos = end;
                last_ctrl = Some(('Q', c));
            }
            'A' => {
                let (rx, ry, rot) = (num(&mut k)?.abs(), num(&mut k)?.abs(), num(&mut k)?.to_radians());
                let (large, sweep) = (num(&mut k)? != 0.0, num(&mut k)? != 0.0);
                let end = base + Vec2::new(num(&mut k)?, num(&mut k)?);
                current.extend(arc_points(pos, end, rx, ry, rot, large, sweep, steps));
                pos = end;
                last_ctrl = None;
            }
            'Z' => {
                pos = start;
                if current.first() != current.last() {
                    current.push(start);
                }
                flush(&mut current, &mut paths);
                current.push(start);
                last_ctrl = None;
                cmd = ' ';
            }
            other => return Err(PathError(format!("unknown command {other:?}"))),
        }
    }
    flush(&mut current, &mut paths);
    // A closed subpath repeats its first point; outlines do not.
    for p in &mut paths {
        if p.len() > 2 && p.first() == p.last() {
            p.pop();
        }
    }
    Ok(paths)
}

/// The points of an SVG elliptical arc after `from`, ending at `to` (SVG implementation
/// notes, F.6.5).
#[allow(clippy::too_many_arguments)]
fn arc_points(from: Vec2, to: Vec2, mut rx: f32, mut ry: f32, rot: f32, large: bool, sweep: bool, steps: u32) -> Vec<Vec2> {
    if rx < 1e-6 || ry < 1e-6 || from == to {
        return vec![to];
    }
    let (sin, cos) = rot.sin_cos();
    let d = (from - to) * 0.5;
    let p = Vec2::new(cos * d.x + sin * d.y, -sin * d.x + cos * d.y);
    let lambda = (p.x * p.x) / (rx * rx) + (p.y * p.y) / (ry * ry);
    if lambda > 1.0 {
        rx *= lambda.sqrt();
        ry *= lambda.sqrt();
    }
    let num = (rx * rx * ry * ry - rx * rx * p.y * p.y - ry * ry * p.x * p.x).max(0.0);
    let den = rx * rx * p.y * p.y + ry * ry * p.x * p.x;
    let mut coef = (num / den.max(1e-12)).sqrt();
    if large == sweep {
        coef = -coef;
    }
    let c = Vec2::new(coef * rx * p.y / ry, -coef * ry * p.x / rx);
    let center = Vec2::new(cos * c.x - sin * c.y, sin * c.x + cos * c.y) + (from + to) * 0.5;
    let angle = |v: Vec2| v.y.atan2(v.x);
    let t0 = angle(Vec2::new((p.x - c.x) / rx, (p.y - c.y) / ry));
    let mut dt = angle(Vec2::new((-p.x - c.x) / rx, (-p.y - c.y) / ry)) - t0;
    if sweep && dt < 0.0 {
        dt += std::f32::consts::TAU;
    } else if !sweep && dt > 0.0 {
        dt -= std::f32::consts::TAU;
    }
    let n = (steps as f32 * (dt.abs() / std::f32::consts::FRAC_PI_2).ceil()).max(1.0) as u32;
    (1..=n)
        .map(|i| {
            let t = t0 + dt * i as f32 / n as f32;
            let (s, c) = t.sin_cos();
            center + Vec2::new(cos * rx * c - sin * ry * s, sin * rx * c + cos * ry * s)
        })
        .collect()
}

/// The signed area of an outline: positive when counter-clockwise.
fn area(points: &[Vec2]) -> f32 {
    (0..points.len()).map(|i| points[i].perp_dot(points[(i + 1) % points.len()])).sum::<f32>() * 0.5
}

impl Shape {
    /// The shape of an SVG path, for [`Geometry::extrude_shape`]: Y flipped to point up,
    /// scaled by `scale` (meters per SVG unit), the largest subpath the outline and the
    /// others holes. Outlines come out counter-clockwise and holes clockwise, whatever the
    /// path's own direction.
    pub fn from_svg_path(d: &str, scale: f32, steps: u32) -> Result<Shape, PathError> {
        let mut paths: Vec<Vec<Vec2>> = svg_path(d, steps)?
            .into_iter()
            .map(|p| p.into_iter().map(|q| Vec2::new(q.x, -q.y) * scale).collect::<Vec<_>>())
            .filter(|p| p.len() >= 3)
            .collect();
        if paths.is_empty() {
            return Err(PathError("no closed outline".into()));
        }
        paths.sort_by(|a, b| area(b).abs().total_cmp(&area(a).abs()));
        let mut outer = paths.remove(0);
        if area(&outer) < 0.0 {
            outer.reverse();
        }
        let holes = paths
            .into_iter()
            .map(|mut h| {
                if area(&h) > 0.0 {
                    h.reverse();
                }
                h
            })
            .collect();
        Ok(Shape { outer, holes })
    }
}

// --- deformers and UVs ------------------------------------------------------------------------

impl Geometry {
    /// Moves every vertex to `f(position)` and recomputes the normals: a general deformer.
    /// Morph targets keep their offsets; skin weights stay.
    pub fn deform(mut self, f: impl Fn(Vec3) -> Vec3) -> Self {
        for p in &mut self.positions {
            *p = f(*p);
        }
        self.recompute_normals_keeping_seams();
        self
    }

    /// Moves every vertex along its normal by `f(position, normal)` meters: lumps, warts,
    /// ripples, noise. Recomputes the normals.
    pub fn displace(mut self, f: impl Fn(Vec3, Vec3) -> f32) -> Self {
        if self.normals.len() != self.positions.len() {
            self.compute_vertex_normals();
        }
        for (p, n) in self.positions.iter_mut().zip(&self.normals) {
            *p += *n * f(*p, *n);
        }
        self.recompute_normals_keeping_seams();
        self
    }

    /// Twists the geometry around the Y axis by `radians_per_meter` along Y.
    pub fn twist(self, radians_per_meter: f32) -> Self {
        self.deform(|p| Quat::from_rotation_y(p.y * radians_per_meter) * p)
    }

    /// Scales the geometry across the Y axis by `scale(y)`: a horn narrowing to a point,
    /// a bottle's neck.
    pub fn taper(self, scale: impl Fn(f32) -> f32) -> Self {
        self.deform(|p| {
            let s = scale(p.y);
            Vec3::new(p.x * s, p.y, p.z * s)
        })
    }

    /// Bends the geometry along Y into an arc toward +X, `radians_per_meter` of turn per
    /// meter of height: a stalk, a hunched back, a curling claw. Y = 0 stays in place.
    pub fn bend(self, radians_per_meter: f32) -> Self {
        if radians_per_meter.abs() < 1e-6 {
            return self;
        }
        let r = 1.0 / radians_per_meter;
        self.deform(|p| {
            let theta = p.y * radians_per_meter;
            // The Y axis becomes an arc of radius r around (r, 0); x moves along the radius.
            let rr = r - p.x;
            Vec3::new(r - rr * theta.cos(), rr * theta.sin(), p.z)
        })
    }

    /// Box-projected UVs, `meters` per texture repeat: each vertex takes the plane of its
    /// normal's main axis. No pinched poles on blobs and rocks; seams where the axis changes
    /// (split vertices there with [`Geometry::flat`] or a crease angle in
    /// [`Geometry::smooth`] for clean ones).
    pub fn box_uvs(mut self, meters: f32) -> Self {
        if self.normals.len() != self.positions.len() {
            self.compute_vertex_normals();
        }
        let s = 1.0 / meters.max(1e-6);
        self.uvs = (self.positions.iter())
            .zip(&self.normals)
            .map(|(p, n)| {
                let a = n.abs();
                if a.x >= a.y && a.x >= a.z {
                    Vec2::new(p.z * n.x.signum(), -p.y) * s
                } else if a.y >= a.z {
                    Vec2::new(p.x, p.z * n.y.signum()) * s
                } else {
                    Vec2::new(-p.x * n.z.signum(), -p.y) * s
                }
            })
            .collect();
        self
    }

    /// Recomputes smooth normals, averaging across vertices at the same position only when
    /// they shared a normal before (so seams in UVs stay smooth and creases stay sharp).
    fn recompute_normals_keeping_seams(&mut self) {
        let before = std::mem::take(&mut self.normals);
        self.compute_vertex_normals();
        if before.len() != self.positions.len() {
            self.update_bounds();
            return;
        }
        // Copies of one vertex (same position and normal before) get the same new normal.
        let key = |p: Vec3, n: Vec3| ((p * 1e4).round().as_ivec3(), (n * 1e3).round().as_ivec3());
        let mut sums: HashMap<(IVec3, IVec3), Vec3> = HashMap::new();
        let positions_before: Vec<Vec3> = self.positions.clone();
        for (i, n) in self.normals.iter().enumerate() {
            *sums.entry(key(positions_before[i], before[i])).or_default() += *n;
        }
        for (i, n) in self.normals.iter_mut().enumerate() {
            *n = sums[&key(positions_before[i], before[i])].normalize_or(*n);
        }
        self.update_bounds();
    }
}

// --- implicit surfaces ------------------------------------------------------------------------

/// Signed distance functions to build implicit surfaces from, negative inside.
pub mod sdf {
    use glam::Vec3;

    pub fn sphere(p: Vec3, center: Vec3, radius: f32) -> f32 {
        (p - center).length() - radius
    }

    /// A capsule from `a` to `b`, with a radius from `ra` at `a` to `rb` at `b`.
    pub fn capsule(p: Vec3, a: Vec3, b: Vec3, ra: f32, rb: f32) -> f32 {
        let ab = b - a;
        let t = ((p - a).dot(ab) / ab.length_squared().max(1e-12)).clamp(0.0, 1.0);
        (p - (a + ab * t)).length() - (ra + (rb - ra) * t)
    }

    /// An ellipsoid with the half sizes `radii` (an approximate distance).
    pub fn ellipsoid(p: Vec3, center: Vec3, radii: Vec3) -> f32 {
        let q = (p - center) / radii;
        let k0 = q.length();
        let k1 = ((p - center) / (radii * radii)).length();
        if k1 < 1e-9 { -radii.min_element() } else { k0 * (k0 - 1.0) / k1 }
    }

    /// A smooth union of two distances: `k` is how far (meters) the blend reaches.
    pub fn smooth_union(a: f32, b: f32, k: f32) -> f32 {
        if k <= 0.0 {
            return a.min(b);
        }
        let h = (0.5 + 0.5 * (b - a) / k).clamp(0.0, 1.0);
        b + (a - b) * h - k * h * (1.0 - h)
    }

    /// Cuts `b` out of `a`.
    pub fn subtract(a: f32, b: f32) -> f32 {
        a.max(-b)
    }
}

impl Geometry {
    /// The surface where `field` is 0 (negative inside) within `bounds`, sampled on a grid of
    /// `cell` meters and meshed with marching tetrahedra. Normals come from the field's
    /// gradient, so the surface shades smooth. The way to one fused, organic body from
    /// spheres and capsules ([`sdf::smooth_union`]) or [`Geometry::metaballs`].
    pub fn from_sdf(field: impl Fn(Vec3) -> f32 + Sync, bounds: Aabb, cell: f32) -> Self {
        let cell = cell.max(1e-4);
        let size = bounds.size();
        let dims = (size / cell).ceil().as_uvec3() + 1;
        let (nx, ny, nz) = (dims.x as usize + 1, dims.y as usize + 1, dims.z as usize + 1);
        let at = |x: usize, y: usize, z: usize| bounds.min + Vec3::new(x as f32, y as f32, z as f32) * cell;
        // The field at every grid point, row by row.
        use rayon::prelude::*;
        let values: Vec<f32> = (0..nx * ny * nz)
            .into_par_iter()
            .map(|i| {
                let (x, y, z) = (i % nx, (i / nx) % ny, i / (nx * ny));
                field(at(x, y, z))
            })
            .collect();
        let index = |x: usize, y: usize, z: usize| x + nx * (y + ny * z);
        // A cube's corners and its six tetrahedra around the main diagonal 0-6.
        const CORNERS: [(usize, usize, usize); 8] = [(0, 0, 0), (1, 0, 0), (1, 1, 0), (0, 1, 0), (0, 0, 1), (1, 0, 1), (1, 1, 1), (0, 1, 1)];
        const TETS: [[usize; 4]; 6] = [[0, 5, 1, 6], [0, 1, 2, 6], [0, 2, 3, 6], [0, 3, 7, 6], [0, 7, 4, 6], [0, 4, 5, 6]];
        let mut positions: Vec<Vec3> = Vec::new();
        let mut indices: Vec<u32> = Vec::new();
        // One vertex per crossed grid edge, shared by every tetrahedron on it. A crossing
        // close to a grid point snaps to it (one vertex for every edge there): otherwise the
        // tetrahedra leave needle triangles, whose face normals are so noisy that the
        // renderers flip the shading on some pixels (speckles). The snapped triangles
        // degenerate and are dropped below.
        const SNAP: f32 = 0.12;
        let mut on_edge: HashMap<(usize, usize), u32> = HashMap::new();
        let mut vertex = |a: usize, b: usize, pa: Vec3, pb: Vec3, va: f32, vb: f32, positions: &mut Vec<Vec3>| {
            let t = (va / (va - vb)).clamp(0.0, 1.0);
            let (key, p) = if t < SNAP {
                ((a, a), pa)
            } else if t > 1.0 - SNAP {
                ((b, b), pb)
            } else {
                ((a.min(b), a.max(b)), pa.lerp(pb, t))
            };
            *on_edge.entry(key).or_insert_with(|| {
                positions.push(p);
                positions.len() as u32 - 1
            })
        };
        for z in 0..nz - 1 {
            for y in 0..ny - 1 {
                for x in 0..nx - 1 {
                    let ids = CORNERS.map(|(dx, dy, dz)| index(x + dx, y + dy, z + dz));
                    let v = ids.map(|i| values[i]);
                    if v.iter().all(|&f| f < 0.0) || v.iter().all(|&f| f >= 0.0) {
                        continue;
                    }
                    let p = CORNERS.map(|(dx, dy, dz)| at(x + dx, y + dy, z + dz));
                    for tet in TETS {
                        let inside: Vec<usize> = tet.iter().copied().filter(|&c| v[c] < 0.0).collect();
                        let outside: Vec<usize> = tet.iter().copied().filter(|&c| v[c] >= 0.0).collect();
                        let mut e = |a: usize, b: usize| vertex(ids[a], ids[b], p[a], p[b], v[a], v[b], &mut positions);
                        let tris: Vec<[u32; 3]> = match inside.len() {
                            1 => vec![[e(inside[0], outside[0]), e(inside[0], outside[1]), e(inside[0], outside[2])]],
                            3 => vec![[e(outside[0], inside[0]), e(outside[0], inside[1]), e(outside[0], inside[2])]],
                            2 => {
                                let (a, b, c, d) = (e(inside[0], outside[0]), e(inside[0], outside[1]), e(inside[1], outside[0]), e(inside[1], outside[1]));
                                vec![[a, b, c], [b, d, c]]
                            }
                            _ => Vec::new(),
                        };
                        indices.extend(tris.into_iter().flatten());
                    }
                }
            }
        }
        // Normals from the gradient, and each triangle turned to face along it (out).
        let eps = cell * 0.5;
        let gradient = |p: Vec3| {
            Vec3::new(
                field(p + Vec3::X * eps) - field(p - Vec3::X * eps),
                field(p + Vec3::Y * eps) - field(p - Vec3::Y * eps),
                field(p + Vec3::Z * eps) - field(p - Vec3::Z * eps),
            )
            .normalize_or(Vec3::Y)
        };
        let normals: Vec<Vec3> = positions.par_iter().map(|&p| gradient(p)).collect();
        for t in indices.chunks_exact_mut(3) {
            let [a, b, c] = [t[0], t[1], t[2]].map(|i| positions[i as usize]);
            let face = (b - a).cross(c - a);
            let n = normals[t[0] as usize] + normals[t[1] as usize] + normals[t[2] as usize];
            if face.dot(n) < 0.0 {
                t.swap(1, 2);
            }
        }
        // Drop the slivers that degenerate where two edge vertices coincide.
        let indices: Vec<u32> = indices
            .chunks_exact(3)
            .filter(|t| t[0] != t[1] && t[1] != t[2] && t[0] != t[2])
            .flatten()
            .copied()
            .collect();
        Geometry::new(positions, normals, Vec::new(), indices)
    }

    /// Metaballs: the surface where the summed field of `balls` (center, radius) reaches
    /// 1, meshed every `cell` meters. Balls closer than their radii melt together.
    pub fn metaballs(balls: &[(Vec3, f32)], cell: f32) -> Self {
        if balls.is_empty() {
            return Geometry::default();
        }
        let bounds = balls.iter().fold(Aabb::EMPTY, |b, &(c, r)| b.union(Aabb::from_points([c - Vec3::splat(r * 1.6), c + Vec3::splat(r * 1.6)])));
        // A smooth, bounded falloff: (1 - d²/R²)² inside R = 1.5 r, scaled so that a single
        // ball's surface lands at its radius.
        let field = |p: Vec3| {
            let sum: f32 = balls
                .iter()
                .map(|&(c, r)| {
                    let big = r * 1.5;
                    let q = 1.0 - (p - c).length_squared() / (big * big);
                    if q > 0.0 { q * q / ((1.0 - 1.0 / 2.25) * (1.0 - 1.0 / 2.25)) } else { 0.0 }
                })
                .sum();
            1.0 - sum
        };
        Self::from_sdf(field, bounds, cell)
    }

    /// One level of Loop subdivision per `levels` (at most 4): every triangle becomes four
    /// and the surface rounds off, so a coarse cage of a creature turns into one smooth
    /// body. Vertices at the same position are welded first; UVs and vertex colors are
    /// averaged along new edges (texture seams smear, so project UVs after with
    /// [`Geometry::box_uvs`]). Skins and morph targets are dropped. Open edges stay where
    /// they are (boundary rules).
    pub fn subdivide(self, levels: u32) -> Self {
        let mut g = self.weld();
        for _ in 0..levels.min(4) {
            g = g.loop_once();
        }
        g.compute_vertex_normals();
        g
    }

    /// One vertex per position (UV and color of the first copy).
    fn weld(&self) -> Self {
        let key = |p: Vec3| (p * 1e5).round().as_ivec3();
        let mut map: HashMap<IVec3, u32> = HashMap::new();
        let mut out = Geometry::default();
        let mut remap = Vec::with_capacity(self.positions.len());
        for (i, p) in self.positions.iter().enumerate() {
            let id = *map.entry(key(*p)).or_insert_with(|| {
                out.positions.push(*p);
                if let Some(uv) = self.uvs.get(i) {
                    out.uvs.push(*uv);
                }
                if let Some(c) = self.colors.get(i) {
                    out.colors.push(*c);
                }
                out.positions.len() as u32 - 1
            });
            remap.push(id);
        }
        out.indices = self.indices.iter().map(|&i| remap[i as usize]).collect();
        out
    }

    fn loop_once(&self) -> Self {
        let n = self.positions.len();
        let tris: Vec<[u32; 3]> = self.indices.chunks_exact(3).map(|t| [t[0], t[1], t[2]]).collect();
        // Edges with the vertices opposite them (one for an open edge, two inside).
        let mut edges: HashMap<(u32, u32), Vec<u32>> = HashMap::new();
        for t in &tris {
            for k in 0..3 {
                let (a, b, c) = (t[k], t[(k + 1) % 3], t[(k + 2) % 3]);
                edges.entry((a.min(b), a.max(b))).or_default().push(c);
            }
        }
        let mut neighbors: Vec<Vec<u32>> = vec![Vec::new(); n];
        let mut boundary_neighbors: Vec<Vec<u32>> = vec![Vec::new(); n];
        for (&(a, b), opposite) in &edges {
            neighbors[a as usize].push(b);
            neighbors[b as usize].push(a);
            if opposite.len() == 1 {
                boundary_neighbors[a as usize].push(b);
                boundary_neighbors[b as usize].push(a);
            }
        }
        // Old vertices move toward their neighbors.
        let mut positions: Vec<Vec3> = (0..n)
            .map(|i| {
                let p = self.positions[i];
                let bn = &boundary_neighbors[i];
                if bn.len() >= 2 {
                    return p * 0.75 + (self.positions[bn[0] as usize] + self.positions[bn[1] as usize]) * 0.125;
                }
                let k = neighbors[i].len();
                if k == 0 {
                    return p;
                }
                let beta = if k == 3 { 3.0 / 16.0 } else { 3.0 / (8.0 * k as f32) };
                let sum: Vec3 = neighbors[i].iter().map(|&j| self.positions[j as usize]).sum();
                p * (1.0 - k as f32 * beta) + sum * beta
            })
            .collect();
        let has_uvs = self.uvs.len() == n;
        let has_colors = self.colors.len() == n;
        let mut uvs = if has_uvs { self.uvs.clone() } else { Vec::new() };
        let mut colors = if has_colors { self.colors.clone() } else { Vec::new() };
        // A new vertex on every edge.
        let mut edge_vertex: HashMap<(u32, u32), u32> = HashMap::new();
        let mut keys: Vec<&(u32, u32)> = edges.keys().collect();
        keys.sort();
        for key in keys {
            let (a, b) = *key;
            let (pa, pb) = (self.positions[a as usize], self.positions[b as usize]);
            let opposite = &edges[key];
            let p = if opposite.len() == 2 {
                (pa + pb) * 0.375 + (self.positions[opposite[0] as usize] + self.positions[opposite[1] as usize]) * 0.125
            } else {
                (pa + pb) * 0.5
            };
            positions.push(p);
            if has_uvs {
                uvs.push((self.uvs[a as usize] + self.uvs[b as usize]) * 0.5);
            }
            if has_colors {
                colors.push((self.colors[a as usize] + self.colors[b as usize]) * 0.5);
            }
            edge_vertex.insert(*key, positions.len() as u32 - 1);
        }
        let mid = |a: u32, b: u32| edge_vertex[&(a.min(b), a.max(b))];
        let mut indices = Vec::with_capacity(tris.len() * 12);
        for [a, b, c] in tris {
            let (ab, bc, ca) = (mid(a, b), mid(b, c), mid(c, a));
            indices.extend([a, ab, ca, ab, b, bc, ca, bc, c, ab, bc, ca]);
        }
        let mut out = Geometry::new(positions, Vec::new(), uvs, indices);
        out.colors = colors;
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn svg_paths_parse() {
        let paths = svg_path("M0 0 L10 0 L10 10 Z m 20 0 h 5 v 5 h -5 z", 4).unwrap();
        assert_eq!(paths.len(), 2);
        assert_eq!(paths[0].len(), 3);
        assert_eq!(paths[1], vec![Vec2::new(20.0, 0.0), Vec2::new(25.0, 0.0), Vec2::new(25.0, 5.0), Vec2::new(20.0, 5.0)]);
        // A cubic curve ends where it says.
        let c = svg_path("M0,0 C 0,10 10,10 10,0", 8).unwrap();
        assert_eq!(*c[0].last().unwrap(), Vec2::new(10.0, 0.0));
        assert_eq!(c[0].len(), 9);
        // A half circle arc of radius 5.
        let a = svg_path("M0 0 A5 5 0 0 1 10 0", 8).unwrap();
        let mid = a[0][a[0].len() / 2];
        assert!((mid - Vec2::new(5.0, 0.0)).length() - 5.0 < 0.05, "{mid}");
        assert!(svg_path("M 0 0 X 1 1", 4).is_err());
    }

    #[test]
    fn svg_shapes_have_outline_and_holes() {
        // A square with a square hole, both drawn clockwise in SVG terms.
        let shape = Shape::from_svg_path("M0 0 H10 V10 H0 Z M3 3 V7 H7 V3 Z", 0.1, 4).unwrap();
        assert_eq!(shape.holes.len(), 1);
        assert!(area(&shape.outer) > 0.0 && area(&shape.holes[0]) < 0.0);
        assert!((area(&shape.outer) - 1.0).abs() < 1e-4);
        let g = Geometry::extrude_shape(&shape, 0.2, 0.0, 1);
        assert!(g.triangle_count() > 8);
    }

    #[test]
    fn smooth_curves_pass_through_their_points() {
        let pts = [Vec2::ZERO, Vec2::new(1.0, 1.0), Vec2::new(2.0, 0.0)];
        let c = smooth_curve(&pts, 4, false);
        assert_eq!(c.len(), 9);
        assert_eq!(c[4], pts[1]);
        assert_eq!(smooth_curve(&pts, 4, true).len(), 12);
    }

    #[test]
    fn skinning_to_segments_follows_the_nearest_bone() {
        let g = Geometry::tube_with(&[Vec3::ZERO, Vec3::Y * 2.0], |_| 0.1, 8);
        let g = g.skin_to_segments(&chain_segments(2, 2.0), 2);
        let skin = g.skin.as_ref().unwrap();
        for (p, (j, w)) in g.positions.iter().zip(skin.joints.iter().zip(&skin.weights)) {
            assert!((w.iter().sum::<f32>() - 1.0).abs() < 1e-4);
            let main = if w[0] >= w[1] { j[0] } else { j[1] };
            if p.y < 0.8 {
                assert_eq!(main, 0);
            }
            if p.y > 1.2 {
                assert_eq!(main, 1);
            }
        }
    }

    #[test]
    fn a_bone_chain_skins_and_bends() {
        let mut scene = Scene::new();
        let root = scene.add(crate::Node::group());
        let joints = add_bone_chain(&mut scene, root, 4, 2.0);
        let skin = Skin::from_rest(&scene, joints.clone(), Mat4::IDENTITY);
        // At rest the inverse bind matrices undo the joints: identity overall.
        for (j, ibm) in joints.iter().zip(skin.inverse_bind_matrices.iter()) {
            assert!((scene.world_matrix(*j) * *ibm).abs_diff_eq(Mat4::IDENTITY, 1e-5));
        }
    }

    #[test]
    fn merge_keeps_skins_and_morph_targets() {
        use crate::geometry::MorphTarget;
        let mut a = Geometry::cuboid(1.0, 1.0, 1.0).skin_to_segments(&[(Vec3::ZERO, Vec3::Y)], 1);
        let n = a.positions.len();
        a.morph_targets.push(MorphTarget { name: Some("grow".into()), positions: vec![Vec3::Y; n], normals: Vec::new() });
        let b = Geometry::sphere(0.5, 8, 6);
        let m = b.positions.len();
        let g = Geometry::merge([a, b]);
        assert_eq!(g.skin.as_ref().unwrap().weights.len(), n + m);
        assert_eq!(g.morph_targets.len(), 1);
        assert_eq!(g.morph_targets[0].positions.len(), n + m);
        assert_eq!(g.morph_targets[0].positions[n], Vec3::ZERO);
        // And smoothing keeps them too.
        let s = g.smooth(30.0);
        assert!(s.skin.is_some() && s.morph_targets.len() == 1);
        assert_eq!(s.morph_targets[0].positions.len(), s.positions.len());
    }

    #[test]
    fn morph_normals_follow_the_moved_surface() {
        use crate::geometry::MorphTarget;
        let mut g = Geometry::plane(1.0, 1.0);
        // Tilt the plane by lifting one edge in the target.
        let positions = g.positions.iter().map(|p| Vec3::Z * (p.x + 0.5)).collect();
        g.morph_targets.push(MorphTarget { name: None, positions, normals: Vec::new() });
        g.compute_morph_normals();
        let n = g.normals[0] + g.morph_targets[0].normals[0];
        assert!(n.x < -0.5, "{n}");
    }

    #[test]
    fn deformers_move_vertices() {
        let g = Geometry::cylinder(0.1, 0.1, 2.0, 8).translate(0.0, 1.0, 0.0);
        let top = |g: &Geometry| g.positions.iter().copied().fold(Vec3::splat(f32::MIN), Vec3::max);
        // Bent a quarter turn over its length: the top comes over toward +X.
        let bent = g.clone().bend(std::f32::consts::FRAC_PI_4);
        assert!(top(&bent).x > 0.5);
        // Tapered to a point at the top.
        let horn = g.clone().taper(|y| 1.0 - y / 2.0);
        assert!(horn.positions.iter().filter(|p| p.y > 1.99).all(|p| p.x.abs() < 1e-3));
        let lumpy = Geometry::sphere(1.0, 16, 12).displace(|_, _| 0.1);
        assert!(lumpy.positions.iter().all(|p| (p.length() - 1.1).abs() < 1e-3));
        let uv = Geometry::icosahedron(1.0, 2).box_uvs(0.5);
        assert_eq!(uv.uvs.len(), uv.positions.len());
    }

    #[test]
    fn sdf_and_metaballs_make_closed_smooth_surfaces() {
        let ball = Geometry::from_sdf(|p| sdf::sphere(p, Vec3::ZERO, 1.0), Aabb::from_points([Vec3::splat(-1.3), Vec3::splat(1.3)]), 0.1);
        assert!(ball.triangle_count() > 500);
        assert!(ball.positions.iter().all(|p| (p.length() - 1.0).abs() < 0.03));
        // Normals point out.
        assert!(ball.positions.iter().zip(&ball.normals).all(|(p, n)| p.normalize().dot(*n) > 0.9));
        // Two touching metaballs fuse into one body with a neck between them.
        let blob = Geometry::metaballs(&[(Vec3::ZERO, 0.6), (Vec3::X * 1.0, 0.6)], 0.06);
        let neck = blob.positions.iter().filter(|p| (p.x - 0.5).abs() < 0.03).count();
        assert!(neck > 0, "the balls did not fuse");
        // A smooth union of a body and a limb.
        let body = Geometry::from_sdf(
            |p| sdf::smooth_union(sdf::ellipsoid(p, Vec3::ZERO, Vec3::new(0.5, 0.3, 0.3)), sdf::capsule(p, Vec3::X * 0.3, Vec3::new(1.0, -0.5, 0.0), 0.08, 0.04), 0.1),
            Aabb::from_points([Vec3::splat(-0.7), Vec3::new(1.2, 0.7, 0.7)]),
            0.05,
        );
        assert!(body.triangle_count() > 500);
    }

    /// Edges not shared by exactly two triangles, after welding by position.
    fn open_edges(g: &Geometry) -> usize {
        let w = g.weld();
        let mut count: HashMap<(u32, u32), u32> = HashMap::new();
        for t in w.indices.chunks_exact(3) {
            for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
                *count.entry((a.min(b), a.max(b))).or_default() += 1;
            }
        }
        count.values().filter(|&&c| c != 2).count()
    }

    /// Edges with an odd number of triangles: holes in the surface.
    fn holes(g: &Geometry) -> usize {
        let w = g.weld();
        let mut count: HashMap<(u32, u32), u32> = HashMap::new();
        for t in w.indices.chunks_exact(3) {
            for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
                *count.entry((a.min(b), a.max(b))).or_default() += 1;
            }
        }
        count.values().filter(|&&c| c % 2 == 1).count()
    }

    /// The share of triangles with a corner under 4 degrees: needles.
    fn needles(g: &Geometry) -> f32 {
        let bad = g
            .indices
            .chunks_exact(3)
            .filter(|t| {
                let [a, b, c] = [t[0], t[1], t[2]].map(|i| g.positions[i as usize]);
                [(a, b, c), (b, c, a), (c, a, b)].iter().any(|&(p, q, r)| (q - p).angle_between(r - p).to_degrees() < 4.0)
            })
            .count();
        bad as f32 / g.triangle_count().max(1) as f32
    }

    #[test]
    fn sdf_surfaces_have_no_needles_and_stay_closed() {
        let body = Geometry::from_sdf(
            |p| sdf::smooth_union(sdf::sphere(p, Vec3::ZERO, 0.6), sdf::capsule(p, Vec3::ZERO, Vec3::new(1.0, 0.4, 0.2), 0.2, 0.1), 0.15),
            Aabb::from_points([Vec3::splat(-0.9), Vec3::new(1.4, 0.9, 0.9)]),
            0.05,
        );
        let share = needles(&body);
        assert!(share < 0.02, "{:.1}% needle triangles", share * 100.0);
        // Snapping can pinch an edge between four triangles, but never opens a hole.
        assert_eq!(holes(&body), 0);
        assert!(open_edges(&body) < 8, "{}", open_edges(&body));
    }

    #[test]
    fn concave_extrusions_are_closed_and_subdivide_cleanly() {
        let l = [Vec2::ZERO, Vec2::new(3.0, 0.0), Vec2::new(3.0, 1.0), Vec2::new(1.0, 1.0), Vec2::new(1.0, 3.0), Vec2::new(0.0, 3.0)];
        for bevel in [0.0, 0.03] {
            let g = Geometry::extrude(&l, 0.5, bevel);
            assert_eq!(open_edges(&g), 0, "bevel {bevel}");
            // Subdivided, the L keeps its volume's sign everywhere: no fins pointing out
            // past the outline.
            let round = g.subdivide(2);
            let b = round.bounds();
            assert!(b.min.x > -0.05 && b.max.x < 3.05 && b.min.y > -0.05 && b.max.y < 3.05, "bevel {bevel}: {b:?}");
        }
    }

    #[test]
    fn loop_subdivision_rounds_a_cube() {
        let cube = Geometry::cuboid(1.0, 1.0, 1.0);
        let round = cube.subdivide(3);
        // Welded and split four times per level: 12 triangles become 12 * 4^3.
        assert_eq!(round.triangle_count(), 12 * 64);
        // The corners pull in toward a sphere; the faces bulge less.
        let r: Vec<f32> = round.positions.iter().map(|p| p.length()).collect();
        let (min, max) = (r.iter().copied().fold(f32::MAX, f32::min), r.iter().copied().fold(0.0, f32::max));
        assert!(max < 0.866 && min > 0.2, "{min} {max}");
        assert_eq!(round.normals.len(), round.positions.len());
    }
}
