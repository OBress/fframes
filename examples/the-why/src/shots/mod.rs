//! The shots of the film. A shot is a 3D picture with SVG over it, or SVG only.

pub mod cursor;
pub mod title;

use std::sync::Arc;

use fframes::media::ImageData;
use fframes::{
    Duration, FFramesContext, FFramesSyncedVideoFrame, Frame, Scene, Svgr, SyncVideoFrameInput,
    svgr,
};
use fframes_3d::{Renderer, Texture};

use crate::WIDTH;
use crate::look::{self, BAR, PICTURE_HEIGHT, SANS};

type Picture = Box<
    dyn for<'a, 'b> Fn(&Renderer, &Frame, &FFramesContext<'a, 'b>) -> ImageData<'static>
        + Send
        + Sync,
>;
type Overlay = Box<dyn Fn(f32) -> Svgr<'static> + Send + Sync>;
type Strength = Box<dyn Fn(f32) -> f32 + Send + Sync>;

/// One shot of the film.
pub struct Shot {
    name: &'static str,
    seconds: f32,
    /// The renderer and the 3D picture at `t` seconds into the shot.
    picture: Option<(Renderer, Picture)>,
    /// The SVG over the picture at `t` seconds.
    overlay: Overlay,
    /// The strength of the vignette at `t` seconds, from 0 to 1.
    vignette: Strength,
}

impl std::fmt::Debug for Shot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Shot")
            .field("name", &self.name)
            .finish_non_exhaustive()
    }
}

impl Shot {
    /// A shot with a 3D picture.
    pub fn new(
        name: &'static str,
        seconds: f32,
        renderer: Renderer,
        picture: impl Fn(&Renderer, f32) -> ImageData<'static> + Send + Sync + 'static,
    ) -> Self {
        Self::with_media(name, seconds, renderer, move |r, frame, _| {
            picture(r, frame.seconds())
        })
    }

    /// A shot with a 3D picture that uses the media of the video, for example the frames of
    /// a video on a screen in the scene.
    pub fn with_media(
        name: &'static str,
        seconds: f32,
        renderer: Renderer,
        picture: impl for<'a, 'b> Fn(&Renderer, &Frame, &FFramesContext<'a, 'b>) -> ImageData<'static>
        + Send
        + Sync
        + 'static,
    ) -> Self {
        Self {
            name,
            seconds,
            picture: Some((renderer, Box::new(picture))),
            overlay: Box::new(|_| Svgr::empty()),
            vignette: Box::new(|_| 1.0),
        }
    }

    /// A shot of SVG only.
    pub fn svg(
        name: &'static str,
        seconds: f32,
        overlay: impl Fn(f32) -> Svgr<'static> + Send + Sync + 'static,
    ) -> Self {
        Self {
            name,
            seconds,
            picture: None,
            overlay: Box::new(overlay),
            vignette: Box::new(|_| 0.0),
        }
    }

    /// Sets the SVG over the picture.
    pub fn overlay(
        mut self,
        overlay: impl Fn(f32) -> Svgr<'static> + Send + Sync + 'static,
    ) -> Self {
        self.overlay = Box::new(overlay);
        self
    }

    /// Sets the strength of the vignette over the picture. A frame that is almost white
    /// needs less, because the dark corners make an ellipse of it.
    pub fn vignette(mut self, strength: impl Fn(f32) -> f32 + Send + Sync + 'static) -> Self {
        self.vignette = Box::new(strength);
        self
    }

    /// A shot that is not made yet: its name on dark gray.
    pub fn placeholder(name: &'static str, seconds: f32) -> Self {
        Self::svg(name, seconds, move |t| {
            let label = format!("{name}  {t:.1}s");
            svgr!(
                <g>
                    <rect x="0" y={BAR} width={WIDTH} height={PICTURE_HEIGHT} fill="#101114" />
                    <text x="160" y="1080" font-family={SANS} font-size="96" fill="#5a5d66">{label}</text>
                </g>
            )
        })
    }
}

/// The frame of the video `file` at the time of `frame`, as a texture for a screen in a 3D
/// scene. The video loops. `None` when the media has no such video.
pub fn video_texture(frame: &Frame, ctx: &FFramesContext, file: &str) -> Option<Arc<Texture>> {
    let video = frame.get_synced_video_frame(
        ctx,
        file,
        &SyncVideoFrameInput {
            start_from: 0.0,
            looping: true,
            editor_fallback_image: None,
        },
    )?;
    Some(Arc::new(Texture::from_image(&video.into_image())))
}

impl Scene for Shot {
    fn name(&self) -> &'static str {
        self.name
    }

    fn duration(&self) -> Duration<'_> {
        Duration::Seconds(self.seconds)
    }

    fn render_frame<'a>(&'a self, frame: Frame, ctx: &FFramesContext<'a, '_>) -> Svgr<'a> {
        let t = frame.seconds();
        let picture = self
            .picture
            .as_ref()
            .map(|(renderer, picture)| picture(renderer, &frame, ctx));
        // the vignette darkens the picture only: the flashes and the titles go over it
        let image = match &picture {
            Some(image) => svgr!(
                <g>
                    <image href={image.href()} x="0" y={BAR} width={WIDTH} height={PICTURE_HEIGHT} preserveAspectRatio="none" />
                    {look::vignette((self.vignette)(t))}
                </g>
            ),
            None => Svgr::empty(),
        };
        svgr!(
            <g>
                {image}
                {(self.overlay)(t)}
            </g>
        )
    }
}
