//! Renders review frames of the city set (shot 4) to `/tmp/the-why-city/`.

#![allow(clippy::unreadable_literal)]

pub const WIDTH: usize = 3840;
pub const HEIGHT: usize = 2160;
pub const FPS: usize = 60;

#[path = "../src/assets.rs"]
pub mod assets;
#[path = "../src/city.rs"]
pub mod city;
#[path = "../src/look.rs"]
pub mod look;
#[path = "../src/stars.rs"]
pub mod stars;

use std::path::{Path, PathBuf};
use std::time::Instant;

use image::RgbaImage;

fn out_dir() -> PathBuf {
    let dir = PathBuf::from("/tmp/the-why-city");
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn times() -> Vec<f32> {
    match std::env::var("CITY_TIMES") {
        Ok(list) => list
            .split(',')
            .filter_map(|s| s.trim().parse().ok())
            .collect(),
        Err(_) => vec![0.0, 1.0, 2.0, 3.0, 4.0, 6.0, 8.0, 10.0, 11.5, 12.99],
    }
}

#[test]
#[ignore = "renders review frames"]
fn review() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("dynamic_media");
    let assets = assets::Assets::load(&dir).unwrap();
    let started = Instant::now();
    let set = city::CitySet::new(&assets);
    println!("new: {:.2} s", started.elapsed().as_secs_f32());
    let renderer = city::CitySet::renderer(1920, 804);
    let mut tiles = Vec::new();
    for t in times() {
        let started = Instant::now();
        let _ = set.moment(t);
        let moment = started.elapsed().as_secs_f32() * 1000.0;
        let started = Instant::now();
        let pixels = look::picture_pixels(&renderer, set.shutter(t), t, |t| set.moment(t));
        let frame = started.elapsed().as_secs_f32() * 1000.0;
        println!(
            "t {t:5.2}: moment {moment:5.1} ms, frame {frame:6.1} ms ({} samples)",
            set.shutter(t)
        );
        let image =
            RgbaImage::from_raw(pixels.width, pixels.height, pixels.to_straight_rgba()).unwrap();
        image
            .save(out_dir().join(format!("t{t:05.2}.png")))
            .unwrap();
        tiles.push(image);
    }
    sheet(&tiles, 2).save(out_dir().join("sheet.png")).unwrap();
}

/// The tiles at half size, `columns` in a row.
fn sheet(tiles: &[RgbaImage], columns: u32) -> RgbaImage {
    let (w, h) = (tiles[0].width() / 2, tiles[0].height() / 2);
    let rows = (tiles.len() as u32).div_ceil(columns);
    let mut out = RgbaImage::new(w * columns, h * rows);
    for (i, tile) in tiles.iter().enumerate() {
        let small = image::imageops::resize(tile, w, h, image::imageops::FilterType::Triangle);
        let (x, y) = ((i as u32 % columns) * w, (i as u32 / columns) * h);
        image::imageops::overlay(&mut out, &small, x.into(), y.into());
    }
    out
}

