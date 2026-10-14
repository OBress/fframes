//! Draws actors with the 3D toon pipeline of `fframes-anime`: the meshes of every actor in a
//! group share one depth buffer, the cel shaded pixels become an image and the line art is
//! vector paths on top.

use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use fframes::Svgr;
use fframes::usvgr::PreloadedImageData;
use fframes_anime::camera::Camera;
use fframes_anime::figure::{self, Cast, Detail, Figure, MATERIALS, Palette};
use fframes_anime::glam::{Affine3A, Vec3};
use fframes_anime::ink::{self, InkStyle};
use fframes_anime::prop::{Prop, PropLight};
use fframes_anime::raster::{self, Raster};
use fframes_anime::rig::Bone;
use fframes_anime::toon::Material;

use crate::actor::{Actor, color_of};
use crate::cast::{Member, Who, member};
use crate::palette::Paint;
use crate::{H, W};

static FIGURES: OnceLock<Mutex<HashMap<Who, Arc<Figure>>>> = OnceLock::new();

fn detail() -> Detail {
    Detail(
        std::env::var("ANIME_OP_DETAIL")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(1.15),
    )
}

/// The meshes of a cast member, built once.
pub fn figure(who: Who) -> Arc<Figure> {
    let cache = FIGURES.get_or_init(Default::default);
    if let Some(f) = cache.lock().unwrap_or_else(|e| e.into_inner()).get(&who) {
        return Arc::clone(f);
    }
    let m = member(who);
    let f = Arc::new(Figure::new(&m.design, &m.skeleton(), detail()));
    cache
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .entry(who)
        .or_insert(f)
        .clone()
}

/// Builds the figures of the whole cast in parallel.
pub fn prewarm(list: &[Who]) {
    std::thread::scope(|s| {
        for who in list {
            s.spawn(move || figure(*who));
        }
    });
}

pub fn rgb(hex: &str) -> Vec3 {
    let h = hex.trim_start_matches('#');
    let v = |i: usize| {
        u8::from_str_radix(h.get(i..i + 2).unwrap_or("00"), 16).unwrap_or(0) as f32 / 255.0
    };
    Vec3::new(v(0), v(2), v(4))
}

const ALL_MATERIALS: [Material; MATERIALS] = [
    Material::Suit,
    Material::Robe,
    Material::RobeLining,
    Material::Shirt,
    Material::Tie,
    Material::Skin,
    Material::Hair,
    Material::Beard,
    Material::Scarf,
    Material::Ink,
    Material::Paper,
    Material::Accent,
];

fn is_silhouette(paint: &Paint) -> bool {
    paint.suit == paint.paper && paint.line == paint.suit && paint.ink == paint.suit
}

pub fn palette(paint: &Paint, m: &Member, cam: &Camera) -> Palette {
    let mut fill = [Vec3::ZERO; MATERIALS];
    for mat in ALL_MATERIALS {
        let c = color_of(mat, m, paint);
        fill[figure::material_index(mat)] = rgb(c);
    }
    let (r, u, f) = cam.basis();
    let silhouette = is_silhouette(paint);
    Palette {
        fill,
        line: rgb(paint.line),
        shade: if silhouette { 0.0 } else { paint.shade },
        terminator: paint.terminator,
        light: (-r * 0.5 + u * 0.8 - f * 0.45).normalize(),
        details: !silhouette,
    }
}

static IMAGES: AtomicU64 = AtomicU64::new(0);

/// Draws the actors (each with its paint) in one pass: correct overlaps between them.
pub fn draw<'a>(list: &[(&Actor, Paint)], t: f32, cam: &Camera) -> Svgr<'a> {
    try_draw(list, t, cam).unwrap_or_else(Svgr::empty)
}

/// A prop placed in a shot for one frame.
#[derive(Clone, Copy)]
pub struct Placed<'p> {
    pub prop: &'p Prop,
    /// Scale, rotation and position.
    pub at: Affine3A,
    /// A flat colour instead of the prop's own (silhouette shots).
    pub flat: Option<&'static str>,
    pub line: &'static str,
    /// Lines boil with the drawing: the time of the drawing on screen.
    pub drawing: f32,
}

impl<'p> Placed<'p> {
    pub fn new(prop: &'p Prop, at: Affine3A, drawing: f32) -> Self {
        Placed {
            prop,
            at,
            flat: None,
            line: crate::palette::INK,
            drawing,
        }
    }

    pub fn flat(mut self, color: &'static str) -> Self {
        self.flat = Some(color);
        self
    }
}

/// [`draw`], `None` when nothing of the actors is in the frame.
pub fn try_draw<'a>(list: &[(&Actor, Paint)], t: f32, cam: &Camera) -> Option<Svgr<'a>> {
    try_draw_with(list, &[], t, cam)
}

/// Draws actors and props together: one depth buffer, so a hand closes around a bar and a cream
/// puff sits in a palm.
pub fn draw_with<'a>(list: &[(&Actor, Paint)], props: &[Placed], t: f32, cam: &Camera) -> Svgr<'a> {
    try_draw_with(list, props, t, cam).unwrap_or_else(Svgr::empty)
}

/// [`draw_with`], `None` when nothing is in the frame.
pub fn try_draw_with<'a>(
    list: &[(&Actor, Paint)],
    props: &[Placed],
    t: f32,
    cam: &Camera,
) -> Option<Svgr<'a>> {
    try_draw_region(list, props, t, cam, None)
}

/// Draws actors only inside the box `(x, y, w, h)` of the frame: a close-up seen through a panel
/// rasterizes the panel, not the whole head around it.
pub fn draw_in<'a>(
    list: &[(&Actor, Paint)],
    t: f32,
    cam: &Camera,
    region: (f32, f32, f32, f32),
) -> Svgr<'a> {
    try_draw_region(list, &[], t, cam, Some(region)).unwrap_or_else(Svgr::empty)
}

