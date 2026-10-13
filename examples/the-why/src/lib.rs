//! THE WHY: a film about engineers and purpose, see `SCRIPT.md`. Every picture is a 3D render
//! of fframes-3d on the GPU. The titles and the robot HUD are SVG in the same frame.

// Colors are hex numbers, as in three.js: `0xff8800`.
#![allow(clippy::unreadable_literal)]

pub mod assets;
pub mod city;
pub mod desert;
pub mod factory;
pub mod figure;
pub mod look;
pub mod montage;
pub mod opening;
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
use city::CitySet;
use desert::DesertSet;
use factory::FactorySet;
use look::Overlay;
use montage::MontageSet;
use opening::OpeningSet;
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
        // the shot opens on the white light of the arc
        .vignette(|t| fframes_3d::math::smoothstep(0.0, 0.45, t));
        let (set, hud) = (factory.clone(), factory.clone());
        let engineer = Shot::new("Engineer", 10.0, renderer(), move |r, t| {
            look::picture(r, set.shutter_engineer(t), t, |t| set.moment_engineer(t))
        })
        .overlay(move |t| hud.overlay_engineer(t));
        let set = factory.clone();
        let wave = Shot::new("Wave", 8.5, renderer(), move |r, t| {
            look::picture(r, set.shutter_wave(t), t, |t| set.moment_wave(t))
        });
        // The billboards of the city play an earlier fframes film, made-of-motion.
        let city = Arc::new(CitySet::new(&assets));
        let set = city.clone();
        let city_shot = Shot::with_media(
            "City",
            13.0,
            CitySet::renderer(width, height),
            move |r, frame, ctx| {
                let t = frame.seconds();
                let film = shots::video_texture(frame, ctx, "made_of_motion.mp4");
                look::picture(r, set.shutter(t), t, |t| {
                    let (mut scene, camera) = set.moment(t);
                    if let Some(film) = &film {
                        for screen in set.screens() {
                            if let Some(material) = scene[*screen].material_mut() {
                                material.map = Some(film.clone());
                            }
                        }
                    }
                    (scene, camera)
                })
            },
        );
        let desert = Arc::new(DesertSet::new(&assets));
        let set = desert.clone();
        let walk = Shot::new(
            "Walk",
            10.0,
            DesertSet::renderer_walk(width, height),
            move |r, t| look::picture(r, set.shutter_walk(t), t, |t| set.moment_walk(t)),
        );
        let set = desert.clone();
        let line = Shot::new(
            "Line",
            6.5,
            DesertSet::renderer_line(width, height),
            move |r, t| look::picture(r, set.shutter_line(t), t, |t| set.moment_line(t)),
        );
        let set = desert.clone();
        let sunrise = Shot::new(
            "Sunrise",
            10.0,
            DesertSet::renderer_sunrise(width, height),
            move |r, t| look::picture(r, set.shutter_sunrise(t), t, |t| set.moment_sunrise(t)),
        )
        .overlay(shots::title::quote)
        // the shot opens on the white flare of the rocket
        .vignette(|t| fframes_3d::math::smoothstep(0.3, 1.5, t));
        let montage = Arc::new(MontageSet::new(&assets));
        let (set, white) = (montage.clone(), montage.clone());
        let why = Shot::new(
            "Why",
            15.0,
            MontageSet::renderer(width, height),
            move |r, t| look::picture(r, set.shutter(t), t, |t| set.moment(t)),
        )
        .overlay(move |t| white.overlay(t))
        .vignette(|t| 1.0 - montage::final_white(t));
        // The cursor grows into the light of the welding arc that opens shot 2.
        let opening = Arc::new(OpeningSet::new(&assets));
        let (set, svg) = (opening.clone(), opening);
        let cursor = Shot::new(
            "Cursor",
            7.0,
            OpeningSet::renderer(width, height),
            move |r, t| look::picture(r, set.shutter(t), t, |t| set.moment(t)),
        )
        .overlay(move |t| svg.overlay(t))
        .vignette(|t| 1.0 - fframes_3d::math::smoothstep(6.55, 6.9, t));
        Self {
            shots: vec![
                cursor,
                factory_shot,
                engineer,
                city_shot,
                walk,
                line,
                wave,
                why,
                sunrise,
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
        let mut tracks = vec![
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
            // the cursor: the hum of the screen in a quiet room, the typed question, the flood
            // of answers, the flash into the arc
            sound("sfx_screens.mp3", 0.0, -30.0)
                .fade_in(1.0)
                .fade_out(1.2),
            sound("sfx_keys.mp3", 1.4, -12.0),
            sound("sfx_flood.mp3", 3.6, -13.0).fade_in(0.8),
            sound("sfx_screens.mp3", 4.2, -20.0)
                .fade_in(0.8)
                .fade_out(0.6),
            sound("sfx_flash.mp3", 5.6, -9.0),
            // a low hit under the cuts into a new world
            sound("sfx_sub.mp3", 7.0, -9.0),
            sound("sfx_sub.mp3", 29.0, -12.0),
            // the factory: the arc on the cut, the hall, the watcher turns, the eye, the dive
            sound("sfx_arc.mp3", 7.0, -6.0),
            sound("sfx_factory.mp3", 7.0, -15.0)
                .fade_in(0.4)
                .fade_out(2.0),
            sound("sfx_servo.mp3", 16.9, -9.0),
            sound("sfx_hud.mp3", 19.0, -12.0),
            sound("sfx_hud_type.mp3", 21.4, -14.0),
            sound("sfx_chip.mp3", 26.25, -12.0),
            sound("sfx_dive.mp3", 27.0, -8.0),
            // the city, the desert, the line
            sound("sfx_city.mp3", 29.0, -17.0)
                .fade_in(1.0)
                .fade_out(1.5),
            // the rush past the Empire State Building
            sound("sfx_streak.mp3", 30.9, -15.0),
            sound("sfx_crane.mp3", 34.0, -18.0).fade_out(1.0),
            sound("sfx_wind.mp3", 42.0, 3.0).fade_in(1.5).fade_out(1.0),
            // the knee on the ground, the finger in the dust, the embers of the line
            sound("sfx_step_3.mp3", 52.37, -8.0),
            sound("sfx_scrape.mp3", 52.95, 8.0).fade_out(0.4),
            sound("sfx_ignite.mp3", 54.3, -10.0),
            sound("sfx_embers.mp3", 54.6, 6.0)
                .fade_in(0.5)
                .fade_out(1.0),
            sound("sfx_hum.mp3", 54.8, -9.0).fade_in(0.8),
            sound("sfx_streak.mp3", 56.6, -7.0),
            // the wave, the builds, the sunrise
            sound("sfx_swell.mp3", 59.25, -11.0).fade_out(1.0),
            sound("sfx_wave.mp3", 60.3, -9.0),
            // the Golden Gate: the bay at night, derricks, spinning wheels, cables, welders,
            // the deck, the first car
            sound("sfx_foghorn.mp3", 67.3, -19.0).fade_out(1.5),
            sound("sfx_crane.mp3", 67.75, -18.0).fade_out(1.0),
            sound("sfx_spin.mp3", 68.65, -16.0).fade_out(0.5),
            sound("sfx_cables.mp3", 68.97, -11.0),
            sound("sfx_arc.mp3", 69.15, -18.0).fade_out(0.6),
            sound("sfx_deck.mp3", 69.9, -5.0),
            sound("sfx_streak.mp3", 70.14, -14.0),
            // the wind farm, the school, the water
            sound("sfx_spin.mp3", 71.2, -19.0).fade_out(0.5),
            sound("sfx_village.mp3", 71.4, -16.0).fade_out(0.8),
            sound("sfx_servo.mp3", 71.96, -14.0).fade_out(0.5),
            sound("sfx_flood.mp3", 73.5, -14.0)
                .fade_in(0.2)
                .fade_out(0.7),
            // the greenhouse: the pen, the glass, the plants, the spark and the whip
            sound("sfx_ignite.mp3", 74.24, -16.0),
            sound("sfx_drones.mp3", 75.7, -17.0)
                .fade_in(0.4)
                .fade_out(1.0),
            sound("sfx_glass.mp3", 76.03, -11.0),
            sound("sfx_bloom.mp3", 76.54, 4.0).fade_out(0.8),
            sound("sfx_streak.mp3", 77.49, -10.0),
            // the launch: rollout, the crane, the deluge, the igniters, ignition, the flare
            sound("sfx_servo.mp3", 77.95, -17.0).fade_out(0.6),
            sound("sfx_crane.mp3", 78.24, -18.0).fade_out(0.6),
            sound("sfx_deluge.mp3", 78.9, -12.0).fade_out(1.0),
            sound("sfx_ignite.mp3", 79.08, -14.0),
            sound("sfx_rocket.mp3", 79.24, -7.0).fade_out(1.0),
            sound("sfx_sun.mp3", 81.55, -9.0),
            sound("sfx_breeze.mp3", 82.6, 10.0)
                .fade_in(2.0)
                .fade_out(2.0),
            sound("sfx_birds.mp3", 84.5, -17.0)
                .fade_in(2.0)
                .fade_out(2.0),
            sound("sfx_boom.mp3", 92.4, -12.0),
        ];
        // The footfalls of the walk. Four takes, leveled to the same peak, and louder as he
        // comes closer to the camera.
        let steps = [
            ("sfx_step_1.mp3", 16.0),
            ("sfx_step_2.mp3", 20.0),
            ("sfx_step_3.mp3", 4.0),
            ("sfx_step_4.mp3", 15.0),
        ];
        let walk_start = 42.0;
        for (i, t) in desert::footfalls().into_iter().enumerate() {
            let (file, level) = steps[i % steps.len()];
            let near = (t / 8.3).clamp(0.0, 1.0);
            tracks.push(sound(file, walk_start + t, level - 24.0 + 16.0 * near));
        }
        AudioMap::from(tracks)
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
