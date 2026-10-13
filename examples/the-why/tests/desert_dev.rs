//! Renders review frames of the desert set (shots 5, 6 and 9) without `src/lib.rs`.
//!
//! `DESERT_SHOTS` selects the shots (`walk,line,sunrise`, default all). `DESERT_TIMES`
//! gives the times in seconds (default 8 times over the shot), or `a..b` for every frame
//! from `a` to `b`. The PNG files and one contact sheet per shot go to `DESERT_OUT`
//! (default `/home/neogoose/dev/the-why-review/desert/dev`).

#![allow(clippy::unreadable_literal)]

pub const WIDTH: usize = 3840;
pub const HEIGHT: usize = 2160;
pub const FPS: usize = 60;

#[path = "../src/assets.rs"]
pub mod assets;
#[path = "../src/desert/mod.rs"]
pub mod desert;
#[path = "../src/figure.rs"]
pub mod figure;
#[path = "../src/look.rs"]
pub mod look;
#[path = "../src/montage/mod.rs"]
pub mod montage;
#[path = "../src/stars.rs"]
pub mod stars;

use std::path::{Path, PathBuf};
use std::time::Instant;

use desert::DesertSet;
use fframes_3d::{Camera, Pixels, Renderer, Scene};
use image::RgbaImage;

/// The picture size of the review frames: 1920 x 804, the 2.39:1 picture at half size.
const SIZE: (u32, u32) = (1920, 804);

fn out_dir() -> PathBuf {
    let dir = std::env::var("DESERT_OUT").map_or_else(
        |_| PathBuf::from("/home/neogoose/dev/the-why-review/desert/dev"),
        PathBuf::from,
    );
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn load() -> DesertSet {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("dynamic_media");
    let assets = assets::Assets::load(&dir).unwrap();
    let start = Instant::now();
    let set = DesertSet::new(&assets);
    eprintln!("new: {:.2} s", start.elapsed().as_secs_f32());
    set
}

/// One shot: its name, its length, its renderer, its moments and its shutter.
struct Shot<'a> {
    name: &'static str,
    seconds: f32,
    renderer: Renderer,
    moment: Box<dyn Fn(f32) -> (Scene, Camera) + 'a>,
    shutter: Box<dyn Fn(f32) -> usize + 'a>,
    /// Work on the pixels after the render, as in `src/lib.rs`.
    post: Post<'a>,
}

/// Work on the pixels of a shot at a time.
type Post<'a> = Box<dyn Fn(f32, &mut Pixels) + 'a>;

fn shots(set: &DesertSet) -> Vec<Shot<'_>> {
    let (w, h) = (SIZE.0 * supersample(), SIZE.1 * supersample());
    vec![
        Shot {
            name: "walk",
            seconds: desert::WALK_SECONDS,
            renderer: DesertSet::renderer_walk(w, h),
            moment: Box::new(|t| set.moment_walk(t)),
            shutter: Box::new(|t| set.shutter_walk(t)),
            post: Box::new(|_, _| {}),
        },
        Shot {
            name: "line",
            seconds: desert::LINE_SECONDS,
            renderer: DesertSet::renderer_line(w, h),
            moment: Box::new(|t| set.moment_line(t)),
            shutter: Box::new(|t| set.shutter_line(t)),
            post: Box::new(|t, pixels| set.heat_shimmer(t, pixels)),
        },
        Shot {
            name: "sunrise",
            seconds: desert::SUNRISE_SECONDS,
            renderer: DesertSet::renderer_sunrise(w, h),
            moment: Box::new(|t| set.moment_sunrise(t)),
            shutter: Box::new(|t| set.shutter_sunrise(t)),
            post: Box::new(|_, _| {}),
        },
    ]
}

