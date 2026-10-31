//! Math types from `glam` and a few helpers.
//!
//! The conventions are the same as in three.js. Coordinates are right-handed, Y is up, a
//! camera looks along -Z, and angles are in radians.

pub use glam::{EulerRot, Mat3, Mat4, Quat, Vec2, Vec3, Vec4, vec2, vec3, vec4};

/// Converts degrees to radians.
#[inline]
pub fn deg(degrees: f32) -> f32 {
    degrees.to_radians()
}

/// Makes a rotation from Euler angles in radians, applied in X, Y, Z order.
/// This is the three.js default order `"XYZ"`.
#[inline]
pub fn euler(x: f32, y: f32, z: f32) -> Quat {
    Quat::from_euler(EulerRot::XYZ, x, y, z)
}

/// Makes a rotation from Euler angles in degrees, applied in X, Y, Z order.
#[inline]
pub fn euler_deg(x: f32, y: f32, z: f32) -> Quat {
    euler(deg(x), deg(y), deg(z))
}

/// Hermite interpolation from 0 to 1 while `x` goes from `edge0` to `edge1`.
/// The result is 0 below `edge0` and 1 above `edge1`, like GLSL `smoothstep`.
#[inline]
pub fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// The light of a spot light at the cosine `cos` of the angle off its axis: the cone's soft
/// edge, and its hot core (see [`SpotLight::hotspot`](crate::SpotLight)). `spot_cone` in
/// `scene.wgsl` is the same.
pub(crate) fn spot_cone(cos: f32, cos_outer: f32, cos_inner: f32, hotspot: f32) -> f32 {
    let edge = smoothstep(cos_outer, cos_inner, cos);
    if hotspot <= 0.0 {
        return edge;
    }
    edge * (1.0 - hotspot) + reflector(cos, cos_outer) * hotspot
}

/// A flashlight reflector's pattern at the cosine `cos` off its axis, 1 on the axis: a hot
/// core, a faint bright ring at the reflector's rim, and a wide dim spill that fades out
/// softly by the cone's edge (no hard circle). `reflector` in `scene.wgsl` is the same.
fn reflector(cos: f32, cos_outer: f32) -> f32 {
    // the angle off the axis as a share of the cone's
    let x = cos.clamp(-1.0, 1.0).acos() / cos_outer.clamp(-1.0, 1.0).acos().max(1e-4);
    let core = (-(x / 0.28).powi(2)).exp();
    let ring = 0.12 * (-((x - 0.42) / 0.06).powi(2)).exp();
    let spill = 0.3 * smoothstep(1.0, 0.45, x);
    (core + ring + spill) / 1.3
}

/// An axis-aligned bounding box. The default value is [`Aabb::EMPTY`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Aabb {
    pub min: Vec3,
    pub max: Vec3,
}

impl Default for Aabb {
    fn default() -> Self {
        Self::EMPTY
    }
}

impl Aabb {
    /// A box that contains no points. Extending it with a point gives a box around that point.
    pub const EMPTY: Aabb = Aabb {
        min: Vec3::splat(f32::INFINITY),
        max: Vec3::splat(f32::NEG_INFINITY),
    };

    /// The smallest box that contains all `points`.
    pub fn from_points(points: impl IntoIterator<Item = Vec3>) -> Self {
        let mut aabb = Self::EMPTY;
        for p in points {
            aabb.extend(p);
        }
        aabb
    }

    /// Grows the box to contain `p`.
    pub fn extend(&mut self, p: Vec3) {
        self.min = self.min.min(p);
        self.max = self.max.max(p);
    }

    /// The smallest box that contains both boxes.
    pub fn union(self, other: Aabb) -> Aabb {
        Aabb {
            min: self.min.min(other.min),
            max: self.max.max(other.max),
        }
    }

    /// True when the box contains no points.
    pub fn is_empty(&self) -> bool {
        self.min.x > self.max.x
    }

    pub fn center(&self) -> Vec3 {
        (self.min + self.max) * 0.5
    }

    pub fn size(&self) -> Vec3 {
        self.max - self.min
    }

    /// The eight corners. Bit 0 of the index selects max X, bit 1 max Y, bit 2 max Z.
    pub fn corners(&self) -> [Vec3; 8] {
        let (a, b) = (self.min, self.max);
        [
            Vec3::new(a.x, a.y, a.z),
            Vec3::new(b.x, a.y, a.z),
            Vec3::new(a.x, b.y, a.z),
            Vec3::new(b.x, b.y, a.z),
            Vec3::new(a.x, a.y, b.z),
            Vec3::new(b.x, a.y, b.z),
            Vec3::new(a.x, b.y, b.z),
            Vec3::new(b.x, b.y, b.z),
        ]
    }

    /// The axis-aligned box around this box after the transform `m`.
    pub fn transformed(&self, m: &Mat4) -> Aabb {
        if self.is_empty() {
            return *self;
        }
        Aabb::from_points(self.corners().map(|c| m.transform_point3(c)))
    }
}
