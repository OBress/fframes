//! Line art over the rasterized characters, as vector strokes (like Pencil+ in 3ds Max or
//! Blender's Line Art): contours where a surface turns away from the camera, material borders
//! and hems. Every point is tested against the depth buffer, runs of visible points become
//! tapered ribbons, outer contours (against the background) are drawn thicker.

use glam::{Vec2, Vec3};

use crate::camera::Projector;
use crate::mesh::{EdgePoint, chain};
use crate::raster::{GroupSource, Raster, Surface};
use crate::shape::Path;

#[derive(Debug, Clone, Copy)]
pub struct InkStyle {
    /// Line width in metres at the drawing's depth (turned into pixels by the projection).
    pub width: f32,
    pub min_px: f32,
    pub max_px: f32,
    /// Multiplier for contours against the background.
    pub outer: f32,
    /// Lines shorter than this many pixels are dropped.
    pub min_length: f32,
    /// How much the pen pressure varies along a stroke (0 = even lines).
    pub pressure: f32,
    /// Sideways wobble of a hand-drawn line in pixels.
    pub wobble: f32,
    /// Which drawing this is: lines are redrawn (boil) when it changes, held otherwise.
    pub seed: u32,
}

impl Default for InkStyle {
    fn default() -> Self {
        InkStyle {
            width: 0.0035,
            min_px: 0.9,
            max_px: 8.0,
            outer: 1.6,
            min_length: 4.0,
            pressure: 0.35,
            wobble: 0.45,
            seed: 0,
        }
    }
}

fn hash(n: u32) -> f32 {
    let mut x = n.wrapping_mul(0x9E37_79B9) ^ 0x85EB_CA6B;
    x ^= x >> 15;
    x = x.wrapping_mul(0x2C1B_3C6D);
    x ^= x >> 12;
    x = x.wrapping_mul(0x297A_2D39);
    x ^= x >> 15;
    (x & 0x00FF_FFFF) as f32 / 0x0100_0000 as f32
}

/// Smooth 1D value noise in -1..1.
fn noise(x: f32, seed: u32) -> f32 {
    let i = x.floor();
    let f = x - i;
    let a = hash(seed ^ (i as i32 as u32).wrapping_mul(0x01F3_5A8D));
    let b = hash(seed ^ ((i as i32 + 1) as u32).wrapping_mul(0x01F3_5A8D));
    let t = f * f * (3.0 - 2.0 * f);
    (a + (b - a) * t) * 2.0 - 1.0
}

/// A 3D polyline to ink, with the outward direction of contour points.
#[derive(Debug, Clone)]
struct Curve {
    pts: Vec<Vec3>,
    /// Outward normals for contours (zero for curves on a surface).
    out: Vec<Vec3>,
    surface: usize,
    weight: f32,
    contour: bool,
    strand: bool,
    /// Contour points inside groups whose inner contours count as strands (hair).
    fine: Vec<bool>,
}

/// The inked lines of a drawing: outlines, and fine strands (inked in their own colour).
#[derive(Debug, Clone, Default)]
pub struct Inked {
    pub lines: String,
    pub strands: String,
}

/// Contours of a surface: where `n · (p - eye)` changes sign.
fn contours(s: &Surface, eye: Vec3) -> Vec<Vec<EdgePoint>> {
    let f: Vec<f32> = s
        .pos
        .iter()
        .zip(&s.nrm)
        .map(|(p, n)| n.dot(*p - eye))
        .collect();
    let mut segments = Vec::new();
    for t in s.tris.iter() {
        let v = t.map(|i| f[i as usize]);
        let mut pts = [EdgePoint::new(0, 0, 0.0); 2];
        let mut k = 0;
        for (i, j) in [(0, 1), (1, 2), (2, 0)] {
            if (v[i] < 0.0) != (v[j] < 0.0) && k < 2 {
                pts[k] = EdgePoint::new(t[i], t[j], v[i] / (v[i] - v[j]));
                k += 1;
            }
        }
        if k == 2 {
            segments.push(pts);
        }
    }
    chain(segments)
}

fn eval(p: &EdgePoint, v: &[Vec3]) -> Vec3 {
    if p.a == p.b {
        v[p.a as usize]
    } else {
        p.eval(v)
    }
}

