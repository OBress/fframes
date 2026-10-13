//! Review frames of the factory hall for ripples (moire, crawl, banding) in
//! `/home/neogoose/dev/the-why-review/hall`. Run with `--ignored`. `SHOT` is `factory` or `wave`. `TIMES=5,5.0167`
//! chooses the seconds into the shot. `SCALE` is the size relative to the 4K picture (default
//! 1). `CPU=k` renders every k-th frame on the CPU, in parallel with the GPU. `CROP=x,y,w,h`
//! also saves a crop. `NOATMO`, `NOREFL` and `HIDE=3,7` (node indices, see `nodes`) remove
//! parts of the scene to find the cause of a pattern.
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

use std::path::{Path, PathBuf};
use std::time::Instant;

use factory::FactorySet;
use fframes_3d::{Camera, NodeId, Object, Pixels, Renderer, Scene};

type Shutter = fn(&FactorySet, f32) -> usize;
type Moment = fn(&FactorySet, f32) -> (Scene, Camera);

fn shot() -> (String, Shutter, Moment) {
    let name = std::env::var("SHOT").unwrap_or_else(|_| "factory".into());
    let (shutter, moment): (Shutter, Moment) = match name.as_str() {
        "wave" => (FactorySet::shutter_wave, FactorySet::moment_wave),
        "engineer" => (FactorySet::shutter_engineer, FactorySet::moment_engineer),
        _ => (FactorySet::shutter_factory, FactorySet::moment_factory),
    };
    (name, shutter, moment)
}

fn env_f32(key: &str, default: f32) -> f32 {
    std::env::var(key).map_or(default, |s| s.parse().unwrap())
}

fn env_list(key: &str) -> Vec<f32> {
    std::env::var(key).map_or_else(
        |_| Vec::new(),
        |s| s.split(',').map(|t| t.trim().parse().unwrap()).collect(),
    )
}

fn set() -> FactorySet {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("dynamic_media");
    let assets = assets::Assets::load(&dir).unwrap();
    FactorySet::new(&assets)
}

fn out_dir() -> PathBuf {
    let out = PathBuf::from("/home/neogoose/dev/the-why-review/hall");
    std::fs::create_dir_all(&out).unwrap();
    out
}

fn image(pixels: &Pixels) -> image::RgbaImage {
    image::RgbaImage::from_raw(pixels.width, pixels.height, pixels.to_straight_rgba()).unwrap()
}

/// The value of `key` in a configuration of `key=value` pairs split by commas.
fn value(config: &str, key: &str) -> Option<f32> {
    config
        .split(',')
        .filter_map(|kv| kv.split_once('='))
        .find(|(k, _)| *k == key)
        .and_then(|(_, v)| v.parse().ok())
}

/// Changes the scene as a configuration says: `steps`, `noise`, `map` (shadow map size),
/// `nodark` (hides spot lights without light), `points` (the number of point and spot lights
/// kept).
fn configure(config: &str, scene: &mut Scene) {
    let hide = config
        .split(',')
        .find_map(|kv| kv.strip_prefix("hide="))
        .unwrap_or("");
    for range in hide.split('+').filter(|r| !r.is_empty()) {
        let (a, b) = range.split_once('-').unwrap_or((range, range));
        for index in a.parse::<u32>().unwrap()..=b.parse::<u32>().unwrap() {
            scene[NodeId(index)].visible = false;
        }
    }
    if let Some(a) = &mut scene.atmosphere {
        a.steps = value(config, "steps").map_or(a.steps, |v| v as u32);
        a.noise = value(config, "noise").unwrap_or(a.noise);
    }
    let ids: Vec<NodeId> = scene.nodes().map(|(id, _)| id).collect();
    let mut points = 0;
    for id in ids {
        match &mut scene[id].object {
            Object::Light(fframes_3d::Light::Directional(d)) => {
                if let (Some(shadow), Some(map)) = (&mut d.shadow, value(config, "map")) {
                    shadow.map_size = map as u32;
                }
            }
            Object::Light(fframes_3d::Light::Spot(spot))
                if spot.intensity == 0.0 && value(config, "nodark").is_some() =>
            {
                scene[id].visible = false;
            }
            Object::Light(fframes_3d::Light::Point(_) | fframes_3d::Light::Spot(_)) => {
                points += 1;
                if value(config, "points").is_some_and(|keep| points > keep as usize) {
                    scene[id].visible = false;
                }
            }
            _ => {}
        }
    }
}

