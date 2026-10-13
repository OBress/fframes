//! Part 1, 0 to 5 s: the Golden Gate Bridge at blue hour, built by machines in time-lapse
//! in the real strait between San Francisco and the Marin Headlands. A bank of fog rolls in
//! from the ocean under the deck.
//!
//! - 0.0: a close shot of the warm pilot line between the future towers. It crosses the
//!   picture at its middle, against the dark blue sky of the east.
//! - 0.12 to 2.2: the camera pulls back 1.2 km and swings to the hill of Battery Spencer
//!   on the Marin side: the view down the bridge to the city.
//! - 0.8 to 2.0: the towers rise lift by lift with climbing cranes and cold work lights.
//! - 1.85 to 2.8: on the catwalks, the spinning wheels run back and forth, and the main
//!   cables grow around the pilot line.
//! - 2.6 to 3.25: the suspenders drop from the cables, from the towers outward.
//! - 2.95 to 4.0: deck segments rise from barges and join from the towers outward.
//! - 4.0 to 4.8: the lamps run on warm, the towers glow in their flood lights, and the
//!   lights of the San Francisco waterfront and of Sausalito come on outward from the
//!   bridge.
//! - 4.3 to 5.0: the first car crosses to Marin. Its headlights flare up, and the flare is
//!   the match cut to the pen of the greenhouse.

use std::sync::Arc;

use fframes_3d::math::{progress, smoothstep};
use fframes_3d::*;

use super::bay::{self, Bay, from_bridge, geo};
use super::golden_gate::{
    self, ANCHOR, CABLE_RADIUS, CABLE_Z, HALF_SPAN, Lift, Paints, TOWER_TOP, cable_at, road_y,
};
use super::{AMBER, COLD, COLD_BLUE, camera, ease, ease_in, ease_out, mix, sky, span};
use crate::assets::Assets;

const TOWERS: (f32, f32) = (0.8, 2.0);
const CATWALKS: f32 = 1.85;
const SPIN: (f32, f32) = (1.9, 2.8);
const SUSPENDERS: (f32, f32) = (2.6, 3.25);
const DECK: f32 = 2.95;
/// The deck closes in the middle of the main span.
const CLOSURE: f32 = 4.0;
/// The time of a deck segment from the barge to its place.
const LIFT: f32 = 0.22;
/// The first car starts at the south end.
const CAR_START: f32 = 4.3;
/// The time-lapse speed of the car in m/s.
const CAR_SPEED: f32 = 1900.0;
/// The bands of the city lights, by distance from the ends of the bridge.
const BANDS: usize = 6;
/// The cable segments of each main cable.
const CABLE_STEPS: usize = 240;

/// The scale of the bridge in shot 9: the main span is 256 m, so the towers stand on the
/// plain beside the canyon of the desert.
pub const LANDMARK_SCALE: f32 = 0.2;

/// The node ids that the frames animate. The bridge parts hang below a group node in the
/// space of the bridge.
struct Ids {
    lifts: Vec<NodeId>,
    cranes: [NodeId; 2],
    crane_lights: [NodeId; 2],
    pilot: NodeId,
    cables: NodeId,
    catwalks: NodeId,
    suspenders: NodeId,
    deck: [NodeId; 2],
    lamps: NodeId,
    barges: NodeId,
    barge_lights: [NodeId; 2],
    floods: [NodeId; 4],
    crown: NodeId,
    sparks: NodeId,
    car: NodeId,
    car_lights: NodeId,
    towers_mesh: NodeId,
    city: Vec<NodeId>,
    city_glow: [NodeId; 3],
    sutro_lights: NodeId,
}

/// The Golden Gate in the strait at blue hour.
pub struct Bridge {
    scene: Scene,
    ids: Ids,
    lifts: Vec<Lift>,
    /// The X of each deck segment and the time it starts to rise. The approaches stand from
    /// the start, with a time below zero.
    segments: Vec<(f32, f32)>,
    /// The top and the bottom of each suspender, and the time it starts to drop.
    suspenders: Vec<(Vec3, Vec3, f32)>,
    /// Over Battery Spencer: the camera of the wide shot, in the world.
    battery: Vec3,
    landmark: Scene,
}

