//! A cel-shading rasterizer for the character meshes.
//!
//! Triangles go into a depth buffer with four samples per pixel; every pixel is then shaded once
//! per triangle that covers it (deferred, like MSAA), by a closure that knows the materials, so
//! painted details (eyes, seams, buttons) are evaluated per pixel in the rest pose and stay
//! crisp at any size. The buffer also answers depth queries for the line art drawn on top.

use std::borrow::Cow;

use glam::{Vec2, Vec3};

use crate::camera::Projector;
use crate::mesh::EdgePoint;

/// Where the material group of a pixel comes from.
#[derive(Debug, Clone, Copy)]
pub enum GroupSource<'a> {
    Fixed(u8),
    /// The nearest groups of every vertex (from [`crate::mesh::Mesh::near`]).
    Near(&'a [[(u8, f32); 3]]),
}

/// Lines a surface contributes to the line art.
#[derive(Debug, Clone, Default)]
pub struct LineSource<'a> {
    /// Contours (where the surface turns away from the camera).
    pub contours: bool,
    /// Curves on the surface (material borders, hems) as points on mesh edges.
    pub curves: Vec<CurveRef<'a>>,
    /// Width multiplier.
    pub weight: f32,
    /// Groups whose contours inside the drawing are fine strands (hair).
    pub fine_groups: &'static [u8],
}

/// A curve on a surface for the line art.
#[derive(Debug, Clone)]
pub struct CurveRef<'a> {
    pub pts: Cow<'a, [EdgePoint]>,
    /// Width multiplier.
    pub weight: f32,
    /// A fine detail line (strands of hair): dropped when it would be thinner than a pixel and
    /// inked separately, so it can take a colour that shows on dark hair.
    pub strand: bool,
}

impl<'a> CurveRef<'a> {
    pub fn new(pts: Cow<'a, [EdgePoint]>) -> Self {
        CurveRef {
            pts,
            weight: 1.0,
            strand: false,
        }
    }
}

/// A mesh posed for one frame.
#[derive(Debug, Clone)]
pub struct Surface<'a> {
    pub pos: Vec<Vec3>,
    pub nrm: Vec<Vec3>,
    pub tris: Cow<'a, [[u32; 3]]>,
    /// Rest pose positions for painted details.
    pub rest: Option<&'a [Vec3]>,
    pub groups: GroupSource<'a>,
    /// Two sided: back faces are drawn with this group.
    pub back: Option<u8>,
    /// Identifies the character (and part) for the shader and the line art.
    pub tag: u32,
    pub lines: LineSource<'a>,
}

/// What the shader gets for one pixel of one triangle.
#[derive(Debug, Clone, Copy)]
pub struct Fragment {
    pub surface: usize,
    pub tag: u32,
    pub group: u8,
    /// The back side of a two-sided surface.
    pub back: bool,
    pub pos: Vec3,
    /// Facing the camera (flipped on back sides).
    pub normal: Vec3,
    pub rest: Vec3,
    /// Unit vector to the eye.
    pub view: Vec3,
    /// Metres per pixel at this depth.
    pub px: f32,
}

/// Rows per band of the parallel rasterizer.
const BAND: usize = 32;

const SAMPLES: [Vec2; 4] = [
    Vec2::new(0.375, 0.125),
    Vec2::new(0.875, 0.375),
    Vec2::new(0.125, 0.625),
    Vec2::new(0.625, 0.875),
];

/// A depth buffer over a rectangle of the frame.
pub struct Raster {
    pub x0: i32,
    pub y0: i32,
    pub w: usize,
    pub h: usize,
    /// 1 / depth per sample, 0 when empty.
    inv_z: Vec<f32>,
    /// Global triangle index + 1 per sample, 0 when empty.
    id: Vec<u32>,
    screen: Vec<Vec<Vec2>>,
    depth: Vec<Vec<f32>>,
    offsets: Vec<u32>,
    tags: Vec<u32>,
    focal: f32,
}

#[inline]
fn edge(a: Vec2, b: Vec2, p: Vec2) -> f32 {
    (b.x - a.x) * (p.y - a.y) - (b.y - a.y) * (p.x - a.x)
}

