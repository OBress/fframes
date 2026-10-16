use std::process::ExitCode;

use fframes::cli::clap;
use fframes::{
    AudioMixOptions, EncoderOptions, LimiterOptions, RenderOptions, StaticMediaProvider, cli,
};
use fframes_skia_renderer::SkiaPipelineConfig;
use fframes_skia_renderer::cli::SkiaRenderers;
use species_in_pieces::{MapStyle, PiecesFilm, PiecesMedia};

#[derive(Debug, clap::Args)]
struct Args {
    /// Show only the animated vector habitat, without the animal or its caption.
    #[arg(long, global = true)]
    habitat_only: bool,
    /// Compare the independently placed flat SVG habitat atlas designs.
    #[arg(long, global = true, value_enum, default_value_t = MapStyle::default())]
    map_style: MapStyle,
}

fn main() -> ExitCode {
    let args = cli::parse::<Args>();
    let media = PiecesMedia::prepare().expect("embedded original score and ambience");
    let mut video = PiecesFilm::new();
    video.habitat_only = args.app.habitat_only;
    video.map_style = args.app.map_style;
    cli::new(
        &video,
        RenderOptions {
            media: Some(&media),
            audio_mix: AudioMixOptions {
                limiter: Some(LimiterOptions {
                    ceiling_db: -1.8,
                    ..Default::default()
                }),
                ..Default::default()
            },
            video_encoder_options: EncoderOptions {
                preferred_encoder: Some("libx264"),
                codec_params: Some(&[("crf", "18"), ("preset", "fast"), ("tune", "animation")]),
                ..Default::default()
            },
            ..Default::default()
        },
    )
    .args(args)
    .renderers(SkiaRenderers::gpu(SkiaPipelineConfig::default()))
    .preview(fframes_native_player::cli_preview)
    .default_output("examples/species-in-pieces/output/living-pieces.mp4")
    .run()
}
