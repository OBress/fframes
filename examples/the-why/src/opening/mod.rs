//! Shot 1, "The cursor" (0.0 to 7.0 s). A macro shot of one screen in a dark room. A cursor
//! blinks, a person types a question, and the machine answers with code, faster and faster.
//! The camera pulls back: the screen is one of thousands in an endless dark field, and all
//! of them write code. Then the screens go dark, one cursor remains, and the camera rushes
//! into it until its light fills the frame: the welding arc that opens shot 2.

mod field;
mod terminal;

use std::sync::Arc;

use fframes::Svgr;
use fframes_3d::math::{hash, noise1, smoothstep};
use fframes_3d::*;

use crate::assets::Assets;
use crate::look;
use field::{Field, MainScreen, SCREEN_Y};
use terminal::{ADVANCE, EM, LINE, QUESTION, ROWS, SCREEN, Terminal};

/// The time when the person starts to type, in seconds.
const TYPE_START: f32 = 1.4;
/// The characters that the person types each second.
const TYPE_RATE: f32 = 13.0;
/// The time when the machine starts to answer.
const ANSWER_START: f32 = 3.4;
/// The time when the machine stops to write.
const ANSWER_END: f32 = 5.9;
/// The time when the far screens start to go dark.
const DARK_START: f32 = 5.72;
/// The cursor grows into light from this time.
const GROW_START: f32 = 6.3;
/// The light of the cursor fills the frame from this time.
const FLASH_START: f32 = 6.62;
/// The frame is white from this time.
const FLASH_END: f32 = 6.93;
/// The cursor burns like a welding arc from this time: it flickers and throws sparks.
const ARC_START: f32 = 6.45;
/// The length of the shot.
const END: f32 = 7.0;
/// The floor reflects the scene from this time.
const REFLECT_START: f32 = 5.95;

/// The center of the main screen in the world. The other monitors stand around it.
const MAIN: Vec3 = vec3(0.0, SCREEN_Y, 0.0);

/// The question (sRGB) and its light. The person writes it, so it is a soft white.
const QUESTION_COLOR: u32 = 0xe8f0fb;
const QUESTION_LIGHT: f32 = 2.6;
/// The comments of the machine and their light.
const COMMENT_COLOR: u32 = 0x5d7390;
const COMMENT_LIGHT: f32 = 1.6;
/// The color of the cursor and of its light.
const CURSOR_COLOR: u32 = 0xdcebff;

/// The motes of dust in front of the main screen.
const DUST: u32 = 220;
/// The specks of dust on the glass of the main screen.
const SPECKS: u32 = 140;
/// The large motes of dust between the lens and the screen. They are far out of focus, so
/// the lens draws them as soft discs.
const BOKEH: u32 = 60;
/// The camera is too far from the dust after this time.
const DUST_END: f32 = 4.6;

/// The number of answer lines written by time `t`: the speed doubles every 0.45 s.
fn written(t: f32) -> f32 {
    4.0 * (((t - ANSWER_START).max(0.0) / 0.45).exp2() - 1.0)
}

/// The characters of the question typed by time `t`.
fn typed(t: f32) -> usize {
    (((t - TYPE_START) * TYPE_RATE).max(0.0) as usize).min(QUESTION.len())
}

/// The light of the cursor at `t`, `0..1`. It comes on, blinks once while it waits for the
/// person, and then stays on. In the blink it keeps a little light, so the macro view of
/// its pixels does not go black.
fn blink(t: f32) -> f32 {
    let wake = smoothstep(0.0, 0.3, t);
    let off = smoothstep(0.48, 0.52, t) * (1.0 - smoothstep(0.98, 1.02, t));
    wake * (1.0 - 0.88 * off)
}

/// How much the cursor has grown into light at `t`, `0..1`.
fn grow(t: f32) -> f32 {
    smoothstep(GROW_START, END - 0.05, t).powi(2)
}

/// The flash at the end of the shot, `0..1`: the light of the cursor fills the air and the
/// frame turns white.
fn flash(t: f32) -> f32 {
    smoothstep(FLASH_START, FLASH_END, t)
}