/// Rasterizes one (already culled) triangle into the band of rows `rows` of a buffer `w` pixels
/// wide: `inv_z` and `id` hold just those rows.
fn triangle(
    p: [Vec2; 3],
    z: [f32; 3],
    tri: u32,
    w: usize,
    rows: std::ops::Range<i32>,
    inv_z: &mut [f32],
    id: &mut [u32],
) {
    let area = edge(p[0], p[1], p[2]);
    let lo = p[0].min(p[1]).min(p[2]);
    let hi = p[0].max(p[1]).max(p[2]);
    let x0 = (lo.x.floor() as i32).max(0);
    let y0 = (lo.y.floor() as i32).max(rows.start);
    let x1 = (hi.x.ceil() as i32).min(w as i32 - 1);
    let y1 = (hi.y.ceil() as i32).min(rows.end - 1);
    if x0 > x1 || y0 > y1 {
        return;
    }
    let inv_area = 1.0 / area;
    let iz = [1.0 / z[0], 1.0 / z[1], 1.0 / z[2]];
    for y in y0..=y1 {
        for x in x0..=x1 {
            let base = ((y - rows.start) as usize * w + x as usize) * 4;
            for (s, off) in SAMPLES.iter().enumerate() {
                let q = Vec2::new(x as f32, y as f32) + *off;
                let b0 = edge(p[1], p[2], q) * inv_area;
                let b1 = edge(p[2], p[0], q) * inv_area;
                let b2 = 1.0 - b0 - b1;
                if b0 < 0.0 || b1 < 0.0 || b2 < 0.0 {
                    continue;
                }
                let inv = b0 * iz[0] + b1 * iz[1] + b2 * iz[2];
                if inv > inv_z[base + s] {
                    inv_z[base + s] = inv;
                    id[base + s] = tri;
                }
            }
        }
    }
}

impl Raster {
    /// Rasterizes the surfaces inside the rectangle `x0, y0, w, h` (frame pixels). Closed
    /// surfaces are drawn front faces only.
    ///
    /// The buffer is split into bands of rows rasterized in parallel, each with the triangles
    /// binned into it, in their original order (the result does not depend on the threads).
    pub fn new(
        x0: i32,
        y0: i32,
        w: usize,
        h: usize,
        surfaces: &[Surface],
        proj: &Projector,
    ) -> Self {
        use rayon::prelude::*;
        let origin = Vec2::new(x0 as f32, y0 as f32);
        let projected: Vec<(Vec<Vec2>, Vec<f32>)> = surfaces
            .par_iter()
            .map(|s| {
                s.pos
                    .iter()
                    .map(|p| {
                        let (q, z) = proj.project(*p);
                        (q - origin, z)
                    })
                    .unzip()
            })
            .collect();
        let mut offsets = Vec::with_capacity(surfaces.len());
        let mut offset = 0_u32;
        for s in surfaces {
            offsets.push(offset);
            offset += s.tris.len() as u32;
        }
        let bands = h.div_ceil(BAND).max(1);
        // bin every visible triangle into the bands of rows it covers
        let mut bins: Vec<Vec<(u32, u32)>> = vec![Vec::new(); bands];
        for (si, s) in surfaces.iter().enumerate() {
            let (screen, depth) = &projected[si];
            let cull = s.back.is_none() && proj.flatten.is_none();
            for (ti, t) in s.tris.iter().enumerate() {
                let z = [
                    depth[t[0] as usize],
                    depth[t[1] as usize],
                    depth[t[2] as usize],
                ];
                if z.iter().any(|z| *z <= 0.02) {
                    continue;
                }
                let p = [
                    screen[t[0] as usize],
                    screen[t[1] as usize],
                    screen[t[2] as usize],
                ];
                let area = edge(p[0], p[1], p[2]);
                if area.abs() < 1e-9 || (cull && area > 0.0) {
                    continue;
                }
                let lo = p[0].min(p[1]).min(p[2]);
                let hi = p[0].max(p[1]).max(p[2]);
                if hi.x < 0.0 || lo.x > w as f32 || hi.y < 0.0 || lo.y > h as f32 {
                    continue;
                }
                let b0 = (lo.y.floor().max(0.0) as usize / BAND).min(bands - 1);
                let b1 = (hi.y.ceil().max(0.0) as usize / BAND).min(bands - 1);
                for bin in &mut bins[b0..=b1] {
                    bin.push((si as u32, ti as u32));
                }
            }
        }
        let mut inv_z = vec![0.0_f32; w * h * 4];
        let mut id = vec![0_u32; w * h * 4];
        inv_z
            .par_chunks_mut(BAND * w * 4)
            .zip(id.par_chunks_mut(BAND * w * 4))
            .enumerate()
            .for_each(|(b, (inv_z, id))| {
                let rows = (b * BAND) as i32..((b * BAND + BAND).min(h)) as i32;
                for &(si, ti) in &bins[b] {
                    let s = &surfaces[si as usize];
                    let t = s.tris[ti as usize];
                    let (screen, depth) = &projected[si as usize];
                    triangle(
                        [
                            screen[t[0] as usize],
                            screen[t[1] as usize],
                            screen[t[2] as usize],
                        ],
                        [
                            depth[t[0] as usize],
                            depth[t[1] as usize],
                            depth[t[2] as usize],
                        ],
                        offsets[si as usize] + ti + 1,
                        w,
                        rows.clone(),
                        inv_z,
                        id,
                    );
                }
            });
        let (screen, depth) = projected.into_iter().unzip();
        Raster {
            x0,
            y0,
            w,
            h,
            inv_z,
            id,
            screen,
            depth,
            offsets,
            tags: surfaces.iter().map(|s| s.tag).collect(),
            focal: proj.focal,
        }
    }