/// Inks the lines of the surfaces that pass `only`: one SVG path (filled ribbons) in frame
/// coordinates. All surfaces still hide lines behind them.
pub fn draw(
    surfaces: &[Surface],
    raster: &Raster,
    proj: &Projector,
    style: &InkStyle,
    only: &dyn Fn(&Surface) -> bool,
) -> Inked {
    let mut curves = Vec::new();
    for (si, s) in surfaces.iter().enumerate() {
        if s.lines.weight <= 0.0 || !only(s) {
            continue;
        }
        if s.lines.contours {
            for line in contours(s, proj.eye) {
                let pts: Vec<Vec3> = line.iter().map(|p| eval(p, &s.pos)).collect();
                let out: Vec<Vec3> = line.iter().map(|p| eval(p, &s.nrm)).collect();
                let fine = line
                    .iter()
                    .map(|p| match s.groups {
                        GroupSource::Near(near) => {
                            s.lines.fine_groups.contains(&near[p.a as usize][0].0)
                        }
                        GroupSource::Fixed(g) => s.lines.fine_groups.contains(&g),
                    })
                    .collect();
                curves.push(Curve {
                    pts,
                    out,
                    surface: si,
                    weight: s.lines.weight,
                    contour: true,
                    strand: false,
                    fine,
                });
            }
        }
        for c in &s.lines.curves {
            curves.push(Curve {
                pts: c.pts.iter().map(|p| eval(p, &s.pos)).collect(),
                out: c.pts.iter().map(|p| eval(p, &s.nrm)).collect(),
                surface: si,
                weight: s.lines.weight * 0.75 * c.weight,
                contour: false,
                strand: c.strand,
                fine: Vec::new(),
            });
        }
    }
    let mut lines = Path::new();
    let mut strands = Path::new();
    for c in &curves {
        ink_curve(&mut lines, &mut strands, c, surfaces, raster, proj, style);
    }
    Inked {
        lines: lines.0,
        strands: strands.0,
    }
}

struct Point {
    p: Vec2,
    w: f32,
}

fn ink_curve(
    lines: &mut Path,
    strands: &mut Path,
    c: &Curve,
    surfaces: &[Surface],
    raster: &Raster,
    proj: &Projector,
    style: &InkStyle,
) {
    let tag = surfaces[c.surface].tag;
    let mut run: Vec<Point> = Vec::new();
    let mut run_strand = c.strand;
    // strands are calm flicks: longer, no wobble, smoothed (see `emit_strand`)
    let strand_style = InkStyle {
        min_length: style.min_length * 8.0,
        wobble: 0.0,
        pressure: style.pressure * 0.5,
        ..*style
    };
    let mut flush = |run: &mut Vec<Point>, strand: bool| {
        if strand {
            emit_strand(strands, run, &strand_style);
        } else {
            emit(lines, run, style);
        }
        run.clear();
    };
    for (i, p3) in c.pts.iter().enumerate() {
        let (p, z) = proj.project(*p3);
        let scale = proj.focal / z;
        let mut raw = style.width * c.weight * scale;
        let mut w = raw.clamp(style.min_px, style.max_px);
        let mut visible;
        let mut strand = c.strand;
        if c.contour {
            // which way is outside, on screen
            let n = c.out[i];
            let (q, _) = proj.project(*p3 + n * (2.0 / scale));
            let out = (q - p).normalize_or_zero();
            visible = sees(raster, p - out * 0.5, z, c.surface, true)
                || sees(raster, p + out * 0.75, z, c.surface, true);
            if visible {
                let beyond = raster.samples(p + out * (w + 1.5));
                let background = beyond.iter().all(|s| s.is_none_or(|(_, t, _)| t != tag));
                if background {
                    w *= style.outer;
                } else if c.fine.get(i).copied().unwrap_or(false) {
                    // a contour inside the hair: a strand
                    strand = true;
                    raw *= 0.6;
                    w = raw.clamp(style.min_px, style.max_px);
                }
            }
        } else {
            // seams on the side of a surface turned away are hidden by it
            let n = c.out[i];
            let faces = n == Vec3::ZERO || n.dot(proj.view(*p3)) > 0.08;
            visible = faces && sees(raster, p, z, c.surface, false);
        }
        // fine lines drop out when the drawing is small, like an animator would leave them out
        if p.x.is_nan() || (strand && raw < 1.8) {
            visible = false;
        }
        if visible && strand != run_strand && !run.is_empty() {
            let last = run.last().map(|l| Point { p: l.p, w: l.w });
            flush(&mut run, run_strand);
            // keep the line continuous across the change
            if let Some(l) = last {
                run.push(l);
            }
        }
        run_strand = strand;
        if visible {
            if let Some(last) = run.last()
                && (last.p - p).length_squared() < 0.6
            {
                continue;
            }
            run.push(Point { p, w });
        } else if !run.is_empty() {
            flush(&mut run, run_strand);
        }
    }
    flush(&mut run, run_strand);
}

/// Whether a point at depth `z` on `surface` shows at frame position `p`.
fn sees(raster: &Raster, p: Vec2, z: f32, surface: usize, contour: bool) -> bool {
    let samples = raster.samples(p);
    samples.iter().any(|s| match s {
        None => true,
        Some((si, _, d)) => {
            let bias = if *si == surface {
                if contour { 0.025 } else { 0.006 }
            } else {
                0.004
            };
            *d >= z - bias - z * 0.002
        }
    })
}

