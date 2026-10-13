//! Shot 3: the engineer, seen by the watcher. The camera is its eye. It locks on to the
//! engineer on the catwalk and finds no role for him. Then it goes back to work: in a macro
//! view it lowers the chip U17 into its footprint on the board. The camera leaves the eye
//! and dives into the chip, where shot 4 goes on.

use fframes::{Svgr, svgr};

use super::*;
use crate::WIDTH;
use crate::look::{BAR, HUD, PICTURE_HEIGHT};

/// Shot 4 starts this high above the top of the chip and falls 1.18 times its height per
/// second. The dive of shot 3 ends the same way.
const DIVE_HEIGHT: f32 = 0.0418;
const DIVE_RATE: f32 = 1.18;
/// The length of shot 3 in seconds.
const SECONDS: f32 = 10.0;
/// The eye leaves the engineer, holds the chip over the board, the chip touches the board,
/// the fingers open, and the camera leaves the eye. Seconds into shot 3.
const WHIP: f32 = 4.4;
const ARRIVE: f32 = 5.6;
const TOUCH: f32 = 7.3;
const RELEASE: f32 = 7.75;
const LEAVE: f32 = 8.0;
/// The camera of the macro view in the J6 frame: 15 cm from the tool axis on the side of the
/// eye.
const MACRO: Vec3 = Vec3::new(0.0, 0.16, 0.11);
/// The middle of the chip in the J6 frame.
const HELD: Vec3 = Vec3::new(0.0, 0.0, HOLD);
/// Half the width of the chip over its pins, in meters.
const CHIP_HALF: f32 = 0.0202;

/// The height of the dive camera above the top of the chip at `t`.
fn dive_height(t: f32) -> f32 {
    DIVE_HEIGHT * (DIVE_RATE * (SECONDS - t)).exp()
}

/// The chip over its place in shot 3: its height above the seat in meters and its turn in
/// radians. It comes down slowly, the turn is corrected in two steps, and at the touch it
/// makes a small hop of 0.4 mm as the pins find the pads.
fn placing(t: f32) -> (f32, f32) {
    let down = smoothstep(ARRIVE + 0.15, TOUCH, t);
    let after = (t - TOUCH).max(0.0);
    let hop = 0.0004 * (after * 22.0).sin().abs() * (-after / 0.09).exp();
    let height = 0.06 * (1.0 - down) + if t > TOUCH { hop } else { 0.0 };
    let turn = MISALIGNED
        * (1.0
            - 0.85 * smoothstep(ARRIVE + 0.3, ARRIVE + 0.75, t)
            - 0.15 * smoothstep(6.55, 6.85, t));
    (height, turn)
}

