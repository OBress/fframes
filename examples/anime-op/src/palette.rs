//! Colours sampled from the reference opening.

pub const YELLOW: &str = "#F6D805";
pub const YELLOW_DEEP: &str = "#EACA03";
pub const INK: &str = "#0E0B08";
pub const ORANGE: &str = "#FA6C08";
pub const MAGENTA: &str = "#C40764";
pub const MAGENTA_DARK: &str = "#A40850";
pub const PURPLE: &str = "#350E98";
pub const GREEN: &str = "#0B6801";
pub const GREEN_LIGHT: &str = "#5EA315";
pub const TEAL: &str = "#27A69C";
pub const TEAL_DARK: &str = "#103B45";
pub const RED: &str = "#A92C23";
pub const BLUE: &str = "#3F469C";
pub const OLIVE: &str = "#B5A636";
pub const MOSS: &str = "#356B2D";
pub const TITLE_RED: &str = "#E3152B";
pub const CREAM: &str = "#FFE9A8";
pub const PUFF: &str = "#F08A1C";
pub const PUFF_DARK: &str = "#B5530E";
pub const WHITE: &str = "#FFF8E1";

/// How a character is coloured in a shot.
#[derive(Debug, Clone, Copy)]
pub struct Paint {
    /// Suit, robe and tie.
    pub suit: &'static str,
    /// Dark hair and dark details.
    pub ink: &'static str,
    /// Skin, shirt, light lines.
    pub paper: &'static str,
    /// Outlines of light shapes and face features.
    pub line: &'static str,
    /// Hair colour override (ginger, white...), `None` = ink.
    pub hair: Option<&'static str>,
    pub beard: Option<&'static str>,
    pub lining: Option<&'static str>,
    /// The robe's outside when it differs from the suit under it.
    pub robe: Option<&'static str>,
    pub scarf: Option<&'static str>,
    /// Accent override (eyes that glow, emblems).
    pub accent: Option<&'static str>,
    /// How much darker the cel shadows are (0 = flat colours).
    pub shade: f32,
    /// Where shadows start (`n · light`), higher is more shadow.
    pub terminator: f32,
}

impl Paint {
    /// The Mashle look: black suit and hair, faces and shirts in the background yellow.
    pub fn two_tone(bg: &'static str) -> Self {
        Paint {
            suit: INK,
            ink: INK,
            paper: bg,
            line: INK,
            hair: None,
            beard: None,
            lining: None,
            robe: None,
            scarf: None,
            accent: None,
            shade: 0.12,
            terminator: -0.26,
        }
    }

    /// Line art: light fills with dark outlines everywhere (close-ups in the reference).
    pub fn line_art(bg: &'static str) -> Self {
        Paint {
            suit: bg,
            ink: INK,
            paper: bg,
            line: INK,
            hair: None,
            beard: None,
            lining: None,
            robe: None,
            scarf: None,
            accent: None,
            shade: 0.1,
            terminator: -0.26,
        }
    }

    /// Everything in one colour.
    pub fn silhouette(color: &'static str) -> Self {
        Paint {
            suit: color,
            ink: color,
            paper: color,
            line: color,
            hair: Some(color),
            beard: Some(color),
            lining: Some(color),
            robe: Some(color),
            scarf: Some(color),
            accent: None,
            shade: 0.0,
            terminator: -0.26,
        }
    }
}

impl Paint {
    pub fn with_suit(mut self, suit: &'static str) -> Self {
        self.suit = suit;
        self
    }

    pub fn with_hair(mut self, hair: &'static str) -> Self {
        self.hair = Some(hair);
        self
    }

    pub fn with_accent(mut self, accent: &'static str) -> Self {
        self.accent = Some(accent);
        self
    }

    pub fn with_lining(mut self, lining: &'static str) -> Self {
        self.lining = Some(lining);
        self
    }

    pub fn with_robe(mut self, robe: &'static str) -> Self {
        self.robe = Some(robe);
        self
    }

    pub fn with_shade(mut self, shade: f32) -> Self {
        self.shade = shade;
        self
    }
}