fn try_draw_region<'a>(
    list: &[(&Actor, Paint)],
    props: &[Placed],
    t: f32,
    cam: &Camera,
    region: Option<(f32, f32, f32, f32)>,
) -> Option<Svgr<'a>> {
    if list.is_empty() && props.is_empty() {
        return None;
    }
    let proj = cam.projector();
    let mut surfaces = Vec::new();
    let mut palettes = Vec::with_capacity(list.len());
    let mut looks = Vec::with_capacity(list.len());
    let mut views = Vec::with_capacity(list.len());
    for (i, (a, paint)) in list.iter().enumerate() {
        let frame = a.frame(t);
        let head = frame.posed.pos(Bone::Head);
        views.push(frame.posed.rot(Bone::Head).inverse() * (cam.eye - head).normalize_or(Vec3::Z));
        surfaces.extend(a.figure.surfaces(
            &frame.posed,
            &frame.sim,
            a.layout(),
            &frame.look,
            (i as u32) << 8,
        ));
        palettes.push(palette(paint, &a.member, cam));
        looks.push(frame.look);
    }
    let n = list.len();
    for (j, p) in props.iter().enumerate() {
        surfaces.push(p.prop.surface(p.at, ((n + j) as u32) << 8));
    }
    let (rr, uu, ff) = cam.basis();
    let lights: Vec<PropLight> = props
        .iter()
        .map(|p| PropLight {
            light: (-rr * 0.5 + uu * 0.8 - ff * 0.45).normalize(),
            shade: 0.14,
            terminator: -0.2,
            flat: p.flat.map(rgb),
        })
        .collect();
    let (mut x0, mut y0, mut w, mut h) = raster::bounds(&surfaces, &proj, W, H, 16.0)?;
    if let Some((rx, ry, rw, rh)) = region {
        let x1 = (x0 + w as i32).min((rx + rw).ceil() as i32);
        let y1 = (y0 + h as i32).min((ry + rh).ceil() as i32);
        x0 = x0.max(rx.floor() as i32);
        y0 = y0.max(ry.floor() as i32);
        if x1 <= x0 || y1 <= y0 {
            return None;
        }
        (w, h) = ((x1 - x0) as usize, (y1 - y0) as usize);
    }
    let r = Raster::new(x0, y0, w, h, &surfaces, &proj);
    let cast: Vec<Cast> = list
        .iter()
        .zip(&palettes)
        .zip(&looks)
        .zip(&views)
        .map(|((((a, _), p), l), v)| Cast {
            figure: &a.figure,
            look: *l,
            palette: p,
            view: *v,
        })
        .collect();
    let pixels = r.resolve(&surfaces, &proj, &|f| {
        let i = (f.tag >> 8) as usize;
        match i.checked_sub(n) {
            Some(j) => props[j].prop.shade(f, &lights[j]),
            None => figure::shade(&cast, f),
        }
    });
    let image = Arc::new(PreloadedImageData {
        data: Cow::Owned(pixels),
        width: w as u32,
        height: h as u32,
        id: format!("anime-op-cel-{}", IMAGES.fetch_add(1, Ordering::Relaxed)),
    });
    let mut lines: Vec<Svgr<'a>> = Vec::new();
    for (i, (a, paint)) in list.iter().enumerate() {
        if is_silhouette(paint) {
            continue;
        }
        let style = InkStyle {
            width: 0.0034 * a.line_weight,
            // a new drawing every `koma` frames: the lines boil with it
            seed: (a.held(t) * 24.0).round() as u32 + i as u32 * 7919,
            ..Default::default()
        };
        let inked = ink::draw(&surfaces, &r, &proj, &style, &|s| {
            (s.tag >> 8) as usize == i
        });
        let debug = std::env::var("ANIME_OP_INK_DEBUG").is_ok();
        if !inked.lines.is_empty() {
            let color = if debug { "#0000FF" } else { paint.line };
            lines.push(fframes::svgr!(<path d={inked.lines} fill={color} />));
        }
        if !inked.strands.is_empty() {
            // strands on dark hair are drawn light, like the yellow lines in black hair; on
            // coloured hair in a darker tone of it (an ink scribble reads as dirt there); on
            // white hair in ink
            let hair = palettes[i].color(Material::Hair);
            let luma = hair.dot(Vec3::new(0.3, 0.59, 0.11));
            let dark = luma < 0.25;
            let color = if debug {
                "#FF0000".to_owned()
            } else if dark {
                paint.paper.to_owned()
            } else if luma < 0.85 && hair != palettes[i].line {
                let c = (hair * 0.5 * 255.0).round();
                format!("#{:02X}{:02X}{:02X}", c.x as u8, c.y as u8, c.z as u8)
            } else {
                paint.line.to_owned()
            };
            let opacity = if dark { 0.85 } else { 1.0 };
            lines.push(fframes::svgr!(<path d={inked.strands} fill={color} opacity={opacity} />));
        }
    }
    for (j, p) in props.iter().enumerate() {
        if p.flat.is_some() {
            continue;
        }
        let style = InkStyle {
            width: 0.0034,
            seed: (p.drawing * 24.0).round() as u32 + (n + j) as u32 * 7919,
            ..Default::default()
        };
        let inked = ink::draw(&surfaces, &r, &proj, &style, &|s| {
            (s.tag >> 8) as usize == n + j
        });
        if !inked.lines.is_empty() {
            lines.push(fframes::svgr!(<path d={inked.lines} fill={p.line} />));
        }
    }
    Some(fframes::svgr!(
        <g>
            <image href={image} x={x0} y={y0} width={w} height={h} />
            {lines}
        </g>
    ))
}
