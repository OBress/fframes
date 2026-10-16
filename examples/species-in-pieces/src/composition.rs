//! Stable compositions chosen against the complete movement of each animal.
//! Reflect the habitat, animal and interaction props together so contact points
//! remain attached when the subject occupies the opposite third of the frame.

use fframes::{Color, Svgr, Transform, svgr};

// Reference navigation screen: white headline, charcoal, small coral accents.
pub(crate) const ENDING_BACKGROUND: Color = Color::rgb(31, 31, 31);

#[derive(Clone, Copy, Debug)]
pub(crate) enum Alignment {
    Left,
    Center,
    Right,
}

impl Alignment {
    pub fn anchor(self) -> &'static str {
        match self {
            Self::Left => "start",
            Self::Center => "middle",
            Self::Right => "end",
        }
    }

    pub fn left(self, x: f32, width: f32) -> f32 {
        match self {
            Self::Left => x,
            Self::Center => x - width / 2.,
            Self::Right => x - width,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Composition {
    pub reflected: bool,
    pub x: f32,
    /// Baseline of the first title line, in the final screen coordinates.
    pub y: f32,
    pub width: usize,
    pub size: usize,
    pub alignment: Alignment,
    pub ink: &'static str,
    /// A soft local veil only where forest details compete with the lettering.
    pub veil: f32,
    pub veil_color: &'static str,
    pub entrance: f32,
}

impl Composition {
    pub fn map(self, (x, y): (f32, f32)) -> (f32, f32) {
        (if self.reflected { 1920. - x } else { x }, y)
    }

    pub fn stage(self, content: Svgr<'static>) -> Svgr<'static> {
        if self.reflected {
            svgr!(<g transform={Transform {
                translate_x: 1920., scale: (-1., 1.).into(), ..Default::default()
            }}>{content}</g>)
        } else {
            content
        }
    }
}

const THIRD_Y: f32 = 1080. / 3.;
const BASE: Composition = Composition {
    reflected: false,
    x: 192.,
    y: THIRD_Y,
    width: 620,
    size: 96,
    alignment: Alignment::Left,
    ink: "#fff3d8",
    veil: 0.,
    veil_color: "#163c40",
    entrance: 2.25,
};

pub(crate) const COMPOSITIONS: [Composition; 18] = [
    // Hornbill: the sky opening above the perch, opposite the bird's head.
    Composition {
        x: 700.,
        y: 145.,
        width: 420,
        size: 90,
        ink: "#194b43",
        ..BASE
    },
    // Vaquita dives to the lower left. Keep the upper right water open for type.
    Composition {
        x: 1690.,
        y: 140.,
        alignment: Alignment::Right,
        ink: "#123e55",
        size: 104,
        ..BASE
    },
    // Tamarin: a lower golden-section block, beneath the diagonal branch.
    Composition {
        x: 540.,
        y: 735.,
        width: 650,
        size: 90,
        ink: "#33283a",
        ..BASE
    },
    // Frog: a single line in the sky opening above its left-third silhouette.
    Composition {
        reflected: true,
        x: 480.,
        y: 145.,
        width: 650,
        size: 80,
        ink: "#123f43",
        ..BASE
    },
    // Owlet: the setting sun leads diagonally from the upper-left title to the bird.
    Composition {
        x: 240.,
        y: 135.,
        size: 90,
        ink: "#4d3040",
        ..BASE
    },
    // Turtle: separate lower openings hold the map and name below the flipper sweep.
    Composition {
        x: 1710.,
        y: 740.,
        width: 650,
        size: 88,
        alignment: Alignment::Right,
        ink: "#123e55",
        ..BASE
    },
    // Oryx: sky above the acacia, outside the horns' complete browsing arc.
    Composition {
        x: 200.,
        y: 150.,
        width: 700,
        size: 90,
        ink: "#633d32",
        ..BASE
    },
    // Iguana: the quiet sky opening above its diagonal branch and raised head.
    Composition {
        reflected: true,
        x: 440.,
        y: 130.,
        width: 740,
        size: 84,
        ink: "#164a43",
        ..BASE
    },
    // Seahorse: negative water space ahead of the snout at the golden height.
    Composition {
        x: 550.,
        y: 280.,
        width: 600,
        size: 90,
        ink: "#103c3b",
        ..BASE
    },
    // Armadillo: the shell and title form an upward diagonal across the frame.
    Composition {
        reflected: true,
        x: 1450.,
        y: 250.,
        width: 730,
        size: 80,
        alignment: Alignment::Right,
        ink: "#374451",
        ..BASE
    },
    // Sloth: the open sky above the rising branch, after its slow assembly.
    Composition {
        x: 660.,
        y: 115.,
        width: 690,
        size: 82,
        ink: "#254a3f",
        entrance: 6.35,
        ..BASE
    },
    // Kakapo: name in the dark left clearing, above the walking and feeding path.
    Composition {
        x: 500.,
        y: 230.,
        size: 106,
        ..BASE
    },
    // Echidna: the pale mist provides a quiet diagonal opposite the low subject.
    Composition {
        x: 1110.,
        y: 135.,
        width: 690,
        alignment: Alignment::Center,
        ink: "#344957",
        ..BASE
    },
    // Penguin: left-third silhouette balanced by type above the right-hand horizon.
    Composition {
        reflected: true,
        x: 1700.,
        y: 145.,
        width: 690,
        alignment: Alignment::Right,
        ink: "#374b54",
        ..BASE
    },
    // Damsel: the stream's diagonal connects the upper-left title to the reed.
    Composition {
        x: 670.,
        y: 185.,
        width: 650,
        ink: "#35553d",
        ..BASE
    },
    // Bear: the animal looks toward the title in the upper golden-section clearing.
    Composition {
        reflected: true,
        x: 1320.,
        y: 170.,
        size: 96,
        alignment: Alignment::Right,
        ink: "#193d37",
        ..BASE
    },
    // Parrotfish: two short lines in open water above the left coral fan.
    Composition {
        x: 230.,
        y: 170.,
        width: 600,
        ink: "#10415a",
        ..BASE
    },
    // Camel: a left-hand mass and high right title balance the mountain diagonal.
    Composition {
        reflected: true,
        x: 1710.,
        y: 115.,
        width: 700,
        size: 90,
        alignment: Alignment::Right,
        ink: "#623e40",
        ..BASE
    },
];
