//! A cast member performing in a shot: motion, expression, exposure and the baked secondary
//! motion (hair, tie, scarf, robe).

use std::sync::Arc;

use fframes::Svgr;
use fframes_anime::camera::Camera;
use fframes_anime::figure::Figure;
use fframes_anime::glam::Vec3;
use fframes_anime::koma;
use fframes_anime::rig::{Bone, Posed, Skeleton};
use fframes_anime::spring::{self, Baked, Snapshot};
use fframes_anime::toon::{Look, Material, RigLayout};

use crate::cast::{Member, Who, member};
use crate::motion::MotionFn;
use crate::palette::Paint;

pub type LookFn = Arc<dyn Fn(f32) -> Look + Send + Sync>;

/// A timesheet as a function: the time of the drawing shown at `t`.
pub type SheetFn = Arc<dyn Fn(f32) -> f32 + Send + Sync>;

pub const FPS: f32 = 24.0;

pub struct Actor {
    pub member: Member,
    pub skeleton: Skeleton,
    pub motion: MotionFn,
    pub look: LookFn,
    /// Frames each drawing is held (2 = on twos).
    pub koma: u32,
    /// A timesheet that replaces the even `koma` timing (keys, in-betweens and holds of their
    /// own lengths).
    sheet: Option<SheetFn>,
    pub figure: Arc<Figure>,
    /// Line art thickness multiplier.
    pub line_weight: f32,
    baked: Baked,
    layout: RigLayout,
}

/// Everything needed to draw an actor at one moment.
pub struct ActorFrame {
    pub posed: Posed,
    pub sim: Snapshot,
    pub look: Look,
}

pub fn look(l: Look) -> LookFn {
    Arc::new(move |_| l)
}

/// An acting track (expression keys, blinks, darts, lip sync) as a look function.
pub fn acting(a: fframes_anime::acting::Acting) -> LookFn {
    Arc::new(move |t| a.at(t))
}

impl Actor {
    /// Bakes the secondary motion over `0..duration` seconds of shot time.
    pub fn new(
        who: Who,
        motion: MotionFn,
        look: LookFn,
        duration: f32,
        koma: u32,
        wind: Vec3,
    ) -> Self {
        Self::with_wind(who, motion, look, duration, koma, Arc::new(move |_| wind))
    }

    pub fn with_wind(
        who: Who,
        motion: MotionFn,
        look: LookFn,
        duration: f32,
        koma: u32,
        wind: Arc<dyn Fn(f32) -> Vec3 + Send + Sync>,
    ) -> Self {
        Self::with_air(
            who,
            motion,
            look,
            duration,
            koma,
            Arc::new(move |t| (wind(t), 0.0)),
        )
    }

    /// Wind and billow (air flowing out from the body, opening robes like wings) over time.
    pub fn with_air(
        who: Who,
        motion: MotionFn,
        look: LookFn,
        duration: f32,
        koma: u32,
        air: Arc<dyn Fn(f32) -> (Vec3, f32) + Send + Sync>,
    ) -> Self {
        Self::build(who, motion, look, duration, koma, None, air, None)
    }

    /// [`Actor::new`] with the ground at height `floor`: hair and cloth fall onto it and rest
    /// there instead of sinking through.
    pub fn on_floor(
        who: Who,
        motion: MotionFn,
        look: LookFn,
        duration: f32,
        koma: u32,
        wind: Vec3,
        floor: f32,
    ) -> Self {
        Self::build(
            who,
            motion,
            look,
            duration,
            koma,
            None,
            Arc::new(move |_| (wind, 0.0)),
            Some(floor),
        )
    }

