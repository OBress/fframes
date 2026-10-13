//! Reads `dynamic_media/nyc/manhattan.bin`, which `nyc.py` makes from the NYC 3D Building
//! Model and the open data of the city. See `nyc.py` for the layout of the file.
//!
//! Frame: meters, Y up, origin at the Empire State Building, +Z uptown along the avenues,
//! +X to the west.

use std::num::NonZero;
use std::sync::Mutex;

use fframes_3d::{Vec2, Vec3, vec2, vec3};

/// One building of the model.
pub struct Building {
    /// The Building Identification Number of the city.
    pub bin: u32,
    /// The year of construction, 0 when it is not known.
    pub year: u16,
    /// A building of the far ring: a box around the real shape.
    pub far: bool,
    /// The height in meters.
    pub height: f32,
    /// The center of the footprint on the ground (x, z).
    pub center: Vec2,
    /// Roof triangles, three corners each, counterclockwise seen from outside.
    pub roofs: Vec<[Vec3; 3]>,
    /// Wall triangles.
    pub walls: Vec<[Vec3; 3]>,
}

/// The kind of a street.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Class {
    Street,
    Avenue,
    Broadway,
    Highway,
}

/// A street segment or a route: a polyline on the ground.
pub struct Street {
    pub class: Class,
    pub lanes: u8,
    /// The cars go along the points (one way), or both ways.
    pub one_way: bool,
    /// The width of the roadway in meters.
    pub width: f32,
    pub points: Vec<Vec2>,
}

/// A building outside the near box as a rectangle: in the other boroughs, in the north and
/// the south of Manhattan, and in New Jersey.
#[derive(Debug, Clone, Copy)]
pub struct Block {
    pub center: Vec2,
    /// Half the length along `angle`, and half the width.
    pub half: Vec2,
    /// The direction of the length, in radians from +X toward +Z.
    pub angle: f32,
    pub height: f32,
}

/// The kind of a light line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineKind {
    Street,
    Highway,
    Bridge,
    Path,
    Ramp,
}

pub struct Manhattan {
    pub buildings: Vec<Building>,
    /// The street segments of Manhattan, block by block.
    pub streets: Vec<Street>,
    /// The avenues as long chains of segments, oriented with the traffic.
    pub routes: Vec<Street>,
    /// The bridges as polylines.
    pub bridges: Vec<Vec<Vec2>>,
    /// The land of the five boroughs, as triangles on the ground.
    pub land: Vec<[Vec2; 3]>,
    /// The parks of Manhattan, as triangles on the ground.
    pub parks: Vec<[Vec2; 3]>,
    /// All other buildings within 20 km.
    pub blocks: Vec<Block>,
    /// The streets, highways, bridges, ramps and park paths of the five boroughs.
    pub lines: Vec<(LineKind, Vec<Vec2>)>,
}

/// Turns around the walls that face into their building. A wall faces in when rays from
/// it to the outside cross the building an odd number of times.
fn face_out(b: &mut Building) {
    let all: Vec<[Vec3; 3]> = b.roofs.iter().chain(&b.walls).copied().collect();
    // Whether the ray `o + d * s` crosses the triangle `t` after its start.
    let crosses = |o: Vec3, d: Vec3, t: &[Vec3; 3]| {
        let (e1, e2) = (t[1] - t[0], t[2] - t[0]);
        let p = d.cross(e2);
        let det = e1.dot(p);
        if det.abs() < 1e-9 {
            return false;
        }
        let s = o - t[0];
        let u = s.dot(p) / det;
        let q = s.cross(e1);
        let v = d.dot(q) / det;
        u >= 0.0 && v >= 0.0 && u + v <= 1.0 && e2.dot(q) / det > 1e-3
    };
    // Three directions, so an open seam of the model does not turn a good wall around.
    let skews = [
        vec3(0.013, 0.007, -0.011),
        vec3(-0.21, 0.05, 0.17),
        vec3(0.08, 0.35, 0.02),
    ];
    for t in &mut b.walls {
        let n = (t[1] - t[0]).cross(t[2] - t[0]).normalize_or_zero();
        let c = (t[0] + t[1] + t[2]) / 3.0 + n * 0.01;
        let inward = n != Vec3::ZERO
            && skews.iter().all(|&skew| {
                let d = (n + skew).normalize();
                all.iter().filter(|o| crosses(c, d, o)).count() % 2 == 1
            });
        if inward {
            t.swap(1, 2);
        }
    }
}

