//! Acting: a character's expressions over time, the way a key animator plans them. Expression
//! keys (表情) change through one in-between, eyes blink at natural intervals (and to cover a
//! change of expression), dart between points of attention, and the mouth follows a lip track.
//! Everything is sampled at the time of the drawing, so it holds on twos with the body.

use glam::Vec2;

use crate::toon::{Look, Mouth};

/// Seconds a blink takes: half closed, closed, half open (a drawing each, on twos).
pub const BLINK: f32 = 6.0 / 24.0;

#[derive(Debug, Clone)]
pub struct Acting {
    keys: Vec<(f32, Look)>,
    /// How long an expression change shows its in-between (seconds).
    pub change: f32,
    blinks: Option<Blinks>,
    darts: Option<Darts>,
    lips: Vec<(f32, Mouth)>,
}

#[derive(Debug, Clone)]
struct Blinks {
    seed: u32,
    /// Mean seconds between blinks.
    every: f32,
    /// Extra blinks (shot seconds), e.g. on a beat or a change of mind.
    at: Vec<f32>,
    /// Blink when the expression changes.
    on_change: bool,
}

#[derive(Debug, Clone, Copy)]
struct Darts {
    seed: u32,
    amount: f32,
    every: f32,
}

fn hash(a: u32, b: u32) -> f32 {
    let mut h = a.wrapping_mul(0x9E37_79B9) ^ b.wrapping_mul(0x85EB_CA6B);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    h = h.wrapping_mul(0x297A_2D39);
    h ^= h >> 15;
    (h & 0x00FF_FFFF) as f32 / 16_777_216.0
}

impl Acting {
    pub fn new(base: Look) -> Self {
        Acting {
            keys: vec![(f32::NEG_INFINITY, base)],
            change: 2.0 / 24.0,
            blinks: None,
            darts: None,
            lips: Vec::new(),
        }
    }

    /// A new expression from `t` on, made from the one before it.
    pub fn then(mut self, t: f32, f: impl FnOnce(Look) -> Look) -> Self {
        let prev = self.base_at(t);
        self.keys.push((t, f(prev)));
        self.keys.sort_by(|a, b| a.0.total_cmp(&b.0));
        self
    }

    /// Natural blinks every `every` seconds on average (2.5 to 4 is calm, 1.2 is nervous), and
    /// one on every change of expression.
    pub fn blinks(mut self, seed: u32, every: f32) -> Self {
        self.blinks = Some(Blinks {
            seed,
            every: every.max(0.5),
            at: Vec::new(),
            on_change: true,
        });
        self
    }

    /// A blink at `t` (added to the natural ones).
    pub fn blink_at(mut self, t: f32) -> Self {
        let b = self.blinks.get_or_insert(Blinks {
            seed: 0,
            every: f32::INFINITY,
            at: Vec::new(),
            on_change: false,
        });
        b.at.push(t);
        self
    }

    /// The eyes jump between nearby points every `every` seconds on average, `amount` of the
    /// way to the corners.
    pub fn darts(mut self, seed: u32, amount: f32, every: f32) -> Self {
        self.darts = Some(Darts {
            seed,
            amount,
            every: every.max(0.2),
        });
        self
    }

    /// A lip track: each mouth from its time until the next one.
    pub fn lips(mut self, track: &[(f32, Mouth)]) -> Self {
        self.lips.extend_from_slice(track);
        self.lips.sort_by(|a, b| a.0.total_cmp(&b.0));
        self
    }

    /// Sings `vowels` (one per syllable, `a i u e o`, `n` or `-` for closed) starting at the
    /// given times: each syllable opens on its vowel and closes `hold` seconds later unless
    /// the next one comes first.
    pub fn sing(self, times: &[f32], vowels: &str, hold: f32) -> Self {
        let vs: Vec<char> = vowels.chars().filter(|c| !c.is_whitespace()).collect();
        let mut track = Vec::new();
        for (i, (t, v)) in times.iter().zip(vs.iter()).enumerate() {
            track.push((*t, Mouth::vowel(*v)));
            let next = times.get(i + 1).copied().unwrap_or(f32::INFINITY);
            if t + hold < next - 0.02 {
                track.push((t + hold, Mouth::vowel('n')));
            }
        }
        self.lips(&track)
    }

