//! The development harness of shot 8 ("The why"). It compiles the montage next to copies
//! of the shared modules and writes review frames to `/home/neogoose/dev/the-why-review/montage/dev`.
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

/// The review times: the bridge, the ring, the drones, the green dome, the whip pan, the
/// line on the tower, the ignition, the steam, the climb, the flame and the flare.
const REVIEW: [f32; 16] = [
    0.0, 0.5, 1.5, 3.0, 4.5, 5.5, 7.5, 8.6, 9.95, 10.5, 11.2, 12.4, 13.2, 14.0, 14.7, 15.0,
];

fn out_dir() -> PathBuf {
    let dir = PathBuf::from("/home/neogoose/dev/the-why-review/montage/dev");
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

/// Renders `jobs` on two GPU workers and one CPU worker in parallel. Each worker takes the
/// next job when it finishes one. `render(renderer, job)` makes the picture of a job.
fn render_parallel<J: Sync, R: Send>(
    renderer: &fframes_3d::Renderer,
    jobs: &[J],
    render: impl Fn(&fframes_3d::Renderer, &J) -> R + Sync,
) -> Vec<R> {
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicUsize, Ordering};
    let next = AtomicUsize::new(0);
    let results: Mutex<Vec<Option<R>>> = Mutex::new((0..jobs.len()).map(|_| None).collect());
    let workers = [renderer.clone(), renderer.clone(), renderer.clone().cpu()];
    std::thread::scope(|scope| {
        for worker in &workers {
            scope.spawn(|| {
                loop {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    let Some(job) = jobs.get(i) else { break };
                    let result = render(worker, job);
                    results.lock().unwrap()[i] = Some(result);
                }
            });
        }
    });
    results
        .into_inner()
        .unwrap()
        .into_iter()
        .map(Option::unwrap)
        .collect()
}

/// Renders the shot times in `DETAIL_T` at 1920 x 804 into `/home/neogoose/dev/the-why-review/montage/detail`, with
/// the prefix `DETAIL_NAME`. GPU and CPU workers render in parallel.
#[test]
#[ignore = "renders detail frames"]
fn detail_frames() {
    let set = load();
    let renderer = montage::MontageSet::renderer(1920, 804);
    let name = std::env::var("DETAIL_NAME").unwrap_or_default();
    let dir = PathBuf::from("/home/neogoose/dev/the-why-review/montage/detail");
    std::fs::create_dir_all(&dir).unwrap();
    let list: Vec<f32> = std::env::var("DETAIL_T")
        .unwrap()
        .split(',')
        .map(|s| s.trim().parse().unwrap())
        .collect();
    let images = render_parallel(&renderer, &list, |r, &t| picture(&set, r, t));
    for (t, image) in list.iter().zip(images) {
        image
            .save(dir.join(format!("{name}t{t:05.2}.png")))
            .unwrap();
    }
}

/// A contact sheet of `DETAIL_SHEET` (`from:to:count` in shot seconds, 4 columns of
/// 960 x 402) into `/home/neogoose/dev/the-why-review/montage/detail/<DETAIL_NAME>sheet.png`. GPU and CPU workers render
/// in parallel.
#[test]
#[ignore = "renders a detail contact sheet"]
fn detail_sheet() {
    let spec = std::env::var("DETAIL_SHEET").unwrap();
    let parts: Vec<f32> = spec.split(':').map(|s| s.parse().unwrap()).collect();
    let (from, to, count) = (parts[0], parts[1], parts[2] as usize);
    let name = std::env::var("DETAIL_NAME").unwrap_or_default();
    let set = load();
    let renderer = montage::MontageSet::renderer(960, 402);
    let list: Vec<f32> = (0..count)
        .map(|i| from + (to - from) * i as f32 / (count - 1).max(1) as f32)
        .collect();
    let images = render_parallel(&renderer, &list, |r, &t| picture(&set, r, t));
    let columns = 4;
    let rows = count.div_ceil(columns);
    let mut sheet = RgbaImage::new(960 * columns as u32, 402 * rows as u32);
    for (i, image) in images.iter().enumerate() {
        image::imageops::replace(
            &mut sheet,
            image,
            (i % columns) as i64 * 960,
            (i / columns) as i64 * 402,
        );
    }
    let dir = PathBuf::from("/home/neogoose/dev/the-why-review/montage/detail");
    std::fs::create_dir_all(&dir).unwrap();
    sheet.save(dir.join(format!("{name}sheet.png"))).unwrap();
}

