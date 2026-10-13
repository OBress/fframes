//! Review frames of the factory shots in `the-why-factory` in the temp directory. Run with
//! `--ignored`. Set `TIMES=1,2.5` to choose the seconds into the shot.
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

use factory::FactorySet;
use fframes_3d::{Camera, Scene};

/// Renders the shot `name` at the seconds in `$TIMES` or else `times`.
fn review(
    name: &str,
    times: &[f32],
    shutter: impl Fn(&FactorySet, f32) -> usize,
    moment: impl Fn(&FactorySet, f32) -> (Scene, Camera),
) {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("dynamic_media");
    let assets = assets::Assets::load(&dir).unwrap();
    let set = FactorySet::new(&assets);
    let renderer = FactorySet::renderer(1920, 804);
    let times: Vec<f32> = std::env::var("TIMES").map_or_else(
        |_| times.to_vec(),
        |s| s.split(',').map(|t| t.parse().unwrap()).collect(),
    );
    let out = std::env::temp_dir().join("the-why-factory");
    std::fs::create_dir_all(&out).unwrap();
    for t in times {
        let start = std::time::Instant::now();
        let pixels = look::picture_pixels(&renderer, shutter(&set, t), t, |t| moment(&set, t));
        eprintln!("{name} {t}: {:?}", start.elapsed());
        image::save_buffer(
            out.join(format!("{name}_{t:05.2}.png")),
            &pixels.to_straight_rgba(),
            pixels.width,
            pixels.height,
            image::ColorType::Rgba8,
        )
        .unwrap();
    }
}

#[test]
#[ignore = "renders review frames"]
fn review_factory() {
    review(
        "factory",
        &[0.0, 1.5, 3.0, 6.0, 9.0, 11.5],
        FactorySet::shutter_factory,
        FactorySet::moment_factory,
    );
}

#[test]
#[ignore = "renders review frames"]
fn review_engineer() {
    review(
        "engineer",
        &[0.0, 1.0, 3.0, 5.0, 6.5, 8.0, 9.0, 9.98],
        FactorySet::shutter_engineer,
        FactorySet::moment_engineer,
    );
}

#[test]
#[ignore = "renders review frames"]
fn review_wave() {
    review(
        "wave",
        &[0.2, 0.7, 1.5, 2.5, 3.5, 4.5, 6.0, 7.2, 8.0, 8.48],
        FactorySet::shutter_wave,
        FactorySet::moment_wave,
    );
}
