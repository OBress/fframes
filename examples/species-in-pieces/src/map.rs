//! Independent cartographic compositions, balanced against each moving animal.
//! Every coast and animated leader is a numeric `Path`, never an SVG string.

use std::sync::OnceLock;

use fframes::animation::{AnimationRuntime, Easing};
use fframes::cli::clap;
use fframes::usvgr::svgtree::{SvgAttributeValue, svgrtypes::PathSegment};
use fframes::{FFramesContext, FontQuery, Frame, Svgr, Transform, svgr};

use crate::composition::COMPOSITIONS;
use crate::geometry::Path as Triangle;

#[derive(Clone, Copy, Debug)]
pub(crate) struct Path<'a> {
    pub contours: &'a [&'a [(f32, f32)]],
    pub closed: bool,
}

impl From<Path<'_>> for SvgAttributeValue<'_> {
    fn from(path: Path<'_>) -> Self {
        let mut segments = Vec::new();
        for contour in path.contours {
            if let Some((&(x, y), rest)) = contour.split_first() {
                segments.push(PathSegment::MoveTo {
                    abs: true,
                    x: x.into(),
                    y: y.into(),
                });
                segments.extend(rest.iter().map(|&(x, y)| PathSegment::LineTo {
                    abs: true,
                    x: x.into(),
                    y: y.into(),
                }));
                if path.closed {
                    segments.push(PathSegment::ClosePath { abs: true });
                }
            }
        }
        segments.into()
    }
}

/// Alternate treatments of the regional habitat drawings.
#[derive(Clone, Copy, Debug, Default, clap::ValueEnum)]
pub enum MapStyle {
    Coastline,
    #[default]
    Atlas,
    Facets,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct RegionalGeometry {
    pub land: Path<'static>,
    pub coast: Path<'static>,
    pub borders: Path<'static>,
    pub pin: (f32, f32),
    pub height: f32,
}

const LABELS: [&[&str]; 18] = [
    &["BORNEO"],
    &["GULF OF", "CALIFORNIA"],
    &["ATLANTIC", "FOREST"],
    &["PACIFIC", "COLOMBIA"],
    &["CENTRAL INDIA"],
    &["GULF OF", "MEXICO"],
    &["CHAD"],
    &["FIJI"],
    &["KNYSNA"],
    &["NORTHEAST", "BRAZIL"],
    &["PANAMA"],
    &["NEW", "ZEALAND"],
    &["NEW GUINEA"],
    &["SOUTH AFRICAN", "CAPE"],
    &["CORFU"],
    &["BORNEO"],
    &["CARIBBEAN"],
    &["GOBI DESERT"],
];

#[derive(Clone, Copy)]
struct MapComposition {
    x: f32,
    y: f32,
    width: f32,
    ink: &'static str,
    paper: &'static str,
    light: bool,
}

const fn atlas(x: f32, y: f32, width: f32, paper: &'static str) -> MapComposition {
    MapComposition {
        x,
        y,
        width,
        ink: "#f0f2dc",
        paper,
        light: true,
    }
}

const fn daylight(x: f32, y: f32, width: f32, paper: &'static str) -> MapComposition {
    MapComposition {
        x,
        y,
        width,
        ink: "#254b4d",
        paper,
        light: false,
    }
}

// Independent screen compositions. The geography is a large, quiet regional
// drawing, with its label near the habitat rather than below a world-map stamp.
const MAP_COMPOSITIONS: [MapComposition; 18] = [
    atlas(490., 700., 690., "#2e625f"),
    daylight(150., 95., 640., "#7bd1d1"),
    MapComposition {
        ink: "#574658",
        ..daylight(1120., 735., 650., "#b89aab")
    },
    atlas(1000., 300., 650., "#417f80"),
    atlas(340., 350., 650., "#9c758b"),
    daylight(430., 645., 620., "#4baeb4"),
    daylight(390., 320., 580., "#d0b0a0"),
    atlas(1050., 665., 660., "#286d8a"),
    atlas(650., 500., 660., "#4a9b8e"),
    atlas(1130., 430., 620., "#a68b95"),
    atlas(670., 710., 700., "#328e8b"),
    atlas(300., 405., 580., "#24473d"),
    atlas(340., 265., 650., "#748f9b"),
    atlas(1110., 275., 640., "#568c9d"),
    atlas(900., 620., 650., "#409c9d"),
    atlas(1000., 300., 650., "#719477"),
    atlas(760., 650., 650., "#205d78"),
    atlas(1160., 330., 620., "#a28da5"),
];

#[derive(Debug)]
struct RegionalDrawing {
    land: Svgr<'static>,
    coast: Svgr<'static>,
    borders: Svgr<'static>,
}

#[derive(Debug)]
pub(crate) struct HabitatMaps {
    drawings: [RegionalDrawing; 18],
    label_widths: [OnceLock<f32>; 18],
    facets: Svgr<'static>,
    reveal: AnimationRuntime,
    annotation: AnimationRuntime,
}

impl Default for HabitatMaps {
    fn default() -> Self {
        let drawings = std::array::from_fn(|index| {
            let region = crate::regional_map_data::REGIONS[index];
            RegionalDrawing {
                land: svgr!(<path d={region.land} fill-rule="evenodd" />),
                coast: svgr!(<path d={region.coast} />),
                borders: svgr!(<path d={region.borders} />),
            }
        });
        let mut facets = Vec::new();
        for row in 0..7 {
            for column in 0..10 {
                let a = (column as f32 * 60., row as f32 * 60.);
                let b = (a.0 + 60., a.1);
                let c = (a.0 + 60., a.1 + 60.);
                let alpha = 0.02 + ((row * 13 + column * 7) % 7) as f32 * 0.008;
                facets.push(svgr!(<path d={Triangle { points: [a, b, c] }} opacity={alpha} />));
            }
        }
        Self {
            drawings,
            label_widths: std::array::from_fn(|_| OnceLock::new()),
            facets: facets.into_iter().collect(),
            reveal: AnimationRuntime::new(1.1, &Easing::CubicBezier(0.16, 1., 0.3, 1.)),
            annotation: AnimationRuntime::new(0.75, &Easing::EaseOut),
        }
    }
}

impl HabitatMaps {
    pub fn backdrop(&self, index: usize, local: f32) -> Svgr<'static> {
        let elapsed = local - COMPOSITIONS[index].entrance - 0.65;
        if elapsed <= 0. {
            return Svgr::empty();
        }
        let layout = MAP_COMPOSITIONS[index];
        let reveal = self.reveal.solve(&elapsed.clamp(0., 1.1)).clamp(0., 1.);
        let height = crate::regional_map_data::REGIONS[index].height * layout.width / 600.;
        let veil = format!("map-paper-{index}");
        svgr!(<g transform={Transform::translate(layout.x, layout.y)} opacity={reveal}>
            <defs><radialGradient id={veil.clone()}>
                <stop offset="0" stop-color={layout.paper} stop-opacity="0.66" />
                <stop offset="0.5" stop-color={layout.paper} stop-opacity="0.46" />
                <stop offset="1" stop-color={layout.paper} stop-opacity="0" />
            </radialGradient></defs>
            <ellipse cx={layout.width * 0.5} cy={height * 0.5}
                rx={layout.width * 0.7} ry={height * 0.75} fill={format!("url(#{veil})")} />
        </g>)
    }