    /// An actor timed by a timesheet (see `fframes_anime::koma::Sheet`): `motion` gives the pose
    /// of the drawing at each time `sheet` returns. `between` is the motion between the
    /// drawings (the in-betweens nobody draws): the hair and cloth follow it, so a snap from one
    /// key to the next swings them instead of kicking them.
    pub fn on_sheet(
        who: Who,
        motion: MotionFn,
        sheet: SheetFn,
        between: MotionFn,
        look: LookFn,
        duration: f32,
    ) -> Self {
        Self::build(
            who,
            motion,
            look,
            duration,
            1,
            Some((sheet, between)),
            Arc::new(|_| (Vec3::ZERO, 0.0)),
            None,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn build(
        who: Who,
        motion: MotionFn,
        look: LookFn,
        duration: f32,
        koma: u32,
        sheet: Option<(SheetFn, MotionFn)>,
        air: Arc<dyn Fn(f32) -> (Vec3, f32) + Send + Sync>,
        floor: Option<f32>,
    ) -> Self {
        let member = member(who);
        let skeleton = member.skeleton();
        let (rig, layout) = member.design.physics(&skeleton.proportions);
        let design = &member.design;
        let baked = spring::bake(&rig, 0.0, duration.max(1.0 / FPS), FPS, 10, 0.8, |t| {
            // Physics sees the same held drawings the audience sees, or the motion between
            // the drawings of a timesheet.
            let posed = match &sheet {
                Some((_, between)) => skeleton.solve(&between(t)),
                None => skeleton.solve(&motion(koma::hold(t, FPS, koma))),
            };
            let (wind, billow) = air(t);
            let mut state = design.body_state(&posed, wind);
            state.billow = billow;
            state.floor = floor;
            state
        });
        if std::env::var("ANIME_OP_ROBE_DEBUG").is_ok() {
            let snap = baked.at(0.5);
            let posed = skeleton.solve(&motion(0.5));
            for (i, c) in snap.cloths.iter().enumerate() {
                let lo = c.iter().map(|p| p.y).fold(f32::INFINITY, f32::min);
                let hi = c.iter().map(|p| p.y).fold(f32::NEG_INFINITY, f32::max);
                eprintln!(
                    "{who:?} cloth {i}: y {lo:.3}..{hi:.3}, chest {:.3}, rest rows {}",
                    posed.pos(Bone::Chest).y,
                    layout.robe_rows
                );
            }
            if let Some(tie) = layout.tie.and_then(|i| snap.chains.get(i)) {
                let chest = posed.pos(Bone::Chest);
                let local: Vec<String> = tie
                    .iter()
                    .map(|p| {
                        format!(
                            "({:.3},{:.3},{:.3})",
                            p.x - chest.x,
                            p.y - chest.y,
                            p.z - chest.z
                        )
                    })
                    .collect();
                eprintln!("tie {}", local.join(" "));
            }
        }
        Actor {
            figure: crate::render::figure(who),
            member,
            skeleton,
            motion,
            look,
            koma,
            sheet: sheet.map(|(s, _)| s),
            line_weight: 1.0,
            baked,
            layout,
        }
    }

    pub fn with_line_weight(mut self, w: f32) -> Self {
        self.line_weight = w;
        self
    }

    pub fn layout(&self) -> &RigLayout {
        &self.layout
    }

    /// The held drawing at `t`: pose, simulation and expression.
    pub fn frame(&self, t: f32) -> ActorFrame {
        let th = self.held(t);
        let slide = self.slide(t, th);
        let mut sim = self.baked.at(th);
        if slide != Vec3::ZERO {
            for p in sim.chains.iter_mut().chain(sim.cloths.iter_mut()).flatten() {
                *p += slide;
            }
        }
        ActorFrame {
            posed: self.posed_at(th, slide),
            sim,
            look: (self.look)(th),
        }
    }

    pub fn held(&self, t: f32) -> f32 {
        match &self.sheet {
            Some(sheet) => sheet(t),
            None => koma::hold(t, FPS, self.koma),
        }
    }

    /// The time of the drawing before the one shown at `th` (a drawing time).
    fn previous(&self, th: f32) -> f32 {
        match &self.sheet {
            Some(sheet) => sheet(th - 0.5 / FPS),
            None => th - self.koma.max(1) as f32 / FPS,
        }
    }

    /// 引きセル (a pulled cel): the drawing is held on twos, but the character travels on ones,
    /// the way a held cel slides over the background. Without it a character held on twos
    /// steps back and forth against a camera that follows it on ones.
    fn slide(&self, t: f32, th: f32) -> Vec3 {
        if (t - th).abs() < 1e-5 {
            return Vec3::ZERO;
        }
        (self.motion)(t).root - (self.motion)(th).root
    }

    fn posed_at(&self, th: f32, slide: Vec3) -> Posed {
        let mut pose = (self.motion)(th);
        pose.root += slide;
        self.skeleton.solve(&pose)
    }

    pub fn posed(&self, t: f32) -> Posed {
        let th = self.held(t);
        self.posed_at(th, self.slide(t, th))
    }

    pub fn draw<'a>(&self, t: f32, cam: &Camera, paint: &Paint) -> Svgr<'a> {
        crate::render::draw(&[(self, *paint)], t, cam)
    }

    /// Screen velocity of the hips at `t`, pixels per frame, seen by `cam` now and by `before`
    /// two frames ago (averaged over two frames so a drawing held on twos does not read as
    /// standing still).
    pub fn screen_velocity(
        &self,
        t: f32,
        cam: &Camera,
        before: &Camera,
    ) -> fframes_anime::glam::Vec2 {
        let now = cam.project(self.posed(t).pos(Bone::Hips)).p;
        let then = before.project(self.posed(t - 2.0 / FPS).pos(Bone::Hips)).p;
        (now - then) * 0.5
    }

    /// Drawn with a motion blur trail when it crosses the screen fast; `before` is the camera
    /// two frames ago (the same camera when it does not move).
    pub fn draw_moving<'a>(
        &self,
        t: f32,
        cam: &Camera,
        before: &Camera,
        paint: &Paint,
    ) -> Svgr<'a> {
        let v = self.screen_velocity(t, cam, before);
        let pivot = cam.project(self.posed(t).pos(Bone::Hips)).p;
        // nothing in the frame: nothing to blur (a filter on an empty group is an error)
        match crate::render::try_draw(&[(self, *paint)], t, cam) {
            Some(drawn) => crate::fx::motion_blur(drawn, v, pivot),
            None => Svgr::empty(),
        }
    }
}

