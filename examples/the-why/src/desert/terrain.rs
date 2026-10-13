//! The ground of the plateau: a heightfield of sand to the horizon, a pan of cracked dry
//! mud around the origin, rocks from pebbles to outcrops, and mesas on the horizon.

use std::f32::consts::TAU;
use std::sync::Arc;

use fframes_3d::math::{hash, random_direction, smoothstep};
use fframes_3d::*;

use super::{ahead, fbm, kneel_heading, noise2, right};
use crate::assets::Assets;

/// The radius of the terrain mesh in meters. The mesas stand inside it.
const RADIUS: f32 = 8500.0;
/// The number of vertices on each ring of the terrain mesh.
const SEGMENTS: u32 = 256;
/// The radius of the pan of dry mud around the origin, in meters.
const PLAYA: f32 = 38.0;
/// The pan of mud is this high above the sand, in meters, so the sand does not show
/// through it.
const PLAYA_LIFT: f32 = 0.012;

/// The shape of the ground.
pub(super) struct Terrain {
    /// The horizontal direction to the sun of shot 9.
    sun: Vec2,
}

/// The meshes and the materials of the ground, shared by the night and the dawn scenes.
pub(super) struct Ground {
    sand: (Arc<Geometry>, Material),
    playa: (Arc<Geometry>, Material),
    rocks: Vec<(Arc<Geometry>, Material, Arc<Instances>)>,
    mesas: (Arc<Geometry>, Material),
}

impl Terrain {
    pub fn new() -> Self {
        let sun = super::sunrise::sun_direction();
        Self {
            sun: vec2(sun.x, sun.z).normalize(),
        }
    }

    /// The height of the ground at `x`, `z` in meters. The ground is flat near the origin
    /// and in the dry lake bed between the origin and the factory.
    pub fn height(&self, x: f32, z: f32) -> f32 {
        let p = vec2(x, z);
        let r = p.length();
        let a = ahead();
        let along = p.dot(vec2(a.x, a.z));
        let across = p.dot(vec2(-a.z, a.x)).abs();

        // Fine relief everywhere. Swells and low dunes away from the pan, rolling land
        // farther out, and ridges far away.
        let fine = 0.03 * fbm(p / 3.0, 3, 1);
        let swells = smoothstep(PLAYA * 0.7, 110.0, r)
            * (1.4 * fbm(p / 60.0, 4, 2) + 0.45 * fbm(p / 13.0, 3, 3));
        let rolling = smoothstep(50.0, 400.0, r) * 10.0 * fbm(p / 280.0, 4, 5);
        let ridge = 1.0 - fbm(p / 650.0, 4, 4).abs();
        let ridges = 75.0 * ridge.powi(3) * smoothstep(300.0, 1500.0, r);
        let mut h = fine + swells + rolling + ridges;

        // The lake bed is a flat valley that opens toward the factory. The land beside it
        // rises, but it does not hide the factory.
        let basin = smoothstep(-60.0, 0.0, along)
            * (1.0 - smoothstep(30.0 + 0.24 * along, 60.0 + 0.42 * along, across));
        h = h * (1.0 - basin) + (fine + 0.15 * swells) * basin;

        // Behind the night cameras, toward the sunrise, the plateau ends at an escarpment.
        // Below it lies a plain with a canyon that runs toward the sun.
        let (s, a) = self.sunward(p);
        let view = smoothstep(0.0, 150.0, s) * (1.0 - smoothstep(700.0, 1500.0, a.abs()));
        let edge = EDGE + 10.0 * fbm(vec2(a / 70.0, 0.5), 3, 31);
        let drop = PLAIN_DROP
            * smoothstep(edge, edge + 45.0, s)
            * (1.0 - smoothstep(1100.0, 1700.0, a.abs()));
        h * (1.0 - 0.8 * view) - drop - canyon_depth(s, a)
    }

    /// The position `p` (x, z) in the frame of the sunrise view: the distance toward the
    /// sun and the distance to the right of that direction, in meters.
    pub fn sunward(&self, p: Vec2) -> (f32, f32) {
        (p.dot(self.sun), p.dot(vec2(-self.sun.y, self.sun.x)))
    }

    /// The point (x, z) at the distance `s` toward the sun and `a` to the right.
    pub fn sunward_point(&self, s: f32, a: f32) -> Vec2 {
        self.sun * s + vec2(-self.sun.y, self.sun.x) * a
    }

