//! Small things for a second viewing. On the rail of the catwalk: a rubber duck and a coffee
//! mug beside the old laptop of the engineer, with stickers on its lid (shot 3). On a column
//! of the hall: a safety sign that forbids deploys on Friday (shot 2).

use std::sync::Arc;

use fframes_3d::math::{hash, smoothstep};
use fframes_3d::*;

use super::hall;

/// The closed laptop in meters: the width, the length from the hinge to the front edge, and
/// the thickness. It stands on its hinge on the rail, so the lid faces the hall.
pub const LAPTOP: Vec3 = Vec3::new(0.40, 0.27, 0.024);
/// The top of the top rail of the catwalk on the side of the hall, in the middle of its
/// width.
const RAIL_TOP: Vec3 = Vec3::new(0.0, hall::CATWALK_Y + 1.10, hall::CATWALK_Z + 0.78);
/// The duck and the mug on the rail. A hanger of the catwalk goes through the rail at
/// X = -3, between them and the laptop.
pub const DUCK: Vec3 = Vec3::new(-3.13, RAIL_TOP.y, RAIL_TOP.z);
const MUG: Vec3 = Vec3::new(-3.29, RAIL_TOP.y, RAIL_TOP.z - 0.005);
/// The safety sign: the middle of its face, on the flange of the column at X = 14,
/// Z = -60 that faces the camera in the flight of shot 2. The size is in meters.
const SIGN: Vec3 = Vec3::new(14.0, 3.5, -60.0 + 0.3 + 0.022);
const SIGN_SIZE: Vec2 = Vec2::new(1.6, 1.0);

/// Adds the duck, the mug and the safety sign to the scene. Returns the group of the duck
/// and the mug.
pub fn build(scene: &mut Scene, bold: &[u8]) -> NodeId {
    let props = scene.add(Node::group());
    duck(scene, props);
    mug(scene, props);
    sign(scene, bold);
    props
}

/// The laptop: an old 17 inch machine, closed, in the frame of its hinge. Y goes from the
/// hinge to the front edge and the lid faces +Z. `serif` is the font of the fframes logo.
pub fn laptop(scene: &mut Scene, bold: &[u8], serif: &[u8]) -> NodeId {
    let (w, l, d) = (LAPTOP.x, LAPTOP.y, LAPTOP.z);
    let laptop = scene.add(Node::group());
    let shell = Material::metal(0x5d6066, 0.5);
    // the base, the lid and the rubber feet on the bottom of the base
    let body = Geometry::merge([
        Geometry::rounded_box(w, l, d * 0.6, 0.006, 2).translate(0.0, l * 0.5, -d * 0.2),
        Geometry::rounded_box(w - 0.002, l - 0.002, d * 0.4, 0.005, 2).translate(
            0.0,
            l * 0.5,
            d * 0.3,
        ),
    ]);
    scene.add_to(laptop, Node::mesh(body, shell));
    let feet = Geometry::merge(
        [(-1.0f32, 0.04), (1.0, 0.04), (-1.0, 0.23), (1.0, 0.23)].map(|(side, y)| {
            Geometry::cylinder(0.006, 0.006, 0.003, 10)
                .rotate_x(deg(90.0))
                .translate(side * (w * 0.5 - 0.03), y, -d * 0.5 - 0.001)
        }),
    );
    scene.add_to(
        laptop,
        Node::mesh(feet, Material::standard(0x151515).roughness(0.8)),
    );
    let (color, metal_rough) = lid_textures(bold, serif);
    let mut lid = Material::standard(0xffffff).metalness(1.0).roughness(1.0);
    lid.map = Some(color);
    lid.metal_rough_map = Some(metal_rough);
    scene.add_to(
        laptop,
        Node::mesh(
            Geometry::plane(w - 0.012, l - 0.012).translate(0.0, l * 0.5, d * 0.5 + 0.0003),
            lid,
        ),
    );
    laptop
}