    pub fn render(
        &self,
        index: usize,
        local: f32,
        style: MapStyle,
        frame: &mut Frame,
        ctx: &FFramesContext<'_, '_>,
    ) -> Svgr<'static> {
        let layout = MAP_COMPOSITIONS[index];
        let elapsed = local - COMPOSITIONS[index].entrance - 0.65;
        if elapsed <= 0. {
            return Svgr::empty();
        }
        let reveal = self.reveal.solve(&elapsed.clamp(0., 1.1)).clamp(0., 1.);
        let annotation = self.annotation.solve(&(elapsed - 0.3).clamp(0., 0.75));
        let drawing = &self.drawings[index];
        let region = crate::regional_map_data::REGIONS[index];
        let pin = region.pin;
        let height = region.height;
        let scale = layout.width / 600.;
        let accent = if layout.light { "#f59d88" } else { "#a83e49" };
        let fade_x = format!("map-fade-x-{index}");
        let fade_y = format!("map-fade-y-{index}");
        let mask_x = format!("map-mask-x-{index}");
        let mask_y = format!("map-mask-y-{index}");
        let label_mask = format!("map-label-clearance-{index}");
        let land_clip = format!("map-land-{index}");
        let (fill, coast, borders, grid) = match style {
            MapStyle::Coastline => (0., 0.64, 0., 0.),
            MapStyle::Atlas => (0.045, 0.62, 0.12, 0.),
            MapStyle::Facets => (0.16, 0.45, 0.1, 0.08),
        };
        let facets = if matches!(style, MapStyle::Facets) {
            self.facets.clone()
        } else {
            Svgr::empty()
        };
        let label_width = if let Some(width) = self.label_widths[index].get() {
            *width
        } else if let Some(width) = measure_label(index, frame, ctx) {
            *self.label_widths[index].get_or_init(|| width)
        } else {
            280.
        };
        let offset = label_offset(index);
        let label_x = pin.0
            + if pin.0 > 350. {
                -offset - label_width
            } else {
                offset
            };
        let label_height = 28. + (LABELS[index].len() - 1) as f32 * 32.;
        let label_top = pin.1 - 37. - label_height;
        let label = location_label(index, pin, layout.ink, accent, annotation);
        let graticule = Path {
            contours: &[
                &[(0., 100.), (600., 100.)],
                &[(0., 200.), (600., 200.)],
                &[(0., 300.), (600., 300.)],
                &[(150., 0.), (150., height)],
                &[(300., 0.), (300., height)],
                &[(450., 0.), (450., height)],
            ],
            closed: false,
        };
        svgr!(<g id="habitat-map" transform={Transform::translate(layout.x, layout.y)} opacity={reveal}>
            <g transform={Transform::scale(scale)}>
                <defs>
                    <linearGradient id={fade_x.clone()} x1="0" y1="0" x2="1" y2="0">
                        <stop offset="0" stop-color="#000" />
                        <stop offset="0.12" stop-color="#fff" />
                        <stop offset="0.88" stop-color="#fff" />
                        <stop offset="1" stop-color="#000" />
                    </linearGradient>
                    <linearGradient id={fade_y.clone()} x1="0" y1="0" x2="0" y2="1">
                        <stop offset="0" stop-color="#000" />
                        <stop offset="0.12" stop-color="#fff" />
                        <stop offset="0.88" stop-color="#fff" />
                        <stop offset="1" stop-color="#000" />
                    </linearGradient>
                    <mask id={mask_x.clone()} maskUnits="userSpaceOnUse" x="0" y="0" width="600" height={height}>
                        <rect width="600" height={height} fill={format!("url(#{fade_x})")} />
                    </mask>
                    <mask id={mask_y.clone()} maskUnits="userSpaceOnUse" x="0" y="0" width="600" height={height}>
                        <rect width="600" height={height} fill={format!("url(#{fade_y})")} />
                    </mask>
                    <clipPath id={land_clip.clone()}>{drawing.land.clone()}</clipPath>
                    <mask id={label_mask.clone()} maskUnits="userSpaceOnUse" x="0" y="0" width="600" height={height}>
                        <rect width="600" height={height} fill="#fff" />
                        <rect x={label_x - 8.} y={label_top - 4.} width={label_width + 16.}
                            height={label_height + 10.} rx="4" fill="#000" />
                    </mask>
                </defs>
                <g mask={format!("url(#{mask_x})")}>
                    <g mask={format!("url(#{mask_y})")}>
                        <g fill={layout.ink} opacity={fill}>{drawing.land.clone()}</g>
                        <g fill={layout.ink} clip-path={format!("url(#{land_clip})")}>{facets}</g>
                        <g mask={format!("url(#{label_mask})")}>
                            <path d={graticule} fill="none" stroke={layout.ink} stroke-width="0.8" opacity={grid} />
                            <g fill="none" stroke={layout.ink} stroke-width="0.85" opacity={borders}>{drawing.borders.clone()}</g>
                            <g fill="none" stroke={layout.ink} stroke-width="1.3" stroke-linejoin="round" opacity={coast}>{drawing.coast.clone()}</g>
                        </g>
                    </g>
                </g>
                {label}
            </g>
        </g>)
    }
}

