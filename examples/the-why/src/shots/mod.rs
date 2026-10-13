//! The shots of the film. A shot is a 3D picture with SVG over it, or SVG only.

pub mod cursor;
pub mod title;

use fframes::media::ImageData;
use fframes::{Duration, FFramesContext, Frame, Scene, Svgr, svgr};
use fframes_3d::Renderer;

use crate::WIDTH;
use crate::look::{BAR, PICTURE_HEIGHT, SANS};

type Picture = Box<dyn Fn(&Renderer, f32) -> ImageData<'static> + Send + Sync>;
type Overlay = Box<dyn Fn(f32) -> Svgr<'static> + Send + Sync>;

/// One shot of the film.
pub struct Shot {
    name: &'static str,
    seconds: f32,
    /// The renderer and the 3D picture at `t` seconds into the shot.
    picture: Option<(Renderer, Picture)>,
    /// The SVG over the picture at `t` seconds.
    overlay: Overlay,
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
        Self {
            name,
            seconds,
            picture: Some((renderer, Box::new(picture))),
            overlay: Box::new(|_| Svgr::empty()),
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

impl Scene for Shot {
    fn name(&self) -> &'static str {
        self.name
    }

    fn duration(&self) -> Duration<'_> {
        Duration::Seconds(self.seconds)
    }

    fn render_frame<'a>(&'a self, frame: Frame, _ctx: &FFramesContext<'a, '_>) -> Svgr<'a> {
        let t = frame.seconds();
        let picture = self
            .picture
            .as_ref()
            .map(|(renderer, picture)| picture(renderer, t));
        let image = match &picture {
            Some(image) => svgr!(
                <image href={image.href()} x="0" y={BAR} width={WIDTH} height={PICTURE_HEIGHT} preserveAspectRatio="none" />
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