    fn locate(&self, id: u32) -> (usize, usize) {
        let g = id - 1;
        let s = self.offsets.partition_point(|o| *o <= g) - 1;
        (s, (g - self.offsets[s]) as usize)
    }

    /// Shades every covered pixel: `shader` returns an opaque RGB colour (0..1). The result is
    /// premultiplied RGBA, `w * h * 4` bytes.
    ///
    /// Rows are shaded in parallel: a face in close-up is millions of shaded pixels.
    pub fn resolve(
        &self,
        surfaces: &[Surface],
        proj: &Projector,
        shader: &(dyn Fn(&Fragment) -> Vec3 + Sync),
    ) -> Vec<u8> {
        use rayon::prelude::*;
        let mut out = vec![0_u8; self.w * self.h * 4];
        let origin = Vec2::new(self.x0 as f32, self.y0 as f32);
        let row_bytes = self.w * 4;
        out.par_chunks_mut(row_bytes)
            .enumerate()
            .for_each(|(y, row)| {
                for x in 0..self.w {
                    let base = (y * self.w + x) * 4;
                    let ids = &self.id[base..base + 4];
                    if ids.iter().all(|i| *i == 0) {
                        continue;
                    }
                    let mut color = Vec3::ZERO;
                    let mut alpha = 0.0;
                    let mut done = [false; 4];
                    for i in 0..4 {
                        if done[i] || ids[i] == 0 {
                            continue;
                        }
                        let id = ids[i];
                        let mut count = 0.0;
                        let mut centroid = Vec2::ZERO;
                        for j in i..4 {
                            if ids[j] == id {
                                done[j] = true;
                                count += 1.0;
                                centroid += SAMPLES[j];
                            }
                        }
                        centroid = centroid / count + Vec2::new(x as f32, y as f32);
                        let frag = self.fragment(surfaces, proj, id, centroid, origin);
                        color += shader(&frag) * (count * 0.25);
                        alpha += count * 0.25;
                    }
                    let px = &mut row[x * 4..x * 4 + 4];
                    px[0] = (color.x.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
                    px[1] = (color.y.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
                    px[2] = (color.z.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
                    px[3] = (alpha * 255.0 + 0.5) as u8;
                }
            });
        out
    }

    fn fragment(
        &self,
        surfaces: &[Surface],
        proj: &Projector,
        id: u32,
        q: Vec2,
        _origin: Vec2,
    ) -> Fragment {
        let (si, ti) = self.locate(id);
        let s = &surfaces[si];
        let t = s.tris[ti];
        let sc = &self.screen[si];
        let dz = &self.depth[si];
        let (a, b, c) = (sc[t[0] as usize], sc[t[1] as usize], sc[t[2] as usize]);
        let area = edge(a, b, c);
        let back = area > 0.0;
        let mut w = [edge(b, c, q) / area, edge(c, a, q) / area, 0.0];
        w[2] = 1.0 - w[0] - w[1];
        // perspective correct
        let pw = [
            w[0] / dz[t[0] as usize],
            w[1] / dz[t[1] as usize],
            w[2] / dz[t[2] as usize],
        ];
        let sum = pw[0] + pw[1] + pw[2];
        let bw = pw.map(|v| (v / sum).clamp(-0.5, 1.5));
        let lerp3 = |v: &[Vec3]| {
            v[t[0] as usize] * bw[0] + v[t[1] as usize] * bw[1] + v[t[2] as usize] * bw[2]
        };
        let pos = lerp3(&s.pos);
        let mut normal = lerp3(&s.nrm).normalize_or(Vec3::Y);
        let view = proj.view(pos);
        let group = match (back, s.back, s.groups) {
            (true, Some(g), _) | (_, _, GroupSource::Fixed(g)) => g,
            (_, _, GroupSource::Near(near)) => {
                // the candidate group with the smallest interpolated distance
                let mut best = (near[t[0] as usize][0].0, f32::INFINITY);
                for v in t {
                    let g = near[v as usize][0].0;
                    let d: f32 = t
                        .iter()
                        .zip(bw)
                        .map(|(u, wt)| {
                            let n = &near[*u as usize];
                            let d = n
                                .iter()
                                .find(|(ng, _)| *ng == g)
                                .map_or(n[2].1.max(n[0].1 + 0.01), |(_, d)| *d);
                            d * wt
                        })
                        .sum();
                    if d < best.1 {
                        best = (g, d);
                    }
                }
                best.0
            }
        };
        if back {
            normal = -normal;
        }
        let rest = s.rest.map_or(Vec3::ZERO, lerp3);
        let depth = 1.0
            / (bw[0] / dz[t[0] as usize] + bw[1] / dz[t[1] as usize] + bw[2] / dz[t[2] as usize])
                .max(1e-6);
        Fragment {
            surface: si,
            tag: s.tag,
            group,
            back,
            pos,
            normal,
            rest,
            view,
            px: depth.max(0.01) / self.focal,
        }
    }

    /// The four samples of the pixel at a frame position: (surface, tag, depth) of each, `None`
    /// where empty or outside the buffer.
    pub fn samples(&self, p: Vec2) -> [Option<(usize, u32, f32)>; 4] {
        let x = (p.x - self.x0 as f32).floor() as i64;
        let y = (p.y - self.y0 as f32).floor() as i64;
        if x < 0 || y < 0 || x >= self.w as i64 || y >= self.h as i64 {
            return [None; 4];
        }
        let base = (y as usize * self.w + x as usize) * 4;
        std::array::from_fn(|i| {
            let id = self.id[base + i];
            if id == 0 {
                None
            } else {
                let (s, _) = self.locate(id);
                Some((s, self.tags[s], 1.0 / self.inv_z[base + i]))
            }
        })
    }
}

/// The bounding rectangle of the projected surfaces, padded and clipped to the frame.
pub fn bounds(
    surfaces: &[Surface],
    proj: &Projector,
    width: f32,
    height: f32,
    pad: f32,
) -> Option<(i32, i32, usize, usize)> {
    let mut lo = Vec2::splat(f32::INFINITY);
    let mut hi = Vec2::splat(f32::NEG_INFINITY);
    for s in surfaces {
        for p in &s.pos {
            let (q, z) = proj.project(*p);
            if z > 0.02 {
                lo = lo.min(q);
                hi = hi.max(q);
            }
        }
    }
    let x0 = (lo.x - pad).floor().max(0.0);
    let y0 = (lo.y - pad).floor().max(0.0);
    let x1 = (hi.x + pad).ceil().min(width);
    let y1 = (hi.y + pad).ceil().min(height);
    if !(x1 > x0 && y1 > y0) {
        return None;
    }
    Some((x0 as i32, y0 as i32, (x1 - x0) as usize, (y1 - y0) as usize))
}
