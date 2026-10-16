//! Numeric SVG geometry. The morph never builds or parses an SVG `d` string.
//!
//! A triangle has a fixed topology: `MoveTo`, `LineTo`, `LineTo`, `ClosePath`. Keeping
//! the same three point identities lets fframes interpolate the entire path.

use fframes::Color;
use fframes::animation::Animatable;
use fframes::usvgr::svgtree::{SvgAttributeValue, svgrtypes::PathSegment};

/// A closed, three-vertex SVG path, in the illustration's 100 × 100 coordinates.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Path {
    pub points: [(f32, f32); 3],
}

impl Path {
    pub fn center(self) -> (f32, f32) {
        let [(ax, ay), (bx, by), (cx, cy)] = self.points;
        ((ax + bx + cx) / 3., (ay + by + cy) / 3.)
    }

    /// Apply a numeric deformation, preserving vertex correspondence.
    pub fn map(self, f: impl Fn((f32, f32)) -> (f32, f32)) -> Self {
        Self {
            points: self.points.map(f),
        }
    }

    pub fn translated(self, x: f32, y: f32) -> Self {
        self.map(|(px, py)| (px + x, py + y))
    }

    pub fn rotated(self, radians: f32) -> Self {
        let (cx, cy) = self.center();
        let (sin, cos) = radians.sin_cos();
        self.map(|(x, y)| {
            let (x, y) = (x - cx, y - cy);
            (cx + x * cos - y * sin, cy + x * sin + y * cos)
        })
    }
}

impl Animatable for Path {
    fn apply_progress(&self, to: &Self, progress: f32) -> Self {
        Self {
            points: std::array::from_fn(|i| self.points[i].apply_progress(&to.points[i], progress)),
        }
    }
}

// This is the boundary between our editable geometry and svgr!. It hands the
// renderer typed commands directly, without format!(), Display or an SVG parser.
impl From<Path> for SvgAttributeValue<'_> {
    fn from(path: Path) -> Self {
        let [(ax, ay), (bx, by), (cx, cy)] = path.points;
        vec![
            PathSegment::MoveTo {
                abs: true,
                x: f64::from(ax),
                y: f64::from(ay),
            },
            PathSegment::LineTo {
                abs: true,
                x: f64::from(bx),
                y: f64::from(by),
            },
            PathSegment::LineTo {
                abs: true,
                x: f64::from(cx),
                y: f64::from(cy),
            },
            PathSegment::ClosePath { abs: true },
        ]
        .into()
    }
}

/// A persistent piece changes both its geometry and its pigment during a morph.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Piece {
    pub path: Path,
    pub color: Color,
}

impl Animatable for Piece {
    fn apply_progress(&self, to: &Self, progress: f32) -> Self {
        Self {
            path: self.path.apply_progress(&to.path, progress),
            color: self.color.apply_progress(&to.color, progress),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interpolated_paths_reach_their_targets_and_remain_typed() {
        let a = Path {
            points: [(0., 0.), (10., 0.), (0., 10.)],
        };
        let b = Path {
            points: [(8., 4.), (30., 6.), (6., 20.)],
        };
        assert_eq!(a.apply_progress(&b, 0.), a);
        assert_eq!(a.apply_progress(&b, 1.), b);
        let halfway = a.apply_progress(&b, 0.5);
        assert_eq!(halfway.points, [(4., 2.), (20., 3.), (3., 15.)]);
        let SvgAttributeValue::PathData(commands) = halfway.into() else {
            panic!("a morph must pass typed commands to the renderer");
        };
        assert_eq!(commands.len(), 4);
        assert!(matches!(commands[0], PathSegment::MoveTo { .. }));
        assert!(matches!(commands[3], PathSegment::ClosePath { .. }));
    }
}
