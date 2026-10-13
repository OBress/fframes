//! Compares the CPU rasterizer with the GPU renderer on frames of every shot, and times both.
//! Run with `--ignored`. `SCALE=1` renders at the 4K picture size (default 0.5). `SHOTS=Factory,Wave`
//! picks shots. Images of the frames and of the differences go to `/home/neogoose/dev/the-why-review/cpu/parity`.
#![allow(
    clippy::unreadable_literal,
    clippy::items_after_statements,
    clippy::type_complexity
)]

use std::path::Path;
use std::time::Instant;

use fframes_3d::{Camera, Pixels, Renderer, Scene};
use the_why::assets::Assets;
use the_why::city::CitySet;
use the_why::desert::DesertSet;
use the_why::factory::FactorySet;
use the_why::look;
use the_why::montage::MontageSet;
use the_why::opening::OpeningSet;

type Moment<'a> = Box<dyn Fn(f32) -> (Scene, Camera) + 'a>;
type Shutter<'a> = Box<dyn Fn(f32) -> usize + 'a>;

struct Shot<'a> {
    name: &'static str,
    renderer: Renderer,
    times: Vec<f32>,
    shutter: Shutter<'a>,
    moment: Moment<'a>,
}

/// The differences of two frames in 8-bit steps.
#[derive(Debug, Default)]
struct Diff {
    /// The mean absolute difference over the RGB channels.
    mean: f64,
    /// The mean signed difference (CPU minus GPU) over the RGB channels: a brightness shift.
    bias: f64,
    /// The share of pixels with a channel that differs by more than 4 steps.
    over4: f64,
    /// The largest mean signed difference of a 32 x 32 pixel block: a local brightness shift
    /// that flickers when CPU and GPU frames alternate.
    block: f64,
    /// The pixel position of that block.
    block_at: (usize, usize),
    /// The 99.9th percentile of the channel differences.
    p999: u8,
}

fn diff(cpu: &Pixels, gpu: &Pixels) -> (Diff, Vec<u8>) {
    let (w, h) = (cpu.width as usize, cpu.height as usize);
    let a = cpu.to_straight_rgba();
    let b = gpu.to_straight_rgba();
    let mut histogram = [0u64; 256];
    let (mut sum, mut signed, mut over) = (0f64, 0f64, 0u64);
    let mut heat = vec![0u8; w * h * 4];
    const BLOCK: usize = 32;
    let (bw, bh) = (w.div_ceil(BLOCK), h.div_ceil(BLOCK));
    let mut blocks = vec![(0f64, 0u32); bw * bh];
    for i in 0..w * h {
        let mut max = 0u8;
        let mut s = 0f64;
        for c in 0..3 {
            let d = i32::from(a[i * 4 + c]) - i32::from(b[i * 4 + c]);
            let ad = d.unsigned_abs() as u8;
            histogram[ad as usize] += 1;
            sum += f64::from(ad);
            s += f64::from(d);
            max = max.max(ad);
        }
        signed += s;
        let block = &mut blocks[(i / w / BLOCK) * bw + (i % w) / BLOCK];
        block.0 += s / 3.0;
        block.1 += 1;
        if max > 4 {
            over += 1;
        }
        let v = (u32::from(max) * 16).min(255) as u8;
        heat[i * 4..i * 4 + 4].copy_from_slice(&[v, v, v, 255]);
    }
    let channels = (w * h * 3) as f64;
    let mut seen = 0u64;
    let target = (channels * 0.999) as u64;
    let p999 = histogram
        .iter()
        .position(|n| {
            seen += n;
            seen >= target
        })
        .unwrap_or(255) as u8;
    let (block_index, block) = blocks
        .iter()
        .map(|(s, n)| (s / f64::from((*n).max(1))).abs())
        .enumerate()
        .fold((0, 0.0), |a, b| if b.1 > a.1 { b } else { a });
    let block_at = ((block_index % bw) * BLOCK, (block_index / bw) * BLOCK);
    (
        Diff {
            mean: sum / channels,
            bias: signed / channels,
            over4: over as f64 / (w * h) as f64,
            block,
            block_at,
            p999,
        },
        heat,
    )
}

