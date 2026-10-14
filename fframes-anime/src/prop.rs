//! Props (小物): rigid objects modelled like the characters, from rounded primitives with
//! material groups ([`crate::sdf`]), meshed once and drawn in the same depth buffer and line art
//! as the cast. A fist closes around a bar and a cream puff sits in a palm with the right
//! overlaps both ways, which a prop pasted over the drawing can never do.

use std::borrow::Cow;

use glam::{Affine3A, Mat3, Vec2, Vec3};

use crate::mesh::{Mesh, surface_nets};
use crate::raster::{CurveRef, Fragment, GroupSource, LineSource, Surface};
use crate::sdf::{MAX_GROUPS, Model};

/// Painted detail of a prop (cracks, sugar, stripes): the colour of a fragment from its rest
/// position, group and flat colour, with `px` the size of a pixel in metres there.
pub type Detail = fn(rest: Vec3, group: u8, color: Vec3, px: f32) -> Vec3;

#[derive(Debug, Clone)]
pub struct Prop {
    pub mesh: Mesh,
    /// Fill colour of every material group.
    pub colors: [Vec3; MAX_GROUPS],
    pub detail: Option<Detail>,
    /// Line weight relative to the cast's lines.
    pub line_weight: f32,
}

/// Colours of a prop's cel shading.
#[derive(Debug, Clone, Copy)]
pub struct PropLight {
    /// Unit vector towards the light.
    pub light: Vec3,
    /// How much darker the shadow side is (0 = flat).
    pub shade: f32,
    /// Where the shadow starts (`n · light`).
    pub terminator: f32,
    /// One colour for everything (a silhouette shot), instead of the prop's own.
    pub flat: Option<Vec3>,
}

impl Prop {
    /// Meshes `model` inside `min..max` with cells of `cell` metres.
    pub fn new(model: &Model, min: Vec3, max: Vec3, cell: f32) -> Self {
        Prop {
            mesh: surface_nets(model, min, max, cell, 0.02, &|_| true),
            colors: [Vec3::splat(0.5); MAX_GROUPS],
            detail: None,
            line_weight: 1.0,
        }
    }

    pub fn with_colors(mut self, colors: &[(u8, Vec3)]) -> Self {
        for (g, c) in colors {
            self.colors[*g as usize] = *c;
        }
        self
    }

    pub fn with_detail(mut self, detail: Detail) -> Self {
        self.detail = Some(detail);
        self
    }

    pub fn with_line_weight(mut self, w: f32) -> Self {
        self.line_weight = w;
        self
    }

    /// The prop placed by `at` (scale, rotation and position) for one frame. `tag` identifies it
    /// for the shader and the line art like a cast member's surfaces.
    pub fn surface(&self, at: Affine3A, tag: u32) -> Surface<'_> {
        let linear = Mat3::from(at.matrix3);
        let normals = linear.inverse().transpose();
        Surface {
            pos: self
                .mesh
                .rest
                .iter()
                .map(|p| at.transform_point3(*p))
                .collect(),
            nrm: self
                .mesh
                .normal
                .iter()
                .map(|n| (normals * *n).normalize_or(*n))
                .collect(),
            tris: Cow::Borrowed(&self.mesh.tris),
            rest: Some(&self.mesh.rest),
            groups: GroupSource::Near(&self.mesh.near),
            back: None,
            tag,
            lines: LineSource {
                contours: true,
                curves: self
                    .mesh
                    .borders
                    .iter()
                    .map(|b| CurveRef::new(Cow::Borrowed(b.pts.as_slice())))
                    .collect(),
                weight: self.line_weight,
                fine_groups: &[],
            },
        }
    }

    /// Cel shading: the group's colour, its painted detail and a clean shadow edge.
    pub fn shade(&self, f: &Fragment, light: &PropLight) -> Vec3 {
        if let Some(c) = light.flat {
            return c;
        }
        let mut color = self.colors[(f.group as usize).min(MAX_GROUPS - 1)];
        if let Some(detail) = self.detail {
            color = detail(f.rest, f.group, color, f.px);
        }
        if light.shade > 0.0 {
            let lit = f.normal.dot(light.light);
            let t = ((lit - light.terminator + 0.04) / 0.08).clamp(0.0, 1.0);
            let k = t * t * (3.0 - 2.0 * t);
            color *= 1.0 - light.shade * (1.0 - k);
        }
        color
    }
}

