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
    AudioTimestamp::{self, Eof, Second},
    AudioTrack, Color, Ducking, Duration, FFramesContext, FadeCurve, Frame, Scene, Scenes, Svgr,
    Video, include_media_dir, svgr,
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
        // The billboards of the city play an earlier fframes film, made-of-motion. Each one
        // plays another part of it: `make-billboards.sh` puts 20 parts in one video.
        let city = Arc::new(CitySet::new(&assets));
        let set = city.clone();
        let city_shot = Shot::with_media(
            "City",
            13.0,
            CitySet::renderer(width, height),
            move |r, frame, ctx| {
                let t = frame.seconds();
                let parts = shots::video_tiles(frame, ctx, "billboards.mp4", 5, 4);
                look::picture(r, set.shutter(t), t, |t| {
                    let (mut scene, camera) = set.moment(t);
                    if let Some(parts) = &parts {
                        for (i, screen) in set.screens().iter().enumerate() {
                            // neighbor screens get parts far apart in the film
                            let part = &parts[i * 7 % parts.len()];
                            if let Some(material) = scene[*screen].material_mut() {
                                material.map = Some(part.clone());
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
            // the hot air over the glowing line bends the picture behind it
            move |r, t| {
                let mut pixels =
                    look::picture_pixels(r, set.shutter_line(t), t, |t| set.moment_line(t));
                set.heat_shimmer(t, &mut pixels);
                pixels.into_image()
            },
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
    ///
    /// Every sound effect is mastered to -24 LUFS, so a gain is a level relative to the
    /// others: with the master gain of 1.5 dB a bed plays at `gain - 22.5` LUFS. The voice
    /// plays at about -15.5 LUFS and the beds stay 6 to 10 LU under it.
    fn audio(&self) -> AudioMap<'_> {
        // The steps of the walk (see the footfalls below): ten cut from one take, three pitched.
        const FOOTSTEPS: [&str; 13] = [
            "sfx_foot_1.wav",
            "sfx_foot_2.wav",
            "sfx_foot_3.wav",
            "sfx_foot_4.wav",
            "sfx_foot_5.wav",
            "sfx_foot_6.wav",
            "sfx_foot_7.wav",
            "sfx_foot_8.wav",
            "sfx_foot_9.wav",
            "sfx_foot_10.wav",
            "sfx_foot_11.wav",
            "sfx_foot_12.wav",
            "sfx_foot_13.wav",
        ];
        let score = Ducking {
            depth_db: -6.0,
            attack: 0.4,
            hold: 0.4,
            release: 1.2,
            ..Default::default()
        };
        // Beds and effects that play under a line of the narration give way a little.
        let under_voice = Ducking {
            depth_db: -5.0,
            attack: 0.3,
            hold: 0.3,
            release: 1.0,
            ..Default::default()
        };
        let say = |file, at: f32| AudioTrack::new(file, Second(at)..Eof).voice().gain_db(5.0);
        let sound = |file, at: f32, gain: f32| AudioTrack::new(file, Second(at)..Eof).gain_db(gain);
        let until = |file, at: f32, end: f32, gain: f32| {
            AudioTrack::new(file, Second(at)..Second(end)).gain_db(gain)
        };
        let score_part = |from: f32, to: AudioTimestamp<'static>, gain: f32| {
            AudioTrack::new("score.wav", Second(from)..to)
                .offset(from)
                .gain_db(gain)
                .duck(score)
                .fade_in(if from > 0.0 { 0.5 } else { 0.0 })
                .fade_out(0.5)
                .fade_curve(FadeCurve::Linear)
        };
        let mut tracks = vec![
            // The score in three parts that cross over in 0.5 s: the sunrise 1.5 dB lower,
            // under the quote, and the last bars louder, the piano line under the title.
            score_part(0.0, Second(82.25), 8.3),
            score_part(81.75, Second(91.5), 6.8),
            score_part(91.0, Eof, 13.5).fade_out(1.0),
            say("voice_how.wav", 10.0),
            say("voice_better.wav", 21.5),
            say("voice_anything.wav", 32.0),
            say("voice_worth.wav", 37.0),
            say("voice_theirs.wav", 61.0),
            say("voice_quote_a.wav", 84.0),
            say("voice_quote_b.wav", 86.4).gain_db(6.5),
            // The cursor: the hum of the screen in a quiet room, the typed question, the flood
            // of answers, the thousands of screens, the rush into the white of the arc.
            until("sfx_screens.wav", 0.0, 3.0, -17.5)
                .fade_in(1.0)
                .fade_out(1.2),
            sound("sfx_keys.wav", 1.4, -11.5),
            sound("sfx_flood.wav", 3.6, 3.5).fade_in(0.8),
            sound("sfx_screens.wav", 4.2, -7.5)
                .fade_in(0.8)
                .fade_out(0.6),
            sound("sfx_flash.wav", 6.7, 3.5),
            // A low hit under the cuts into a new world.
            sound("sfx_sub.wav", 7.0, 5.5),
            sound("sfx_sub.wav", 29.0, 0.0),
            // The factory: the arc on the cut, the hall, the watcher turns, the eye, the dive.
            sound("sfx_arc.wav", 7.0, 0.5),
            sound("sfx_factory.wav", 7.0, -2.5)
                .fade_in(0.4)
                .fade_out(2.0)
                .duck(under_voice),
            sound("sfx_servo.wav", 16.9, 3.5),
            sound("sfx_hud.wav", 19.0, -7.5),
            sound("sfx_hud_type.wav", 21.4, -10.5),
            sound("sfx_chip.wav", 26.25, 1.5),
            sound("sfx_dive.wav", 27.0, 3.5),
            // The city: no people, a hum, the rush past the Empire State Building, a crane.
            sound("sfx_city.wav", 29.0, -5.5).fade_in(1.0).fade_out(1.5),
            sound("sfx_streak.wav", 30.9, -4.5),
            sound("sfx_crane.wav", 34.0, -10.5).pan(-0.4).fade_out(1.0),
            // The desert: the low wind and its air, wide, from the stars to the end of the
            // line.
            until("sfx_wind.wav", 42.0, 58.5, -6.0)
                .fade_in(1.5)
                .fade_out(1.5),
            until("sfx_air.wav", 42.0, 58.5, -3.0)
                .fade_in(1.5)
                .fade_out(1.5),
            // The knee in the dust and the finger that draws: close and soft.
            sound("sfx_knee.wav", 52.37, -14.0),
            sound("sfx_finger.wav", 52.95, -9.5).fade_out(0.4),
            // The line begins to glow: its spark, its embers, its hum; then its light runs
            // across the desert into the factory.
            sound("sfx_ignite.wav", 54.3, -3.5),
            sound("sfx_embers.wav", 54.6, -13.5)
                .fade_in(0.5)
                .fade_out(1.0),
            sound("sfx_hum.wav", 54.8, -3.5).fade_in(0.8).fade_out(1.0),
            sound("sfx_streak_low.wav", 56.6, 0.5),
            // The wave: the glass wall sings, the arms stop one after another.
            sound("sfx_swell.wav", 59.25, 0.5)
                .fade_out(1.0)
                .duck(under_voice),
            sound("sfx_wave.wav", 60.3, 1.5).duck(under_voice),
            // The Golden Gate: the bay at night, derricks, spinning wheels, cables, welders,
            // the deck, the headlights of the first car on the cut.
            sound("sfx_foghorn.wav", 67.3, -4.5).pan(-0.3).fade_out(1.5),
            sound("sfx_crane.wav", 67.75, -9.5).pan(0.4).fade_out(1.0),
            sound("sfx_spin.wav", 68.65, -2.5).fade_out(0.5),
            sound("sfx_cables.wav", 68.97, -4.5),
            sound("sfx_arc.wav", 69.15, -7.5).pan(0.3).fade_out(0.6),
            sound("sfx_deck.wav", 69.9, -3.5),
            sound("sfx_streak_high.wav", 70.26, -2.5),
            // The wind farm, the school, the water.
            sound("sfx_spin_low.wav", 71.0, -4.5).fade_out(0.5),
            sound("sfx_village.wav", 71.4, -8.5).pan(-0.3).fade_out(0.8),
            sound("sfx_servo_high.wav", 71.96, -1.5).fade_out(0.5),
            until("sfx_water.wav", 73.2, 74.6, -0.5)
                .fade_in(0.2)
                .fade_out(0.5),
            // The greenhouse: the pen, the drones, the glass, the plants, the whip pan.
            sound("sfx_ignite.wav", 74.24, -6.5),
            sound("sfx_drones.wav", 75.7, -5.5)
                .fade_in(0.4)
                .fade_out(1.0),
            sound("sfx_glass.wav", 76.03, -4.5),
            until("sfx_bloom.wav", 75.4, 77.85, -7.5).fade_out(0.6),
            sound("sfx_streak.wav", 77.35, -0.5),
            // The launch: rollout, the crane, the deluge, the igniters, ignition, and the
            // flare that becomes the sun on the cut.
            sound("sfx_servo_low.wav", 77.95, -3.5).fade_out(0.6),
            sound("sfx_crane.wav", 78.24, -10.5).pan(-0.2).fade_out(0.6),
            sound("sfx_deluge.wav", 78.9, -4.0).fade_out(1.0),
            sound("sfx_ignite.wav", 79.08, -6.5),
            until("sfx_rocket.wav", 79.24, 83.0, -0.5).fade_out(1.2),
            sound("sfx_sun.wav", 80.85, -2.0),
            // The sunrise: the morning air and the first birds, then the hit of the title.
            sound("sfx_breeze.wav", 82.6, -9.5)
                .fade_in(2.0)
                .fade_out(2.0),
            sound("sfx_birds.wav", 84.5, -13.5)
                .fade_in(2.0)
                .fade_out(2.0)
                .duck(under_voice),
            sound("sfx_boom.wav", 92.4, -3.5),
        ];
        // The footfalls of the walk. He is 20 m away when the shot opens and 3 m away when he
        // kneels. A step is 17 dB under the wind far away and 11 dB under it close by (the
        // level follows the distance, compressed), varies a little from step to step, and
        // pans with him from left of the centre of the frame to the centre. The last one is
        // the heavy step into the kneel.
        let walk_start = 42.0;
        // The level of a step that is as loud as the wind and its air (about -26 LUFS): the
        // energy of a step lies in 150 ms, so it measures 4.5 dB louder than its mastered
        // loudness.
        let wind = -8.0;
        let footfalls = desert::footfalls();
        let kneel = footfalls.len() - 1;
        for (i, t) in footfalls.into_iter().enumerate() {
            // He walks at 1 m/s and stops at 8.5 s.
            let near = (t / 8.5).clamp(0.0, 1.0);
            let distance = 20.0 - 17.0 * near;
            let under = 11.0 + 6.0 * (distance / 3.0).log10() / (20.0_f32 / 3.0).log10();
            let vary = 2.4 * fframes_3d::math::hash(i as u32 + 7) - 1.2;
            // Five steps apart in the list: the same step never comes twice in a row.
            let file = if i == kneel {
                "sfx_foot_11.wav"
            } else {
                FOOTSTEPS[i * 5 % FOOTSTEPS.len()]
            };
            tracks.push(sound(file, walk_start + t, wind - under + vary).pan(-0.25 * (1.0 - near)));
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
