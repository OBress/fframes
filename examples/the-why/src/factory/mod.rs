//! The factory: shots 2 (the factory of answers), 3 (the engineer, seen by a robot) and 7 (the
//! wave). Thousands of arms work on eight production lines in a hall of 120 by 440 meters.

pub mod arm;
mod crowd;
mod hall;

use std::f32::consts::FRAC_PI_2;

use fframes_3d::math::{hash, noise1, smoothstep};
use fframes_3d::*;

use fframes::{Svgr, svgr};

use crate::WIDTH;
use crate::assets::Assets;
use crate::figure::{Figure, FigureStyle, Joint, Pose};
use crate::look::{BAR, HUD, PICTURE_HEIGHT, lens};
use arm::{Arm, Expression, Joints, Palette};
use crowd::{Crowd, Posed, Station};
use hall::Hall;

/// The color of the eyes and the status lights of the arms at work.
const COLD: u32 = 0x6fb8ff;
/// The hero arms stand on the aisle side of line 7 and face the conveyor (+X). The first welds
/// in the close-up that opens shot 2. The second stands under the catwalk and sees the
/// engineer.
const WELDER_Z: f32 = -24.5;
const WATCHER_Z: f32 = -62.0;
/// The work phases of the hero arms. The welder welds at the start of shot 2.
const WELDER_PHASE: f32 = 1.2;
const WATCHER_PHASE: f32 = 3.1;
/// The time in shot 7 at which the light of the line reaches the glass wall, and its speed
/// down the hall in meters per second.
const WAVE_ARRIVAL: f32 = 0.9;
const WAVE_SPEED: f32 = 45.0;
/// Where the engineer stands on the catwalk (his feet).
pub const ENGINEER: Vec3 = Vec3::new(3.2, hall::CATWALK_Y, hall::CATWALK_Z + 0.35);

/// A hero arm: the detailed model at a station.
struct Hero {
    arm: Arm,
    base: Mat4,
    phase: f32,
}

/// The factory set, built once.
pub struct FactorySet {
    scene: Scene,
    hall: Hall,
    crowd: Crowd,
    stations: Vec<Station>,
    welder: Hero,
    watcher: Hero,
    sun: NodeId,
    engineer: Figure,
    /// The chip in the gripper of the watcher.
    chip: NodeId,
}

/// The jig where the watcher places the chip, in the frame of its base: the top of the
/// circuit board.
const JIG: Vec3 = Vec3::new(0.0, 0.98, 1.25);
/// The size of the chip: its width and its height in meters.
const CHIP: (f32, f32) = (0.04, 0.004);

/// The base of an arm on the aisle side of line 7 at `z`, facing the conveyor.
fn aisle_station(z: f32) -> Mat4 {
    Mat4::from_rotation_translation(Quat::from_rotation_y(FRAC_PI_2), vec3(7.0 - 2.2, 0.0, z))
}

