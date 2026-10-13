//! THE WHY: a film about engineers and purpose, see `SCRIPT.md`. Every picture is a 3D render
//! of fframes-3d on the GPU. The titles and the robot HUD are SVG in the same frame.

// Colors are hex numbers, as in three.js: `0xff8800`.
#![allow(clippy::unreadable_literal)]

pub mod assets;
pub mod factory;
pub mod figure;
pub mod look;
pub mod shots;
pub mod stars;

use std::sync::Arc;

use fframes::{
    AudioMap, Color, Duration, FFramesContext, Frame, Scene, Scenes, Svgr, Video,
    include_media_dir, svgr,
};

use assets::Assets;
use factory::FactorySet;
use look::Overlay;
use shots::Shot;

include_media_dir!(pub struct TheWhyMedia, "examples/the-why/media");

pub const WIDTH: usize = 3840;
pub const HEIGHT: usize = 2160;
pub const FPS: usize = 60;

/// The film.
pub struct TheWhy {
    shots: Vec<Shot>,
    overlay: Overlay,
}

impl std::fmt::Debug for TheWhy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TheWhy").finish_non_exhaustive()
    }
}

impl TheWhy {
    /// The film with the 3D rendered at `scale` times the frame size.
    pub fn new(assets: Arc<Assets>, scale: f64) -> Self {
        let (width, height) = look::picture_size(scale);
        let factory = Arc::new(FactorySet::new(&assets));
        let renderer = || FactorySet::renderer(width, height);
        let set = factory.clone();
        let factory_shot = Shot::new("Factory", 12.0, renderer(), move |r, t| {
            look::picture(r, set.shutter_factory(t), t, |t| set.moment_factory(t))
        })
        .overlay(|t| look::flash_in(t, 0.45));
        let (set, hud) = (factory.clone(), factory.clone());
        let engineer = Shot::new("Engineer", 10.0, renderer(), move |r, t| {
            look::picture(r, set.shutter_engineer(t), t, |t| set.moment_engineer(t))
        })
        .overlay(move |t| hud.overlay_engineer(t));
        let set = factory.clone();
        let wave = Shot::new("Wave", 8.5, renderer(), move |r, t| {
            look::picture(r, set.shutter_wave(t), t, |t| set.moment_wave(t))
        });
        Self {
            shots: vec![
                Shot::svg("Cursor", 7.0, shots::cursor::cursor),
                factory_shot,
                engineer,
                Shot::placeholder("City", 13.0),
                Shot::placeholder("Walk", 10.0),
                Shot::placeholder("Line", 6.5),
                wave,
                Shot::placeholder("Why", 15.0),
                Shot::placeholder("Sunrise", 10.0).overlay(shots::title::quote),
                Shot::svg("Title", 6.0, shots::title::title),
            ],
            overlay: Overlay::new(),
        }
    }
}

impl Video for TheWhy {
    const FPS: usize = FPS;
    const WIDTH: usize = WIDTH;
    const HEIGHT: usize = HEIGHT;
    const BACKGROUND_COLOR: Color = Color::BLACK;

    fn duration(&self) -> Duration<'_> {
        Duration::Auto
    }

    fn audio(&self) -> AudioMap<'_> {
        AudioMap::none()
    }

    fn define_scenes(&self) -> Scenes<'_> {
        let shots: Vec<&dyn Scene> = self.shots.iter().map(|s| s as &dyn Scene).collect();
        Scenes::from(shots)
    }

    fn render_frame<'a>(&'a self, frame: Frame, ctx: &FFramesContext<'a, '_>) -> Svgr<'a> {
        svgr!(
            <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 3840 2160" width={WIDTH} height={HEIGHT}>
                {ctx.render_scenes(&frame)}
                {self.overlay.render(&frame, 0.12)}
            </svg>
        )
    }
}