/// The time at which the deck segment at `x` starts to rise: from the towers outward.
fn segment_start(x: f32) -> f32 {
    let from_tower = (x.abs() - HALF_SPAN).abs();
    DECK + from_tower / HALF_SPAN * (CLOSURE - DECK - LIFT)
}

impl Bridge {
    pub fn new(assets: &Assets) -> Self {
        let bay = Bay::load();
        let towers = golden_gate::towers();
        let (scene, ids) = build(assets, &bay, &towers.shapes, towers.crown);
        let segments = golden_gate::segment_centers()
            .into_iter()
            .map(|x| {
                (
                    x,
                    if x.abs() < ANCHOR {
                        segment_start(x)
                    } else {
                        -1.0
                    },
                )
            })
            .collect();
        let suspenders = golden_gate::suspenders()
            .into_iter()
            .map(|(top, bottom)| {
                let from_tower = (top.x.abs() - HALF_SPAN).abs() / HALF_SPAN;
                (
                    top,
                    bottom,
                    mix(SUSPENDERS.0, SUSPENDERS.1 - 0.2, from_tower),
                )
            })
            .collect();
        let battery = geo(37.8282, -122.4842, 0.0);
        let battery = battery + Vec3::Y * (bay.land.at(battery.x, battery.z) + 170.0);
        Self {
            scene,
            ids,
            lifts: towers.lifts,
            segments,
            suspenders,
            battery,
            landmark: golden_gate::bridge(&Paints::plain(), LANDMARK_SCALE, &|_, _| 0.0),
        }
    }

    /// The scene and the camera at `t` seconds, `0..5`.
    pub fn moment(&self, t: f32) -> (Scene, Camera) {
        let camera = self.camera_at(t);
        let mut scene = self.scene.clone();
        self.pose(&mut scene, t, &camera);
        if let Some(air) = &mut scene.atmosphere {
            // the fog comes in from the ocean through the Gate
            air.offset = vec3(-60.0 * t, 0.0, -8.0 * t);
        }
        (scene, camera)
    }

    /// The finished bridge for shot 9, see [`super::Landmarks`].
    pub fn landmark(&self) -> Scene {
        self.landmark.clone()
    }