/// A classic yellow rubber duck, 10 cm long, on its flat bottom. It looks toward the hall
/// and a little toward the laptop.
fn duck(scene: &mut Scene, parent: NodeId) {
    let yellow = Material::standard(0xffc81a).roughness(0.28).clearcoat(0.7);
    let body = Geometry::merge([
        // the body, flat underneath
        Geometry::sphere(0.045, 32, 20)
            .scale(1.12, 0.78, 0.92)
            .translate(0.0, 0.032, 0.0),
        // the tail rises at the back
        Geometry::sphere(0.02, 16, 10)
            .scale(1.3, 1.0, 1.0)
            .rotate_z(deg(-35.0))
            .translate(-0.043, 0.052, 0.0),
        // the head
        Geometry::sphere(0.027, 28, 18).translate(0.022, 0.083, 0.0),
    ]);
    let beak = Geometry::sphere(0.013, 20, 12)
        .scale(1.35, 0.5, 1.15)
        .translate(0.047, 0.077, 0.0);
    let eyes = Geometry::merge(
        [-1.0f32, 1.0]
            .map(|side| Geometry::sphere(0.0042, 10, 8).translate(0.037, 0.091, side * 0.017)),
    );
    let group = scene.add_to(
        parent,
        Node::group()
            .at(DUCK.x, DUCK.y, DUCK.z)
            .rotate_y(deg(-62.0)),
    );
    scene.add_to(group, Node::mesh(body, yellow));
    scene.add_to(
        group,
        Node::mesh(
            beak,
            Material::standard(0xff6a0a).roughness(0.3).clearcoat(0.6),
        ),
    );
    scene.add_to(
        group,
        Node::mesh(eyes, Material::standard(0x0a0a0a).roughness(0.15)),
    );
}

/// A white enamel mug of coffee, 9.5 cm tall. The handle points along the rail.
fn mug(scene: &mut Scene, parent: NodeId) {
    let outer = Geometry::lathe(
        &[
            vec2(0.0, 0.0),
            vec2(0.039, 0.0),
            vec2(0.041, 0.004),
            vec2(0.041, 0.095),
            vec2(0.037, 0.095),
            vec2(0.037, 0.008),
            vec2(0.0, 0.008),
        ],
        40,
    );
    let handle = Geometry::torus(0.025, 0.0055, 12, 24)
        .scale(0.8, 1.0, 1.0)
        .translate(0.041, 0.05, 0.0);
    let enamel = Material::standard(0xe9e6df).roughness(0.22).clearcoat(0.5);
    let group = scene.add_to(
        parent,
        Node::group().at(MUG.x, MUG.y, MUG.z).rotate_y(deg(200.0)),
    );
    scene.add_to(group, Node::mesh(Geometry::merge([outer, handle]), enamel));
    scene.add_to(
        group,
        Node::mesh(
            Geometry::cylinder(0.037, 0.037, 0.002, 32).translate(0.0, 0.078, 0.0),
            Material::standard(0x1a0d06).roughness(0.1),
        )
        .shadows(false, true),
    );
}

/// The safety sign on the column: an enamel plate on a backing rail, held by four bolts.
fn sign(scene: &mut Scene, bold: &[u8]) {
    let (w, h) = (SIGN_SIZE.x, SIGN_SIZE.y);
    let group = scene.add(Node::group().at(SIGN.x, SIGN.y, SIGN.z));
    let steel = Material::metal(0x4a4e55, 0.6);
    // two flat bars bolted to the flange of the column
    let bars = Geometry::merge(
        [-0.3f32, 0.3].map(|y| Geometry::cuboid(w * 0.75, 0.06, 0.012).translate(0.0, y, -0.014)),
    );
    scene.add_to(group, Node::mesh(bars, steel.clone()));
    scene.add_to(
        group,
        Node::mesh(
            Geometry::rounded_box(w, h, 0.006, 0.01, 2).translate(0.0, 0.0, -0.004),
            Material::metal(0x8a8d90, 0.55),
        ),
    );
    let face = Material::standard(0xffffff)
        .roughness(0.55)
        .map(sign_texture(bold));
    scene.add_to(group, Node::mesh(Geometry::plane(w, h), face));
    let bolts = Geometry::merge(
        [(-1.0f32, -1.0f32), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)].map(|(x, y)| {
            Geometry::cylinder(0.011, 0.012, 0.006, 12)
                .rotate_x(deg(90.0))
                .translate(x * (w * 0.5 - 0.05), y * (h * 0.5 - 0.05), 0.003)
        }),
    );
    scene.add_to(group, Node::mesh(bolts, steel));
}

