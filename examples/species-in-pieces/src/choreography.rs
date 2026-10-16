//! The original website's CSS state machine, sampled with fframes easing.
//!
//! The website advances only when a visitor asks. This film advances every ten
//! seconds: two seconds to assemble, then two complete four-second idle cycles.
//! All polygon targets, transition times, delays and curves are imported data.

use std::collections::BTreeSet;

use fframes::Color;
use fframes::animation::{Animatable, AnimationRuntime, Easing};

use crate::geometry::{Path, Piece};
use crate::source_data::{CELLS, SHAPES, SPECIES, TIMINGS};

pub const SPECIES_COUNT: usize = 18;
pub const PIECES: usize = 33; // Thirty morph pieces, plus the original three details.
pub const SECONDS_PER_SPECIES: f32 = 10.;
pub const SECONDS: f32 = SPECIES_COUNT as f32 * SECONDS_PER_SPECIES;

#[derive(Clone, Copy, Debug)]
pub(crate) struct CssTiming {
    pub duration: f32,
    pub delay: f32,
    pub curve: [f32; 4],
}
impl CssTiming {
    pub(crate) fn runtime(self, factor: f32) -> AnimationRuntime {
        let [x1, y1, x2, y2] = self.curve;
        AnimationRuntime::new(
            (self.duration * factor).max(f32::EPSILON),
            &Easing::CubicBezier(x1, y1, x2, y2),
        )
    }
}
const CSS_EASE: CssTiming = CssTiming {
    duration: 1.,
    delay: 0.,
    curve: [0.25, 0.1, 0.25, 1.],
};

#[derive(Clone, Copy, Debug)]
pub(crate) struct Cell {
    pub shape: usize,
    pub path_timing: usize,
    pub color_timing: usize,
    pub z: u8,
}
#[derive(Clone, Copy, Debug)]
pub(crate) struct Mover {
    pub offset: (f32, f32),
    pub timing: usize,
}
#[derive(Debug)]
pub(crate) struct SourceSpecies {
    pub id: &'static str,
    pub background: Color,
    pub states: [[usize; PIECES]; 9],
    pub movers: [Mover; 9],
    pub bubbles: [f32; PIECES],
}

#[derive(Debug)]
struct Segment<T> {
    changed_at: f32,
    starts: f32,
    duration: f32,
    from: T,
    to: T,
    reverse_start: T,
    shortening: f32,
    runtime: AnimationRuntime,
}
impl<T: Animatable> Segment<T> {
    fn progress(&self, t: f32) -> f32 {
        if t < self.starts {
            0.
        } else if t >= self.starts + self.duration {
            1.
        } else {
            self.runtime.solve(&(t - self.starts)).clamp(0., 1.)
        }
    }
    fn sample(&self, t: f32) -> T {
        self.from.apply_progress(&self.to, self.progress(t))
    }
}

#[derive(Debug)]
struct Track<T> {
    initial: T,
    segments: Vec<Segment<T>>,
}
impl<T: Animatable + PartialEq> Track<T> {
    fn new(initial: T) -> Self {
        Self {
            initial,
            segments: Vec::new(),
        }
    }
    fn sample(&self, t: f32) -> T {
        let index = self.segments.partition_point(|s| s.changed_at <= t);
        index
            .checked_sub(1)
            .map_or(self.initial, |i| self.segments[i].sample(t))
    }
    fn retarget(&mut self, now: f32, to: T, timing: CssTiming) {
        let previous = self.segments.last();
        if previous.map_or(self.initial, |s| s.to) == to {
            return;
        }
        let from = self.sample(now);
        let mut shortening = 1.;
        let mut reverse_start = from;
        // CSS Transitions' reversing-shortening rule matters for the original
        // 100 ms double blinks and any stroke interrupted by the next state.
        if let Some(old) = previous
            && now < old.starts + old.duration
            && old.reverse_start == to
        {
            shortening = (old.progress(now) * old.shortening + (1. - old.shortening))
                .abs()
                .clamp(0., 1.);
            reverse_start = old.to;
        }
        self.segments.push(Segment {
            changed_at: now,
            starts: now + timing.delay,
            duration: timing.duration * shortening,
            from,
            to,
            reverse_start,
            shortening,
            runtime: timing.runtime(shortening),
        });
    }
}

pub fn species_at(t: f32) -> usize {
    ((t.max(0.) / SECONDS_PER_SPECIES) as usize).min(SPECIES_COUNT - 1)
}