    /// The height of the surface of the mud pan at `x`, `z`.
    pub fn playa(&self, x: f32, z: f32) -> f32 {
        self.height(x, z) + PLAYA_LIFT
    }

    /// The unit normal of the ground at `x`, `z`, from differences over `eps` meters.
    pub fn normal(&self, x: f32, z: f32, eps: f32) -> Vec3 {
        let dx = self.height(x + eps, z) - self.height(x - eps, z);
        let dz = self.height(x, z + eps) - self.height(x, z - eps);
        vec3(-dx, 2.0 * eps, -dz).normalize()
    }

    /// Builds the meshes of the ground.
    pub fn meshes(&self, assets: &Assets) -> Ground {
        let sand = assets.sand.material(0xd8c0a0, 1.0, 0.0).env_intensity(0.45);
        let playa = dirt(assets).transparent();
        let rock = assets.rock.material(0xb89a80, 0.95, 0.0);
        Ground {
            sand: (Arc::new(self.sand_mesh()), sand),
            playa: (Arc::new(self.playa_mesh()), playa),
            rocks: self.rocks(&rock),
            mesas: (
                Arc::new(self.mesas()),
                rock.color(0x9a9aa4).normal_scale(0.6),
            ),
        }
    }

    /// The sand to the horizon: rings around the origin that get wider with the distance,
    /// so the vertices are dense near the cameras.
    fn sand_mesh(&self) -> Geometry {
        let mut radii = Vec::new();
        let mut r = 0.3;
        while r < RADIUS {
            radii.push(r);
            r *= 1.0 + TAU / SEGMENTS as f32;
        }
        radii.push(RADIUS);
        polar_grid(&radii, |p| {
            let r = p.length();
            let h = self.height(p.x, p.y);
            let n = self.normal(p.x, p.y, (r * 0.015).max(0.05));
            // Large patches of lighter and darker sand hide the repeat of the texture.
            let tone = 0.82 + 0.3 * fbm(p / 40.0, 3, 11) + 0.08 * fbm(p / 4.0, 2, 12);
            let red = 1.0 + 0.06 * fbm(p / 300.0, 2, 13);
            // Steep slopes are bare rock in bands of red and brown.
            let band = 0.8 + 0.25 * noise2(vec2(h / 6.0, 0.5), 41);
            let rock = vec3(0.62, 0.4, 0.3) * band;
            let sand = vec3(tone * red, tone, tone / red);
            let c = sand.lerp(rock, smoothstep(0.25, 0.5, 1.0 - n.y));
            (h, n, c.extend(1.0))
        })
    }

    /// The pan of cracked mud around the origin. Its edge fades into the sand.
    fn playa_mesh(&self) -> Geometry {
        let mut radii = Vec::new();
        let mut r = 0.02;
        while r < PLAYA * 1.5 {
            radii.push(r);
            r *= 1.0 + TAU / SEGMENTS as f32;
        }
        polar_grid(&radii, |p| {
            let r = p.length();
            let edge = PLAYA * (1.0 + 0.3 * fbm(p.normalize_or_zero() * 2.5, 3, 21));
            let alpha = 1.0 - smoothstep(edge * 0.75, edge, r);
            let n = self.normal(p.x, p.y, 0.05);
            let tone = playa_tone(p);
            (self.playa(p.x, p.y), n, vec4(tone, tone, tone, alpha))
        })
    }