fn measure_label(index: usize, frame: &mut Frame, ctx: &FFramesContext<'_, '_>) -> Option<f32> {
    let font = FontQuery {
        family: "DM Sans",
        size: 28,
        weight: 500,
        ..Default::default()
    };
    let mut width = 0_f32;
    for line in LABELS[index] {
        let glyphs = frame.text_width(ctx, font, line)? as f32;
        width = width.max(glyphs + line.chars().count().saturating_sub(1) as f32 * 2.);
    }
    Some(width)
}

fn label_offset(index: usize) -> f32 {
    // Keep the New Zealand label east of the narrow South Island silhouette.
    if index == 11 { 84. } else { 44. }
}

fn location_label(
    index: usize,
    pin: (f32, f32),
    ink: &'static str,
    accent: &'static str,
    alpha: f32,
) -> Svgr<'static> {
    let left = pin.0 > 350.;
    let direction = if left { -1. } else { 1. };
    let offset = label_offset(index);
    let label_x = pin.0 + direction * offset;
    let lines = LABELS[index];
    let baseline = pin.1 - 37.;
    let leader = Path {
        contours: &[&[
            (pin.0 + direction * 10., pin.1 - 5.),
            (pin.0 + direction * 29., pin.1 - 27.),
            (pin.0 + direction * (offset + 16.), pin.1 - 27.),
        ]],
        closed: false,
    };
    let words: Svgr<'static> = lines
        .iter()
        .enumerate()
        .map(|(i, line)| {
            svgr!(<text x={label_x} y={baseline - (lines.len() - 1 - i) as f32 * 32.}
            text-anchor={if left { "end" } else { "start" }} font-family="DM Sans" font-size="28"
            font-weight="500" letter-spacing="2" fill={ink}>{*line}</text>)
        })
        .collect();
    svgr!(<g opacity={alpha}>
        <path d={leader} fill="none" stroke={ink} stroke-width="1.2" opacity="0.65" />
        <circle cx={pin.0} cy={pin.1} r="9" fill="none" stroke={ink} stroke-width="1.1" opacity="0.7" />
        <circle cx={pin.0} cy={pin.1} r="4.2" fill={accent} />
        <g opacity="0.92">{words}</g>
    </g>)
}
