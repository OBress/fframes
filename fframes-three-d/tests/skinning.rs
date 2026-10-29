//! Several copies of one skinned model in a scene, each posed by its own clips, and
//! crossfades between clips: what a story with more than one character needs.

use std::sync::Arc;

use fframes_three_d::*;

/// A model with one joint, `Bone`, and a 1 m quad skinned to it, and two clips that move
/// the joint 1 m in one second: `Up` along +Y and `Side` along +X.
fn bone_model() -> Model {
    let mut scene = Scene::new();
    let bone = scene.add(Node::group().named("Bone"));
    let mut quad = Geometry::plane(1.0, 1.0);
    let n = quad.positions.len();
    quad.skin = Some(SkinWeights {
        joints: vec![[0; 4]; n],
        weights: vec![[1.0, 0.0, 0.0, 0.0]; n],
    });
    let mesh = scene.add(Node::mesh(quad, Material::unlit(0xffffff)));
    scene[mesh].mesh_mut().unwrap().skin = Some(Skin {
        joints: vec![bone],
        inverse_bind_matrices: Arc::new(vec![Mat4::IDENTITY]),
    });
    let clip = |name: &str, to: Vec3| AnimationClip {
        name: name.into(),
        duration: 1.0,
        channels: vec![Channel {
            node: bone,
            interpolation: Interpolation::Linear,
            times: vec![0.0, 1.0],
            keyframes: Keyframes::Translation(vec![Vec3::ZERO, to]),
        }],
    };
    Model {
        scene,
        animations: vec![clip("Up", Vec3::Y), clip("Side", Vec3::X)],
    }
}

/// The center of the white pixels in the left or right half of the image, in meters, for
/// the orthographic camera of [`skinned_copies_deform_independently`].
fn center(pixels: &Pixels, right: bool) -> Vec2 {
    let (w, h) = (pixels.width as usize, pixels.height as usize);
    let (mut sum, mut count) = (Vec2::ZERO, 0.0);
    for y in 0..h {
        for x in if right { w / 2..w } else { 0..w / 2 } {
            if pixels.rgba[(y * w + x) * 4] > 128 {
                sum += vec2(x as f32 + 0.5, y as f32 + 0.5);
                count += 1.0;
            }
        }
    }
    assert!(count > 0.0, "nothing drawn in the {} half", if right { "right" } else { "left" });
    let px = sum / count;
    // 8 x 4 meters on screen
    vec2(px.x / w as f32 * 8.0 - 4.0, 2.0 - px.y / h as f32 * 4.0)
}

#[test]
fn copies_of_a_model_animate_independently() {
    let model = bone_model();
    let (up, side) = (model.animation("Up").unwrap(), model.animation("Side").unwrap());
    let mut scene = Scene::new();
    let a = model.add_to(&mut scene, None);
    let b = model.add_to(&mut scene, None);
    let (bone_a, bone_b) = (a.find(&scene, "Bone").unwrap(), b.find(&scene, "Bone").unwrap());

    // The second copy's skin points to its own joint.
    let skin_of = |instance: &ModelInstance| {
        let mesh = (0..instance.len)
            .map(|i| NodeId(instance.offset + i))
            .find(|id| scene[*id].as_mesh().is_some())
            .unwrap();
        scene[mesh].as_mesh().unwrap().skin.clone().unwrap().joints
    };
    assert_eq!(skin_of(&a), vec![bone_a]);
    assert_eq!(skin_of(&b), vec![bone_b]);

    a.play(&mut scene, up, 0.5);
    b.play(&mut scene, side, 0.25);
    assert!((scene[bone_a].position - vec3(0.0, 0.5, 0.0)).length() < 1e-6);
    assert!((scene[bone_b].position - vec3(0.25, 0.0, 0.0)).length() < 1e-6);

    // A crossfade: the first clip at full weight, the second over it at the fade's progress.
    a.play(&mut scene, up, 0.5);
    a.blend(&mut scene, side, 0.5, 0.25);
    assert!((scene[bone_a].position - vec3(0.125, 0.375, 0.0)).length() < 1e-6);
    // `play` loops: the end of a clip is its start.
    a.play(&mut scene, up, 1.0);
    assert!(scene[bone_a].position.length() < 1e-6);
    // The other copy keeps its pose.
    assert!((scene[bone_b].position - vec3(0.25, 0.0, 0.0)).length() < 1e-6);
}

/// Two copies side by side, one halfway through `Up`, the other halfway through a crossfade
/// from `Up` to `Side`: each quad follows its own joint, on the CPU and on the GPU.
#[test]
fn skinned_copies_deform_independently() {
    let model = bone_model();
    let (up, side) = (model.animation("Up").unwrap(), model.animation("Side").unwrap());
    let mut scene = Scene::new().background(Background::Color(Rgb::hex(0x000000)));
    let a = model.add_to(&mut scene, None);
    let b = model.add_to(&mut scene, None);
    scene[a.root].position = vec3(-2.0, 0.0, 0.0);
    scene[b.root].position = vec3(2.0, -1.0, 0.0);
    a.play(&mut scene, up, 0.5);
    b.play(&mut scene, up, 0.5);
    b.blend(&mut scene, side, 0.5, 0.5);

    let camera = Camera::orthographic(4.0).at(0.0, 0.0, 5.0).look_at(Vec3::ZERO);
    let mut renderers = vec![Renderer::new(400, 200).cpu()];
    let gpu = Renderer::new(400, 200).gpu();
    if gpu.uses_gpu() {
        renderers.push(gpu);
    }
    for renderer in renderers {
        let pixels = renderer.render_pixels(&scene, &camera);
        let (left, right) = (center(&pixels, false), center(&pixels, true));
        let gpu = renderer.uses_gpu();
        eprintln!("gpu {gpu}: quads at {left} and {right}");
        assert!((left - vec2(-2.0, 0.5)).length() < 0.05, "gpu {gpu}: left quad at {left}");
        assert!((right - vec2(2.25, -0.75)).length() < 0.05, "gpu {gpu}: right quad at {right}");
    }
}
