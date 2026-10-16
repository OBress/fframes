//! Eighteen species, one set of thirty pieces.
//!
//! Original In Pieces polygons and CSS motion, implemented as numeric `Path {}`
//! values and fframes easing. See `source-manifest.json` for the pinned source.

use fframes::animation::{AnimationRuntime, Easing};
use fframes::{
    AudioMap, AudioTimestamp, AudioTrack, Color, Duration, FFramesContext, Frame, Scene, Scenes,
    Svgr, Video, include_media_dir, svgr,
};

mod armadillo_ball;
pub mod choreography;
mod composition;
pub mod geometry;
mod habitat;
mod map;
mod performance;
mod populations;
mod regional_map_data;
mod source_data;
mod supports;
mod title_card;
mod title_number;
mod title_shards;

use choreography::{SECONDS, SECONDS_PER_SPECIES, species_at};
use composition::{COMPOSITIONS, ENDING_BACKGROUND};
pub use map::MapStyle;
use title_card::{ENDING_SECONDS, INTRO_SECONDS};

include_media_dir!(pub struct PiecesMedia, "examples/species-in-pieces/media");

#[derive(Debug)]
pub struct PiecesFilm {
    /// Render the living habitat alone for inspecting or exporting its SVG.
    pub habitat_only: bool,
    /// Flat SVG habitat map treatment, for comparing the visual studies.
    pub map_style: MapStyle,
    captions: populations::Captions,
    performance: performance::Performance,
    habitats: [habitat::Habitat; 18],
    transition: AnimationRuntime,
    bookends: title_card::Bookends,
}
impl Default for PiecesFilm {
    fn default() -> Self {
        Self {
            habitat_only: false,
            map_style: MapStyle::default(),
            captions: populations::Captions::default(),
            performance: performance::Performance::default(),
            habitats: std::array::from_fn(habitat::Habitat::new),
            transition: AnimationRuntime::new(1.8, &Easing::CubicBezier(0.7, 0.3, 0., 1.)),
            bookends: title_card::Bookends::default(),
        }
    }
}
impl PiecesFilm {
    pub fn new() -> Self {
        Self::default()
    }

    fn opening_frame(&self, frame: &mut Frame, ctx: &FFramesContext<'_, '_>) -> Svgr<'static> {
        let time = frame.seconds();
        let words = self.bookends.opening_alpha(time);
        svgr!(<svg xmlns="http://www.w3.org/2000/svg" width={Self::WIDTH} height={Self::HEIGHT} viewBox="0 0 1920 1080">
            <rect width="1920" height="1080" fill={ENDING_BACKGROUND} />
            {title_card::pieces(&self.performance.opening(time), words)}
            {self.bookends.words(words, 1., frame, ctx)}
        </svg>)
    }

    fn ending_frame(
        &self,
        time: f32,
        frame: &mut Frame,
        ctx: &FFramesContext<'_, '_>,
    ) -> Svgr<'static> {
        let local = time - SECONDS;
        let background = self.transition.solve(&local.clamp(0., 1.8)).clamp(0., 1.);
        let title = self.bookends.ending_alpha(local);
        svgr!(<svg xmlns="http://www.w3.org/2000/svg" width={Self::WIDTH} height={Self::HEIGHT} viewBox="0 0 1920 1080">
            <rect width="1920" height="1080" fill={ENDING_BACKGROUND} />
            <g opacity={1. - background}>
                {COMPOSITIONS[17].stage(self.habitats[17].background(time))}
                {COMPOSITIONS[17].stage(self.performance.props(17, SECONDS_PER_SPECIES))}
                {COMPOSITIONS[17].stage(self.habitats[17].foreground(time))}
            </g>
            {title_card::pieces(&self.performance.ending(local), title)}
            {self.bookends.words(title, title, frame, ctx)}
        </svg>)
    }
}

#[derive(Debug)]
struct Chapter(usize);
impl Scene for Chapter {
    fn name(&self) -> &'static str {
        source_data::SPECIES[self.0].id
    }
    fn duration(&self) -> Duration<'_> {
        Duration::Seconds(SECONDS_PER_SPECIES)
    }
    fn render_frame<'a>(&'a self, _frame: Frame, _ctx: &FFramesContext<'a, '_>) -> Svgr<'a> {
        Svgr::empty()
    }
}
static CHAPTERS: [Chapter; 18] = [
    Chapter(0),
    Chapter(1),
    Chapter(2),
    Chapter(3),
    Chapter(4),
    Chapter(5),
    Chapter(6),
    Chapter(7),
    Chapter(8),
    Chapter(9),
    Chapter(10),
    Chapter(11),
    Chapter(12),
    Chapter(13),
    Chapter(14),
    Chapter(15),
    Chapter(16),
    Chapter(17),
];

