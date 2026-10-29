//! A mirror on a wall (a vertical reflector), on the CPU and the GPU.

use fframes_three_d::*;

#[allow(dead_code)]
mod common;

fn renderers() -> Vec<Renderer> {
    let mut r = vec![Renderer::new(160, 120).cpu()];
    let gpu = Renderer::new(160, 120).gpu();
    if gpu.uses_gpu() {
        r.push(gpu);
    }
    r
}

fn mean_diff(a: &Pixels, b: &Pixels) -> f32 {
    let d: f64 = a.rgba.iter().zip(&b.rgba).map(|(x, y)| f64::from(x.abs_diff(*y))).sum();
    (d / a.rgba.len() as f64) as f32
}

#[test]
fn a_wall_mirror_shows_the_room_in_front_of_it() {
    let mut images = Vec::new();
    for renderer in renderers() {
        let mut scene = Scene::new().background(Background::Color(Rgb::BLACK)).reflector(Reflector::new(vec3(0.0, 1.0, -2.0), Vec3::Z));
        // A grey wall at z = -2.006, a mirror 6 mm in front of it, a red box behind the
        // camera's shoulder that only the mirror shows.
        scene.add(Node::mesh(Geometry::plane(6.0, 3.0), Material::standard(0x808080)).at(0.0, 1.0, -2.006));
        scene.add(Node::mesh(Geometry::plane(1.6, 1.2), Material::metal(0xd0d0d0, 0.02).reflection(1.0)).at(0.0, 1.0, -2.0));
        scene.add(Node::mesh(Geometry::cuboid(0.8, 0.8, 0.8), Material::unlit(0xff0000)).at(0.0, 1.0, 2.5));
        scene.add(Node::light(Light::ambient(Rgb::WHITE, 1.0)));
        let camera = Camera::perspective(50.0).at(0.0, 1.0, 1.0).look_at(vec3(0.0, 1.0, -2.0));
        let p = renderer.render_pixels(&scene, &camera);
        common::save(&format!("mirror_{}", if renderer.uses_gpu() { "gpu" } else { "cpu" }), &p);
        let i = ((60 * p.width + 80) * 4) as usize;
        let [r, g, b] = [p.rgba[i], p.rgba[i + 1], p.rgba[i + 2]];
        eprintln!("gpu {}: mirror center {r} {g} {b}", renderer.uses_gpu());
        assert!(r > 120 && g < 60 && b < 60, "gpu {}: the mirror shows {r} {g} {b}, not the red box", renderer.uses_gpu());
        images.push(p);
    }
    if let [cpu, gpu] = &images[..] {
        let d = mean_diff(cpu, gpu);
        eprintln!("mirror cpu vs gpu: {d:.2} / 255");
        assert!(d < 3.0, "{d}");
    }
}
