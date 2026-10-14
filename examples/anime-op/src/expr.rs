//! The cast's expression sheet as code: each preset clears the previous expression (keeping
//! the hands and switches) and draws a new one. Shots key them over time with
//! [`fframes_anime::acting::Acting`].

use fframes_anime::glam::Vec2;
use fframes_anime::toon::{Look, Mouth};

pub use fframes_anime::acting::Acting;

/// The face at rest, keeping hands, glint and switches of `l`.
pub fn neutral(l: Look) -> Look {
    Look {
        hand_l: l.hand_l,
        hand_r: l.hand_r,
        face: l.face,
        glint: l.glint,
        accent_eyes: l.accent_eyes,
        ..Look::default()
    }
}

pub fn happy(l: Look) -> Look {
    Look {
        squint: 0.6,
        brows: 0.3,
        mouth: Mouth::Smile,
        ..neutral(l)
    }
}

pub fn laugh(l: Look) -> Look {
    Look {
        eyes: 0.0,
        happy: true,
        brows: 0.5,
        mouth: Mouth::Open(0.8),
        blush: 0.4,
        ..neutral(l)
    }
}

/// Eyes shut in bliss (a bite of cream puff).
pub fn bliss(l: Look) -> Look {
    Look {
        eyes: 0.0,
        happy: true,
        brows: 0.4,
        mouth: Mouth::Smile,
        blush: 0.7,
        ..neutral(l)
    }
}

pub fn angry(l: Look) -> Look {
    Look {
        brow_angle: 1.0,
        pupils: 0.75,
        mouth: Mouth::Teeth(0.6),
        vein: 1.0,
        ..neutral(l)
    }
}

/// Shouting with everything (a battle cry).
pub fn shout(l: Look) -> Look {
    Look {
        brow_angle: 1.0,
        squint: 0.3,
        pupils: 0.7,
        mouth: Mouth::Open(1.0),
        ..neutral(l)
    }
}

pub fn shocked(l: Look) -> Look {
    Look {
        pupils: 0.4,
        brows: 1.0,
        mouth: Mouth::O(0.8),
        gloom: 0.9,
        sweat: 1.0,
        ..neutral(l)
    }
}

pub fn surprised(l: Look) -> Look {
    Look {
        pupils: 0.6,
        brows: 1.0,
        mouth: Mouth::O(0.5),
        ..neutral(l)
    }
}

pub fn smug(l: Look) -> Look {
    Look {
        lids: 0.6,
        brow_tilt: 0.7,
        mouth: Mouth::Smirk(1.0),
        ..neutral(l)
    }
}

pub fn worried(l: Look) -> Look {
    Look {
        brow_angle: -1.0,
        mouth: Mouth::Wavy,
        sweat: 0.8,
        ..neutral(l)
    }
}

pub fn determined(l: Look) -> Look {
    Look {
        brow_angle: 0.6,
        squint: 0.45,
        pupils: 0.85,
        mouth: Mouth::Line,
        ..neutral(l)
    }
}

/// Lifting something too heavy.
pub fn strain(l: Look) -> Look {
    Look {
        eyes: 0.55,
        brow_angle: 1.0,
        squint: 0.8,
        mouth: Mouth::Teeth(1.0),
        vein: 0.9,
        sweat: 0.7,
        ..neutral(l)
    }
}

pub fn excited(l: Look) -> Look {
    Look {
        sparkle: 1.0,
        pupils: 1.25,
        brows: 0.6,
        mouth: Mouth::Open(0.6),
        blush: 0.5,
        ..neutral(l)
    }
}

pub fn bored(l: Look) -> Look {
    Look {
        lids: 1.0,
        gaze: Vec2::new(0.0, -0.3),
        brows: -0.4,
        mouth: Mouth::Line,
        ..neutral(l)
    }
}

pub fn shy(l: Look) -> Look {
    Look {
        blush: 1.0,
        gaze: Vec2::new(0.3, -0.8),
        lids: 0.3,
        mouth: Mouth::Pout,
        ..neutral(l)
    }
}

/// Sets where the eyes look (keeps the rest).
pub fn gaze(x: f32, y: f32) -> impl FnOnce(Look) -> Look {
    move |l| Look {
        gaze: Vec2::new(x, y),
        ..l
    }
}

/// Changes only the mouth.
pub fn mouth(m: Mouth) -> impl FnOnce(Look) -> Look {
    move |l| Look { mouth: m, ..l }
}

/// Chewing from `from` to `to`: the jaw opens and closes every `every` seconds.
pub fn chew(a: Acting, from: f32, to: f32, every: f32) -> Acting {
    let mut track = Vec::new();
    let mut t = from;
    let mut open = true;
    while t < to {
        track.push((
            t,
            if open {
                Mouth::Talk {
                    open: 0.35,
                    wide: 0.45,
                }
            } else {
                Mouth::Smile
            },
        ));
        open = !open;
        t += every;
    }
    track.push((to, Mouth::Smile));
    a.lips(&track)
}

/// Talking from `from` to `to`: syllables of random vowels about `rate` per second, with short
/// pauses.
pub fn talk(a: Acting, seed: u32, from: f32, to: f32, rate: f32) -> Acting {
    let vowels = ['a', 'i', 'u', 'e', 'o', 'a', 'o'];
    let mut track = Vec::new();
    let mut t = from;
    let mut n = seed;
    while t < to {
        n = n.wrapping_mul(1_103_515_245).wrapping_add(12_345);
        let v = vowels[(n >> 16) as usize % vowels.len()];
        let pause = (n >> 8).is_multiple_of(7);
        track.push((t, Mouth::vowel(if pause { 'n' } else { v })));
        t += (0.6 + ((n >> 4) % 5) as f32 * 0.2) / rate;
        track.push((t - 0.05, Mouth::vowel('n')));
    }
    track.push((to, Mouth::vowel('n')));
    a.lips(&track)
}

/// Laughing from `from` to `to`: "ha ha ha", the jaw dropping `rate` times a second.
pub fn ha(a: Acting, from: f32, to: f32, rate: f32) -> Acting {
    let mut track = Vec::new();
    let mut t = from;
    let mut wide = true;
    while t < to {
        track.push((t, Mouth::Open(if wide { 1.0 } else { 0.5 })));
        wide = !wide;
        t += 0.5 / rate;
    }
    a.lips(&track)
}
