//! The Bling-Bang-Bang-Born dance: the solo in the letterbox and the chorus line.

use fframes::{FFramesContext, Frame, Svgr};
use fframes_anime::camera::Camera;
use fframes_anime::glam::{Vec2, Vec3};
use fframes_anime::toon::{HandShape, Look};

use crate::actor::{Actor, acting};
use crate::cast::{Who, member};
use crate::expr::Acting;
use crate::palette::{self, Paint};
use crate::procedural::bbbb;
use crate::{H, Shot, W};

pub struct Solo {
    hero: Actor,
}

impl Solo {
    pub fn new(start: f32, len: f32) -> Self {
        let hero = dancer(Who::Neogoose, start, len, Vec3::ZERO, true);
        Solo { hero }
    }
}

impl Shot for Solo {
    fn para(&self) -> f32 {
        0.5
    }

    fn render<'a>(
        &'a self,
        t: f32,
        _len: f32,
        _frame: &Frame,
        _ctx: &FFramesContext<'a, '_>,
    ) -> Svgr<'a> {
        // Waist-up and locked off like the reference: all the movement is in the drawings.
        let mut cam = Camera::new(
            Vec3::new(0.0, 1.4, 2.15),
            Vec3::new(0.0, 1.4, 0.0),
            30.0,
            W,
            H,
        );
        cam.shift = Vec2::new(-30.0, 60.0);
        let paint = Paint::two_tone(palette::YELLOW);
        fframes::svgr!(
            <g>
                <rect x="0" y="0" width="1920" height="1080" fill={palette::YELLOW} />
                {self.hero.draw(t, &cam, &paint)}
                <rect x="0" y="0" width="1920" height="130" fill={palette::INK} />
                <rect x="0" y="950" width="1920" height="130" fill={palette::INK} />
            </g>
        )
    }
}

/// The chorus line on magenta: the six in front, rows of black clones behind, all dancing.
pub struct Chorus {
    front: Vec<Actor>,
    back: Vec<Actor>,
}

impl Chorus {
    pub fn new(start: f32, len: f32) -> Self {
        let front = crate::cast::ALL
            .iter()
            .enumerate()
            .map(|(i, who)| {
                // shoulder to shoulder, fists nearly touching the next one's, like the
                // reference's wall of dancers
                let x = (i as f32 - 2.5) * 0.53;
                dancer(*who, start, len, Vec3::new(x, 0.0, 0.0), true)
            })
            .collect();
        let back = (0..10)
            .map(|i| {
                let row = i / 5;
                let x = (i % 5) as f32 * 0.85 - 1.7 + row as f32 * 0.42;
                dancer(
                    crate::cast::ALL[i % 6],
                    start,
                    len,
                    Vec3::new(x, 0.0, -1.3 - row as f32 * 1.2),
                    false,
                )
            })
            .collect();
        Chorus { front, back }
    }
}

impl Shot for Chorus {
    fn render<'a>(
        &'a self,
        t: f32,
        _len: f32,
        _frame: &Frame,
        _ctx: &FFramesContext<'a, '_>,
    ) -> Svgr<'a> {
        // waist-up across the line, all six in, the fists of the outer two at the frame edge
        let cam = Camera::new(
            Vec3::new(-0.02, 1.42, 3.2),
            Vec3::new(-0.02, 1.28, 0.0),
            32.0,
            W,
            H,
        );
        let back: Vec<&Actor> = self.back.iter().collect();
        let front: Vec<&Actor> = self.front.iter().collect();
        fframes::svgr!(
            <g>
                <rect x="0" y="0" width="1920" height="1080" fill={palette::MAGENTA} />
                {crate::stage::actors(&back, t, &cam, &Paint::silhouette(palette::MAGENTA_DARK))}
                <rect x="0" y="930" width="1920" height="150" fill={palette::INK} />
                {crate::stage::actors(&front, t, &cam, &Paint::two_tone(palette::YELLOW))}
            </g>
        )
    }
}

/// Rows of tiny dancers on purple between the lyrics, rows glitching sideways.
pub struct Rows {
    dancers: Vec<Actor>,
}

impl Rows {
    pub fn new(start: f32, len: f32) -> Self {
        let dancers = crate::cast::ALL
            .iter()
            .map(|who| dancer(*who, start, len, Vec3::ZERO, true))
            .collect();
        Rows { dancers }
    }
}