/// The light of the air in the flash. It grows by the same factor in each frame, from 0.02
/// to 4, so the frame brightens evenly after the tone mapping.
fn air_glow(t: f32) -> f32 {
    let progress = ((t - FLASH_START) / (FLASH_END - FLASH_START)).clamp(0.0, 1.0);
    0.02 * 200f32.powf(progress) * smoothstep(FLASH_START - 0.02, FLASH_START + 0.04, t)
}

/// The text of the main terminal at `t`: the typed characters, the written answer lines,
/// and the scroll in lines.
struct Text {
    typed: usize,
    count: f32,
    scroll: f32,
}

impl Text {
    fn at(t: f32) -> Self {
        let count = written(t.min(ANSWER_END));
        Self {
            typed: typed(t),
            count,
            scroll: (1.0 + count - ROWS as f32).max(0.0),
        }
    }

    /// The center of the cursor on the screen: after the question, then after the newest
    /// answer.
    fn cursor(&self) -> Vec2 {
        let (row, column) = if self.count <= 0.0 {
            (0.0, self.typed)
        } else {
            let last = self.count.ceil() as usize - 1;
            let line = terminal::answer(terminal::page_line(last));
            (1.0 + last as f32 - self.scroll, line.len())
        };
        terminal::baseline(row) + vec2((column as f32 + 0.5) * ADVANCE, 0.33 * EM)
    }
}

/// A pose of the camera.
#[derive(Debug, Clone, Copy)]
struct Pose {
    /// The point that the camera looks at and focuses on, relative to the main screen.
    target: Vec3,
    /// The distance from the target in meters.
    distance: f32,
    /// The direction from the target to the camera in degrees: around Y from the normal of
    /// the screen, and above the horizon.
    azimuth: f32,
    elevation: f32,
    /// The focal length in mm.
    lens: f32,
    /// The blur of the depth of field in pixels at 1080 lines.
    blur: f32,
}

impl Pose {
    /// The pose as values that interpolate well: the distance is a logarithm, so the camera
    /// moves at the same speed relative to its distance.
    fn values(&self) -> [f32; 8] {
        [
            self.target.x,
            self.target.y,
            self.target.z,
            self.distance.ln(),
            self.azimuth,
            self.elevation,
            self.lens,
            self.blur,
        ]
    }

    fn from_values(v: [f32; 8]) -> Self {
        Self {
            target: vec3(v[0], v[1], v[2]),
            distance: v[3].exp(),
            azimuth: v[4],
            elevation: v[5],
            lens: v[6],
            blur: v[7],
        }
    }
}

/// The value at `t` of a smooth curve through `keys` of time and value. Between two keys the
/// curve stays between their values (monotone cubic interpolation, Fritsch and Carlson), so
/// a move does not overshoot. The curve stops at the first and the last key.
fn curve(keys: &[(f32, f32)], t: f32) -> f32 {
    let last = keys.len() - 1;
    if t <= keys[0].0 {
        return keys[0].1;
    }
    if t >= keys[last].0 {
        return keys[last].1;
    }
    let slope = |i: usize| {
        if i == 0 || i == last {
            return 0.0;
        }
        let (h0, h1) = (keys[i].0 - keys[i - 1].0, keys[i + 1].0 - keys[i].0);
        let d0 = (keys[i].1 - keys[i - 1].1) / h0;
        let d1 = (keys[i + 1].1 - keys[i].1) / h1;
        if d0 * d1 <= 0.0 {
            return 0.0;
        }
        let mean = (d0 * h1 + d1 * h0) / (h0 + h1);
        mean.signum() * mean.abs().min(3.0 * d0.abs().min(d1.abs()))
    };
    let i = keys
        .iter()
        .rposition(|k| k.0 <= t)
        .unwrap_or(0)
        .min(last - 1);
    let ((t0, p0), (t1, p1)) = (keys[i], keys[i + 1]);
    let h = t1 - t0;
    let s = (t - t0) / h;
    let (s2, s3) = (s * s, s * s * s);
    (2.0 * s3 - 3.0 * s2 + 1.0) * p0
        + (s3 - 2.0 * s2 + s) * h * slope(i)
        + (-2.0 * s3 + 3.0 * s2) * p1
        + (s3 - s2) * h * slope(i + 1)
}

