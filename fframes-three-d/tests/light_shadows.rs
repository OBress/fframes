//! Shadows of spot and point lights: a flashlight throws the shadow of what it lights, a bare
//! bulb throws shadows on every side, and the CPU and the GPU agree.

use fframes_three_d::*;

#[allow(dead_code)]
mod common;

/// The luminance of the pixel at `(x, y)`, `0..1`.
fn lum(p: &Pixels, x: u32, y: u32) -> f32 {
    let i = ((y * p.width + x) * 4) as usize;
    (0.2126 * f32::from(p.rgba[i]) + 0.7152 * f32::from(p.rgba[i + 1]) + 0.0722 * f32::from(p.rgba[i + 2])) / 255.0
}

/// The mean difference of two images in 8-bit steps.
fn mean_diff(a: &Pixels, b: &Pixels) -> f32 {
    let d: f64 = a.rgba.iter().zip(&b.rgba).map(|(x, y)| f64::from(x.abs_diff(*y))).sum();
    (d / a.rgba.len() as f64) as f32
}

fn renderers() -> Vec<Renderer> {
    let mut r = vec![Renderer::new(240, 240).cpu()];
    let gpu = Renderer::new(240, 240).gpu();
    if gpu.uses_gpu() {
        r.push(gpu);
    }
    r
}

/// A grey floor and a cube 1 m over it, seen from straight above.
fn stage(light: Light, at: Vec3) -> (Scene, Camera) {
    let mut scene = Scene::new().background(Background::Color(Rgb::BLACK));
    scene.add(Node::mesh(Geometry::floor(8.0, 8.0), Material::standard(0xbbbbbb).roughness(1.0)));
    scene.add(Node::mesh(Geometry::cuboid(0.6, 0.6, 0.6), Material::standard(0x444444)).at(0.0, 1.3, 0.0));
    scene.add(Node::light(light).at(at.x, at.y, at.z));
    let camera = Camera::perspective(50.0).at(0.0, 6.0, 0.001).look_at(Vec3::ZERO);
    (scene, camera)
}

#[test]
fn a_spot_light_casts_a_shadow() {
    let spot = Light::spot(Rgb::WHITE, 60.0, deg(40.0), 0.3).pointing(Vec3::NEG_Y);
    let mut images = Vec::new();
    for renderer in renderers() {
        let (plain, camera) = stage(spot.clone(), vec3(0.0, 3.0, 0.0));
        let (shadowed, _) = stage(spot.clone().with_shadow(Shadow::default()), vec3(0.0, 3.0, 0.0));
        let without = renderer.render_pixels(&plain, &camera);
        let with = renderer.render_pixels(&shadowed, &camera);
        // Beside the cube's footprint on the floor, the floor is lit in both. The floor
        // straight under the cube is hidden from the camera by the cube, so look where the
        // shadow reaches past it: the light is above the cube, so its shadow is a bigger
        // square around the cube on the floor.
        let gpu = renderer.uses_gpu();
        let (lit_x, shadow_x) = (200, 97);
        eprintln!(
            "gpu {gpu}: under the shadow edge {:.3} without, {:.3} with; lit floor {:.3} / {:.3}",
            lum(&without, shadow_x, 120),
            lum(&with, shadow_x, 120),
            lum(&without, lit_x, 120),
            lum(&with, lit_x, 120)
        );
        common::save(&format!("spot_shadow_{}", if gpu { "gpu" } else { "cpu" }), &with);
        assert!(lum(&with, shadow_x, 120) < lum(&without, shadow_x, 120) * 0.3, "gpu {gpu}: no shadow");
        assert!((lum(&with, lit_x, 120) - lum(&without, lit_x, 120)).abs() < 0.03, "gpu {gpu}: lit floor changed");
        images.push(with);
    }
    if let [cpu, gpu] = &images[..] {
        let d = mean_diff(cpu, gpu);
        eprintln!("spot shadow cpu vs gpu: {d:.2} / 255");
        assert!(d < 3.0);
    }
}

