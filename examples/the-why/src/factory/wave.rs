//! Shot 7: the wave. Night in the hall: cold lamps, the arms still at work. The light of the
//! line comes over the desert and through the glass wall, and runs down the center aisle.
//! Each arm stops when the light passes it and turns to the glass wall and the light. The
//! shot ends close on the line, which becomes the first cable of the bridge in shot 8.

use super::*;

/// The time in shot 7 at which the light of the line reaches the glass wall.
const ARRIVAL: f32 = 0.9;
/// The speed of the light down the hall: slow at the glass wall, then fast, in meters per
/// second.
const SLOW: f32 = 12.0;
const FAST: f32 = 60.0;
/// The time that an arm takes to turn to the light, in seconds.
const TURN: f32 = 1.4;

/// The Z of the front of the light at `t`. It is at the glass wall at `ARRIVAL`.
fn front(t: f32) -> f32 {
    let s = (t - ARRIVAL).max(0.0);
    hall::Z_FAR + SLOW * s + (FAST - SLOW) * glide(0.8, 3.5, s)
}

/// The time at which the light passes `z`.
fn passes(z: f32) -> f32 {
    if z <= hall::Z_FAR {
        return ARRIVAL;
    }
    // the front moves forward all the time, so a bisection finds the time
    let (mut a, mut b) = (ARRIVAL, ARRIVAL + 20.0);
    for _ in 0..24 {
        let m = f32::midpoint(a, b);
        if front(m) < z {
            a = m;
        } else {
            b = m;
        }
    }
    f32::midpoint(a, b)
}

/// How far an arm at `z` has turned to the light at `t`, `0..1`, with a small overshoot at
/// the end of the turn as the servos stop.
fn turned(z: f32, t: f32) -> f32 {
    let k = ((t - passes(z)) / TURN).clamp(0.0, 1.0);
    let ease = k * k * (3.0 - 2.0 * k);
    ease + 0.06 * (std::f32::consts::PI * k).sin() * k
}