/// Shot 1: the cursor and the field of screens.
pub struct OpeningSet {
    /// The template: the floor, the monitors and the lights.
    scene: Scene,
    /// The group of the main screen. Its children are in screen space.
    main: NodeId,
    cursor_light: NodeId,
    screen_light: NodeId,
    hall_light: NodeId,
    terminal: Terminal,
    field: Field,
    /// The cursor block, with UVs for the pixel grid.
    cursor: Arc<Geometry>,
    /// The keys of the camera move: the time and the values of the pose.
    keys: Vec<(f32, [f32; 8])>,
    /// The dust in the air in front of the main screen, in screen space.
    dust: Vec<Vec3>,
    /// The dust on the glass of the main screen, in screen space.
    specks: Vec<Vec3>,
    /// The large motes near the lens, in screen space.
    bokeh: Vec<Vec3>,
}

impl OpeningSet {
    /// Builds the terminal, the field of monitors and the camera move.
    pub fn new(assets: &Assets) -> Self {
        let terminal = Terminal::new();
        let field = Field::new(terminal.page.clone());
        let mut scene = Scene::new()
            .background(Background::Color(Rgb::BLACK))
            .environment(Environment::Studio(Studio {
                sky: Rgb::linear(0.002, 0.003, 0.005),
                horizon: Rgb::linear(0.003, 0.005, 0.009),
                ground: Rgb::BLACK,
                // a far light of the room, which the glass of the screens reflects
                softboxes: vec![SoftBox::new(
                    vec3(-0.6, 0.05, 0.8),
                    60.0,
                    7.0,
                    Rgb::linear(0.7, 0.85, 1.1),
                )],
                intensity: 1.0,
                rotation: 0.0,
            }))
            .reflector(Reflector::floor());
        scene.add(
            Node::mesh(
                Geometry::floor(900.0, 900.0),
                assets
                    .concrete
                    .material(0x24272c, 0.35, 0.0)
                    .reflection(0.7),
            )
            .at(0.0, 0.0, -300.0)
            .shadows(false, false),
        );
        // the faint light of the hall on the bodies of the monitors, for the wide views
        let hall_light = scene.add(Node::light(Light::hemisphere(0x24385a, 0x000000, 0.0)));
        field.add_monitors(&mut scene);

        let main = scene.add(Node::group().with_position(MAIN));
        scene.add_to(
            main,
            Node::mesh(
                terminal::pixel_plane(SCREEN),
                Material::unlit(
                    Rgb::hex(field::BLEED) * (field::BLEED_LIGHT / terminal::GRID_MEAN),
                )
                .map(terminal.grid.clone()),
            )
            .shadows(false, false),
        );
        let cursor_light = scene.add_to(main, Node::light(Light::point(CURSOR_COLOR, 0.0, 0.0)));
        scene.add_to(
            main,
            Node::mesh(
                Geometry::plane(SCREEN.x, SCREEN.y).translate(0.0, 0.0, 0.004),
                field::glass(),
            )
            .shadows(false, false),
        );
        let screen_light = scene.add_to(
            main,
            Node::light(Light::spot(field::CODE, 0.0, 70f32.to_radians(), 0.9).pointing(Vec3::Z))
                .at(0.0, 0.0, 0.05),
        );

        let start = Text::at(0.0).cursor().extend(0.0);
        let dust = (0..DUST)
            .map(|i| {
                let r = |k: u32| hash(i * 5 + k);
                start
                    + vec3(
                        -0.25 + 0.6 * r(0),
                        -0.16 + 0.3 * r(1),
                        0.06 + 0.4 * r(2).powf(1.5),
                    )
            })
            .collect();
        let specks = (0..SPECKS)
            .map(|i| {
                let r = |k: u32| hash(i * 3 + 90_001 + k);
                let (angle, radius) = (r(0) * std::f32::consts::TAU, 0.1 * r(1).sqrt());
                start + vec3(angle.cos() * radius, angle.sin() * radius * 0.7, 0.0042)
            })
            .collect();

        let end = Text::at(ANSWER_END).cursor().extend(0.0);
        let question = terminal::baseline(0.0).extend(0.0)
            + vec3(QUESTION.len() as f32 * 0.5 * ADVANCE, 0.33 * EM, 0.0);
        let pose = |target: Vec3, distance, azimuth, elevation, lens, blur| Pose {
            target,
            distance,
            azimuth,
            elevation,
            lens,
            blur,
        };
        // The distances of the keys follow two eased curves in the logarithm of the
        // distance: one pull back from the pixels of the cursor to the whole field, and a
        // rush back in that speeds up until the end.
        let keys: Vec<(f32, [f32; 8])> = vec![
            (
                0.0,
                pose(
                    start + vec3(0.1 * EM, 0.1 * EM, 0.0),
                    0.1,
                    32.0,
                    6.0,
                    65.0,
                    22.0,
                ),
            ),
            (
                1.4,
                pose(
                    start + vec3(0.7 * EM, 0.2 * EM, 0.0),
                    0.2,
                    28.0,
                    6.0,
                    65.0,
                    18.0,
                ),
            ),
            (
                2.4,
                pose(
                    terminal::baseline(0.0).extend(0.0) + vec3(9.0 * ADVANCE, 0.33 * EM, 0.0),
                    0.4,
                    22.0,
                    5.0,
                    60.0,
                    10.0,
                ),
            ),
            (
                3.4,
                pose(
                    question - vec3(0.0, 1.5 * LINE, 0.0),
                    0.85,
                    14.0,
                    4.0,
                    55.0,
                    7.0,
                ),
            ),
            (
                4.3,
                pose(
                    terminal::baseline(5.0).extend(0.0) + vec3(18.0 * EM, 0.0, 0.0),
                    2.3,
                    8.0,
                    // high enough that the pull back passes over the monitor in front
                    9.0,
                    50.0,
                    8.0,
                ),
            ),
            // the main screen stays the subject while the field opens around it
            (
                5.1,
                pose(vec3(0.0, -0.15, -2.0), 18.6, 11.0, 10.0, 38.0, 7.0),
            ),
            (
                5.7,
                pose(vec3(0.0, -0.6, -16.0), 50.0, 14.0, 14.0, 32.0, 5.0),
            ),
            (
                5.85,
                pose(vec3(0.3, -0.6, -16.0), 49.0, 14.3, 14.2, 32.0, 5.0),
            ),
            // The rush passes through the gap between the monitor in front of the main
            // monitor and its left neighbor, 3.2 m in front of the cursor.
            (6.2, pose(end, 27.7, 3.0, 20.0, 34.0, 3.0)),
            (6.6, pose(end, 2.34, -9.0, 9.0, 40.0, 3.0)),
            (6.85, pose(end, 0.157, -2.0, 3.0, 45.0, 2.5)),
            (6.95, pose(end, 0.041, -0.5, 1.0, 45.0, 2.0)),
            (END, pose(end, 0.02, 0.0, 0.5, 45.0, 2.0)),
        ]
        .into_iter()
        .map(|(t, pose)| (t, pose.values()))
        .collect();

        let mut set = Self {
            scene,
            main,
            cursor_light,
            screen_light,
            hall_light,
            terminal,
            field,
            cursor: Arc::new(terminal::pixel_plane(vec2(ADVANCE, 1.15 * EM))),
            keys,
            dust,
            specks,
            bokeh: Vec::new(),
        };
        set.bokeh = set.bokeh_motes();
        set
    }