    /// Sets the parts of `scene` to the state at `t`. `camera` sets the width of the thin
    /// lines, so they stay visible far away.
    fn pose(&self, scene: &mut Scene, t: f32, camera: &Camera) {
        let ids = &self.ids;
        let set = |scene: &mut Scene, id: NodeId, instances: Instances| {
            if let Some(mesh) = scene[id].mesh_mut() {
                mesh.instances = Some(Arc::new(instances));
            }
        };
        let mut sparks = Vec::new();
        let eye = bay::bridge_turn().inverse() * camera.position;
        // the size of one pixel of a 804 line picture at the point `p` of the bridge
        let pixel = |p: Vec3| 2.0 * eye.distance(p) * (camera.fov.to_radians() * 0.5).tan() / 804.0;

        // the towers rise lift by lift; a crane with cold lights rides on each
        let mut shapes: Vec<Vec<Mat4>> = vec![Vec::new(); ids.lifts.len()];
        let mut tops = [0.0f32; 2];
        for lift in &self.lifts {
            let start = mix(TOWERS.0, TOWERS.1 - 0.1, lift.bottom / TOWER_TOP);
            let g = progress(t, start, start + 0.1);
            if g <= 0.0 {
                continue;
            }
            let grown = lift.transform * Mat4::from_scale(vec3(1.0, g.max(0.02), 1.0));
            shapes[lift.shape].push(grown);
            let tower = usize::from(lift.transform.w_axis.x > 0.0);
            tops[tower] = tops[tower].max(mix(lift.bottom, lift.top, g));
        }
        for (id, transforms) in ids.lifts.iter().zip(shapes) {
            set(scene, *id, Instances::new(transforms));
        }
        let building = smoothstep(TOWERS.0 - 0.1, TOWERS.0, t)
            * (1.0 - smoothstep(TOWERS.1, TOWERS.1 + 0.3, t));
        for (i, x) in [-HALF_SPAN, HALF_SPAN].into_iter().enumerate() {
            let crane = &mut scene[ids.cranes[i]];
            crane.visible = building > 0.0;
            crane.position = vec3(x, tops[i], 0.0);
            crane.rotation =
                Quat::from_rotation_y(i as f32 * 2.5 + 0.8 * (t * 1.3 + i as f32).sin());
            scene[ids.crane_lights[i]].visible = building > 0.0;
            if let Some(light) = scene[ids.crane_lights[i]].light_mut() {
                *light.intensity_mut() = 3.0e4 * building;
            }
            if building > 0.0 {
                for z in [-CABLE_Z, CABLE_Z] {
                    let blink = if (t * 7.0 + z).fract() < 0.5 {
                        40.0
                    } else {
                        8.0
                    };
                    sparks.push(Particle::new(
                        vec3(x, tops[i] + 1.0, z),
                        1.0,
                        Rgb::hex(COLD_BLUE) * blink,
                    ));
                }
            }
        }

        // the main cables grow around the pilot line while the wheels spin them; the
        // catwalks hang below them
        let spun = ease(progress(t, SPIN.0, SPIN.1));
        let step = |k: usize, z: f32| {
            let x = |k: usize| -ANCHOR + 2.0 * ANCHOR * k as f32 / CABLE_STEPS as f32;
            (cable_at(x(k), z), cable_at(x(k + 1), z))
        };
        let mut cables = Vec::new();
        let mut walks = Vec::new();
        for z in [-CABLE_Z, CABLE_Z] {
            for k in 0..CABLE_STEPS {
                let (a, b) = step(k, z);
                if spun > 0.0 {
                    let r = (CABLE_RADIUS * spun).max(0.6 * pixel(a) * spun);
                    cables.push(span(a, b, r));
                }
                let down = Vec3::Y * 1.6;
                walks.push(span(a - down, b - down, 0.25_f32.max(0.5 * pixel(a))));
            }
        }
        set(scene, ids.cables, Instances::new(cables));
        set(scene, ids.catwalks, Instances::new(walks));
        scene[ids.catwalks].visible = t > CATWALKS && t < CLOSURE + 0.3;
        // the pilot line: warm and about one pixel wide at any distance
        let pilot: Vec<Mat4> = (0..CABLE_STEPS)
            .map(|k| {
                let (a, b) = step(k, -CABLE_Z);
                span(a, b, (0.7 * pixel(a)).max(0.02))
            })
            .collect();
        set(scene, ids.pilot, Instances::new(pilot));
        scene[ids.pilot].visible = spun < 0.9;
        // the spinning wheels: two on each cable, running from anchorage to anchorage
        if t > SPIN.0 - 0.05 && t < SPIN.1 + 0.1 {
            for (i, z) in [-CABLE_Z, -CABLE_Z, CABLE_Z, CABLE_Z]
                .into_iter()
                .enumerate()
            {
                let phase = t * 0.9 + i as f32 * 0.37;
                let x = ANCHOR * 0.98 * (1.0 - 2.0 * (2.0 * phase.fract() - 1.0).abs());
                let back = if phase.fract() < 0.5 { -40.0 } else { 40.0 };
                let p = cable_at(x, z) + Vec3::Y * 1.2;
                let tail = cable_at(x + back, z) + Vec3::Y * 1.2 - p;
                sparks.push(Particle::new(p, 1.4, Rgb::hex(COLD) * 40.0).tail(tail));
            }
        }

        // the suspenders drop from the cables
        let ropes: Vec<Mat4> = self
            .suspenders
            .iter()
            .filter_map(|&(top, bottom, start)| {
                let g = ease_out(progress(t, start, start + 0.25));
                (g > 0.0).then(|| span(top, top.lerp(bottom, g), 0.07_f32.max(0.45 * pixel(top))))
            })
            .collect();
        set(scene, ids.suspenders, Instances::new(ropes));

        // the deck segments rise from barges to their place; the lamps on them light up
        // from the middle outward
        let mut deck = Vec::new();
        let mut lamp_colors = Vec::new();
        let mut lifting = Vec::new();
        for &(x, start) in &self.segments {
            let g = progress(t, start, start + LIFT);
            if g <= 0.0 && start >= 0.0 {
                continue;
            }
            let rise = (1.0 - ease_out(g)) * (road_y(x) - 2.0);
            deck.push(Mat4::from_translation(Vec3::Y * -rise) * golden_gate::segment_at(x));
            if g > 0.0 && g < 1.0 {
                lifting.push(x);
            }
            let on = CLOSURE + 0.05 + x.abs() / ANCHOR * 0.5;
            let lamp =
                Rgb::hex(0x0c0d10).lerp(Rgb::hex(0xffa850) * 10.0, smoothstep(on, on + 0.08, t));
            lamp_colors.push(lamp);
        }
        for id in ids.deck {
            set(scene, id, Instances::new(deck.clone()));
        }
        set(scene, ids.lamps, Instances::new(deck).colors(lamp_colors));
        // a barge under each segment that rises, with cold flood lights
        let mut barges = Vec::new();
        for &x in &lifting {
            barges.push(Mat4::from_translation(vec3(x, 0.0, 0.0)));
            for z in [-CABLE_Z, CABLE_Z] {
                sparks.push(Particle::new(
                    vec3(x, road_y(x) + 6.0, z),
                    0.9,
                    Rgb::hex(COLD) * 30.0,
                ));
                sparks.push(Particle::new(
                    vec3(x, 4.0, z),
                    1.2,
                    Rgb::hex(COLD_BLUE) * 25.0,
                ));
            }
        }
        set(scene, ids.barges, Instances::new(barges));
        for (i, id) in ids.barge_lights.into_iter().enumerate() {
            let x = lifting.get(i * 2).copied();
            scene[id].visible = x.is_some();
            if let Some(x) = x {
                scene[id].position = vec3(x, 30.0, -CABLE_Z - 10.0);
            }
        }

        // the flood lights of the towers and the red lights on their tops
        let floods = smoothstep(CLOSURE + 0.15, CLOSURE + 0.5, t);
        for id in ids.floods {
            scene[id].visible = floods > 0.0;
            if let Some(light) = scene[id].light_mut() {
                *light.intensity_mut() = 9.0e5 * floods;
            }
        }
        if let Some(m) = scene[ids.crown].material_mut() {
            m.emissive = Rgb::hex(0xffa060);
            m.emissive_intensity = 0.6 * floods;
        }
        if t > TOWERS.1 && (t * 1.1).fract() < 0.55 {
            for x in [-HALF_SPAN, HALF_SPAN] {
                for z in [-CABLE_Z, CABLE_Z] {
                    let p = vec3(x, TOWER_TOP + 3.0, z);
                    sparks.push(Particle::new(p, 1.6, Rgb::hex(0xff2010) * 30.0));
                }
            }
        }

        // the city lights up outward from the ends of the bridge, Sausalito too
        for (band, id) in ids.city.iter().enumerate() {
            let at = CLOSURE + 0.2 + band as f32 * 0.12;
            let on = smoothstep(at, at + 0.25, t);
            if let Some(m) = scene[*id].material_mut() {
                m.color = Rgb::WHITE * mix(0.25, 1.6, on);
            }
        }
        let city = smoothstep(CLOSURE + 0.4, CLOSURE + 0.9, t);
        if let Some(m) = scene[ids.towers_mesh].material_mut() {
            m.emissive_intensity = mix(0.4, 2.2, city);
        }
        for id in ids.city_glow {
            if let Some(light) = scene[id].light_mut() {
                *light.intensity_mut() = mix(2.0e5, 9.0e5, city);
            }
        }
        if let Object::Particles(p) = &mut scene[ids.sutro_lights].object {
            let blink = if (t * 0.9).fract() < 0.5 { 30.0 } else { 3.0 };
            for item in &mut p.items {
                item.color = Rgb::hex(0xff2010) * blink;
            }
        }

        // the first car
        let car = &mut scene[ids.car];
        car.visible = t > CAR_START;
        car.position = car_position(t);
        // its head lights flare up before the cut; the glare is a lens effect that sits
        // 30 m nearer to the camera on the same ray, so the deck does not hide it
        let flare = ease_in(progress(t, 4.62, 5.0));
        let to_camera = (eye - car_position(t)).normalize();
        if let Object::Particles(p) = &mut scene[ids.car_lights].object {
            p.items[0].position = vec3(2.4, 0.8, 0.0) + to_camera * 30.0;
            p.items[0].size = 4.0 + 12.0 * flare;
            p.items[0].color = Rgb::hex(AMBER) * (3.0 + 32.0 * flare);
        }

        if let Object::Particles(p) = &mut scene[ids.sparks].object {
            p.items = sparks;
        }
    }