fn save(path: &Path, w: u32, h: u32, rgba: &[u8]) {
    image::save_buffer(path, rgba, w, h, image::ColorType::Rgba8).unwrap();
}

fn render(renderer: &Renderer, shot: &Shot, t: f32) -> (Pixels, f64) {
    let start = Instant::now();
    let pixels = look::picture_pixels(renderer, (shot.shutter)(t), t, &shot.moment);
    (pixels, start.elapsed().as_secs_f64() * 1000.0)
}

#[test]
#[ignore = "renders frames of every shot on the CPU and the GPU"]
fn parity() {
    let scale: f64 = std::env::var("SCALE").map_or(0.5, |s| s.parse().unwrap());
    let only = std::env::var("SHOTS").ok();
    let out = Path::new("/home/neogoose/dev/the-why-review/cpu/parity");
    std::fs::create_dir_all(out).unwrap();
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("dynamic_media");
    let assets = Assets::load(&dir).unwrap();
    let (w, h) = look::picture_size(scale);
    let opening = OpeningSet::new(&assets);
    let factory = FactorySet::new(&assets);
    let city = CitySet::new(&assets);
    let desert = DesertSet::new(&assets);
    let montage = MontageSet::new(&assets);
    let shots = vec![
        Shot {
            name: "Cursor",
            renderer: OpeningSet::renderer(w, h),
            times: vec![1.0, 4.5, 6.5],
            shutter: Box::new(|t| opening.shutter(t)),
            moment: Box::new(|t| opening.moment(t)),
        },
        Shot {
            name: "Factory",
            renderer: FactorySet::renderer(w, h),
            times: vec![0.5, 2.0, 5.0, 8.0, 11.0],
            shutter: Box::new(|t| factory.shutter_factory(t)),
            moment: Box::new(|t| factory.moment_factory(t)),
        },
        Shot {
            name: "Engineer",
            renderer: FactorySet::renderer(w, h),
            times: vec![1.0, 5.0, 9.5],
            shutter: Box::new(|t| factory.shutter_engineer(t)),
            moment: Box::new(|t| factory.moment_engineer(t)),
        },
        Shot {
            name: "City",
            renderer: CitySet::renderer(w, h),
            times: vec![0.5, 4.0, 8.0, 12.0],
            shutter: Box::new(|t| city.shutter(t)),
            moment: Box::new(|t| city.moment(t)),
        },
        Shot {
            name: "Walk",
            renderer: DesertSet::renderer_walk(w, h),
            times: vec![1.0, 5.0, 9.0],
            shutter: Box::new(|t| desert.shutter_walk(t)),
            moment: Box::new(|t| desert.moment_walk(t)),
        },
        Shot {
            name: "Line",
            renderer: DesertSet::renderer_line(w, h),
            times: vec![1.0, 3.2, 5.5],
            shutter: Box::new(|t| desert.shutter_line(t)),
            moment: Box::new(|t| desert.moment_line(t)),
        },
        Shot {
            name: "Wave",
            renderer: FactorySet::renderer(w, h),
            times: vec![1.0, 4.0, 7.5],
            shutter: Box::new(|t| factory.shutter_wave(t)),
            moment: Box::new(|t| factory.moment_wave(t)),
        },
        Shot {
            name: "Why",
            renderer: MontageSet::renderer(w, h),
            times: vec![1.0, 4.0, 7.0, 10.0, 13.0],
            shutter: Box::new(|t| montage.shutter(t)),
            moment: Box::new(|t| montage.moment(t)),
        },
        Shot {
            name: "Sunrise",
            renderer: DesertSet::renderer_sunrise(w, h),
            times: vec![1.0, 5.0, 9.0],
            shutter: Box::new(|t| desert.shutter_sunrise(t)),
            moment: Box::new(|t| desert.moment_sunrise(t)),
        },
    ];
    println!(
        "{:<9} {:>5} {:>2} {:>8} {:>8} {:>6} {:>6} {:>7} {:>6} {:>4}",
        "shot", "t", "mb", "gpu ms", "cpu ms", "mean", "bias", "over4%", "block", "p999"
    );
    for shot in &shots {
        if only
            .as_ref()
            .is_some_and(|o| !o.split(',').any(|s| s == shot.name))
        {
            continue;
        }
        let gpu_renderer = shot.renderer.clone().gpu();
        let cpu_renderer = shot.renderer.clone().cpu();
        // the first GPU frame of a shot uploads its meshes and compiles its pipelines
        let _ = render(&gpu_renderer, shot, shot.times[0]);
        for &t in &shot.times {
            let (gpu, gpu_ms) = render(&gpu_renderer, shot, t);
            let (cpu, cpu_ms) = render(&cpu_renderer, shot, t);
            let (d, heat) = diff(&cpu, &gpu);
            println!(
                "{:<9} {:>5.1} {:>2} {:>8.0} {:>8.0} {:>6.2} {:>6.2} {:>7.3} {:>6.2} {:>4} {:?}",
                shot.name,
                t,
                (shot.shutter)(t),
                gpu_ms,
                cpu_ms,
                d.mean,
                d.bias,
                d.over4 * 100.0,
                d.block,
                d.p999,
                d.block_at
            );
            let name = format!("{}_{t}", shot.name);
            save(
                &out.join(format!("{name}_gpu.png")),
                w,
                h,
                &gpu.to_straight_rgba(),
            );
            save(
                &out.join(format!("{name}_cpu.png")),
                w,
                h,
                &cpu.to_straight_rgba(),
            );
            save(&out.join(format!("{name}_diff.png")), w, h, &heat);
        }
    }
}