#[test]
fn a_point_light_casts_shadows_all_around() {
    // A bulb in the middle of a room of four walls and a floor, with a post on each side:
    // each post's shadow falls on a different cube face's wall, and on the floor.
    let mut images = Vec::new();
    for renderer in renderers() {
        let build = |shadow: bool| {
            let mut scene = Scene::new().background(Background::Color(Rgb::BLACK));
            let wall = Material::standard(0xbbbbbb).roughness(1.0);
            scene.add(Node::mesh(Geometry::floor(8.0, 8.0), wall.clone()));
            for (x, z, turn) in [(0.0f32, -4.0f32, 0.0f32), (0.0, 4.0, 180.0), (-4.0, 0.0, 90.0), (4.0, 0.0, -90.0)] {
                scene.add(Node::mesh(Geometry::plane(8.0, 4.0), wall.clone()).at(x, 2.0, z).rotate_y(deg(turn)));
            }
            for (x, z) in [(1.5f32, 0.0f32), (-1.5, 0.0), (0.0, 1.5), (0.0, -1.5)] {
                scene.add(Node::mesh(Geometry::cuboid(0.3, 2.4, 0.3), Material::standard(0x333333)).at(x, 1.2, z));
            }
            let mut bulb = Light::point(Rgb::WHITE, 30.0, 0.0);
            if shadow {
                bulb = bulb.with_shadow(Shadow { map_size: 512, ..Default::default() });
            }
            scene.add(Node::light(bulb).at(0.0, 1.5, 0.0));
            scene
        };
        // Looking at the -Z wall from the room's middle, above the bulb: the post at
        // z = -1.5 throws a stripe of shadow up that wall.
        let camera = Camera::perspective(70.0).at(0.0, 3.4, 3.0).look_at(vec3(0.0, 1.5, -4.0));
        let without = renderer.render_pixels(&build(false), &camera);
        let with = renderer.render_pixels(&build(true), &camera);
        let gpu = renderer.uses_gpu();
        let name = if gpu { "gpu" } else { "cpu" };
        common::save(&format!("point_shadow_{name}"), &with);
        common::save(&format!("point_shadow_off_{name}"), &without);
        // The darkest column of the far wall's middle band compared with the band's mean.
        let row = 70;
        let band: Vec<f32> = (40..200).map(|x| lum(&with, x, row)).collect();
        let plain: Vec<f32> = (40..200).map(|x| lum(&without, x, row)).collect();
        let min = band.iter().copied().fold(f32::MAX, f32::min);
        let plain_min = plain.iter().copied().fold(f32::MAX, f32::min);
        eprintln!("gpu {gpu}: far wall darkest {min:.3} with shadows, {plain_min:.3} without");
        assert!(min < plain_min * 0.4, "gpu {gpu}: no shadow on the far wall");
        // And the floor, which is another cube face: the shadows of the side posts.
        // The floor in the bottom left corner, where the left post's shadow falls.
        let region = |p: &Pixels| (0..60).flat_map(|x| (215..240).map(move |y| (x, y))).map(|(x, y)| lum(p, x, y)).sum::<f32>();
        let (floor, floor_plain) = (region(&with), region(&without));
        eprintln!("gpu {gpu}: floor corner {floor:.1} with shadows, {floor_plain:.1} without");
        assert!(floor < floor_plain * 0.7, "gpu {gpu}: no shadow on the floor");
        images.push(with);
    }
    if let [cpu, gpu] = &images[..] {
        let d = mean_diff(cpu, gpu);
        eprintln!("point shadow cpu vs gpu: {d:.2} / 255");
        assert!(d < 3.0);
    }
}

#[test]
fn shadows_beyond_the_limit_are_dropped_the_same_on_both() {
    // Five spot lights with shadows: the first four shadow, the fifth lights through the
    // cube. Both renderers must agree.
    let mut images = Vec::new();
    for renderer in renderers() {
        let mut scene = Scene::new().background(Background::Color(Rgb::BLACK));
        scene.add(Node::mesh(Geometry::floor(12.0, 12.0), Material::standard(0xbbbbbb).roughness(1.0)));
        for i in 0..5 {
            let x = -4.0 + i as f32 * 2.0;
            scene.add(Node::mesh(Geometry::cuboid(0.5, 0.5, 0.5), Material::standard(0x444444)).at(x, 1.0, 0.0));
            let spot = Light::spot(Rgb::WHITE, 20.0, deg(30.0), 0.3).pointing(vec3(0.3, -1.0, 0.0)).with_shadow(Shadow::default());
            scene.add(Node::light(spot).at(x - 0.6, 3.0, 0.0));
        }
        let camera = Camera::perspective(60.0).at(0.0, 9.0, 0.001).look_at(Vec3::ZERO);
        images.push(renderer.render_pixels(&scene, &camera));
    }
    if let [cpu, gpu] = &images[..] {
        let d = mean_diff(cpu, gpu);
        eprintln!("five shadowed spots cpu vs gpu: {d:.2} / 255");
        assert!(d < 3.0);
    }
}

#[test]
fn many_lights_light_the_same_on_both() {
    // 40 point lights: past MAX_LIGHTS both renderers leave the same ones out.
    let mut images = Vec::new();
    for renderer in renderers() {
        let mut scene = Scene::new().background(Background::Color(Rgb::BLACK));
        scene.add(Node::mesh(Geometry::floor(20.0, 20.0), Material::standard(0xbbbbbb).roughness(1.0)));
        for i in 0..40 {
            let (x, z) = ((i % 8) as f32 * 2.0 - 7.0, (i / 8) as f32 * 2.0 - 4.0);
            scene.add(Node::light(Light::point(Rgb::hsl(i as f32 * 0.07, 0.8, 0.5), 3.0, 3.0)).at(x, 0.8, z));
        }
        let camera = Camera::perspective(60.0).at(0.0, 12.0, 0.001).look_at(Vec3::ZERO);
        images.push(renderer.render_pixels(&scene, &camera));
    }
    assert!(MAX_LIGHTS >= 32);
    if let [cpu, gpu] = &images[..] {
        let d = mean_diff(cpu, gpu);
        eprintln!("40 point lights cpu vs gpu: {d:.2} / 255");
        assert!(d < 3.0);
    }
}
