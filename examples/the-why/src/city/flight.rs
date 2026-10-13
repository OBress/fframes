//! The flight of the camera: the dive into the chip, the pass of the Empire State Building,
//! the run north to Times Square, the glide down Seventh Avenue between the video screens,
//! and the climb into the stars.

use std::sync::OnceLock;

use fframes_3d::math::smoothstep;
use fframes_3d::*;

use super::traffic::Path;
use super::{DIVE, DURATION, END_DIRECTION, START_HEIGHT, ease};
use crate::look;

/// The ground track of the flight, in meters. A smooth curve passes through these points:
/// from the dive over Broadway at 29th Street, west of the Empire State Building, to
/// Seventh Avenue at 40th Street, and north through the middle of Times Square.
const TRACK: [Vec2; 21] = [
    DIVE,
    Vec2::new(100.0, -210.0),
    Vec2::new(120.0, -90.0),
    Vec2::new(170.0, 30.0),
    Vec2::new(245.0, 150.0),
    Vec2::new(330.0, 280.0),
    Vec2::new(412.0, 400.0),
    Vec2::new(474.0, 505.0),
    Vec2::new(508.0, 600.0),
    Vec2::new(516.0, 680.0),
    Vec2::new(510.0, 760.0),
    Vec2::new(499.0, 830.0),
    Vec2::new(498.0, 900.0),
    Vec2::new(503.0, 970.0),
    Vec2::new(512.0, 1040.0),
    Vec2::new(523.0, 1110.0),
    Vec2::new(540.0, 1190.0),
    Vec2::new(562.0, 1270.0),
    Vec2::new(590.0, 1350.0),
    Vec2::new(622.0, 1430.0),
    Vec2::new(655.0, 1510.0),
];

/// The point in Times Square that the camera looks at when it drops into the square.
const SQUARE: Vec3 = Vec3::new(505.0, 30.0, 960.0);

/// The track as a polyline with the distance along it.
pub fn track() -> &'static Path {
    static TRACK_PATH: OnceLock<Path> = OnceLock::new();
    TRACK_PATH.get_or_init(|| {
        // Catmull-Rom through the points, 16 steps per span.
        let n = TRACK.len();
        let mut points = Vec::new();
        for i in 0..n - 1 {
            let p0 = TRACK[i.saturating_sub(1)];
            let (p1, p2) = (TRACK[i], TRACK[i + 1]);
            let p3 = TRACK[(i + 2).min(n - 1)];
            for k in 0..16 {
                let u = k as f32 / 16.0;
                let (u2, u3) = (u * u, u * u * u);
                points.push(
                    (p1 * 2.0
                        + (p2 - p0) * u
                        + (p0 * 2.0 - p1 * 5.0 + p2 * 4.0 - p3) * u2
                        + (p1 * 3.0 - p0 - p2 * 3.0 + p3) * u3)
                        * 0.5,
                );
            }
        }
        points.push(TRACK[n - 1]);
        Path::new(points)
    })
}

/// Whether a point on the ground is near enough to the flight for small details.
pub fn near_flight(p: Vec2) -> bool {
    track().project(p).0 < 420.0
}

/// The speed along the track in meters per second at `t`. The camera falls straight at
/// first, as at the end of shot 3. While it still looks down on the roofs it moves slowly,
/// so the roofs do not smear. It runs north at up to 310 m/s once it looks ahead, slows
/// down to 70 m/s when it drops into Times Square, and speeds up again for the climb.
/// From 6.1 s on, the flight is the same as before the slow start was added, so the times
/// of the screens and the lights do not move.
fn speed(t: f32) -> f32 {
    222.0 * smoothstep(0.25, 2.3, t) - 110.0 * bump(0.4, 3.2, t) + 88.0 * bump(2.6, 6.1, t)
        - 122.0 * smoothstep(5.4, 6.8, t)
        - 30.0 * smoothstep(7.0, 8.0, t)
        + 40.0 * smoothstep(9.9, 11.2, t)
}

/// A smooth bump from `a` to `b` with the height 1 in the middle: zero with zero slope at
/// both ends. Its integral is `(b - a) / 2`.
fn bump(a: f32, b: f32, t: f32) -> f32 {
    if t <= a || t >= b {
        return 0.0;
    }
    (std::f32::consts::PI * (t - a) / (b - a)).sin().powi(2)
}

/// The distance along the track at `t`: the integral of the speed.
pub fn distance(t: f32) -> f32 {
    let step = 1.0f32 / 240.0;
    let (mut along, mut s) = (0.0, 0.0);
    while s < t {
        let dt = step.min(t - s);
        along += speed(s + dt * 0.5) * dt;
        s += dt;
    }
    along
}

