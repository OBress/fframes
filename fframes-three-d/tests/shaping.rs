//! Shapes from `shaping`, rendered: a fused SDF creature body, a subdivided cage, a skinned
//! tapered tentacle bent by its bones, an extruded SVG outline, a lumpy displaced rock with
//! box UVs. Checks that they render the same on the CPU and the GPU and saves the picture.

use fframes_three_d::shaping::{add_bone_chain, chain_segments, sdf};
use fframes_three_d::*;

#[allow(dead_code)]
mod common;

fn showcase() -> (Scene, Camera) {
    let mut scene = Scene::new().environment(Environment::studio()).background(Background::Color(Rgb::hex(0x202020)));
    scene.add(Node::mesh(Geometry::floor(12.0, 12.0), Material::standard(0x606060)));
    scene.add(Node::light(Light::sun(Rgb::WHITE, 2.0, vec3(3.0, 6.0, 4.0))));

    // A creature body: an ellipsoid torso fused with four limbs and a head.
    let field = |p: Vec3| {
        let mut d = sdf::ellipsoid(p, vec3(0.0, 0.6, 0.0), vec3(0.5, 0.28, 0.3));
        for (x, z) in [(-0.35f32, -0.2f32), (0.35, -0.2), (-0.35, 0.2), (0.35, 0.2)] {
            d = sdf::smooth_union(d, sdf::capsule(p, vec3(x * 0.8, 0.55, z), vec3(x * 1.6, 0.0, z * 1.8), 0.08, 0.04), 0.08);
        }
        sdf::smooth_union(d, sdf::sphere(p, vec3(0.55, 0.8, 0.0), 0.18), 0.1)
    };
    let body = Geometry::from_sdf(field, Aabb::from_points([vec3(-0.9, -0.05, -0.8), vec3(0.9, 1.1, 0.8)]), 0.03);
    scene.add(Node::mesh(body, Material::standard(0x9a8a7a).roughness(0.6)).at(-3.0, 0.0, 0.0));

    // A box cage rounded by subdivision.
    let cage = Geometry::merge([Geometry::cuboid(0.8, 0.5, 0.5), Geometry::cuboid(0.3, 0.3, 0.3).translate(0.5, 0.2, 0.0)]);
    scene.add(Node::mesh(cage.subdivide(3), Material::standard(0x7a9aba).roughness(0.4)).at(-1.4, 0.4, 0.0));

    // A tentacle: tapered tube skinned to a chain, the chain curled.
    let root = scene.add(Node::group().at(0.0, 0.0, 0.0));
    let joints = add_bone_chain(&mut scene, root, 8, 2.0);
    let tube: Vec<Vec3> = (0..=40).map(|i| Vec3::Y * 2.0 * i as f32 / 40.0).collect();
    let geometry = Geometry::tube_with(&tube, |s| 0.12 * (1.0 - s) + 0.01, 12).skin_to_segments(&chain_segments(8, 2.0), 2);
    let mut node = Node::mesh(geometry, Material::standard(0x8a3a4a).roughness(0.5));
    node.mesh_mut().unwrap().skin = Some(Skin::from_rest(&scene, joints.clone(), Mat4::IDENTITY));
    scene.add_to(root, node);
    for (i, j) in joints.iter().enumerate() {
        scene[*j].rotation = Quat::from_rotation_z(-0.22 - i as f32 * 0.02);
    }

    // An extruded SVG outline: a bat-wing sign.
    let shape = Shape::from_svg_path("M0 20 Q10 0 20 10 Q30 0 40 10 Q50 0 60 20 Q50 15 40 25 Q30 15 20 25 Q10 15 0 20 Z", 0.02, 6).unwrap();
    scene.add(Node::mesh(Geometry::extrude_shape(&shape, 0.06, 0.01, 2), Material::standard(0xc0b080)).at(1.2, 1.2, 0.0));

    // A rock: an icosphere displaced, bent and twisted, with box UVs on a checker.
    let rock = Geometry::icosahedron(0.45, 3)
        .displace(|p, _| 0.08 * ((p.x * 9.0).sin() * (p.y * 7.0).cos() + (p.z * 11.0).sin()))
        .twist(0.8)
        .box_uvs(0.25);
    let checker = Texture::from_fn(64, 64, |u, v| if ((u * 8.0) as u32 + (v * 8.0) as u32) % 2 == 0 { Rgb::hex(0x888888) } else { Rgb::hex(0x444444) });
    scene.add(Node::mesh(rock, Material::standard(Rgb::WHITE).map(checker)).at(3.2, 0.45, 0.0));

    let camera = Camera::perspective(40.0).at(0.0, 2.2, 7.5).look_at(vec3(0.0, 0.7, 0.0));
    (scene, camera)
}

#[test]
fn shaped_geometry_renders_the_same_on_cpu_and_gpu() {
    let (scene, camera) = showcase();
    let cpu = Renderer::new(960, 540).cpu().render_pixels(&scene, &camera);
    common::save("shaping_cpu", &cpu);
    let gpu = Renderer::new(960, 540).gpu();
    if gpu.uses_gpu() {
        let gpu = gpu.render_pixels(&scene, &camera);
        common::save("shaping_gpu", &gpu);
        let d: f64 = cpu.rgba.iter().zip(&gpu.rgba).map(|(a, b)| f64::from(a.abs_diff(*b))).sum();
        let mean = (d / cpu.rgba.len() as f64) as f32;
        eprintln!("shaping cpu vs gpu: {mean:.2} / 255");
        assert!(mean < 6.0);
    }
}