/// Renders the greenhouse part at its local times in `MONTAGE_GH`: `from:to:count` or a
/// list (default: every second frame from 0 to 1.5 s, the reveal of the ground). The
/// frames render in parallel threads, every second one on the CPU, at the width
/// `MONTAGE_WIDTH` (default 960). Writes `gh/t*.png` and the contact sheet `gh/sheet.png`.
#[test]
#[ignore = "renders the greenhouse"]
fn greenhouse_frames() {
    let spec = std::env::var("MONTAGE_GH").unwrap_or_else(|_| "0:1.5:46".to_owned());
    let times: Vec<f32> = if spec.contains(':') {
        let p: Vec<f32> = spec.split(':').map(|s| s.parse().unwrap()).collect();
        let n = p[2] as usize;
        (0..n)
            .map(|i| p[0] + (p[1] - p[0]) * i as f32 / (n - 1).max(1) as f32)
            .collect()
    } else {
        spec.split(',').map(|s| s.trim().parse().unwrap()).collect()
    };
    let width: u32 = std::env::var("MONTAGE_WIDTH").map_or(960, |s| s.parse().unwrap());
    let height = (width as f32 / 2.388).round() as u32 & !1;
    let set = load();
    let gpu = montage::MontageSet::renderer(width, height);
    let cpu = gpu.clone().cpu();
    let dir = out_dir().join("gh");
    std::fs::create_dir_all(&dir).unwrap();
    let start = Instant::now();
    let frames: Vec<RgbaImage> = std::thread::scope(|s| {
        let jobs: Vec<_> = times
            .iter()
            .enumerate()
            .map(|(i, &t)| {
                let (set, renderer) = (&set, if i % 2 == 0 { &gpu } else { &cpu });
                s.spawn(move || picture(set, renderer, montage::GREENHOUSE + t))
            })
            .collect();
        jobs.into_iter().map(|j| j.join().unwrap()).collect()
    });
    println!(
        "{} frames in {:.1} s",
        frames.len(),
        start.elapsed().as_secs_f32()
    );
    let columns = 6;
    let rows = frames.len().div_ceil(columns) as u32;
    let mut sheet = RgbaImage::new(width * columns as u32, height * rows);
    for (i, (frame, t)) in frames.iter().zip(&times).enumerate() {
        frame.save(dir.join(format!("t{t:05.3}.png"))).unwrap();
        image::imageops::replace(
            &mut sheet,
            frame,
            (i % columns) as i64 * i64::from(width),
            (i / columns) as i64 * i64::from(height),
        );
    }
    sheet.save(dir.join("sheet.png")).unwrap();
}

/// Measures what costs time in the frame at the shot time `DETAIL_T` on the GPU: the full
/// frame, and the frame without its particles, its point and spot lights, its haze, or its
/// large instanced meshes. Each is the best of four runs.
#[test]
#[ignore = "measures the parts of a frame"]
fn detail_cost() {
    use fframes_3d::{Light, Object};
    let set = load();
    let renderer = montage::MontageSet::renderer(1920, 804);
    let t: f32 = std::env::var("DETAIL_T").unwrap().parse().unwrap();
    let (scene, camera) = set.moment(t);
    let best = |scene: &fframes_3d::Scene| {
        (0..4)
            .map(|_| {
                let start = Instant::now();
                let _ = renderer.render_pixels(scene, &camera);
                start.elapsed().as_secs_f32() * 1000.0
            })
            .fold(f32::MAX, f32::min)
    };
    println!("full: {:.1} ms", best(&scene));
    // frames that each come with new instance lists, as in a film
    for dt in [0.0, 0.01, 0.02, 0.03] {
        let (scene, camera) = set.moment(t + dt);
        let start = Instant::now();
        let _ = renderer.render_pixels(&scene, &camera);
        println!(
            "  new frame: {:.1} ms",
            start.elapsed().as_secs_f32() * 1000.0
        );
    }
    let mut without = scene.clone();
    for (id, _) in scene.nodes() {
        if let Object::Particles(p) = &mut without[id].object {
            let large = p
                .items
                .iter()
                .filter(|q| q.size / camera.position.distance(q.position).max(0.1) > 0.02)
                .count();
            println!(
                "  particles {id:?}: {} ({large} large on the screen)",
                p.items.len()
            );
            p.items.clear();
        }
    }
    println!("no particles: {:.1} ms", best(&without));
    let mut without = scene.clone();
    for (id, node) in scene.nodes() {
        if matches!(node.object, Object::Light(Light::Point(_) | Light::Spot(_))) {
            without[id].visible = false;
        }
    }
    println!("no point and spot lights: {:.1} ms", best(&without));
    let mut without = scene.clone();
    without.atmosphere = None;
    println!("no haze: {:.1} ms", best(&without));
    let mut without = scene.clone();
    for (id, node) in scene.nodes() {
        if let Some(mesh) = node.as_mesh()
            && mesh
                .instances
                .as_ref()
                .is_some_and(|i| i.transforms.len() > 200)
        {
            println!(
                "  instanced: {} x {} triangles",
                mesh.instances.as_ref().unwrap().transforms.len(),
                mesh.geometry.triangle_count()
            );
            without[id].visible = false;
        }
    }
    println!("no large instanced meshes: {:.1} ms", best(&without));
}