impl FactorySet {
    /// Shot 3: the engineer, seen by the watcher.
    pub fn moment_engineer(&self, t: f32) -> (Scene, Camera) {
        let (q, grip) = self.pov_joints(t);
        let camera = self.pov_camera(t, &q);
        let jig = self.watcher.base.transform_point3(JIG);
        // the shadows: of the hall for the light shafts behind the engineer, then of the
        // chip and the fingers on the board
        let bounds = if t < WHIP + 0.6 {
            Aabb {
                min: vec3(-30.0, 0.0, hall::CATWALK_Z - 100.0),
                max: vec3(30.0, hall::WALL + 4.0, WATCHER_Z + 10.0),
            }
        } else {
            Aabb {
                min: jig - vec3(0.35, 0.1, 0.35),
                max: jig + vec3(0.35, 0.6, 0.35),
            }
        };
        let time = t + 12.0;
        // In the dive the light of the hall goes away and the light of shot 4 comes: a dark
        // studio with one large soft light straight above.
        let daylight = 1.0 - smoothstep(8.4, 9.6, t);
        let mut scene = self.day(time, bounds, daylight);
        let view = View::new(&camera);
        self.work(&mut scene, time, &view);
        self.pose_hero(
            &mut scene,
            &self.welder,
            self.welder_joints(time),
            time,
            0.0,
        );
        let awake = 1.0 - smoothstep(WHIP, WHIP + 0.6, t);
        self.pose_watcher(&mut scene, &q, &grip, awake);
        self.pose_engineer(&mut scene, 24.0 + t, smoothstep(1.4, 3.1, t));
        self.show(&mut scene, &self.welder, &view);
        // The inspection lamp lights the board from the far side, so the pads and the
        // traces glint toward the eye. It goes off in the dive.
        let on = smoothstep(WHIP, ARRIVE, t) * (1.0 - smoothstep(8.2, 9.0, t));
        let lamp = jig + vec3(0.16, 0.4, 0.42);
        // The sky of the hall fades first, then the studio of shot 4 comes in its place.
        if let Environment::Map { intensity, .. } = &mut scene.environment {
            *intensity = SKY * (1.0 - smoothstep(8.6, 8.95, t));
        }
        let city = smoothstep(8.95, 9.3, t);
        if t > 8.95 {
            scene.environment = chip_light(city);
            scene.add(Node::light(Light::hemisphere(
                0x0a1424,
                0x2a4470,
                0.35 * city,
            )));
        }
        // The board falls into black around the chip, as the ground around the chip of
        // shot 4. Flat pins as in shot 4 take the place of the gull wing pins.
        let veil = smoothstep(8.5, 9.4, t);
        let leads = smoothstep(8.9, 9.6, t);
        // As the flat pins come, the sheet rises over the gull wing pins to 0.3 mm under the
        // top of the chip.
        scene[self.veil].position.y = JIG.y + 0.0001 + (0.0021 + CHIP_TOP - 0.0004) * leads;
        for (node, opacity) in [(self.veil, veil), (self.leads, leads)] {
            scene[node].visible = opacity > 0.0;
            if let Some(m) = scene[node].material_mut() {
                m.opacity = opacity;
            }
        }
        let lamp_node = &mut scene[self.lamp];
        lamp_node.position = lamp;
        lamp_node.object =
            Object::Light(Light::spot(0xe8f2ff, 1.0 * on, 0.5, 0.7).pointing(jig - lamp));
        (scene, camera)
    }

    pub fn shutter_engineer(&self, t: f32) -> usize {
        if (WHIP..ARRIVE).contains(&t) || t > LEAVE + 0.4 {
            3
        } else {
            2
        }
    }

    /// The joints of the watcher in shot 3 and what its gripper does. It looks at the
    /// engineer with small saccades, swings back to the board, places the chip, lets go and
    /// lifts the gripper away.
    fn pov_joints(&self, t: f32) -> (Joints, Grip) {
        // saccades: the gaze jumps to a new point every 0.7 s and settles fast
        let step = (t / 0.7).floor();
        let k = smoothstep(0.0, 0.18, t - step * 0.7);
        let jitter = |n: f32| vec2(noise1(11, n), noise1(12, n)) * 0.01;
        let offset = jitter(step - 1.0).lerp(jitter(step), k);
        let mut gaze = self.watcher_gaze();
        gaze.q[0] += offset.x;
        gaze.q[4] += offset.y;

        let (height, turn) = placing(t);
        let hover = holding(seated() + vec3(0.0, 0.06, 0.0), MISALIGNED, &gaze);
        let held = t < RELEASE;
        let grip = Grip {
            chip: seated() + vec3(0.0, height, 0.0),
            turn,
            gap: CLOSED + 0.022 * smoothstep(RELEASE, RELEASE + 0.25, t),
            held,
        };
        if t < ARRIVE {
            let swing = smoothstep(WHIP, ARRIVE, t);
            let grip = Grip {
                chip: seated() + vec3(0.0, 0.06, 0.0),
                turn: MISALIGNED,
                ..grip
            };
            return (blend(&gaze, &hover, swing), grip);
        }
        // After the release the gripper rises and moves back toward the base.
        let away = smoothstep(LEAVE - 0.1, LEAVE + 0.8, t);
        let target = grip.chip + vec3(0.0, 0.3, -0.35) * away;
        (holding(target, turn, &hover), grip)
    }

