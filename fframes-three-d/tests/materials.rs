//! The rim (fresnel) glow, edge fade and shadow-only materials, on the CPU and the GPU.

use fframes_three_d::*;

#[allow(dead_code)]
mod common;

fn lum(p: &Pixels, x: u32, y: u32) -> f32 {
    let i = ((y * p.width + x) * 4) as usize;
    (0.2126 * f32::from(p.rgba[i]) + 0.7152 * f32::from(p.rgba[i + 1]) + 0.0722 * f32::from(p.rgba[i + 2])) / 255.0
}

fn mean_diff(a: &Pixels, b: &Pixels) -> f32 {
    let d: f64 = a.rgba.iter().zip(&b.rgba).map(|(x, y)| f64::from(x.abs_diff(*y))).sum();
    (d / a.rgba.len() as f64) as f32
}

fn renderers() -> Vec<Renderer> {
    let mut r = vec![Renderer::new(200, 200).cpu()];
    let gpu = Renderer::new(200, 200).gpu();
    if gpu.uses_gpu() {
        r.push(gpu);
    }
    r
}

#[test]
fn a_rim_glows_at_the_silhouette() {
    let mut images = Vec::new();
    for renderer in renderers() {
        let mut scene = Scene::new().background(Background::Color(Rgb::BLACK));
        // An unlit black ball with a white rim: dark in the middle, bright at the edge.
        scene.add(Node::mesh(Geometry::sphere(1.0, 48, 32), Material::unlit(0x000000).rim(Rgb::WHITE, 3.0)));
        // A lit ball with a red rim beside it.
        scene.add(Node::mesh(Geometry::sphere(0.5, 48, 32), Material::standard(0x404040).rim(Rgb::hex(0xff0000), 2.0)).at(1.6, 0.0, 0.0));
        scene.add(Node::light(Light::directional(Rgb::WHITE, 2.0, vec3(0.0, 0.0, 5.0))));
        let camera = Camera::perspective(40.0).at(0.0, 0.0, 6.0).look_at(vec3(0.4, 0.0, 0.0));
        let p = renderer.render_pixels(&scene, &camera);
        common::save(&format!("rim_{}", if renderer.uses_gpu() { "gpu" } else { "cpu" }), &p);
        // The middle of the big ball, and a point just inside its silhouette.
        let (center_x, edge_x) = (82, 38);
        let (center, edge) = (lum(&p, center_x, 100), lum(&p, edge_x, 100));
        eprintln!("gpu {}: rim center {center:.3}, edge {edge:.3}", renderer.uses_gpu());
        assert!(center < 0.05 && edge > 0.2);
        images.push(p);
    }
    if let [cpu, gpu] = &images[..] {
        let d = mean_diff(cpu, gpu);
        eprintln!("rim cpu vs gpu: {d:.2} / 255");
        assert!(d < 3.0);
    }
}

#[test]
fn a_shadow_only_mesh_casts_its_shadow_unseen() {
    let mut images = Vec::new();
    for renderer in renderers() {
        let build = |cube: Option<Material>| {
            let mut scene = Scene::new().background(Background::Color(Rgb::BLACK));
            scene.add(Node::mesh(Geometry::floor(8.0, 8.0), Material::standard(0xbbbbbb).roughness(1.0)));
            if let Some(m) = cube {
                scene.add(Node::mesh(Geometry::cuboid(0.8, 0.8, 0.8), m).at(0.0, 1.2, 0.0));
            }
            // A flashlight from above, slanted, so the shadow lands beside the cube.
            let spot = Light::spot(Rgb::WHITE, 80.0, deg(45.0), 0.2).pointing(vec3(0.5, -1.0, 0.0)).with_shadow(Shadow::default());
            scene.add(Node::light(spot).at(-1.5, 3.5, 0.0));
            scene
        };
        let camera = Camera::perspective(50.0).at(0.0, 6.0, 0.001).look_at(Vec3::ZERO);
        let empty = renderer.render_pixels(&build(None), &camera);
        let ghost = renderer.render_pixels(&build(Some(Material::standard(0xff0000).shadow_only())), &camera);
        let gpu = renderer.uses_gpu();
        common::save(&format!("shadow_only_{}", if gpu { "gpu" } else { "cpu" }), &ghost);
        // Straight down onto the cube: floor, not red, the same as with no cube there.
        let i = ((100 * 200 + 100) * 4) as usize;
        let (r, g) = (ghost.rgba[i], ghost.rgba[i + 1]);
        eprintln!("gpu {gpu}: under the cube {r} {g}; shadow {:.3} vs {:.3}", lum(&ghost, 140, 100), lum(&empty, 140, 100));
        assert!(r.abs_diff(g) < 8, "gpu {gpu}: the cube is drawn");
        // Its shadow falls on the floor beside it, away from the light.
        assert!(lum(&ghost, 140, 100) < lum(&empty, 140, 100) * 0.4, "gpu {gpu}: no shadow");
        images.push(ghost);
    }
    if let [cpu, gpu] = &images[..] {
        let d = mean_diff(cpu, gpu);
        eprintln!("shadow only cpu vs gpu: {d:.2} / 255");
        assert!(d < 3.0);
    }
}

#[test]
fn edge_fade_softens_a_glowing_shell_toward_its_silhouette() {
    let mut images = Vec::new();
    for renderer in renderers() {
        let mut scene = Scene::new().background(Background::Color(Rgb::BLACK));
        // Two additive double-sided balls: left plain, right fading to its edge.
        let glow = Material::unlit(Rgb::BLACK).emissive(Rgb::WHITE, 0.12).additive().double_sided();
        scene.add(Node::mesh(Geometry::sphere(1.0, 48, 32), glow.clone()).at(-1.2, 0.0, 0.0));
        scene.add(Node::mesh(Geometry::sphere(1.0, 48, 32), glow.edge_fade(2.0)).at(1.2, 0.0, 0.0));
        let camera = Camera::perspective(40.0).at(0.0, 0.0, 7.0).look_at(Vec3::ZERO);
        let p = renderer.render_pixels(&scene, &camera);
        common::save(&format!("edge_fade_{}", if renderer.uses_gpu() { "gpu" } else { "cpu" }), &p);
        // Centers and points just inside each silhouette (39 px to a meter).
        let (plain_c, plain_e) = (lum(&p, 53, 100), lum(&p, 20, 100));
        let (fade_c, fade_e) = (lum(&p, 147, 100), lum(&p, 180, 100));
        eprintln!("gpu {}: plain {plain_c:.3} / {plain_e:.3}, faded {fade_c:.3} / {fade_e:.3}", renderer.uses_gpu());
        // The plain shell is as bright or brighter at its edge; the faded one dims there.
        assert!(plain_e >= plain_c * 0.9, "plain {plain_c} {plain_e}");
        assert!(fade_e < fade_c * 0.7, "faded {fade_c} {fade_e}");
        assert!((fade_c - plain_c).abs() < 0.08, "the middle faces the camera: {fade_c} vs {plain_c}");
        images.push(p);
    }
    if let [cpu, gpu] = &images[..] {
        let d = mean_diff(cpu, gpu);
        eprintln!("edge fade cpu vs gpu: {d:.2} / 255");
        assert!(d < 1.0, "{d}");
    }
}
