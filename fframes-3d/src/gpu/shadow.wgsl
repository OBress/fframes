// The depth pass into a shadow map.

struct ShadowPass {
    view_proj: mat4x4f,
}

// The start of `Draw` in `scene.wgsl`.
struct Draw {
    model: mat4x4f,
}

@group(0) @binding(0) var<uniform> shadow_pass: ShadowPass;
@group(1) @binding(0) var<uniform> draw: Draw;

// The model matrix of one copy of the mesh: the identity for a mesh without copies.
struct InstanceIn {
    @location(1) model_0: vec4f,
    @location(2) model_1: vec4f,
    @location(3) model_2: vec4f,
    @location(4) model_3: vec4f,
}

@vertex
fn vs_shadow(@location(0) position: vec3f, copy: InstanceIn) -> @builtin(position) vec4f {
    let model = mat4x4f(copy.model_0, copy.model_1, copy.model_2, copy.model_3);
    return shadow_pass.view_proj * (draw.model * (model * vec4f(position, 1.0)));
}