    /// Rocks of four sizes. Each size has four shapes, each shape is one instanced mesh.
    /// No rock lies on the path of the engineer, on the line or on the path of its light.
    fn rocks(&self, material: &Material) -> Vec<(Arc<Geometry>, Material, Arc<Instances>)> {
        // count, nearest and farthest distance, smallest and largest radius, mesh detail
        let classes = [
            (2600u32, 1.2f32, 45.0f32, 0.02f32, 0.09f32, 1u32),
            (1100, 24.0, 260.0, 0.15, 0.55, 2),
            (340, 14.0, 1600.0, 0.9, 3.5, 3),
            (70, 140.0, 2600.0, 6.0, 18.0, 3),
        ];
        let walk = kneel_heading();
        let walk_dir = vec2(walk.sin(), walk.cos());
        let (a, side) = (ahead(), right());
        let (a, side) = (vec2(a.x, a.z), vec2(side.x, side.z));
        let mut out = Vec::new();
        for (class, &(count, near, far, small, large, detail)) in classes.iter().enumerate() {
            let seed = class as u32 * 7919;
            let mut lists: [Vec<(Mat4, Rgb)>; 4] = Default::default();
            let mut placed = 0;
            for k in 0..count * 6 {
                if placed == count {
                    break;
                }
                let h = |n: u32| hash(seed * 31 + k * 13 + n);
                // more rocks near the origin
                let r = near + (far - near) * h(1).powf(1.8);
                let angle = h(2) * TAU;
                let p = vec2(angle.cos(), angle.sin()) * r;
                let size = small + (large - small) * h(3).powi(3);
                // the line and the path of its light to the factory
                let along = p.dot(a);
                if along > -3.0 && p.dot(side).abs() < 1.5 + size + 0.03 * along {
                    continue;
                }
                // the path of the engineer
                let back = -p.dot(walk_dir);
                if back > -1.0 && back < 14.0 && (p + walk_dir * back).length() < 1.0 + size {
                    continue;
                }
                // big rocks stay out of the sunrise view
                let (s, across) = self.sunward(p);
                if class >= 2 && s > -60.0 && across.abs() < if s > 60.0 { 900.0 } else { 30.0 } {
                    continue;
                }
                let squash = if class == 3 { 0.45 } else { 0.6 + 0.3 * h(4) };
                let scale = vec3(size * (0.8 + 0.5 * h(5)), size * squash, size);
                let tilt = self.normal(p.x, p.y, size.max(0.3));
                let rotation = Quat::from_rotation_arc(Vec3::Y, tilt)
                    * Quat::from_rotation_y(h(6) * TAU)
                    * Quat::from_rotation_x((h(7) - 0.5) * 0.4);
                let position = vec3(p.x, self.height(p.x, p.y) - 0.25 * scale.y, p.y);
                let tone = 0.7 + 0.45 * h(9);
                lists[(h(8) * 4.0) as usize % 4].push((
                    Mat4::from_scale_rotation_translation(scale, rotation, position),
                    Rgb::linear(tone * 1.04, tone, tone * 0.94),
                ));
                placed += 1;
            }
            for (k, list) in lists.into_iter().enumerate() {
                let instances = Instances::new(list.iter().map(|(m, _)| *m))
                    .colors(list.iter().map(|(_, c)| *c));
                out.push((
                    Arc::new(rock(seed + k as u32, detail)),
                    material.clone(),
                    Arc::new(instances),
                ));
            }
        }
        out
    }

    /// Flat topped mesas with cliffs and slopes of scree, on the horizon. None stands in
    /// front of the factory or of the sunrise.
    fn mesas(&self) -> Geometry {
        let a = ahead();
        let factory = a.z.atan2(a.x);
        let sun = self.sun.y.atan2(self.sun.x);
        let away = |angle: f32, from: f32| {
            let d = (angle - from + TAU * 1.5).rem_euclid(TAU) - TAU * 0.5;
            d.abs()
        };
        let mut parts = Vec::new();
        for i in 0..22u32 {
            let angle = (i as f32 + 0.6 * hash(i * 3 + 1)) / 22.0 * TAU;
            if away(angle, factory) < 0.5 || away(angle, sun) < 0.25 {
                continue;
            }
            let distance = 3600.0 + 3800.0 * hash(i * 3 + 2);
            let center = vec2(angle.cos(), angle.sin()) * distance;
            let radius = 260.0 + 600.0 * hash(i * 5 + 3);
            let height = 70.0 + 190.0 * hash(i * 7 + 4).powi(2);
            parts.push(mesa(center, radius, height, i));
        }
        Geometry::merge(parts)
    }
}

/// The cracked dry mud of the pan.
pub(super) fn dirt(assets: &Assets) -> Material {
    assets.dirt.material(0xc8b8a4, 1.0, 0.0).normal_scale(1.4)
}

/// The vertex color of the mud pan at `p` (x, z): large patches of lighter and darker mud.
pub(super) fn playa_tone(p: Vec2) -> f32 {
    0.9 + 0.12 * fbm(p / 6.0, 3, 22)
}

/// The distance from the origin to the edge of the plateau, toward the sun, in meters.
pub(super) const EDGE: f32 = 42.0;
/// The plain below the escarpment is this much lower than the plateau, in meters.
const PLAIN_DROP: f32 = 45.0;
/// The depth of the canyon below the plain in meters.
const CANYON_DEPTH: f32 = 65.0;