    /// The camera of shot 3: the eye of the watcher, then the macro view of the chip from the
    /// wrist, then the dive into the chip.
    fn pov_camera(&self, t: f32, q: &Joints) -> Camera {
        let flange = self.watcher.base * arm::flange(q);
        // The eye camera blends into the macro camera during the swing. Both move with J6.
        let k = smoothstep(WHIP + 0.5, ARRIVE, t);
        let eye_local = arm::EYE + Vec3::Z * 0.06;
        let local = eye_local.lerp(MACRO, k);
        let look_local = Vec3::Z.lerp((HELD - MACRO).normalize(), k).normalize();
        let (eye, forward) = if t < LEAVE {
            (
                flange.transform_point3(local),
                flange.transform_vector3(look_local).normalize(),
            )
        } else {
            // the camera stays where the eye let it go
            let (q, _) = self.pov_joints(LEAVE);
            let flange = self.watcher.base * arm::flange(&q);
            (
                flange.transform_point3(MACRO),
                flange.transform_vector3(look_local).normalize(),
            )
        };
        let up = Quat::from_axis_angle(forward, 0.05 * (1.0 - k)) * Vec3::Y;

        // The eye zooms in steps, like a machine, out for the swing and in for the macro.
        let stage = smoothstep(0.6, 0.85, t) + smoothstep(1.6, 1.85, t) + smoothstep(2.6, 2.85, t)
            - 3.0 * smoothstep(WHIP, WHIP + 0.4, t);
        let zoom = [35.0f32, 50.0, 70.0, 85.0];
        let i = (stage.floor() as usize).min(2);
        let gaze_mm = zoom[i] + (zoom[i + 1] - zoom[i]) * (stage - i as f32);
        let macro_mm = 66.0 + 14.0 * smoothstep(ARRIVE + 0.1, ARRIVE + 0.35, t);
        let mm = gaze_mm + (macro_mm - gaze_mm) * k;

        // The dive: the camera rises from the eye over the chip and falls straight down at
        // the speed with which shot 4 goes on. The top of the image is +Z, as in shot 4.
        let top = self.watcher.base.transform_point3(seated()) + vec3(0.0, CHIP_TOP, 0.0);
        let dive = smoothstep(LEAVE, 9.0, t);
        let position = eye.lerp(top + Vec3::Y * dive_height(t), dive);
        let look = if dive > 0.0 {
            (top - position).normalize()
        } else {
            forward
        };
        let mut camera = lens(mm + (35.0 - mm) * dive)
            .position(position)
            .look_at(position + look);
        camera.up = up.lerp(Vec3::Z, dive).normalize();
        camera.near = 0.005;
        camera.far = 600.0;

        // The eye hunts for focus on the engineer, locks, and racks again when he moves. In
        // the macro view it holds the chip in focus.
        let head = ENGINEER + vec3(0.0, 1.62, 0.0);
        let hunt = 0.35 * (1.0 - smoothstep(0.0, 0.9, t)) * (t * 30.0).sin()
            + 0.06 * (smoothstep(2.2, 2.35, t) - smoothstep(2.35, 2.6, t));
        let focus = if t < WHIP + 0.5 {
            eye + (head - eye) * (1.0 + hunt)
        } else {
            top
        };
        // the blur ends at the value of shot 4
        let blur = 4.0 + 6.0 * smoothstep(WHIP, ARRIVE, t) - 8.0 * smoothstep(8.6, 9.8, t);
        camera.focus_on(focus, blur)
    }

