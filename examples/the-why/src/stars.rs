//! The night sky: stars on a sphere around the camera. Shots 4 and 5 show the same stars, so
//! the cut between them matches.

use fframes_3d::math::{hash, random_direction};
use fframes_3d::*;

/// The distance of the stars from the camera in meters. It is less than the far plane of the
/// default camera.
const RADIUS: f32 = 400.0;

/// The stars. Set the position of the node to the camera position in each frame, so the stars
/// stay infinitely far away.
pub fn stars() -> Node {
    // the Milky Way is a band around this axis
    let band = vec3(0.35, 0.25, 1.0).normalize();
    let items = (0..12000u32)
        .filter_map(|i| {
            let dir = random_direction(i * 2 + 1);
            let in_band = 1.0 - dir.dot(band).abs();
            // most of the faint stars are in the band
            if dir.y < -0.2 || hash(i * 5 + 3) > 0.2 + 0.8 * in_band.powi(10) {
                return None;
            }
            // few bright stars, many faint ones
            let magnitude = hash(i * 7 + 5).powf(14.0);
            let warm = hash(i * 11 + 7);
            let color =
                Rgb::linear(0.75 + 0.25 * warm, 0.85, 1.0 - 0.3 * warm) * (0.5 + 9.0 * magnitude);
            Some(Particle::new(dir * RADIUS, 0.04 + 0.1 * magnitude, color))
        })
        .collect();
    Node::particles(items, Blending::Additive).named("stars")
}