/// The scene at `t` with the parts that the environment removes.
fn moment_for_test(set: &FactorySet, moment: Moment, t: f32) -> (Scene, Camera) {
    let (mut scene, camera) = moment(set, t);
    if std::env::var("NOATMO").is_ok() {
        scene.atmosphere = None;
    }
    if std::env::var("NOREFL").is_ok() {
        scene.reflector = None;
    }
    if let Ok(config) = std::env::var("CONFIG") {
        configure(&config, &mut scene);
    }
    for index in env_list("HIDE") {
        scene[NodeId(index as u32)].visible = false;
    }
    for index in env_list("NOCAST") {
        if let Some(mesh) = scene[NodeId(index as u32)].mesh_mut() {
            mesh.cast_shadow = false;
        }
    }
    let sun = env_f32("SUN", 1.0);
    let ids: Vec<NodeId> = scene.nodes().map(|(id, _)| id).collect();
    for id in ids {
        if let Object::Light(fframes_3d::Light::Directional(d)) = &mut scene[id].object {
            d.intensity *= sun;
        }
    }
    if let Some(a) = &mut scene.atmosphere {
        a.steps = env_f32("STEPS", a.steps as f32) as u32;
        a.noise = env_f32("HAZE_NOISE", a.noise);
        a.density = env_f32("DENSITY", a.density);
    }
    (scene, camera)
}

fn render(set: &FactorySet, renderer: &Renderer, t: f32) -> Pixels {
    let (_, shutter, moment) = shot();
    let samples = std::env::var("SAMPLES").map_or_else(|_| shutter(set, t), |s| s.parse().unwrap());
    look::picture_pixels(renderer, samples, t, |t| moment_for_test(set, moment, t))
}

fn renderer() -> Renderer {
    let scale = env_f32("SCALE", 1.0);
    let (w, h) = (
        (3840.0 * scale).round() as u32 & !1,
        (1608.0 * scale).round() as u32 & !1,
    );
    FactorySet::renderer(w, h)
}

fn crop() -> Option<[u32; 4]> {
    let c = env_list("CROP");
    (c.len() == 4).then(|| [c[0] as u32, c[1] as u32, c[2] as u32, c[3] as u32])
}

/// Renders the frames at `$TIMES`: every `$CPU`-th frame on the CPU, the others on the GPU,
/// in parallel.
fn render_all(set: &FactorySet, times: &[f32]) -> Vec<(f32, Pixels)> {
    let renderer = renderer();
    let every = env_f32("CPU", 0.0) as usize;
    let (cpu, gpu): (Vec<_>, Vec<_>) = times
        .iter()
        .copied()
        .enumerate()
        .partition(|(i, _)| every > 0 && i % every == every - 1);
    let cpu_renderer = renderer.clone().cpu();
    let mut out: Vec<(usize, f32, Pixels)> = std::thread::scope(|s| {
        let cpu_thread = s.spawn(|| {
            cpu.iter()
                .map(|(i, t)| {
                    let start = Instant::now();
                    let p = render(set, &cpu_renderer, *t);
                    eprintln!("cpu {t}: {:?}", start.elapsed());
                    (*i, *t, p)
                })
                .collect::<Vec<_>>()
        });
        let mut done: Vec<(usize, f32, Pixels)> = gpu
            .iter()
            .map(|(i, t)| {
                let start = Instant::now();
                let p = render(set, &renderer, *t);
                eprintln!("gpu {t}: {:?}", start.elapsed());
                (*i, *t, p)
            })
            .collect();
        done.extend(cpu_thread.join().unwrap());
        done
    });
    out.sort_by_key(|(i, ..)| *i);
    out.into_iter().map(|(_, t, p)| (t, p)).collect()
}

/// Saves each frame at `$TIMES`, and its crop at `$CROP`.
#[test]
#[ignore = "renders review frames"]
fn frames() {
    let (name, ..) = shot();
    let tag = std::env::var("TAG").unwrap_or_default();
    let set = set();
    let times = env_list("TIMES");
    for (t, pixels) in render_all(&set, &times) {
        let image = image(&pixels);
        let path = out_dir().join(format!("{tag}{name}_{t:06.3}.png"));
        image.save(&path).unwrap();
        if let Some([x, y, w, h]) = crop() {
            let crop = image::imageops::crop_imm(&image, x, y, w, h).to_image();
            crop.save(out_dir().join(format!("{tag}{name}_{t:06.3}_crop.png")))
                .unwrap();
        }
        eprintln!("{}", path.display());
    }
}

/// `$N` consecutive frames from `$FROM` seconds: their crops at `$CROP` in a column.
#[test]
#[ignore = "renders a column of crops of consecutive frames"]
fn seq() {
    let (name, ..) = shot();
    let tag = std::env::var("TAG").unwrap_or_default();
    let set = set();
    let from = env_f32("FROM", 5.0);
    let step = env_f32("STEP", 1.0 / FPS as f32);
    let n = env_f32("N", 4.0) as usize;
    let times: Vec<f32> = (0..n).map(|i| from + i as f32 * step).collect();
    let [x, y, w, h] = crop().unwrap_or([1400, 0, 1000, 500]);
    let frames = render_all(&set, &times);
    let mut column = image::RgbaImage::new(w, h * n as u32);
    for (i, (_, pixels)) in frames.iter().enumerate() {
        let crop = image::imageops::crop_imm(&image(pixels), x, y, w, h).to_image();
        image::imageops::overlay(&mut column, &crop, 0, (i as u32 * h).into());
    }
    let path = out_dir().join(format!("{tag}{name}_seq_{from:06.3}.png"));
    column.save(&path).unwrap();
    eprintln!("{}", path.display());
}

