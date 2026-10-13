#![allow(clippy::unreadable_literal)]

use fframes_3d::*;

mod common;
use common::{cubes, hall, save, showcase};

#[test]
fn renders_showcase() {
    let (scene, camera) = showcase();
    let start = std::time::Instant::now();
    let pixels = Renderer::new(1920, 1080)
        .bloom(Bloom::default())
        .render_pixels(&scene, &camera);
    eprintln!("showcase 1080p: {:?}", start.elapsed());
    save("showcase", &pixels);
    // The gradient background is opaque.
    assert_eq!(pixels.get(5, 5)[3], 255);
}

#[test]
fn renders_text_with_holes() {
    let Ok(font) = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../examples/conference-splash-screen/media/Inter_18pt-ExtraBold.ttf"
    )) else {
        return;
    };
    let text = Geometry::text(&font, "fframes aeo8B", 1.0, 0.25, 0.03).unwrap();
    let b = text.bounds();
    let mut scene = Scene::new()
        .environment(Environment::studio())
        .background(Background::Color(Rgb::hex(0xf2f2f0)));
    scene.add(
        Node::mesh(
            text,
            Material::standard(0x141414).roughness(0.3).clearcoat(1.0),
        )
        .at(-b.center().x, 0.0, 0.0),
    );
    scene.add(Node::light(Light::sun(
        Rgb::WHITE,
        2.5,
        vec3(2.0, 5.0, 4.0),
    )));
    let camera = Camera::perspective(30.0)
        .at(0.0, 0.35, 5.5)
        .look_at(vec3(0.0, 0.35, 0.0));
    let pixels = Renderer::new(1600, 400).render_pixels(&scene, &camera);
    save("text", &pixels);
    assert!(b.size().x > 5.0);
}

#[test]
fn renders_depth_of_field() {
    let (scene, camera) = showcase();
    let camera = camera.focus_on(vec3(-0.8, 0.6, 0.6), 14.0);
    let start = std::time::Instant::now();
    let pixels = Renderer::new(1920, 1080).render_pixels(&scene, &camera);
    eprintln!("depth of field 1080p: {:?}", start.elapsed());
    save("depth_of_field", &pixels);
}

/// Renders an STL file into `stl.png` with the lighting of the showcase. Run it with
/// `FFRAMES_3D_STL=model.stl cargo test -p fframes-3d --test render renders_an_stl -- --nocapture`.
#[test]
fn renders_an_stl() {
    let Ok(path) = std::env::var("FFRAMES_3D_STL") else {
        return;
    };
    let bytes = std::fs::read(&path).unwrap();
    let geometry = Geometry::from_stl(&bytes).unwrap().smooth(40.0);
    // Fits the model into a 1.6 m box on the floor, for all units and up axes.
    let bounds = geometry.bounds();
    let size = bounds.size().max_element();
    let geometry = geometry
        .translate(-bounds.center().x, -bounds.min.y, -bounds.center().z)
        .scale(1.6 / size, 1.6 / size, 1.6 / size);
    eprintln!(
        "{path}: {} triangles, {} vertices after smoothing",
        geometry.triangle_count(),
        geometry.vertex_count()
    );
    let mut scene = Scene::new()
        .environment(Environment::studio())
        .reflector(Reflector::floor());
    scene.add(Node::mesh(
        Geometry::floor(20.0, 20.0),
        Material::standard(0x2a2c30).roughness(0.35).reflection(0.6),
    ));
    scene.add(Node::mesh(
        geometry,
        Material::standard(0xd8d2c4).roughness(0.4).clearcoat(0.6),
    ));
    scene.add(Node::light(Light::sun(
        Rgb::WHITE,
        2.5,
        vec3(2.0, 5.0, 3.0),
    )));
    let camera = Camera::perspective(32.0)
        .at(2.2, 1.6, 3.0)
        .look_at(vec3(0.0, 0.7, 0.0));
    let pixels = Renderer::new(960, 540)
        .bloom(Bloom::default())
        .render_pixels(&scene, &camera);
    save("stl", &pixels);
}

#[test]
fn instances_render_like_separate_meshes() {
    let renderer = Renderer::new(480, 270).cpu();
    let (instanced, camera) = cubes(true);
    let (separate, _) = cubes(false);
    let a = renderer.render_pixels(&instanced, &camera);
    let b = renderer.render_pixels(&separate, &camera);
    save("instances", &a);
    let diff = a
        .rgba
        .iter()
        .zip(&b.rgba)
        .map(|(x, y)| f64::from(x.abs_diff(*y)))
        .sum::<f64>()
        / a.rgba.len() as f64;
    eprintln!("instances against separate meshes: mean difference {diff:.3} / 255");
    assert!(diff < 0.1, "{diff}");
}

#[test]
fn renders_atmosphere() {
    let renderer = Renderer::new(960, 540).cpu();
    let (scene, camera) = hall();
    let hazy = renderer.render_pixels(&scene, &camera);
    let mut clear = scene;
    clear.atmosphere = None;
    let clear = renderer.render_pixels(&clear, &camera);
    save("atmosphere", &hazy);
    let mean = |p: &Pixels| p.rgba.iter().map(|&v| f64::from(v)).sum::<f64>() / p.rgba.len() as f64;
    let (hazy, clear) = (mean(&hazy), mean(&clear));
    eprintln!("mean brightness: {hazy:.1} with haze, {clear:.1} without");
    // the air scatters the light of the sun and the lamps into the dark hall
    assert!(hazy > clear + 5.0);
}
