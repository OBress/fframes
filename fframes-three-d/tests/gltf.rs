#![allow(clippy::unreadable_literal)]

use base64::Engine;
use fframes_three_d::*;

/// A `.gltf` without scenes: a root with a node of one primitive and a node of two
/// primitives with a morph target, and an animation of the weights and of a translation.
fn small_gltf() -> Vec<u8> {
    let floats: Vec<f32> = [
        // 0: a triangle
        &[0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0][..],
        // 36: its morph target moves it along Z
        &[0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0],
        // 72: the keyframe times
        &[0.0, 1.0],
        // 80: the weights
        &[0.0, 1.0],
        // 88: the translations
        &[0.0, 0.0, 0.0, 2.0, 0.0, 0.0],
    ]
    .concat();
    let bytes: Vec<u8> = floats.iter().flat_map(|f| f.to_le_bytes()).collect();
    let uri = base64::engine::general_purpose::STANDARD.encode(&bytes);
    format!(
        r#"{{
        "asset": {{ "version": "2.0" }},
        "buffers": [{{ "byteLength": {len}, "uri": "data:application/octet-stream;base64,{uri}" }}],
        "bufferViews": [
            {{ "buffer": 0, "byteOffset": 0, "byteLength": 36 }},
            {{ "buffer": 0, "byteOffset": 36, "byteLength": 36 }},
            {{ "buffer": 0, "byteOffset": 72, "byteLength": 8 }},
            {{ "buffer": 0, "byteOffset": 80, "byteLength": 8 }},
            {{ "buffer": 0, "byteOffset": 88, "byteLength": 24 }}
        ],
        "accessors": [
            {{ "bufferView": 0, "componentType": 5126, "count": 3, "type": "VEC3", "min": [0, 0, 0], "max": [1, 1, 0] }},
            {{ "bufferView": 1, "componentType": 5126, "count": 3, "type": "VEC3", "min": [0, 0, 1], "max": [0, 0, 1] }},
            {{ "bufferView": 2, "componentType": 5126, "count": 2, "type": "SCALAR", "min": [0], "max": [1] }},
            {{ "bufferView": 3, "componentType": 5126, "count": 2, "type": "SCALAR" }},
            {{ "bufferView": 4, "componentType": 5126, "count": 2, "type": "VEC3" }}
        ],
        "materials": [{{ "pbrMetallicRoughness": {{ "baseColorFactor": [1, 0, 0, 0.5] }} }}],
        "meshes": [
            {{ "primitives": [{{ "attributes": {{ "POSITION": 0 }}, "material": 0 }}] }},
            {{
                "primitives": [
                    {{ "attributes": {{ "POSITION": 0 }}, "targets": [{{ "POSITION": 1 }}] }},
                    {{ "attributes": {{ "POSITION": 0 }}, "targets": [{{ "POSITION": 1 }}] }}
                ],
                "weights": [0],
                "extras": {{ "targetNames": ["push"] }}
            }}
        ],
        "nodes": [
            {{ "name": "Root", "children": [1, 2] }},
            {{ "name": "Single", "mesh": 0, "translation": [0, 1, 0] }},
            {{ "name": "Double", "mesh": 1 }}
        ],
        "animations": [{{
            "name": "Go",
            "channels": [
                {{ "sampler": 0, "target": {{ "node": 2, "path": "weights" }} }},
                {{ "sampler": 1, "target": {{ "node": 1, "path": "translation" }} }}
            ],
            "samplers": [{{ "input": 2, "output": 3 }}, {{ "input": 2, "output": 4 }}]
        }}]
    }}"#,
        len = bytes.len()
    )
    .into_bytes()
}