/// The differences of one frame with features turned off, to find the features that differ.
/// `SHOT=Factory T=0.5`. `DIM=0.01` renders without bloom at that exposure, to compare the
/// brightest parts.
#[test]
#[ignore = "renders frames on the CPU and the GPU"]
fn ablate() {
    use fframes_3d::Object;
    let scale: f64 = std::env::var("SCALE").map_or(0.5, |s| s.parse().unwrap());
    let name = std::env::var("SHOT").unwrap_or_else(|_| "Factory".into());
    let t: f32 = std::env::var("T").map_or(0.5, |s| s.parse().unwrap());
    let out = Path::new("/home/neogoose/dev/the-why-review/cpu/ablate");
    std::fs::create_dir_all(out).unwrap();
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("dynamic_media");
    let assets = Assets::load(&dir).unwrap();
    let (w, h) = look::picture_size(scale);
    let (renderer, (scene, camera)) = match name.as_str() {
        "Cursor" => (
            OpeningSet::renderer(w, h),
            OpeningSet::new(&assets).moment(t),
        ),
        "Engineer" => (
            FactorySet::renderer(w, h),
            FactorySet::new(&assets).moment_engineer(t),
        ),
        "Wave" => (
            FactorySet::renderer(w, h),
            FactorySet::new(&assets).moment_wave(t),
        ),
        "City" => (CitySet::renderer(w, h), CitySet::new(&assets).moment(t)),
        "Walk" => (
            DesertSet::renderer_walk(w, h),
            DesertSet::new(&assets).moment_walk(t),
        ),
        "Line" => (
            DesertSet::renderer_line(w, h),
            DesertSet::new(&assets).moment_line(t),
        ),
        "Sunrise" => (
            DesertSet::renderer_sunrise(w, h),
            DesertSet::new(&assets).moment_sunrise(t),
        ),
        "Why" => (
            MontageSet::renderer(w, h),
            MontageSet::new(&assets).moment(t),
        ),
        _ => (
            FactorySet::renderer(w, h),
            FactorySet::new(&assets).moment_factory(t),
        ),
    };
    let only: Option<Vec<String>> = std::env::var("ONLY")
        .ok()
        .map(|s| s.split(',').map(str::to_owned).collect());
    type Edit = fn(&mut Renderer, &mut Scene, &mut Camera);
    let edits: [(&str, Edit); 13] = [
        ("no_shadows", |_, s, _| {
            let ids: Vec<_> = s.nodes().map(|(id, _)| id).collect();
            for id in ids {
                if let Some(fframes_3d::Light::Directional(l)) = s[id].light_mut() {
                    l.shadow = None;
                }
            }
        }),
        ("air_no_noise", |_, s, _| {
            if let Some(a) = &mut s.atmosphere {
                a.noise = 0.0;
            }
        }),
        ("air_only", |r, s, c| {
            c.focus = None;
            s.reflector = None;
            r.bloom = None;
            let ids: Vec<_> = s.nodes().map(|(id, _)| id).collect();
            for id in ids {
                if matches!(s[id].object, Object::Mesh(_) | Object::Particles(_)) {
                    s[id].visible = false;
                }
            }
        }),
        ("full", |_, _, _| {}),
        ("no_dof", |_, _, c| c.focus = None),
        ("no_air", |_, s, _| s.atmosphere = None),
        ("no_refl", |_, s, _| s.reflector = None),
        ("no_bloom", |r, _, _| r.bloom = None),
        ("no_fog", |_, s, _| s.fog = None),
        ("no_particles", |_, s, _| {
            let ids: Vec<_> = s
                .nodes()
                .filter(|(_, n)| matches!(n.object, Object::Particles(_)))
                .map(|(id, _)| id)
                .collect();
            for id in ids {
                s[id].visible = false;
            }
        }),
        ("no_maps", |_, s, _| {
            let ids: Vec<_> = s.nodes().map(|(id, _)| id).collect();
            for id in ids {
                if let Some(m) = s[id].material_mut() {
                    m.normal_map = None;
                }
            }
        }),
        ("no_msaa", |r, _, _| r.antialias = false),
        ("bare", |r, s, c| {
            c.focus = None;
            s.atmosphere = None;
            s.reflector = None;
            s.fog = None;
            r.bloom = None;
        }),
    ];
    println!(
        "{:<13} {:>8} {:>8} {:>6} {:>6} {:>7} {:>6} {:>4}",
        "variant", "gpu ms", "cpu ms", "mean", "bias", "over4%", "block", "p999"
    );
    let _ = renderer.clone().gpu().render_pixels(&scene, &camera);
    for (label, edit) in edits {
        if only.as_ref().is_some_and(|o| !o.iter().any(|s| s == label)) {
            continue;
        }
        let (mut r, mut s, mut c) = (renderer.clone(), scene.clone(), camera);
        edit(&mut r, &mut s, &mut c);
        if let Some(exposure) = std::env::var("DIM").ok().and_then(|d| d.parse().ok()) {
            r.bloom = None;
            r.exposure = exposure;
        }
        let start = Instant::now();
        let gpu = r.clone().gpu().render_pixels(&s, &c);
        let gpu_ms = start.elapsed().as_secs_f64() * 1000.0;
        let start = Instant::now();
        let cpu = r.clone().cpu().render_pixels(&s, &c);
        let cpu_ms = start.elapsed().as_secs_f64() * 1000.0;
        let (d, heat) = diff(&cpu, &gpu);
        println!(
            "{:<13} {:>8.0} {:>8.0} {:>6.2} {:>6.2} {:>7.3} {:>6.2} {:>4} {:?}",
            label,
            gpu_ms,
            cpu_ms,
            d.mean,
            d.bias,
            d.over4 * 100.0,
            d.block,
            d.p999,
            d.block_at
        );
        let stem = format!("{name}_{t}_{label}");
        save(
            &out.join(format!("{stem}_gpu.png")),
            w,
            h,
            &gpu.to_straight_rgba(),
        );
        save(
            &out.join(format!("{stem}_cpu.png")),
            w,
            h,
            &cpu.to_straight_rgba(),
        );
        save(&out.join(format!("{stem}_diff.png")), w, h, &heat);
    }
}