    /// The renderer of the shot for pictures of `width` x `height` pixels.
    pub fn renderer(width: u32, height: u32) -> Renderer {
        Renderer::new(width, height).bloom(Bloom {
            strength: 0.7,
            radius: 0.65,
            threshold: 1.0,
        })
    }

    /// The pose of the camera at `t`.
    fn pose(&self, t: f32) -> Pose {
        Pose::from_values(std::array::from_fn(|c| {
            let keys: Vec<(f32, f32)> = self.keys.iter().map(|(time, v)| (*time, v[c])).collect();
            curve(&keys, t)
        }))
    }

    /// The large motes near the lens. Each one is where the camera sees it at a moment of
    /// the pull back: between 10 and 35 percent of the way to what the camera looks at, at a
    /// place in the frame. The motes stay in place, so the pull back carries them through
    /// the frame.
    fn bokeh_motes(&self) -> Vec<Vec3> {
        (0..BOKEH)
            .map(|i| {
                let r = |k: u32| hash(i * 5 + 70_001 + k);
                let (pose, position) = self.place(DUST_END * 0.75 * r(0));
                let camera = look::lens(pose.lens)
                    .position(position)
                    .look_at(MAIN + pose.target);
                let depth = pose.distance * (0.1 + 0.25 * r(1));
                let half = (camera.fov.to_radians() * 0.5).tan() * depth;
                let offset = camera.right() * ((r(2) * 2.0 - 1.0) * half * look::ASPECT)
                    + camera.up_vector() * ((r(3) * 2.0 - 1.0) * half);
                position + camera.forward() * depth + offset - MAIN
            })
            .collect()
    }