#[test]
fn loads_gltf_like_three_js() {
    let model = Model::from_gltf(&small_gltf()).unwrap();
    // without scenes the roots are the nodes that are nobody's child, so each node comes
    // once: Root, Single, Double and its two primitives
    assert_eq!(model.scene.len(), 5);

    let mut scene = Scene::new();
    let instance = model.add_to(&mut scene, None);
    let single = instance.find(&scene, "Single").unwrap();
    let double = instance.find(&scene, "Double").unwrap();

    // a node with one primitive is the mesh, as in three.js
    let mesh = scene[single].as_mesh().unwrap();
    // an OPAQUE material ignores the alpha of its color
    assert_eq!(mesh.material.opacity, 1.0);
    assert!(!mesh.material.transparent);
    // no normals in the file: flat shading
    assert!(mesh.material.flat_shading);
    // a node with two primitives is a group with a mesh for each
    assert!(scene[double].as_mesh().is_none());
    assert_eq!(scene[double].children().len(), 2);

    // Blender's shape key names come through `extras.targetNames`.
    let first = scene[double].children()[0];
    assert_eq!(scene[first].as_mesh().unwrap().geometry.morph_index("push"), Some(0));
    assert_eq!(instance.set_morph(&mut scene, "push", 0.7), 2);
    assert_eq!(scene[first].as_mesh().unwrap().morph_weights, vec![0.7]);
    assert_eq!(instance.set_morph(&mut scene, "pull", 1.0), 0);

    instance.play(&mut scene, model.animation("Go").unwrap(), 0.5);
    assert!((scene[single].position - vec3(1.0, 0.0, 0.0)).length() < 1e-6);
    for &child in scene[double].children() {
        let weights = &scene[child].as_mesh().unwrap().morph_weights;
        assert_eq!(weights.len(), 1);
        assert!((weights[0] - 0.5).abs() < 1e-6, "{weights:?}");
    }
}

/// Renders three poses of `RobotExpressive.glb` from three.js (CC0, Tomás Laulhé), a model
/// with a skin, morph targets and 14 clips. It runs when `FFRAMES_3D_ROBOT` is the path of
/// a copy.
#[test]
fn renders_skinned_gltf() {
    let Ok(path) = std::env::var("FFRAMES_3D_ROBOT") else {
        return;
    };
    let model = Model::from_gltf(&std::fs::read(path).unwrap()).unwrap();
    let names: Vec<&str> = model.animations.iter().map(|a| a.name.as_str()).collect();
    eprintln!("clips: {names:?}, bounds {:?}", model.bounds());
    let dance = model.animation("Dance").unwrap();

    let mut stage = Scene::new()
        .environment(Environment::studio())
        .background(Background::Color(Rgb::hex(0x202020)));
    stage.add(
        Node::mesh(Geometry::floor(40.0, 40.0), Material::standard(0x404040)).shadows(false, true),
    );
    stage.add(Node::light(Light::hemisphere(0xffffff, 0x444444, 2.0)));
    stage.add(Node::light(Light::sun(
        Rgb::WHITE,
        2.5,
        vec3(0.0, 20.0, 10.0),
    )));
    let camera = Camera::perspective(45.0)
        .at(-5.0, 3.0, 10.0)
        .look_at(vec3(0.0, 2.0, 0.0));

    let dir = std::env::temp_dir().join("fframes_three_d");
    std::fs::create_dir_all(&dir).unwrap();
    for (i, t) in [0.0, 0.4, 0.8].into_iter().enumerate() {
        let mut scene = stage.clone();
        let robot = model.add_to(&mut scene, None);
        robot.play(&mut scene, dance, t);
        let pixels = Renderer::new(960, 540).render_pixels(&scene, &camera);
        image::save_buffer(
            dir.join(format!("robot_{i}.png")),
            &pixels.to_straight_rgba(),
            pixels.width,
            pixels.height,
            image::ColorType::Rgba8,
        )
        .unwrap();
    }
}