/// A name and a change of the scene, for [`timing`].
type Variant = (&'static str, fn(&mut fframes_3d::Scene));

#[test]
#[ignore = "measures the frame time"]
fn timing() {
    use fframes_3d::{Light, Object};
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("dynamic_media");
    let assets = assets::Assets::load(&dir).unwrap();
    let set = city::CitySet::new(&assets);
    let renderer = city::CitySet::renderer(1920, 804);
    let t = 8.0;
    let variants: [Variant; 5] = [
        ("all", |_| {}),
        ("no haze", |s| s.atmosphere = None),
        ("no reflector", |s| s.reflector = None),
        ("no shadow", |s| {
            let ids: Vec<_> = s.nodes().map(|(id, _)| id).collect();
            for id in ids {
                if let Object::Light(Light::Directional(d)) = &mut s[id].object {
                    d.shadow = None;
                }
            }
        }),
        ("no lamps", |s| {
            let ids: Vec<_> = s.nodes().map(|(id, _)| id).collect();
            for id in ids {
                if let Object::Light(Light::Point(_) | Light::Spot(_)) = &s[id].object {
                    s[id].visible = false;
                }
            }
        }),
    ];
    let t: f32 = std::env::var("CITY_T")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(t);
    let mut best = [f32::MAX; 5];
    for _ in 0..16 {
        for (k, (_, change)) in variants.iter().enumerate() {
            let (mut scene, camera) = set.moment(t);
            change(&mut scene);
            let started = Instant::now();
            let _ = renderer.render_pixels(&scene, &camera);
            best[k] = best[k].min(started.elapsed().as_secs_f32() * 1000.0);
        }
    }
    for ((name, _), ms) in variants.iter().zip(best) {
        println!("{name:>14}: {ms:6.1} ms");
    }
}

/// One frame without haze, for checking the surfaces.
#[test]
#[ignore = "renders a debug frame"]
fn clear() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("dynamic_media");
    let assets = assets::Assets::load(&dir).unwrap();
    let set = city::CitySet::new(&assets);
    let renderer = city::CitySet::renderer(1920, 804);
    let t: f32 = std::env::var("CITY_T")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(8.0);
    let (mut scene, mut camera) = set.moment(t);
    scene.atmosphere = None;
    for (id, node) in scene.nodes() {
        if node.name.as_deref() == Some("crane") {
            println!("crane at {:?}", scene.world_position(id));
        }
    }
    // `CITY_CRANE=n` looks at crane n from 120 m.
    if let Some(n) = std::env::var("CITY_CRANE")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
    {
        let crane = scene
            .nodes()
            .filter(|(_, node)| node.name.as_deref() == Some("crane"))
            .map(|(id, _)| id)
            .nth(n)
            .unwrap();
        let base = scene.world_position(crane);
        let top = base + fframes_3d::Vec3::Y * 95.0;
        camera = look::lens(35.0)
            .position(top + fframes_3d::vec3(-90.0, 15.0, -130.0))
            .look_at(top)
            .clip(1.0, 3000.0);
    }
    let pixels = renderer.render_pixels(&scene, &camera);
    let image =
        RgbaImage::from_raw(pixels.width, pixels.height, pixels.to_straight_rgba()).unwrap();
    image.save(out_dir().join("clear.png")).unwrap();
}

/// The chip of shot 3 in a studio: a close-up at 85 mm from 0.15 m, and the view straight
/// down that matches the first frame of shot 4.
#[test]
#[ignore = "renders the chip"]
fn chip() {
    use fframes_3d::*;
    let renderer = city::CitySet::renderer(1920, 804);
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("dynamic_media");
    let assets = assets::Assets::load(&dir).unwrap();
    let mut scene = Scene::new()
        .environment(Environment::map(assets.factory_sky.clone(), 1.0))
        .background(Background::Environment { blur: 0.4 });
    scene.add(city::chip());
    scene.add(Node::mesh(
        Geometry::floor(1.0, 1.0).translate(0.0, -0.0021, 0.0),
        Material::standard(0x2a2d33).roughness(0.6),
    ));
    scene.add(Node::light(Light::sun(0xcfe4ff, 2.0, vec3(0.3, 1.0, 0.4))));
    let close = look::lens(85.0)
        .position(vec3(0.0, 0.09, 0.12))
        .look_at(vec3(0.0, 0.0, 0.0))
        .clip(0.01, 10.0);
    let close = close.focus_on(Vec3::ZERO, 6.0);
    let mut top = look::lens(35.0)
        .position(vec3(
            0.0,
            0.0015 + city::START_HEIGHT * city::CHIP_SCALE,
            0.0,
        ))
        .look_at(vec3(0.0, 0.0, 0.0))
        .clip(0.001, 10.0);
    top.up = Vec3::Z;
    for (name, camera) in [("chip_close", close), ("chip_top", top)] {
        let pixels = renderer.render_pixels(&scene, &camera);
        let image =
            RgbaImage::from_raw(pixels.width, pixels.height, pixels.to_straight_rgba()).unwrap();
        image.save(out_dir().join(format!("{name}.png"))).unwrap();
    }
}

/// The camera at the end of the shot looks exactly along `END_DIRECTION` with +Y up. It
/// prints the positions of the cranes and the camera path.
#[test]
fn end_direction() {
    let camera = city::camera(city::DURATION);
    let forward = camera.forward();
    let expected = city::END_DIRECTION.normalize();
    assert!(forward.distance(expected) < 1e-5, "{forward} != {expected}");
    assert!(camera.right().y.abs() < 1e-5, "the horizon is level");
    assert!(camera.up_vector().y > 0.0);
    for t in [0.0, 2.0, 4.4, 6.0, 8.0, 9.0, 10.0, 11.0, 12.0, 13.0] {
        let c = city::camera(t);
        println!(
            "t {t:5.2}: {:?} forward {:?} fov {:.1}",
            c.position,
            c.forward(),
            c.fov
        );
    }
}