    /// The pose of the camera at `t` and its position in the world.
    fn place(&self, t: f32) -> (Pose, Vec3) {
        let pose = self.pose(t);
        let (azimuth, elevation) = (pose.azimuth.to_radians(), pose.elevation.to_radians());
        let direction = vec3(
            azimuth.sin() * elevation.cos(),
            elevation.sin(),
            azimuth.cos() * elevation.cos(),
        );
        (pose, MAIN + pose.target + direction * pose.distance)
    }

    /// The scene and the camera at `t` seconds into the shot.
    pub fn moment(&self, t: f32) -> (Scene, Camera) {
        let (pose, position) = self.place(t);
        let target = MAIN + pose.target;
        // In the wide views the camera looks past the main screen, and the focus stays on
        // the screen.
        let focus = MAIN + pose.target * vec3(1.0, 1.0, 0.0);
        let camera = look::lens(pose.lens)
            .position(position)
            .look_at(target)
            .clip((pose.distance * 0.05).min(0.5), 1200.0)
            .shake(t, 0.0012, 11)
            .focus_on(focus, pose.blur);

        let mut scene = self.scene.clone();
        let text = Text::at(t);
        // The main screen shows its text as meshes near the camera and as the page far away,
        // where the meshes are smaller than the pixels.
        let width = camera.pixel_size(MAIN, SCREEN.x, 1.0) / look::ASPECT;
        // The eye adapts to the dark: far away, where the code is small, it looks brighter.
        let far = smoothstep(0.5f32.ln(), 20f32.ln(), pose.distance.ln());
        let adapt = 1.0 + 0.8 * far;
        let near = smoothstep(0.18, 0.35, width);
        let dark = 1.0 - smoothstep(DARK_START + 0.3, DARK_START + 0.4, t);
        self.add_text(&mut scene, &text, near * dark * adapt);
        let glow = smoothstep(ANSWER_START, ANSWER_START + 0.8, t) * dark;
        let main = MainScreen {
            lines: text.count - ROWS as f32,
            code: (1.0 - near) * dark,
            pool: glow,
            hero: smoothstep(4.7, 5.2, t),
        };
        self.field.add_screens(&mut scene, t, &main, adapt);

        // the cursor and its light
        let cursor = text.cursor();
        let arc = smoothstep(ARC_START, ARC_START + 0.15, t);
        // the flicker calms down before the flash, so the light grows without dips
        let calm = 1.0 - smoothstep(FLASH_START - 0.06, FLASH_START + 0.02, t);
        let flicker = 1.0 + arc * calm * 0.35 * noise1(23, t * 40.0);
        let light = blink(t) * (1.0 + 4.0 * grow(t) + 300.0 * flash(t).powi(2)) * flicker;
        if light > 0.0 {
            // The pixels of the cursor melt into one light in the flash, and the light grows.
            let flash = flash(t);
            let melt = smoothstep(FLASH_START - 0.05, FLASH_START + 0.15, t);
            let size = vec3(1.0 + 10.0 * flash.powi(2), 1.0 + 5.0 * flash.powi(2), 1.0);
            scene.add_to(
                self.main,
                Node::mesh(
                    self.cursor.clone(),
                    Material::unlit(
                        Rgb::hex(CURSOR_COLOR) * (1.6 * light * (1.0 - melt) / terminal::GRID_MEAN),
                    )
                    .map(self.terminal.grid.clone()),
                )
                .at(cursor.x, cursor.y, 0.0007)
                .scaled3(size.x, size.y, size.z)
                .shadows(false, false),
            );
            if melt > 0.0 {
                scene.add_to(
                    self.main,
                    Node::mesh(
                        self.cursor.clone(),
                        Material::glow(CURSOR_COLOR, 1.6 * light * melt),
                    )
                    .at(cursor.x, cursor.y, 0.0009)
                    .scaled3(size.x, size.y, size.z)
                    .shadows(false, false),
                );
            }
        }
        // The light is just behind the glass, so the glass does not reflect it.
        let cursor = cursor.extend(0.003);
        scene[self.cursor_light].position = cursor;
        // When the screens go dark, the cursor is the only light of the field. Closer than
        // 3 m the light dims with the square of the distance of the camera, so its halo in
        // the air keeps its brightness and the frame of the monitor does not burn white.
        // The flash then comes from the air, see `atmosphere`.
        let alone = smoothstep(DARK_START + 0.1, DARK_START + 0.6, t);
        let near = (pose.distance / 3.0).min(1.0).powi(2);
        *scene[self.cursor_light]
            .light_mut()
            .expect("the cursor light is a light")
            .intensity_mut() = (0.03 * blink(t)
            + 16.0 * alone * (1.0 + grow(t)) * near * (1.0 - flash(t)).powi(2))
            * flicker;
        *scene[self.hall_light]
            .light_mut()
            .expect("the hall light is a light")
            .intensity_mut() = 0.1 * far * (0.6 + 0.4 * field::screen_light(t, 0.3));
        *scene[self.screen_light]
            .light_mut()
            .expect("the screen light is a light")
            .intensity_mut() = 0.08 * glow + 0.01 * text.typed as f32 / QUESTION.len() as f32;

        // the dust shows only in the close views of the first cursor
        if t < DUST_END {
            scene.add(self.dust(t, &camera, cursor, light, glow));
        }
        scene.add(star(&camera, MAIN + cursor, light));
        if t > ARC_START {
            // the velocity of the camera, so the sparks streak as the lens sees them
            let velocity = (self.place(t + 0.004).1 - self.place(t - 0.004).1) / 0.008;
            scene.add(Node::particles(
                sparks(MAIN + cursor, t, velocity),
                Blending::Additive,
            ));
        }
        scene.atmosphere = Some(atmosphere(t, far, pose.distance));
        // The floor shows the light of the cursor in the rush. Before, its reflections do
        // not show and cost a second view.
        if t < REFLECT_START {
            scene.reflector = None;
        }
        (scene, camera)
    }

