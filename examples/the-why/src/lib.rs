//! THE WHY: a film about engineers and purpose, see `SCRIPT.md`. Every picture is a 3D render
//! of fframes-3d on the GPU. The titles and the robot HUD are SVG in the same frame.

// Colors are hex numbers, as in three.js: `0xff8800`.
#![allow(clippy::unreadable_literal)]

pub mod assets;
pub mod factory;
pub mod figure;
pub mod look;
pub mod montage;
pub mod shots;
pub mod stars;

use std::sync::Arc;

use fframes::{
    AudioMap,
    AudioTimestamp::{Eof, Second},
    AudioTrack, Color, Ducking, Duration, FFramesContext, Frame, Scene, Scenes, Svgr, Video,
    include_media_dir, svgr,
};

use assets::Assets;
use factory::FactorySet;
use look::Overlay;
use montage::MontageSet;
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
        let montage = Arc::new(MontageSet::new(&assets));
        let (set, white) = (montage.clone(), montage.clone());
        let why = Shot::new(
            "Why",
            15.0,
            MontageSet::renderer(width, height),
            move |r, t| look::picture(r, set.shutter(t), t, |t| set.moment(t)),
        )
        .overlay(move |t| white.overlay(t));
        Self {
            shots: vec![
                Shot::svg("Cursor", 7.0, shots::cursor::cursor),
                factory_shot,
                engineer,
                Shot::placeholder("City", 13.0),
                Shot::placeholder("Walk", 10.0),
                Shot::placeholder("Line", 6.5),
                wave,
                why,
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

    /// The sound of the film, made by `make-soundtrack.sh`: one score composed to the cut,
    /// the narration and the sound effects, at the times of SCRIPT.md.
    fn audio(&self) -> AudioMap<'_> {
        let score = Ducking {
            depth_db: -7.0,
            attack: 0.4,
            hold: 0.4,
            release: 1.2,
            ..Default::default()
        };
        let say = |file, at: f32| AudioTrack::new(file, Second(at)..Eof).voice().gain_db(1.0);
        let sound = |file, at: f32, gain: f32| AudioTrack::new(file, Second(at)..Eof).gain_db(gain);
        AudioMap::from(vec![
            AudioTrack::new("score.mp3", Second(0.0)..Eof)
                .gain_db(-4.0)
                .duck(score),
            say("voice_how.mp3", 10.0),
            say("voice_better.mp3", 21.5),
            say("voice_anything.mp3", 32.0),
            say("voice_worth.mp3", 37.0),
            say("voice_theirs.mp3", 61.0),
            say("voice_quote_a.mp3", 84.0),
            say("voice_quote_b.mp3", 86.4),
            // the cursor: the typed question, the flood of answers, the flash into the arc
            sound("sfx_keys.mp3", 1.4, -12.0),
            sound("sfx_flood.mp3", 3.6, -13.0).fade_in(0.8),
            sound("sfx_flash.mp3", 5.6, -9.0),
            // the factory: the arc on the cut, the hall, the watcher turns, the eye, the dive
            sound("sfx_arc.mp3", 7.0, -6.0),
            sound("sfx_factory.mp3", 7.0, -15.0)
                .fade_in(0.4)
                .fade_out(2.0),
            sound("sfx_servo.mp3", 16.9, -9.0),
            sound("sfx_hud.mp3", 19.0, -12.0),
            sound("sfx_hud_type.mp3", 21.4, -14.0),
            sound("sfx_dive.mp3", 26.2, -8.0),
            // the city, the desert, the line
            sound("sfx_city.mp3", 29.0, -17.0)
                .fade_in(1.0)
                .fade_out(1.5),
            sound("sfx_crane.mp3", 34.0, -18.0).fade_out(1.0),
            sound("sfx_wind.mp3", 42.0, -8.0).fade_in(1.5).fade_out(1.0),
            sound("sfx_steps.mp3", 45.0, -9.0).fade_out(1.0),
            sound("sfx_ignite.mp3", 54.3, -10.0),
            sound("sfx_hum.mp3", 54.8, -9.0).fade_in(0.8),
            sound("sfx_streak.mp3", 56.6, -7.0),
            // the wave, the builds, the sunrise
            sound("sfx_wave.mp3", 59.6, -9.0),
            sound("sfx_cables.mp3", 69.2, -11.0),
            sound("sfx_village.mp3", 70.8, -16.0).fade_out(1.0),
            sound("sfx_glass.mp3", 73.5, -11.0),
            sound("sfx_streak.mp3", 76.8, -10.0),
            sound("sfx_rocket.mp3", 78.9, -7.0).fade_out(1.0),
            sound("sfx_sun.mp3", 81.3, -9.0),
            sound("sfx_birds.mp3", 84.5, -17.0)
                .fade_in(2.0)
                .fade_out(2.0),
            sound("sfx_boom.mp3", 92.4, -12.0),
        ])
    }

    fn define_scenes(&self) -> Scenes<'_> {
        let shots: Vec<&dyn Scene> = self.shots.iter().map(|s| s as &dyn Scene).collect();
        Scenes::from(shots)
    }

    fn render_frame<'a>(&'a self, frame: Frame, ctx: &FFramesContext<'a, '_>) -> Svgr<'a> {
        svgr!(
            <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 3840 2160" width={WIDTH} height={HEIGHT}>
                {ctx.render_scenes(&frame)}
                {self.overlay.render(&frame, 0.09)}
            </svg>
        )
    }
}
