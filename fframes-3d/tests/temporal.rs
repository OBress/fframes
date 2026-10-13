//! A moving camera must change the frame smoothly: shadow maps that do not crawl, haze
//! that does not shimmer, hand-held shake that does not stop and start.
#![allow(clippy::unreadable_literal)]

use fframes_3d::*;

#[allow(dead_code)]
mod common;

/// A long hall with a roof of slats. The sun falls through the slots and makes regular
/// shafts in the haze. The camera at `z` looks down the hall below the roof, so that the
/// frame shows the shafts against the black far end.
fn shafts(z: f32, steps: u32) -> (Scene, Camera) {
    let mut scene = Scene::new()
        .background(Background::Color(Rgb::BLACK))
        .atmosphere(Atmosphere {
            density: 0.03,
            falloff: 0.0,
            anisotropy: 0.6,
            noise: 0.8,
            noise_size: 2.0,
            distance: 150.0,
            steps,
            ..Default::default()
        });
    let dark = Material::standard(0x101010).roughness(0.9);
    // the roof glazing of the film: a slot of 4 m every 12 m
    for i in 0..15 {
        let slat_z = 10.0 - i as f32 * 12.0;
        scene.add(Node::mesh(Geometry::cuboid(40.0, 0.3, 8.0), dark.clone()).at(0.0, 12.0, slat_z));
    }
    scene.add(Node::light(
        Light::directional(Rgb::WHITE, 4.0, vec3(0.4, 6.0, -2.5)).with_shadow(Shadow {
            map_size: 2048,
            bounds: Some(Aabb {
                min: vec3(-20.0, 0.0, -170.0),
                max: vec3(20.0, 12.5, 12.0),
            }),
            ..Default::default()
        }),
    ));
    let camera = Camera::perspective(40.0)
        .at(0.0, 5.0, z)
        .look_at(vec3(-30.0, 7.0, z - 8.0));
    (scene, camera)
}

/// The mean brightness of the blocks of 8 x 8 pixels. The rounding to 8 bits averages
/// out, the shimmer of the haze, which is as large as the filter of the haze, stays.
fn blocks(p: &Pixels) -> Vec<f64> {
    let (w, h) = (p.width as usize / 8, p.height as usize / 8);
    let mut out = vec![0.0; w * h];
    for y in 0..h * 8 {
        for x in 0..w * 8 {
            let i = (y * p.width as usize + x) * 4;
            let sum: f64 = p.rgba[i..i + 3].iter().map(|&v| f64::from(v)).sum();
            out[(y / 8) * w + x / 8] += sum / (3.0 * 64.0);
        }
    }
    out
}

/// The mean of `|a - 2b + c|`: the part of the change between frames that is not linear.
fn second_difference(a: &[f64], b: &[f64], c: &[f64]) -> f64 {
    let sum: f64 = a
        .iter()
        .zip(b)
        .zip(c)
        .map(|((a, b), c)| (a - 2.0 * b + c).abs())
        .sum();
    sum / a.len() as f64
}

/// The mean of `|a - b|`.
fn difference(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(a, b)| (a - b).abs()).sum::<f64>() / a.len() as f64
}

/// The second difference of the frames of a camera that moves down the hall by 4 cm per
/// frame (2.4 m/s at 60 fps).
fn haze_roughness(renderer: &Renderer, name: &str) -> (f64, f64) {
    let frames: Vec<Vec<f64>> = (0..6)
        .map(|i| {
            let (scene, camera) = shafts(-0.04 * i as f32, 32);
            let pixels = renderer.render_pixels(&scene, &camera);
            if i == 0 {
                common::save(name, &pixels);
            }
            blocks(&pixels)
        })
        .collect();
    let n = frames.len() - 2;
    let rough = frames
        .windows(3)
        .map(|w| second_difference(&w[0], &w[1], &w[2]))
        .sum::<f64>()
        / n as f64;
    let change = frames
        .windows(2)
        .map(|w| difference(&w[0], &w[1]))
        .sum::<f64>()
        / (frames.len() - 1) as f64;
    eprintln!("{name}: second difference {rough:.3}, change per frame {change:.3} / 255");
    (rough, change)
}

#[test]
fn haze_changes_smoothly_when_the_camera_moves() {
    let renderer = Renderer::new(480, 270).cpu();
    let (rough, change) = haze_roughness(&renderer, "temporal_haze_cpu");
    // a smooth change is nearly linear over a few frames
    assert!(change > 0.1, "{change}");
    assert!(rough < 0.5 * change, "{rough} {change}");
}

#[cfg(feature = "gpu")]
#[test]
fn gpu_haze_changes_smoothly_when_the_camera_moves() {
    let renderer = Renderer::new(480, 270);
    if !renderer.uses_gpu() {
        eprintln!("no GPU, skipped");
        return;
    }
    let (rough, change) = haze_roughness(&renderer, "temporal_haze_gpu");
    assert!(change > 0.1, "{change}");
    assert!(rough < 0.5 * change, "{rough} {change}");
}

