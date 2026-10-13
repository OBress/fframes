//! The factory: shots 2 (the factory of answers), 3 (the engineer, seen by a robot) and 7 (the
//! wave). Thousands of arms work on eight production lines in a hall of 120 by 440 meters.

pub mod arm;
mod board;
mod crowd;
mod engineer;
mod hall;
mod wave;

use std::f32::consts::FRAC_PI_2;

use fframes_3d::math::{hash, noise1, smoothstep};
use fframes_3d::*;

use crate::assets::Assets;
use crate::figure::{Figure, FigureStyle, Joint, Pose, RIGHT};
use crate::look::{ASPECT, lens};
use arm::{Arm, Expression, Joints, Palette};
use crowd::{Crowd, Lod, Posed, Station};
use hall::Hall;

/// The intensity of the sky light in the hall by day.
const SKY: f32 = 0.35;
/// The color of the eyes and the status lights of the arms at work.
const COLD: u32 = 0x6fb8ff;
/// The color of human intent: the light of the line.
const WARM: u32 = 0xffb35c;
/// The hero arms stand on the aisle side of line 7 and face the conveyor (+X). The welder
/// welds in the close-up that opens shot 2. The watcher places chips on a circuit board under
/// the catwalk and sees the engineer.
const WELDER_Z: f32 = -21.5;
const WATCHER_Z: f32 = -71.0;
/// The work phase of the welder: its arc lights at the first frame of shot 2.
const WELDER_PHASE: f32 = 1.1;
/// The work phase of the watcher: at `WATCHER_STOPS` it holds the chip over the board.
const WATCHER_PHASE: f32 = 3.8;
/// The time in shot 2 at which the watcher stops its work.
const WATCHER_STOPS: f32 = 9.6;
/// Where the engineer stands on the catwalk (his feet).
pub const ENGINEER: Vec3 = Vec3::new(-2.5, hall::CATWALK_Y, hall::CATWALK_Z + 0.35);
/// The top of the circuit board on the jig of the watcher, in the frame of its base.
const JIG: Vec3 = Vec3::new(0.0, 0.98, 1.25);
/// The tray of chips beside the jig, in the frame of the base.
const TRAY: Vec3 = Vec3::new(0.62, 0.92, 1.1);
/// The chip of the city (`crate::city::chip`): its top and its bottom above its origin.
const CHIP_TOP: f32 = 0.0015;
const CHIP_BOTTOM: f32 = -0.0021;
/// The fingers hold the chip this far along the tool axis from the J6 flange, 3 mm above
/// the ends of the pads. The grip point of the arm is 32 mm above it.
const HOLD: f32 = 0.272;
/// Half the finger gap that holds the chip by its pins.
const CLOSED: f32 = 0.0272;
/// The turn of the chip in the gripper before the arm aligns it, in radians.
const MISALIGNED: f32 = 0.045;

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
    laptop: NodeId,
    /// The chip U17 that the watcher places.
    chip: NodeId,
    /// The inspection lamp over the jig.
    lamp: NodeId,
    /// The star sky outside the glass wall, hidden by day.
    stars: NodeId,
    /// A black sheet over the board around the chip, for the end of the dive.
    veil: NodeId,
    /// Flat pins over the left and the right pins of the seated chip, as the pins of the
    /// chip in shot 4 (`crate::city`). They show at the end of the dive.
    leads: NodeId,
}

/// The test whether a sphere can show in the picture of a camera.
struct View {
    view: Mat4,
    tan: Vec2,
    position: Vec3,
}

impl View {
    fn new(camera: &Camera) -> Self {
        let tan = (camera.fov.to_radians() * 0.5).tan();
        Self {
            view: camera.view_matrix(),
            tan: vec2(tan * ASPECT, tan),
            position: camera.position,
        }
    }

    /// Whether a sphere at `center` with `radius` meters can show in the picture.
    fn sees(&self, center: Vec3, radius: f32) -> bool {
        let p = self.view.transform_point3(center);
        let depth = -p.z;
        let margin = radius * 1.3;
        depth > -radius
            && p.x.abs() <= depth * self.tan.x + margin
            && p.y.abs() <= depth * self.tan.y + margin
    }