/// The middle of the canyon at the distance `s` toward the sun, in meters to the right of
/// the sun direction (negative: to the left).
pub(super) fn canyon_center(s: f32) -> f32 {
    -45.0 + 30.0 * (s / 300.0).sin()
}

/// The width of the canyon from rim to rim at the distance `s` toward the sun.
pub(super) fn canyon_width(s: f32) -> f32 {
    100.0 + 0.07 * s.max(0.0)
}

/// The depth of the canyon at the distance `s` toward the sun and `a` to the right. It
/// begins on the plain below the escarpment.
fn canyon_depth(s: f32, a: f32) -> f32 {
    let rim = 0.5 * canyon_width(s) + 8.0 * fbm(vec2(s / 45.0, 3.0), 3, 7);
    let d = (a - canyon_center(s)).abs();
    let wall = 1.0 - smoothstep(rim - 22.0, rim, d);
    CANYON_DEPTH * wall * smoothstep(160.0, 300.0, s)
}

/// A mesh of rings around the origin. `vertex(p)` gives the height, the normal and the
/// vertex color at the point `p` (x, z). The UVs are x and z in meters.
fn polar_grid(radii: &[f32], vertex: impl Fn(Vec2) -> (f32, Vec3, Vec4) + Sync) -> Geometry {
    let s = SEGMENTS as usize;
    let points: Vec<Vec2> = std::iter::once(Vec2::ZERO)
        .chain(radii.iter().flat_map(|&r| {
            (0..s).map(move |j| {
                let angle = j as f32 / s as f32 * TAU;
                vec2(angle.cos(), angle.sin()) * r
            })
        }))
        .collect();
    let data: Vec<(f32, Vec3, Vec4)> = std::thread::scope(|scope| {
        let chunks: Vec<_> = points
            .chunks(points.len().div_ceil(16))
            .map(|chunk| scope.spawn(|| chunk.iter().map(|&p| vertex(p)).collect::<Vec<_>>()))
            .collect();
        chunks
            .into_iter()
            .flat_map(|h| h.join().unwrap_or_default())
            .collect()
    });
    let positions = points
        .iter()
        .zip(&data)
        .map(|(p, (h, _, _))| vec3(p.x, *h, p.y))
        .collect();
    let normals = data.iter().map(|(_, n, _)| *n).collect();
    let uvs = points.clone();
    let mut indices = Vec::with_capacity(radii.len() * s * 6);
    let ring = |k: usize, j: usize| (1 + k * s + j % s) as u32;
    for j in 0..s {
        indices.extend([0, ring(0, j + 1), ring(0, j)]);
    }
    for k in 0..radii.len() - 1 {
        for j in 0..s {
            let (a, b) = (ring(k, j), ring(k, j + 1));
            let (c, d) = (ring(k + 1, j), ring(k + 1, j + 1));
            indices.extend([a, b, c, b, d, c]);
        }
    }
    let mut geometry = Geometry::new(positions, normals, uvs, indices);
    geometry.colors = data.iter().map(|(_, _, c)| *c).collect();
    geometry
}

/// A rock of radius about 1 m: a sphere with noise. The bottom is flat. Each triangle
/// has its own vertices and its own UV projection, so the texture does not stretch.
pub(super) fn rock(seed: u32, detail: u32) -> Geometry {
    let sphere = Geometry::icosahedron(1.0, detail);
    let shape = |d: Vec3| {
        let n = |q: Vec3, s: u32| noise2(vec2(q.x, q.z) + q.y * 1.7, seed * 17 + s) * 2.0 - 1.0;
        // big lumps and some flat faces of broken stone
        let mut r = 1.0 + 0.28 * n(d * 1.3, 1) + 0.12 * n(d * 3.1, 2) + 0.04 * n(d * 7.0, 3);
        for k in 0..5 {
            let plane = random_direction(seed * 5 + k);
            r = r.min(0.92 / d.dot(plane).max(0.3));
        }
        let mut p = d * r;
        p.y = p.y.max(-0.35);
        p
    };
    let moved: Vec<Vec3> = sphere.positions.iter().map(|&p| shape(p)).collect();
    let welded = Geometry::new(
        moved.clone(),
        Vec::new(),
        Vec::new(),
        sphere.indices.clone(),
    );
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut uvs = Vec::new();
    for tri in sphere.indices.as_chunks::<3>().0 {
        let [a, b, c] = tri.map(|i| moved[i as usize]);
        let face = (b - a).cross(c - a);
        let axis = face.abs();
        let project = |p: Vec3| {
            if axis.x >= axis.y && axis.x >= axis.z {
                vec2(p.z, p.y)
            } else if axis.y >= axis.z {
                vec2(p.x, p.z)
            } else {
                vec2(p.x, p.y)
            }
        };
        for &i in tri {
            let p = moved[i as usize];
            positions.push(p);
            normals.push(welded.normals[i as usize]);
            uvs.push(project(p) + vec2(hash(seed), hash(seed + 1)) * 4.0);
        }
    }
    let indices = (0..positions.len() as u32).collect();
    Geometry::new(positions, normals, uvs, indices)
}