impl FactorySet {
    pub fn new(assets: &Assets) -> Self {
        let mut scene = Scene::new();
        let hall = Hall::build(&mut scene, assets);
        let palette = Palette::new();
        let font = include_bytes!("../../media/Inter_24pt-Bold.ttf");
        let detail = crowd::Detail::bake(&palette, font);
        let crowd = Crowd::build(&mut scene, &crowd::Model::new(), &detail, &palette);
        let mut hero = |z: f32, phase: f32| {
            let arm = Arm::build(&mut scene, &palette, font);
            let base = aisle_station(z);
            let (_, rotation, translation) = base.to_scale_rotation_translation();
            scene[arm.base].position = translation;
            scene[arm.base].rotation = rotation;
            Hero { arm, base, phase }
        };
        let welder = hero(WELDER_Z, WELDER_PHASE);
        let watcher = hero(WATCHER_Z, WATCHER_PHASE);
        let heroes = [welder.base, watcher.base].map(|b| b.w_axis.truncate());
        let stations = crowd::stations(&hall::LINES, 10.0, hall::Z_FAR + 8.0, 4.5)
            .into_iter()
            .filter(|s| {
                heroes
                    .iter()
                    .all(|h| s.base.w_axis.truncate().distance(*h) > 0.5)
            })
            .collect();
        let sun = scene.add(Node::group());
        scene.environment = Environment::map(assets.factory_sky.clone(), 0.35);
        scene.reflector = Some(Reflector::floor());

        // The engineer on the catwalk, and his old laptop on the rail.
        let engineer = Figure::build(&mut scene, None, &FigureStyle::default());
        let laptop = Geometry::merge([
            Geometry::rounded_box(0.33, 0.011, 0.23, 0.004, 2).translate(0.0, 0.0055, 0.0),
            Geometry::rounded_box(0.33, 0.007, 0.23, 0.004, 2).translate(0.0, 0.0145, 0.0),
        ]);
        scene.add(
            Node::mesh(laptop, Material::metal(0x6a6d72, 0.4))
                .at(
                    ENGINEER.x + 0.62,
                    hall::CATWALK_Y + 1.1,
                    hall::CATWALK_Z + 0.79,
                )
                .rotate_y(0.12),
        );

        // The jig beside the watcher with a circuit board, and the chip in its gripper.
        let jig = watcher.base.transform_point3(JIG);
        scene.add(
            Node::mesh(
                Geometry::cuboid(0.5, JIG.y - 0.03, 0.5),
                Material::metal(0x3a3e45, 0.5),
            )
            .at(jig.x, (JIG.y - 0.03) * 0.5, jig.z),
        );
        scene.add(
            Node::mesh(
                Geometry::cuboid(0.3, 0.016, 0.22),
                Material::standard(0x0b1a24).roughness(0.35).clearcoat(0.6),
            )
            .at(jig.x, JIG.y - 0.008, jig.z),
        );
        let chip = scene.add(Node::mesh(
            Geometry::rounded_box(CHIP.0, CHIP.1, CHIP.0, 0.0008, 1),
            Material::standard(0x0c0d0f).roughness(0.3).clearcoat(0.5),
        ));
        // the inspection lamp over the jig
        scene.add(
            Node::light(Light::spot(0xe8f2ff, 6.0, 0.5, 0.6).pointing(vec3(0.15, -1.0, 0.1))).at(
                jig.x - 0.2,
                JIG.y + 1.1,
                jig.z - 0.15,
            ),
        );
        Self {
            scene,
            hall,
            crowd,
            stations,
            welder,
            watcher,
            sun,
            engineer,
            chip,
        }
    }

    /// Poses the engineer: he stands at the rail and looks down at the arms.
    fn pose_engineer(&self, scene: &mut Scene, t: f32) {
        let mut pose = Pose::stand(t + 3.0).placed(ENGINEER, 0.0);
        pose[Joint::Head] *= Quat::from_rotation_x(0.35);
        pose[Joint::Neck] *= Quat::from_rotation_x(0.12);
        self.engineer.apply(scene, &pose);
    }

    /// Puts the chip between the fingers of the watcher.
    fn hold_chip(&self, scene: &mut Scene, q: &Joints) {
        let grip = self.watcher.base * arm::tip(q);
        let (_, rotation, translation) = grip.to_scale_rotation_translation();
        // the chip lies flat across the fingers: its normal is along the tool axis
        scene[self.chip].rotation = rotation * Quat::from_rotation_x(-FRAC_PI_2);
        scene[self.chip].position = translation;
    }

    /// The renderer of the factory shots.
    pub fn renderer(width: u32, height: u32) -> Renderer {
        Renderer::new(width, height).exposure(1.05).bloom(Bloom {
            strength: 0.55,
            radius: 0.55,
            threshold: 1.2,
        })
    }

    /// The hall by day: cold light falls through the roof glazing.
    fn day(&self, t: f32, shadow_bounds: Aabb) -> Scene {
        let mut scene = self.scene.clone();
        scene[self.sun].object = Object::Light(
            Light::directional(0xdfe9ff, 7.0, vec3(0.35, 0.75, -1.0)).with_shadow(Shadow {
                map_size: 4096,
                softness: 1.5,
                bounds: Some(shadow_bounds),
                ..Default::default()
            }),
        );
        scene.atmosphere = Some(Atmosphere {
            density: 0.025,
            color: Rgb::hex(0xe4ecff),
            falloff: 0.02,
            anisotropy: 0.7,
            ambient: Rgb::hex(0x070b12),
            noise: 0.55,
            noise_size: 7.0,
            offset: vec3(0.4, 0.05, 0.25) * t,
            distance: 220.0,
            steps: 40,
            ..Default::default()
        });
        scene.background = Background::Color(Rgb::hex(0xc8d6ea));
        scene.add(Node::light(Light::hemisphere(0x5d6a80, 0x16181c, 0.5)));
        self.hall.light_lamps(&mut scene, Rgb::hex(0xdcecff) * 6.0);
        scene
    }