    /// Adds the question and the answers on the main screen. `light` dims all text.
    fn add_text(&self, scene: &mut Scene, text: &Text, light: f32) {
        if light <= 0.0 {
            return;
        }
        let line = |scene: &mut Scene, geometry: &Arc<Geometry>, row: f32, color: Rgb| {
            // a line fades out when it scrolls into the margin at the top
            let light = (row + 1.0).clamp(0.0, 1.0) * light;
            if light <= 0.0 || row >= ROWS as f32 {
                return;
            }
            let at = terminal::baseline(row);
            scene.add_to(
                self.main,
                Node::mesh(
                    geometry.clone(),
                    Material::unlit(color * (light / terminal::GRID_MEAN))
                        .map(self.terminal.grid.clone()),
                )
                .at(at.x, at.y, 0.0005)
                .shadows(false, false),
            );
        };
        line(
            scene,
            &self.terminal.question[text.typed],
            -text.scroll,
            Rgb::hex(QUESTION_COLOR) * QUESTION_LIGHT,
        );
        for i in 0..text.count.ceil() as usize {
            let row = 1.0 + i as f32 - text.scroll;
            if row <= -1.0 {
                continue;
            }
            let answer = terminal::page_line(i);
            let color = if terminal::answer(answer).starts_with("//") {
                Rgb::hex(COMMENT_COLOR) * COMMENT_LIGHT
            } else {
                Rgb::hex(field::CODE) * field::CODE_LIGHT
            };
            line(scene, &self.terminal.answers[answer], row, color);
        }
    }

