//! The development harness of shot 8 ("The why"). It compiles the montage next to copies
//! of the shared modules and writes review frames to `/tmp/the-why-montage`.
//!
//! - `MONTAGE_T=0,0.5,1.5` renders these times (default: the review times).
//! - `MONTAGE_SHEET=0:5:12` renders 12 frames from 0 to 5 s into one contact sheet.
#![allow(clippy::unreadable_literal)]

pub const WIDTH: usize = 3840;
pub const HEIGHT: usize = 2160;
pub const FPS: usize = 60;
#[path = "../src/assets.rs"]
pub mod assets;
#[path = "../src/look.rs"]
pub mod look;
#[path = "../src/montage/mod.rs"]
pub mod montage;

use std::path::{Path, PathBuf};
use std::time::Instant;

use image::RgbaImage;

const REVIEW: [f32; 13] = [
    0.0, 0.5, 1.5, 3.0, 4.5, 5.5, 7.0, 9.0, 10.5, 12.0, 13.5, 14.5, 15.0,
];

fn out_dir() -> PathBuf {
    let dir = PathBuf::from("/tmp/the-why-montage");
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn load() -> montage::MontageSet {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("dynamic_media");
    let assets = assets::Assets::load(&dir).unwrap();
    let start = Instant::now();
    let set = montage::MontageSet::new(&assets);
    println!("new: {:.2} s", start.elapsed().as_secs_f32());
    set
}

/// Renders the picture at `t`, the last frame at 15.0 s with its overlay as a flat color.
fn picture(set: &montage::MontageSet, renderer: &fframes_3d::Renderer, t: f32) -> RgbaImage {
    let t_in = t.min(montage::DURATION - 0.001);
    let start = Instant::now();
    let pixels = look::picture_pixels(renderer, set.shutter(t_in), t_in, |t| set.moment(t));
    println!(
        "t {t:5.2}: {:6.1} ms ({} samples)",
        start.elapsed().as_secs_f32() * 1000.0,
        set.shutter(t_in)
    );
    let mut image =
        RgbaImage::from_raw(pixels.width, pixels.height, pixels.to_straight_rgba()).unwrap();
    let white = montage::final_white(t_in);
    if white > 0.0 {
        for p in image.pixels_mut() {
            let over: [u8; 3] = [0xff, 0xf3, 0xdc];
            for (c, o) in p.0.iter_mut().zip(over) {
                *c = (f32::from(*c) * (1.0 - white) + f32::from(o) * white).round() as u8;
            }
        }
    }
    image
}

fn times() -> Vec<f32> {
    std::env::var("MONTAGE_T").map_or(REVIEW.to_vec(), |list| {
        list.split(',').map(|s| s.trim().parse().unwrap()).collect()
    })
}

#[test]
#[ignore = "renders review frames"]
fn review() {
    let set = load();
    let renderer = montage::MontageSet::renderer(1920, 804);
    for t in times() {
        let image = picture(&set, &renderer, t);
        image
            .save(out_dir().join(format!("t{t:05.2}.png")))
            .unwrap();
    }
}

#[test]
#[ignore = "renders a contact sheet"]
fn sheet() {
    let spec = std::env::var("MONTAGE_SHEET").unwrap_or_else(|_| "0:15:24".to_owned());
    let parts: Vec<f32> = spec.split(':').map(|s| s.parse().unwrap()).collect();
    let (from, to, count) = (parts[0], parts[1], parts[2] as usize);
    let set = load();
    let renderer = montage::MontageSet::renderer(960, 402);
    let columns = 4;
    let rows = count.div_ceil(columns);
    let mut sheet = RgbaImage::new(960 * columns as u32, 402 * rows as u32);
    for i in 0..count {
        let t = from + (to - from) * i as f32 / (count - 1).max(1) as f32;
        let image = picture(&set, &renderer, t);
        image::imageops::replace(
            &mut sheet,
            &image,
            (i % columns) as i64 * 960,
            (i / columns) as i64 * 402,
        );
    }
    sheet
        .save(out_dir().join(format!("sheet_{from:.2}_{to:.2}.png")))
        .unwrap();
}

#[test]
#[ignore = "measures the GPU frame time"]
fn timing() {
    let set = load();
    let renderer = montage::MontageSet::renderer(1920, 804);
    assert!(renderer.uses_gpu(), "no GPU");
    // warm up the caches of the GPU renderer
    for t in [0.5, 7.0, 12.0] {
        picture(&set, &renderer, t);
    }
    let mut worst: f32 = 0.0;
    let mut sum = 0.0;
    let mut n = 0;
    let mut moment_worst: f32 = 0.0;
    let step: f32 = std::env::var("MONTAGE_STEP").map_or(0.25, |s| s.parse().unwrap());
    for i in 0..(15.0 / step) as usize {
        let t = i as f32 * step;
        let m = Instant::now();
        let _ = set.moment(t);
        moment_worst = moment_worst.max(m.elapsed().as_secs_f32() * 1000.0);
        let start = Instant::now();
        let _ = look::picture_pixels(&renderer, set.shutter(t), t, |t| set.moment(t));
        let ms = start.elapsed().as_secs_f32() * 1000.0;
        println!("t {t:5.2}: {ms:6.1} ms ({} samples)", set.shutter(t));
        worst = worst.max(ms);
        sum += ms;
        n += 1;
    }
    println!(
        "frame: mean {:.1} ms, worst {:.1} ms; moment worst {:.1} ms",
        sum / n as f32,
        worst,
        moment_worst
    );
}

/// Renders the scene at `MONTAGE_T` from the camera in `MONTAGE_CAM`: position, target and
/// vertical field of view, `px,py,pz,tx,ty,tz,fov`.
#[test]
#[ignore = "renders a debug view"]
fn debug_view() {
    let set = load();
    let renderer = montage::MontageSet::renderer(1920, 804);
    let v: Vec<f32> = std::env::var("MONTAGE_CAM")
        .unwrap()
        .split(',')
        .map(|s| s.trim().parse().unwrap())
        .collect();
    for t in times() {
        let (scene, camera) = set.moment(t);
        let camera = camera
            .position(fframes_3d::vec3(v[0], v[1], v[2]))
            .look_at(fframes_3d::vec3(v[3], v[4], v[5]))
            .fov(v[6]);
        let camera = fframes_3d::Camera {
            focus: None,
            near: 0.1,
            ..camera
        };
        let pixels = renderer.render_pixels(&scene, &camera);
        let image =
            RgbaImage::from_raw(pixels.width, pixels.height, pixels.to_straight_rgba()).unwrap();
        image
            .save(out_dir().join(format!("debug_t{t:05.2}.png")))
            .unwrap();
    }
}

/// Places the landmarks for shot 9 in a plain sunrise desert, seen from a plateau.
#[test]
#[ignore = "renders the landmarks"]
fn landmarks() {
    use fframes_3d::*;
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("dynamic_media");
    let assets = assets::Assets::load(&dir).unwrap();
    let set = montage::MontageSet::new(&assets);
    let marks = set.landmarks();
    let mut scene = Scene::new()
        .environment(Environment::map(assets.sunrise_sky.clone(), 1.0).rotated(1.68))
        .background(Background::Environment { blur: 0.0 });
    scene.add(Node::mesh(
        Geometry::floor(20000.0, 20000.0),
        assets.sand.material(0xf0d2b0, 1.0, 0.0),
    ));
    scene.add(Node::light(Light::sun(
        0xffdcb0,
        3.0,
        vec3(0.5, 0.12, -0.84),
    )));
    for (scene_part, position, turn) in [
        (&marks.bridge, vec3(-900.0, 0.0, -2600.0), 0.2f32),
        (&marks.greenhouse, vec3(420.0, 0.0, -1100.0), 0.0),
        (&marks.contrail, vec3(900.0, 0.0, -4200.0), 0.0),
    ] {
        let anchor = scene.add(
            Node::group()
                .with_position(position)
                .with_rotation(Quat::from_rotation_y(turn)),
        );
        scene.append(Some(anchor), scene_part);
    }
    scene.atmosphere = Some(Atmosphere {
        density: 0.00015,
        falloff: 0.002,
        distance: 6000.0,
        ..Default::default()
    });
    let camera = look::lens(35.0)
        .position(vec3(0.0, 60.0, 0.0))
        .look_at(vec3(0.0, 300.0, -3000.0))
        .clip(1.0, 20000.0);
    let renderer = montage::MontageSet::renderer(1920, 804);
    let pixels = renderer.render_pixels(&scene, &camera);
    RgbaImage::from_raw(pixels.width, pixels.height, pixels.to_straight_rgba())
        .unwrap()
        .save(out_dir().join("landmarks.png"))
        .unwrap();
}