    /// Poses the crowd at time `t` and adds the sparks of the arms near `camera`.
    fn work(&self, scene: &mut Scene, t: f32, camera: Vec3) {
        let hint = crowd::rest();
        let eye = Rgb::hex(COLD) * 4.0;
        let posed: Vec<Posed> = self
            .stations
            .iter()
            .map(|s| Posed {
                base: s.base,
                joints: crowd::joints(&crowd::work(t + s.phase), &hint),
                eye,
            })
            .collect();
        self.crowd.pose(scene, &posed, |a| near(a, camera));
        self.hall.conveyors(
            scene,
            t,
            camera.z + 20.0,
            camera.z - 180.0,
            Rgb::hex(0x8fd0ff) * 5.0,
        );

        let mut sparks = Vec::new();
        let mut arcs = Vec::new();
        for (index, (station, arm)) in self.stations.iter().zip(&posed).enumerate() {
            let distance = station.base.w_axis.truncate().distance(camera);
            if distance > 60.0 {
                continue;
            }
            let tip =
                crowd::frames(arm.base, &arm.joints)[6].transform_point3(vec3(0.0, 0.0, arm::GRIP));
            let welding = |b: f32| crowd::work(b + station.phase).welding;
            if welding(t) {
                arcs.push((distance, index as u32, tip));
            }
            sparks.extend(weld_sparks(index as u32, tip, t, welding));
        }
        scene.add(Node::particles(sparks, Blending::Additive));
        // the nearest arcs light their surroundings
        arcs.sort_by(|a, b| a.0.total_cmp(&b.0));
        for (_, seed, tip) in arcs.into_iter().take(4) {
            scene.add(arc_light(seed, tip, t));
        }
    }

    /// Shot 2: the factory of answers. The welding arc of the first hero arm in close-up. The
    /// camera slides down into the center aisle and dollies at floor level, 24 mm. At the end
    /// the second hero arm stops and turns its eye up to the engineer on the catwalk.
    pub fn moment_factory(&self, t: f32) -> (Scene, Camera) {
        let arc = self.tip(&self.welder, self.welder_joints(t));
        // the close-up holds still, then the camera moves into the aisle and dollies
        let slide = smoothstep(1.6, 5.5, t);
        let dolly = (t - 4.0).max(0.0);
        let close = vec3(6.1, 1.12, WELDER_Z + 1.25);
        let aisle = vec3(1.4, 0.55, WELDER_Z + 1.0 - 1.15 * dolly);
        let position = close.lerp(aisle, slide);
        let ahead = vec3(3.0, 3.2, WATCHER_Z - 30.0);
        let target = arc.lerp(ahead, slide);
        let mut camera = lens(24.0)
            .position(position)
            .look_at(target)
            .shake(t * 0.4, 0.004, 7);
        camera.far = 600.0;
        // the focus moves from the arc to the watcher
        let watcher = self.watcher.base.w_axis.truncate() + vec3(0.6, 1.8, 0.0);
        let camera = camera.focus_on(arc.lerp(watcher, slide), 14.0 - 11.0 * slide);
        let bounds = Aabb {
            min: vec3(-hall::HALF_WIDTH, 0.0, position.z - 150.0),
            max: vec3(hall::HALF_WIDTH, hall::WALL + 4.0, position.z + 25.0),
        };
        let mut scene = self.day(t, bounds);
        self.work(&mut scene, t, position);
        self.pose_hero(&mut scene, &self.welder, self.welder_joints(t), t, 0.0);
        let awake = smoothstep(10.4, 11.2, t);
        let watcher = self.watcher_joints(t);
        self.pose_hero(&mut scene, &self.watcher, watcher, t.min(9.6), awake);
        self.hold_chip(&mut scene, &watcher);
        self.pose_engineer(&mut scene, t);
        (scene, camera)
    }

    pub fn shutter_factory(&self, t: f32) -> usize {
        if (1.4..6.0).contains(&t) { 6 } else { 3 }
    }

