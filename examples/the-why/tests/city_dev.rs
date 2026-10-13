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
    println!(
        "new: {:.2} s, {} screens",
        started.elapsed().as_secs_f32(),
        set.screens().len()
    );
    let renderer = city::CitySet::renderer(1920, 804);
    let (scene, _) = set.moment(8.0);
    for &id in set.screens() {
        if let Some(mesh) = scene[id].as_mesh() {
            let b = mesh.geometry.bounds();
            println!("screen {:?} {:?} seen", b.center(), b.size());
        }
    }
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
        ("no particles", |s| {
            let ids: Vec<_> = s.nodes().map(|(id, _)| id).collect();
            for id in ids {
                if let Object::Particles(p) = &mut s[id].object {
                    p.items.clear();
                }
            }
        }),
        ("no blended", |s| {
            let ids: Vec<_> = s.nodes().map(|(id, _)| id).collect();
            for id in ids {
                if s[id].material_mut().is_some_and(|m| m.transparent) {
                    s[id].visible = false;
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
    // `CITY_HIDE=3,5` hides the root nodes 3 and 5.
    if let Ok(list) = std::env::var("CITY_HIDE") {
        let roots: Vec<_> = scene
            .nodes()
            .filter(|(_, n)| n.parent().is_none())
            .map(|(id, _)| id)
            .collect();
        for item in list.split(',') {
            let (a, b) = item.split_once('-').unwrap_or((item, item));
            let (a, b) = (
                a.parse::<usize>().unwrap_or(0),
                b.parse::<usize>().unwrap_or(0),
            );
            for &id in roots.iter().take(b + 1).skip(a) {
                scene[id].visible = false;
            }
        }
        println!("roots {}", roots.len());
    }
    if std::env::var("CITY_SINGLE").is_ok() {
        let ids: Vec<_> = scene.nodes().map(|(id, _)| id).collect();
        for id in ids {
            if let Some(m) = scene[id].material_mut() {
                m.side = fframes_3d::Side::Front;
            }
        }
    }
    for (id, node) in scene.nodes() {
        if node.name.as_deref() == Some("crane") {
            println!("crane at {:?}", scene.world_position(id));
        }
    }
    // `CITY_LOOK=x,y,z,tx,ty,tz` looks from a point at a target.
    if let Ok(look) = std::env::var("CITY_LOOK") {
        let v: Vec<f32> = look.split(',').filter_map(|v| v.parse().ok()).collect();
        camera = look::lens(28.0)
            .position(fframes_3d::vec3(v[0], v[1], v[2]))
            .look_at(fframes_3d::vec3(v[3], v[4], v[5]))
            .clip(1.0, 9000.0);
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
    // The frame 0 of the cut: straight down, the image top is +Z.
    let first = city::camera(0.0);
    assert!(first.forward().distance(-fframes_3d::Vec3::Y) < 1e-5);
    assert!(first.up_vector().z > 0.999, "{}", first.up_vector());
    assert!(
        (first.position.y - city::START_HEIGHT).abs() < 0.01,
        "{}",
        first.position
    );
    for k in 0..=26 {
        let t = k as f32 * 0.5;
        let c = city::camera(t);
        let p = c.position;
        let speed = c.position.distance(city::camera(t + 0.01).position) / 0.01;
        println!(
            "t {t:5.2}: ({:6.1} {:6.1} {:6.1}) {speed:5.1} m/s forward ({:5.2} {:5.2} {:5.2}) fov {:.1} roll {:.2}",
            p.x,
            p.y,
            p.z,
            c.forward().x,
            c.forward().y,
            c.forward().z,
            c.fov,
            c.roll.to_degrees()
        );
    }
}

/// The distance of the flight from the buildings: the times where the camera passes
/// closer than 12 m to a building that is higher than the camera.
#[test]
fn clearance() {
    use fframes_3d::*;
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("dynamic_media/nyc/manhattan.bin");
    let data = city::data::Manhattan::load(&std::fs::read(&path).unwrap()).unwrap();
    // The distance from `p` to the triangle `t` on the ground plane.
    let distance = |p: Vec2, t: [Vec2; 3]| {
        let side = |a: Vec2, b: Vec2| (b - a).perp_dot(p - a);
        let s = [side(t[0], t[1]), side(t[1], t[2]), side(t[2], t[0])];
        if s.iter().all(|&v| v >= 0.0) || s.iter().all(|&v| v <= 0.0) {
            return 0.0;
        }
        (0..3)
            .map(|i| {
                let (a, b) = (t[i], t[(i + 1) % 3]);
                let k = ((p - a).dot(b - a) / (b - a).length_squared().max(1e-6)).clamp(0.0, 1.0);
                p.distance(a + (b - a) * k)
            })
            .fold(f32::MAX, f32::min)
    };
    let mut worst = (f32::MAX, 0.0);
    for k in 0..=1300 {
        let t = k as f32 * 0.01;
        let p = city::camera(t).position;
        let ground = vec2(p.x, p.z);
        let mut near = f32::MAX;
        for b in data
            .buildings
            .iter()
            .filter(|b| b.center.distance(ground) < 250.0)
        {
            for tri in b.roofs.iter().chain(&b.walls) {
                if tri.iter().map(|q| q.y).fold(0.0, f32::max) < p.y - 3.0 {
                    continue;
                }
                near = near.min(distance(ground, tri.map(|q| vec2(q.x, q.z))));
            }
        }
        if near < worst.0 {
            worst = (near, t);
        }
        if near < 12.0 {
            println!("t {t:5.2}: {near:4.1} m from a building at {p}");
        }
    }
    println!("closest: {:.1} m at {:.2} s", worst.0, worst.1);
    assert!(worst.0 > 6.0);
}

/// An overview of the Manhattan data, to check its frame and its triangles.
#[test]
#[ignore = "renders the raw data"]
fn nyc_overview() {
    use fframes_3d::*;
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("dynamic_media/nyc/manhattan.bin");
    let started = Instant::now();
    let bytes = std::fs::read(&path).unwrap();
    let data = city::data::Manhattan::load(&bytes).unwrap();
    println!("load {:.2} s", started.elapsed().as_secs_f32());
    let (mut up, mut down) = (0, 0);
    let mut positions = Vec::new();
    for b in &data.buildings {
        for t in b.roofs.iter().chain(&b.walls) {
            positions.extend_from_slice(t);
        }
        for t in &b.roofs {
            if (t[1] - t[0]).cross(t[2] - t[0]).y > 0.0 {
                up += 1;
            } else {
                down += 1;
            }
        }
    }
    println!(
        "roofs up {up} down {down}, buildings {}",
        data.buildings.len()
    );
    let mut scene = Scene::new()
        .environment(Environment::studio())
        .background(Background::Color(Rgb::hex(0x101820)));
    scene.add(Node::mesh(
        Geometry::from_triangles(&positions),
        Material::normals().double_sided(),
    ));
    let flat = |tris: &[[Vec2; 3]], y: f32| {
        let p: Vec<Vec3> = tris
            .iter()
            .flat_map(|t| t.map(|p| vec3(p.x, y, p.y)))
            .collect();
        Geometry::from_triangles(&p)
    };
    scene.add(Node::mesh(
        flat(&data.land, 0.0),
        Material::unlit(0x303438).double_sided(),
    ));
    scene.add(Node::mesh(
        flat(&data.parks, 0.1),
        Material::unlit(0x1d4a2a).double_sided(),
    ));
    let mut streets = Vec::new();
    for s in &data.streets {
        for w in s.points.windows(2) {
            let d = (w[1] - w[0]).normalize_or_zero();
            let side = vec2(-d.y, d.x) * s.width * 0.5;
            let c =
                [w[0] + side, w[1] + side, w[1] - side, w[0] - side].map(|p| vec3(p.x, 0.3, p.y));
            streets.extend_from_slice(&[c[0], c[1], c[2], c[0], c[2], c[3]]);
        }
    }
    scene.add(Node::mesh(
        Geometry::from_triangles(&streets),
        Material::unlit(0xc0a060).double_sided(),
    ));
    let renderer = city::CitySet::renderer(1920, 804).cpu().gpu();
    for (name, pos, target) in [
        ("nyc_top", vec3(0.0, 9000.0, 300.0), vec3(0.0, 0.0, 301.0)),
        ("nyc_esb", vec3(400.0, 350.0, -900.0), vec3(0.0, 200.0, 0.0)),
        (
            "nyc_north",
            vec3(117.0, 300.0, -240.0),
            vec3(300.0, 100.0, 800.0),
        ),
    ] {
        let mut camera = look::lens(28.0)
            .position(pos)
            .look_at(target)
            .clip(1.0, 20000.0);
        if name == "nyc_top" {
            camera.up = Vec3::Z;
            camera = camera.fov(60.0);
        }
        let pixels = renderer.render_pixels(&scene, &camera);
        let image =
            RgbaImage::from_raw(pixels.width, pixels.height, pixels.to_straight_rgba()).unwrap();
        image.save(out_dir().join(format!("{name}.png"))).unwrap();
    }
}