    fn base_at(&self, t: f32) -> Look {
        let i = self.keys.partition_point(|k| k.0 <= t).max(1) - 1;
        self.keys[i].1
    }

    /// The expression at `t` (a drawing time).
    pub fn at(&self, t: f32) -> Look {
        let i = self.keys.partition_point(|k| k.0 <= t).max(1) - 1;
        let (kt, key) = self.keys[i];
        let mut look = if i > 0 && t - kt < self.change {
            self.keys[i - 1].1.lerp(&key, 0.5)
        } else {
            key
        };
        if let Some(d) = self.darts {
            // the time line split into irregular fixations
            let slot = (t / d.every).floor();
            let jitter = hash(d.seed, slot as i32 as u32) * 0.8;
            let n = if t / d.every - slot < jitter {
                slot - 1.0
            } else {
                slot
            } as i32 as u32;
            if hash(d.seed ^ 0x55, n) > 0.25 {
                let g = Vec2::new(
                    hash(d.seed + 1, n) * 2.0 - 1.0,
                    (hash(d.seed + 2, n) * 2.0 - 1.0) * 0.6,
                );
                look.gaze += g * d.amount;
            }
        }
        if let Some((_, m)) = self.lips.iter().rev().find(|l| l.0 <= t) {
            look.mouth = *m;
        }
        if let Some(b) = &self.blinks {
            let blink = |s: f32| (0.0..BLINK).contains(&(t - s)).then(|| (t - s) / BLINK);
            let mut phase = b.at.iter().find_map(|s| blink(*s));
            if phase.is_none() && b.on_change && i > 0 {
                phase = blink(kt - BLINK * 0.3);
            }
            if phase.is_none() && b.every.is_finite() {
                // natural blinks: one in every slot of `every` seconds, at a random moment,
                // sometimes twice in a row
                let slot = (t / b.every).floor();
                let n = slot as i32 as u32;
                let s = (slot + 0.15 + 0.7 * hash(b.seed, n)) * b.every;
                phase = blink(s);
                if phase.is_none() && hash(b.seed ^ 0xAB, n) > 0.8 {
                    phase = blink(s + BLINK + 0.12);
                }
            }
            if let Some(u) = phase
                && look.eyes > 0.2
            {
                let k = if u < 0.33 {
                    0.4
                } else if u < 0.67 {
                    0.0
                } else {
                    0.5
                };
                look.eyes *= k;
            }
        }
        look
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expression_changes_through_an_inbetween() {
        let a = Acting::new(Look::default()).then(1.0, |l| Look { brows: 1.0, ..l });
        assert_eq!(a.at(0.5).brows, 0.0);
        assert!((a.at(1.02).brows - 0.5).abs() < 1e-6);
        assert_eq!(a.at(1.5).brows, 1.0);
    }

    #[test]
    fn blinks_close_the_eyes_briefly() {
        let a = Acting::new(Look::default()).blinks(3, 2.0);
        let closed = (0..240)
            .filter(|f| a.at(*f as f32 / 24.0).eyes < 0.1)
            .count();
        // about 5 blinks in 10 s, 2 closed frames each (a little more with double blinks)
        assert!((6..=16).contains(&closed), "{closed}");
    }

    #[test]
    fn singing_opens_and_closes() {
        let a = Acting::new(Look::default()).sing(&[0.0, 0.5], "ia", 0.2);
        assert!(matches!(a.at(0.1).mouth, Mouth::Talk { wide, .. } if wide > 0.9));
        assert!(matches!(a.at(0.3).mouth, Mouth::Talk { open, .. } if open < 0.05));
        assert!(matches!(a.at(0.55).mouth, Mouth::Talk { open, .. } if open > 0.9));
    }
}