    /// Shot 3: the engineer, seen by the watcher. The camera is its eye. It locks on to the
    /// engineer, finds no role for him, and goes back to work: it places the chip on the jig.
    /// The camera leaves the eye and dives into the chip.
    pub fn moment_engineer(&self, t: f32) -> (Scene, Camera) {
        let q = self.pov_joints(t);
        let camera = self.pov_camera(t, &q);
        let bounds = Aabb {
            min: vec3(-30.0, 0.0, hall::CATWALK_Z - 120.0),
            max: vec3(30.0, hall::WALL + 4.0, hall::CATWALK_Z + 20.0),
        };
        let time = t + 12.0;
        let mut scene = self.day(time, bounds);
        self.work(&mut scene, time, camera.position);
        self.pose_hero(
            &mut scene,
            &self.welder,
            self.welder_joints(time),
            time,
            0.0,
        );
        self.pose_hero(&mut scene, &self.watcher, q, 9.6, 1.0);
        self.hold_chip(&mut scene, &q);
        self.pose_engineer(&mut scene, time);
        (scene, camera)
    }

    pub fn shutter_engineer(&self, t: f32) -> usize {
        if t > 5.0 { 5 } else { 2 }
    }

    /// The joints of the watcher in shot 3. It looks at the engineer with small saccades, then
    /// brings the chip over the jig and lowers it.
    fn pov_joints(&self, t: f32) -> Joints {
        let looking = self.watcher_joints(12.0);
        // saccades: the gaze jumps to a new point every 0.7 s and settles fast
        let step = (t / 0.7).floor();
        let k = smoothstep(0.0, 0.18, t - step * 0.7);
        let jitter = |n: f32| vec2(noise1(11, n), noise1(12, n)) * 0.012;
        let offset = jitter(step - 1.0).lerp(jitter(step), k);
        let mut gaze = looking;
        gaze.q[0] += offset.x;
        gaze.q[4] += offset.y;
        let over = crowd::tool_down_at(JIG + vec3(0.0, 0.26, 0.0), &gaze);
        let placed = crowd::tool_down_at(JIG + vec3(0.0, CHIP.1 * 0.5 + 0.001, 0.0), &over);
        let blend = |a: &Joints, b: &Joints, k: f32| Joints {
            q: std::array::from_fn(|i| crowd::turn(a.q[i], b.q[i], k)),
        };
        let raised = blend(&gaze, &over, smoothstep(5.1, 6.7, t));
        blend(&raised, &placed, smoothstep(6.6, 8.2, t))
    }

    /// The camera of shot 3: the eye of the watcher, then the dive into the chip.
    fn pov_camera(&self, t: f32, q: &Joints) -> Camera {
        let flange = self.watcher.base * arm::flange(q);
        let forward = flange.transform_vector3(Vec3::Z).normalize();
        // just in front of the lens, with the image almost level
        let eye = flange.transform_point3(arm::EYE) + forward * 0.06;
        let up = Quat::from_axis_angle(forward, 0.06) * Vec3::Y;
        // the eye zooms in steps, like a machine, and back out when it returns to work
        let stage = smoothstep(0.6, 0.85, t) + smoothstep(1.6, 1.85, t) + smoothstep(2.6, 2.85, t)
            - 3.0 * smoothstep(5.0, 5.4, t);
        let zoom = [35.0f32, 50.0, 70.0, 85.0];
        let i = (stage.floor() as usize).min(2);
        let mm = zoom[i] + (zoom[i + 1] - zoom[i]) * (stage - i as f32);
        // the dive: from the eye to straight above the chip
        let turn = smoothstep(7.2, 9.0, t);
        let dive = smoothstep(7.2, 10.0, t);
        let chip = self
            .watcher
            .base
            .transform_point3(JIG + vec3(0.0, CHIP.1, 0.0));
        let above = chip + vec3(0.0, 0.0432, 0.0);
        let position = eye.lerp(above, dive * dive);
        let look = forward.lerp(Vec3::NEG_Y, turn).normalize();
        let mut camera = lens(mm + (35.0 - mm) * turn)
            .position(position)
            .look_at(position + look);
        camera.up = up.lerp(Vec3::NEG_Z, turn).normalize();
        camera.near = 0.005;
        camera.far = 600.0;
        // the eye hunts for focus on the engineer, then focuses on the chip
        let head = ENGINEER + vec3(0.0, 1.6, 0.0);
        let hunt = 1.0 + 0.35 * (1.0 - smoothstep(0.0, 0.9, t)) * (t * 30.0).sin();
        let focus = if t < 5.6 {
            eye + (head - eye) * hunt
        } else {
            chip
        };
        camera.focus_on(focus, 4.0 + 6.0 * smoothstep(5.6, 8.0, t))
    }