/// Coverage of a region with signed distance `d` (negative inside) at pixel size `px`, for
/// painting crisp details.
pub fn cover(d: f32, px: f32) -> f32 {
    (0.5 - d / px.max(1e-6)).clamp(0.0, 1.0)
}

/// Distances to the nearest and second nearest feature points of a 3D cell pattern with cells of
/// `size` (cracks run where they are equal), and the nearest point's cell hash.
pub fn cells(p: Vec3, size: f32, seed: u32) -> (f32, f32, f32) {
    let q = p / size;
    let base = q.floor();
    let (mut d1, mut d2, mut id) = (f32::INFINITY, f32::INFINITY, 0.0);
    for z in -1..=1 {
        for y in -1..=1 {
            for x in -1..=1 {
                let c = base + Vec3::new(x as f32, y as f32, z as f32);
                let h = |k: u32| {
                    let n = (c.x as i32 as u32).wrapping_mul(0x8DA6_B343)
                        ^ (c.y as i32 as u32).wrapping_mul(0xD816_3841)
                        ^ (c.z as i32 as u32).wrapping_mul(0xCB1A_B31F)
                        ^ seed.wrapping_add(k).wrapping_mul(0x9E37_79B9);
                    let mut n = n ^ (n >> 15);
                    n = n.wrapping_mul(0x2C1B_3C6D);
                    n ^= n >> 12;
                    (n & 0xFFFF) as f32 / 65_536.0
                };
                let point = c + Vec3::new(h(1), h(2), h(3)) * 0.8 + Vec3::splat(0.1);
                let d = (point - q).length();
                if d < d1 {
                    d2 = d1;
                    d1 = d;
                    id = h(4);
                } else if d < d2 {
                    d2 = d;
                }
            }
        }
    }
    (d1 * size, d2 * size, id)
}

/// A 2D hash in 0..1 (for sprinkles).
pub fn hash2(p: Vec2, seed: u32) -> f32 {
    let n = (p.x as i32 as u32).wrapping_mul(0x8DA6_B343)
        ^ (p.y as i32 as u32).wrapping_mul(0xD816_3841)
        ^ seed.wrapping_mul(0x9E37_79B9);
    let mut n = n ^ (n >> 15);
    n = n.wrapping_mul(0x2C1B_3C6D);
    n ^= n >> 12;
    (n & 0xFFFF) as f32 / 65_536.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rig::Bone;
    use crate::sdf::{Prim, Shape};
    use glam::Quat;

    #[test]
    fn prop_surface_follows_its_placement() {
        let mut m = Model::default();
        m.push(Prim::new(Shape::Sphere(0.05), 0, Bone::Hips));
        m.push(Prim::new(Shape::Sphere(0.03), 1, Bone::Hips).at(Vec3::new(0.0, 0.05, 0.0)));
        let prop = Prop::new(&m, Vec3::splat(-0.1), Vec3::splat(0.1), 0.005);
        assert!(prop.mesh.tris.len() > 100);
        assert!(
            prop.mesh.borders.iter().any(|b| b.pts.len() > 3),
            "the two groups meet along a curve"
        );
        let at = Affine3A::from_scale_rotation_translation(
            Vec3::splat(2.0),
            Quat::from_rotation_y(1.0),
            Vec3::new(1.0, 2.0, 3.0),
        );
        let s = prop.surface(at, 7);
        let centre = s.pos.iter().copied().sum::<Vec3>() / s.pos.len() as f32;
        assert!(
            (centre - Vec3::new(1.0, 2.0, 3.0)).length() < 0.06,
            "{centre}"
        );
        let r = s
            .pos
            .iter()
            .map(|p| (*p - centre).length())
            .fold(0.0, f32::max);
        assert!(r > 0.09, "scaled up: {r}");
    }

    #[test]
    fn cell_pattern_has_borders() {
        let (d1, d2, _) = cells(Vec3::new(0.013, 0.021, 0.007), 0.01, 3);
        assert!(d1 <= d2);
    }
}
