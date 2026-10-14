//! The shot list, cut on the reference opening's cuts (times are song seconds).

use fframes::{FFramesContext, Frame, Svgr};

use crate::motion::Library;
use crate::palette;
use crate::{Cut, Shot};

mod banner;
mod dance;
mod finale;
mod friends;
mod heads;
pub mod intro;
mod lab;
mod middle;
mod middle2;
pub mod title;
mod villains;

/// Shot names and where they start; each ends where the next one starts.
pub const CUTS: &[(&str, f32)] = &[
    ("Open", 0.0),
    ("TitleNe", 2.002),
    ("BannerFff", 3.462),
    ("TitleOg", 4.838),
    ("BannerFframes", 6.423),
    ("TitleOo", 8.091),
    ("BannerOdiff", 8.842),
    ("TitleSe", 9.509),
    ("BannerZlob", 10.260),
    ("Catch", 11.011),
    ("Glitch", 12.804),
    ("Eyes", 14.056),
    ("Pattern", 15.015),
    ("Split", 16.725),
    ("Wind", 17.309),
    ("Barbell", 18.268),
    ("Pushups", 20.270),
    ("Pile", 21.646),
    ("Black", 22.397),
    ("Plate", 22.647),
    ("Dive", 23.315),
    ("Chomp", 24.065),
    ("Door", 24.608),
    ("EyeClose", 25.943),
    ("WalkTwo", 27.445),
    ("Laugh", 28.946),
    ("WalkGroup", 30.489),
    ("Collage", 32.032),
    ("Eat", 36.787),
    ("Lineup", 37.496),
    ("Rivals", 38.580),
    ("Glint", 40.040),
    ("Panic", 40.958),
    ("RedEye", 41.667),
    ("Bars", 42.793),
    ("Jump", 44.169),
    ("Keys", 44.711),
    ("Teal", 47.047),
    ("Strobe", 47.798),
    ("Flood", 49.049),
    ("Listen", 50.467),
    ("Villains", 50.926),
    ("Turn", 56.932),
    ("Grins", 60.435),
    ("Compilin", 62.354),
    ("Solo", 63.063),
    ("Lyric", 65.440),
    ("Chorus", 65.857),
    ("LyricRed", 68.235),
    ("Rows", 68.735),
    ("Cape", 71.488),
    ("Point", 72.447),
    ("Fight", 73.70),
    ("Smoke", 77.786),
    ("Kick", 78.453),
    ("Swirl", 79.204),
    ("Watch", 80.413),
    ("Charge", 81.414),
    ("Caught", 84.627),
    ("Bite", 85.544),
    ("Logo", 86.712),
    ("End", 89.173),
];

pub const SONG_END: f32 = 90.125;

/// A shot that is not drawn yet: its name on yellow.
struct Todo(&'static str);

impl Shot for Todo {
    fn render<'a>(
        &'a self,
        t: f32,
        len: f32,
        _frame: &Frame,
        _ctx: &FFramesContext<'a, '_>,
    ) -> Svgr<'a> {
        let label = format!("{} {:.2}/{:.2}", self.0, t, len);
        fframes::svgr!(
            <g>
                <rect x="0" y="0" width="1920" height="1080" fill={palette::YELLOW_DEEP} />
                <text x="960" y="560" font-family="Zen Kaku Gothic New" font-weight="900" font-size="90" text-anchor="middle" fill={palette::INK}>{label}</text>
            </g>
        )
    }
}

