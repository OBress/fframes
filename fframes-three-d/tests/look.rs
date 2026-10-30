//! The look settings of the renderer: ambient occlusion, AgX, the soft bloom, height fog,
//! and the low-fi vertex snapping and affine textures, on the CPU and the GPU.

use std::sync::Arc;

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

fn mean_lum(p: &Pixels) -> f32 {
    let mut sum = 0.0;
    for y in 0..p.height {
        for x in 0..p.width {
            sum += lum(p, x, y);
        }
    }
    sum / (p.width * p.height) as f32
}

fn renderers(width: u32, height: u32) -> Vec<Renderer> {
    let mut r = vec![Renderer::new(width, height).cpu()];
    let gpu = Renderer::new(width, height).gpu();
    if gpu.uses_gpu() {
        r.push(gpu);
    }
    r
}

fn name(renderer: &Renderer) -> &'static str {
    if renderer.uses_gpu() { "gpu" } else { "cpu" }
}

/// The inside corner of a room lit evenly, seen from the room.
fn corner() -> (Scene, Camera) {
    let mut scene = Scene::new().background(Background::Color(Rgb::BLACK));
    let wall = Material::standard(0xb0b0b0).roughness(1.0);
    scene.add(Node::mesh(Geometry::floor(6.0, 6.0), wall.clone()));
    scene.add(Node::mesh(Geometry::cuboid(6.0, 3.0, 0.1), wall.clone()).at(0.0, 1.5, -3.0));
    scene.add(Node::mesh(Geometry::cuboid(0.1, 3.0, 6.0), wall.clone()).at(-3.0, 1.5, 0.0));
    // A box on the floor, for the contact shade under it.
    scene.add(Node::mesh(Geometry::cuboid(0.6, 0.6, 0.6), wall).at(0.5, 0.3, -1.5));
    scene.add(Node::light(Light::ambient(Rgb::WHITE, 0.8)));
    let camera = Camera::perspective(60.0).at(1.5, 1.6, 2.5).look_at(vec3(-1.5, 0.4, -2.0));
    (scene, camera)
}

#[test]
fn ambient_occlusion_darkens_corners_and_leaves_open_surfaces() {
    let mut images = Vec::new();
    for base in renderers(240, 160) {
        let (scene, camera) = corner();
        let plain = base.clone().render_pixels(&scene, &camera);
        let ao = base.clone().ambient_occlusion(AmbientOcclusion { strength: 1.0, radius: 0.6 });
        let shaded = ao.render_pixels(&scene, &camera);
        common::save(&format!("ao_{}", name(&base)), &shaded);
        // Where the three planes meet, and an open stretch of floor.
        let project = |p: Vec3| {
            let s = camera.project(p, 240.0, 160.0).unwrap();
            (s.x.clamp(0.0, 239.0) as u32, s.y.clamp(0.0, 159.0) as u32)
        };
        let (cx, cy) = project(vec3(-2.9, 0.05, -2.9));
        let (ox, oy) = project(vec3(0.8, 0.0, 0.2));
        let (bx, by) = project(vec3(0.5, 0.02, -1.15));
        let corner = (lum(&plain, cx, cy), lum(&shaded, cx, cy));
        let open = (lum(&plain, ox, oy), lum(&shaded, ox, oy));
        let contact = (lum(&plain, bx, by), lum(&shaded, bx, by));
        eprintln!("{}: corner {corner:?}, open {open:?}, under the box {contact:?}", name(&base));
        assert!(corner.1 < corner.0 * 0.9, "the corner darkens");
        assert!(contact.1 < contact.0 * 0.9, "the floor next to the box darkens");
        assert!((open.1 - open.0).abs() < 0.02, "open floor keeps its light");
        images.push(shaded);
    }
    if let [cpu, gpu] = &images[..] {
        let d = mean_diff(cpu, gpu);
        eprintln!("ao cpu vs gpu: {d:.2} / 255");
        assert!(d < 3.0);
    }
}

#[test]
fn agx_rolls_bright_light_off_instead_of_clipping() {
    let mut images = Vec::new();
    for base in renderers(200, 40) {
        // Five patches of orange light, each four times brighter than the last.
        let mut scene = Scene::new().background(Background::Color(Rgb::BLACK));
        for i in 0..5 {
            let intensity = 0.25 * 4f32.powi(i);
            scene.add(Node::mesh(Geometry::plane(0.9, 0.9), Material::unlit(0x000000).emissive(0xff8020, intensity)).at(-2.0 + i as f32, 0.0, 0.0));
        }
        let camera = Camera::orthographic(1.0).at(0.0, 0.0, 5.0).look_at(Vec3::ZERO);
        let agx = base.clone().tone_mapping(ToneMapping::Agx).render_pixels(&scene, &camera);
        let aces = base.clone().tone_mapping(ToneMapping::Aces).render_pixels(&scene, &camera);
        common::save(&format!("agx_{}", name(&base)), &agx);
        let centers: Vec<u32> = (0..5).map(|i| 20 + 40 * i).collect();
        let px = |p: &Pixels, x: u32| p.get(x, 20);
        let agx_l: Vec<f32> = centers.iter().map(|&x| lum(&agx, x, 20)).collect();
        eprintln!("{} agx {:?} / aces {:?}", name(&base), centers.iter().map(|&x| px(&agx, x)).collect::<Vec<_>>(), centers.iter().map(|&x| px(&aces, x)).collect::<Vec<_>>());
        // Each step stays brighter than the last (no flat clip)...
        for w in agx_l.windows(2) {
            assert!(w[1] > w[0] + 0.01, "{agx_l:?}");
        }
        // ...and the brightest goes toward white: its blue rises, while the dimmest stays
        // orange.
        let (dim, bright) = (px(&agx, centers[0]), px(&agx, centers[4]));
        assert!(dim[0] > dim[2] + 40);
        assert!(bright[2] > 150);
        images.push(agx);
    }
    if let [cpu, gpu] = &images[..] {
        let d = mean_diff(cpu, gpu);
        eprintln!("agx cpu vs gpu: {d:.2} / 255");
        assert!(d < 1.0);
    }
}