/// Reads little endian values from a byte slice.
struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl Reader<'_> {
    fn take<const N: usize>(&mut self) -> Result<[u8; N], String> {
        let end = self.at + N;
        let slice = self
            .bytes
            .get(self.at..end)
            .ok_or_else(|| "manhattan.bin: the file ends too early".to_owned())?;
        self.at = end;
        Ok(slice.try_into().unwrap_or([0; N]))
    }

    fn u8(&mut self) -> Result<u8, String> {
        Ok(self.take::<1>()?[0])
    }

    fn u16(&mut self) -> Result<u16, String> {
        Ok(u16::from_le_bytes(self.take()?))
    }

    fn u32(&mut self) -> Result<u32, String> {
        Ok(u32::from_le_bytes(self.take()?))
    }

    fn f32(&mut self) -> Result<f32, String> {
        Ok(f32::from_le_bytes(self.take()?))
    }

    fn count(&mut self) -> Result<usize, String> {
        Ok(self.u32()? as usize)
    }

    fn vec2(&mut self) -> Result<Vec2, String> {
        Ok(vec2(self.f32()?, self.f32()?))
    }

    fn triangles(&mut self, count: usize) -> Result<Vec<[Vec3; 3]>, String> {
        let mut out = Vec::with_capacity(count);
        for _ in 0..count {
            let mut tri = [Vec3::ZERO; 3];
            for p in &mut tri {
                *p = vec3(self.f32()?, self.f32()?, self.f32()?);
            }
            out.push(tri);
        }
        Ok(out)
    }

    fn polyline(&mut self) -> Result<Vec<Vec2>, String> {
        let n = self.count()?;
        (0..n).map(|_| self.vec2()).collect()
    }

    fn streets(&mut self) -> Result<Vec<Street>, String> {
        let n = self.count()?;
        let mut out = Vec::with_capacity(n);
        for _ in 0..n {
            let class = match self.u8()? {
                1 => Class::Avenue,
                2 => Class::Broadway,
                3 => Class::Highway,
                _ => Class::Street,
            };
            let lanes = self.u8()?;
            let direction = self.u8()?;
            self.u8()?;
            let width = self.f32()?;
            let mut points = self.polyline()?;
            // The file gives the direction of the traffic. Here the points always go with
            // the traffic.
            if direction == 2 {
                points.reverse();
            }
            out.push(Street {
                class,
                lanes,
                one_way: direction != 0,
                width,
                points,
            });
        }
        Ok(out)
    }

    fn areas(&mut self) -> Result<Vec<[Vec2; 3]>, String> {
        let n = self.count()?;
        let mut out = Vec::with_capacity(n);
        for _ in 0..n {
            out.push([self.vec2()?, self.vec2()?, self.vec2()?]);
        }
        Ok(out)
    }
}

impl Manhattan {
    /// Turns around the walls that face into their building, on all buildings but the
    /// boxes of the far ring. A few walls of the model have the wrong order of corners,
    /// and the renderer does not draw the back of a wall. It takes about half a second.
    pub fn face_walls_out(&mut self) {
        let queue = Mutex::new(self.buildings.chunks_mut(16).collect::<Vec<_>>());
        // The lock is free again when `next` returns.
        let next = || queue.lock().ok().and_then(|mut q| q.pop());
        let threads = std::thread::available_parallelism().map_or(4, NonZero::get);
        std::thread::scope(|s| {
            for _ in 0..threads {
                s.spawn(|| {
                    while let Some(part) = next() {
                        part.iter_mut().filter(|b| !b.far).for_each(face_out);
                    }
                });
            }
        });
    }

    /// Reads the file made by `nyc.py`.
    pub fn load(bytes: &[u8]) -> Result<Self, String> {
        let mut r = Reader { bytes, at: 0 };
        if &r.take::<4>()? != b"NYC2" {
            return Err("manhattan.bin: not a NYC2 file, run fetch-nyc.sh".to_owned());
        }
        let n = r.count()?;
        let mut buildings = Vec::with_capacity(n);
        for _ in 0..n {
            let bin = r.u32()?;
            let year = r.u16()?;
            let flags = r.u16()?;
            let height = r.f32()?;
            let center = r.vec2()?;
            let (roof_count, wall_count) = (r.count()?, r.count()?);
            let roofs = r.triangles(roof_count)?;
            let walls = r.triangles(wall_count)?;
            buildings.push(Building {
                bin,
                year,
                far: flags & 1 != 0,
                height,
                center,
                roofs,
                walls,
            });
        }
        let streets = r.streets()?;
        let routes = r.streets()?;
        let bridges = (0..r.count()?)
            .map(|_| r.polyline())
            .collect::<Result<_, _>>()?;
        let land = r.areas()?;
        let parks = r.areas()?;
        let n = r.count()?;
        let mut blocks = Vec::with_capacity(n);
        for _ in 0..n {
            blocks.push(Block {
                center: r.vec2()?,
                half: r.vec2()?,
                angle: r.f32()?,
                height: r.f32()?,
            });
        }
        let n = r.count()?;
        let mut lines = Vec::with_capacity(n);
        for _ in 0..n {
            let kind = match r.u8()? {
                1 => LineKind::Highway,
                2 => LineKind::Bridge,
                3 => LineKind::Path,
                4 => LineKind::Ramp,
                _ => LineKind::Street,
            };
            lines.push((kind, r.polyline()?));
        }
        Ok(Self {
            buildings,
            streets,
            routes,
            bridges,
            land,
            parks,
            blocks,
            lines,
        })
    }
}
