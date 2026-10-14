//! Signed distance modelling of characters in their rest pose.
//!
//! A [`Model`] is a sequence of rounded primitives combined with smooth unions, cuts and clips.
//! Every primitive belongs to a material group (skin, suit, hair...) and is bound to a bone, so
//! the surface extracted from the model knows its materials (and the exact curves where they
//! meet) and how it deforms with the skeleton.

use glam::{Quat, Vec3};

use crate::rig::Bone;

/// Up to this many material groups per model.
pub const MAX_GROUPS: usize = 16;

/// Group of a [`Op::Cut`] or [`Op::Clip`] that applies to every group.
pub const ALL: u8 = u8::MAX;

#[derive(Debug, Clone, Copy)]
pub enum Shape {
    Sphere(f32),
    /// Ellipsoid with these radii (approximate distance).
    Ellipsoid(Vec3),
    /// A round cone from `a` to `b` with radii `ra` and `rb` (a capsule when they are equal).
    Cone {
        a: Vec3,
        b: Vec3,
        ra: f32,
        rb: f32,
    },
    /// A box with these half extents and rounded edges of radius `r` (inside the extents).
    Box {
        half: Vec3,
        r: f32,
    },
    /// A torus around the local Y axis.
    Torus {
        major: f32,
        minor: f32,
    },
    /// Everything below the local XZ plane (`y < 0`); clip with it to cut a shape flat.
    Below,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
    /// Smooth union with the group so far.
    Add,
    /// Smooth subtraction from the group so far.
    Cut,
    /// Smooth intersection with the group so far.
    Clip,
}

#[derive(Debug, Clone, Copy)]
pub struct Prim {
    pub shape: Shape,
    /// Local frame of the shape in the rest pose.
    pub at: Vec3,
    pub rot: Quat,
    /// Stretch of the local axes (elliptic cross sections, flattened shapes).
    pub stretch: Vec3,
    /// Smoothing radius of the operation.
    pub blend: f32,
    pub op: Op,
    pub group: u8,
    pub bone: Bone,
}

impl Prim {
    pub fn new(shape: Shape, group: u8, bone: Bone) -> Self {
        Prim {
            shape,
            at: Vec3::ZERO,
            rot: Quat::IDENTITY,
            stretch: Vec3::ONE,
            blend: 0.0,
            op: Op::Add,
            group,
            bone,
        }
    }

    pub fn at(mut self, at: Vec3) -> Self {
        self.at = at;
        self
    }

    pub fn rot(mut self, rot: Quat) -> Self {
        self.rot = rot;
        self
    }

    pub fn stretch(mut self, stretch: Vec3) -> Self {
        self.stretch = stretch;
        self
    }

    pub fn blend(mut self, k: f32) -> Self {
        self.blend = k;
        self
    }

    pub fn op(mut self, op: Op) -> Self {
        self.op = op;
        self
    }

    /// Distance from `p` (rest space) to the shape.
    pub fn distance(&self, p: Vec3) -> f32 {
        let q = self.rot.inverse() * (p - self.at) / self.stretch;
        let d = match self.shape {
            Shape::Sphere(r) => q.length() - r,
            Shape::Ellipsoid(r) => ellipsoid(q, r),
            Shape::Cone { a, b, ra, rb } => round_cone(q, a, b, ra, rb),
            Shape::Box { half, r } => {
                let d = q.abs() - (half - Vec3::splat(r));
                d.max(Vec3::ZERO).length() + d.max_element().min(0.0) - r
            }
            Shape::Torus { major, minor } => {
                let x = Vec3::new(q.x, 0.0, q.z).length() - major;
                (x * x + q.y * q.y).sqrt() - minor
            }
            Shape::Below => q.y,
        };
        d * self.stretch.min_element()
    }
}

fn ellipsoid(p: Vec3, r: Vec3) -> f32 {
    let k0 = (p / r).length();
    let k1 = (p / (r * r)).length();
    if k1 < 1e-9 {
        -r.min_element()
    } else {
        k0 * (k0 - 1.0) / k1
    }
}

/// Exact distance to a round cone (Inigo Quilez).
fn round_cone(p: Vec3, a: Vec3, b: Vec3, r1: f32, r2: f32) -> f32 {
    let ba = b - a;
    let l2 = ba.dot(ba);
    if l2 < 1e-12 {
        return (p - a).length() - r1.max(r2);
    }
    let rr = r1 - r2;
    let a2 = l2 - rr * rr;
    let il2 = 1.0 / l2;
    let pa = p - a;
    let y = pa.dot(ba);
    let z = y - l2;
    let x2 = (pa * l2 - ba * y).length_squared();
    let y2 = y * y * l2;
    let z2 = z * z * l2;
    let k = rr.signum() * rr * rr * x2;
    if z.signum() * a2 * z2 > k {
        return (x2 + z2).sqrt() * il2 - r2;
    }
    if y.signum() * a2 * y2 < k {
        return (x2 + y2).sqrt() * il2 - r1;
    }
    ((x2 * a2 * il2).sqrt() + y * rr) * il2 - r1
}

/// Polynomial smooth minimum.
pub fn smin(a: f32, b: f32, k: f32) -> f32 {
    if k <= 0.0 {
        return a.min(b);
    }
    let h = (k - (a - b).abs()).max(0.0) / k;
    a.min(b) - h * h * k * 0.25
}

pub fn smax(a: f32, b: f32, k: f32) -> f32 {
    -smin(-a, -b, k)
}

/// A character part made of primitives.
#[derive(Debug, Clone, Default)]
pub struct Model {
    pub prims: Vec<Prim>,
}