    /// The HUD of the eye in shot 3: brackets on the engineer, labels and telemetry.
    pub fn overlay_engineer(&self, t: f32) -> Svgr<'static> {
        let fade = (1.0 - smoothstep(7.4, 8.4, t)) * smoothstep(0.0, 0.25, t);
        if fade <= 0.0 {
            return crate::look::iris(t, 0.4);
        }
        let q = self.pov_joints(t);
        let camera = self.pov_camera(t, &q);
        let (w, h) = (WIDTH as f32, PICTURE_HEIGHT as f32);
        let top = BAR as f32;
        let color = "#9fd4ff";
        // the box of the engineer on the picture
        let corners = [
            vec3(-0.3, 0.0, 0.0),
            vec3(0.3, 1.82, 0.0),
            vec3(0.3, 0.0, 0.0),
            vec3(-0.3, 1.82, 0.0),
        ];
        let points: Vec<Vec2> = corners
            .iter()
            .filter_map(|c| camera.project(ENGINEER + *c, w, h))
            .map(Vec3::truncate)
            .collect();
        let lock = smoothstep(0.5, 1.1, t) * (1.0 - smoothstep(5.0, 5.5, t));
        let mut target = Svgr::empty();
        if points.len() == 4 && lock > 0.0 {
            let min = points.iter().fold(Vec2::splat(f32::MAX), |m, p| m.min(*p));
            let max = points.iter().fold(Vec2::splat(f32::MIN), |m, p| m.max(*p));
            // the brackets snap in from a larger box
            let pad = 40.0 + 260.0 * (1.0 - lock);
            let (x0, y0) = (min.x - pad, min.y - pad + top);
            let (x1, y1) = (max.x + pad, max.y + pad + top);
            let l = 70.0;
            let brackets = format!(
                "M{x0} {a} V{y0} H{b} M{c} {y0} H{x1} V{a} M{x1} {d} V{y1} H{c} M{b} {y1} H{x0} V{d}",
                a = y0 + l,
                b = x0 + l,
                c = x1 - l,
                d = y1 - l,
            );
            // the role types itself after a blinking cursor
            let role = if t < 2.4 {
                format!(
                    "ROLE: {}",
                    if ((t * 3.0) as usize).is_multiple_of(2) {
                        "_"
                    } else {
                        ""
                    }
                )
            } else {
                format!("ROLE: {}", &"NONE"[..(((t - 2.4) * 14.0) as usize).min(4)])
            };
            let x = x1 + 36.0;
            target = svgr!(
                <g opacity={lock}>
                    <path d={brackets} fill="none" stroke={color} stroke-width="4" />
                    <text x={x} y={y0 + 40.0} font-family={HUD} font-weight="600" font-size="44" fill={color}>"HUMAN"</text>
                    <text x={x} y={y0 + 96.0} font-family={HUD} font-size="34" fill={color} opacity="0.8">"ID 0451 · CONF 99.7%"</text>
                    <text x={x} y={y0 + 146.0} font-family={HUD} font-weight="600" font-size="34" fill="#ffffff">{role}</text>
                </g>
            );
        }
        let status = if t < 5.0 {
            "TASK: WELD SEAM B · PAUSED"
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
            </g>
            </g>
        )
    }

    /// The hall at night: dim cold lamps, dark haze and stars over the desert behind the glass
    /// wall. The warm light of the line comes in through the glass wall from `arrival` on.
    fn night(&self, t: f32, arrival: f32, shadow_bounds: Aabb) -> Scene {
        let mut scene = self.scene.clone();
        let warm = smoothstep(arrival, arrival + 2.5, t);
        scene[self.sun].object = Object::Light(
            Light::directional(0xffb35c, 3.0 * warm, vec3(0.0, 0.06, -1.0)).with_shadow(Shadow {
                map_size: 4096,
                softness: 2.0,
                bounds: Some(shadow_bounds),
                ..Default::default()
            }),
        );
        scene.atmosphere = Some(Atmosphere {
            density: 0.02,
            color: Rgb::hex(0xe8eef8),
            falloff: 0.03,
            anisotropy: 0.6,
            ambient: Rgb::hex(0x0a1220),
            noise: 0.5,
            noise_size: 8.0,
            offset: vec3(0.3, 0.05, 0.2) * t,
            distance: 260.0,
            steps: 40,
            ..Default::default()
        });
        scene.background = Background::Color(Rgb::hex(0x02040a));
        scene.environment = Environment::None;
        scene.add(Node::light(Light::hemisphere(0x2a3a55, 0x050608, 0.5)));
        self.hall.light_lamps(&mut scene, Rgb::hex(0xbcd8ff) * 5.0);
        self.hall.night(&mut scene);
        // the desert outside the glass wall
        scene.add(
            Node::mesh(
                Geometry::floor(4000.0, 4000.0),
                Material::standard(0x1a1714).roughness(1.0),
            )
            .at(0.0, -0.02, hall::Z_FAR - 2002.0)
            .shadows(false, false),
        );
        scene
    }

    /// The time at which the line passes `z` in shot 7.
    fn line_passes(z: f32) -> f32 {
        WAVE_ARRIVAL + (z - hall::Z_FAR) / WAVE_SPEED
    }

    /// Shot 7: the wave. The light of the line comes in through the glass wall and runs down
    /// the center aisle. Each arm stops when the line passes it and turns to the light.
    pub fn moment_wave(&self, t: f32) -> (Scene, Camera) {
        let camera = self.wave_camera(t);
        let bounds = Aabb {
            min: vec3(-hall::HALF_WIDTH, 0.0, hall::Z_FAR),
            max: vec3(hall::HALF_WIDTH, hall::WALL + 4.0, hall::Z_FAR + 260.0),
        };
        let mut scene = self.night(t, WAVE_ARRIVAL, bounds);

        // the arms: at work until the line passes, then they turn to the glass wall
        let time = t + 40.0;
        let hint = crowd::rest();
        let cold = Rgb::hex(COLD) * 3.0;
        let warm = Rgb::hex(0xffb35c) * 10.0;
        let posed: Vec<Posed> = self
            .stations
            .iter()
            .map(|s| {
                let z = s.base.w_axis.z;
                let passes = Self::line_passes(z);
                let stop = time.min(passes + 40.0);
                let working = crowd::joints(&crowd::work(stop + s.phase), &hint);
                let k = smoothstep(passes, passes + 1.3, t);
                let joints = if k > 0.0 {
                    let eye = vec3(0.0, 1.95, 0.6);
                    let look = s.base.inverse().transform_vector3(vec3(0.0, 0.3, -1.0));
                    let looking = crowd::looking(eye, look, &working);
                    Joints {
                        q: std::array::from_fn(|i| crowd::turn(working.q[i], looking.q[i], k)),
                    }
                } else {
                    working
                };
                Posed {
                    base: s.base,
                    joints,
                    eye: cold.0.lerp(warm.0, k).into(),
                }
            })
            .collect();
        self.crowd
            .pose(&mut scene, &posed, |a| near(a, camera.position));
        self.hall.conveyors(
            &mut scene,
            time.min(WAVE_ARRIVAL + 40.0),
            camera.position.z + 40.0,
            camera.position.z - 260.0,
            Rgb::hex(0x8fd0ff) * 3.0,
        );
        for hero in [&self.welder, &self.watcher] {
            let z = hero.base.w_axis.z;
            let passes = Self::line_passes(z);
            let working = crowd::joints(&crowd::work(time.min(passes + 40.0) + hero.phase), &hint);
            let k = smoothstep(passes, passes + 1.3, t);
            let look = hero.base.inverse().transform_vector3(vec3(0.0, 0.3, -1.0));
            let looking = crowd::looking(vec3(0.0, 1.95, 0.6), look, &working);
            let q = Joints {
                q: std::array::from_fn(|i| crowd::turn(working.q[i], looking.q[i], k)),
            };
            hero.arm.apply(
                &mut scene,
                &q,
                arm::OPEN * 0.4,
                &Expression {
                    ring: 2.0 * (1.0 - k),
                    status: 3.0 * (1.0 - k),
                    eye: 4.0 + 8.0 * k,
                    lid: 1.0,
                },
            );
        }
        self.hold_chip(&mut scene, &self.watcher_joints(0.0));
        // nobody stands on the catwalk at night
        scene[self.engineer.root].visible = false;
        self.add_line(&mut scene, t);
        (scene, camera)
    }

    pub fn shutter_wave(&self, t: f32) -> usize {
        if t < 3.0 { 6 } else { 4 }
    }

    /// The light of the line: a streak over the desert, then a glowing line on the floor of
    /// the aisle with a bright head that lights the arms as it passes.
    fn add_line(&self, scene: &mut Scene, t: f32) {
        let core = Rgb::hex(0xffb35c);
        if t < WAVE_ARRIVAL {
            // the streak outside, coming to the glass wall along the ground
            let z = hall::Z_FAR - (WAVE_ARRIVAL - t) * 400.0;
            let head = vec3(0.0, 0.15, z);
            scene.add(Node::particles(
                vec![Particle {
                    position: head,
                    tail: vec3(0.0, 0.0, -60.0),
                    size: 0.5,
                    color: core * 40.0,
                    opacity: 1.0,
                }],
                Blending::Additive,
            ));
            scene.add(Node::light(Light::point(0xffb35c, 400.0, 0.0)).at(head.x, 0.6, head.z));
            return;
        }
        let front = (hall::Z_FAR + (t - WAVE_ARRIVAL) * WAVE_SPEED).min(hall::Z_NEAR - 2.0);
        let length = front - hall::Z_FAR;
        if length > 0.01 {
            scene.add(
                Node::mesh(
                    Geometry::cuboid(0.05, 0.01, length),
                    Material::glow(core, 14.0),
                )
                .at(0.0, 0.006, hall::Z_FAR + length * 0.5)
                .shadows(false, false),
            );
            // a soft glow around the line on the floor
            scene.add(
                Node::mesh(
                    Geometry::cuboid(0.5, 0.004, length),
                    Material::glow(core, 0.6),
                )
                .at(0.0, 0.003, hall::Z_FAR + length * 0.5)
                .shadows(false, false),
            );
        }
        if front < hall::Z_NEAR - 3.0 {
            let head = vec3(0.0, 0.05, front);
            scene.add(Node::particles(
                vec![
                    Particle::new(head, 0.12, core * 80.0),
                    Particle::new(head, 0.6, core * 4.0),
                ],
                Blending::Additive,
            ));
            scene.add(Node::light(Light::point(0xffb35c, 60.0, 0.0)).at(head.x, 0.5, head.z));
        }
        // lights along the line keep the floor and the arms warm
        for i in 0..4 {
            let z = hall::Z_FAR + 30.0 + i as f32 * 60.0;
            if z < front {
                scene.add(Node::light(Light::point(0xffa04a, 8.0, 30.0)).at(0.0, 0.3, z));
            }
        }
    }

    /// The camera of shot 7: it waits at the glass wall, rises and turns to watch the wave
    /// go down the hall, then comes down to the floor and frames the line across the
    /// picture with a long lens.
    fn wave_camera(&self, t: f32) -> Camera {
        let z0 = hall::Z_FAR;
        let at_glass = vec3(0.4, 1.4, z0 + 22.0);
        let high = vec3(-3.0, 4.5, z0 + 14.0) + vec3(0.0, 0.0, 6.0) * smoothstep(2.6, 5.0, t);
        let low = vec3(-4.0, 0.035, z0 + 95.0);
        let rise = smoothstep(1.1, 2.8, t);
        let fall = smoothstep(5.0, 8.0, t);
        let position = at_glass.lerp(high, rise).lerp(low, fall);
        let glass = vec3(0.0, 1.0, z0 - 60.0);
        let hall_view = vec3(0.0, 1.2, z0 + 120.0);
        let line = vec3(0.0, 0.006, z0 + 95.0);
        let target = glass
            .lerp(hall_view, smoothstep(1.2, 2.9, t))
            .lerp(line, fall);
        let mm = 35.0 + 100.0 * smoothstep(5.5, 8.0, t);
        let shake = 0.004 * (1.0 - smoothstep(6.0, 8.0, t));
        let mut camera = lens(mm)
            .position(position)
            .look_at(target)
            .shake(t * 0.4, shake, 21);
        camera.near = 0.02;
        camera.far = 3000.0;
        // the focus follows the front of the wave, then rests on the line
        let front = z0 + ((t - WAVE_ARRIVAL) * WAVE_SPEED).clamp(15.0, 90.0);
        let focus = vec3(0.0, 1.5, front).lerp(line, fall);
        camera.focus_on(focus, 3.0 + 6.0 * fall)
    }

    /// The joints of the welder: it works with its neighbors.
    fn welder_joints(&self, t: f32) -> Joints {
        crowd::joints(&crowd::work(t + self.welder.phase), &crowd::rest())
    }

    /// The joints of the watcher: it works, stops at 9.6 s and looks up at the engineer.
    fn watcher_joints(&self, t: f32) -> Joints {
        let working = crowd::joints(
            &crowd::work(t.min(9.6) + self.watcher.phase),
            &crowd::rest(),
        );
        let head = ENGINEER + vec3(0.0, 1.6, 0.0);
        let eye = vec3(-0.25, 1.85, 0.75);
        let look = self.watcher.base.inverse().transform_point3(head) - eye;
        let looking = crowd::looking(eye, look, &working);
        let k = smoothstep(9.9, 11.6, t);
        Joints {
            q: std::array::from_fn(|i| crowd::turn(working.q[i], looking.q[i], k)),
        }
    }

    /// The grip point of a hero arm in the world.
    fn tip(&self, hero: &Hero, q: Joints) -> Vec3 {
        (hero.base * arm::tip(&q)).w_axis.truncate()
    }

    /// Poses a hero arm and adds the sparks of its torch. `work_time` is the time of its work
    /// cycle; `awake` brightens its eye.
    fn pose_hero(&self, scene: &mut Scene, hero: &Hero, q: Joints, work_time: f32, awake: f32) {
        hero.arm.apply(
            scene,
            &q,
            arm::OPEN * 0.4,
            &Expression {
                ring: 2.0,
                status: 3.0,
                eye: 4.0 + 10.0 * awake,
                lid: 1.0,
            },
        );
        let phase = hero.phase;
        let tip = self.tip(hero, q);
        let welding = |b: f32| crowd::work(b + phase).welding;
        let seed = hero.phase.to_bits();
        if welding(work_time) {
            scene.add(arc_light(seed, tip, work_time));
        }
        scene.add(Node::particles(
            weld_sparks(seed, tip, work_time, welding),
            Blending::Additive,
        ));
    }
}