impl Shot for Rows {
    fn render<'a>(
        &'a self,
        t: f32,
        _len: f32,
        _frame: &Frame,
        _ctx: &FFramesContext<'a, '_>,
    ) -> Svgr<'a> {
        // A locked-off line: every dancer is its own cel placed side by side (seen straight
        // on, like the reference), the six drawings of a frame reused along the line.
        let paint = Paint::two_tone(palette::YELLOW);
        let mut cam = Camera::new(
            Vec3::new(0.0, 0.95, 5.6),
            Vec3::new(0.0, 0.95, 0.0),
            30.0,
            W,
            H,
        );
        cam.shift = Vec2::new(0.0, 30.0);
        let cels: Vec<Svgr<'a>> = self
            .dancers
            .iter()
            .map(|a| a.draw(t, &cam, &paint))
            .collect();
        let spacing = 215.0;
        let items: Vec<Svgr<'a>> = (0..11)
            .map(|k| {
                let x = 960.0 + (k as f32 - 5.0) * spacing - 960.0;
                // the hero once, in the middle of the line; the friends around him
                let cel = cels[if k == 5 { 0 } else { 1 + k % 5 }].clone();
                fframes::svgr!(<g transform={fframes::Transform::translate(x, 0.0)}>{cel}</g>)
            })
            .collect();
        fframes::svgr!(
            <g>
                <rect x="0" y="0" width="1920" height="1080" fill={palette::PURPLE} />
                <text x="40" y="360" font-family={crate::shots::title::BLACKLETTER} font-weight="700" font-size="215" fill={palette::YELLOW}>"Bling-Bang-Bang,"</text>
                {items}
                <text x="40" y="880" font-family={crate::shots::title::BLACKLETTER} font-weight="700" font-size="196" fill={palette::YELLOW} stroke={palette::INK} stroke-width="16" paint-order="stroke" stroke-linejoin="round">"Bling-Bang-Bang-Born"</text>
                <rect x="0" y="0" width="1920" height="150" fill={palette::INK} />
                <rect x="0" y="930" width="1920" height="150" fill={palette::INK} />
            </g>
        )
    }
}

/// A cast member doing the dance, timed by its timesheet, singing along.
fn dancer(who: Who, start: f32, len: f32, at: Vec3, face: bool) -> Actor {
    let sk = member(who).skeleton();
    let dance = bbbb(&sk, start, len, at, 0.0, 0.0);
    Actor::on_sheet(
        who,
        dance.motion,
        dance.sheet,
        dance.between,
        acting(singing(who, start, len, face)),
        len,
    )
}

/// "Bling-Bang-Bang, Bling-Bang-Bang, Bling-Bang-Bang-Born" on the vocal's half-beat grid
/// (read off the song's 500-2500 Hz onsets), each member in their own mood.
fn singing(who: Who, start: f32, len: f32, face: bool) -> Acting {
    const FIRST: f32 = 164.87;
    let pattern = "iaa-iaa-iaao";
    let mut times = Vec::new();
    let mut vowels = String::new();
    let mut borns = Vec::new();
    for (k, v) in pattern.chars().cycle().enumerate() {
        let t = crate::procedural::beat_time(FIRST + 0.5 * k as f32) - start;
        if t > len + 0.5 {
            break;
        }
        if t > -0.5 {
            times.push(t);
            vowels.push(v);
            if v == 'o' {
                borns.push(t);
            }
        }
    }
    // the fists are drawn big, like the reference's: they carry the dance
    let base = Look {
        face,
        hand_l: HandShape::Fist,
        hand_r: HandShape::Fist,
        fists: 1.3,
        ..Default::default()
    };
    let mood = match who {
        Who::Neogoose => Look {
            squint: 0.35,
            brows: 0.2,
            sparkle: 0.5,
            ..base
        },
        Who::Dillon => Look {
            lids: 0.55,
            brow_tilt: 0.5,
            ..base
        },
        Who::Shreyas => Look {
            eyes: 0.0,
            happy: true,
            blush: 0.4,
            ..base
        },
        Who::Theo => Look {
            brow_angle: 0.5,
            squint: 0.3,
            ..base
        },
        Who::Teej => Look {
            brows: 0.6,
            pupils: 1.2,
            sparkle: 0.8,
            ..base
        },
        _ => Look {
            squint: 0.5,
            brows: 0.3,
            ..base
        },
    };
    let mut a = Acting::new(mood)
        .sing(&times, &vowels, 0.13)
        .blinks(200 + who as u32, 2.2);
    // "Born!": a wink and a grin on the last syllable of the line
    for t in borns {
        let wink = if who == Who::Shreyas { 0.0 } else { -1.0 };
        a = a
            .then(t, move |l| Look {
                wink,
                sparkle: 1.0,
                ..l
            })
            .then(t + 0.3, move |_| mood);
    }
    a
}