    /// The dust in front of the main screen: motes in the air and specks on the glass. The
    /// cursor (at `cursor`, with the light `light`) and the text (`glow`) light them. The
    /// motes in front of and behind the focus are large and faint, as the lens shows them.
    fn dust(&self, t: f32, camera: &Camera, cursor: Vec3, light: f32, glow: f32) -> Node {
        let focus = camera.focus.map_or(1.0, |f| f.distance);
        let blur = camera.focus.map_or(0.0, |f| f.blur);
        let forward = camera.forward();
        let pixel = 2.0 * (camera.fov.to_radians() * 0.5).tan() / 1080.0;
        let mote = |p: Vec3, core: f32, shine: f32| {
            let world = MAIN + p;
            let depth = (world - camera.position).dot(forward);
            if depth < camera.near * 2.0 {
                return None;
            }
            // the light of the cursor and of the screen that the mote scatters to the lens
            let to_mote = p - cursor;
            let toward = to_mote
                .normalize_or_zero()
                .dot((camera.position - world).normalize_or_zero())
                .max(0.0);
            let lit = light / (to_mote.length_squared() / 0.0025 + 0.1)
                * (0.3 + 3.0 * toward.powi(8))
                + 2.0 * glow / (1.0 + (p.z / 0.25).powi(2))
                + 0.004;
            let defocus = blur * (1.0 - focus / depth).abs() * pixel * depth;
            let size = core.hypot(defocus);
            let energy = (core / size).powi(2);
            Some(Particle::new(
                world,
                size,
                Rgb::hex(CURSOR_COLOR) * (lit * energy * shine),
            ))
        };
        let motes = self.dust.iter().enumerate().filter_map(|(i, base)| {
            let s = i as u32 * 3;
            let drift = vec3(
                noise1(s, t * 0.25),
                noise1(s + 1, t * 0.2) + 0.2 * t,
                noise1(s + 2, t * 0.22),
            ) * 0.01;
            mote(*base + drift, 0.00004 * (0.5 + hash(s + 7)), 15.0)
        });
        let specks = self.specks.iter().enumerate().filter_map(|(i, p)| {
            mote(
                *p,
                0.00003 * (0.4 + hash(i as u32 + 5)),
                30.0 * hash(i as u32 + 9),
            )
        });
        // The large motes near the lens are discs as wide as their blur. Each disc keeps a
        // faint light, so the eye sees the air between the lens and the screen.
        let discs = self.bokeh.iter().enumerate().filter_map(|(i, base)| {
            let s = i as u32 * 5 + 3;
            let p = *base + vec3(noise1(s, t * 0.3), noise1(s + 1, t * 0.25), 0.0) * 0.01;
            let world = MAIN + p;
            let depth = (world - camera.position).dot(forward);
            let disc = blur * (1.0 - focus / depth).abs();
            if depth < camera.near * 2.0 || disc < 6.0 {
                return None;
            }
            let size = disc * pixel * depth;
            let to_cursor = (p - cursor).length_squared();
            let lit = 0.04 + 0.15 * light / (to_cursor / 0.01 + 1.0) + 0.1 * glow;
            Some(Particle::new(
                world,
                size,
                Rgb::hex(CURSOR_COLOR) * (lit * (0.6 + 0.8 * hash(s + 2))),
            ))
        });
        Node::particles(
            motes.chain(specks).chain(discs).collect(),
            Blending::Additive,
        )
    }

    /// Motion blur samples at `t`: the scrolling code and the fast moves of the camera blur.
    /// The pull back moves the screens near the camera fast, so it takes many samples.
    /// Without the reflection of the floor they cost less.
    pub fn shutter(&self, t: f32) -> usize {
        if t < ANSWER_START + 0.2 {
            1
        } else if t < 4.0 {
            4
        } else if t < REFLECT_START {
            7
        } else {
            6
        }
    }

    /// The SVG over the picture at `t`. The shot has none: the flash at the end comes from
    /// the light of the cursor in the 3D picture.
    pub fn overlay(&self, _t: f32) -> Svgr<'static> {
        Svgr::empty()
    }
}