/// A strand inside the hair: contours on the mesh step from edge to edge, which inked as is reads
/// as a crack. The run is relaxed into a smooth curve first, and dropped when it still zig-zags
/// (an animator draws a few clean flicks, not every fold).
fn emit_strand(path: &mut Path, run: &[Point], style: &InkStyle) {
    if run.len() < 4 {
        return;
    }
    let mut pts: Vec<Vec2> = run.iter().map(|p| p.p).collect();
    for _ in 0..6 {
        let prev = pts.clone();
        for i in 1..pts.len() - 1 {
            pts[i] = prev[i] * 0.5 + (prev[i - 1] + prev[i + 1]) * 0.25;
        }
    }
    let length: f32 = pts.windows(2).map(|w| (w[1] - w[0]).length()).sum();
    // total turning per pixel of length: a clean flick bends gently and one way
    let mut turning = 0.0;
    for w in pts.windows(3) {
        let a = (w[1] - w[0]).normalize_or_zero();
        let b = (w[2] - w[1]).normalize_or_zero();
        turning += a.perp_dot(b).abs();
    }
    if length < 1.0 || turning / length > 0.035 {
        return;
    }
    let smoothed: Vec<Point> = pts
        .iter()
        .zip(run)
        .map(|(p, r)| Point { p: *p, w: r.w })
        .collect();
    emit(path, &smoothed, style);
}

/// A tapered ribbon through the run.
fn emit(path: &mut Path, run: &[Point], style: &InkStyle) {
    if run.len() < 2 {
        return;
    }
    let pts: Vec<Vec2> = chaikin(&run.iter().map(|p| p.p).collect::<Vec<_>>());
    let ws: Vec<f32> = chaikin_w(&run.iter().map(|p| p.w).collect::<Vec<_>>());
    let mut length = 0.0;
    let mut along = Vec::with_capacity(pts.len());
    along.push(0.0);
    for i in 1..pts.len() {
        length += (pts[i] - pts[i - 1]).length();
        along.push(length);
    }
    if length < style.min_length {
        return;
    }
    let closed = (pts[0] - pts[pts.len() - 1]).length() < 1.0 && pts.len() > 4;
    let wmax = ws.iter().copied().fold(0.0, f32::max);
    let taper = (wmax * 4.0).min(length * 0.3).max(1.0);
    // Every stroke gets its own pen: a seed from where it starts (stable while the drawing is
    // held) and the drawing number (a new pen when the drawing changes).
    let q = (pts[0] / 24.0).floor();
    let stroke_seed = style.seed.wrapping_mul(0x27D4_EB2F)
        ^ (q.x as i32 as u32).wrapping_mul(0x1656_67B1)
        ^ (q.y as i32 as u32).wrapping_mul(0x9E37_79B9);
    let mut left = Vec::with_capacity(pts.len());
    let mut right = Vec::with_capacity(pts.len());
    for i in 0..pts.len() {
        let prev = pts[i.saturating_sub(1)];
        let next = pts[(i + 1).min(pts.len() - 1)];
        let d = (next - prev).normalize_or(Vec2::X);
        let n = Vec2::new(-d.y, d.x);
        // pen pressure: slow swells along the stroke, heavier where it bends
        let bend = if i > 0 && i + 1 < pts.len() {
            let a = (pts[i] - prev).normalize_or_zero();
            let b = (next - pts[i]).normalize_or_zero();
            (1.0 - a.dot(b)).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let swell = 1.0
            + style.pressure * noise(along[i] / 55.0, stroke_seed)
            + bend * 1.5 * style.pressure;
        let mut w = ws[i] * 0.5 * swell.max(0.3);
        if !closed {
            let e = along[i].min(length - along[i]);
            let k = (e / taper).clamp(0.0, 1.0);
            w *= 0.2 + 0.8 * k * (2.0 - k);
        }
        let wob = style.wobble
            * noise(along[i] / 38.0, stroke_seed ^ 0x5bd1_e995)
            * (ws[i] / 2.0).clamp(0.5, 1.5);
        let p = pts[i] + n * wob;
        left.push(p + n * w);
        right.push(p - n * w);
    }
    right.reverse();
    left.extend(right);
    path.polygon(&left);
}

fn chaikin(p: &[Vec2]) -> Vec<Vec2> {
    if p.len() < 3 {
        return p.to_vec();
    }
    let mut out = Vec::with_capacity(p.len() * 2);
    out.push(p[0]);
    for w in p.windows(2) {
        out.push(w[0].lerp(w[1], 0.25));
        out.push(w[0].lerp(w[1], 0.75));
    }
    out.push(p[p.len() - 1]);
    out
}

fn chaikin_w(w: &[f32]) -> Vec<f32> {
    if w.len() < 3 {
        return w.to_vec();
    }
    let mut out = Vec::with_capacity(w.len() * 2);
    out.push(w[0]);
    for p in w.windows(2) {
        out.push(p[0] * 0.75 + p[1] * 0.25);
        out.push(p[0] * 0.25 + p[1] * 0.75);
    }
    out.push(w[w.len() - 1]);
    out
}