/// The CPU time of this process in seconds.
fn cpu_seconds() -> f64 {
    let stat = std::fs::read_to_string("/proc/self/stat").unwrap();
    let fields: Vec<&str> = stat
        .rsplit(')')
        .next()
        .unwrap()
        .split_whitespace()
        .collect();
    let ticks: f64 = fields[11].parse::<f64>().unwrap() + fields[12].parse::<f64>().unwrap();
    ticks / 100.0
}

/// Times CPU frames of a shot one after another and `N` at once. `SHOT=Factory T=0.5 N=4`.
#[test]
#[ignore = "renders frames on the CPU"]
fn cpu_bench() {
    let scale: f64 = std::env::var("SCALE").map_or(0.5, |s| s.parse().unwrap());
    let name = std::env::var("SHOT").unwrap_or_else(|_| "Factory".into());
    let t: f32 = std::env::var("T").map_or(0.5, |s| s.parse().unwrap());
    let n: usize = std::env::var("N").map_or(4, |s| s.parse().unwrap());
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("dynamic_media");
    let assets = Assets::load(&dir).unwrap();
    let (w, h) = look::picture_size(scale);
    let factory = FactorySet::new(&assets);
    let desert = DesertSet::new(&assets);
    let montage = MontageSet::new(&assets);
    let (renderer, moment): (Renderer, Box<dyn Fn(f32) -> (Scene, Camera) + Sync>) =
        match name.as_str() {
            "Line" => (
                DesertSet::renderer_line(w, h),
                Box::new(|t| desert.moment_line(t)),
            ),
            "Why" => (MontageSet::renderer(w, h), Box::new(|t| montage.moment(t))),
            _ => (
                FactorySet::renderer(w, h),
                Box::new(|t| factory.moment_factory(t)),
            ),
        };
    let cpu = renderer.clone().cpu();
    let (scene, camera) = moment(t);
    let _ = cpu.render_pixels(&scene, &camera);
    let (start, cpu0) = (Instant::now(), cpu_seconds());
    for i in 0..n {
        let (scene, camera) = moment(t + i as f32 / 60.0);
        let _ = cpu.render_pixels(&scene, &camera);
    }
    let (wall, used) = (start.elapsed().as_secs_f64(), cpu_seconds() - cpu0);
    println!(
        "serial:   {:.0} ms per frame, {:.1} cores busy",
        wall * 1000.0 / n as f64,
        used / wall
    );
    let (start, cpu0) = (Instant::now(), cpu_seconds());
    std::thread::scope(|s| {
        for i in 0..n {
            let (cpu, moment) = (&cpu, &moment);
            s.spawn(move || {
                let (scene, camera) = moment(t + i as f32 / 60.0);
                let _ = cpu.render_pixels(&scene, &camera);
            });
        }
    });
    let (wall, used) = (start.elapsed().as_secs_f64(), cpu_seconds() - cpu0);
    println!(
        "parallel: {:.0} ms per frame, {:.1} cores busy",
        wall * 1000.0 / n as f64,
        used / wall
    );
}