/// The height of the flight over the point `z` of the track (uptown), in meters: high
/// over the roofs, above Three Times Square at 42nd Street, down to 80 m in the square,
/// and up over its north end.
fn track_height(z: f32) -> f32 {
    400.0
        - 215.0 * ease(100.0, 640.0, z)
        - 3.0 * ease(640.0, 760.0, z)
        - 96.0 * ease(760.0, 900.0, z)
        - 6.0 * ease(900.0, 980.0, z)
        + 160.0 * ease(980.0, 1260.0, z)
        + 50.0 * smoothstep(1260.0, 1600.0, z)
}

/// The position of the camera at `t` seconds.
pub fn flight_position(t: f32) -> Vec3 {
    let (ground, _) = track().at(distance(t));
    // The height falls at 1.18 times itself per second at the start, as at the end of
    // shot 3, and slows down to 400 m at 2.6 s.
    let dive_end = 2.6;
    let level = 400.0;
    let u = (t / dive_end).min(1.0);
    let drop = (START_HEIGHT / level).ln();
    let k = 1.0 - (1.0 - u).powf(1.18 * dive_end / drop);
    let dive = (START_HEIGHT.ln() - drop * k).exp();
    let height = dive + (track_height(ground.y) - dive) * smoothstep(2.4, 3.4, t);
    vec3(ground.x, height, ground.y)
}

/// The yaw (around +Y, 0 = +Z) and the pitch of the direction from `from` to `to`.
fn angles(from: Vec3, to: Vec3) -> (f32, f32) {
    let d = to - from;
    (d.x.atan2(d.z), d.y.atan2(vec2(d.x, d.z).length()))
}

