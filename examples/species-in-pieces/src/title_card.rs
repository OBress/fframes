//! The same thirty animal paths assemble an eighteen at both ends of the film.

use std::sync::OnceLock;

use fframes::animation::{AnimationRuntime, Easing};
use fframes::{Color, FFramesContext, FontQuery, FontStyle, Frame, Svgr, Transform, svgr};

use crate::geometry::Piece;
use crate::title_number::NUMBER;
use crate::title_shards::SHARDS;

pub(crate) const INTRO_SECONDS: f32 = 8.;
pub(crate) const ENDING_SECONDS: f32 = 8.;
pub(crate) const SCATTER_START: f32 = 5.3;

const PAPER: Color = Color::rgb(242, 240, 233);
const PARAGRAPH: [&str; 3] = [
    "Thirty pieces. Eighteen species.",
    "A glimpse of their lives and",
    "the worlds they depend on.",
];

pub(crate) fn assembled() -> [Piece; 30] {
    NUMBER.map(|path| Piece { path, color: PAPER })
}

pub(crate) fn scattered() -> [Piece; 30] {
    // Keep the reference's scattered silhouettes in the left half, so no piece
    // passes through the paragraph as the number forms.
    let inks = [
        Color::rgb(114, 113, 109),
        Color::rgb(193, 191, 183),
        Color::rgb(69, 69, 66),
        Color::rgb(156, 154, 147),
        Color::rgb(89, 88, 83),
    ];
    std::array::from_fn(|i| Piece {
        path: SHARDS[i].map(|(x, y)| (65. + x * 9.5, 75. + y * 8.7)),
        color: inks[i % inks.len()],
    })
}

pub(crate) fn pieces(pose: &[Piece; 30], seams: f32) -> Svgr<'static> {
    pose.iter()
        .enumerate()
        .map(|(i, piece)| {
            svgr!(<path id={format!("animal-piece-{i}")} d={piece.path} fill={piece.color}
            stroke="#1f1f1f" stroke-width={seams * 1.15} stroke-linejoin="round" />)
        })
        .collect()
}

#[derive(Clone, Copy, Debug)]
struct Metrics {
    title: usize,
    paragraph: usize,
}

fn measure(frame: &mut Frame, ctx: &FFramesContext<'_, '_>) -> Option<Metrics> {
    let mut title = 160;
    loop {
        let font = FontQuery {
            family: "Instrument Serif",
            size: title,
            weight: 400,
            style: FontStyle::Italic,
            ..Default::default()
        };
        let width = frame
            .text_width(ctx, font, "Species")?
            .max(frame.text_width(ctx, font, "in Pieces")?);
        if width <= 620 || title <= 120 {
            break;
        }
        title -= 2;
    }
    let mut paragraph = 40;
    loop {
        let font = FontQuery {
            family: "DM Sans",
            size: paragraph,
            weight: 500,
            ..Default::default()
        };
        let mut width = 0;
        for line in PARAGRAPH {
            width = width.max(frame.text_width(ctx, font, line)?);
        }
        if width <= 620 || paragraph <= 32 {
            break;
        }
        paragraph -= 2;
    }
    Some(Metrics { title, paragraph })
}

#[derive(Debug)]
pub(crate) struct Bookends {
    metrics: OnceLock<Metrics>,
    enter: AnimationRuntime,
    exit: AnimationRuntime,
}

impl Default for Bookends {
    fn default() -> Self {
        Self {
            metrics: OnceLock::new(),
            enter: AnimationRuntime::new(0.7, &Easing::EaseOut),
            exit: AnimationRuntime::new(0.55, &Easing::EaseIn),
        }
    }
}

impl Bookends {
    pub fn opening_alpha(&self, time: f32) -> f32 {
        1. - self
            .exit
            .solve(&(time - (INTRO_SECONDS - 0.55)).clamp(0., 0.55))
    }

    pub fn ending_alpha(&self, time: f32) -> f32 {
        self.enter.solve(&(time - 1.65).clamp(0., 0.7))
    }

    pub fn words(
        &self,
        opacity: f32,
        entrance: f32,
        frame: &mut Frame,
        ctx: &FFramesContext<'_, '_>,
    ) -> Svgr<'static> {
        if self.metrics.get().is_none()
            && let Some(metrics) = measure(frame, ctx)
        {
            let _ = self.metrics.set(metrics);
        }
        let metrics = self.metrics.get().copied().unwrap_or(Metrics {
            title: 160,
            paragraph: 38,
        });
        svgr!(<g id="title-card-copy" opacity={opacity}
            transform={Transform::translate(0., (1. - entrance) * 14.)}>
            <g font-family="DM Sans" font-weight="500">
                <text x="1095" y="272" font-size="28" letter-spacing="4" fill="#f03252">
                    "ONE SHARED WORLD"
                </text>
                <line x1="1095" y1="310" x2="1161" y2="310" stroke="#f03252" stroke-width="2" />
            </g>
            <g font-family="Instrument Serif" font-style="italic" font-weight="400"
                font-size={metrics.title} letter-spacing="-1.5" fill={PAPER}>
                <text x="1082" y="458">"Species"</text>
                <text x="1082" y="594">"in Pieces"</text>
            </g>
            <g font-family="DM Sans" font-weight="500" font-size={metrics.paragraph} fill="#bdbcb7">
                <text x="1095" y="692">{PARAGRAPH[0]}</text>
                <text x="1095" y="744">{PARAGRAPH[1]}</text>
                <text x="1095" y="796">{PARAGRAPH[2]}</text>
            </g>
        </g>)
    }
}