fn state_at(t: f32) -> usize {
    let centiseconds = (t * 100. + 0.001) as usize;
    if centiseconds % 1000 < 200 {
        return 0;
    }
    let pose = (centiseconds / 100) % 4;
    let blink = matches!(centiseconds % 300, 100..=109 | 140..=149);
    1 + pose + if blink { 4 } else { 0 }
}

/// Precompiled transitions: no CSS parsing, I/O, or pose construction per frame.
#[derive(Debug)]
pub struct Choreography {
    paths: [Track<Path>; PIECES],
    colors: [Track<Color>; PIECES],
    mover: Track<(f32, f32)>,
    background: Track<Color>,
    ease: AnimationRuntime,
}
impl Default for Choreography {
    fn default() -> Self {
        let mut motion = Self {
            paths: std::array::from_fn(|_| {
                Track::new(Path {
                    points: [(50., 50.); 3],
                })
            }),
            colors: std::array::from_fn(|i| {
                Track::new(SHAPES[CELLS[SPECIES[0].states[0][i]].shape].color)
            }),
            mover: Track::new((0., 0.)),
            background: Track::new(SPECIES[0].background),
            ease: CSS_EASE.runtime(1.),
        };
        let mut events = BTreeSet::new();
        for time in (0..SECONDS as u32 * 100).step_by(100) {
            events.insert(time);
        }
        for period in (0..SECONDS as u32 * 100).step_by(300) {
            for offset in [100, 110, 140, 150] {
                if period + offset < SECONDS as u32 * 100 {
                    events.insert(period + offset);
                }
            }
        }
        for event in events {
            let t = event as f32 / 100.;
            let source = &SPECIES[species_at(t)];
            let state = state_at(t);
            for i in 0..PIECES {
                let cell = CELLS[source.states[state][i]];
                let piece = SHAPES[cell.shape];
                motion.paths[i].retarget(t, piece.path, TIMINGS[cell.path_timing]);
                motion.colors[i].retarget(t, piece.color, TIMINGS[cell.color_timing]);
            }
            let mover = source.movers[state];
            motion
                .mover
                .retarget(t, mover.offset, TIMINGS[mover.timing]);
            motion.background.retarget(
                t,
                source.background,
                CssTiming {
                    duration: 0.5,
                    ..CSS_EASE
                },
            );
        }
        motion
    }
}
impl Choreography {
    pub fn background(&self, t: f32) -> Color {
        self.background.sample(t)
    }

    pub fn offset(&self, t: f32) -> (f32, f32) {
        self.mover.sample(t)
    }

    pub fn order(&self, t: f32) -> [usize; PIECES] {
        let source = &SPECIES[species_at(t)];
        let state = state_at(t);
        let mut order = std::array::from_fn(|i| i);
        order.sort_by_key(|&i| (CELLS[source.states[state][i]].z, i));
        order
    }

    pub fn piece(&self, t: f32, i: usize) -> Piece {
        let mut piece = Piece {
            path: self.paths[i].sample(t),
            color: self.colors[i].sample(t),
        };
        let source = &SPECIES[species_at(t)];
        let bubble_delay = source.bubbles[i];
        if bubble_delay >= 0. {
            let local = t % SECONDS_PER_SPECIES - bubble_delay;
            if local >= 0. {
                let p = local.fract();
                let rise = self.ease.solve(&(p / 0.98).clamp(0., 1.));
                piece.path = piece.path.translated(0., 5. - 10. * rise);
                let alpha = if p < 0.49 {
                    self.ease.solve(&(p / 0.49))
                } else {
                    1. - self.ease.solve(&((p - 0.49) / 0.49).clamp(0., 1.))
                };
                piece.color = Color::rgba(255, 255, 255, (255. * alpha) as u8);
            }
        }
        // The original travelling highlight: .8 s per shard, .05 s stagger,
        // active for two seconds in every seven-second global cycle.
        let phase = t % 7.;
        if i < 30 && (4. ..6.).contains(&phase) {
            let elapsed = phase - 4. - (0.07 + i as f32 * 0.05);
            let p = elapsed / 0.8;
            if (0. ..1.).contains(&p) {
                let opacity = if p < 0.35 {
                    self.ease.solve(&(p / 0.35)) * 0.1
                } else {
                    (1. - self.ease.solve(&((p - 0.35) / 0.65))) * 0.1
                };
                piece.color = piece.color.apply_progress(&Color::WHITE, opacity);
            }
        }
        piece
    }

    pub fn caption_opacity(&self, t: f32) -> f32 {
        let local = t % SECONDS_PER_SPECIES;
        if local < 0.15 {
            0.
        } else {
            self.ease.solve(&((local - 0.15) / 0.35).clamp(0., 1.))
        }
    }
}
