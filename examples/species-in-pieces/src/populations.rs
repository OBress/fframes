//! Only documented population figures appear in the film. Dates and units stay
//! attached to the figures; full sources and taxonomic notes are in README.md.

use std::sync::OnceLock;

use fframes::animation::{AnimationRuntime, Easing};
use fframes::{FFramesContext, FontQuery, FontStyle, Frame, Svgr, svgr};

use crate::composition::{COMPOSITIONS, Composition};
use crate::map::{HabitatMaps, MapStyle};

struct Population {
    line: &'static str,
}

struct Caption {
    lines: &'static [&'static str],
    population: Option<Population>,
}

const CAPTIONS: [Caption; 18] = [
    Caption {
        lines: &["Helmeted", "hornbill"],
        population: None,
    },
    Caption {
        lines: &["Vaquita"],
        population: Some(Population {
            line: "7 to 10 observed (2025 survey)",
        }),
    },
    Caption {
        lines: &["Golden lion", "tamarin"],
        population: Some(Population {
            line: "About 4,800 in the wild (2023 census)",
        }),
    },
    Caption {
        lines: &["Golden poison frog"],
        population: None,
    },
    Caption {
        lines: &["Forest owlet"],
        population: Some(Population {
            line: "250 to 999 mature individuals (2018 estimate)",
        }),
    },
    Caption {
        lines: &["Kemp’s ridley", "turtle"],
        population: Some(Population {
            line: "About 22,000 mature individuals (2019 estimate)",
        }),
    },
    Caption {
        lines: &["Scimitar oryx"],
        population: Some(Population {
            line: "More than 650 in the wild (mid 2026)",
        }),
    },
    Caption {
        lines: &["Fiji crested iguana"],
        population: Some(Population {
            line: "About 13,000 (estimate checked 2026)",
        }),
    },
    Caption {
        lines: &["Knysna", "seahorse"],
        population: None,
    },
    Caption {
        lines: &["Brazilian three-banded", "armadillo"],
        population: None,
    },
    Caption {
        lines: &["Pygmy three-toed", "sloth"],
        population: None,
    },
    Caption {
        lines: &["Kākāpō"],
        population: Some(Population {
            line: "325 alive (October 2026)",
        }),
    },
    Caption {
        lines: &["Long-beaked", "echidna"],
        population: None,
    },
    Caption {
        lines: &["African penguin"],
        population: Some(Population {
            line: "Fewer than 10,000 breeding pairs (2025 estimate)",
        }),
    },
    Caption {
        lines: &["Greek red", "damsel"],
        population: None,
    },
    Caption {
        lines: &["Sun bear"],
        population: None,
    },
    Caption {
        lines: &["Rainbow", "parrotfish"],
        population: None,
    },
    Caption {
        lines: &["Wild Bactrian camel"],
        population: Some(Population {
            line: "About 1,050 in the wild (estimate checked 2026)",
        }),
    },
];

#[derive(Clone, Copy, Debug)]
struct Metrics {
    size: usize,
    width: f32,
}

fn measure(index: usize, frame: &mut Frame, ctx: &FFramesContext<'_, '_>) -> Option<Metrics> {
    let layout = COMPOSITIONS[index];
    let caption = &CAPTIONS[index];
    let mut size = layout.size;
    let width = loop {
        let query = FontQuery {
            family: "Instrument Serif",
            size,
            weight: 400,
            style: FontStyle::Italic,
            ..Default::default()
        };
        let width = caption
            .lines
            .iter()
            .map(|line| frame.text_width(ctx, query, line))
            .collect::<Option<Vec<_>>>()?
            .into_iter()
            .max()
            .unwrap_or(0);
        if width <= layout.width || size <= 72 {
            break width;
        }
        size -= 2;
    };
    let mut width = width;
    if let Some(p) = &caption.population {
        width = width.max(frame.text_width(
            ctx,
            FontQuery {
                family: "DM Sans",
                size: 28,
                weight: 500,
                ..Default::default()
            },
            p.line,
        )?);
    }
    Some(Metrics {
        size,
        width: width as f32,
    })
}

