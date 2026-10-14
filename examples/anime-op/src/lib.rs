//! An anime opening in the style of MASHLE's second opening ("Bling-Bang-Bang-Born"), starring the
//! author of fff, fframes, odiff and zlob and his mutuals. Characters are 3D skeletons driven by
//! the Bandai Namco Research motion capture dataset and hand-keyed moves, with spring-bone hair,
//! ties and scarves and cloth robes, drawn as flat two-tone toon shapes on twos.

use std::path::Path;

use fframes::{
    AudioMap, AudioTimestamp, AudioTrack, Color, Duration, FFramesContext, Frame, Scene, Scenes,
    Svgr, Video, include_media_dir,
};

pub mod actor;
pub mod cast;
pub mod emblem;
pub mod expr;
pub mod fx;
pub mod gpu;
pub mod jojo;
pub mod motion;
pub mod palette;
pub mod physics2d;
pub mod physics3d;
pub mod procedural;
pub mod props;
pub mod render;
pub mod rigid;
pub mod shots;
pub mod sprites;
pub mod stage;

include_media_dir!(pub struct AnimeOpMedia, "examples/anime-op/media");

pub const WIDTH: usize = 1920;
pub const HEIGHT: usize = 1080;
pub const W: f32 = WIDTH as f32;
pub const H: f32 = HEIGHT as f32;

/// What a shot draws inside the frame.
pub trait Shot: Send + Sync {
    /// `t` is seconds since the cut, `len` the cut's length.
    fn render<'a>(
        &'a self,
        t: f32,
        len: f32,
        frame: &Frame,
        ctx: &FFramesContext<'a, '_>,
    ) -> Svgr<'a>;

    /// How much the compositing darkens the corners (パラ, 0 = none): flat graphic cards keep
    /// clean colour to the edges, drawn shots get the lens falloff.
    fn para(&self) -> f32 {
        1.0
    }
}

/// A shot placed on the timeline between two cuts of the song.
pub struct Cut {
    pub name: &'static str,
    pub start: f32,
    pub end: f32,
    pub shot: Box<dyn Shot>,
}

impl std::fmt::Debug for Cut {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {:.2}..{:.2}", self.name, self.start, self.end)
    }
}

thread_local! {
    static SONG_TIME: std::cell::Cell<f32> = const { std::cell::Cell::new(0.0) };
}

/// Seconds into the song of the frame being drawn on this thread: for things that run across
/// cuts (a band of text that keeps scrolling from shot to shot).
pub fn song_time() -> f32 {
    SONG_TIME.with(std::cell::Cell::get)
}

fn profiling() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var("ANIME_OP_PROFILE").is_ok())
}

/// Frames per second of the output. The animation is timed on 24 fps whatever this is.
pub const OUTPUT_FPS: usize = if cfg!(feature = "fps60") { 60 } else { 24 };

fn frame_of(seconds: f32) -> usize {
    (seconds * OUTPUT_FPS as f32).round() as usize
}

impl Scene for Cut {
    fn duration(&self) -> Duration<'_> {
        Duration::Frames(frame_of(self.end) - frame_of(self.start))
    }

    fn render_frame<'a>(&'a self, frame: Frame, ctx: &FFramesContext<'a, '_>) -> Svgr<'a> {
        let t = frame.seconds();
        if !profiling() {
            return self.shot.render(t, self.end - self.start, &frame, ctx);
        }
        // ANIME_OP_PROFILE=1: how long each frame of a shot takes to build (rasterizing and inking
        // the characters), to find the shots the real-time preview cannot keep up with
        let started = std::time::Instant::now();
        let out = self.shot.render(t, self.end - self.start, &frame, ctx);
        eprintln!(
            "profile {} {:.2}s {:.1}ms",
            self.name,
            self.start + t,
            started.elapsed().as_secs_f64() * 1000.0
        );
        out
    }

    fn name(&self) -> &'static str {
        self.name
    }
}

pub struct AnimeOp {
    pub cuts: Vec<Cut>,
}

impl AnimeOp {
    /// `dynamic` holds `mocap/` (the Bandai Namco BVH files) and `audio/song.wav`.
    pub fn new(dynamic: &Path) -> Self {
        let lib = motion::Library::load(&dynamic.join("mocap"));
        let mut everyone = cast::ALL.to_vec();
        everyone.extend(cast::RIVALS);
        everyone.push(cast::Who::Demon);
        render::prewarm(&everyone);
        AnimeOp {
            cuts: shots::cuts(&lib),
        }
    }
}

impl Video for AnimeOp {
    const FPS: usize = OUTPUT_FPS;
    const WIDTH: usize = WIDTH;
    const HEIGHT: usize = HEIGHT;
    const BACKGROUND_COLOR: Color = Color::BLACK;

    fn duration(&self) -> Duration<'_> {
        Duration::Auto
    }

    fn define_scenes(&self) -> Scenes<'_> {
        let scenes: Vec<&dyn Scene> = self.cuts.iter().map(|c| c as &dyn Scene).collect();
        Scenes::from(scenes)
    }

    fn audio(&self) -> AudioMap<'_> {
        AudioMap::from([AudioTrack::new(
            "song.wav",
            AudioTimestamp::Second(0.)..AudioTimestamp::Eof,
        )])
    }

    fn render_frame<'a>(&'a self, frame: Frame, ctx: &FFramesContext<'a, '_>) -> Svgr<'a> {
        // 撮影 (compositing), on the GPU over every frame: a diffusion glow blooms out of the
        // bright areas, and a soft gradient ("para") darkens the corners of drawn shots.
        let now = frame.seconds();
        SONG_TIME.with(|c| c.set(now));
        let para = self
            .cuts
            .iter()
            .find(|c| (c.start..c.end).contains(&now))
            .map_or(1.0, |c| c.shot.para());
        fframes::svgr!(
            <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1920 1080" width={WIDTH} height={HEIGHT}>
                <defs>
                    <filter id="satsuei" x="0" y="0" width="1920" height="1080" filterUnits="userSpaceOnUse" color-interpolation-filters="sRGB">
                        <feGaussianBlur in="SourceGraphic" stdDeviation="14" result="blur" />
                        <feComponentTransfer in="blur" result="glow">
                            <feFuncR type="gamma" amplitude="0.42" exponent="2.6" offset="0" />
                            <feFuncG type="gamma" amplitude="0.42" exponent="2.6" offset="0" />
                            <feFuncB type="gamma" amplitude="0.42" exponent="2.6" offset="0" />
                        </feComponentTransfer>
                        <feBlend in="SourceGraphic" in2="glow" mode="screen" />
                    </filter>
                    <radialGradient id="para" cx="0.5" cy="0.45" r="0.75">
                        <stop offset="0.62" stop-color="#ffffff" />
                        <stop offset="1" stop-color="#cdc4d0" />
                    </radialGradient>
                </defs>
                <g filter="url(#satsuei)">
                    {ctx.render_scenes(&frame)}
                </g>
                <rect x="0" y="0" width="1920" height="1080" fill="url(#para)" opacity={para} style="mix-blend-mode:multiply" />
            </svg>
        )
    }
}
