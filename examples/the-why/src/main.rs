use std::path::Path;
use std::process::ExitCode;
use std::sync::Arc;

use fframes::cli::{self, clap};
use fframes::{
    AudioMixOptions, CombinedMediaProvider, EncoderOptions, LimiterOptions, MediaDirectory,
    MediaProvider, RenderOptions,
};
use fframes_skia_renderer::{
    SkiaFFramesRenderer, SkiaPipelineConcurrencyPolicy, SkiaPipelineConfig,
};
use the_why::assets::Assets;
use the_why::{HEIGHT, TheWhy, TheWhyMedia, WIDTH};

#[derive(Debug, clap::Args)]
struct Args {
    /// Render the frame with the tiny-skia CPU backend instead of Skia on the GPU.
    #[arg(long, global = true)]
    cpu: bool,
}

fn main() -> ExitCode {
    let args = cli::parse::<Args>();
    let media = TheWhyMedia::new().expect("media");
    let folder = Path::new(env!("CARGO_MANIFEST_DIR")).join("dynamic_media");
    let assets = Assets::load(&folder).expect("dynamic_media: run ./fetch-assets.sh first");
    // the videos on screens in the scenes and the soundtrack load at run time
    let play = MediaDirectory::read_folder(folder.join("play")).expect("dynamic_media/play");
    let play = play.process_media_source().expect("dynamic_media/play");
    let media = CombinedMediaProvider::from([&media as &dyn MediaProvider, &play]);
    let scale = args.scale.unwrap_or(1.0);
    let video = TheWhy::new(Arc::new(assets), scale);

    let options = RenderOptions {
        media: Some(&media),
        video_encoder_options: EncoderOptions {
            preferred_encoder: Some("libx264"),
            codec_params: Some(&[("crf", "16"), ("preset", "slow"), ("tune", "film")]),
            ..Default::default()
        },
        // The mix measures about -15.5 LUFS. This gain brings it near -14 LUFS for the web.
        audio_mix: AudioMixOptions {
            master_gain_db: 1.5,
            limiter: Some(LimiterOptions {
                ceiling_db: -1.2,
                ..Default::default()
            }),
            ..Default::default()
        },
        ..Default::default()
    };

    let cpu = args.app.cpu;
    let command = cli::new(&video, options)
        .args(args)
        .default_output("out.mp4");
    if cpu {
        return command.run();
    }

    let (width, height) = (
        (WIDTH as f64 * scale).round() as usize,
        (HEIGHT as f64 * scale).round() as usize,
    );
    #[cfg(target_os = "macos")]
    let gpu = fframes_skia_renderer::metal::SkiaMetalCtx::new(width, height).expect("GPU context");
    #[cfg(not(target_os = "macos"))]
    let gpu =
        fframes_skia_renderer::vulkan::SkiaVulkanCtx::new(width, height).expect("GPU context");
    let pipeline = SkiaPipelineConfig {
        concurrency_policy: SkiaPipelineConcurrencyPolicy::MaxPerformance,
        ..Default::default()
    };
    #[cfg(target_os = "macos")]
    let backend = SkiaFFramesRenderer::new_metal(&gpu, pipeline);
    #[cfg(not(target_os = "macos"))]
    let backend = SkiaFFramesRenderer::new_vulkan(&gpu, pipeline);
    command.backend(backend.expect("skia renderer")).run()
}
