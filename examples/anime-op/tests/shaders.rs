//! Every effect shader compiles for the Skia backend.

#[test]
fn effect_shaders_compile() {
    for (name, src) in [
        ("speedlines", anime_op::gpu::SPEEDLINES),
        ("burst", anime_op::gpu::BURST),
        ("aura", anime_op::gpu::AURA),
        ("smoke", anime_op::gpu::SMOKE),
        ("rays", anime_op::gpu::RAYS),
        ("shine", anime_op::gpu::SHINE),
    ] {
        let shader = anime_op::gpu::compile(src);
        if let Err(e) = fframes_skia_renderer::render::compile_shader(&shader) {
            panic!("{name}: {e}");
        }
    }
}
