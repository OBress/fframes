//! Math types from `glam` and helper functions for animation and procedural scenes.
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
    let t = progress(x, edge0, edge1);
    t * t * (3.0 - 2.0 * t)
}

/// Linear progress of `t` from `start` to `end`, clamped to `0..1`.
#[inline]
pub fn progress(t: f32, start: f32, end: f32) -> f32 {
    ((t - start) / (end - start)).clamp(0.0, 1.0)
}

/// Maps an integer to a pseudo-random value in `0..1`. The same input always gives the
/// same value.
#[inline]
pub fn hash(n: u32) -> f32 {
    let mut x = n.wrapping_mul(0x9E37_79B9) ^ 0x85EB_CA6B;
    x ^= x >> 16;
    x = x.wrapping_mul(0x7FEB_352D);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846C_A68B);
    x ^= x >> 16;
    (x >> 8) as f32 / (1u32 << 24) as f32
}

/// Smooth 1D noise in about `-1..1`. Use it for flicker and slow drift:
/// `noise1(seed, t * frequency)`. A different `seed` gives a different curve.
///
/// The curve goes through a random value at each integer `x` (a Catmull-Rom spline), and
/// its slope there comes from the neighbour values. So it does not stop at each integer, as
/// value noise with smoothstep weights does. For camera motion use [`gradient_noise1`].
pub fn noise1(seed: u32, x: f32) -> f32 {
    let i = x.floor();
    let t = x - i;
    let cell = (i as i32 as u32).wrapping_add(seed.wrapping_mul(1013));
    let key = |k: i32| hash(cell.wrapping_add(k as u32)) * 2.0 - 1.0;
    let (p0, p1, p2, p3) = (key(-1), key(0), key(1), key(2));
    let (m1, m2) = ((p2 - p0) * 0.5, (p3 - p1) * 0.5);
    let (t2, t3) = (t * t, t * t * t);
    (2.0 * t3 - 3.0 * t2 + 1.0) * p1
        + (t3 - 2.0 * t2 + t) * m1
        + (-2.0 * t3 + 3.0 * t2) * p2
        + (t3 - t2) * m2
}

/// Smooth 1D gradient noise in `-1..1` (Perlin noise). Each integer `x` has a random slope
/// and the value 0, and a quintic curve blends the slopes of the two neighbours. The speed
/// of the curve changes smoothly and seldom gets to zero, so a camera that it moves does
/// not stop and start again. Its mean amplitude is about half of [`noise1`].
pub fn gradient_noise1(seed: u32, x: f32) -> f32 {
    let i = x.floor();
    let f = x - i;
    let cell = (i as i32 as u32).wrapping_add(seed.wrapping_mul(1013));
    let g0 = hash(cell) * 2.0 - 1.0;
    let g1 = hash(cell.wrapping_add(1)) * 2.0 - 1.0;
    let u = f * f * f * (f * (f * 6.0 - 15.0) + 10.0);
    let (a, b) = (g0 * f, g1 * (f - 1.0));
    // the largest value is 0.5
    2.0 * (a + (b - a) * u)
}

/// A pseudo-random unit vector. The same `seed` always gives the same vector.
pub fn random_direction(seed: u32) -> Vec3 {
    let u = hash(seed.wrapping_mul(2).wrapping_add(1)) * 2.0 - 1.0;
    let phi = hash(seed.wrapping_mul(2).wrapping_add(2)) * std::f32::consts::TAU;
    let r = (1.0 - u * u).max(0.0).sqrt();
    Vec3::new(r * phi.cos(), u, r * phi.sin())
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