/// `DESERT_SS`: the review frames render this many times larger and shrink to [`SIZE`].
/// With 2 or more they show the picture without aliasing.
fn supersample() -> u32 {
    std::env::var("DESERT_SS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(1)
}

fn selected(name: &str) -> bool {
    std::env::var("DESERT_SHOTS").map_or(true, |s| s.split(',').any(|n| n == name))
}

fn times(seconds: f32) -> Vec<f32> {
    match std::env::var("DESERT_TIMES") {
        Ok(range) if range.contains("..") => {
            let (a, b) = range.split_once("..").unwrap();
            let (a, b): (f32, f32) = (a.parse().unwrap(), b.parse().unwrap());
            let first = (a * FPS as f32).round() as u32;
            let last = (b * FPS as f32).round() as u32;
            (first..last).map(|f| f as f32 / FPS as f32).collect()
        }
        Ok(list) => list
            .split(',')
            .filter_map(|s| s.trim().parse().ok())
            .collect(),
        Err(_) => (0..8)
            .map(|i| i as f32 / 7.0 * (seconds - 1.0 / 60.0))
            .collect(),
    }
}

fn to_image(pixels: &Pixels) -> RgbaImage {
    RgbaImage::from_raw(pixels.width, pixels.height, pixels.to_straight_rgba()).unwrap()
}

/// Saves the frames as one image with `columns` frames in a row, each at half size.
fn contact_sheet(frames: &[RgbaImage], columns: usize, path: &Path) {
    let (w, h) = (frames[0].width() / 2, frames[0].height() / 2);
    let rows = frames.len().div_ceil(columns);
    let mut sheet = RgbaImage::new(
        w * columns as u32 + 4 * (columns as u32 - 1),
        h * rows as u32 + 4 * (rows as u32 - 1),
    );
    for (i, frame) in frames.iter().enumerate() {
        let small = image::imageops::resize(frame, w, h, image::imageops::FilterType::Triangle);
        let (x, y) = (
            (i % columns) as u32 * (w + 4),
            (i / columns) as u32 * (h + 4),
        );
        image::imageops::overlay(&mut sheet, &small, x.into(), y.into());
    }
    sheet.save(path).unwrap();
}

#[test]
#[ignore = "renders review frames"]
fn review() {
    let set = load();
    for shot in shots(&set) {
        if !selected(shot.name) {
            continue;
        }
        let mut frames = Vec::new();
        for t in times(shot.seconds) {
            let start = Instant::now();
            let mut pixels =
                look::picture_pixels(&shot.renderer, (shot.shutter)(t), t, &shot.moment);
            // `DESERT_RAW` leaves out the work on the pixels, for comparisons.
            if std::env::var("DESERT_RAW").is_err() {
                (shot.post)(t, &mut pixels);
            }
            eprintln!(
                "{} t={t:.2}: {:.0} ms",
                shot.name,
                start.elapsed().as_secs_f32() * 1000.0
            );
            let mut image = to_image(&pixels);
            if supersample() > 1 {
                let (w, h) = SIZE;
                image =
                    image::imageops::resize(&image, w, h, image::imageops::FilterType::Lanczos3);
            }
            image
                .save(out_dir().join(format!("{}_t{t:05.2}.png", shot.name)))
                .unwrap();
            frames.push(image);
        }
        if (2..=24).contains(&frames.len()) {
            contact_sheet(
                &frames,
                2,
                &out_dir().join(format!("{}_sheet.png", shot.name)),
            );
        }
    }
}

/// Measures the time of a frame on the GPU, with the motion blur of each moment. Each frame
/// renders three times and the fastest run counts, because other jobs share the GPU.
#[test]
#[ignore = "measures the frame time"]
fn frame_time() {
    let set = load();
    // An empty frame shows the cost of the readback and of the other jobs on the GPU.
    let (w, h) = SIZE;
    let empty = (0..5)
        .map(|_| {
            let start = Instant::now();
            let _ = Renderer::new(w, h).render_pixels(&Scene::new(), &Camera::default());
            start.elapsed().as_secs_f32()
        })
        .fold(f32::INFINITY, f32::min);
    eprintln!("empty frame: {:.0} ms", empty * 1000.0);
    for shot in shots(&set) {
        if !selected(shot.name) {
            continue;
        }
        assert!(shot.renderer.uses_gpu(), "no GPU");
        let mut frames = Vec::new();
        let mut moment_worst = 0.0f32;
        for i in 0..24 {
            let t = i as f32 / 24.0 * shot.seconds;
            let start = Instant::now();
            let _ = (shot.moment)(t);
            moment_worst = moment_worst.max(start.elapsed().as_secs_f32());
            let fastest = (0..3)
                .map(|_| {
                    let start = Instant::now();
                    let _ =
                        look::picture_pixels(&shot.renderer, (shot.shutter)(t), t, &shot.moment);
                    start.elapsed().as_secs_f32()
                })
                .fold(f32::INFINITY, f32::min);
            frames.push((fastest, t));
        }
        frames.sort_by(|a, b| a.0.total_cmp(&b.0));
        let (worst, at) = frames[frames.len() - 1];
        eprintln!(
            "{}: median frame {:.0} ms, worst frame {:.0} ms at {at:.2} s, worst moment {:.1} ms",
            shot.name,
            frames[frames.len() / 2].0 * 1000.0,
            worst * 1000.0,
            moment_worst * 1000.0
        );
    }
}

/// Renders the scene of a shot at `DESERT_T` from the camera `DESERT_EYE` (x,y,z) that looks
/// at `DESERT_AT` (x,y,z), to inspect the world.
#[test]
#[ignore = "renders a probe image"]
fn probe() {
    let set = load();
    let vec = |name: &str, default: [f32; 3]| {
        let v: Vec<f32> = std::env::var(name).map_or_else(
            |_| default.to_vec(),
            |s| s.split(',').filter_map(|x| x.trim().parse().ok()).collect(),
        );
        fframes_3d::vec3(v[0], v[1], v[2])
    };
    let t: f32 = std::env::var("DESERT_T")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(5.0);
    let mm: f32 = std::env::var("DESERT_MM")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(35.0);
    for shot in shots(&set) {
        if !selected(shot.name) {
            continue;
        }
        let (scene, camera) = (shot.moment)(t);
        let eye = vec("DESERT_EYE", camera.position.to_array());
        let at = vec("DESERT_AT", camera.target.to_array());
        let camera = look::lens(mm).position(eye).look_at(at).clip(0.05, 9000.0);
        let pixels = shot.renderer.render_pixels(&scene, &camera);
        to_image(&pixels)
            .save(out_dir().join(format!("probe_{}.png", shot.name)))
            .unwrap();
    }
}