    /// How an arm with its base at `base` draws. Arms just out of the view still draw for
    /// their shadows. The distances are for the 24 mm lens (0.314 is the tangent of half its
    /// vertical field of view) and grow with the zoom.
    fn lod(&self, base: Vec3) -> Lod {
        if !self.sees(base + vec3(0.0, 1.2, 0.0), 5.0) {
            return Lod::Hidden;
        }
        let distance = base.distance(self.position) * self.tan.y / 0.314;
        if distance < 13.0 {
            Lod::Detail
        } else if distance < 75.0 {
            Lod::Coarse
        } else {
            Lod::Far
        }
    }
}

/// The base of an arm on the aisle side of line 7 at `z`, facing the conveyor.
fn aisle_station(z: f32) -> Mat4 {
    Mat4::from_rotation_translation(Quat::from_rotation_y(FRAC_PI_2), vec3(7.0 - 2.2, 0.0, z))
}

/// The distance that a move covers until `x` when its speed rises from 0 to 1 between `a`
/// and `b` with a smoothstep: the integral of `smoothstep(a, b, s)` from `a` to `x`.
fn glide(a: f32, b: f32, x: f32) -> f32 {
    let u = ((x - a) / (b - a)).clamp(0.0, 1.0);
    (b - a) * (u * u * u - 0.5 * u * u * u * u) + (x - b).max(0.0)
}

/// What the gripper of the watcher does: where the chip is in the frame of its base, its
/// turn, half the finger gap, and whether the fingers hold it.
#[derive(Debug, Clone, Copy)]
struct Grip {
    chip: Vec3,
    turn: f32,
    gap: f32,
    held: bool,
}

/// The place of the chip on the board, in the frame of the base of the watcher.
fn seated() -> Vec3 {
    JIG - vec3(0.0, CHIP_BOTTOM, 0.0)
}

/// The pick and place cycle of the watcher at `u` seconds into its cycle. It carries a chip
/// from the tray to the board, lowers it, lets go, and picks the next chip. At 1.4 s it holds
/// the chip 6 cm over its place.
fn place_cycle(u: f32) -> Grip {
    let u = u.rem_euclid(crowd::CYCLE);
    let hover = seated() + vec3(0.0, 0.06, 0.0);
    let above_tray = TRAY + vec3(0.0, 0.12, 0.0);
    let lift = |k: f32| vec3(0.0, 0.1 * (std::f32::consts::PI * k).sin(), 0.0);
    let (chip, turn, gap, held) = if u < 1.4 {
        let k = smoothstep(0.0, 1.4, u);
        (
            above_tray.lerp(hover, k) + lift(k),
            MISALIGNED * k,
            CLOSED,
            true,
        )
    } else if u < 2.4 {
        let k = smoothstep(1.4, 2.4, u);
        (
            hover.lerp(seated(), k),
            MISALIGNED * (1.0 - smoothstep(1.4, 2.2, u)),
            CLOSED,
            true,
        )
    } else if u < 3.3 {
        let open = smoothstep(2.4, 2.6, u);
        let k = smoothstep(2.6, 3.3, u);
        (
            seated().lerp(above_tray, k) + lift(k),
            0.0,
            CLOSED + 0.02 * open,
            false,
        )
    } else {
        let down = smoothstep(3.3, 3.6, u) - smoothstep(3.75, 4.0, u);
        let close = smoothstep(3.6, 3.75, u);
        (
            above_tray - vec3(0.0, 0.11 * down, 0.0),
            0.0,
            CLOSED + 0.02 * (1.0 - close),
            close > 0.5,
        )
    };
    Grip {
        chip,
        turn,
        gap,
        held,
    }
}

/// The joints of an arm whose fingers hold a chip at `chip` (in the frame of its base) with
/// the tool down. The fingers close along the X axis of the world turned by `turn`, and the
/// eye looks from the side of -Z of the world.
fn holding(chip: Vec3, turn: f32, hint: &Joints) -> Joints {
    let fingers = Quat::from_rotation_y(turn) * Vec3::NEG_Z;
    arm::solve(
        chip + vec3(0.0, HOLD - arm::GRIP, 0.0),
        arm::tool_down(fingers),
        hint,
    )
}