    /// The HUD of the eye in shot 3: brackets on the engineer, then on the footprint of the
    /// chip, labels and telemetry.
    pub fn overlay_engineer(&self, t: f32) -> Svgr<'static> {
        let fade = (1.0 - smoothstep(LEAVE - 0.1, LEAVE + 0.4, t)) * smoothstep(0.0, 0.25, t);
        if fade <= 0.0 {
            return crate::look::iris(t, 0.4);
        }
        let (q, _) = self.pov_joints(t);
        let camera = self.pov_camera(t, &q);
        let (w, h) = (WIDTH as f32, PICTURE_HEIGHT as f32);
        let top = BAR as f32;
        let color = "#9fd4ff";
        let project = |points: &[Vec3]| -> Option<(Vec2, Vec2)> {
            let points: Vec<Vec2> = points
                .iter()
                .filter_map(|p| camera.project(*p, w, h))
                .map(Vec3::truncate)
                .collect();
            (points.len() >= 4).then(|| {
                let min = points.iter().fold(Vec2::splat(f32::MAX), |m, p| m.min(*p));
                let max = points.iter().fold(Vec2::splat(f32::MIN), |m, p| m.max(*p));
                (min, max)
            })
        };
        // corner brackets around a box, `pad` pixels out, kept inside the picture
        let brackets = |(min, max): (Vec2, Vec2), pad: f32, l: f32| {
            let (x0, y0) = ((min.x - pad).max(60.0), (min.y - pad).max(60.0) + top);
            let (x1, y1) = (
                (max.x + pad).min(w - 60.0),
                (max.y + pad).min(h - 60.0) + top,
            );
            let path = format!(
                "M{x0} {a} V{y0} H{b} M{c} {y0} H{x1} V{a} M{x1} {d} V{y1} H{c} M{b} {y1} H{x0} V{d}",
                a = y0 + l,
                b = x0 + l,
                c = x1 - l,
                d = y1 - l,
            );
            (path, x1, y0, y1)
        };

        // The engineer: the brackets snap in from a larger box.
        let lock = smoothstep(0.5, 1.1, t) * (1.0 - smoothstep(WHIP - 0.2, WHIP + 0.1, t));
        let corners = [
            vec3(-0.3, 0.0, 0.0),
            vec3(0.3, 1.82, 0.0),
            vec3(0.3, 0.0, 0.0),
            vec3(-0.3, 1.82, 0.0),
        ]
        .map(|c| ENGINEER + c);
        let mut target = Svgr::empty();
        if let (true, Some(bounds)) = (lock > 0.0, project(&corners)) {
            let (path, x1, y0, _) = brackets(bounds, 40.0 + 260.0 * (1.0 - lock), 70.0);
            // the role types itself after a blinking cursor
            let role = if t < 2.4 {
                let cursor = if ((t * 3.0) as usize).is_multiple_of(2) {
                    "_"
                } else {
                    ""
                };
                format!("ROLE: {cursor}")
            } else {
                format!("ROLE: {}", &"NONE"[..(((t - 2.4) * 14.0) as usize).min(4)])
            };
            let x = (x1 + 36.0).min(w - 600.0);
            target = svgr!(
                <g opacity={lock}>
                    <path d={path} fill="none" stroke={color} stroke-width="4" />
                    <rect x={x - 18.0} y={y0} width="560" height="168" fill="#04080e" opacity="0.5" />
                    <text x={x} y={y0 + 40.0} font-family={HUD} font-weight="600" font-size="44" fill={color}>"HUMAN"</text>
                    <text x={x} y={y0 + 96.0} font-family={HUD} font-size="34" fill={color} opacity="0.8">"ID 0451 · CONF 99.7%"</text>
                    <text x={x} y={y0 + 146.0} font-family={HUD} font-weight="600" font-size="34" fill="#ffffff">{role}</text>
                </g>
            );
        }