// ---- Textures

/// A color in sRGB, `0..1`.
type Srgb = Vec3;

fn srgb(hex: u32) -> Srgb {
    vec3(
        ((hex >> 16) & 0xff) as f32,
        ((hex >> 8) & 0xff) as f32,
        (hex & 0xff) as f32,
    ) / 255.0
}

/// A point of a painted surface: its color, its roughness and its metalness.
#[derive(Clone, Copy)]
struct Paint {
    color: Srgb,
    roughness: f32,
    metalness: f32,
}

/// Paints a texture of `width` x `height` texels over a rectangle of `size` meters. `paint`
/// gives the paint at a point in meters, X to the right and Y down from the top left corner.
/// Each texel is the mean of 2 x 2 samples. The second texture holds the roughness in green
/// and the metalness in blue.
fn bake(width: u32, height: u32, size: Vec2, paint: impl Fn(Vec2) -> Paint) -> (Texture, Texture) {
    let mut color = Vec::with_capacity((width * height * 4) as usize);
    let mut data = Vec::with_capacity((width * height * 4) as usize);
    let texel = size / vec2(width as f32, height as f32);
    let byte = |v: f32| (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
    for y in 0..height {
        for x in 0..width {
            let corner = vec2(x as f32, y as f32) * texel;
            let mut sum = Paint {
                color: Vec3::ZERO,
                roughness: 0.0,
                metalness: 0.0,
            };
            for (sx, sy) in [(0.25, 0.25), (0.75, 0.25), (0.25, 0.75), (0.75, 0.75)] {
                let p = paint(corner + vec2(sx, sy) * texel);
                sum.color += p.color * 0.25;
                sum.roughness += p.roughness * 0.25;
                sum.metalness += p.metalness * 0.25;
            }
            color.extend([byte(sum.color.x), byte(sum.color.y), byte(sum.color.z), 255]);
            data.extend([0, byte(sum.roughness), byte(sum.metalness), 255]);
        }
    }
    (
        Texture::from_rgba8(width, height, &color, true),
        Texture::from_rgba8(width, height, &data, false),
    )
}

/// Smooth value noise in `0..1` with cells of one unit.
fn value(p: Vec2, seed: u32) -> f32 {
    let i = p.floor();
    let f = p - i;
    let f = f * f * (Vec2::splat(3.0) - 2.0 * f);
    let at = |dx: f32, dy: f32| {
        let (x, y) = ((i.x + dx) as i32 as u32, (i.y + dy) as i32 as u32);
        hash(x.wrapping_mul(0x8da6_b343) ^ y.wrapping_mul(0xd816_3841) ^ seed)
    };
    let top = at(0.0, 0.0) + (at(1.0, 0.0) - at(0.0, 0.0)) * f.x;
    let bottom = at(0.0, 1.0) + (at(1.0, 1.0) - at(0.0, 1.0)) * f.x;
    top + (bottom - top) * f.y
}

/// Thin random scratches: `0..1`, 1 on a scratch. There is one scratch in each cell of
/// `cell` meters, `width` meters wide.
fn scratches(p: Vec2, cell: f32, width: f32, seed: u32) -> f32 {
    let c = (p / cell).floor();
    let mut best = f32::MAX;
    for dy in -1..=1 {
        for dx in -1..=1 {
            let k = c + vec2(dx as f32, dy as f32);
            let n = (k.x as i32 as u32).wrapping_mul(0x27d4_eb2d) ^ (k.y as i32 as u32) ^ seed;
            let center = (k + vec2(hash(n), hash(n ^ 0x55))) * cell;
            let angle = hash(n ^ 0x99) * 0.9 - 0.45;
            let half = cell * (0.3 + 0.9 * hash(n ^ 0x3c));
            let dir = vec2(angle.cos(), angle.sin()) * half;
            best = best.min(segment(p, center - dir, center + dir));
        }
    }
    1.0 - smoothstep(width * 0.5, width, best)
}

/// The distance from `p` to the segment from `a` to `b`.
fn segment(p: Vec2, a: Vec2, b: Vec2) -> f32 {
    let ab = b - a;
    let t = ((p - a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0);
    p.distance(a + ab * t)
}

/// Text as filled outlines in a flat frame in meters, Y down, for textures.
struct Ink {
    /// Per glyph: its box and its contours.
    glyphs: Vec<(Vec2, Vec2, Vec<Vec<Vec2>>)>,
}

impl Ink {
    /// `text` in `font` with the em size `size` in meters. The middle of the line, half way
    /// up the capitals, is at `center`.
    fn new(font: &[u8], text: &str, size: f32, center: Vec2) -> Self {
        let shapes = Geometry::text_shapes(font, text, size, 0.0).unwrap_or_default();
        let width = shapes.last().map_or(0.0, |g| g.origin.x + g.advance);
        let origin = center + vec2(-width * 0.5, size * 0.36);
        let glyphs = shapes
            .into_iter()
            .map(|g| {
                let contours: Vec<Vec<Vec2>> = g
                    .shapes
                    .iter()
                    .flat_map(|s| std::iter::once(&s.outer).chain(&s.holes))
                    .map(|c| {
                        c.iter()
                            .map(|p| origin + vec2(g.origin.x + p.x, -(g.origin.y + p.y)))
                            .collect()
                    })
                    .collect();
                let points = contours.iter().flatten();
                let min = points.clone().fold(Vec2::MAX, |m, p| m.min(*p));
                let max = points.fold(Vec2::MIN, |m, p| m.max(*p));
                (min, max, contours)
            })
            .collect();
        Self { glyphs }
    }

    /// The index of the letter that covers `p`, by the even-odd rule.
    fn letter(&self, p: Vec2) -> Option<usize> {
        self.glyphs.iter().position(|(min, max, contours)| {
            if p.cmplt(*min).any() || p.cmpgt(*max).any() {
                return false;
            }
            let mut inside = false;
            for c in contours {
                for (a, b) in c.iter().zip(c.iter().cycle().skip(1)) {
                    if (a.y > p.y) != (b.y > p.y)
                        && p.x < a.x + (p.y - a.y) / (b.y - a.y) * (b.x - a.x)
                    {
                        inside = !inside;
                    }
                }
            }
            inside
        })
    }
}

/// A rectangular sticker with round corners, turned by `angle` radians around `center`.
/// One corner can peel: `peel` gives the corner (signs of X and Y) and how far the fold
/// line is from it along the diagonal, in meters.
struct Sticker {
    center: Vec2,
    half: Vec2,
    angle: f32,
    radius: f32,
    peel: Option<(Vec2, f32)>,
}

/// What a sticker shows at a point.
enum Layer {
    /// The printed face, at a point in the frame of the sticker.
    Face(Vec2),
    /// The white back of the flap that peels off.
    Back,
    /// Glue left on the surface under the flap.
    Glue,
    /// The soft shadow of the sticker edge or of the flap, `0..1`.
    Shadow(f32),
}

impl Sticker {
    fn local(&self, p: Vec2) -> Vec2 {
        let d = p - self.center;
        let (s, c) = (-self.angle).sin_cos();
        vec2(d.x * c - d.y * s, d.x * s + d.y * c)
    }

    /// The signed distance to the edge of the rounded rectangle, at a point in its frame.
    fn edge(&self, q: Vec2) -> f32 {
        let d = q.abs() - self.half + Vec2::splat(self.radius);
        d.max(Vec2::ZERO).length() + d.x.max(d.y).min(0.0) - self.radius
    }

    fn layer(&self, p: Vec2) -> Option<Layer> {
        let q = self.local(p);
        let inside = self.edge(q) < 0.0;
        if let Some((corner, fold)) = self.peel {
            let n = corner.normalize();
            let k = corner * self.half;
            // the distance from the corner along the diagonal
            let d = (k - q).dot(n);
            // the flap is the corner folded back over the fold line
            let mirrored = q + 2.0 * (d - fold) * n;
            if d > fold && d < 2.0 * fold && self.edge(mirrored) < 0.0 {
                return Some(Layer::Back);
            }
            if d > fold && d < 2.0 * fold + 0.004 && self.edge(mirrored) < 0.003 {
                return Some(Layer::Shadow(0.5));
            }
            if inside && d <= fold {
                return Some(Layer::Glue);
            }
        }
        if inside {
            return Some(Layer::Face(q));
        }
        let e = self.edge(q - vec2(0.0, -0.0008));
        (e < 0.0012).then(|| Layer::Shadow(0.35 * (1.0 - e / 0.0012)))
    }
}

/// Ferris the crab, the mascot of Rust, as a die cut sticker. `q` is in units of half its
/// width, Y up. Returns the color of the print, or `None` outside of the crab.
fn ferris(q: Vec2) -> Option<Srgb> {
    let orange = srgb(0xf74c00);
    let shade = srgb(0xc23a00);
    // the shell: an ellipse with a row of points along its top edge
    let body = {
        let p = q - vec2(0.0, -0.05);
        let tooth = 1.0 - ((p.x * 3.3 + 0.5).fract() * 2.0 - 1.0).abs();
        let ry = if p.y > 0.0 && p.x.abs() < 0.5 {
            0.34 + 0.08 * tooth
        } else {
            0.34
        };
        (p.x / 0.62).powi(2) + (p.y / ry).powi(2) < 1.0
    };
    let mut claw = false;
    let mut limb = false;
    for s in [-1.0f32, 1.0] {
        limb |= segment(q, vec2(s * 0.5, -0.05), vec2(s * 0.8, 0.3)) < 0.075;
        let c = vec2(s * 0.82, 0.42);
        let r = q - c;
        // the pincers open upward
        let notch = r.y > -0.02 && (r.x + s * 0.03).abs() < f32::midpoint(r.y, 0.02);
        claw |= r.length() < 0.18 && !notch;
        for k in 0..3 {
            let k = k as f32;
            limb |= segment(
                q,
                vec2(s * (0.22 + 0.13 * k), -0.28),
                vec2(s * (0.42 + 0.17 * k), -0.55 + 0.05 * k),
            ) < 0.045;
        }
    }
    if !(body || claw || limb) {
        return None;
    }
    let mut color = orange.lerp(shade, smoothstep(0.0, -0.4, q.y));
    if body {
        for s in [-1.0f32, 1.0] {
            let eye = q - vec2(s * 0.17, 0.1);
            if eye.length() < 0.115 {
                color = Vec3::ONE;
            }
            if (eye - vec2(-s * 0.025, 0.01)).length() < 0.065 {
                color = srgb(0x111111);
            }
        }
        let smile = q - vec2(0.0, 0.0);
        if (smile.length() - 0.14).abs() < 0.024 && smile.y < -0.06 {
            color = srgb(0x2a0a00);
        }
    }
    Some(color)
}

/// The lid of the laptop: worn dark aluminium with three stickers. Ferris the crab, the
/// fframes logo and "works on my machine". The corners peel and the print is faded.
fn lid_textures(bold: &[u8], serif: &[u8]) -> (Arc<Texture>, Arc<Texture>) {
    let size = vec2(LAPTOP.x, LAPTOP.y) - Vec2::splat(0.012);
    let ferris_center = vec2(0.1, 0.085);
    let ferris_half = 0.068;
    let ferris_angle: f32 = deg(-7.0);
    let logo = Sticker {
        center: vec2(0.305, 0.078),
        half: vec2(0.06, 0.026),
        angle: deg(6.0),
        radius: 0.007,
        peel: Some((vec2(-1.0, -1.0), 0.007)),
    };
    let logo_ink = Ink::new(serif, "fframes", 0.044, Vec2::ZERO);
    let works = Sticker {
        center: vec2(0.255, 0.198),
        half: vec2(0.122, 0.042),
        angle: deg(-3.0),
        radius: 0.006,
        peel: Some((vec2(1.0, 1.0), 0.011)),
    };
    let works_top = Ink::new(bold, "WORKS ON", 0.026, vec2(0.026, -0.0135));
    let works_bottom = Ink::new(bold, "MY MACHINE", 0.026, vec2(0.026, 0.0165));
    let metal = |p: Vec2| {
        let grime = 0.6 * value(p * 30.0, 3) + 0.4 * value(p * 140.0, 4);
        let scratch = scratches(p, 0.02, 0.0005, 7) * 0.6 + scratches(p, 0.05, 0.0004, 9) * 0.4;
        // the edges are polished by hands
        let edge = 1.0 - smoothstep(0.0, 0.012, p.x.min(p.y).min(size.x - p.x).min(size.y - p.y));
        Paint {
            color: Vec3::splat(0.36 - 0.06 * grime + 0.12 * scratch + 0.08 * edge),
            roughness: 0.55 + 0.15 * grime - 0.2 * scratch,
            metalness: 1.0,
        }
    };
    let vinyl = |color: Srgb, p: Vec2| {
        // faded by light, with scuffs where the bag rubs
        let scuff =
            smoothstep(0.55, 0.8, value(p * 90.0, 21)) * 0.5 + scratches(p, 0.015, 0.0004, 31);
        Paint {
            color: color.lerp(Vec3::splat(0.85), 0.08 + 0.25 * scuff.min(1.0)),
            roughness: 0.3 + 0.3 * scuff.min(1.0),
            metalness: 0.0,
        }
    };
    let (color, data) = bake(1024, 690, size, |p| {
        let mut paint = metal(p);
        let stick =
            |paint: &mut Paint, sticker: &Sticker, print: &dyn Fn(Vec2) -> Srgb| match sticker
                .layer(p)
            {
                Some(Layer::Face(q)) => *paint = vinyl(print(q), p),
                Some(Layer::Back) => {
                    *paint = Paint {
                        color: srgb(0xe8e4da),
                        roughness: 0.85,
                        metalness: 0.0,
                    }
                }
                Some(Layer::Glue) => {
                    paint.color = paint.color * 0.8 + Vec3::splat(0.06);
                    paint.roughness = 0.8;
                }
                Some(Layer::Shadow(s)) => paint.color *= 1.0 - s,
                None => {}
            };
        // fframes: the mark of the logo in the colors of the brand
        stick(&mut paint, &logo, &|q| match logo_ink.letter(q) {
            Some(0) => srgb(0x8a4320),
            Some(1) => srgb(0xdc5e22),
            Some(_) => Vec3::ONE,
            None => srgb(0x0d0d0d),
        });
        // Ferris: a white border around the crab, cut to its shape
        let q = {
            let d = p - ferris_center;
            let (s, c) = (-ferris_angle).sin_cos();
            vec2(d.x * c - d.y * s, -(d.x * s + d.y * c)) / ferris_half
        };
        if q.abs().max_element() < 1.15 {
            let crab = ferris(q);
            let border = crab.is_some()
                || (0..12).any(|i| {
                    let a = i as f32 * std::f32::consts::TAU / 12.0;
                    ferris(q + vec2(a.cos(), a.sin()) * 0.09).is_some()
                });
            if let Some(color) = crab {
                paint = vinyl(color, p);
            } else if border {
                paint = vinyl(srgb(0xf2f0ea), p);
            }
        }
        // works on my machine: a seal with a check mark and two lines of text
        stick(&mut paint, &works, &|q| {
            let seal = q - vec2(-0.088, 0.0);
            let check = segment(seal, vec2(-0.013, 0.0), vec2(-0.004, 0.01)).min(segment(
                seal,
                vec2(-0.004, 0.01),
                vec2(0.015, -0.011),
            ));
            if check < 0.0035 && seal.length() < 0.026 {
                Vec3::ONE
            } else if seal.length() < 0.026 {
                srgb(0x2e9e44)
            } else if works.edge(q) > -0.003 {
                srgb(0x1f5fbf)
            } else if works_top.letter(q).is_some() || works_bottom.letter(q).is_some() {
                srgb(0x111111)
            } else {
                srgb(0xf7f5ef)
            }
        });
        paint
    });
    (Arc::new(color), Arc::new(data))
}

/// The face of the safety sign: white enamel with a red border, the sign that forbids a
/// rocket, "NO DEPLOYS ON FRIDAY", and the order of the site manager. It is old: the red is
/// faded, rust runs down from the bolts, the edges are chipped and dirt collects at the
/// bottom.
fn sign_texture(bold: &[u8]) -> Arc<Texture> {
    let size = SIGN_SIZE;
    let red = srgb(0xc4161c);
    let symbol = vec2(0.3, 0.44);
    let radius = 0.24;
    let top = Ink::new(bold, "NO DEPLOYS", 0.114, vec2(1.065, 0.35));
    let bottom = Ink::new(bold, "ON FRIDAY", 0.114, vec2(1.065, 0.55));
    let order = Ink::new(bold, "BY ORDER OF THE SITE MANAGER", 0.042, vec2(0.8, 0.88));
    let bolts = [(0.05f32, 0.05f32), (1.55, 0.05), (0.05, 0.95), (1.55, 0.95)].map(Vec2::from);
    let (color, _) = bake(1024, 640, size, |p| {
        // the rocket in the symbol, in its own frame: Y along the rocket, R = 1
        let rocket = {
            let r = (p - symbol) / radius;
            let (s, c) = deg(45.0).sin_cos();
            let r = vec2(r.x * c - r.y * s, -(r.x * s + r.y * c));
            let body = r.x.abs() < 0.16 && (-0.42..0.3).contains(&r.y);
            let nose = (0.3..0.66).contains(&r.y) && r.x.abs() < 0.16 * (0.66 - r.y) / 0.36;
            let fins =
                (-0.55..-0.12).contains(&r.y) && r.x.abs() < 0.16 + 0.16 * (-0.12 - r.y) / 0.43;
            let window = (r - vec2(0.0, 0.12)).length() < 0.07;
            (body || nose || fins) && !window
        };
        let r = (p - symbol) / radius;
        let ring = (0.8..1.0).contains(&r.length());
        // the bar from the top left to the bottom right
        let bar = (r.x - r.y).abs() < 0.13 && r.length() < 0.9;
        let border = {
            let d = p.min(size - p).min_element();
            (0.025..0.045).contains(&d)
        };
        let text_red = bottom.letter(p).is_some();
        let text_black = top.letter(p).is_some() || order.letter(p).is_some() || (rocket && !bar);
        let line = (p.y - 0.8).abs() < 0.003 && (0.1..1.5).contains(&p.x);
        let fade = 0.12 + 0.12 * value(p * 4.0, 51);
        let mut color = if ring || bar || border || text_red {
            red.lerp(srgb(0xe8b0a8), fade)
        } else if text_black || line {
            srgb(0x141414)
        } else {
            srgb(0xe4e0d4)
        };
        // grime: more at the bottom and in blotches
        let blotch = value(p * 9.0, 52) * 0.6 + value(p * 37.0, 53) * 0.4;
        let grime = (0.08 + 0.25 * smoothstep(0.5, 1.0, p.y / size.y)) * blotch;
        color = color.lerp(srgb(0x5a5246), grime);
        // rust runs down from the bolts
        for b in bolts {
            let d = p - b;
            let streak = (1.0 - smoothstep(0.0, 0.012 + 0.02 * d.y.max(0.0), d.x.abs()))
                * (1.0 - smoothstep(0.0, 0.25 + 0.2 * hash(b.x as u32), d.y))
                * if d.y > -0.01 { 1.0 } else { 0.0 };
            color = color.lerp(
                srgb(0x7a4a26),
                0.55 * streak * (0.6 + 0.4 * value(p * 60.0, 54)),
            );
            if d.length() < 0.016 {
                color = srgb(0x3a2a20);
            }
        }
        // chips in the enamel near the edges show the dark steel
        let near_edge = 1.0 - smoothstep(0.0, 0.06, p.min(size - p).min_element());
        if value(p * 50.0, 55) * near_edge > 0.55 {
            color = srgb(0x2d2a28);
        }
        color *= 1.0 - 0.15 * scratches(p, 0.06, 0.0015, 56);
        Paint {
            color,
            roughness: 0.5,
            metalness: 0.0,
        }
    });
    Arc::new(color)
}
