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
const SEGMENTS: u32 = 512;
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
    /// The sand of the sunrise, with wind ripples, see [`dawn_sand`].
    dawn_sand: Material,
    /// The canyon of the sunrise.
    canyon: (Arc<Geometry>, Material),
    playa: (Arc<Geometry>, Material),
    /// Rocks and dry shrubs. Each shape is one instanced mesh.
    scatter: Vec<(Arc<Geometry>, Material, Arc<Instances>)>,
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
            dawn_sand: dawn_sand(assets, &sand),
            canyon: (
                Arc::new(self.canyon_mesh()),
                rock.clone().env_intensity(0.6),
            ),
            sand: (Arc::new(self.sand_mesh()), sand),
            playa: (Arc::new(self.playa_mesh()), playa),
            scatter: [self.rocks(&rock), self.shrubs()].concat(),
            mesas: (
                Arc::new(self.mesas()),
                rock.color(0x9a9aa4).normal_scale(0.6),
            ),
        }
    }

    /// How far the sand mesh lies below the ground at `p` (x, z), in meters: 4 m inside the
    /// canyon mesh, and 0 at its edge.
    fn under_canyon(&self, p: Vec2) -> f32 {
        let (s, a) = self.sunward(p);
        let d = (a - canyon_center(s)).abs() - canyon_rim(s);
        let inside = smoothstep(CANYON_FROM, CANYON_FROM + 40.0, s)
            * (1.0 - smoothstep(CANYON_TO - 200.0, CANYON_TO, s))
            * (1.0 - smoothstep(0.3, 1.0, d / canyon_margin(s)));
        4.0 * inside
    }

    /// The canyon below the plateau of the sunrise, with its own vertices: the sand mesh
    /// is too coarse for its cliffs. The columns are closer in the walls. The colors are
    /// bands of red and pale rock by height, with sand on the ledges. The UVs are in
    /// meters: U goes along the canyon, V across and down, so the cliffs do not stretch
    /// the texture.
    fn canyon_mesh(&self) -> Geometry {
        let mut rows = Vec::new();
        let mut s = CANYON_FROM;
        while s < CANYON_TO {
            rows.push(s);
            s += 2.0 + 0.004 * s;
        }
        // The columns on one side: 0 to 1 is the floor, 1 to 2 the wall, 2 to 3 the plain
        // beyond the rim.
        let floor = (0..17).map(|k| k as f32 / 17.0);
        let wall = (0..=72).map(|k| 1.0 + k as f32 / 72.0);
        let plain = (1..=12).map(|k| 2.0 + k as f32 / 12.0);
        let side: Vec<f32> = floor.chain(wall).chain(plain).collect();
        let columns: Vec<f32> = side
            .iter()
            .rev()
            .map(|d| -d)
            .chain(side.iter().skip(1).copied())
            .collect();
        let offset = |s: f32, c: f32| {
            let rim = canyon_rim(s);
            let d = c.abs();
            let meters = if d < 1.0 {
                (rim - WALL) * d
            } else if d <= 2.0 {
                rim - WALL + WALL * (d - 1.0)
            } else {
                rim + canyon_margin(s) * (d - 2.0)
            };
            meters * c.signum()
        };
        let mut positions = Vec::new();
        let mut normals = Vec::new();
        let mut uvs = Vec::new();
        let mut colors = Vec::new();
        for &s in &rows {
            for &c in &columns {
                let a = canyon_center(s) + offset(s, c);
                let p = self.sunward_point(s, a);
                let h = self.height(p.x, p.y);
                let n = self.normal(p.x, p.y, 0.4);
                positions.push(vec3(p.x, h, p.y));
                normals.push(n);
                uvs.push(vec2(s, offset(s, c).abs() - h));
                // Layers of rock: red and pale bands, thick and thin.
                let layer =
                    0.6 * noise2(vec2(h / 5.5, 0.5), 51) + 0.4 * noise2(vec2(h / 1.7, 0.5), 52);
                let red = vec3(0.66, 0.38, 0.26);
                let pale = vec3(0.86, 0.72, 0.56);
                let rock = red.lerp(pale, smoothstep(0.3, 0.7, layer)) * (0.85 + 0.2 * layer);
                let sand = vec3(0.9, 0.78, 0.62) * (0.82 + 0.3 * fbm(p / 40.0, 3, 11));
                // the ledges and the floor hold sand, the cliffs are bare
                let color = sand.lerp(rock, smoothstep(0.3, 0.6, 1.0 - n.y));
                colors.push(color.extend(1.0));
            }
        }
        let width = columns.len() as u32;
        let mut indices = Vec::new();
        for r in 0..rows.len() as u32 - 1 {
            for c in 0..width - 1 {
                let (a, b) = (r * width + c, r * width + c + 1);
                indices.extend([a, a + width, b, b, a + width, b + width]);
            }
        }
        let mut geometry = Geometry::new(positions, normals, uvs, indices);
        geometry.colors = colors;
        geometry
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
            let h = self.height(p.x, p.y) - self.under_canyon(p);
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
                // Big rocks stay out of the sunrise view, and stones out of the lower left of
                // it, where the quote is.
                let (s, across) = self.sunward(p);
                if class >= 2 && s > -60.0 && across.abs() < if s > 60.0 { 900.0 } else { 30.0 } {
                    continue;
                }
                if class == 1 && (0.0..40.0).contains(&s) && (-30.0..1.0).contains(&across) {
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

    /// Dry shrubs, from 0.4 m to 1.4 m wide, in four shapes. None grows on the path of
    /// the engineer, on the line or its light, in the middle of the view of the walk, near
    /// the sunrise camera, or in the lower left of the sunrise view, where the quote is.
    fn shrubs(&self) -> Vec<(Arc<Geometry>, Material, Arc<Instances>)> {
        let wood = Material::standard(0x4a3c30).roughness(0.9);
        let walk = kneel_heading();
        let walk_dir = vec2(walk.sin(), walk.cos());
        let (a, side) = (ahead(), right());
        let (a, side) = (vec2(a.x, a.z), vec2(side.x, side.z));
        let mut lists: [Vec<Mat4>; 4] = Default::default();
        for k in 0..2400u32 {
            let h = |n: u32| hash(k * 29 + n * 7919 + 1301);
            let r = 3.0 + 300.0 * h(1).powf(2.2);
            let angle = h(2) * TAU;
            let p = vec2(angle.cos(), angle.sin()) * r;
            let along = p.dot(a);
            if along > -4.0 && p.dot(side).abs() < 2.5 + 0.03 * along {
                continue;
            }
            let back = -p.dot(walk_dir);
            if back > -2.0 && back < 15.0 && (p + walk_dir * back).length() < 2.5 {
                continue;
            }
            // the middle of the view of the walk, between the camera and the factory
            let near = along + 9.0;
            if (0.0..50.0).contains(&near) && (p.dot(side) - 2.0).abs() < 3.0 + 0.25 * near {
                continue;
            }
            let (s, across) = self.sunward(p);
            if (0.0..42.0).contains(&s) && (-30.0..8.0).contains(&across) {
                continue;
            }
            // fewer on the mud of the pan, more on the sand around it
            if r < PLAYA && h(3) > 0.25 {
                continue;
            }
            let size = 0.4 + 1.0 * h(4).powi(2);
            let rotation = Quat::from_rotation_y(h(5) * TAU);
            let position = vec3(p.x, self.height(p.x, p.y) - 0.02, p.y);
            lists[(h(6) * 4.0) as usize % 4].push(Mat4::from_scale_rotation_translation(
                vec3(size, size * (0.7 + 0.5 * h(7)), size),
                rotation,
                position,
            ));
        }
        lists
            .into_iter()
            .enumerate()
            .map(|(k, list)| {
                (
                    Arc::new(shrub(k as u32)),
                    wood.clone(),
                    Arc::new(Instances::new(list)),
                )
            })
            .collect()
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

/// The width of the walls of the canyon from the rim to the floor, in meters.
const WALL: f32 = 34.0;
/// The number of cliffs in a wall. A ledge lies between two cliffs.
const CLIFFS: f32 = 4.0;

/// The distance from the middle of the canyon to its rim at the distance `s` toward the
/// sun. Spurs of rock stand out of the walls, and gullies cut into them.
fn canyon_rim(s: f32) -> f32 {
    0.5 * canyon_width(s)
        + 8.0 * fbm(vec2(s / 45.0, 3.0), 3, 7)
        + 7.0 * fbm(vec2(s / 38.0, 9.0), 2, 8)
}

/// The depth of the canyon at the distance `s` toward the sun and `a` to the right. It
/// begins on the plain below the escarpment. Each wall goes down in [`CLIFFS`] steps: a
/// cliff of hard rock, then a ledge of scree, as layers of rock erode.
fn canyon_depth(s: f32, a: f32) -> f32 {
    let d = (a - canyon_center(s)).abs();
    let x = ((canyon_rim(s) - d) / WALL).clamp(0.0, 1.0) * CLIFFS;
    let step = x.floor();
    let cliff = smoothstep(0.0, 0.3, x - step);
    let ledge = 0.12 * (x - step);
    let fall = ((step + cliff * 0.88 + ledge) / CLIFFS).min(1.0);
    CANYON_DEPTH * fall * smoothstep(160.0, 300.0, s)
}

/// The canyon mesh begins at this distance toward the sun, in meters.
const CANYON_FROM: f32 = 170.0;
/// The canyon mesh ends at this distance toward the sun, in meters.
const CANYON_TO: f32 = 2600.0;

/// How far the canyon mesh goes beyond the rim at the distance `s` toward the sun, in
/// meters. Far away the sand mesh is coarse, so the canyon mesh must cover more.
fn canyon_margin(s: f32) -> f32 {
    30.0 + 0.05 * s
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

/// A dry shrub about 1 m wide and 0.6 m high: branches grow from one root, bend out and up,
/// and fork into twigs.
fn shrub(seed: u32) -> Geometry {
    let mut parts = Vec::new();
    for b in 0..22u32 {
        let h = |n: u32| hash(seed * 997 + b * 31 + n);
        let angle = h(1) * TAU;
        let out = vec3(angle.cos(), 0.0, angle.sin());
        let length = 0.3 + 0.3 * h(2);
        // the middle branches stand up, the outer ones spread
        let steep = 0.7 + 1.6 * h(3).powi(2);
        let bend = vec3(h(4) - 0.5, 0.0, h(5) - 0.5) * 0.2;
        let point = |u: f32| {
            (out * u + bend * u * u) * length + Vec3::Y * (steep * length * u * (1.2 - 0.4 * u))
        };
        let path: Vec<Vec3> = (0..=4).map(|k| point(k as f32 / 4.0)).collect();
        parts.push(Geometry::tube(&path, 0.006, 4));
        // three twigs fork from the outer part of the branch
        for t in 0..3u32 {
            let from = point(0.35 + 0.2 * t as f32 + 0.1 * h(6 + t));
            let twig = random_direction(seed * 131 + b * 7 + t);
            let reach = (twig.with_y(twig.y.abs() + 0.5)).normalize() * (0.06 + 0.12 * h(9 + t));
            parts.push(Geometry::tube(
                &[from, from + reach * 0.5, from + reach],
                0.0025,
                3,
            ));
        }
    }
    Geometry::merge(parts)
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
    // The side in rings from the rim to the plain, one smooth mesh: a steep cliff in the
    // upper half, then a slope of scree. Gullies cut the cliff and boulders bulge the
    // scree, so no facet is flat.
    let rings = 18;
    let mut positions = Vec::new();
    let mut colors = Vec::new();
    for k in 0..=rings {
        let u = k as f32 / rings as f32;
        let cliff = smoothstep(0.0, 0.45, u);
        let scree = smoothstep(0.4, 1.0, u);
        for (i, p) in outline.iter().enumerate() {
            let angle = i as f32 / n as f32 * TAU;
            let bumps = fbm(vec2(angle * 5.0, u * 4.0 + seed as f32 * 2.1), 3, seed + 7);
            let out = 1.0 + 0.05 * bumps * (0.6 + scree);
            let s = (1.0 + 0.06 * cliff + 1.1 * height / radius * scree.powf(0.85)) * out;
            let y = height * (1.0 - 0.5 * cliff) - (height * 0.5 + 10.0) * scree
                + 0.03 * height * bumps * scree;
            let q = center + *p * s;
            positions.push(vec3(q.x, y, q.y));
            // bands of rock on the cliff, sand between the boulders of the scree
            let band = 0.8 + 0.08 * noise2(vec2(y / 14.0, 0.5), seed);
            let rock = vec3(band * 0.8, band * 0.78, band * 0.8);
            let sand = vec3(0.7, 0.65, 0.58);
            colors.push(rock.lerp(sand, 0.6 * scree).extend(1.0));
        }
    }
    let mut indices = Vec::new();
    for k in 0..rings as u32 {
        for i in 0..n as u32 {
            let (a, b) = (k * n as u32 + i, k * n as u32 + (i + 1) % n as u32);
            let (c, d) = (a + n as u32, b + n as u32);
            indices.extend([a, b, c, b, d, c]);
        }
    }
    let uvs = positions.iter().map(|p| vec2(p.x + p.z, p.y)).collect();
    let mut side = Geometry::new(positions, Vec::new(), uvs, indices);
    side.colors = colors;
    let mut parts = vec![side];
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
    /// Adds the ground of the night to `scene`: the sand, the pan of mud, the rocks, the
    /// shrubs and the mesas.
    pub fn add_to(&self, scene: &mut Scene) {
        scene.add(Node::mesh(self.sand.0.clone(), self.sand.1.clone()).shadows(false, true));
        scene.add(Node::mesh(self.playa.0.clone(), self.playa.1.clone()).shadows(false, true));
        // The red of the rock stays cold at night: the moon takes the color of a stone. The
        // mesas on the horizon are dark shapes against the sky.
        self.add_scatter(scene, Rgb::hex(0x8ea0c4), Rgb::hex(0x5a6684));
    }

    /// Adds the ground of the sunrise to `scene`. The sand has wind ripples and no pan of
    /// mud: the cracks of the mud and the fine grain of the sand flicker in the low sun.
    /// `tint` multiplies the color of the sand.
    pub fn add_dawn_to(&self, scene: &mut Scene, tint: Rgb) {
        let sand = self.dawn_sand.clone();
        let sand = sand.clone().color(sand.color * tint);
        scene.add(Node::mesh(self.sand.0.clone(), sand).shadows(false, true));
        let (canyon, rock) = &self.canyon;
        scene.add(
            Node::mesh(canyon.clone(), rock.clone().color(rock.color * tint * 4.0))
                .shadows(false, true),
        );
        self.add_scatter(scene, Rgb::WHITE, Rgb::WHITE);
    }

    /// Adds the rocks, the shrubs and the mesas. `tint` multiplies the color of the rocks
    /// and the shrubs, `far` the color of the mesas.
    fn add_scatter(&self, scene: &mut Scene, tint: Rgb, far: Rgb) {
        for (geometry, material, instances) in &self.scatter {
            scene.add(Node::instanced(
                geometry.clone(),
                material.clone().color(material.color * tint),
                instances.clone(),
            ));
        }
        let (mesas, rock) = &self.mesas;
        scene.add(
            Node::mesh(mesas.clone(), rock.clone().color(rock.color * far)).shadows(false, true),
        );
    }
}

/// The sand of the sunrise: the sand texture with wind ripples and a weaker grain.
///
/// In a low sun the fine grain of the sand texture makes strong light and dark specks that
/// crawl when the camera moves. Here the grain is at 40 % of its depth. Ripples of 0.9 to
/// 1.6 m show the wind instead: they are large enough to stay still in the picture. Their
/// crests turn and split, and in some patches the wind has smoothed them away. The map
/// repeats every [`DAWN_TILE`] meters.
fn dawn_sand(assets: &Assets, sand: &Material) -> Material {
    const SIZE: u32 = 2048;
    let texel = DAWN_TILE / SIZE as f32;
    // Smooth noise that repeats `n` times over the tile, in -1..1.
    let wave = |p: Vec2, n: f32, seed: u32| {
        let q = p / DAWN_TILE * n;
        let cell = q.floor();
        let f = q - cell;
        let u = f * f * (3.0 - 2.0 * f);
        let at = |i: f32, j: f32| {
            let (i, j) = (
                (cell.x + i).rem_euclid(n) as u32,
                (cell.y + j).rem_euclid(n) as u32,
            );
            hash(i * 7919 + j * 104_729 + seed * 31) * 2.0 - 1.0
        };
        let a = at(0.0, 0.0) + (at(1.0, 0.0) - at(0.0, 0.0)) * u.x;
        let b = at(0.0, 1.0) + (at(1.0, 1.0) - at(0.0, 1.0)) * u.x;
        a + (b - a) * u.y
    };
    // The height of the ripples in meters at `p` (x, z) in meters. The crests run across
    // the sunrise view. A whole number of waves fits the tile, so the map has no seam.
    let ripples = |p: Vec2| {
        let train = |k: Vec2, seed: u32| {
            let bend = 4.0 * wave(p, 2.0, seed)
                + 1.6 * wave(p, 5.0, seed + 1)
                + 0.5 * wave(p, 13.0, seed + 2);
            let phase = p.dot(k * TAU / DAWN_TILE) + bend;
            // The slope on the wind side is long and the lee side is short.
            0.5 + 0.5 * (phase + 0.5 * phase.sin()).sin()
        };
        // Two trains of ripples meet in patches. Where they meet, crests split and end.
        let mix = smoothstep(-0.35, 0.35, wave(p, 3.0, 20));
        let crest = train(vec2(-1.0, -9.0), 10) * (1.0 - mix) + train(vec2(-3.0, -10.0), 30) * mix;
        // In some patches the wind has smoothed the ripples away.
        let calm = smoothstep(-0.7, 0.5, wave(p, 2.0, 40) + 0.5 * wave(p, 6.0, 41));
        (0.003 + 0.011 * calm) * crest * crest
    };
    let grain = &assets.sand.normal;
    let mut data = vec![[0u8; 4]; (SIZE * SIZE) as usize];
    std::thread::scope(|scope| {
        for (row, chunk) in data.chunks_mut((SIZE * 64) as usize).enumerate() {
            scope.spawn(move || {
                for (i, out) in chunk.iter_mut().enumerate() {
                    let i = row * (SIZE * 64) as usize + i;
                    let (x, y) = ((i as u32 % SIZE) as f32, (i as u32 / SIZE) as f32);
                    let p = vec2(x + 0.5, y + 0.5) * texel;
                    let dx = (ripples(p + vec2(texel, 0.0)) - ripples(p - vec2(texel, 0.0)))
                        / (2.0 * texel);
                    let dy = (ripples(p + vec2(0.0, texel)) - ripples(p - vec2(0.0, texel)))
                        / (2.0 * texel);
                    let fine = grain.sample(p).truncate() * 2.0 - 1.0;
                    let n = vec3(fine.x * 0.4 - dx, fine.y * 0.4 + dy, fine.z).normalize();
                    let n = n * 0.5 + 0.5;
                    *out = [n.x, n.y, n.z, 1.0].map(|c| (c * 255.0).round() as u8);
                }
            });
        }
    });
    let normal = Texture::from_rgba8(SIZE, SIZE, data.as_flattened(), false)
        .repeat(1.0 / DAWN_TILE, 1.0 / DAWN_TILE);
    sand.clone().normal_map(Arc::new(normal))
}

/// The size of one repeat of the ripples of [`dawn_sand`], in meters.
const DAWN_TILE: f32 = 10.0;