pub fn cuts(lib: &Library) -> Vec<Cut> {
    match std::env::var("ANIME_OP_LAB").as_deref() {
        Ok("dive0") => {
            return vec![Cut {
                name: "DiveLab",
                start: 0.0,
                end: 0.75,
                shot: Box::new(intro::DiveLab(intro::Dive::with_speed(0.75, 0.0, true))),
            }];
        }
        Ok("dive1") => {
            return vec![Cut {
                name: "DiveLab",
                start: 0.0,
                end: 0.75,
                shot: Box::new(intro::DiveLab(intro::Dive::with_speed(0.75, 0.0, false))),
            }];
        }
        Ok("dive") => {
            return vec![Cut {
                name: "DiveLab",
                start: 0.0,
                end: 0.75,
                shot: Box::new(intro::DiveLab(intro::Dive::new(0.75))),
            }];
        }
        Ok("hero") => {
            return vec![Cut {
                name: "HeroHead",
                start: 0.0,
                end: 4.0,
                shot: Box::new(heads::HeroHead::new(crate::cast::Who::Neogoose)),
            }];
        }
        Ok("theo") => {
            return vec![Cut {
                name: "HeroHead",
                start: 0.0,
                end: 4.0,
                shot: Box::new(heads::HeroHead::new(crate::cast::Who::Theo)),
            }];
        }
        Ok(name) if name.starts_with("faces-") => {
            return vec![Cut {
                name: "Expressions",
                start: 0.0,
                end: 4.0,
                shot: Box::new(heads::Expressions::new(lab_who(&name[6..]))),
            }];
        }
        Ok(name) if name.starts_with("sheet-") => {
            return vec![Cut {
                name: "Sheet",
                start: 0.0,
                end: 1.0,
                shot: Box::new(heads::Sheet::new(lab_who(&name[6..]))),
            }];
        }
        Ok("props") => {
            return vec![Cut {
                name: "Props",
                start: 0.0,
                end: 4.0,
                shot: Box::new(heads::Props),
            }];
        }
        Ok(name) if name.starts_with("jojo-") => {
            return vec![Cut {
                name: "Poses",
                start: 0.0,
                end: 1.0,
                shot: Box::new(heads::Poses::new(lab_who(&name[5..]))),
            }];
        }
        Ok(name) if name.starts_with("turn-") => {
            return vec![Cut {
                name: "HeroHead",
                start: 0.0,
                end: 4.0,
                shot: Box::new(heads::HeroHead::turning(lab_who(&name[5..]))),
            }];
        }
        Ok(name) if name.starts_with("head-") => {
            let who = lab_who(&name[5..]);
            return vec![Cut {
                name: "HeroHead",
                start: 0.0,
                end: 4.0,
                shot: Box::new(heads::HeroHead::new(who)),
            }];
        }
        Ok("hands") => {
            return vec![Cut {
                name: "Hands",
                start: 0.0,
                end: 1.0,
                shot: Box::new(heads::Hands::new()),
            }];
        }
        Ok("heads") => {
            return vec![Cut {
                name: "Heads",
                start: 0.0,
                end: 4.0,
                shot: Box::new(heads::Heads::new()),
            }];
        }
        Ok(_) => {
            return vec![Cut {
                name: "Lab",
                start: 0.0,
                end: 8.0,
                shot: Box::new(lab::Lab::new(lib)),
            }];
        }
        Err(_) => {}
    }
    CUTS.iter()
        .enumerate()
        .map(|(i, (name, start))| {
            let end = CUTS.get(i + 1).map_or(SONG_END, |c| c.1);
            let len = end - start;
            let shot: Box<dyn Shot> = match *name {
                "Open" => Box::new(title::Open),
                "TitleNe" => Box::new(title::Letters::new("NE", len, 0)),
                "TitleOg" => Box::new(title::Letters::new("OG", len, 1)),
                "TitleOo" => Box::new(title::Letters::new("OO", len, 2)),
                "TitleSe" => Box::new(title::Letters::new("SE", len, 3)),
                "BannerFff" => Box::new(banner::Banner {
                    house: banner::House::Fff,
                }),
                "BannerFframes" => Box::new(banner::Banner {
                    house: banner::House::Fframes,
                }),
                "BannerOdiff" => Box::new(banner::Banner {
                    house: banner::House::Odiff,
                }),
                "BannerZlob" => Box::new(banner::Banner {
                    house: banner::House::Zlob,
                }),
                "Black" | "End" => Box::new(title::Solid(palette::INK)),
                "Lyric" => Box::new(title::LyricCard {
                    color: palette::YELLOW,
                }),
                "LyricRed" => Box::new(title::LyricCard {
                    color: palette::TITLE_RED,
                }),
                "Compilin" => Box::new(title::Compilin::new(len)),
                "Logo" => Box::new(title::Logo),
                "Catch" => Box::new(intro::Catch::new(len)),
                "Glitch" => Box::new(intro::Glitch),
                "Eyes" => Box::new(intro::Eyes::new(len)),
                "Pattern" => Box::new(intro::Pattern::new(len)),
                "Split" => Box::new(intro::Split::new(len)),
                "Wind" => Box::new(intro::Wind::new(len)),
                "Barbell" => Box::new(intro::Barbell::new(*start, len)),
                "Pushups" => Box::new(intro::Pushups::new(*start, len)),
                "Pile" => Box::new(intro::Pile::new(len)),
                "Plate" => Box::new(intro::Plate::new(len)),
                "Dive" => Box::new(intro::Dive::new(len)),
                "Chomp" => Box::new(intro::Chomp::new(len)),
                "Door" => Box::new(friends::Door::new(len)),
                "EyeClose" => Box::new(friends::EyeClose::new(len)),
                "WalkTwo" => Box::new(friends::WalkTwo::new(lib, len)),
                "Laugh" => Box::new(friends::Laugh::new(len)),
                "WalkGroup" => Box::new(friends::WalkGroup::new(lib, len)),
                "Collage" => Box::new(middle::Collage::new(lib, len)),
                "Eat" => Box::new(middle::Eat::new(len)),
                "Lineup" => Box::new(middle::Lineup::new(lib, len)),
                "Rivals" => Box::new(middle::Rivals::new(lib, len)),
                "Glint" => Box::new(middle::Glint::new(len)),
                "Panic" => Box::new(middle::Panic::new(len)),
                "RedEye" => Box::new(middle::RedEye::new(len)),
                "Bars" => Box::new(middle::Bars::new(len)),
                "Jump" => Box::new(middle2::Jump::new(len)),
                "Keys" => Box::new(middle2::Keys::new(len)),
                "Teal" => Box::new(middle2::Teal::new(len)),
                "Strobe" => Box::new(middle2::Strobe::new(lib, len)),
                "Flood" => Box::new(middle2::Flood::new(lib, len)),
                "Listen" => Box::new(villains::Listen::new(len)),
                "Villains" => Box::new(villains::Villains::new(*start, len)),
                "Turn" => Box::new(villains::Turn::new(len)),
                "Grins" => Box::new(villains::Grins::new(len)),
                "Chorus" => Box::new(dance::Chorus::new(*start, len)),
                "Rows" => Box::new(dance::Rows::new(*start, len)),
                "Cape" => Box::new(finale::Cape::new(len)),
                "Point" => Box::new(finale::Point::new(len)),
                "Fight" => Box::new(finale::Fight::new(lib, len)),
                "Smoke" => Box::new(finale::Smoke::new(len)),
                "Kick" => Box::new(finale::Kick::new(lib, len)),
                "Swirl" => Box::new(finale::Swirl::new(len)),
                "Watch" => Box::new(finale::Watch::new(len)),
                "Charge" => Box::new(finale::Charge::new(len)),
                "Caught" => Box::new(finale::Caught::new(len)),
                "Bite" => Box::new(finale::Bite::new(len)),
                "Solo" => Box::new(dance::Solo::new(*start, len)),
                _ => Box::new(Todo(name)),
            };
            Cut {
                name,
                start: *start,
                end,
                shot,
            }
        })
        .collect()
}

fn lab_who(name: &str) -> crate::cast::Who {
    match name {
        "chshersh" => crate::cast::Who::Chshersh,
        "shreyas" => crate::cast::Who::Shreyas,
        "dillon" => crate::cast::Who::Dillon,
        "theo" => crate::cast::Who::Theo,
        "teej" => crate::cast::Who::Teej,
        "demon" => crate::cast::Who::Demon,
        "bloat" => crate::cast::Who::Bloat,
        "latency" => crate::cast::Who::Latency,
        "segfault" => crate::cast::Who::Segfault,
        _ => crate::cast::Who::Neogoose,
    }
}