#[derive(Debug)]
struct Opening;
impl Scene for Opening {
    fn name(&self) -> &'static str {
        "opening"
    }
    fn duration(&self) -> Duration<'_> {
        Duration::Seconds(INTRO_SECONDS)
    }
    fn render_frame<'a>(&'a self, _frame: Frame, _ctx: &FFramesContext<'a, '_>) -> Svgr<'a> {
        Svgr::empty()
    }
}
static OPENING: Opening = Opening;

#[derive(Debug)]
struct Ending;
impl Scene for Ending {
    fn name(&self) -> &'static str {
        "ending"
    }
    fn duration(&self) -> Duration<'_> {
        Duration::Seconds(ENDING_SECONDS)
    }
    fn render_frame<'a>(&'a self, _frame: Frame, _ctx: &FFramesContext<'a, '_>) -> Svgr<'a> {
        Svgr::empty()
    }
}
static ENDING: Ending = Ending;

impl Video for PiecesFilm {
    const FPS: usize = 60;
    const WIDTH: usize = 1920;
    const HEIGHT: usize = 1080;
    const BACKGROUND_COLOR: Color = Color::rgb(186, 148, 222);

    fn duration(&self) -> Duration<'_> {
        Duration::Seconds(INTRO_SECONDS + SECONDS + ENDING_SECONDS)
    }
    fn define_scenes(&self) -> Scenes<'_> {
        let mut scenes: Vec<&dyn Scene> = vec![&OPENING];
        scenes.extend(CHAPTERS.iter().map(|c| c as &dyn Scene));
        scenes.push(&ENDING);
        Scenes::from(scenes)
    }
    fn audio(&self) -> AudioMap<'_> {
        AudioMap::from([AudioTrack::new(
            "pieces.wav",
            AudioTimestamp::Second(0.)..AudioTimestamp::Eof,
        )
        .gain_db(-2.)
        .fade_in(2.)
        .fade_out(4.)])
    }

    fn render_frame<'a>(&'a self, mut frame: Frame, ctx: &FFramesContext<'a, '_>) -> Svgr<'a> {
        let time = frame.seconds();
        if time < INTRO_SECONDS {
            return self.opening_frame(&mut frame, ctx);
        }
        // Keep every animal's movement and habitat phase on the original clock.
        let t = time - INTRO_SECONDS;
        if t >= SECONDS {
            return self.ending_frame(t, &mut frame, ctx);
        }
        let index = species_at(t);
        let local = t - index as f32 * SECONDS_PER_SPECIES;
        let progress = if local >= 1.8 {
            1.
        } else {
            self.transition.solve(&local).clamp(0., 1.)
        };
        let previous = index.saturating_sub(1);
        let old_background = if index > 0 && progress < 1. {
            COMPOSITIONS[previous].stage(self.habitats[previous].background(t))
        } else {
            Svgr::empty()
        };
        let old_foreground = if index > 0 && progress < 1. {
            COMPOSITIONS[previous].stage(self.habitats[previous].foreground(t))
        } else {
            Svgr::empty()
        };
        let pose = self.performance.sample(t);
        let pieces: Svgr<'static> = self
            .performance
            .order(index)
            .into_iter()
            .filter(|&i| i < 30 && !self.habitat_only)
            .map(|i| {
                let piece = pose[i];
                svgr!(<path id={format!("animal-piece-{i}")} d={piece.path} fill={piece.color} />)
            })
            .collect();
        let action_props = if self.habitat_only {
            Svgr::empty()
        } else {
            self.performance.props(index, local)
        };
        let appendages = if self.habitat_only {
            Svgr::empty()
        } else {
            self.performance.appendages(index, local)
        };
        let caption = if self.habitat_only {
            Svgr::empty()
        } else {
            self.captions
                .render(index, local, self.map_style, &mut frame, ctx)
        };
        let map_backdrop = if self.habitat_only {
            Svgr::empty()
        } else {
            self.captions.map_backdrop(index, local)
        };
        svgr!(<svg xmlns="http://www.w3.org/2000/svg" width={Self::WIDTH} height={Self::HEIGHT} viewBox="0 0 1920 1080">
            <rect width="1920" height="1080" fill={ENDING_BACKGROUND} />
            {old_background}
            <g opacity={progress}>{COMPOSITIONS[index].stage(self.habitats[index].background(t))}</g>
            {map_backdrop}
            <g opacity={progress}>{COMPOSITIONS[index].stage(svgr!(<g>{action_props}{appendages}</g>))}</g>
            {pieces}
            <g opacity={1. - progress}>{old_foreground}</g>
            <g opacity={progress}>{COMPOSITIONS[index].stage(self.habitats[index].foreground(t))}</g>
            {caption}
        </svg>)
    }
}
