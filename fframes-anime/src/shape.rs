//! 2D outlines for flat toon shapes, written as SVG path data: capsules for limbs, convex hulls
//! for heads and torsos, ribbons for hair strands and smooth curves through points.

use std::f32::consts::{PI, TAU};
use std::fmt::Write;

use glam::Vec2;

/// Path data builder with one decimal per coordinate.
#[derive(Debug, Clone, Default)]
pub struct Path(pub String);

impl Path {
    pub fn new() -> Self {
        Path(String::with_capacity(256))
    }

    pub fn move_to(&mut self, p: Vec2) -> &mut Self {
        let _ = write!(self.0, "M{:.1} {:.1}", p.x, p.y);
        self
    }

    pub fn line_to(&mut self, p: Vec2) -> &mut Self {
        let _ = write!(self.0, "L{:.1} {:.1}", p.x, p.y);
        self
    }

    pub fn cubic_to(&mut self, c1: Vec2, c2: Vec2, p: Vec2) -> &mut Self {
        let _ = write!(
            self.0,
            "C{:.1} {:.1} {:.1} {:.1} {:.1} {:.1}",
            c1.x, c1.y, c2.x, c2.y, p.x, p.y
        );
        self
    }

    pub fn quad_to(&mut self, c: Vec2, p: Vec2) -> &mut Self {
        let _ = write!(self.0, "Q{:.1} {:.1} {:.1} {:.1}", c.x, c.y, p.x, p.y);
        self
    }

    pub fn close(&mut self) -> &mut Self {
        self.0.push('Z');
        self
    }

    /// A closed polygon. Closed shapes always wind the same way, so overlapping subpaths of one
    /// path add up under the nonzero fill rule instead of cutting holes.
    pub fn polygon(&mut self, pts: &[Vec2]) -> &mut Self {
        let pts = &oriented(pts);
        if let Some((first, rest)) = pts.split_first() {
            self.move_to(*first);
            for p in rest {
                self.line_to(*p);
            }
            self.close();
        }
        self
    }

    pub fn polyline(&mut self, pts: &[Vec2]) -> &mut Self {
        if let Some((first, rest)) = pts.split_first() {
            self.move_to(*first);
            for p in rest {
                self.line_to(*p);
            }
        }
        self
    }

    /// A closed Catmull-Rom spline through the points (oriented like [`Path::polygon`]).
    pub fn smooth_closed(&mut self, pts: &[Vec2], tension: f32) -> &mut Self {
        let pts = &oriented(pts);
        let n = pts.len();
        if n < 3 {
            return self.polygon(pts);
        }
        self.move_to(pts[0]);
        for i in 0..n {
            let p0 = pts[(i + n - 1) % n];
            let p1 = pts[i];
            let p2 = pts[(i + 1) % n];
            let p3 = pts[(i + 2) % n];
            let c1 = p1 + (p2 - p0) * (tension / 6.0);
            let c2 = p2 - (p3 - p1) * (tension / 6.0);
            self.cubic_to(c1, c2, p2);
        }
        self.close()
    }

    /// An open Catmull-Rom spline through the points.
    pub fn smooth_open(&mut self, pts: &[Vec2], tension: f32) -> &mut Self {
        let n = pts.len();
        if n < 3 {
            return self.polyline(pts);
        }
        self.move_to(pts[0]);
        for i in 0..n - 1 {
            let p0 = pts[i.saturating_sub(1)];
            let p1 = pts[i];
            let p2 = pts[i + 1];
            let p3 = pts[(i + 2).min(n - 1)];
            let c1 = p1 + (p2 - p0) * (tension / 6.0);
            let c2 = p2 - (p3 - p1) * (tension / 6.0);
            self.cubic_to(c1, c2, p2);
        }
        self
    }

    pub fn circle(&mut self, c: Vec2, r: f32) -> &mut Self {
        let _ = write!(
            self.0,
            "M{:.1} {:.1}a{r:.1} {r:.1} 0 1 0 {:.1} 0a{r:.1} {r:.1} 0 1 0 {:.1} 0Z",
            c.x - r,
            c.y,
            2.0 * r,
            -2.0 * r,
        );
        self
    }