#[test]
fn soft_bloom_spreads_light_without_adding_any() {
    let mut images = Vec::new();
    for base in renderers(200, 200) {
        // A small bright square on grey, mapped linearly so nothing clips.
        let mut scene = Scene::new().background(Background::Color(Rgb::linear(0.1, 0.1, 0.1)));
        scene.add(Node::mesh(Geometry::plane(0.4, 0.4), Material::unlit(0xffffff).emissive(0xffffff, 0.0)).at(0.0, 0.0, 0.0));
        let camera = Camera::orthographic(4.0).at(0.0, 0.0, 5.0).look_at(Vec3::ZERO);
        let linear = base.clone().tone_mapping(ToneMapping::Linear);
        let plain = linear.clone().render_pixels(&scene, &camera);
        let soft = linear.soft_bloom(0.3, 0.6).render_pixels(&scene, &camera);
        common::save(&format!("soft_bloom_{}", name(&base)), &soft);
        // sRGB means are not linear light, but with this little spread they move together.
        let (before, after) = (mean_lum(&plain), mean_lum(&soft));
        // Next to the square it glows; the square itself gives some light away.
        let (near_before, near_after) = (lum(&plain, 100, 115), lum(&soft, 100, 115));
        eprintln!("{}: mean {before:.4} -> {after:.4}, next to it {near_before:.3} -> {near_after:.3}", name(&base));
        assert!(near_after > near_before + 0.02);
        assert!(lum(&soft, 100, 100) < lum(&plain, 100, 100));
        assert!((after - before).abs() < 0.01 * before.max(0.1) + 0.004);
        images.push(soft);
    }
    if let [cpu, gpu] = &images[..] {
        let d = mean_diff(cpu, gpu);
        eprintln!("soft bloom cpu vs gpu: {d:.2} / 255");
        assert!(d < 2.0);
    }
}

#[test]
fn height_fog_lies_on_the_ground() {
    let mut images = Vec::new();
    for base in renderers(160, 160) {
        let mut scene = Scene::new().background(Background::Color(Rgb::BLACK)).fog(Fog::Height {
            color: Rgb::hex(0x8090a0),
            density: 0.0,
            ground: 0.6,
            height: 0.0,
            falloff: 2.0,
        });
        // A dark wall 8 m away, from the ground up to 4 m.
        scene.add(Node::mesh(Geometry::plane(8.0, 4.0), Material::unlit(0x101010)).at(0.0, 2.0, -8.0));
        let camera = Camera::perspective(40.0).at(0.0, 2.0, 0.0).look_at(vec3(0.0, 2.0, -8.0));
        let p = base.render_pixels(&scene, &camera);
        common::save(&format!("height_fog_{}", name(&base)), &p);
        // The bottom of the wall (0.4 m up) is in thick fog, the top (3.7 m) in almost none.
        let (low, high) = (lum(&p, 80, 124), lum(&p, 80, 36));
        eprintln!("{}: low {low:.3}, high {high:.3}", name(&base));
        assert!(low > high + 0.15);
        images.push(p);
    }
    if let [cpu, gpu] = &images[..] {
        let d = mean_diff(cpu, gpu);
        eprintln!("height fog cpu vs gpu: {d:.2} / 255");
        assert!(d < 1.5);
    }
}

#[test]
fn low_fi_snaps_vertices_and_maps_textures_affinely() {
    let gpu = Renderer::new(160, 120).gpu();
    if !gpu.uses_gpu() {
        return;
    }
    // A checkered floor running away from the camera: perspective-correct lines converge,
    // affine ones bend.
    let mut rgba = Vec::new();
    for y in 0..64 {
        for x in 0..64 {
            let on = ((x / 8) + (y / 8)) % 2 == 0;
            rgba.extend_from_slice(if on { &[230, 230, 230, 255] } else { &[30, 30, 30, 255] });
        }
    }
    let checker = Arc::new(Texture::from_rgba8(64, 64, &rgba, true));
    let mut scene = Scene::new().background(Background::Color(Rgb::BLACK));
    let mut floor = Material::unlit(0xffffff);
    floor.map = Some(checker);
    scene.add(Node::mesh(Geometry::floor(4.0, 20.0), floor).at(0.0, 0.0, -8.0));
    let camera = Camera::perspective(60.0).at(0.0, 1.0, 1.0).look_at(vec3(0.0, 0.0, -6.0));
    let plain = gpu.clone().render_pixels(&scene, &camera);
    let affine = gpu.clone().low_fi(0.0, 1.0).render_pixels(&scene, &camera);
    let snapped = gpu.clone().low_fi(8.0, 0.0).render_pixels(&scene, &camera);
    common::save("low_fi_plain", &plain);
    common::save("low_fi_affine", &affine);
    common::save("low_fi_snapped", &snapped);
    let (a, s) = (mean_diff(&plain, &affine), mean_diff(&plain, &snapped));
    eprintln!("affine {a:.2}, snapped {s:.2} / 255");
    assert!(a > 5.0, "affine mapping changes the floor");
    assert!(s > 0.5, "snapping moves the edges");
    // The CPU ignores both.
    let cpu = Renderer::new(160, 120).cpu();
    let d = mean_diff(&cpu.clone().render_pixels(&scene, &camera), &cpu.low_fi(8.0, 1.0).render_pixels(&scene, &camera));
    assert!(d < 0.01);
}