/// Whether an arm is near enough to `camera` to show the detailed parts.
fn near(arm: &Posed, camera: Vec3) -> bool {
    arm.base.w_axis.truncate().distance(camera) < 18.0
}

/// The sparks of a welding torch at `tip` at time `t`. `welding(b)` tells whether the torch
/// burned at time `b`. The sparks have no state: each spark comes from its birth time.
fn weld_sparks(seed: u32, tip: Vec3, t: f32, welding: impl Fn(f32) -> bool) -> Vec<Particle> {
    const RATE: f32 = 240.0;
    const LIFE: f32 = 0.7;
    let mut out = Vec::new();
    let newest = (t * RATE).floor() as i64;
    for k in 0..(RATE * LIFE) as i64 {
        let born = newest - k;
        let birth = born as f32 / RATE;
        if !welding(birth) {
            continue;
        }
        let age = t - birth;
        let h = |i: u32| {
            hash(
                seed.wrapping_mul(7919)
                    .wrapping_add(born as u32)
                    .wrapping_mul(31)
                    .wrapping_add(i),
            )
        };
        // most sparks leave upward and outward, away from the part
        let velocity =
            vec3(h(1) * 2.0 - 1.0, h(2) * 1.2 + 0.4, h(3) * 2.0 - 1.0) * (1.5 + 3.5 * h(4) * h(4));
        let gravity = vec3(0.0, -9.8, 0.0);
        let mut position = tip + velocity * age + gravity * (0.5 * age * age);
        let speed = velocity + gravity * age;
        // the sparks bounce off the parts on the belt
        let floor = hall::BELT + 0.23;
        if position.y < floor {
            position.y = floor + (floor - position.y) * 0.3;
        }
        let life = 1.0 - age / (LIFE * (0.4 + 0.6 * h(5)));
        if life <= 0.0 {
            continue;
        }
        let heat = life * life;
        let color = Rgb::linear(0.8 + 0.2 * heat, 0.88 + 0.12 * heat, 1.0) * (3.0 + 45.0 * heat);
        out.push(Particle {
            position,
            tail: -speed * 0.01,
            size: 0.003 + 0.002 * h(6),
            color,
            opacity: 1.0,
        });
    }
    if welding(t) {
        // the arc: a flickering white core in a blue halo
        let flicker = 0.75 + 0.25 * noise1(seed, t * 45.0);
        out.push(Particle::new(
            tip,
            0.008,
            Rgb::hex(0xffffff) * (900.0 * flicker),
        ));
        out.push(Particle::new(
            tip,
            0.05,
            Rgb::hex(0x9fc8ff) * (6.0 * flicker),
        ));
    }
    out
}

/// The flickering light of a welding arc at `tip`.
fn arc_light(seed: u32, tip: Vec3, t: f32) -> Node {
    let flicker = 0.7 + 0.3 * noise1(seed, t * 45.0);
    Node::light(Light::point(0xbfdcff, 6.0 * flicker, 9.0)).at(tip.x, tip.y + 0.05, tip.z)
}
