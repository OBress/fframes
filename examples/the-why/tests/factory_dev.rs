//! Review frames of the factory shots in `the-why-factory` in the temp directory. Run with
//! `--ignored`. `SHOT` is `factory`, `engineer` or `wave`. `TIMES=1,2.5` chooses the seconds
//! into the shot. `sheet` makes a contact sheet of `N` frames from `FROM` to `TO`.
#![allow(clippy::unreadable_literal)]

pub const WIDTH: usize = 3840;
pub const HEIGHT: usize = 2160;
pub const FPS: usize = 60;
#[path = "../src/assets.rs"]
pub mod assets;
#[path = "../src/city.rs"]
pub mod city;
#[path = "../src/factory/mod.rs"]
pub mod factory;
#[path = "../src/figure.rs"]
pub mod figure;
#[path = "../src/look.rs"]
pub mod look;
#[path = "../src/stars.rs"]
pub mod stars;

use std::path::Path;
use std::time::Instant;

use factory::FactorySet;
use fframes_3d::{Camera, Pixels, Scene};

type Shutter = fn(&FactorySet, f32) -> usize;
type Moment = fn(&FactorySet, f32) -> (Scene, Camera);

/// The shot in `$SHOT`: its name, the motion blur samples and the moment.
fn shot() -> (String, Shutter, Moment) {
    let name = std::env::var("SHOT").unwrap_or_else(|_| "factory".into());
    let (shutter, moment): (Shutter, Moment) = match name.as_str() {
        "engineer" => (FactorySet::shutter_engineer, FactorySet::moment_engineer),
        "wave" => (FactorySet::shutter_wave, FactorySet::moment_wave),
        _ => (FactorySet::shutter_factory, FactorySet::moment_factory),
    };
    (name, shutter, moment)
}

fn env_f32(key: &str, default: f32) -> f32 {
    std::env::var(key).map_or(default, |s| s.parse().unwrap())
}

fn set() -> FactorySet {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("dynamic_media");
    let assets = assets::Assets::load(&dir).unwrap();
    FactorySet::new(&assets)
}

fn out_dir() -> std::path::PathBuf {
    let out = std::env::temp_dir().join("the-why-factory");
    std::fs::create_dir_all(&out).unwrap();
    out
}

fn image(pixels: &Pixels) -> image::RgbaImage {
    image::RgbaImage::from_raw(pixels.width, pixels.height, pixels.to_straight_rgba()).unwrap()
}

/// Renders the frames at `$TIMES` at 1920 x 804 and prints the time of each.
#[test]
#[ignore = "renders review frames"]
fn review() {
    let (name, shutter, moment) = shot();
    let set = set();
    let renderer = FactorySet::renderer(1920, 804);
    let times: Vec<f32> = std::env::var("TIMES").map_or_else(
        |_| vec![0.0, 3.0, 6.0],
        |s| s.split(',').map(|t| t.parse().unwrap()).collect(),
    );
    for t in times {
        let start = Instant::now();
        let _ = moment(&set, t);
        let posed = start.elapsed();
        let start = Instant::now();
        let samples = shutter(&set, t);
        let pixels = look::picture_pixels(&renderer, samples, t, |t| moment(&set, t));
        eprintln!(
            "{name} {t}: {:?} with {samples} samples, moment {posed:?}",
            start.elapsed()
        );
        image(&pixels)
            .save(out_dir().join(format!("{name}_{t:05.2}.png")))
            .unwrap();
    }
}

/// A contact sheet of `$N` frames from `$FROM` to `$TO` seconds, 5 frames in a row.
#[test]
#[ignore = "renders a contact sheet"]
fn sheet() {
    let (name, shutter, moment) = shot();
    let set = set();
    let (w, h) = (768u32, 322u32);
    let renderer = FactorySet::renderer(w, h);
    let (from, to) = (env_f32("FROM", 0.0), env_f32("TO", 8.0));
    let n = env_f32("N", 10.0) as u32;
    let columns = 5.min(n);
    let mut sheet = image::RgbaImage::new(w * columns, h * n.div_ceil(columns));
    for i in 0..n {
        let t = from + (to - from) * i as f32 / (n - 1).max(1) as f32;
        let pixels = look::picture_pixels(&renderer, shutter(&set, t), t, |t| moment(&set, t));
        let (x, y) = (i % columns * w, i / columns * h);
        image::imageops::overlay(&mut sheet, &image(&pixels), x.into(), y.into());
    }
    let path = out_dir().join(format!("{name}_sheet_{from:05.2}_{to:05.2}.png"));
    sheet.save(&path).unwrap();
    eprintln!("{}", path.display());
}

/// The frame time at 1920 x 804 over the shot in `$SHOT`, every `$STEP` seconds.
#[test]
#[ignore = "measures frame times"]
fn bench() {
    let (name, shutter, moment) = shot();
    let set = set();
    let renderer = FactorySet::renderer(1920, 804);
    let (length, step) = (env_f32("LENGTH", 12.0), env_f32("STEP", 0.5));
    // compile the pipelines first
    for t in [0.0, length * 0.5, length - 0.1] {
        let _ = look::picture_pixels(&renderer, shutter(&set, t), t, |t| moment(&set, t));
    }
    let mut times = Vec::new();
    let mut t = 0.0;
    // the best of three renders, as other programs may use the GPU at the same time
    while t < length {
        let ms = (0..3)
            .map(|_| {
                let start = Instant::now();
                let _ = look::picture_pixels(&renderer, shutter(&set, t), t, |t| moment(&set, t));
                start.elapsed().as_secs_f32() * 1000.0
            })
            .fold(f32::MAX, f32::min);
        times.push((t, shutter(&set, t), ms));
        t += step;
    }
    for (t, samples, ms) in &times {
        eprintln!("{name} {t:5.2}: {ms:6.1} ms, {samples} samples");
    }
    let mean = times.iter().map(|t| t.2).sum::<f32>() / times.len() as f32;
    let max = times.iter().map(|t| t.2).fold(0.0, f32::max);
    eprintln!("{name}: mean {mean:.1} ms, max {max:.1} ms");
}
