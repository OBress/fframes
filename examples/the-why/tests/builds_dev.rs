//! The development harness of the quick builds of shot 8 and of its timeline. It compiles
//! the montage next to copies of the shared modules and writes review frames to
//! `/home/neogoose/dev/the-why-review/montage/builds`.
//!
//! - `BUILD=wind|school|water` renders that part at `BUILD_T` (seconds into the part).
//!   `BUILD=shot` renders the whole shot at `BUILD_T` (seconds into the shot).
//! - `BUILD_T=0,0.5,1` or `BUILD_T=0:1.25:8` (from, to, count).
//! - `BUILD_CPU=1` renders every second frame with the CPU rasterizer.
//! - `BUILD_SCALE=0.5` renders at half the review size.
//! - The frames render in parallel threads. A contact sheet of all frames goes to
//!   `sheet_<BUILD>.png`.
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

use fframes_3d::{Camera, Renderer, Scene};
use image::RgbaImage;

fn out_dir() -> PathBuf {
    let dir = PathBuf::from("/home/neogoose/dev/the-why-review/montage/builds");
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn times() -> Vec<f32> {
    let spec = std::env::var("BUILD_T").unwrap_or_else(|_| "0:1.25:6".to_owned());
    if spec.contains(':') {
        let p: Vec<f32> = spec.split(':').map(|s| s.parse().unwrap()).collect();
        let n = p[2] as usize;
        (0..n)
            .map(|i| p[0] + (p[1] - p[0]) * i as f32 / (n - 1).max(1) as f32)
            .collect()
    } else {
        spec.split(',').map(|s| s.trim().parse().unwrap()).collect()
    }
}

/// The picture at `t` as an image, with the warm white of the end of the shot.
fn picture(
    renderer: &Renderer,
    samples: usize,
    t: f32,
    white: f32,
    moment: impl Fn(f32) -> (Scene, Camera),
) -> RgbaImage {
    let pixels = look::picture_pixels(renderer, samples, t, moment);
    let mut image =
        RgbaImage::from_raw(pixels.width, pixels.height, pixels.to_straight_rgba()).unwrap();
    if white > 0.0 {
        for p in image.pixels_mut() {
            for (c, o) in p.0.iter_mut().zip([0xffu8, 0xf3, 0xdc]) {
                *c = (f32::from(*c) * (1.0 - white) + f32::from(o) * white).round() as u8;
            }
        }
    }
    image
}

#[test]
#[ignore = "renders review frames"]
fn review() {
    let part = std::env::var("BUILD").unwrap_or_else(|_| "wind".to_owned());
    let scale: f32 = std::env::var("BUILD_SCALE").map_or(1.0, |s| s.parse().unwrap());
    let cpu = std::env::var("BUILD_CPU").is_ok();
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("dynamic_media");
    let assets = assets::Assets::load(&dir).unwrap();
    let start = Instant::now();
    let (width, height) = ((1920.0 * scale) as u32 & !1, (804.0 * scale) as u32 & !1);
    let renderer = montage::MontageSet::renderer(width, height);
    let set: Box<dyn Fn(f32) -> (Scene, Camera) + Sync> = match part.as_str() {
        "wind" => {
            let p = montage::wind::Wind::new(&assets);
            Box::new(move |t| p.moment(t))
        }
        "school" => {
            let p = montage::school::School::new(&assets);
            Box::new(move |t| p.moment(t))
        }
        "water" => {
            let p = montage::water::Water::new(&assets, montage::school::heading());
            Box::new(move |t| p.moment(t))
        }
        _ => {
            let p = montage::MontageSet::new(&assets);
            Box::new(move |t| p.moment(t))
        }
    };
    let wind = montage::wind::Wind::new(&assets);
    let school = montage::school::School::new(&assets);
    let water = montage::water::Water::new(&assets, montage::school::heading());
    let shot = (part == "shot").then(|| montage::MontageSet::new(&assets));
    let shutter = |t: f32| -> usize {
        match part.as_str() {
            "wind" => wind.shutter(t),
            "school" => school.shutter(t),
            "water" => water.shutter(t),
            _ => shot.as_ref().unwrap().shutter(t),
        }
    };
    println!("new: {:.2} s", start.elapsed().as_secs_f32());
    let times = times();
    let images: Vec<(f32, RgbaImage)> = std::thread::scope(|s| {
        let handles: Vec<_> = times
            .iter()
            .enumerate()
            .map(|(i, &t)| {
                let renderer = if cpu && i % 2 == 1 {
                    renderer.clone().cpu()
                } else {
                    renderer.clone()
                };
                let (set, shutter) = (&set, &shutter);
                let white = if part == "shot" {
                    montage::final_white(t)
                } else {
                    0.0
                };
                s.spawn(move || {
                    let start = Instant::now();
                    let samples = shutter(t);
                    let image = picture(&renderer, samples, t, white, set);
                    println!(
                        "t {t:6.3}: {:7.1} ms ({samples} samples{})",
                        start.elapsed().as_secs_f32() * 1000.0,
                        if renderer.uses_gpu() { "" } else { ", cpu" }
                    );
                    (t, image)
                })
            })
            .collect();
        handles.into_iter().map(|h| h.join().unwrap()).collect()
    });
    let (w, h) = (images[0].1.width(), images[0].1.height());
    let columns = 3.min(images.len()) as u32;
    let rows = (images.len() as u32).div_ceil(columns);
    let mut sheet = RgbaImage::new(w * columns, h * rows);
    for (i, (t, image)) in images.iter().enumerate() {
        image
            .save(out_dir().join(format!("{part}_{t:06.3}.png")))
            .unwrap();
        let i = i as u32;
        image::imageops::replace(
            &mut sheet,
            image,
            i64::from(i % columns * w),
            i64::from(i / columns * h),
        );
    }
    sheet
        .save(out_dir().join(format!("sheet_{part}.png")))
        .unwrap();
}

#[test]
#[ignore = "measures the GPU frame time of the shot"]
fn timing() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("dynamic_media");
    let assets = assets::Assets::load(&dir).unwrap();
    let set = montage::MontageSet::new(&assets);
    let renderer = montage::MontageSet::renderer(1920, 804);
    assert!(renderer.uses_gpu(), "no GPU");
    let (from, to) = std::env::var("BUILD_RANGE").map_or((3.5, 8.0), |s| {
        let p: Vec<f32> = s.split(':').map(|s| s.parse().unwrap()).collect();
        (p[0], p[1])
    });
    for t in [from, to] {
        look::picture_pixels(&renderer, set.shutter(t), t, |t| set.moment(t));
    }
    let (mut worst, mut sum, mut n) = (0.0f32, 0.0, 0);
    let mut t = from;
    while t < to {
        let start = Instant::now();
        let _ = look::picture_pixels(&renderer, set.shutter(t), t, |t| set.moment(t));
        let ms = start.elapsed().as_secs_f32() * 1000.0;
        println!("t {t:6.3}: {ms:6.1} ms ({} samples)", set.shutter(t));
        worst = worst.max(ms);
        sum += ms;
        n += 1;
        t += 0.1;
    }
    println!("frame: mean {:.1} ms, worst {worst:.1} ms", sum / n as f32);
}