/// The air at `t`. `far` is `0..1` from a camera at the screen to a camera far away. Near
/// the screen the haze is dense and fine, so the light of the cursor shows in it. In the
/// field it is thin and wide: the glow of thousands of screens. In the flash the air
/// between the lens and the cursor, `distance` meters, fills with light, so the
/// brightness of the frame follows [`air_glow`].
fn atmosphere(t: f32, far: f32, distance: f32) -> Atmosphere {
    // after the pull back the air stays thin, also when the camera rushes back in
    let wide = far.max(smoothstep(4.3, 4.9, t));
    let lerp = |a: f32, b: f32| a + (b - a) * wide;
    let screens = field::screen_light(t, 0.3);
    Atmosphere {
        density: lerp(0.3, 0.02) + 2.5 * flash(t).powi(2) / distance.max(0.02),
        color: Rgb::hex(0xcfe4ff),
        base: SCREEN_Y,
        falloff: lerp(0.0, 0.25),
        anisotropy: 0.55,
        ambient: Rgb::linear(0.018, 0.036, 0.08) * (wide * screens)
            + Rgb::hex(CURSOR_COLOR) * air_glow(t),
        noise: 0.6,
        noise_size: lerp(0.05, 6.0),
        offset: vec3(0.02 * t, 0.01 * t, 0.0),
        distance: lerp(2.0, 220.0),
        steps: 32,
    }
}

/// The cursor as a point of light that stays visible from far away, where its block is
/// smaller than a pixel. `light` is the light of the cursor block.
fn star(camera: &Camera, at: Vec3, light: f32) -> Node {
    // the radius of 1.6 pixels at 1080 lines, at the distance of the cursor
    let pixel = camera.pixel_size(at, 1.0, 1080.0).recip();
    let size = 1.6 * pixel;
    // The star shows only where the block is smaller than a pixel. It spreads the light
    // of the block over its own area, four times stronger, so the eye finds it.
    let block = 0.6 * EM;
    let shine = (block / size).powi(2).min(1.0) * smoothstep(0.5 * block, 1.5 * block, size);
    Node::particles(
        vec![Particle::new(
            at,
            size,
            Rgb::hex(CURSOR_COLOR) * (12.0 * light * shine),
        )],
        Blending::Additive,
    )
}

/// The sparks of the cursor when it burns like a welding arc, as in the factory of shot 2.
/// The sparks fly out of the screen toward the camera. Each spark comes from its birth
/// time, so they have no state. The tails show the motion relative to a camera that moves
/// at `camera` meters per second, so the motion blur samples join into one streak.
fn sparks(at: Vec3, t: f32, camera: Vec3) -> Vec<Particle> {
    const RATE: f32 = 320.0;
    const LIFE: f32 = 0.45;
    let newest = (t * RATE).floor() as i64;
    let first = (ARC_START * RATE).ceil() as i64;
    (0..(RATE * LIFE) as i64)
        .map(|k| newest - k)
        .filter(|born| *born >= first)
        .filter_map(|born| {
            let age = t - born as f32 / RATE;
            let h = |i: u32| hash((born as u32).wrapping_mul(31).wrapping_add(i + 7001));
            let life = 1.0 - age / (LIFE * (0.4 + 0.6 * h(5)));
            if life <= 0.0 {
                return None;
            }
            // out of the glass, spread up and to the sides
            let velocity =
                vec3(h(1) * 2.0 - 1.0, h(2) * 1.6 - 0.4, 0.5 + h(3)) * (0.4 + 1.6 * h(4) * h(4));
            let gravity = vec3(0.0, -6.0, 0.0);
            let position = at + velocity * age + gravity * (0.5 * age * age);
            let speed = velocity + gravity * age;
            let heat = life * life;
            Some(Particle {
                position,
                tail: (camera - speed) * 0.012,
                size: 0.0005 + 0.0004 * h(6),
                color: Rgb::linear(0.8 + 0.2 * heat, 0.88 + 0.12 * heat, 1.0) * (3.0 + 40.0 * heat),
                opacity: 1.0,
            })
        })
        .collect()
}