pub fn color_of(m: Material, member: &Member, paint: &Paint) -> &'static str {
    match m {
        Material::Suit | Material::Tie => paint.suit,
        Material::Robe => paint.robe.unwrap_or(paint.suit),
        Material::RobeLining => paint.lining.unwrap_or(paint.suit),
        Material::Shirt | Material::Skin | Material::Paper => paint.paper,
        Material::Hair => paint.hair.or(member.hair).unwrap_or(paint.ink),
        Material::Beard => paint.beard.or(member.beard).unwrap_or(paint.ink),
        Material::Scarf => paint.scarf.unwrap_or(paint.ink),
        Material::Ink => paint.line,
        Material::Accent => paint.accent.unwrap_or(member.accent),
    }
}

impl Actor {
    /// Anime smears: when a hand or foot moved far since the last drawing, a tapered streak
    /// from where it was to where it is (drawn under the character).
    pub fn smears<'a>(&self, t: f32, cam: &Camera, color: &'static str) -> Svgr<'a> {
        let th = self.held(t);
        let slide = self.slide(t, th);
        let now = self.posed_at(th, slide);
        let t1 = self.previous(th);
        let before = self.posed_at(t1, slide);
        let earlier = self.posed_at(self.previous(t1), slide);
        let mut path = fframes_anime::shape::Path::new();
        for bone in [Bone::HandL, Bone::HandR, Bone::FootL, Bone::FootR] {
            let p0 = cam.project(earlier.pos(bone));
            let p1 = cam.project(before.pos(bone));
            let p2 = cam.project(now.pos(bone));
            let moved = (p2.p - p1.p).length();
            let w = 0.09 * p2.scale;
            if moved < w * 0.9 {
                continue;
            }
            let pts = [p0.p, p1.p, p2.p];
            path.smooth_closed(
                &fframes_anime::shape::ribbon(&pts, &[0.0, w * 0.55, w * 1.1]),
                0.7,
            );
        }
        if path.is_empty() {
            return Svgr::empty();
        }
        fframes::svgr!(<path d={path.build()} fill={color} opacity="0.85" />)
    }
}
