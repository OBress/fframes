//! A perspective camera that projects the 3D rig into the 2D frame (SVG coordinates: x right,
//! y down), with roll and lens shift for anime camera work.

use glam::{Vec2, Vec3};

#[derive(Debug, Clone, Copy)]
pub struct Camera {
    pub eye: Vec3,
    pub target: Vec3,
    pub up: Vec3,
    /// Vertical field of view in degrees.
    pub fov_y: f32,
    pub width: f32,
    pub height: f32,
    /// Rotation of the image in degrees (dutch angle).
    pub roll: f32,
    /// Moves the image in pixels without changing the perspective.
    pub shift: Vec2,
    /// Flattens everything onto the ground (y = 0) along this light direction before projecting:
    /// drawing a character with it gives its cast shadow.
    pub flatten: Option<Vec3>,
}

/// [`Camera::project`] with the camera basis precomputed.
#[derive(Debug, Clone, Copy)]
pub struct Projector {
    pub eye: Vec3,
    pub r: Vec3,
    pub u: Vec3,
    pub f: Vec3,
    pub focal: f32,
    center: Vec2,
    roll: Vec2,
    pub flatten: Option<Vec3>,
}

impl Projector {
    /// Screen position and view depth.
    #[inline]
    pub fn project(&self, p: Vec3) -> (Vec2, f32) {
        let p = match self.flatten {
            Some(l) => p - l * (p.y / l.y),
            None => p,
        };
        let d = p - self.eye;
        let z = d.dot(self.f).max(0.01);
        let x = d.dot(self.r) / z * self.focal;
        let y = -d.dot(self.u) / z * self.focal;
        let (c, s) = (self.roll.x, self.roll.y);
        (self.center + Vec2::new(x * c - y * s, x * s + y * c), z)
    }

    /// Unit vector from `p` towards the eye.
    pub fn view(&self, p: Vec3) -> Vec3 {
        (self.eye - p).normalize_or(-self.f)
    }
}

/// A projected point.
#[derive(Debug, Clone, Copy)]
pub struct Projected {
    pub p: Vec2,
    /// Distance along the view direction (bigger is further away).
    pub depth: f32,
    /// Pixels per metre at this depth.
    pub scale: f32,
}

impl Camera {
    pub fn new(eye: Vec3, target: Vec3, fov_y: f32, width: f32, height: f32) -> Self {
        Camera {
            eye,
            target,
            up: Vec3::Y,
            fov_y,
            width,
            height,
            roll: 0.0,
            shift: Vec2::ZERO,
            flatten: None,
        }
    }

    /// A camera circling `target`: `yaw` 0 looks at the character's front (from +Z), positive
    /// yaw moves around to the character's left; `pitch` > 0 looks down from above.
    pub fn orbit(
        target: Vec3,
        distance: f32,
        yaw: f32,
        pitch: f32,
        fov_y: f32,
        width: f32,
        height: f32,
    ) -> Self {
        let (sy, cy) = yaw.to_radians().sin_cos();
        let (sp, cp) = pitch.to_radians().sin_cos();
        let eye = target + Vec3::new(sy * cp, sp, cy * cp) * distance;
        Camera::new(eye, target, fov_y, width, height)
    }

    /// Right, up and forward unit vectors.
    pub fn basis(&self) -> (Vec3, Vec3, Vec3) {
        let f = (self.target - self.eye).normalize_or(Vec3::NEG_Z);
        let r = f.cross(self.up).normalize_or(Vec3::X);
        let u = r.cross(f);
        (r, u, f)
    }

    pub fn focal(&self) -> f32 {
        self.height * 0.5 / (self.fov_y.to_radians() * 0.5).tan()
    }

    /// The same camera drawing shadows cast by a light shining along `light` (pointing down).
    pub fn shadow(&self, light: Vec3) -> Camera {
        Camera {
            flatten: Some(light),
            ..*self
        }
    }

    pub fn project(&self, p: Vec3) -> Projected {
        let p = match self.flatten {
            Some(l) if l.y.abs() > 1e-4 => p - l * (p.y / l.y),
            _ => p,
        };
        let (r, u, f) = self.basis();
        let d = p - self.eye;
        let z = d.dot(f).max(0.01);
        let focal = self.focal();
        let mut x = d.dot(r) / z * focal;
        let mut y = -d.dot(u) / z * focal;
        if self.roll != 0.0 {
            let (s, c) = self.roll.to_radians().sin_cos();
            let (rx, ry) = (x * c - y * s, x * s + y * c);
            x = rx;
            y = ry;
        }
        Projected {
            p: Vec2::new(
                self.width * 0.5 + x + self.shift.x,
                self.height * 0.5 + y + self.shift.y,
            ),
            depth: z,
            scale: focal / z,
        }
    }

    pub fn to_screen(&self, p: Vec3) -> Vec2 {
        self.project(p).p
    }

    /// The projection with the basis computed once, for projecting many points.
    pub fn projector(&self) -> Projector {
        let (r, u, f) = self.basis();
        let (s, c) = self.roll.to_radians().sin_cos();
        Projector {
            eye: self.eye,
            r,
            u,
            f,
            focal: self.focal(),
            center: Vec2::new(self.width * 0.5, self.height * 0.5) + self.shift,
            roll: Vec2::new(c, s),
            flatten: self.flatten.filter(|l| l.y.abs() > 1e-4),
        }
    }

    /// Whether a surface with this normal at `p` faces the camera.
    pub fn faces(&self, p: Vec3, normal: Vec3) -> f32 {
        (self.eye - p).normalize_or_zero().dot(normal)
    }

    /// A camera that draws a model `k` times bigger than life with its `anchor` (say, its feet)
    /// standing at `place` in this camera's world: the same view, moved `k` times closer to
    /// the anchor. Giants and tiny creatures composite into a shot without rescaling the rig.
    pub fn scaled_about(&self, place: Vec3, anchor: Vec3, k: f32) -> Camera {
        Camera {
            eye: anchor + (self.eye - place) / k,
            target: anchor + (self.target - place) / k,
            ..*self
        }
    }

    pub fn lerp(&self, other: &Camera, t: f32) -> Camera {
        Camera {
            eye: self.eye.lerp(other.eye, t),
            target: self.target.lerp(other.target, t),
            up: self.up.lerp(other.up, t).normalize_or(Vec3::Y),
            fov_y: self.fov_y + (other.fov_y - self.fov_y) * t,
            width: self.width,
            height: self.height,
            roll: self.roll + (other.roll - self.roll) * t,
            shift: self.shift.lerp(other.shift, t),
            flatten: self.flatten,
        }
    }
}