#[derive(Debug)]
pub(crate) struct Captions {
    metrics: [OnceLock<Metrics>; 18],
    maps: HabitatMaps,
    enter: AnimationRuntime,
    exit: AnimationRuntime,
}

impl Default for Captions {
    fn default() -> Self {
        Self {
            metrics: std::array::from_fn(|_| OnceLock::new()),
            maps: HabitatMaps::default(),
            enter: AnimationRuntime::new(0.65, &Easing::EaseOut),
            exit: AnimationRuntime::new(0.45, &Easing::EaseIn),
        }
    }
}

impl Captions {
    fn alpha(&self, index: usize, local: f32) -> f32 {
        self.enter
            .solve(&(local - COMPOSITIONS[index].entrance).clamp(0., 0.65))
            * (1. - self.exit.solve(&(local - 9.5).clamp(0., 0.45)))
    }

    pub fn map_backdrop(&self, index: usize, local: f32) -> Svgr<'static> {
        svgr!(<g opacity={self.alpha(index, local)}>{self.maps.backdrop(index, local)}</g>)
    }

    pub fn render(
        &self,
        index: usize,
        local: f32,
        map_style: MapStyle,
        frame: &mut Frame,
        ctx: &FFramesContext<'_, '_>,
    ) -> Svgr<'static> {
        let layout = COMPOSITIONS[index];
        let alpha = self.alpha(index, local);
        if alpha <= 0. {
            return Svgr::empty();
        }
        let metrics = if let Some(metrics) = self.metrics[index].get() {
            *metrics
        } else if let Some(metrics) = measure(index, frame, ctx) {
            *self.metrics[index].get_or_init(|| metrics)
        } else {
            Metrics {
                size: 72,
                width: layout.width as f32,
            }
        };
        let map = self.maps.render(index, local, map_style, frame, ctx);
        svgr!(<g id="species-annotation" opacity={alpha}>
            {draw(index, layout, metrics, 1.)}
            {map}
        </g>)
    }
}

fn draw(index: usize, layout: Composition, metrics: Metrics, alpha: f32) -> Svgr<'static> {
    let caption = &CAPTIONS[index];
    let line_height = metrics.size as f32 * 1.04;
    let bottom = layout.y + line_height * (caption.lines.len() - 1) as f32;
    let left = layout.alignment.left(layout.x, metrics.width);
    let top = layout.y - metrics.size as f32 * 0.85;
    let height = bottom - top
        + if caption.population.is_some() {
            64.
        } else {
            24.
        };
    let center = (left + metrics.width / 2., top + height / 2.);
    let veil = if layout.veil > 0. {
        let id = format!("title-veil-{index}");
        svgr!(<g>
            <defs><radialGradient id={id.clone()}>
                <stop offset="0" stop-color={layout.veil_color} stop-opacity={layout.veil} />
                <stop offset="0.48" stop-color={layout.veil_color} stop-opacity={layout.veil * 0.8} />
                <stop offset="1" stop-color={layout.veil_color} stop-opacity="0" />
            </radialGradient></defs>
            <ellipse cx={center.0} cy={center.1} rx={metrics.width * 0.78 + 65.}
                ry={height * 0.9 + 80.} fill={format!("url(#{id})")} />
        </g>)
    } else {
        Svgr::empty()
    };
    let title: Svgr<'static> = caption.lines.iter().enumerate().map(|(i, &line)| {
        svgr!(<text x={layout.x} y={layout.y + i as f32 * line_height}
            font-family="Instrument Serif" font-style="italic" font-weight="400" font-size={metrics.size}>{line}</text>)
    }).collect();
    let population = caption.population.as_ref().map_or_else(Svgr::empty, |p| {
        svgr!(<g font-family="DM Sans" font-size="28">
        <text x={layout.x} y={bottom + 49.} font-weight="500">{p.line}</text>
    </g>)
    });
    svgr!(<g id="species-caption" opacity={alpha} fill={layout.ink} text-anchor={layout.alignment.anchor()}>
        {veil}{title}{population}
    </g>)
}