    /// The camera at `t`. It starts 10 m from the pilot line on the ocean side with a long
    /// lens, pulls back and swings up over Battery Spencer, drifts while the bridge grows,
    /// and at the end turns to the headlights of the first car.
    fn camera_at(&self, t: f32) -> Camera {
        let line = from_bridge(cable_at(0.0, -CABLE_Z));
        let start = from_bridge(cable_at(0.0, -CABLE_Z) + vec3(0.0, -2.0, -10.0));
        let wide_target = from_bridge(vec3(260.0, 70.0, 0.0));
        let car = from_bridge(car_position(5.0));

        // the pull back: the distance grows evenly in log space, and the direction turns
        // from the west to the Marin hill
        let pull = ease(progress(t, 0.12, 2.2));
        let turn = ease(progress(t, 0.35, 2.2));
        let d0 = start.distance(line);
        let d1 = self.battery.distance(wide_target);
        let distance = (d0.ln() + (d1.ln() - d0.ln()) * pull).exp();
        let dir = ((start - line) / d0)
            .lerp((self.battery - wide_target) / d1, turn)
            .normalize();
        let tilt = ease(progress(t, 0.9, 2.3));
        let mut target = line.lerp(wide_target, tilt);
        let mut position = target + dir * distance;
        let mut mm = mix(85.0, 24.0, ease(progress(t, 0.1, 1.9)));

        // the slow push while the bridge grows, then the turn to the car
        let drift = ease(progress(t, 2.2, 5.4));
        position += (wide_target - self.battery) * 0.08 * drift;
        let follow = ease(progress(t, CAR_START - 0.4, 5.0));
        target = target.lerp(car, follow);
        mm = mix(mm, 28.0, follow);

        let focus = line.lerp(target, tilt);
        let blur = mix(9.0, 2.5, ease(progress(t, 0.1, 1.2)));
        camera(mm, position, target)
            .focus_on(focus, blur)
            .clip(0.5, 30_000.0)
            .shake(t, 0.0004, 5)
    }
}

