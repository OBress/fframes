//! Exposure timing. Japanese TV animation is drawn "on twos" or "on threes" (a drawing held for
//! 2 or 3 frames of 24 fps) while camera moves and effects run on ones; mixing the two is what
//! makes digital motion read as hand-drawn.

/// The time of the drawing shown at `t` when each drawing is held for `koma` frames at `fps`.
pub fn hold(t: f32, fps: f32, koma: u32) -> f32 {
    if koma <= 1 {
        return t;
    }
    let n = koma as f32;
    ((t * fps / n).floor() * n) / fps
}

/// An exposure sheet: which drawing (cel) is shown from which frame on, like a column of a
/// timesheet. Frames before the first entry show nothing.
#[derive(Debug, Clone, Default)]
pub struct Exposure {
    entries: Vec<(u32, Option<u32>)>,
}

impl Exposure {
    /// `entries` are `(frame, Some(cel))` or `(frame, None)` for an empty frame, in any order.
    pub fn new(mut entries: Vec<(u32, Option<u32>)>) -> Self {
        entries.sort_by_key(|e| e.0);
        Exposure { entries }
    }

    /// Cel `0, 1, 2...` each held for `koma` frames starting at `start`, `count` drawings.
    pub fn sequence(start: u32, count: u32, koma: u32) -> Self {
        Exposure::new((0..count).map(|i| (start + i * koma, Some(i))).collect())
    }

    pub fn at(&self, frame: u32) -> Option<u32> {
        let i = self.entries.partition_point(|e| e.0 <= frame);
        if i == 0 { None } else { self.entries[i - 1].1 }
    }
}

/// A key drawing (原画) on a timesheet.
#[derive(Debug, Clone, Default)]
pub struct Key {
    /// Frame the key lands on.
    pub frame: i32,
    /// The timing chart of the in-betweens (中割り) leading into this key from the previous one:
    /// how far along the move each drawing is, one drawing every `koma` frames before the key.
    /// `[0.5]` is a single breakdown; fractions bunched near 1 (`[0.7, 0.9]`) snap into the
    /// key, fractions near 0 (`[0.1, 0.3]`) hold back and then jump (タメ).
    pub chart: Vec<f32>,
    /// The first drawing of the key goes this fraction of the move past it, the next one
    /// settles back (a snap that hits). 0 = lands clean.
    pub overshoot: f32,
}

/// One drawing of a [`Sheet`]: `t` of the way from key `from` to key `to` (beyond 1 when it
/// overshoots), first shown at `frame`. `hold` counts the drawings of a moving hold after the
/// key (0 for in-betweens and the key itself).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Cel {
    pub frame: i32,
    pub from: usize,
    pub to: usize,
    pub t: f32,
    pub hold: u32,
}

/// A timesheet built the way a key animator times a cut: keys on their frames, in-betweens from
/// the timing charts, overshoots, and moving holds (a new, slightly different drawing every
/// `hold_koma` frames while a key holds, so the character never freezes).
#[derive(Debug, Clone, Default)]
pub struct Sheet {
    cels: Vec<Cel>,
}

impl Sheet {
    /// `keys` must be sorted by frame. In-betweens are drawn on `koma`s; when there is not
    /// enough room between two keys the earliest in-betweens are dropped.
    pub fn new(keys: &[Key], koma: u32, hold_koma: u32) -> Sheet {
        let koma = koma.max(1) as i32;
        let hold_koma = hold_koma.max(1) as i32;
        let mut cels: Vec<Cel> = Vec::new();
        for (i, key) in keys.iter().enumerate() {
            let from = i.saturating_sub(1);
            // the drawing that has to stay before the in-betweens of this key
            let floor = cels.last().map_or(i32::MIN, |c| c.frame);
            let n = key.chart.len() as i32;
            for (j, t) in key.chart.iter().enumerate() {
                let frame = key.frame - koma * (n - j as i32);
                if i > 0 && frame > floor {
                    cels.push(Cel {
                        frame,
                        from,
                        to: i,
                        t: *t,
                        hold: 0,
                    });
                }
            }
            let mut frame = key.frame;
            if key.overshoot != 0.0 && i > 0 {
                cels.push(Cel {
                    frame,
                    from,
                    to: i,
                    t: 1.0 + key.overshoot,
                    hold: 0,
                });
                frame += koma;
            }
            // the key and its moving hold, up to the first in-between of the next key
            let next = keys
                .get(i + 1)
                .map_or(i32::MAX, |k| k.frame - koma * k.chart.len() as i32);
            let mut hold = u32::from(key.overshoot != 0.0 && i > 0);
            while frame < next {
                cels.push(Cel {
                    frame,
                    from,
                    to: i,
                    t: 1.0,
                    hold,
                });
                if next == i32::MAX {
                    break;
                }
                frame += hold_koma;
                hold += 1;
            }
        }
        cels.dedup_by(|b, a| {
            a.frame == b.frame && {
                *a = *b;
                true
            }
        });
        Sheet { cels }
    }

    /// The drawing shown at `frame` (the first drawing before the first key).
    pub fn at(&self, frame: i32) -> Option<&Cel> {
        let i = self.cels.partition_point(|c| c.frame <= frame);
        self.cels.get(i.saturating_sub(1))
    }

    pub fn cels(&self) -> &[Cel] {
        &self.cels
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn holds_on_twos() {
        let fps = 24.0;
        assert_eq!(hold(0.0, fps, 2), 0.0);
        assert_eq!(hold(1.0 / 24.0, fps, 2), 0.0);
        assert!((hold(2.0 / 24.0, fps, 2) - 2.0 / 24.0).abs() < 1e-6);
        assert!((hold(5.0 / 24.0, fps, 3) - 3.0 / 24.0).abs() < 1e-6);
    }

    #[test]
    fn exposure_lookup() {
        let e = Exposure::new(vec![(0, Some(1)), (3, Some(2)), (6, None), (8, Some(1))]);
        assert_eq!(e.at(0), Some(1));
        assert_eq!(e.at(4), Some(2));
        assert_eq!(e.at(7), None);
        assert_eq!(e.at(100), Some(1));
    }

    #[test]
    fn sheet_snaps_and_holds() {
        let keys = [
            Key {
                frame: 0,
                ..Default::default()
            },
            Key {
                frame: 8,
                chart: vec![0.6],
                overshoot: 0.1,
            },
            Key {
                frame: 20,
                chart: vec![0.3, 0.8],
                overshoot: 0.0,
            },
        ];
        let sheet = Sheet::new(&keys, 2, 4);
        let frames: Vec<(i32, f32, u32)> = sheet
            .cels()
            .iter()
            .map(|c| (c.frame, c.t, c.hold))
            .collect();
        assert_eq!(
            frames,
            vec![
                (0, 1.0, 0),
                (4, 1.0, 1),
                (6, 0.6, 0),
                (8, 1.1, 0),
                (10, 1.0, 1),
                (14, 1.0, 2),
                (16, 0.3, 0),
                (18, 0.8, 0),
                (20, 1.0, 0),
            ]
        );
        assert_eq!(sheet.at(9).map(|c| c.t), Some(1.1));
        assert_eq!(sheet.at(-3).map(|c| c.frame), Some(0));
    }
}
