//! Review frames of the factory shots for the jaws of the gripper, rendered in parallel:
//! the frames at odd places render on the CPU. Run with `--ignored`. `SHOT` is `factory`,
//! `engineer` or `wave`, `TIMES=1,2.5` the seconds into the shot, `OUT` a name prefix.
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
use fframes_3d::{Camera, Scene};

type Shutter = fn(&FactorySet, f32) -> usize;
type Moment = fn(&FactorySet, f32) -> (Scene, Camera);

#[test]
#[ignore = "renders review frames"]
fn jaws() {
    let name = std::env::var("SHOT").unwrap_or_else(|_| "engineer".into());
    let prefix = std::env::var("OUT").unwrap_or_default();
    let (shutter, moment): (Shutter, Moment) = match name.as_str() {
        "factory" => (FactorySet::shutter_factory, FactorySet::moment_factory),
        "wave" => (FactorySet::shutter_wave, FactorySet::moment_wave),
        _ => (FactorySet::shutter_engineer, FactorySet::moment_engineer),
    };
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("dynamic_media");
    let assets = assets::Assets::load(&dir).unwrap();
    let set = FactorySet::new(&assets);
    let renderer = FactorySet::renderer(1920, 804);
    let times: Vec<f32> = std::env::var("TIMES")
        .unwrap_or_else(|_| "5.8,6.6,7.4".into())
        .split(',')
        .map(|t| t.parse().unwrap())
        .collect();
    let out = std::env::temp_dir().join("the-why-jaws");
    std::fs::create_dir_all(&out).unwrap();
    std::thread::scope(|s| {
        for (i, t) in times.into_iter().enumerate() {
            let cpu = i % 2 == 1;
            let renderer = if cpu {
                renderer.clone().cpu()
            } else {
                renderer.clone()
            };
            let (set, out, name, prefix) = (&set, &out, &name, &prefix);
            s.spawn(move || {
                let start = Instant::now();
                let pixels =
                    look::picture_pixels(&renderer, shutter(set, t), t, |t| moment(set, t));
                let device = if cpu { "cpu" } else { "gpu" };
                eprintln!("{name} {t} on the {device}: {:?}", start.elapsed());
                image::RgbaImage::from_raw(pixels.width, pixels.height, pixels.to_straight_rgba())
                    .unwrap()
                    .save(out.join(format!("{prefix}{name}_{t:05.2}_{device}.png")))
                    .unwrap();
            });
        }
    });
}