/// A mesa at `center` (x, z): a flat top `height` meters high, a cliff, and a slope of
/// scree down to the plain.
fn mesa(center: Vec2, radius: f32, height: f32, seed: u32) -> Geometry {
    let n = 64;
    let outline: Vec<Vec2> = (0..n)
        .map(|i| {
            let angle = i as f32 / n as f32 * TAU;
            let dir = vec2(angle.cos(), angle.sin());
            let wobble = 0.3 * fbm(dir * 1.8 + seed as f32 * 3.7, 4, seed);
            dir * radius * (1.0 + wobble) * vec2(1.0, 0.7 + 0.3 * hash(seed))
        })
        .collect();
    // height, scale of the outline: the top, the foot of the cliff, the foot of the scree
    let rings = [
        (height, 1.0),
        (height * 0.97, 1.01),
        (height * 0.5, 1.06),
        (-10.0, 1.0 + 1.1 * height / radius),
    ];
    let mut parts = Vec::new();
    for w in rings.windows(2) {
        let [(y0, s0), (y1, s1)] = [w[0], w[1]];
        let mut positions = Vec::new();
        for (y, s) in [(y0, s0), (y1, s1)] {
            for p in &outline {
                let q = center + *p * s;
                positions.push(vec3(q.x, y, q.y));
            }
        }
        let mut indices = Vec::new();
        for i in 0..n {
            let (a, b) = (i as u32, ((i + 1) % n) as u32);
            let (c, d) = (a + n as u32, b + n as u32);
            indices.extend([a, b, c, b, d, c]);
        }
        let uvs = positions.iter().map(|p| vec2(p.x + p.z, p.y)).collect();
        let mut part = Geometry::new(positions, Vec::new(), uvs, indices);
        // bands of rock in other tones
        part.colors = part
            .positions
            .iter()
            .map(|p| {
                let band = 0.8 + 0.08 * noise2(vec2(p.y / 14.0, 0.5), seed);
                vec4(band * 0.8, band * 0.78, band * 0.8, 1.0)
            })
            .collect();
        parts.push(part);
    }
    let mut positions = vec![vec3(center.x, height, center.y)];
    positions.extend(
        outline
            .iter()
            .map(|p| vec3(center.x + p.x, height, center.y + p.y)),
    );
    let mut indices = Vec::new();
    for i in 0..n as u32 {
        indices.extend([0, 1 + (i + 1) % n as u32, 1 + i]);
    }
    let uvs = positions.iter().map(|p| vec2(p.x, p.z)).collect();
    let mut top = Geometry::new(positions, Vec::new(), uvs, indices);
    top.colors = vec![vec4(0.72, 0.68, 0.66, 1.0); top.positions.len()];
    parts.push(top);
    Geometry::merge(parts)
}

impl Ground {
    /// Adds the ground to `scene`. `tint` multiplies the color of the sand and the mud.
    pub fn add_to(&self, scene: &mut Scene, tint: Rgb) {
        let tinted = |m: &Material| m.clone().color(m.color * tint);
        scene.add(Node::mesh(self.sand.0.clone(), tinted(&self.sand.1)).shadows(false, true));
        scene.add(Node::mesh(self.playa.0.clone(), tinted(&self.playa.1)).shadows(false, true));
        for (geometry, material, instances) in &self.rocks {
            scene.add(Node::instanced(
                geometry.clone(),
                material.clone(),
                instances.clone(),
            ));
        }
        scene.add(Node::mesh(self.mesas.0.clone(), self.mesas.1.clone()).shadows(false, true));
    }
}
