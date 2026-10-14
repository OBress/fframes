//! Turntable of the cast, for designing the characters.

use fframes::{FFramesContext, Frame, Svgr};
use fframes_anime::camera::Camera;
use fframes_anime::glam::Vec3;
use fframes_anime::toon::{HandShape, Look, Mouth};

use crate::actor::{Actor, look};
use crate::cast::ALL;
use crate::motion::{Library, Play, in_place};
use crate::palette::{self, Paint};
use crate::{H, Shot, W};

pub struct Lab {
    actors: Vec<Actor>,
}

impl Lab {
    pub fn new(lib: &Library) -> Self {
        let styles = [
            "walk_proud",
            "walk_normal",
            "walk_happy",
            "walk_chimpira",
            "walk_musical",
            "walk_active",
        ];
        let actors = ALL
            .iter()
            .enumerate()
            .map(|(i, who)| {
                let sk = crate::cast::member(*who).skeleton();
                let x = (i as f32 - 2.5) * 0.75;
                let motion = in_place(
                    lib,
                    styles[i],
                    &sk,
                    Play {
                        at: Vec3::new(x, 0.0, 0.0),
                        ..Default::default()
                    },
                );
                Actor::new(
                    *who,
                    motion,
                    look(Look {
                        mouth: Mouth::Smile,
                        hand_l: HandShape::Relaxed,
                        hand_r: HandShape::Fist,
                        ..Default::default()
                    }),
                    8.0,
                    2,
                    Vec3::new(0.0, 0.0, 0.0),
                )
            })
            .collect();
        Lab { actors }
    }
}

impl Shot for Lab {
    fn render<'a>(
        &'a self,
        t: f32,
        _len: f32,
        _frame: &Frame,
        _ctx: &FFramesContext<'a, '_>,
    ) -> Svgr<'a> {
        let cam = if t < 4.0 {
            Camera::orbit(
                Vec3::new(0.0, 1.0, 0.0),
                6.5,
                -40.0 + t * 12.0,
                4.0,
                30.0,
                W,
                H,
            )
        } else {
            // Close-ups: one actor per 2/3 s, three angles each.
            let k = ((t - 4.0) / (4.0 / 6.0)).floor().min(5.0) as usize;
            let local = (t - 4.0) - k as f32 * (4.0 / 6.0);
            let head = self.actors[k].posed(t).pos(fframes_anime::rig::Bone::Head)
                + Vec3::new(0.0, 0.08, 0.0);
            let yaw = [-35.0, 0.0, 35.0][((local / (4.0 / 18.0)) as usize).min(2)];
            Camera::orbit(head, 1.1, yaw, 3.0, 30.0, W, H)
        };
        let paint = Paint::two_tone(palette::YELLOW);
        let mut layers: Vec<(f32, Svgr<'a>)> = self
            .actors
            .iter()
            .map(|a| {
                (
                    cam.project(a.posed(t).pos(fframes_anime::rig::Bone::Chest))
                        .depth,
                    a.draw(t, &cam, &paint),
                )
            })
            .collect();
        layers.sort_by(|a, b| b.0.total_cmp(&a.0));
        let people: Vec<Svgr<'a>> = layers.into_iter().map(|l| l.1).collect();
        fframes::svgr!(
            <g>
                <rect x="0" y="0" width="1920" height="1080" fill={palette::YELLOW} />
                {people}
            </g>
        )
    }
}