/// Blends two poses joint by joint, each joint the short way around.
fn blend(a: &Joints, b: &Joints, k: f32) -> Joints {
    Joints {
        q: std::array::from_fn(|i| crowd::turn(a.q[i], b.q[i], k)),
    }
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
        scene.environment = Environment::map(assets.factory_sky.clone(), SKY);
        scene.reflector = Some(Reflector::floor());
        let stars = scene.add(crate::stars::stars().hidden());

        // The engineer on the catwalk: dark cloth with a thin cold rim, a silhouette against
        // the windows. His right hand rests on his old laptop on the rail.
        let rim = Rgb::hex(0x9fb4d8) * 0.18;
        let cloth = |color: u32, roughness: f32| {
            Material::standard(color).roughness(roughness).rim(rim, 4.0)
        };
        let style = FigureStyle {
            skin: cloth(0x1a1411, 0.6),
            jacket: cloth(0x121418, 0.85),
            trousers: cloth(0x0e1013, 0.9),
            boots: cloth(0x121010, 0.6),
            hair: cloth(0x0a0909, 0.75),
        };
        let engineer = Figure::build(&mut scene, None, &style);
        let laptop = scene.add(Node::group());
        scene.add_to(
            laptop,
            Node::mesh(
                Geometry::merge([
                    Geometry::rounded_box(0.33, 0.011, 0.23, 0.004, 2).translate(0.0, 0.0055, 0.0),
                    Geometry::rounded_box(0.33, 0.007, 0.23, 0.004, 2).translate(0.0, 0.0145, 0.0),
                ]),
                Material::metal(0x6a6d72, 0.45),
            ),
        );
        // a sticker on the lid
        scene.add_to(
            laptop,
            Node::mesh(
                Geometry::rounded_box(0.05, 0.001, 0.05, 0.004, 1).translate(0.06, 0.0185, -0.04),
                Material::standard(0x2c3440).roughness(0.7),
            ),
        );

        // The jig of the watcher: a stand with the circuit board, and the tray of chips
        // beside it. The inspection lamp lights the board in shot 3.
        let jig = watcher.base.transform_point3(JIG);
        let stand = Material::metal(0x3a3e45, 0.5);
        scene.add(
            Node::mesh(Geometry::cuboid(0.42, JIG.y - 0.05, 0.36), stand.clone()).at(
                jig.x + 0.02,
                (JIG.y - 0.05) * 0.5,
                jig.z + 0.02,
            ),
        );
        board::build(&mut scene, jig, font);
        let tray = watcher.base.transform_point3(TRAY);
        scene.add(
            Node::mesh(Geometry::cuboid(0.3, TRAY.y - 0.01, 0.24), stand).at(
                tray.x,
                (TRAY.y - 0.01) * 0.5,
                tray.z,
            ),
        );
        scene.add(
            Node::mesh(
                Geometry::rounded_box(0.28, 0.012, 0.2, 0.004, 1),
                Material::standard(0x101215).roughness(0.6),
            )
            .at(tray.x, TRAY.y - 0.006, tray.z),
        );
        let chips = Geometry::merge((0..12).map(|i| {
            Geometry::cuboid(0.036, 0.003, 0.036).translate(
                -0.09 + (i % 4) as f32 * 0.06,
                0.0015,
                -0.05 + (i / 4) as f32 * 0.05,
            )
        }));
        scene.add(
            Node::mesh(chips, Material::standard(0x1c1d20).roughness(0.4))
                .at(tray.x, TRAY.y, tray.z),
        );
        let chip = scene.add(crate::city::chip());
        // The sheet lies over the pads (40 µm) and under the pins (300 µm).
        let veil = scene.add(
            Node::mesh(
                Geometry::floor(0.5, 0.5),
                Material::unlit(0x010203).transparent(),
            )
            .at(jig.x, JIG.y + 0.0001, jig.z)
            .shadows(false, false)
            .hidden(),
        );
        // 44 pins on each side, 36 m apart and 99 m long out of a chip of 1620 m in the
        // city, at 1 / 45000.
        let s = crate::city::CHIP_SCALE;
        let leads = Geometry::merge((0..44).flat_map(|k| {
            let z = (k as f32 - 21.5) * 36.0 * s;
            [-1.0f32, 1.0].map(|side| {
                Geometry::cuboid(99.0 * s, 0.0002, 8.0 * 1.8 * s).translate(
                    side * (810.0 + 49.5) * s,
                    CHIP_TOP - 0.0001,
                    z,
                )
            })
        }));
        let seat = watcher.base.transform_point3(seated());
        let leads = scene.add(
            Node::mesh(leads, Material::metal(0xd9dde3, 0.24).transparent())
                .at(seat.x, seat.y, seat.z)
                .shadows(false, false)
                .hidden(),
        );
        let lamp = scene.add(Node::light(Light::spot(0xe8f2ff, 0.0, 0.5, 0.7)));
        Self {
            scene,
            hall,
            crowd,
            stations,
            welder,
            watcher,
            sun,
            engineer,
            laptop,
            chip,
            lamp,
            stars,
            veil,
            leads,
        }
    }

    /// The renderer of the factory shots.
    pub fn renderer(width: u32, height: u32) -> Renderer {
        Renderer::new(width, height).exposure(1.05).bloom(Bloom {
            strength: 0.55,
            radius: 0.55,
            threshold: 1.2,
        })
    }

    /// The hall by day: cold light falls through the roof glazing. `shadow_bounds` is the
    /// part of the hall with sharp shadows and light shafts. `daylight` (`0..1`) dims the
    /// sun, the light of the hemisphere and the haze.
    fn day(&self, t: f32, shadow_bounds: Aabb, daylight: f32) -> Scene {
        let mut scene = self.scene.clone();
        scene[self.sun].object = Object::Light(
            Light::directional(0xdfe9ff, 7.0 * daylight, vec3(0.35, 0.75, -1.0)).with_shadow(
                Shadow {
                    map_size: 4096,
                    softness: 1.5,
                    bounds: Some(shadow_bounds),
                    ..Default::default()
                },
            ),
        );
        scene.atmosphere = Some(Atmosphere {
            density: 0.022 * daylight,
            color: Rgb::hex(0xe4ecff),
            base: 2.0,
            falloff: 0.03,
            anisotropy: 0.7,
            ambient: Rgb::hex(0x060a10),
            noise: 0.6,
            noise_size: 7.0,
            offset: vec3(0.4, 0.05, 0.25) * t,
            distance: 220.0,
            // The shafts of the roof glazing repeat every 12 m along the hall. With fewer
            // steps the far steps are longer than half of this period, and the shafts
            // make a moire that crawls when the camera moves.
            steps: 64,
        });
        scene.background = Background::Color(Rgb::hex(0xc8d6ea));
        scene.add(Node::light(Light::hemisphere(
            0x5d6a80,
            0x16181c,
            0.5 * daylight,
        )));
        self.hall.light_lamps(&mut scene, Rgb::hex(0xdcecff) * 6.0);
        scene
    }

    /// Poses the crowd at time `t`, moves the machines and adds the sparks and the arc lights
    /// of the arms that `view` sees.
    fn work(&self, scene: &mut Scene, t: f32, view: &View) {
        let hint = crowd::rest();
        let eye = Rgb::hex(COLD) * 4.0;
        let posed: Vec<Posed> = self
            .stations
            .iter()
            .enumerate()
            .map(|(i, s)| Posed {
                base: s.base,
                joints: crowd::joints(&crowd::work(t + s.phase), &hint),
                eye,
                tint: crowd::tint(i),
            })
            .collect();
        self.crowd
            .pose(scene, &posed, |a| view.lod(a.base.w_axis.truncate()));
        let camera = view.position;
        self.hall.conveyors(
            scene,
            t,
            camera.z + 20.0,
            camera.z - 180.0,
            Rgb::hex(0x8fd0ff) * 5.0,
        );
        self.hall.machines(
            scene,
            &|_| t,
            |time, phase| crowd::work(time + phase).welding,
            |p, r| view.sees(p, r),
        );

        let mut sparks = Vec::new();
        let mut arcs = Vec::new();
        for (index, (station, arm)) in self.stations.iter().zip(&posed).enumerate() {
            let base = station.base.w_axis.truncate();
            let distance = base.distance(camera);
            if distance > 60.0 || !view.sees(base + vec3(0.0, 1.2, 0.0), 2.5) {
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

    /// Hides a hero arm that is out of the view.
    fn show(&self, scene: &mut Scene, hero: &Hero, view: &View) {
        let base = hero.base.w_axis.truncate();
        scene[hero.arm.base].visible = view.sees(base + vec3(0.0, 1.2, 0.0), 3.0);
    }

    /// Shot 2: the factory of answers. The welding arc of the first hero arm in close-up. The
    /// camera rises over the conveyor and flies down the line between the arms. Then it
    /// cranes up over the endless grid of arms and settles on the watcher, which stops and
    /// turns its eye up to the engineer on the catwalk.
    pub fn moment_factory(&self, t: f32) -> (Scene, Camera) {
        let welder = self.welder_joints(t);
        let arc = self.tip(&self.welder, welder);
        let camera = self.factory_camera(t, arc);
        let bounds = Aabb {
            min: vec3(-hall::HALF_WIDTH, 0.0, camera.position.z - 130.0),
            max: vec3(hall::HALF_WIDTH, hall::WALL + 4.0, camera.position.z + 25.0),
        };
        let mut scene = self.day(t, bounds, 1.0);
        let view = View::new(&camera);
        self.work(&mut scene, t, &view);
        self.pose_hero(&mut scene, &self.welder, welder, t, 0.0);
        // The arc flares at the cut: the white of shot 1 is its light, which fills the lens
        // and shrinks to the arc. The glow stands just in front of the lens, so nothing
        // hides it.
        let flare = 1.0 - smoothstep(0.0, 0.45, t);
        if flare > 0.0 {
            let lens_glow = camera.position + (arc - camera.position).normalize() * 0.25;
            scene.add(Node::particles(
                vec![
                    Particle::new(
                        lens_glow,
                        0.02 + 0.7 * flare,
                        Rgb::hex(0xeef4ff) * (12.0 * flare * flare * flare),
                    ),
                    Particle::new(arc, 0.05 + 0.3 * flare, Rgb::hex(0xeef4ff) * (40.0 * flare)),
                ],
                Blending::Additive,
            ));
        }
        let awake = smoothstep(10.4, 11.2, t);
        let (watcher, grip) = self.watcher_at_work(t);
        self.pose_watcher(&mut scene, &watcher, &grip, awake);
        // The eye of the watcher scans the engineer: a faint beam in the haze, from the eye
        // to his chest. It grows from the eye.
        let scan = smoothstep(10.8, 11.6, t);
        if scan > 0.0 {
            let flange = self.watcher.base * arm::flange(&watcher);
            let eye = flange.transform_point3(arm::EYE + Vec3::Z * 0.04);
            let chest = ENGINEER + vec3(0.0, 1.3, 0.0);
            let length = eye.distance(chest) * scan;
            let dir = (chest - eye).normalize();
            let beam = (0..300)
                .map(|i| {
                    let s = i as f32 / 299.0;
                    let fade = (1.0 - s).powf(0.7) * scan;
                    Particle::new(
                        eye + dir * (length * s),
                        0.05 + 0.25 * s,
                        Rgb::hex(0x9fd4ff) * (0.14 * fade),
                    )
                })
                .collect();
            scene.add(Node::particles(beam, Blending::Additive));
        }
        self.pose_engineer(&mut scene, 12.0 + t, 0.0);
        self.show(&mut scene, &self.welder, &view);
        self.show(&mut scene, &self.watcher, &view);
        (scene, camera)
    }

    pub fn shutter_factory(&self, t: f32) -> usize {
        if (4.2..8.6).contains(&t) { 3 } else { 2 }
    }

    /// The camera of shot 2, 24 mm. `arc` is the welding arc of the welder.
    fn factory_camera(&self, t: f32, arc: Vec3) -> Camera {
        // The flight: the speed rises to 6 m/s, holds, and falls to 0 as the crane rises.
        let flown = 6.1 * (glide(2.6, 4.8, t) - glide(8.0, 11.4, t));
        let up = smoothstep(2.6, 4.4, t);
        let crane = smoothstep(8.2, 11.6, t);
        // In the flight the camera sways a little from side to side and banks into the
        // sway, as a camera on a cable rig does.
        let flight = up * (1.0 - crane);
        let sway = 0.25 * (t * 0.9).sin() * flight;
        let bob = 0.06 * (t * 1.3).sin() * flight;
        // the crane rises a little too far and settles back, as a heavy arm does
        let settle = 0.12 * (std::f32::consts::PI * smoothstep(11.0, 12.0, t)).sin();
        let position = vec3(
            6.1 + 0.9 * up + 2.5 * crane + sway,
            1.12 + 1.23 * up + 3.65 * crane + bob + settle,
            WELDER_Z + 1.25 - flown,
        );
        let ahead = vec3(7.0, 1.4, position.z - 25.0);
        let watcher = self.watcher.base.w_axis.truncate();
        let end = vec3(1.5, 2.9, hall::CATWALK_Z + 2.0);
        let target = arc
            .lerp(ahead, smoothstep(2.6, 4.2, t))
            .lerp(end, smoothstep(8.4, 11.8, t));
        let mut camera = lens(24.0)
            .position(position)
            .look_at(target)
            .roll(0.03 * (t * 0.9).sin() * flight)
            .shake(t * 0.4, 0.003, 7);
        camera.far = 600.0;
        // the focus moves from the arc down the line, then to the watcher
        let focus = arc
            .lerp(ahead, smoothstep(2.6, 4.2, t))
            .lerp(watcher + vec3(0.0, 1.8, 0.0), smoothstep(8.6, 11.0, t));
        camera.focus_on(focus, 12.0 - 9.0 * smoothstep(1.8, 4.0, t))
    }

    /// The joints of the welder: it works with its neighbors.
    fn welder_joints(&self, t: f32) -> Joints {
        crowd::joints(&crowd::work(t + self.welder.phase), &crowd::rest())
    }

    /// The joints of the watcher at shot 2 time `t`, and what its gripper does. It places
    /// chips until it stops with a chip over the board, then turns its eye up to the
    /// engineer.
    fn watcher_at_work(&self, t: f32) -> (Joints, Grip) {
        let grip = place_cycle(t.min(WATCHER_STOPS) + self.watcher.phase);
        let working = holding(grip.chip, grip.turn, &crowd::rest());
        let q = blend(&working, &self.watcher_gaze(), smoothstep(9.9, 11.6, t));
        (q, grip)
    }

    /// The pose in which the watcher looks at the head of the engineer.
    fn watcher_gaze(&self) -> Joints {
        let grip = place_cycle(WATCHER_STOPS + self.watcher.phase);
        let working = holding(grip.chip, grip.turn, &crowd::rest());
        let head = ENGINEER + vec3(0.0, 1.62, 0.0);
        let look = self.watcher.base.inverse().transform_point3(head) - vec3(0.0, 2.05, 0.0);
        crowd::looking(look, 2.05, &working)
    }

    /// Poses the watcher with joints `q`. The chip is in its fingers when `grip` holds it,
    /// else on the board. `awake` brightens its eye.
    fn pose_watcher(&self, scene: &mut Scene, q: &Joints, grip: &Grip, awake: f32) {
        self.watcher.arm.apply(
            scene,
            q,
            grip.gap,
            &Expression {
                ring: 2.0,
                status: 3.0 * (1.0 - awake),
                eye: 4.0 + 10.0 * awake,
                lid: 1.0,
            },
        );
        let flange = self.watcher.base * arm::flange(q);
        let (position, rotation) = if grip.held {
            // The chip turns with the fingers. With the tool down, its fingers along -X and
            // its eye to -Z, the chip lies flat and square to the world.
            let level = Quat::from_mat3(&Mat3::from_cols(Vec3::NEG_X, Vec3::NEG_Z, Vec3::NEG_Y));
            let (_, rotation, _) = flange.to_scale_rotation_translation();
            (
                flange.transform_point3(vec3(0.0, 0.0, HOLD)),
                rotation * level.inverse(),
            )
        } else {
            (self.watcher.base.transform_point3(seated()), Quat::IDENTITY)
        };
        scene[self.chip].position = position;
        scene[self.chip].rotation = rotation;
    }

    /// Poses the engineer at `time` seconds: he leans on the rail with his right hand on the
    /// laptop. `down` lowers his head from a look over the hall (0) to a look at the watcher
    /// (1).
    fn pose_engineer(&self, scene: &mut Scene, time: f32, down: f32) {
        let rail = vec3(0.0, 1.10, 0.43);
        let mut pose = Pose::at_rail(time, rail);
        // The right hand lies on the laptop, 18 mm over the rail.
        let on_laptop = Pose::at_rail(time, rail + vec3(0.0, 0.018, 0.0));
        for joint in [
            Joint::ClavicleR,
            Joint::ShoulderR,
            Joint::ElbowR,
            Joint::WristR,
        ] {
            pose[joint] = on_laptop[joint];
        }
        pose.fingers[RIGHT] = [0.25, 0.15, 0.2, 0.25, 0.3];
        // At the rail he looks 20 degrees down. First he looks out over the hall, then down
        // at the watcher.
        let pitch = deg(-20.0) + deg(22.0) * down;
        pose[Joint::Neck] *= Quat::from_rotation_x(0.4 * pitch);
        pose[Joint::Head] *= Quat::from_rotation_x(0.6 * pitch);
        let pose = pose.placed(ENGINEER, 0.0);
        self.engineer.apply(scene, &pose);
        let palm = self.engineer.palms(scene)[RIGHT];
        let laptop = &mut scene[self.laptop];
        laptop.position = vec3(palm.x - 0.03, ENGINEER.y + 1.10, ENGINEER.z + 0.43);
        laptop.rotation = Quat::from_rotation_y(0.08);
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