/// Distances of a point to every group of a model.
#[derive(Debug, Clone, Copy)]
pub struct Groups {
    pub d: [f32; MAX_GROUPS],
}

impl Groups {
    /// The surface: the closest group.
    pub fn min(&self) -> f32 {
        self.d.iter().copied().fold(f32::INFINITY, f32::min)
    }

    /// The `n` closest groups and their distances, nearest first.
    pub fn nearest<const N: usize>(&self) -> [(u8, f32); N] {
        let mut out = [(u8::MAX, f32::INFINITY); N];
        for (g, &d) in self.d.iter().enumerate() {
            if !d.is_finite() {
                continue;
            }
            if let Some(slot) = out.iter().position(|(_, od)| d < *od) {
                out[slot..].rotate_right(1);
                out[slot] = (g as u8, d);
            }
        }
        out
    }
}

impl Model {
    pub fn push(&mut self, prim: Prim) -> &mut Self {
        self.prims.push(prim);
        self
    }

    pub fn extend(&mut self, other: &Model) {
        self.prims.extend_from_slice(&other.prims);
    }

    /// Distance to every group.
    pub fn groups(&self, p: Vec3) -> Groups {
        let mut d = [f32::INFINITY; MAX_GROUPS];
        for prim in &self.prims {
            let dp = prim.distance(p);
            if prim.group == ALL {
                for v in d.iter_mut().filter(|v| v.is_finite()) {
                    *v = match prim.op {
                        Op::Clip => smax(*v, dp, prim.blend),
                        _ => smax(*v, -dp, prim.blend),
                    };
                }
                continue;
            }
            let g = prim.group as usize;
            d[g] = match prim.op {
                Op::Add => {
                    if d[g].is_finite() {
                        smin(d[g], dp, prim.blend)
                    } else {
                        dp
                    }
                }
                Op::Cut => smax(d[g], -dp, prim.blend),
                Op::Clip => smax(d[g], dp, prim.blend),
            };
        }
        Groups { d }
    }

    pub fn distance(&self, p: Vec3) -> f32 {
        self.groups(p).min()
    }

    pub fn gradient(&self, p: Vec3, eps: f32) -> Vec3 {
        let dx = Vec3::new(eps, 0.0, 0.0);
        let dy = Vec3::new(0.0, eps, 0.0);
        let dz = Vec3::new(0.0, 0.0, eps);
        Vec3::new(
            self.distance(p + dx) - self.distance(p - dx),
            self.distance(p + dy) - self.distance(p - dy),
            self.distance(p + dz) - self.distance(p - dz),
        ) / (2.0 * eps)
    }

    /// Skinning weights at `p`: bones of the nearby primitives, closer ones weigh more. `spread`
    /// is the distance over which neighbouring bones blend (a few centimetres at a joint).
    pub fn bone_weights(&self, p: Vec3, spread: f32) -> [(u8, f32); 4] {
        let mut per_bone = [0.0_f32; crate::rig::BONE_COUNT];
        let mut nearest = f32::INFINITY;
        let ds: Vec<(Bone, f32)> = self
            .prims
            .iter()
            .filter(|p| p.op == Op::Add)
            .map(|prim| {
                let d = prim.distance(p);
                nearest = nearest.min(d);
                (prim.bone, d)
            })
            .collect();
        for (bone, d) in ds {
            per_bone[bone.index()] += (-(d - nearest).max(0.0) / spread).exp();
        }
        let mut out = [(0_u8, 0.0_f32); 4];
        for (b, &w) in per_bone.iter().enumerate() {
            if let Some(slot) = out.iter().position(|(_, ow)| w > *ow) {
                out[slot..].rotate_right(1);
                out[slot] = (b as u8, w);
            }
        }
        let sum: f32 = out.iter().map(|(_, w)| *w).sum();
        if sum > 0.0 {
            for o in &mut out {
                o.1 /= sum;
            }
        }
        // Drop tiny influences so a hand does not drag the hips along.
        let mut out = out.map(|(b, w)| if w < 0.02 { (b, 0.0) } else { (b, w) });
        let sum: f32 = out.iter().map(|(_, w)| *w).sum();
        for o in &mut out {
            o.1 /= sum.max(1e-6);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn primitives_measure_distance() {
        let s = Prim::new(Shape::Sphere(1.0), 0, Bone::Hips);
        assert!((s.distance(Vec3::new(2.0, 0.0, 0.0)) - 1.0).abs() < 1e-5);
        let c = Prim::new(
            Shape::Cone {
                a: Vec3::ZERO,
                b: Vec3::Y,
                ra: 0.2,
                rb: 0.2,
            },
            0,
            Bone::Hips,
        );
        assert!((c.distance(Vec3::new(1.0, 0.5, 0.0)) - 0.8).abs() < 1e-5);
        let b = Prim::new(
            Shape::Box {
                half: Vec3::ONE,
                r: 0.1,
            },
            0,
            Bone::Hips,
        );
        assert!((b.distance(Vec3::new(3.0, 0.0, 0.0)) - 2.0).abs() < 1e-5);
    }

    #[test]
    fn groups_and_weights() {
        let mut m = Model::default();
        m.push(Prim::new(Shape::Sphere(0.5), 0, Bone::Hips));
        m.push(Prim::new(Shape::Sphere(0.5), 1, Bone::Head).at(Vec3::new(1.0, 0.0, 0.0)));
        let g = m.groups(Vec3::new(0.1, 0.0, 0.0));
        let near = g.nearest::<2>();
        assert_eq!(near[0].0, 0);
        assert_eq!(near[1].0, 1);
        let w = m.bone_weights(Vec3::new(0.5, 0.0, 0.0), 0.05);
        assert!((w[0].1 - 0.5).abs() < 0.01);
    }
}
