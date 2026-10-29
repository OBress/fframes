// The depth pass into a shadow map.

struct ShadowPass {
    view_proj: mat4x4f,
    // A perspective map: the light's position and the map's range; w is 0 for an
    // orthographic map.
    light: vec4f,
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

struct ShadowOut {
    @builtin(position) clip: vec4f,
    @location(0) depth: f32,
    @location(1) world: vec3f,
}

// An orthographic light camera: `w` is 1 and the depth is linear across a triangle. The
// rasterizer would clip a caster beyond the near plane, but a caster between the light and
// the map still throws its shadow. The vertex sits in the middle of the depth range, and the
// fragment writes the depth clamped to the near plane, as the CPU rasterizer does.
//
// A perspective light camera (spot lights, point light cube faces) clips at its near plane
// as usual, and the fragment writes its distance to the light over the map's range: the
// receivers compare distances, which do not depend on the face or the projection.
@vertex
fn vs_shadow(@location(0) position: vec3f, copy: InstanceIn) -> ShadowOut {
    let model = mat4x4f(copy.model_0, copy.model_1, copy.model_2, copy.model_3);
    let world = draw.model * (model * vec4f(position, 1.0));
    let clip = shadow_pass.view_proj * world;
    var out: ShadowOut;
    out.world = world.xyz;
    if (shadow_pass.light.w > 0.0) {
        out.clip = clip;
    } else {
        out.clip = vec4f(clip.xy, 0.5 * clip.w, clip.w);
    }
    out.depth = clip.z / clip.w;
    return out;
}

@fragment
fn fs_shadow(in: ShadowOut) -> @builtin(frag_depth) f32 {
    if (shadow_pass.light.w > 0.0) {
        return clamp(length(in.world - shadow_pass.light.xyz) / shadow_pass.light.w, 0.0, 1.0);
    }
    return clamp(in.depth, 0.0, 1.0);
}
