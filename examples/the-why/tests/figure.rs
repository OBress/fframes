//! Renders the engineer in his poses and checks the feet and the drawing finger.
//!
//! The PNG files go to `std::env::temp_dir().join("the-why-figure")`.

// Colors are hex numbers as in three.js.
#![allow(clippy::unreadable_literal)]

use std::path::PathBuf;

use fframes_3d::*;
use image::RgbaImage;
use the_why::figure::{Figure, FigureStyle, Joint, LINE_END, LINE_START, Pose};

fn out_dir() -> PathBuf {
    let dir = std::env::temp_dir().join("the-why-figure");
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Light colors that show the form in the studio.
fn clay() -> FigureStyle {
    FigureStyle {
        skin: Material::standard(0xc89a7c).roughness(0.6),
        jacket: Material::standard(0x5b6f86).roughness(0.8),
        trousers: Material::standard(0x3b3f47).roughness(0.85),
        boots: Material::standard(0x5a4030).roughness(0.6),
        hair: Material::standard(0x2a2018).roughness(0.7),
    }
}

/// A studio with a floor and a sun.
fn studio() -> (Scene, Figure) {
    let mut scene = Scene::new()
        .environment(Environment::studio())
        .background(Background::Color(Rgb::hex(0x9aa0a8)));
    scene.add(Node::mesh(
        Geometry::floor(30.0, 30.0),
        Material::standard(0x8a8580).roughness(0.95),
    ));
    scene.add(Node::light(Light::sun(
        Rgb::WHITE,
        2.5,
        vec3(2.0, 5.0, 3.0),
    )));
    let figure = Figure::build(&mut scene, None, &clay());
    (scene, figure)
}

/// The dark figure in front of a bright sky, with a warm rim light from behind.
fn backlit() -> (Scene, Figure) {
    let mut scene = Scene::new()
        .environment(Environment::gradient(0xffd9a0, 0x2a2622, 0.5))
        .background(Background::Gradient {
            top: Rgb::hex(0xffe6bb),
            bottom: Rgb::hex(0xf0904a),
        });
    scene.add(Node::mesh(
        Geometry::floor(60.0, 60.0),
        Material::standard(0x3a2c22).roughness(1.0),
    ));
    scene.add(Node::light(Light::directional(
        Rgb::hex(0xffd2a0),
        5.0,
        vec3(0.4, 1.2, -4.0),
    )));
    let figure = Figure::build(&mut scene, None, &FigureStyle::default());
    (scene, figure)
}

fn render(
    renderer: &Renderer,
    scene: &Scene,
    figure: &Figure,
    pose: &Pose,
    camera: &Camera,
) -> RgbaImage {
    let mut scene = scene.clone();
    figure.apply(&mut scene, pose);
    let pixels = renderer.render_pixels(&scene, camera);
    RgbaImage::from_raw(pixels.width, pixels.height, pixels.to_straight_rgba()).unwrap()
}

/// Saves the tiles as one image with `columns` tiles in a row.
fn save_sheet(name: &str, tiles: &[RgbaImage], columns: usize) {
    let (w, h) = (tiles[0].width(), tiles[0].height());
    let rows = tiles.len().div_ceil(columns);
    let mut sheet = RgbaImage::new(w * columns as u32, h * rows as u32);
    for (i, tile) in tiles.iter().enumerate() {
        let (x, y) = ((i % columns) as u32 * w, (i / columns) as u32 * h);
        image::imageops::replace(&mut sheet, tile, i64::from(x), i64::from(y));
    }
    sheet.save(out_dir().join(format!("{name}.png"))).unwrap();
}

/// The poses of the film, with the height of the camera target.
fn poses() -> Vec<(Pose, f32)> {
    let mut poses = vec![(Pose::stand(0.0), 0.95), (Pose::stand(4.0), 0.95)];
    for k in 0..8 {
        poses.push((Pose::walk(k as f32 / 8.0, 1.0), 0.95));
    }
    for t in [0.35, 0.7, 1.0] {
        poses.push((Pose::kneel(t), 0.7));
    }
    for t in [0.0, 0.5, 1.0] {
        poses.push((Pose::draw(t), 0.55));
    }
    poses.push((Pose::look_up(1.0), 0.95));
    poses
}

#[test]
fn contact_sheet() {
    let (scene, figure) = studio();
    let renderer = Renderer::new(360, 540).cpu();
    for (view, azimuth) in [
        ("front", 0.0),
        ("three_quarter", 40.0),
        ("side", 90.0),
        ("back", 200.0),
    ] {
        let tiles: Vec<RgbaImage> = poses()
            .iter()
            .map(|(pose, height)| {
                let target = pose.position + Vec3::Y * *height;
                let camera = Camera::perspective(26.0).orbit(target, 5.0, azimuth, 6.0);
                render(&renderer, &scene, &figure, pose, &camera)
            })
            .collect();
        save_sheet(&format!("sheet_{view}"), &tiles, 6);
    }
}

#[test]
fn silhouette() {
    let (scene, figure) = backlit();
    let renderer = Renderer::new(540, 720).cpu();
    let poses = [
        Pose::stand(0.0),
        Pose::walk(0.0, 1.0),
        Pose::walk(0.3, 1.0),
        Pose::walk(0.6, 1.0),
        Pose::kneel(1.0),
        Pose::draw(0.3),
        Pose::look_up(1.0),
    ];
    for (view, azimuth) in [("side", 90.0), ("three_quarter", 140.0)] {
        let tiles: Vec<RgbaImage> = poses
            .iter()
            .map(|pose| {
                let target = pose.position + Vec3::Y * 0.85;
                let camera = Camera::perspective(30.0).orbit(target, 4.2, azimuth, -7.0);
                render(&renderer, &scene, &figure, pose, &camera)
            })
            .collect();
        save_sheet(&format!("silhouette_{view}"), &tiles, 4);
    }
}

#[test]
fn hand_close_up() {
    let renderer = Renderer::new(640, 480).cpu();
    for (name, (scene, figure)) in [("clay", studio()), ("dark", backlit())] {
        let mut tiles = Vec::new();
        for t in [0.0, 0.5, 1.0] {
            let pose = Pose::draw(t);
            let target = LINE_START.lerp(LINE_END, t) + Vec3::Y * 0.05;
            for azimuth in [-60.0, 30.0] {
                let camera = Camera::perspective(30.0).orbit(target, 0.7, azimuth, 30.0);
                tiles.push(render(&renderer, &scene, &figure, &pose, &camera));
            }
        }
        save_sheet(&format!("hand_{name}"), &tiles, 2);
    }
}

/// The lowest point of the soles stays on the ground, and a point of a sole moves less than
/// 2 cm while it touches the ground.
#[test]
fn walk_keeps_the_feet_on_the_ground() {
    let mut scene = Scene::new();
    let figure = Figure::build(&mut scene, None, &FigureStyle::default());
    for speed in [0.5, 0.7, 1.0, 1.4] {
        // Where each point (heel, ball, toe tip) of each foot touched the ground.
        let mut touch: [[Option<Vec3>; 3]; 2] = [[None; 3]; 2];
        let samples = 400;
        for k in 0..=2 * samples {
            let phase = k as f32 / samples as f32;
            let pose = Pose::walk(phase, speed).placed(vec3(2.0, 0.0, -1.0), 0.7);
            figure.apply(&mut scene, &pose);
            let feet = figure.feet(&scene);
            let lowest = feet
                .iter()
                .flatten()
                .map(|p| p.y)
                .fold(f32::INFINITY, f32::min);
            assert!(
                lowest.abs() < 0.02,
                "speed {speed}, phase {phase}: the lowest sole point is at {lowest} m"
            );
            for (foot, points) in feet.iter().enumerate() {
                for (j, p) in points.iter().enumerate() {
                    if p.y > 0.003 {
                        touch[foot][j] = None;
                        continue;
                    }
                    let start = *touch[foot][j].get_or_insert(*p);
                    let slide = (*p - start).with_y(0.0).length();
                    assert!(
                        slide < 0.02,
                        "speed {speed}, phase {phase}: foot {foot} point {j} slides {slide} m"
                    );
                }
            }
        }
    }
}

#[test]
fn kneel_keeps_the_feet_and_the_knee_on_the_ground() {
    let mut scene = Scene::new();
    let figure = Figure::build(&mut scene, None, &FigureStyle::default());
    for k in 0..=50 {
        let t = k as f32 / 50.0;
        figure.apply(&mut scene, &Pose::kneel(t));
        let lowest = figure
            .feet(&scene)
            .iter()
            .flatten()
            .map(|p| p.y)
            .fold(f32::INFINITY, f32::min);
        assert!(
            lowest.abs() < 0.02,
            "kneel {t}: the lowest sole point is at {lowest} m"
        );
    }
    let knee = scene.world_position(figure.joint(Joint::KneeR));
    assert!((knee.y - 0.055).abs() < 0.01, "the right knee is at {knee}");
}

#[test]
fn fingertip_follows_the_line() {
    let mut scene = Scene::new();
    let figure = Figure::build(&mut scene, None, &FigureStyle::default());
    let (origin, heading) = (vec3(-3.0, 0.0, 5.0), 2.0);
    let root = Mat4::from_rotation_translation(Quat::from_rotation_y(heading), origin);
    for k in 0..=40 {
        let t = k as f32 / 40.0;
        figure.apply(&mut scene, &Pose::draw(t).placed(origin, heading));
        let tip = figure.fingertip(&scene);
        let line = root.transform_point3(LINE_START.lerp(LINE_END, t));
        assert!(
            tip.distance(line) < 0.02,
            "draw {t}: the fingertip is at {tip}, the line at {line}"
        );
    }
}

/// The rail of shot 3: the top of the railing in the root space of the figure.
const RAIL: Vec3 = Vec3::new(0.0, 1.08, 0.42);

/// A catwalk floor and a railing with a flat top rail 8 cm deep, in the space of the figure.
fn catwalk(scene: &mut Scene) {
    let steel = Material::metal(0x8a9098, 0.45);
    scene.add(Node::mesh(Geometry::cuboid(4.0, 0.04, 1.3), steel.clone()).at(0.0, -0.02, -0.1));
    let top = Geometry::cuboid(4.0, 0.04, 0.08);
    scene.add(Node::mesh(top, steel.clone()).with_position(RAIL - Vec3::Y * 0.02));
    let middle = Geometry::cuboid(4.0, 0.03, 0.03);
    scene.add(Node::mesh(middle, steel.clone()).at(0.0, 0.55, RAIL.z));
    for x in [-1.5, -0.6, 0.6, 1.5] {
        let post = Geometry::cuboid(0.05, RAIL.y, 0.05);
        scene.add(Node::mesh(post, steel.clone()).at(x, RAIL.y * 0.5, RAIL.z));
    }
}

/// The palms stay on the rail while he breathes and moves his weight for 10 s.
#[test]
fn rail_keeps_the_palms_on_the_rail() {
    let mut scene = Scene::new();
    let figure = Figure::build(&mut scene, None, &FigureStyle::default());
    let (origin, heading) = (vec3(4.0, 3.0, -2.0), -0.8);
    let root = Mat4::from_rotation_translation(Quat::from_rotation_y(heading), origin);
    let (point, along) = (root.transform_point3(RAIL), root.transform_vector3(Vec3::X));
    for k in 0..=200 {
        let time = k as f32 * 0.05;
        figure.apply(
            &mut scene,
            &Pose::at_rail(time, RAIL).placed(origin, heading),
        );
        for palm in figure.palms(&scene) {
            let off = palm - point;
            let distance = (off - along * off.dot(along)).length();
            assert!(
                distance < 0.01,
                "time {time}: a palm is {distance} m from the rail"
            );
        }
        let lowest = figure
            .feet(&scene)
            .iter()
            .flatten()
            .map(|p| p.y - origin.y)
            .fold(f32::INFINITY, f32::min);
        assert!(
            lowest.abs() < 0.02,
            "time {time}: the lowest sole point is at {lowest} m"
        );
    }
}

#[test]
fn rail_sheet() {
    let renderer = Renderer::new(480, 640).cpu();
    let (mut scene, figure) = studio();
    catwalk(&mut scene);
    let below = vec3(0.6, -7.0, 4.0);
    let mut tiles = Vec::new();
    for time in [0.0, 3.0, 6.0, 9.0] {
        let pose = Pose::at_rail(time, RAIL);
        let cameras = [
            Camera::perspective(26.0).orbit(vec3(0.0, 1.0, 0.1), 4.5, 35.0, 8.0),
            Camera::perspective(26.0).orbit(vec3(0.0, 1.0, 0.1), 4.5, 90.0, 4.0),
            Camera::perspective(26.0).orbit(vec3(0.0, 1.1, 0.1), 7.0, 20.0, -30.0),
            Camera::perspective(12.0)
                .position(below)
                .look_at(vec3(0.0, 1.35, 0.1)),
        ];
        for camera in &cameras {
            tiles.push(render(&renderer, &scene, &figure, &pose, camera));
        }
    }
    save_sheet("rail", &tiles, 4);
}
