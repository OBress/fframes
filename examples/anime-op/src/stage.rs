//! Drawing several actors in one shot: they share one depth buffer, so they overlap correctly.

use fframes::Svgr;
use fframes_anime::camera::Camera;

use crate::actor::Actor;
use crate::palette::Paint;

pub fn actors<'a>(list: &[&Actor], t: f32, cam: &Camera, paint: &Paint) -> Svgr<'a> {
    let painted: Vec<(&Actor, Paint)> = list.iter().map(|a| (*a, *paint)).collect();
    crate::render::draw(&painted, t, cam)
}

/// Actors with individual paints (accent colours in group shots).
pub fn actors_painted<'a>(list: &[(&Actor, Paint)], t: f32, cam: &Camera) -> Svgr<'a> {
    crate::render::draw(list, t, cam)
}