/// `a` turned toward `b` by `k` (`0..1`), the short way around.
fn turn(a: f32, b: f32, k: f32) -> f32 {
    let d = (b - a + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI;
    a + d * k
}

/// The yaw and the pitch of the camera at `t`, before the roll.
fn aim(t: f32) -> (f32, f32) {
    let end = END_DIRECTION.normalize();
    let (end_yaw, end_pitch) = (end.x.atan2(end.z), end.y.asin());
    let at = flight_position(t);
    // The direction of the track 80 m ahead.
    let s = distance(t);
    let (ahead, _) = track().at(s + 80.0);
    let (path_yaw, _) = angles(at, vec3(ahead.x, at.y, ahead.y));
    let (square_yaw, square_pitch) = angles(at, SQUARE);

    // Straight down at the cut. While the camera pulls out of the dive, the picture turns
    // slowly to the track. The Empire State Building passes on the right.
    let mut yaw = turn(0.0, path_yaw, ease(0.9, 4.2, t));
    yaw = turn(
        yaw,
        square_yaw,
        0.6 * ease(5.6, 6.8, t) * (1.0 - ease(8.2, 9.4, t)),
    );
    yaw = turn(yaw, end_yaw, ease(10.4, 12.5, t));

    let mut pitch = deg(-90.0) + deg(83.0) * smoothstep(0.35, 4.0, t);
    pitch += (square_pitch - pitch) * 0.9 * ease(5.8, 7.0, t);
    pitch += (deg(-6.0) - pitch) * ease(8.0, 9.4, t);
    pitch += (deg(7.0) - pitch) * ease(10.2, 12.0, t);
    // The tilt into the stars: fast, with a soft end on the direction of shot 5.
    let tilt = 1.0 - (1.0 - ease(12.1, DURATION, t)).powi(3);
    pitch += (end_pitch - pitch) * tilt;
    yaw = turn(yaw, end_yaw, tilt);
    (yaw, pitch)
}

/// The camera at `t` seconds.
pub fn camera(t: f32) -> Camera {
    let (yaw, pitch) = aim(t);
    let forward = vec3(
        yaw.sin() * pitch.cos(),
        pitch.sin(),
        yaw.cos() * pitch.cos(),
    );
    // The up direction of the image. Looking straight down, it is the heading. At the end
    // it gives the same image as +Y.
    let up = vec3(
        -yaw.sin() * pitch.sin(),
        pitch.cos(),
        -yaw.cos() * pitch.sin(),
    );
    let position = flight_position(t);
    // 35 mm at the cuts, 24 mm for the Empire State Building and in Times Square.
    let mm = 35.0 - 11.0 * ease(1.2, 2.6, t) + 2.0 * ease(3.5, 5.0, t) - 2.0 * ease(6.0, 7.5, t)
        + 11.0 * ease(11.4, 12.9, t);
    let mut camera = look::lens(mm)
        .position(position)
        .look_at(position + forward * 1000.0)
        .clip(1.0, 250_000.0);
    camera.up = up;
    // The camera banks into its turns a little, like a drone.
    let rate = turn(
        0.0,
        aim((t + 0.05).min(DURATION)).0 - aim((t - 0.05).max(0.0)).0,
        1.0,
    ) / 0.1;
    camera.roll = (-rate * 0.06).clamp(-deg(5.0), deg(5.0))
        * smoothstep(0.8, 2.0, t)
        * (1.0 - smoothstep(12.0, 12.8, t));
    // The focus: the ground on the dive, the roofs in the middle of the picture during the
    // run, Times Square, the stars at the end.
    let ground = position.y / (-pitch.sin()).max(0.2);
    let roofs = if pitch < -0.03 {
        ((position.y - 100.0) / -pitch.sin()).clamp(150.0, 1500.0)
    } else {
        1500.0
    };
    let mut focus = ground;
    focus += (roofs - focus) * smoothstep(1.0, 2.0, t);
    focus += (80.0 - focus) * smoothstep(6.4, 7.6, t);
    focus += (400.0 - focus) * smoothstep(10.5, 12.5, t);
    // Little lens blur during the run over the city, so the windows stay sharp. A bit
    // more in the canyon, where the facades are near.
    let blur = 2.0 - 1.0 * smoothstep(0.8, 1.8, t)
        + 0.6 * smoothstep(6.5, 7.5, t)
        + 1.9 * smoothstep(10.5, 11.5, t)
        - 2.5 * smoothstep(10.5, 12.5, t);
    let shake = 0.0012 * smoothstep(3.0, 4.5, t) * (1.0 - smoothstep(11.0, 12.4, t));
    camera.focus(focus, blur).shake(t, shake, 41)
}

/// The largest path of the city on the picture while the shutter of the frame at `t` is
/// open (a 180 degree shutter, [`look::SHUTTER`]), in pixels of a picture 1920 pixels wide.
/// The points are where rays through a grid over the picture meet the roofs (80 m above the
/// street), or 2.5 km away.
pub fn motion(t: f32) -> f32 {
    let (width, height) = (1920.0, 1920.0 / look::ASPECT);
    let [before, now, after] = [-0.5, 0.0, 0.5].map(|k| camera(t + k * look::SHUTTER));
    let inverse = now.view_projection(width / height).inverse();
    let mut most = 0.0f32;
    for i in 0..5 {
        for j in 0..3 {
            let ndc = vec2(-0.9 + 0.45 * i as f32, -0.8 + 0.8 * j as f32);
            let a = inverse.project_point3(ndc.extend(0.2));
            let b = inverse.project_point3(ndc.extend(0.8));
            let mut d = (b - a).normalize_or(now.forward());
            if d.dot(now.forward()) < 0.0 {
                d = -d;
            }
            let reach = if d.y < -0.01 {
                ((now.position.y - 80.0) / -d.y).clamp(30.0, 2500.0)
            } else {
                2500.0
            };
            let p = now.position + d * reach;
            if let (Some(p0), Some(p1)) = (
                before.project(p, width, height),
                after.project(p, width, height),
            ) {
                most = most.max(p0.truncate().distance(p1.truncate()));
            }
        }
    }
    most
}

/// The longest path on the picture during one exposure, in pixels of a picture 1920
/// pixels wide. Frames that move more close the shutter sooner (see [`exposure_time`]).
/// The dive at the cut keeps the full shutter of shot 3. The run over the city keeps the
/// least blur, so its windows stay sharp.
fn max_blur(t: f32) -> f32 {
    12.0 - 5.6 * smoothstep(0.3, 0.8, t)
        - 2.8 * smoothstep(0.9, 1.4, t) * (1.0 - smoothstep(6.3, 6.8, t))
}

/// The part (`0..1`) of the 180 degree shutter that is open in the frame at `t`.
pub fn exposure(t: f32) -> f32 {
    (max_blur(t) / motion(t).max(1e-3)).min(1.0)
}

/// The motion blur samples of the frame at `t`: enough that two samples are at most 1.6
/// pixels apart on a picture 1920 pixels wide, so the blur is a smear and not a row of
/// copies. From 3 to 5.
pub fn samples(t: f32) -> usize {
    let blur = motion(t) * exposure(t);
    (blur / 1.6).ceil().clamp(3.0, 5.0) as usize
}

/// The time to show for a motion blur sample at `t`. The film renders the frame at `f`
/// from samples at `f + offset * look::SHUTTER`, with the offsets in `-0.5..0.5`. This
/// moves the samples toward the frame time by its [`exposure`], so the shutter of a fast
/// frame closes sooner. Other times stay as they are.
pub fn exposure_time(t: f32) -> f32 {
    let fps = crate::FPS as f32;
    let frame = (t * fps).round() / fps;
    let offset = t - frame;
    if offset.abs() > look::SHUTTER * 0.5 + 1e-5 {
        return t;
    }
    frame + offset * exposure(frame)
}