/// A broken file whose node is its own descendant loads without recursing forever.
#[test]
fn a_node_cycle_loads() {
    let json = r#"{"asset":{"version":"2.0"},"scene":0,"scenes":[{"nodes":[0]}],
        "nodes":[{"name":"a","children":[1]},{"name":"b","children":[0]}]}"#;
    if let Ok(model) = Model::from_gltf(json.as_bytes()) {
        let mut scene = Scene::new();
        model.add_to(&mut scene, None);
    }
}

/// A `.gltf` that requires `KHR_texture_transform`, with a `MASK` material (Poly Haven
/// models are like this).
fn masked_gltf() -> Vec<u8> {
    let positions: Vec<u8> = [0.0f32, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0]
        .iter()
        .flat_map(|f| f.to_le_bytes())
        .collect();
    let buffer = base64::engine::general_purpose::STANDARD.encode(&positions);
    // a 1x1 PNG
    let png = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==";
    let transform =
        r#"{ "KHR_texture_transform": { "offset": [0.5, 0], "rotation": 1.0, "scale": [2, 3] } }"#;
    format!(
        r#"{{
        "asset": {{ "version": "2.0" }},
        "extensionsUsed": ["KHR_texture_transform"],
        "extensionsRequired": ["KHR_texture_transform"],
        "buffers": [{{ "byteLength": 36, "uri": "data:application/octet-stream;base64,{buffer}" }}],
        "bufferViews": [{{ "buffer": 0, "byteOffset": 0, "byteLength": 36 }}],
        "accessors": [{{ "bufferView": 0, "componentType": 5126, "count": 3, "type": "VEC3", "min": [0, 0, 0], "max": [1, 1, 0] }}],
        "images": [{{ "uri": "data:image/png;base64,{png}" }}],
        "textures": [{{ "source": 0 }}],
        "materials": [{{
            "alphaMode": "MASK",
            "alphaCutoff": 0.4,
            "pbrMetallicRoughness": {{
                "baseColorFactor": [1, 1, 1, 0.8],
                "baseColorTexture": {{ "index": 0, "extensions": {transform} }}
            }},
            "normalTexture": {{ "index": 0, "extensions": {transform} }}
        }}],
        "meshes": [{{ "primitives": [{{ "attributes": {{ "POSITION": 0 }}, "material": 0 }}] }}],
        "nodes": [{{ "name": "Wire", "mesh": 0 }}]
    }}"#
    )
    .into_bytes()
}

#[test]
fn loads_texture_transforms_and_mask_materials() {
    let model = Model::from_gltf(&masked_gltf()).unwrap();
    let mut scene = Scene::new();
    let instance = model.add_to(&mut scene, None);
    let wire = instance.find(&scene, "Wire").unwrap();
    let material = &scene[wire].as_mesh().unwrap().material;
    for map in [&material.map, &material.normal_map] {
        let map = map.as_ref().unwrap();
        assert_eq!(map.repeat, vec2(2.0, 3.0));
        assert_eq!(map.rotation, 1.0);
        assert_eq!(map.offset, vec2(0.5, 0.0));
    }
    // MASK cuts at alphaCutoff over the alpha of the base color, and does not blend
    assert!((material.alpha_test - 0.5).abs() < 1e-6);
    assert!(!material.transparent);
    assert_eq!(material.opacity, 1.0);
}

/// Blender's shape key names on a real export: it runs when `FFRAMES_3D_SHAPE_KEYS` is the
/// path of a `.glb` with shape keys (the asset lab's `flesh.glb` has five).
#[test]
fn reads_blender_shape_key_names() {
    let Ok(path) = std::env::var("FFRAMES_3D_SHAPE_KEYS") else {
        return;
    };
    let model = Model::from_gltf(&std::fs::read(path).unwrap()).unwrap();
    let names: Vec<String> = model
        .scene
        .nodes()
        .filter_map(|(_, n)| n.as_mesh())
        .flat_map(|m| m.geometry.morph_targets.iter().filter_map(|t| t.name.clone()))
        .collect();
    eprintln!("shape keys: {names:?}");
    assert!(!names.is_empty());
}