/// Prints the nodes of the scene: index, instances, vertices and the material color.
#[test]
#[ignore = "lists the nodes"]
fn nodes() {
    let (_, _, moment) = shot();
    let set = set();
    let (scene, _) = moment(&set, env_f32("FROM", 5.0));
    for (id, node) in scene.nodes() {
        if let Object::Light(light) = &node.object {
            eprintln!("{:4} light visible {} {light:?}", id.index(), node.visible);
        }
        if let Object::Mesh(mesh) = &node.object {
            let count = mesh.instances.as_ref().map_or(1, |i| i.transforms.len());
            eprintln!(
                "{:4} x{:6} v{:7} {:?} rough {:.2} metal {:.2} map {}",
                id.index(),
                count,
                mesh.geometry.positions.len(),
                mesh.material.color,
                mesh.material.roughness,
                mesh.material.metalness,
                mesh.material.map.is_some()
            );
        }
    }
}

/// The GPU frame time at 1920 x 804 at `$TIMES`, the best of three.
#[test]
#[ignore = "measures frame times"]
fn bench() {
    let (name, ..) = shot();
    let set = set();
    let renderer = FactorySet::renderer(1920, 804);
    let times = env_list("TIMES");
    for t in &times {
        let _ = render(&set, &renderer, *t);
    }
    for t in times {
        let ms = (0..env_f32("REPS", 3.0) as usize)
            .map(|_| {
                let start = Instant::now();
                let _ = render(&set, &renderer, t);
                start.elapsed().as_secs_f32() * 1000.0
            })
            .fold(f32::MAX, f32::min);
        eprintln!("{name} {t:5.2}: {ms:6.1} ms");
    }
}

/// The GPU frame time at 1920 x 804 at `$TIMES` for each configuration in `$CONFIGS`,
/// interleaved, the best of `$REPS`. A configuration is `key=value` pairs split by commas:
/// `steps`, `noise`, `map` (shadow map size), `points` (the number of point lights kept).
/// Configurations are split by semicolons.
#[test]
#[ignore = "measures frame times"]
fn bench_steps() {
    let (name, shutter, moment) = shot();
    let set = set();
    let renderer = FactorySet::renderer(1920, 804);
    let configs: Vec<String> = std::env::var("CONFIGS")
        .unwrap()
        .split(';')
        .map(str::to_owned)
        .collect();
    for t in env_list("TIMES") {
        let mut best = vec![f32::MAX; configs.len()];
        for _ in 0..env_f32("REPS", 5.0) as usize {
            for (k, config) in configs.iter().enumerate() {
                let start = Instant::now();
                let _ = look::picture_pixels(&renderer, shutter(&set, t), t, |t| {
                    let (mut scene, camera) = moment(&set, t);
                    configure(config, &mut scene);
                    (scene, camera)
                });
                best[k] = best[k].min(start.elapsed().as_secs_f32() * 1000.0);
            }
        }
        for (config, ms) in configs.iter().zip(best) {
            eprintln!("{name} {t:5.2} [{config}]: {ms:6.1} ms");
        }
    }
}

/// The motion of far points over the shutter, in pixels of the 4K picture, every `$STEP`
/// seconds from `$FROM` to `$TO`: at the center and at the corners of the picture.
#[test]
#[ignore = "prints the motion in the shutter"]
fn shutter_motion() {
    let (name, shutter, moment) = shot();
    let set = set();
    let (w, h) = (3840.0, 1608.0);
    let (from, to, step) = (
        env_f32("FROM", 0.0),
        env_f32("TO", 12.0),
        env_f32("STEP", 0.25),
    );
    let mut t = from;
    while t <= to {
        let (_, a) = moment(&set, t - look::SHUTTER * 0.5);
        let (_, b) = moment(&set, t + look::SHUTTER * 0.5);
        // points 300 m away on the rays of the center and the corners of the first camera
        let inverse = (a.projection_matrix(w / h) * a.view_matrix()).inverse();
        let motion: Vec<f32> = [
            (0.0, 0.0),
            (-0.9, -0.9),
            (0.9, -0.9),
            (-0.9, 0.9),
            (0.9, 0.9),
        ]
        .iter()
        .map(|&(x, y)| {
            let far = inverse.project_point3(fframes_3d::vec3(x, y, 0.5));
            let dir = (far - a.position).normalize();
            let p = a.position + dir * 300.0;
            match (a.project(p, w, h), b.project(p, w, h)) {
                (Some(pa), Some(pb)) => pa.truncate().distance(pb.truncate()),
                _ => f32::NAN,
            }
        })
        .collect();
        eprintln!(
            "{name} {t:5.2} samples {}: center {:4.1} px, corners {:4.1} {:4.1} {:4.1} {:4.1}",
            shutter(&set, t),
            motion[0],
            motion[1],
            motion[2],
            motion[3],
            motion[4]
        );
        t += step;
    }
}