/// The position of the first car at `t`, in the space of the bridge: on the lanes to
/// Marin, nearer to the bay.
fn car_position(t: f32) -> Vec3 {
    let x = -ANCHOR + CAR_SPEED * (t - CAR_START);
    vec3(x, road_y(x) + 0.05, 5.0)
}

/// Builds the scene: the sky, the place, the bridge parts and the lights.
fn build(assets: &Assets, bay: &Bay, shapes: &[Geometry], crown: usize) -> (Scene, Ids) {
    // the last light of the day low in the west-south-west, the deep blue sky of the east
    let west = vec3(-0.94, 0.0, 0.34);
    let mut scene = Scene::new()
        .environment(Environment::map(
            sky(
                Rgb::hex(0x0b1838) * 0.9,
                Rgb::hex(0x5a78b0) * 0.4,
                Rgb::hex(0xff8a50) * 0.7,
                west.z.atan2(west.x),
            ),
            1.0,
        ))
        .background(Background::Environment { blur: 0.0 })
        .reflector(Reflector::new(Vec3::ZERO, Vec3::Y).scale(0.4));

    // the place
    let land = bay::land_mesh(bay);
    scene.add(Node::mesh(land, assets.dirt.material(0xffffff, 1.0, 0.0)).shadows(false, false));
    scene.add(bay::water());
    scene.add(bay::presidio(bay));
    let towers_mesh = scene.add(bay::towers(bay));
    let (sutro, sutro_spots) = bay::sutro(bay);
    scene.add(sutro);
    let sutro_lights = scene.add(Node::particles(
        sutro_spots
            .into_iter()
            .map(|p| Particle::new(p, 3.0, Rgb::hex(0xff2010) * 20.0))
            .collect(),
        Blending::Additive,
    ));
    let ends = [
        from_bridge(vec3(-ANCHOR, 0.0, 0.0)),
        from_bridge(vec3(ANCHOR, 0.0, 0.0)),
    ];
    let (texture, bands) = bay::city_lights(bay, ends, BANDS);
    let city = bands
        .into_iter()
        .map(|g| {
            let lights = Material::unlit(Rgb::WHITE).map(texture.clone()).additive();
            scene.add(Node::mesh(g, lights).shadows(false, false))
        })
        .collect();
    // the glow of the city in the haze: high, so it is a wide dome and not a spot
    let city_glow = [
        geo(37.800, -122.440, 450.0),
        geo(37.790, -122.405, 450.0),
        geo(37.857, -122.485, 350.0),
    ]
    .map(|p| scene.add(Node::light(Light::point(0xffb070, 0.0, 4000.0)).with_position(p)));

    // the bridge, in its own space
    let paints = Paints {
        // International Orange with the dents and the roughness of worn steel plate
        steel: Material::standard(golden_gate::ORANGE)
            .roughness(0.7)
            .normal_map(assets.steel.normal.clone())
            .ao_map(assets.steel.orm.clone()),
        concrete: assets.concrete.material(0xc4bcb0, 0.9, 0.0),
        road: assets.asphalt.material(0x707070, 0.9, 0.0),
    };
    let gate = scene.add(Node::group().with_rotation(bay::bridge_turn()));
    let ground = |x: f32, z: f32| {
        let p = from_bridge(vec3(x, 0.0, z));
        bay.land.at(p.x, p.z)
    };
    let lifts: Vec<NodeId> = shapes
        .iter()
        .map(|g| {
            let node = Node::instanced(g.clone(), paints.steel.clone(), Instances::default());
            scene.add_to(gate, node)
        })
        .collect();
    let crown = lifts[crown];
    let concrete = golden_gate::concrete(&ground);
    scene.add_to(gate, Node::mesh(concrete, paints.concrete.clone()));
    let fort = ground(-ANCHOR - 70.0, -20.0);
    scene.add_to(
        gate,
        Node::mesh(golden_gate::fort_point_arch(fort), paints.steel.clone()),
    );
    scene.add_to(
        gate,
        Node::mesh(
            golden_gate::fort_point(fort),
            Material::standard(0x7a4a3a).roughness(0.9),
        ),
    );
    let unit = Arc::new(Geometry::cylinder(1.0, 1.0, 1.0, 8).translate(0.0, 0.5, 0.0));
    let instanced =
        |material: Material| Node::instanced(unit.clone(), material, Instances::default());
    let cables = scene.add_to(gate, instanced(paints.steel.clone()));
    let pilot = scene.add_to(
        gate,
        instanced(Material::unlit(Rgb::hex(AMBER) * 9.0)).shadows(false, false),
    );
    let catwalks = scene.add_to(
        gate,
        instanced(Material::standard(0x3a3632).roughness(0.9)).shadows(false, false),
    );
    let suspenders = scene.add_to(gate, instanced(paints.steel.clone()).shadows(false, false));
    let (steel, road, lamp_heads) = golden_gate::deck_segment();
    let deck = [(steel, paints.steel.clone()), (road, paints.road.clone())]
        .map(|(g, m)| scene.add_to(gate, Node::instanced(g, m, Instances::default())));
    let lamps = scene.add_to(
        gate,
        Node::instanced(
            lamp_heads,
            Material::unlit(Rgb::WHITE),
            Instances::default(),
        )
        .shadows(false, false),
    );

    // the machines: climbing cranes on the towers and barges with derricks
    let crane_paint = Material::standard(0x3a3e44).roughness(0.6);
    let mut cranes = [NodeId(0); 2];
    let mut crane_lights = [NodeId(0); 2];
    for i in 0..2 {
        cranes[i] = scene.add_to(gate, Node::group());
        let jib = Geometry::merge([
            Geometry::cuboid(2.0, 16.0, 2.0).translate(0.0, 8.0, 0.0),
            Geometry::cuboid(40.0, 1.6, 1.6).translate(14.0, 16.0, 0.0),
            Geometry::cuboid(12.0, 1.4, 1.4).translate(-7.0, 16.0, 0.0),
        ]);
        scene.add_to(cranes[i], Node::mesh(jib, crane_paint.clone()));
        scene.add_to(
            cranes[i],
            Node::mesh(Geometry::sphere(0.7, 8, 6), Material::glow(COLD, 16.0)).at(34.0, 15.0, 0.0),
        );
        crane_lights[i] = scene.add_to(
            cranes[i],
            Node::light(Light::point(COLD, 0.0, 300.0)).at(20.0, 12.0, 0.0),
        );
    }
    let barge = Geometry::merge([
        Geometry::cuboid(34.0, 3.0, 22.0).translate(0.0, 1.0, 0.0),
        Geometry::cuboid(1.2, 18.0, 1.2).translate(-12.0, 11.5, -8.0),
        Geometry::cuboid(1.2, 18.0, 1.2).translate(-12.0, 11.5, 8.0),
        Geometry::cuboid(1.0, 1.0, 17.0).translate(-12.0, 20.0, 0.0),
    ]);
    let barges = scene.add_to(
        gate,
        Node::instanced(barge, crane_paint, Instances::default()),
    );
    let barge_lights =
        [0; 2].map(|_| scene.add_to(gate, Node::light(Light::point(COLD, 6.0e3, 200.0)).hidden()));
    // flood lights on the deck that shine up the north faces of the legs, the faces that
    // Marin sees
    let floods = [(-1.0f32, -1.0f32), (-1.0, 1.0), (1.0, -1.0), (1.0, 1.0)].map(|(side, z)| {
        let x = side * HALF_SPAN;
        let at = vec3(x + 45.0, road_y(x) - 4.0, z * CABLE_Z);
        scene.add_to(
            gate,
            Node::light(Light::spot(0xffc890, 0.0, 0.3, 0.9).pointing(vec3(-0.22, 1.0, 0.0)))
                .with_position(at)
                .hidden(),
        )
    });
    let sparks = scene.add_to(gate, Node::particles(Vec::new(), Blending::Additive));

    // the first car: a dark body with head and tail lights
    let car = scene.add_to(gate, Node::group().hidden());
    scene.add_to(
        car,
        Node::mesh(
            Geometry::rounded_box(4.6, 1.4, 1.9, 0.3, 2).translate(0.0, 0.9, 0.0),
            Material::standard(0x1a1c20).roughness(0.3).clearcoat(1.0),
        ),
    );
    let car_lights = scene.add_to(
        car,
        Node::particles(
            vec![
                Particle::new(vec3(3.2, 0.8, 0.0), 4.0, Rgb::hex(AMBER) * 3.0),
                Particle::new(vec3(2.4, 0.8, 0.65), 0.45, Rgb::hex(0xfff0d8) * 70.0),
                Particle::new(vec3(2.4, 0.8, -0.65), 0.45, Rgb::hex(0xfff0d8) * 70.0),
                Particle::new(vec3(-2.4, 0.9, 0.7), 0.3, Rgb::hex(0xff2a10) * 20.0),
                Particle::new(vec3(-2.4, 0.9, -0.7), 0.3, Rgb::hex(0xff2a10) * 20.0),
            ],
            Blending::Additive,
        ),
    );
    scene.add_to(
        car,
        Node::light(Light::spot(0xffe2b8, 3.0e4, 0.25, 0.6).pointing(vec3(1.0, -0.05, 0.0)))
            .at(2.6, 0.8, 0.0),
    );

    // the afterglow in the west-south-west and the light of the blue sky
    scene.add(Node::light(Light::directional(
        0xff9a70,
        0.5,
        west + Vec3::Y * 0.1,
    )));
    scene.add(Node::light(Light::hemisphere(0x5a78c0, 0x14161c, 1.6)));
    // the fog: thick on the water, thin above the deck, in banks that drift in
    scene.atmosphere = Some(Atmosphere {
        density: 0.014,
        color: Rgb::WHITE,
        base: 0.0,
        falloff: 0.045,
        anisotropy: 0.35,
        ambient: Rgb::hex(0x3a5490) * 0.28,
        noise: 0.95,
        noise_size: 110.0,
        offset: Vec3::ZERO,
        distance: 9000.0,
        steps: 36,
    });

    let ids = Ids {
        lifts,
        cranes,
        crane_lights,
        pilot,
        cables,
        catwalks,
        suspenders,
        deck,
        lamps,
        barges,
        barge_lights,
        floods,
        crown,
        sparks,
        car,
        car_lights,
        towers_mesh,
        city,
        city_glow,
        sutro_lights,
    };
    (scene, ids)
}