    pub fn append(&mut self, other: &Path) -> &mut Self {
        self.0.push_str(&other.0);
        self
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn build(self) -> String {
        self.0
    }
}

/// Twice the signed area of a polygon (positive = clockwise on screen, y down).
pub fn signed_area(pts: &[Vec2]) -> f32 {
    let n = pts.len();
    (0..n).map(|i| pts[i].perp_dot(pts[(i + 1) % n])).sum()
}

/// The polygon wound clockwise on screen.
pub fn oriented(pts: &[Vec2]) -> Vec<Vec2> {
    let mut v = pts.to_vec();
    if signed_area(&v) < 0.0 {
        v.reverse();
    }
    v
}

/// Points of an arc around `c` from angle `a0` to `a1` (radians, y down).
pub fn arc_points(
    c: Vec2,
    r: f32,
    a0: f32,
    a1: f32,
    segments: usize,
) -> impl Iterator<Item = Vec2> {
    (0..=segments).map(move |i| {
        let a = a0 + (a1 - a0) * i as f32 / segments as f32;
        c + Vec2::new(a.cos(), a.sin()) * r
    })
}

/// Outline of two circles joined by their outer tangents: a limb from `a` (radius `ra`) to `b`
/// (radius `rb`).
pub fn capsule(a: Vec2, ra: f32, b: Vec2, rb: f32) -> Vec<Vec2> {
    let d = b - a;
    let len = d.length();
    let seg = |r: f32| ((r * 0.6).clamp(4.0, 24.0)) as usize;
    if len <= (ra - rb).abs() + 1e-3 {
        let (c, r) = if ra >= rb { (a, ra) } else { (b, rb) };
        return arc_points(c, r, 0.0, TAU, seg(r) * 2).collect();
    }
    let u = d / len;
    let s = ((ra - rb) / len).clamp(-1.0, 1.0);
    let phi = s.acos(); // angle between u and the tangent normals
    let base = u.y.atan2(u.x);
    let mut pts = Vec::with_capacity(seg(ra) + seg(rb) + 2);
    // Cap around b: from base - phi to base + phi through base.
    pts.extend(arc_points(b, rb, base - phi, base + phi, seg(rb)));
    // Cap around a: from base + phi to base + 2PI - phi through base + PI.
    pts.extend(arc_points(a, ra, base + phi, base + TAU - phi, seg(ra)));
    pts
}

/// A tapered ribbon along a polyline: `widths` are full widths at each point.
pub fn ribbon(pts: &[Vec2], widths: &[f32]) -> Vec<Vec2> {
    let n = pts.len();
    if n < 2 {
        return Vec::new();
    }
    let mut left = Vec::with_capacity(n);
    let mut right = Vec::with_capacity(n);
    for i in 0..n {
        let dir = if i == 0 {
            pts[1] - pts[0]
        } else if i == n - 1 {
            pts[n - 1] - pts[n - 2]
        } else {
            pts[i + 1] - pts[i - 1]
        }
        .normalize_or(Vec2::Y);
        let normal = Vec2::new(-dir.y, dir.x);
        let w = widths.get(i).copied().unwrap_or(0.0) * 0.5;
        left.push(pts[i] + normal * w);
        right.push(pts[i] - normal * w);
    }
    right.reverse();
    left.extend(right);
    left
}

/// Convex hull (counter-clockwise in a y-up sense, monotone chain).
pub fn convex_hull(points: &[Vec2]) -> Vec<Vec2> {
    let mut pts: Vec<Vec2> = points.to_vec();
    if pts.len() < 3 {
        return pts;
    }
    pts.sort_by(|a, b| a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y)));
    let cross = |o: Vec2, a: Vec2, b: Vec2| (a - o).perp_dot(b - o);
    let mut lower: Vec<Vec2> = Vec::new();
    for p in &pts {
        while lower.len() >= 2 && cross(lower[lower.len() - 2], lower[lower.len() - 1], *p) <= 0.0 {
            lower.pop();
        }
        lower.push(*p);
    }
    let mut upper: Vec<Vec2> = Vec::new();
    for p in pts.iter().rev() {
        while upper.len() >= 2 && cross(upper[upper.len() - 2], upper[upper.len() - 1], *p) <= 0.0 {
            upper.pop();
        }
        upper.push(*p);
    }
    lower.pop();
    upper.pop();
    lower.extend(upper);
    lower
}

/// Points of an ellipse.
pub fn ellipse(c: Vec2, rx: f32, ry: f32, rotation: f32, segments: usize) -> Vec<Vec2> {
    let (s, co) = rotation.sin_cos();
    (0..segments)
        .map(|i| {
            let a = i as f32 / segments as f32 * TAU;
            let v = Vec2::new(a.cos() * rx, a.sin() * ry);
            c + Vec2::new(v.x * co - v.y * s, v.x * s + v.y * co)
        })
        .collect()
}

/// A star/burst polygon with `n` points.
pub fn star(c: Vec2, outer: f32, inner: f32, n: usize, rotation: f32) -> Vec<Vec2> {
    (0..n * 2)
        .map(|i| {
            let r = if i % 2 == 0 { outer } else { inner };
            let a = rotation + i as f32 * PI / n as f32 - PI / 2.0;
            c + Vec2::new(a.cos(), a.sin()) * r
        })
        .collect()
}

/// Rounds a polygon's corners by cutting each corner with a quadratic curve.
pub fn rounded(pts: &[Vec2], radius: f32) -> Path {
    let n = pts.len();
    let mut path = Path::new();
    if n < 3 {
        path.polygon(pts);
        return path;
    }
    for i in 0..n {
        let prev = pts[(i + n - 1) % n];
        let cur = pts[i];
        let next = pts[(i + 1) % n];
        let r1 = radius.min((cur - prev).length() * 0.5);
        let r2 = radius.min((next - cur).length() * 0.5);
        let a = cur + (prev - cur).normalize_or_zero() * r1;
        let b = cur + (next - cur).normalize_or_zero() * r2;
        if i == 0 {
            path.move_to(a);
        } else {
            path.line_to(a);
        }
        path.quad_to(cur, b);
    }
    path.close();
    path
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hull_of_square_with_center() {
        let h = convex_hull(&[
            Vec2::new(0.0, 0.0),
            Vec2::new(1.0, 0.0),
            Vec2::new(1.0, 1.0),
            Vec2::new(0.0, 1.0),
            Vec2::new(0.5, 0.5),
        ]);
        assert_eq!(h.len(), 4);
    }

    #[test]
    fn capsule_encloses_its_ends() {
        let pts = capsule(Vec2::new(0.0, 0.0), 10.0, Vec2::new(100.0, 0.0), 5.0);
        let max_x = pts.iter().map(|p| p.x).fold(f32::MIN, f32::max);
        let min_x = pts.iter().map(|p| p.x).fold(f32::MAX, f32::min);
        assert!((max_x - 105.0).abs() < 0.5, "{max_x}");
        assert!((min_x + 10.0).abs() < 0.5, "{min_x}");
    }
}