        // The chip U17: brackets that follow it down to its footprint, and its error that
        // goes to zero as it comes down.
        let place = smoothstep(ARRIVE - 0.1, ARRIVE + 0.3, t);
        let (height, turn) = placing(t);
        let chip = self.watcher.base.transform_point3(seated()) + vec3(0.0, height, 0.0);
        let corners = [(-1.0f32, -1.0f32), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)]
            .map(|(x, z)| chip + vec3(x * CHIP_HALF, CHIP_TOP, z * CHIP_HALF));
        let mut placement = Svgr::empty();
        if let (true, Some(bounds)) = (place > 0.0, project(&corners)) {
            let (path, x1, y0, y1) = brackets(bounds, 24.0 + 120.0 * (1.0 - place), 50.0);
            let seated = t > TOUCH;
            let state = if t > RELEASE + 0.1 {
                "PLACED · OK"
            } else if seated {
                "SEATED"
            } else {
                "ALIGN"
            };
            let z = format!("Z  {:+.2} MM", height * 1000.0);
            let theta = format!("ROT {:+.2} DEG", turn.to_degrees().max(0.0));
            let x = (x1 + 30.0).min(w - 600.0);
            let mid = f32::midpoint(y0, y1);
            placement = svgr!(
                <g opacity={place}>
                    <path d={path} fill="none" stroke={color} stroke-width="3" />
                    <rect x={x - 18.0} y={y0} width="520" height="200" fill="#04080e" opacity="0.5" />
                    <text x={x} y={y0 + 34.0} font-family={HUD} font-weight="600" font-size="38" fill={color}>"U17 · QFP-176"</text>
                    <text x={x} y={y0 + 84.0} font-family={HUD} font-size="32" fill={color} opacity="0.85">{z}</text>
                    <text x={x} y={y0 + 128.0} font-family={HUD} font-size="32" fill={color} opacity="0.85">{theta}</text>
                    <text x={x} y={y0 + 178.0} font-family={HUD} font-weight="600" font-size="32" fill="#ffffff">{state}</text>
                    <line x1={f32::midpoint(bounds.0.x, bounds.1.x) - 24.0} y1={mid} x2={f32::midpoint(bounds.0.x, bounds.1.x) + 24.0} y2={mid} stroke={color} stroke-width="2" opacity="0.7" />
                </g>
            );
        }

        let status = if t < WHIP {
            "TASK: PLACE U17 · PAUSED"
        } else {
            "TASK: PLACE U17 · RESUMED"
        };
        let clock = format!("T+{:06.2}", 723.0 + t);
        let frame = format!(
            "M80 {a} V{b} H200 M{c} {b} H{d} V{a} M{d} {e} V{f} H{c} M200 {f} H80 V{e}",
            a = top + 200.0,
            b = top + 80.0,
            c = w - 200.0,
            d = w - 80.0,
            e = top + h - 200.0,
            f = top + h - 80.0,
        );
        let scan = top + h * ((t * 0.9) % 1.0);
        svgr!(
            <g>
            {crate::look::iris(t, 0.4)}
            <g opacity={fade}>
                <path d={frame} fill="none" stroke={color} stroke-width="3" opacity="0.7" />
                <text x="120" y={top + 140.0} font-family={HUD} font-size="34" fill={color} opacity="0.85">"AX-07 · VISION"</text>
                <text x="120" y={top + 190.0} font-family={HUD} font-size="30" fill={color} opacity="0.6">{clock}</text>
                <text x={w - 120.0} y={top + 140.0} font-family={HUD} font-size="34" fill={color} opacity="0.85" text-anchor="end">"LINE 07 · STATION 62"</text>
                <text x={w - 120.0} y={top + 190.0} font-family={HUD} font-size="30" fill={color} opacity="0.6" text-anchor="end">"THROUGHPUT 14 212 / H"</text>
                <text x="120" y={top + h - 120.0} font-family={HUD} font-size="32" fill={color} opacity="0.85">{status}</text>
                <circle cx={w * 0.5} cy={top + h * 0.5} r="18" fill="none" stroke={color} stroke-width="3" opacity="0.6" />
                <rect x="0" y={scan} width={w} height="2" fill={color} opacity="0.12" />
                {target}
                {placement}
            </g>
            </g>
        )
    }
}

/// The light of the chip at the start of shot 4 (`crate::city`): a dark night sky, a band of
/// city glow at the horizon and one large soft light straight above, times `intensity`.
fn chip_light(intensity: f32) -> Environment {
    let glow = Rgb::hex(0x5d7cb0) * 0.9;
    let band = |azimuth: f32| {
        let a = deg(azimuth);
        SoftBox::new(vec3(a.sin(), 0.05, a.cos()), 125.0, 7.0, glow)
    };
    Environment::Studio(Studio {
        sky: Rgb::hex(0x03060d),
        horizon: Rgb::hex(0x1c2c48),
        ground: Rgb::hex(0x0b1220),
        softboxes: vec![
            band(0.0),
            band(120.0),
            band(240.0),
            SoftBox::new(vec3(0.12, 1.0, 0.08), 64.0, 52.0, Rgb::hex(0xf4f1ea) * 1.1),
        ],
        intensity,
        rotation: 0.0,
    })
}
