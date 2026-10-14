use std::path::Path;
use std::process::ExitCode;

use anime_op::{AnimeOp, AnimeOpMedia, HEIGHT, WIDTH};
use fframes::{
    CombinedMediaProvider, EncoderOptions, MediaDirectory, RenderOptions, StaticMediaProvider, cli,
};
use fframes_skia_renderer::{
    SkiaFFramesRenderer, SkiaPipelineConcurrencyPolicy, SkiaPipelineConfig, metal::SkiaMetalCtx,
};

fn main() -> ExitCode {
    let media = AnimeOpMedia::prepare().expect("media");
    let dynamic = Path::new(env!("CARGO_MANIFEST_DIR")).join("dynamic");
    // The song is read at runtime so it stays stereo.
    let folder = MediaDirectory::read_folder(dynamic.join("audio")).expect("dynamic/audio folder");
    let audio = folder.process_media_source().expect("dynamic audio");
    let combined = CombinedMediaProvider::from([
        &media as &dyn fframes::MediaProvider,
        &audio as &dyn fframes::MediaProvider,
    ]);

    let video = AnimeOp::new(&dynamic);
    let gpu = SkiaMetalCtx::new(WIDTH, HEIGHT).expect("GPU context");

    cli::new(
        &video,
        RenderOptions {
            media: Some(&combined),
            video_encoder_options: EncoderOptions {
                preferred_encoder: Some("libx264"),
                codec_params: Some(&[("crf", "16"), ("preset", "slow")]),
                ..Default::default()
            },
            ..Default::default()
        },
    )
    .backend(
        SkiaFFramesRenderer::new_metal(
            &gpu,
            SkiaPipelineConfig {
                concurrency_policy: SkiaPipelineConcurrencyPolicy::MaxPerformance,
                ..Default::default()
            },
        )
        .expect("skia renderer"),
    )
    .preview(fframes_native_player::cli_preview)
    .default_output("anime-op.mp4")
    .run()
}
