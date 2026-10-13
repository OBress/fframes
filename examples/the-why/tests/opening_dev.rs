//! The development harness of shot 1 ("The cursor"). It compiles the opening next to copies
//! of the shared modules and writes review frames to `/tmp/the-why-opening`.
//!
//! - `OPENING_T=0,0.5,1.5` renders these times (default: the review times).
//! - `OPENING_SHEET=0:7:10` renders 10 frames from 0 to 7 s into one contact sheet.
//! - `OPENING_ONION=4.3:5.7:8` blends 8 frames from 4.3 to 5.7 s into one image.
#![allow(clippy::unreadable_literal)]

pub const WIDTH: usize = 3840;
pub const HEIGHT: usize = 2160;
pub const FPS: usize = 60;
#[path = "../src/assets.rs"]
pub mod assets;
#[path = "../src/look.rs"]
pub mod look;
#[path = "../src/opening/mod.rs"]
pub mod opening;

use std::path::{Path, PathBuf};
use std::time::Instant;

use image::RgbaImage;

const REVIEW: [f32; 12] = [0.0, 0.7, 1.4, 2.3, 3.3, 4.0, 4.7, 5.3, 5.8, 6.4, 6.8, 6.98];

fn out_dir() -> PathBuf {
    let dir = PathBuf::from("/tmp/the-why-opening");
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn load() -> opening::OpeningSet {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("dynamic_media");
    let assets = assets::Assets::load(&dir).unwrap();
    let start = Instant::now();
    let set = opening::OpeningSet::new(&assets);
    println!("new: {:.2} s", start.elapsed().as_secs_f32());
    set
}

/// Renders the picture at `t`.
fn picture(set: &opening::OpeningSet, renderer: &fframes_3d::Renderer, t: f32) -> RgbaImage {
    let start = Instant::now();
    let pixels = look::picture_pixels(renderer, set.shutter(t), t, |t| set.moment(t));
    println!(
        "t {t:5.2}: {:6.1} ms ({} samples)",
        start.elapsed().as_secs_f32() * 1000.0,
        set.shutter(t)
    );
    RgbaImage::from_raw(pixels.width, pixels.height, pixels.to_straight_rgba()).unwrap()
}

fn times() -> Vec<f32> {
    std::env::var("OPENING_T").map_or(REVIEW.to_vec(), |list| {
        list.split(',').map(|s| s.trim().parse().unwrap()).collect()
    })
}

/// `from:to:count` from the variable `name`.
fn span(name: &str, default: &str) -> (f32, f32, usize) {
    let spec = std::env::var(name).unwrap_or_else(|_| default.to_owned());
    let parts: Vec<f32> = spec.split(':').map(|s| s.parse().unwrap()).collect();
    (parts[0], parts[1], parts[2] as usize)
}

#[test]
#[ignore = "renders review frames"]
fn review() {
    let set = load();
    let renderer = opening::OpeningSet::renderer(1920, 804);
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
    let (from, to, count) = span("OPENING_SHEET", "0:6.98:10");
    let set = load();
    let renderer = opening::OpeningSet::renderer(960, 402);
    let columns = 5;
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
#[ignore = "renders an onion skin"]
fn onion() {
    let (from, to, count) = span("OPENING_ONION", "4.3:5.7:8");
    let set = load();
    let renderer = opening::OpeningSet::renderer(960, 402);
    let mut sum = vec![0.0f32; 960 * 402 * 4];
    for i in 0..count {
        let t = from + (to - from) * i as f32 / (count - 1).max(1) as f32;
        let image = picture(&set, &renderer, t);
        for (s, p) in sum.iter_mut().zip(image.as_raw()) {
            *s += f32::from(*p) / count as f32;
        }
    }
    let raw = sum.iter().map(|v| v.round() as u8).collect();
    RgbaImage::from_raw(960, 402, raw)
        .unwrap()
        .save(out_dir().join(format!("onion_{from:.2}_{to:.2}.png")))
        .unwrap();
}

#[test]
#[ignore = "measures the GPU frame time"]
fn timing() {
    let set = load();
    let renderer = opening::OpeningSet::renderer(1920, 804);
    assert!(renderer.uses_gpu(), "no GPU");
    // warm up the caches of the GPU renderer
    for t in [0.5, 5.0, 6.5] {
        picture(&set, &renderer, t);
    }
    let (mut worst, mut sum, mut n, mut moment_worst) = (0.0f32, 0.0, 0, 0.0f32);
    let step: f32 = std::env::var("OPENING_STEP").map_or(0.1, |s| s.parse().unwrap());
    for i in 0..(7.0 / step) as usize {
        let t = i as f32 * step;
        let m = Instant::now();
        let _ = set.moment(t);
        moment_worst = moment_worst.max(m.elapsed().as_secs_f32() * 1000.0);
        // the faster of two runs: other renders may share the GPU
        let ms = (0..2)
            .map(|_| {
                let start = Instant::now();
                let _ = look::picture_pixels(&renderer, set.shutter(t), t, |t| set.moment(t));
                start.elapsed().as_secs_f32() * 1000.0
            })
            .fold(f32::MAX, f32::min);
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

#[test]
#[ignore = "prints the camera path"]
fn path() {
    let set = load();
    for i in 0..=420 {
        let t = i as f32 / 60.0;
        let (_, camera) = set.moment(t);
        let distance = camera.position.distance(camera.target);
        let d = camera.target - camera.position;
        let azimuth = (-d.x).atan2(-d.z).to_degrees();
        let elevation = (-d.y / distance).asin().to_degrees();
        println!(
            "{t:.4} {:.5} {azimuth:.3} {elevation:.3} {:.3}",
            distance.ln(),
            camera.fov
        );
    }
}