impl FactorySet {
    /// The hall at night: dim cold lamps, dark haze, and the night sky over the desert behind
    /// the glass wall. The warm light of the line comes in through the glass wall from
    /// `ARRIVAL` on.
    fn night(&self, t: f32, camera: &Camera, shadow_bounds: Aabb) -> Scene {
        let mut scene = self.scene.clone();
        // The warm light comes in, then dims a little as the camera comes down to the line.
        let warm = smoothstep(ARRIVAL, ARRIVAL + 1.2, t) * (1.0 - 0.3 * smoothstep(5.8, 8.0, t));
        // At the end the air over the floor glows with the light of the line, so the arms
        // stand dark against it.
        let glow_air = smoothstep(4.5, 7.5, t);
        scene[self.sun].object = Object::Light(
            Light::directional(WARM, 2.2 * warm, vec3(0.0, 0.07, -1.0)).with_shadow(Shadow {
                map_size: 4096,
                softness: 2.0,
                bounds: Some(shadow_bounds),
                ..Default::default()
            }),
        );
        // The haze is thin while the low warm light comes through the glass wall. Thick
        // haze in front of the light turns the far rows of arms into a flat bright wall.
        let thin = smoothstep(ARRIVAL, ARRIVAL + 1.0, t) * (1.0 - smoothstep(4.0, 6.0, t));
        scene.atmosphere = Some(Atmosphere {
            density: 0.014 * (1.0 - 0.75 * thin),
            color: Rgb::hex(0xe8eef8),
            base: 1.0,
            falloff: 0.05,
            anisotropy: 0.65 + 0.2 * thin,
            ambient: Rgb::hex(0x0c1a30).lerp(Rgb::hex(0x46260e), glow_air),
            noise: 0.5,
            noise_size: 8.0,
            offset: vec3(0.3, 0.05, 0.2) * t,
            distance: 260.0,
            // more steps than by day: the low warm light makes long shafts through the
            // arms, and with few steps their noise flickers
            steps: 40,
        });
        // the night sky over the desert, lighter at the horizon
        scene.background = Background::Gradient {
            top: Rgb::hex(0x060c1c),
            bottom: Rgb::hex(0x1c2c48),
        };
        scene.environment = Environment::None;
        scene.add(Node::light(Light::hemisphere(0x34486a, 0x06080c, 0.8)));
        self.hall.light_lamps(&mut scene, Rgb::hex(0xbcd8ff) * 6.0);
        // The lamps nearest to the start of the camera. A choice that follows the camera
        // swaps a lamp during the shot, and its light pops.
        self.hall.lamp_lights(
            &mut scene,
            vec3(2.9, 0.85, hall::Z_FAR + 40.0),
            5,
            Light::point(0xbcd8ff, 280.0, 32.0),
        );
        self.hall.night(&mut scene);
        // The glass wall flares when the light comes through it and stays warm. The light
        // from behind draws a warm rim on the arms.
        // After the flare the panes glow only a little: the haze behind them carries the
        // light, so the wall has depth and the far arms stand dark against it.
        let flash = smoothstep(ARRIVAL - 0.05, ARRIVAL + 0.1, t)
            * (1.0 - smoothstep(ARRIVAL + 0.1, ARRIVAL + 1.4, t));
        self.hall
            .light_glass(&mut scene, Rgb::hex(WARM) * (0.015 * warm + 0.9 * flash));
        // Outside, the light of the line glows over the desert, low behind the middle of
        // the glass wall. It is the source of the warm light in the hall.
        // Its glow lies along the horizon: a band that fades to the sides and upward, so the
        // upper panes keep the night sky and the wall has a gradient behind the arms.
        // The glow sits low: its discs reach up to 7 m on the wall, so the upper panes
        // keep the night and the far arms stand dark against the band of light. Wider
        // discs turn the whole wall into one flat field of light.
        let glow = vec3(0.0, 1.5, hall::Z_FAR - 50.0);
        let mut particles = vec![
            Particle::new(glow, 4.0, Rgb::hex(0xffd29a) * (3.0 * warm)),
            Particle::new(
                vec3(0.0, -4.0, hall::Z_FAR - 50.0),
                22.0,
                Rgb::hex(WARM) * (0.4 * warm),
            ),
        ];
        particles.extend((-4..=4).map(|i| {
            let side = 1.0 - (i as f32 / 5.0).abs();
            Particle::new(
                vec3(i as f32 * 22.0, -6.0, hall::Z_FAR - 50.0),
                26.0,
                Rgb::hex(0xff8a2a) * (0.1 * side * warm),
            )
        }));
        scene.add(Node::particles(particles, Blending::Additive));
        self.crowd.rim(&mut scene, Rgb::hex(WARM) * (0.25 * warm));
        // the stars over the desert
        let stars = &mut scene[self.stars];
        stars.visible = true;
        stars.position = camera.position;
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

    /// Shot 7: the wave.
    pub fn moment_wave(&self, t: f32) -> (Scene, Camera) {
        let camera = self.wave_camera(t);
        let bounds = Aabb {
            min: vec3(-hall::HALF_WIDTH, 0.0, hall::Z_FAR),
            max: vec3(hall::HALF_WIDTH, hall::WALL + 4.0, hall::Z_FAR + 200.0),
        };
        let mut scene = self.night(t, &camera, bounds);
        let view = View::new(&camera);

        // The arms work until the light passes them. Then they stop and turn to the glass
        // wall. Their eyes go from cold to warm.
        let time = t + 40.0;
        let hint = crowd::rest();
        let cold = Rgb::hex(COLD) * 3.0;
        let warm = Rgb::hex(WARM) * 10.0;
        let posed: Vec<Posed> = self
            .stations
            .iter()
            .enumerate()
            .map(|(i, s)| {
                let z = s.base.w_axis.z;
                let stop = time.min(passes(z) + 40.0);
                let working = crowd::joints(&crowd::work(stop + s.phase), &hint);
                let k = turned(z, t);
                let joints = if k > 0.0 {
                    let look = s.base.inverse().transform_vector3(vec3(0.0, 0.3, -1.0));
                    let looking = crowd::looking(look, 1.95, &working);
                    blend(&working, &looking, k)
                } else {
                    working
                };
                Posed {
                    base: s.base,
                    joints,
                    eye: cold.0.lerp(warm.0, k.min(1.0)).into(),
                    tint: crowd::tint(i),
                }
            })
            .collect();
        self.crowd
            .pose(&mut scene, &posed, |a| view.lod(a.base.w_axis.truncate()));
        let belt_time = time.min(ARRIVAL + 40.0);
        self.hall.conveyors(
            &mut scene,
            belt_time,
            camera.position.z + 40.0,
            camera.position.z - 260.0,
            Rgb::hex(0x8fd0ff) * 3.0,
        );
        self.hall.machines(
            &mut scene,
            &|p| time.min(passes(p.z) + 40.0),
            |time, phase| crowd::work(time + phase).welding,
            |p, r| view.sees(p, r),
        );

        // the sparks of the arms near the camera die when they stop
        let mut sparks = Vec::new();
        for (index, (station, arm)) in self.stations.iter().zip(&posed).enumerate() {
            let base = station.base.w_axis.truncate();
            if base.distance(camera.position) > 45.0 || !view.sees(base + Vec3::Y, 2.5) {
                continue;
            }
            let stop = passes(base.z) + 40.0;
            let tip =
                crowd::frames(arm.base, &arm.joints)[6].transform_point3(vec3(0.0, 0.0, arm::GRIP));
            let welding = |b: f32| b < stop && crowd::work(b + station.phase).welding;
            sparks.extend(weld_sparks(index as u32, tip, time, welding));
        }
        add_sparks(&mut scene, sparks, &camera);

        for hero in [&self.welder, &self.watcher] {
            let z = hero.base.w_axis.z;
            let working =
                crowd::joints(&crowd::work(time.min(passes(z) + 40.0) + hero.phase), &hint);
            let k = turned(z, t);
            let look = hero.base.inverse().transform_vector3(vec3(0.0, 0.3, -1.0));
            let looking = crowd::looking(look, 1.95, &working);
            hero.arm.apply(
                &mut scene,
                &blend(&working, &looking, k),
                arm::OPEN * 0.4,
                &Expression {
                    ring: 2.0 * (1.0 - k),
                    status: 3.0 * (1.0 - k),
                    eye: 4.0 + 8.0 * k,
                    lid: 1.0,
                },
            );
            self.show(&mut scene, hero, &view);
        }
        // nobody stands on the catwalk at night
        scene[self.engineer.root].visible = false;
        scene[self.laptop].visible = false;
        scene[self.props].visible = false;
        self.add_line(&mut scene, t);
        // Close to the floor, the line rakes the concrete in front of the camera with warm
        // light. Without it the lower half of the last frames is black.
        let rake = smoothstep(5.0, 7.0, t);
        if rake > 0.0 {
            // The low light shows the bumps of the concrete as glints. Fine bumps make
            // glints that change in each frame when the camera drifts, so they flatten.
            if let Some(m) = scene[self.hall.floor].material_mut() {
                m.normal_scale *= 1.0 - 0.75 * rake;
            }
            // Five lights act as one long light. Uneven gaps and levels keep their sheens on
            // the floor from looking like a row of lamps. They stand still in the hall: a
            // light that moves with the camera makes the glints on the floor crawl. The
            // engine draws 16 lights, and the scene has room for five more.
            for (dz, level) in [
                (-7.1f32, 0.35f32),
                (-5.3, 0.22),
                (-4.2, 0.4),
                (-2.5, 0.28),
                (-1.1, 0.33),
            ] {
                // 30 cm over the floor: the sheen is broad and soft. Lights closer to the
                // floor graze the bumps of the concrete and make glints that boil.
                scene.add(Node::light(Light::point(WARM, level * rake, 5.0)).at(
                    0.0,
                    0.3,
                    hall::Z_FAR + 36.1 + dz,
                ));
            }
        }
        (scene, camera)
    }

    /// Two motion blur samples: the turn of the camera is slow enough for them.
    pub fn shutter_wave(&self, _t: f32) -> usize {
        2
    }

    /// The light of the line: a streak over the desert, then a glowing line on the floor of
    /// the center aisle with a bright head that lights the arms as it passes.
    fn add_line(&self, scene: &mut Scene, t: f32) {
        let core = Rgb::hex(WARM);
        if t < ARRIVAL {
            // the streak outside, coming to the glass wall along the ground
            let z = hall::Z_FAR - (ARRIVAL - t) * 400.0;
            let head = vec3(0.0, 0.15, z);
            scene.add(Node::particles(
                vec![
                    Particle {
                        position: head,
                        tail: vec3(0.0, 0.0, -60.0),
                        size: 0.6,
                        color: core * 40.0,
                        opacity: 1.0,
                    },
                    Particle::new(head, 6.0, core * 1.5),
                ],
                Blending::Additive,
            ));
            scene.add(Node::light(Light::point(WARM, 400.0, 0.0)).at(head.x, 0.6, head.z));
            return;
        }
        // the flare where the light comes through the glass
        let flare = 1.0 - smoothstep(ARRIVAL, ARRIVAL + 1.5, t);
        if flare > 0.0 {
            let at = vec3(0.0, 0.3, hall::Z_FAR + 0.3);
            scene.add(Node::particles(
                vec![Particle::new(at, 0.2 + 0.4 * flare, core * (25.0 * flare))],
                Blending::Additive,
            ));
        }
        let head = front(t).min(hall::Z_NEAR - 2.0);
        let length = head - hall::Z_FAR;
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
        if head < hall::Z_NEAR - 3.0 {
            let tip = vec3(0.0, 0.05, head);
            scene.add(Node::particles(
                vec![
                    Particle::new(tip, 0.12, core * 80.0),
                    Particle::new(tip, 0.6, core * 4.0),
                ],
                Blending::Additive,
            ));
            scene.add(Node::light(Light::point(WARM, 60.0, 0.0)).at(tip.x, 0.5, tip.z));
        }
        // lights along the line keep the floor and the arms warm
        for z in [hall::Z_FAR + 30.0, hall::Z_FAR + 100.0] {
            if z < head {
                scene.add(Node::light(Light::point(0xffa04a, 10.0, 36.0)).at(0.0, 0.3, z));
            }
        }
    }

    /// The camera of shot 7. It stands low in the hall and looks past the working arms at the
    /// glass wall. The light comes through the wall and runs toward the camera, and the arms
    /// turn to it. Then the camera goes down to the floor beside the line and turns 30
    /// degrees off the axis of the hall: the line lies across the picture, and the turned
    /// arms stand over it against the warm glass wall.
    fn wave_camera(&self, t: f32) -> Camera {
        let z0 = hall::Z_FAR;
        let start = vec3(2.9, 0.85, z0 + 40.0) + vec3(0.0, -0.1, -2.5) * smoothstep(0.0, 3.6, t);
        // the end, with a slow drift along the line
        let floor = vec3(3.4, 0.05, z0 + 36.6 - 0.5 * smoothstep(6.0, 8.5, t));
        let down = smoothstep(3.6, 7.2, t);
        let position = start.lerp(floor, down);
        // The view turns from the glass wall to the point of the line ahead of the camera.
        let across = vec3(-0.866, 0.0, -0.5);
        let line = floor + across * (floor.x / 0.866);
        let glass = (vec3(-0.5, 2.4, z0) - start).normalize();
        let ahead = (vec3(line.x, 0.006, line.z) - floor).normalize();
        let look = glass.slerp(ahead, smoothstep(3.8, 7.0, t));
        let mm = 30.0 + 10.0 * smoothstep(4.5, 7.4, t);
        let shake = 0.003 * (1.0 - smoothstep(6.0, 8.0, t));
        let mut camera = lens(mm)
            .position(position)
            .look_at(position + look * 10.0)
            .shake(t * 0.4, shake, 21);
        camera.near = 0.02;
        camera.far = 3000.0;
        // The focus follows the front of the wave to the camera, then rests on the line. The
        // blur stays low, so the arms behind the line stay clear.
        let front = vec3(4.8, 1.5, front(t).clamp(z0 + 6.0, z0 + 34.0));
        let focus = front.lerp(vec3(line.x, 0.006, line.z), smoothstep(3.6, 6.0, t));
        camera.focus_on(focus, 4.0 - 1.5 * smoothstep(5.6, 8.0, t))
    }
}