/// A box on a floor in the sun, with the shadow bounds around the point `center`.
fn shadowed_box(center: Vec3) -> (Scene, Camera) {
    let mut scene = Scene::new().background(Background::Color(Rgb::hex(0x202020)));
    let grey = Material::standard(0x909090).roughness(0.8);
    scene.add(Node::mesh(Geometry::floor(20.0, 20.0), grey.clone()));
    scene.add(Node::mesh(Geometry::cuboid(1.0, 1.0, 1.0), grey).at(0.3, 0.5, 0.0));
    scene.add(Node::light(
        Light::directional(Rgb::WHITE, 3.0, vec3(-3.0, 2.5, 1.0)).with_shadow(Shadow {
            map_size: 256,
            bounds: Some(Aabb {
                min: center - vec3(6.0, 1.0, 6.0),
                max: center + vec3(6.0, 2.0, 6.0),
            }),
            ..Default::default()
        }),
    ));
    let camera = Camera::perspective(40.0)
        .at(3.0, 4.0, 5.0)
        .look_at(vec3(0.3, 0.0, 0.0));
    (scene, camera)
}

/// Moves the shadow box by a fraction of a texel, as a box that follows the camera does.
fn shadow_change(renderer: &Renderer) -> f64 {
    // the map is 256 texels over about 16 m: one texel is about 6 cm
    let (scene, camera) = shadowed_box(vec3(0.0, 0.0, 0.0));
    let a = renderer.render_pixels(&scene, &camera);
    let (scene, camera) = shadowed_box(vec3(0.013, 0.004, -0.009));
    let b = renderer.render_pixels(&scene, &camera);
    common::save("temporal_shadow", &a);
    // the shadow is in the frame
    let mut unshadowed = scene;
    for (id, _) in unshadowed.clone().nodes() {
        if let Object::Light(Light::Directional(light)) = &mut unshadowed[id].object {
            light.shadow = None;
        }
    }
    let unshadowed = renderer.render_pixels(&unshadowed, &camera);
    let shadow = difference(&blocks(&b), &blocks(&unshadowed));
    assert!(shadow > 1.0, "{shadow}");
    a.rgba
        .iter()
        .zip(&b.rgba)
        .map(|(a, b)| f64::from(a.abs_diff(*b)))
        .sum::<f64>()
        / a.rgba.len() as f64
}

#[test]
fn shadow_map_stays_still_when_its_box_moves() {
    let change = shadow_change(&Renderer::new(320, 180).cpu());
    eprintln!("cpu: a box moved by a fraction of a texel changes the frame by {change:.4} / 255");
    assert!(change == 0.0, "{change}");
}

#[cfg(feature = "gpu")]
#[test]
fn gpu_shadow_map_stays_still_when_its_box_moves() {
    let renderer = Renderer::new(320, 180);
    if !renderer.uses_gpu() {
        eprintln!("no GPU, skipped");
        return;
    }
    let change = shadow_change(&renderer);
    eprintln!("gpu: a box moved by a fraction of a texel changes the frame by {change:.4} / 255");
    assert!(change == 0.0, "{change}");
}

/// The speed of the shaking camera target at the integer steps of the noise, against its
/// mean speed. Value noise stops at each step, so this is near 0 for it.
#[test]
fn shake_does_not_stop() {
    let base = Camera::perspective(40.0)
        .at(0.0, 0.0, 5.0)
        .look_at(Vec3::ZERO);
    let target = |t: f32| base.shake(t, 0.01, 7).target;
    let dt = 1e-3;
    let speed = |t: f32| (target(t + dt) - target(t - dt)).length() / (2.0 * dt);
    // `shake` runs the noise at 4.5 steps per second
    let samples = 2000;
    let mean = (0..samples)
        .map(|i| speed(0.3 + i as f32 * 0.00737))
        .sum::<f32>()
        / samples as f32;
    let keys = 180;
    let at_keys = (1..=keys).map(|k| speed(k as f32 / 4.5)).sum::<f32>() / keys as f32;
    let slowest = (0..samples * 10)
        .map(|i| speed(0.3 + i as f32 * 0.000737))
        .filter(|&s| s < 0.1 * mean)
        .count() as f32
        / (samples * 10) as f32;
    eprintln!(
        "shake: mean speed {mean:.4}, at the noise steps {at_keys:.4}, below 10% of the mean {:.1}% of the time",
        slowest * 100.0
    );
    assert!(at_keys > 0.9 * mean, "{at_keys} {mean}");
    assert!(slowest < 0.01, "{slowest}");

    // the amplitude stays about as before
    let angle = (0..samples)
        .map(|i| {
            let t = target(i as f32 * 0.0137);
            (t - base.target).length() / 5.0
        })
        .sum::<f32>()
        / samples as f32;
    eprintln!("shake: mean angle {angle:.4} for an amount of 0.01");
    assert!((0.004..0.012).contains(&angle), "{angle}");
}

/// `noise1` keeps its values at the integers and its amplitude, but it does not stop there.
#[test]
fn noise_moves_at_its_keys() {
    let dt = 1e-3;
    let slope = |x: f32| (math::noise1(3, x + dt) - math::noise1(3, x - dt)) / (2.0 * dt);
    let at_keys = (1..400).map(|k| slope(k as f32).abs()).sum::<f32>() / 399.0;
    let rms = ((0..4000)
        .map(|i| math::noise1(3, i as f32 * 0.1003).powi(2))
        .sum::<f32>()
        / 4000.0)
        .sqrt();
    eprintln!("noise1: mean |slope| at the keys {at_keys:.3}, rms {rms:.3}");
    assert!(at_keys > 0.3, "{at_keys}");
    assert!((0.4..0.6).contains(&rms), "{rms}");
}