/// The differences of each moment of a motion blurred frame alone, and of their average.
/// `SHOT=Factory T=0.5 N=2`.
#[test]
#[ignore = "renders frames on the CPU and the GPU"]
fn moments() {
    let scale: f64 = std::env::var("SCALE").map_or(0.5, |s| s.parse().unwrap());
    let t: f32 = std::env::var("T").map_or(0.5, |s| s.parse().unwrap());
    let n: usize = std::env::var("N").map_or(2, |s| s.parse().unwrap());
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("dynamic_media");
    let assets = Assets::load(&dir).unwrap();
    let (w, h) = look::picture_size(scale);
    let factory = FactorySet::new(&assets);
    let renderer = FactorySet::renderer(w, h);
    let (gpu_r, cpu_r) = (renderer.clone().gpu(), renderer.clone().cpu());
    let offsets: Vec<f32> = (0..n).map(|i| (i as f32 + 0.5) / n as f32 - 0.5).collect();
    let _ = gpu_r.render_pixels(&factory.moment_factory(t).0, &factory.moment_factory(t).1);
    for &o in &offsets {
        let (s, c) = factory.moment_factory(t + o * look::SHUTTER);
        let (d, _) = diff(&cpu_r.render_pixels(&s, &c), &gpu_r.render_pixels(&s, &c));
        println!(
            "moment {o:+.2}: mean {:.2} block {:.2} {:?}",
            d.mean, d.block, d.block_at
        );
    }
    let m = |o: f32| factory.moment_factory(t + o * look::SHUTTER);
    let (d, _) = diff(
        &cpu_r.render_motion_blur_pixels(n, m),
        &gpu_r.render_motion_blur_pixels(n, m),
    );
    println!(
        "blurred: mean {:.2} block {:.2} {:?}",
        d.mean, d.block, d.block_at
    );
}
