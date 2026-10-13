//! Shot 4, "The perfect city" (29.0 to 42.0 s of the film, 13 s).
//!
//! The camera of shot 3 dives straight down into a chip. This shot continues the dive. The
//! chip is a city at night: the traces are streets, the components are the roofs of
//! buildings, the pins are rows of long buildings. The buildings rise, the windows light
//! up, and the camera levels out into a flight between the towers. Cranes build without
//! people, traffic lights change for nobody. At the end the camera tilts up into the stars,
//! where shot 5 starts.
//!
//! The world is in meters. The chip of shot 3 ([`chip`]) has the same plan at 1/45000
//! scale.
//!
//! Seconds of the shot:
//! - 0.0: straight down on the chip from 1880 m. The image top is +Z. The chip with its
//!   pins fills 94 % of the picture width.
//! - 0.6 to 2.4: the gold of the traces goes dark and they glow up with cold light. Then
//!   the glow fades and the street lamps and the traffic come on.
//! - 0.55 to 4.5: the buildings rise from the center outward and their windows light up.
//!   The die becomes concrete, the components become roofs.
//! - 1.1 to 4.6: the camera pulls out of the dive and levels out at 140 m.
//! - 4.6 to 10.2: the flight along the central boulevard and its canal, between the
//!   towers. The lights of the crossing ahead turn red at 7.5 s and green for the empty
//!   street at 8 s. At 9 to 11 s the camera turns toward a crane.
//! - 10.2 to 13.0: the camera climbs and tilts up into the stars.

#[path = "city/build.rs"]
mod build;
#[path = "city/layout.rs"]
mod layout;
#[path = "city/textures.rs"]
mod textures;
#[path = "city/traffic.rs"]
mod traffic;

use std::sync::Arc;

use fframes::Svgr;
use fframes_3d::math::{hash, smoothstep};
use fframes_3d::*;

use crate::assets::{Assets, Pbr};
use crate::look;
use build::{CraneParts, MeshBuf, RISE_TIME, RoofLight, STYLES, Signal};
use layout::{BAY, CHIP_HALF, FLOOR, PIN_PITCH, PIN_REACH, PINS, Plan, Use};
use traffic::Lane;

/// The direction of the camera at the end of the shot. Shot 5 starts with it.
pub const END_DIRECTION: Vec3 = Vec3::new(0.145, 0.906, 0.397);
/// The height of the camera above the street at the first frame, in meters. The chip
/// with its pins (1818 m) fills 94 % of the picture width.
pub const START_HEIGHT: f32 = 1880.0;
/// The length of the shot in seconds.
pub const DURATION: f32 = 13.0;
/// The size of the chip relative to the city.
pub const CHIP_SCALE: f32 = 1.0 / 45000.0;

/// The colors of the chip. Shot 3 shows the same materials on [`chip`].
const DIE: u32 = 0x0b0c0e;
const GOLD: u32 = 0xd99c3c;
const COPPER: u32 = 0xc0683a;
const SILVER: u32 = 0xd9dde3;
/// The faint cold glow of the traces.
const TRACE_GLOW: u32 = 0x9fc4ff;
/// The color of the components. The roof picture multiplies it.
const COMPONENT: u32 = 0x9aa0a8;

/// The cold light of the machines.
const COLD: u32 = 0xcfe4ff;
/// The point lights that light the mist over the boulevard ahead of the camera.
const AVENUE_LIGHTS: usize = 4;

/// The nodes of the buildings that rise together.
struct RiseNodes {
    root: NodeId,
    walls: [NodeId; 4],
    /// The roofs, in the order of [`build::RoofKind`].
    roofs: [NodeId; 3],
    crowns: NodeId,
    start: f32,
}

/// The nodes of one construction site.
struct SiteNodes {
    root: NodeId,
    slew: NodeId,
    trolley: NodeId,
    cable: NodeId,
    hook: NodeId,
    placed: NodeId,
    lamp: NodeId,
    start: f32,
    /// The base of the mast.
    base: Vec3,
    /// The angle of the jib over the frame, in radians around Y.
    place: f32,
    /// The distance from the mast to the frame center, in meters.
    radius: f32,
    reach: f32,
    height: f32,
    /// The height of the top floor of the frame.
    top: f32,
    seed: u32,
}

pub struct CitySet {
    scene: Scene,
    groups: Vec<RiseNodes>,
    ground: NodeId,
    parks: NodeId,
    park_root: NodeId,
    park_start: f32,
    streets: NodeId,
    /// The gold and the copper traces of the chip.
    traces: [NodeId; 2],
    lamp_light: NodeId,
    markings: NodeId,
    lamp_heads: NodeId,
    small_lights: NodeId,
    roof_lights: NodeId,
    /// The lights on the roofs at full height, with the start time of their rise.
    roof_light_list: Vec<(RoofLight, f32)>,
    cars: NodeId,
    car_bodies: NodeId,
    lanes: Vec<Lane>,
    signals: Vec<Signal>,
    signal_heads: NodeId,
    signal_pools: NodeId,
    /// The point lights of the key crossing: the light for the boulevard and for the
    /// street, with the index of a signal of each.
    signal_lights: [(NodeId, usize); 2],
    /// The lamps of the boulevard, by distance along it, and the point lights that move
    /// along them with the camera.
    avenue_lamps: Vec<Vec3>,
    avenue_lights: [NodeId; AVENUE_LIGHTS],
    /// The video screens, with the start time of the rise of their tower.
    screens: Vec<(NodeId, f32)>,
    screen_ids: Vec<NodeId>,
    sites: Vec<SiteNodes>,
    stars: NodeId,
    materials: Materials,
    /// The light of the city, after the chip.
    city_light: Environment,
}

/// The materials that change from the chip to the city.
struct Materials {
    die: Material,
    ground: Material,
    park_die: Material,
    park: Material,
    street_die: Material,
    asphalt: Material,
    gold: Material,
    copper: Material,
    /// The roofs on the chip, in the order of [`build::RoofKind`].
    chip_roofs: [Material; 3],
    roof: Material,
}

/// A copy of `city` with the scalar properties between `chip` (at `k` = 0) and `city`
/// (at `k` = 1).
fn blend(chip: &Material, city: &Material, k: f32) -> Material {
    let mix = |a: f32, b: f32| a + (b - a) * k;
    let mut m = city.clone();
    m.color = chip.color.lerp(city.color, k);
    m.roughness = mix(chip.roughness, city.roughness);
    m.metalness = mix(chip.metalness, city.metalness);
    m.emissive = chip.emissive * chip.emissive_intensity * (1.0 - k)
        + city.emissive * city.emissive_intensity * k;
    m.emissive_intensity = 1.0;
    m.env_intensity = mix(chip.env_intensity, city.env_intensity);
    m.normal_scale = mix(chip.normal_scale, city.normal_scale);
    m
}

/// `smoothstep` with zero acceleration at both ends (Perlin's smootherstep).
fn ease(edge0: f32, edge1: f32, t: f32) -> f32 {
    let x = ((t - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    x * x * x * (x * (x * 6.0 - 15.0) + 10.0)
}

/// A material with the normal and occlusion maps of `pbr`, without its color map.
fn surface(pbr: &Pbr, tint: u32, roughness: f32) -> Material {
    Material::standard(tint)
        .roughness(roughness)
        .normal_map(pbr.normal.clone())
        .ao_map(pbr.orm.clone())
}

/// The light of the night: a dark sky and a band of city glow around the horizon for the
/// reflections in the glass. The chip also has one large soft light straight above that
/// makes its metal shine. The city must not have it: the mirror glass of the towers would
/// show it when the camera looks up.
fn environment(chip: bool) -> Environment {
    let glow = Rgb::hex(0x5d7cb0) * 0.9;
    let band = |azimuth: f32| {
        let a = deg(azimuth);
        SoftBox::new(vec3(a.sin(), 0.05, a.cos()), 125.0, 7.0, glow)
    };
    let mut softboxes = vec![band(0.0), band(120.0), band(240.0)];
    if chip {
        softboxes.push(SoftBox::new(
            vec3(0.12, 1.0, 0.08),
            64.0,
            52.0,
            Rgb::hex(0xf4f1ea) * 1.1,
        ));
    }
    Environment::Studio(Studio {
        sky: Rgb::hex(0x03060d),
        horizon: Rgb::hex(0x1c2c48),
        ground: Rgb::hex(0x0b1220),
        softboxes,
        intensity: 1.0,
        rotation: 0.0,
    })
}

impl CitySet {
    pub fn new(assets: &Assets) -> Self {
        let plan = Plan::new();
        let mut scene =
            Scene::new()
                .environment(environment(true))
                .background(Background::Gradient {
                    top: Rgb::hex(0x061128),
                    bottom: Rgb::hex(0x091733),
                });

        // The chip and the city states of the materials that change.
        let roof_texture = textures::roof();
        let chip_roof = |color: u32, metalness: f32| {
            Material::standard(color)
                .roughness(0.28)
                .metalness(metalness)
                .map(roof_texture.clone())
        };
        let materials = Materials {
            die: surface(&assets.concrete, DIE, 0.3)
                .env_intensity(0.45)
                .normal_scale(0.2),
            ground: surface(&assets.concrete, 0x3c3e42, 0.9)
                .map(assets.concrete.color.clone())
                .env_intensity(0.3),
            park_die: Material::standard(DIE).roughness(0.3).env_intensity(0.45),
            park: Material::standard(0x0b130e)
                .roughness(0.95)
                .env_intensity(0.3),
            street_die: surface(&assets.asphalt, DIE, 0.3)
                .env_intensity(0.45)
                .normal_scale(0.2),
            // Wet asphalt: the lamps make highlights on it.
            asphalt: surface(&assets.asphalt, 0x111215, 0.3).env_intensity(0.7),
            gold: Material::metal(GOLD, 0.26)
                .emissive(TRACE_GLOW, 0.1)
                .normal_scale(0.1),
            copper: Material::metal(COPPER, 0.3)
                .emissive(TRACE_GLOW, 0.07)
                .normal_scale(0.1),
            chip_roofs: [
                chip_roof(COMPONENT, 0.6),
                chip_roof(0x000000, 0.0).env_intensity(0.0),
                Material::metal(SILVER, 0.24).map(roof_texture.clone()),
            ],
            roof: Material::standard(0x7c7f86)
                .roughness(0.85)
                .map(roof_texture)
                .env_intensity(0.2),
        };

        // The ground, the canal and the streets.
        let (ground, water, canal_walls) = build::ground();
        let ground = scene.add(Node::mesh(ground, materials.die.clone()).shadows(false, true));
        scene.add(
            Node::mesh(water, Material::standard(0x010306).roughness(0.06)).shadows(false, false),
        );
        scene.add(
            Node::mesh(
                canal_walls,
                surface(&assets.concrete, 0x3a3c40, 0.9).env_intensity(0.3),
            )
            .shadows(false, true),
        );
        let streets = scene.add(
            Node::mesh(build::streets(&plan), materials.street_die.clone()).shadows(false, true),
        );
        let [gold, copper] = build::traces(&plan, 1.0, 0.2);
        let traces = [
            scene.add(Node::mesh(gold.geometry(), materials.gold.clone()).shadows(false, false)),
            scene
                .add(Node::mesh(copper.geometry(), materials.copper.clone()).shadows(false, false)),
        ];
        let mut park_ground = MeshBuf::default();
        for b in plan.blocks.iter().filter(|b| b.use_ == Use::Park) {
            park_ground.ground_rect(b.min, b.max, 0.02);
        }
        let parks = scene.add(
            Node::mesh(park_ground.geometry(), materials.park_die.clone()).shadows(false, true),
        );
        let lamp_light = scene.add(
            Node::mesh(
                build::lamp_light(&plan),
                Material::unlit(Rgb::BLACK)
                    .map(textures::lamp_pools())
                    .additive(),
            )
            .shadows(false, false),
        );
        let markings = scene.add(
            Node::mesh(
                build::markings(&plan),
                Material::unlit(Rgb::BLACK).additive(),
            )
            .shadows(false, false),
        );
        let (bridges, bridge_lights) = build::bridges(&plan);
        scene.add(
            Node::mesh(
                bridges,
                surface(&assets.concrete, 0x6a6c70, 0.8).env_intensity(0.4),
            )
            .shadows(true, true),
        );

        // The street lamps.
        let (poles, heads) = build::lamps(&plan);
        scene.add(
            Node::instanced(
                build::lamp_pole(),
                Material::metal(0x5a5e66, 0.45).env_intensity(0.4),
                Instances::new(poles),
            )
            .shadows(false, false),
        );
        let lamp_heads = scene.add(Node::particles(
            heads
                .iter()
                .map(|&p| Particle::new(p, 0.4, Rgb::hex(0xe4efff) * 5.0))
                .collect(),
            Blending::Additive,
        ));
        // Every third lamp of the boulevard can carry a point light.
        let mut avenue_lamps: Vec<Vec3> =
            heads.iter().copied().filter(|h| h.x.abs() < 30.0).collect();
        avenue_lamps.sort_by(|a, b| a.z.total_cmp(&b.z));
        let avenue_lamps: Vec<Vec3> = avenue_lamps.into_iter().step_by(3).collect();
        let avenue_lights = [(); AVENUE_LIGHTS]
            .map(|()| scene.add(Node::light(Light::point(Rgb::hex(0xdbe8ff), 0.0, 70.0))));

        // The parks: trees, paths and their lamps. The trees rise with the buildings around.
        let park_root = scene.add(Node::group().scaled3(1.0, 0.002, 1.0));
        let park_start =
            build::group_start(build::group_of(build::rise_start(vec2(-545.0, 50.0), 7)));
        let tree = Geometry::merge([
            Geometry::cylinder(0.03, 0.04, 0.4, 6).translate(0.0, 0.2, 0.0),
            Geometry::icosahedron(0.33, 1)
                .scale(1.0, 1.15, 1.0)
                .translate(0.0, 0.68, 0.0),
        ]);
        scene.add_to(
            park_root,
            Node::instanced(
                tree,
                Material::standard(0x16241a)
                    .roughness(0.9)
                    .env_intensity(0.4),
                Instances::new(build::trees(&plan)),
            )
            .shadows(false, false),
        );
        let (paths, path_lights) = build::park_paths(&plan);
        scene.add(
            Node::mesh(paths, Material::standard(0x2a2c30).roughness(0.9)).shadows(false, true),
        );
        let small_lights = scene.add(Node::particles(
            path_lights
                .iter()
                .chain(&bridge_lights)
                .map(|&p| Particle::new(p, 0.3, Rgb::hex(COLD) * 4.0))
                .collect(),
            Blending::Additive,
        ));

        // The buildings, in groups that rise together.
        let facades: Vec<textures::Facade> = std::thread::scope(|s| {
            let handles: Vec<_> = STYLES
                .iter()
                .enumerate()
                .map(|(i, &style)| {
                    s.spawn(move || textures::Facade::new(style, 31 + i as u32 * 97))
                })
                .collect();
            handles.into_iter().filter_map(|h| h.join().ok()).collect()
        });
        let facade_material = |f: &textures::Facade| {
            let mut m = Material::standard(Rgb::WHITE)
                .map(f.color.clone())
                .emissive_map(f.emissive.clone())
                .emissive(Rgb::WHITE, 0.0)
                .roughness(1.0)
                .metalness(1.0)
                .env_intensity(0.9);
            m.metal_rough_map = Some(f.metal_rough.clone());
            m
        };
        let screen_texture = textures::screen();
        let details = Material::metal(0x6c7078, 0.55).env_intensity(0.35);
        let mut groups = Vec::new();
        let mut roof_light_list = Vec::new();
        let mut screens = Vec::new();
        for g in build::rise_groups(&plan) {
            let root = scene.add(Node::group().scaled3(1.0, 0.002, 1.0));
            let mut walls = [root; 4];
            for ((id, mesh), facade) in walls.iter_mut().zip(g.walls).zip(&facades) {
                *id = scene.add_to(root, Node::mesh(mesh.geometry(), facade_material(facade)));
            }
            let mut roofs = [root; 3];
            for ((id, mesh), material) in roofs.iter_mut().zip(g.roofs).zip(&materials.chip_roofs) {
                *id = scene.add_to(root, Node::mesh(mesh.geometry(), material.clone()));
            }
            scene.add_to(root, Node::mesh(g.details.geometry(), details.clone()));
            let crowns = scene.add_to(
                root,
                Node::mesh(g.crowns.geometry(), Material::glow(COLD, 0.0)).shadows(false, false),
            );
            for quad in g.screens {
                let id = scene.add_to(
                    root,
                    Node::mesh(
                        quad.geometry(),
                        Material::unlit(Rgb::BLACK).map(screen_texture.clone()),
                    )
                    .shadows(false, false)
                    .named("screen"),
                );
                screens.push((id, g.start));
            }
            roof_light_list.extend(g.lights.iter().map(|&l| (l, g.start)));
            groups.push(RiseNodes {
                root,
                walls,
                roofs,
                crowns,
                start: g.start,
            });
        }
        let screen_ids = screens.iter().map(|s| s.0).collect();
        let roof_lights = scene.add(Node::particles(Vec::new(), Blending::Additive));

        // The construction sites and their cranes.
        let steel = surface(&assets.steel, 0x8a8f98, 0.5)
            .metalness(0.6)
            .env_intensity(0.5);
        let crane_paint = Material::standard(0xd8dce2)
            .roughness(0.45)
            .env_intensity(0.5);
        let mut sites = Vec::new();
        for site in &plan.sites {
            let start =
                build::group_start(build::group_of(build::rise_start(site.center, site.seed)));
            let root = scene.add(Node::group().scaled3(1.0, 0.002, 1.0));
            let (frame, slabs, glass, work) = build::site_frame(site);
            scene.add_to(root, Node::mesh(frame, steel.clone()));
            scene.add_to(
                root,
                Node::mesh(
                    slabs,
                    surface(&assets.concrete, 0x7a7c80, 0.9).env_intensity(0.3),
                ),
            );
            scene.add_to(root, Node::mesh(glass, facade_material(&facades[0])));
            scene.add_to(
                root,
                Node::particles(
                    work.iter()
                        .map(|&p| Particle::new(p, 0.6, Rgb::hex(0xeef5ff) * 7.0))
                        .collect(),
                    Blending::Additive,
                ),
            );
            let parts = CraneParts::new(site.crane_height);
            let base = vec3(site.crane.x, 0.0, site.crane.y);
            let crane = scene.add_to(root, Node::group().with_position(base).named("crane"));
            scene.add_to(crane, Node::mesh(parts.mast, crane_paint.clone()));
            let slew = scene.add_to(crane, Node::group().at(0.0, site.crane_height, 0.0));
            scene.add_to(slew, Node::mesh(parts.top, crane_paint.clone()));
            let trolley = scene.add_to(slew, Node::mesh(parts.trolley, crane_paint.clone()));
            let cable = scene.add_to(
                trolley,
                Node::mesh(parts.cable, Material::metal(0x30333a, 0.4)),
            );
            let hook = scene.add_to(
                trolley,
                Node::mesh(parts.hook, Material::metal(0xb8bcc4, 0.4)),
            );
            scene.add_to(hook, Node::mesh(parts.load.clone(), steel.clone()));
            // The beam that the crane has put on the frame.
            let top = site.floors as f32 * FLOOR;
            let placed = scene.add_to(
                root,
                Node::mesh(parts.load, steel.clone())
                    .at(site.center.x, top + 3.5, site.center.y)
                    .hidden(),
            );
            // A flood light on the jib that lights the top of the frame. Only the sites
            // next to the flight have it, for the light limit of the renderer.
            let near = site.center.x.abs() < 300.0 && site.center.y < 700.0;
            let mut lamp =
                Node::light(Light::spot(0xe6f0ff, 0.0, deg(28.0), 0.6)).at(16.0, -0.5, 0.0);
            lamp.visible = near;
            let lamp = scene.add_to(slew, lamp);
            let to_frame = site.center - site.crane;
            sites.push(SiteNodes {
                root,
                slew,
                trolley,
                cable,
                hook,
                placed,
                lamp,
                start,
                base,
                place: (-to_frame.y).atan2(to_frame.x),
                radius: to_frame.length(),
                reach: parts.reach,
                height: site.crane_height,
                top,
                seed: site.seed,
            });
        }

        // The traffic and the traffic lights.
        let lanes = traffic::lanes(&plan);
        let cars = scene.add(Node::particles(Vec::new(), Blending::Additive));
        let car_bodies = scene.add(
            Node::instanced(
                Geometry::rounded_box(1.9, 1.35, 4.6, 0.35, 2).translate(0.0, 0.7, 0.0),
                Material::standard(0x101318)
                    .roughness(0.3)
                    .clearcoat(1.0)
                    .env_intensity(0.3),
                Instances::default(),
            )
            .shadows(false, false),
        );
        let (signal_poles, signals) = build::signal_poles(&plan);
        scene.add(
            Node::mesh(
                signal_poles,
                Material::metal(0x3a3e46, 0.5).env_intensity(0.4),
            )
            .shadows(false, false),
        );
        let signal_heads = scene.add(Node::particles(Vec::new(), Blending::Additive));
        let signal_pools = scene.add(
            Node::instanced(
                Geometry::floor(1.0, 1.0),
                Material::unlit(Rgb::WHITE).map(textures::spot()).additive(),
                Instances::default(),
            )
            .shadows(false, false),
        );
        // The key crossing gets two colored point lights: one for each direction.
        let signal_lights = [true, false].map(|avenue| {
            let i = signals
                .iter()
                .position(|s| s.key && s.avenue == avenue)
                .unwrap_or(0);
            let head = signals.get(i).map_or(Vec3::ZERO, |s| s.head);
            let id =
                scene.add(Node::light(Light::point(Rgb::WHITE, 0.0, 50.0)).with_position(head));
            (id, i)
        });

        // No moon: on the mirror glass it makes glints brighter than the windows.
        scene.add(Node::light(Light::hemisphere(0x0a1424, 0x2a4470, 0.35)));

        let stars = scene.add(crate::stars::stars());

        Self {
            scene,
            groups,
            ground,
            parks,
            park_root,
            park_start,
            streets,
            traces,
            lamp_light,
            markings,
            lamp_heads,
            small_lights,
            roof_lights,
            roof_light_list,
            cars,
            car_bodies,
            lanes,
            signals,
            signal_heads,
            signal_pools,
            signal_lights,
            avenue_lamps,
            avenue_lights,
            screens,
            screen_ids,
            sites,
            stars,
            materials,
            city_light: environment(false),
        }
    }

    /// The renderer of the shot for pictures of `width` x `height` pixels.
    pub fn renderer(width: u32, height: u32) -> Renderer {
        Renderer::new(width, height).exposure(1.0).bloom(Bloom {
            strength: 0.7,
            radius: 0.6,
            threshold: 1.0,
        })
    }

    /// Motion blur samples at `t`: four for the dive and the tilt, three for the flight.
    pub fn shutter(&self, t: f32) -> usize {
        if t < 4.6 || (10.6..12.6).contains(&t) {
            4
        } else {
            3
        }
    }

    /// Nothing over the picture.
    pub fn overlay(&self, _t: f32) -> Svgr<'_> {
        Svgr::empty()
    }

    /// The video screens on five towers along the flight. Each is an unlit quad of 30 x
    /// 22.5 m with UVs from 0 to 1 (V = 0 at the top), named "screen". To show a video, set
    /// the `map` of its material in the scene from [`CitySet::moment`]. The color of the
    /// material is the brightness: it comes on with the windows of the tower.
    pub fn screens(&self) -> &[NodeId] {
        &self.screen_ids
    }

    /// The scene and the camera at `t` seconds into the shot.
    pub fn moment(&self, t: f32) -> (Scene, Camera) {
        let camera = camera(t);
        let mut scene = self.scene.clone();
        let m = &self.materials;
        // The chip turns into the city between 0.8 and 2.8 s. The traces are gone at 2.4 s,
        // so the light above them can go then.
        let city = smoothstep(0.8, 2.8, t);
        if t >= 2.4 {
            scene.environment = self.city_light.clone();
        }
        set_material(&mut scene, self.ground, blend(&m.die, &m.ground, city));
        set_material(&mut scene, self.parks, blend(&m.park_die, &m.park, city));
        set_material(
            &mut scene,
            self.streets,
            blend(&m.street_die, &m.asphalt, city),
        );
        // The metal of the traces goes dark while they glow up with cold light: the current
        // runs. Then the glow fades out while the street lamps come on.
        let metal = smoothstep(0.6, 1.4, t);
        let flare = 0.9 * smoothstep(0.6, 1.3, t);
        let fade = 1.0 - smoothstep(1.6, 2.4, t);
        for (&id, chip) in self.traces.iter().zip([&m.gold, &m.copper]) {
            let mut trace = blend(chip, &m.asphalt, metal);
            trace.emissive =
                (chip.emissive * chip.emissive_intensity + Rgb::hex(0x7fb4ff) * flare) * fade;
            set_material(&mut scene, id, trace);
            scene[id].visible = fade > 0.0;
        }

        // The buildings rise. Their windows light up when they stand.
        let roofs = m
            .chip_roofs
            .each_ref()
            .map(|chip| blend(chip, &m.roof, city));
        for g in &self.groups {
            let rise = ease(g.start, g.start + RISE_TIME, t);
            scene[g.root].scale.y = rise.max(0.002);
            let windows = smoothstep(g.start + 0.6, g.start + RISE_TIME + 0.8, t);
            for (&w, light) in g.walls.iter().zip([1.5, 1.4, 1.3, 1.1]) {
                if let Some(mat) = scene[w].material_mut() {
                    mat.emissive_intensity = windows * light * textures::WINDOW_LIGHT;
                }
            }
            if let Some(mat) = scene[g.crowns].material_mut() {
                mat.color = Rgb::WHITE * (2.0 * windows);
            }
            for (&id, roof) in g.roofs.iter().zip(&roofs) {
                set_material(&mut scene, id, roof.clone());
            }
        }
        for &(id, start) in &self.screens {
            if let Some(mat) = scene[id].material_mut() {
                mat.color = Rgb::WHITE * (1.3 * smoothstep(start + 1.0, start + 2.2, t));
            }
        }
        scene[self.park_root].scale.y =
            ease(self.park_start, self.park_start + RISE_TIME, t).max(0.002);

        // The red aviation lights blink together, once a second.
        let blink = if t.fract() < 0.4 { 1.0 } else { 0.06 };
        let mut lights: Vec<Particle> = self
            .roof_light_list
            .iter()
            .map(|&(l, start)| {
                let rise = ease(start, start + RISE_TIME, t);
                let at = l.at * vec3(1.0, rise, 1.0);
                if l.aviation {
                    Particle::new(at, 0.6, Rgb::hex(0xff2218) * (8.0 * blink * rise))
                } else {
                    Particle::new(at, 0.35, Rgb::hex(0x7dffc0) * (4.0 * rise))
                }
            })
            .collect();

        // The street lights come on while the traces fade.
        let lamps = smoothstep(1.0, 2.6, t);
        if let Some(mat) = scene[self.lamp_light].material_mut() {
            mat.color = Rgb::hex(0x9fc0ff) * (0.3 * lamps);
        }
        if let Some(mat) = scene[self.markings].material_mut() {
            mat.color = Rgb::hex(0x8a92a2) * (0.3 * smoothstep(1.6, 3.0, t));
        }
        for id in [self.lamp_heads, self.small_lights] {
            if let Object::Particles(p) = &mut scene[id].object {
                for item in &mut p.items {
                    item.opacity = lamps;
                }
            }
        }
        // The point lights of the boulevard lamps ahead of the camera. They fade in far
        // ahead and out behind, so they can move from lamp to lamp.
        let z = camera.position.z;
        let first = self.avenue_lamps.partition_point(|p| p.z < z - 40.0);
        for (k, &id) in self.avenue_lights.iter().enumerate() {
            let Some(&lamp) = self.avenue_lamps.get(first + k) else {
                scene[id].visible = false;
                continue;
            };
            let fade = smoothstep(z - 40.0, z - 10.0, lamp.z)
                * (1.0 - smoothstep(z + 200.0, z + 300.0, lamp.z));
            scene[id].position = lamp - Vec3::Y * 0.6;
            if let Some(light) = scene[id].light_mut() {
                *light.intensity_mut() = 260.0 * fade * smoothstep(3.0, 4.5, t);
            }
        }

        // The traffic.
        let traffic = smoothstep(1.4, 3.0, t);
        let (car_lights, bodies) =
            traffic::cars(&self.lanes, t, camera.position, camera.forward(), traffic);
        set_particles(&mut scene, self.cars, car_lights);
        scene[self.car_bodies].visible = !bodies.is_empty();
        if let Some(mesh) = scene[self.car_bodies].mesh_mut() {
            mesh.instances = Some(Arc::new(Instances::new(bodies)));
        }

        // The traffic lights at the empty crossings.
        let signal = smoothstep(1.8, 3.2, t);
        let colors: Vec<Rgb> = self
            .signals
            .iter()
            .map(|s| traffic::signal_color(s, t) * signal)
            .collect();
        set_particles(
            &mut scene,
            self.signal_heads,
            self.signals
                .iter()
                .zip(&colors)
                .map(|(s, &c)| Particle::new(s.head, if s.key { 0.45 } else { 0.32 }, c * 6.0))
                .collect(),
        );
        if let Some(mesh) = scene[self.signal_pools].mesh_mut() {
            mesh.instances = Some(Arc::new(
                Instances::new(self.signals.iter().map(|s| {
                    let size = if s.key { 22.0 } else { 11.0 };
                    Mat4::from_scale_rotation_translation(
                        vec3(size, 1.0, size),
                        Quat::IDENTITY,
                        vec3(s.pool.x, 0.25, s.pool.y),
                    )
                }))
                .colors(colors.iter().map(|&c| c * 0.3)),
            ));
        }
        // Each light costs time on every pixel, so the lights behind the camera go out.
        let ahead = |p: Vec3| (p - camera.position).dot(camera.forward()) > -40.0;
        for &(id, i) in &self.signal_lights {
            scene[id].visible = ahead(scene[id].position);
            if let Some(Light::Point(p)) = scene[id].light_mut() {
                p.color = colors.get(i).copied().unwrap_or(Rgb::BLACK);
                p.intensity = 900.0;
            }
        }

        // The cranes lift beams and place them on the frames.
        for s in &self.sites {
            let rise = ease(s.start, s.start + RISE_TIME, t);
            scene[s.root].scale.y = rise.max(0.002);
            lights.extend(crane(&mut scene, s, t, rise, blink));
            if !ahead(s.base) {
                scene[s.lamp].visible = false;
            }
        }
        set_particles(&mut scene, self.roof_lights, lights);

        // The stars stay around the camera.
        scene[self.stars].position = camera.position;

        // Haze: none on the chip. In the city a dense mist lies between the towers, and the
        // tops stand clear.
        let air = smoothstep(0.6, 3.4, t);
        scene.atmosphere = Some(Atmosphere {
            density: 0.0055 * air,
            color: Rgb::WHITE,
            base: 10.0,
            falloff: 0.02,
            anisotropy: 0.5,
            ambient: Rgb::hex(0x4a6a9a) * 0.32,
            noise: 0.65,
            noise_size: 55.0,
            offset: vec3(t * 6.0, 0.0, t * 2.5),
            distance: 3000.0,
            steps: 18,
        });
        (scene, camera)
    }
}

fn set_material(scene: &mut Scene, id: NodeId, material: Material) {
    if let Some(m) = scene[id].material_mut() {
        *m = material;
    }
}

fn set_particles(scene: &mut Scene, id: NodeId, items: Vec<Particle>) {
    if let Object::Particles(p) = &mut scene[id].object {
        p.items = items;
    }
}

/// Poses a crane at `t` and returns its lights: a string of small lamps along the jib and
/// red lights at the ends. `blink` is the brightness of the red lights. A cycle of 18 s:
/// the hook takes a beam at the foot of the frame, lifts it, turns, sets it on the top
/// floor and turns back.
fn crane(scene: &mut Scene, s: &SiteNodes, t: f32, rise: f32, blink: f32) -> Vec<Particle> {
    let cycle = 18.0;
    let c = (t + hash(s.seed) * cycle).rem_euclid(cycle);
    // The jib turns from the pick up angle to the frame and back.
    let side = if hash(s.seed + 2) < 0.5 { 1.0 } else { -1.0 };
    let pick = s.place + deg(75.0) * side;
    let turn = ease(5.0, 9.5, c) - ease(13.5, 17.5, c);
    let angle = pick + (s.place - pick) * turn;
    scene[s.slew].rotation = Quat::from_rotation_y(angle);
    // The trolley runs between the pick up point and the frame center.
    let out = 30.0 + (s.radius - 30.0) * turn;
    scene[s.trolley].position = vec3(out.min(s.reach - 3.0), 0.0, 0.0);
    // The hook goes down to the ground, up, and down onto the frame.
    let ground = s.height - 4.0;
    let high = 10.0;
    let set = s.height - s.top - 4.0;
    let drop = if c < 1.5 {
        ground
    } else if c < 9.0 {
        ground + (high - ground) * ease(1.5, 6.5, c)
    } else if c < 12.0 {
        high + (set - high) * ease(9.0, 11.5, c)
    } else {
        set + (high - set) * ease(12.5, 14.5, c)
    };
    scene[s.cable].scale = vec3(1.0, drop, 1.0);
    scene[s.hook].position = vec3(0.0, -drop, 0.0);
    // From 11.8 s of the cycle the beam stays on the frame, until the next one comes.
    let carried = !(11.8..17.5).contains(&c);
    if let Some(&load) = scene[s.hook].children().first() {
        scene[load].visible = carried;
    }
    scene[s.placed].visible = !carried;
    if let Some(light) = scene[s.lamp].light_mut() {
        *light.intensity_mut() = 14000.0 * rise;
    }
    let rotation = Quat::from_rotation_y(angle);
    let pivot = s.base + Vec3::Y * (s.height * rise);
    let at = |x: f32, y: f32| pivot + rotation * vec3(x, y * rise, 0.0);
    let red = Rgb::hex(0xff2218) * (8.0 * blink * rise);
    let mut lights: Vec<Particle> = (0..((s.reach + 18.0) / 4.0) as u32)
        .map(|k| {
            Particle::new(
                at(-18.0 + 4.0 * k as f32, 2.6),
                0.22,
                Rgb::hex(0xdfeaff) * (3.0 * rise),
            )
        })
        .collect();
    lights.push(Particle::new(at(s.reach, 1.0), 0.6, red));
    lights.push(Particle::new(at(-18.0, 3.6), 0.6, red));
    lights
}

/// The camera at `t` seconds.
pub fn camera(t: f32) -> Camera {
    let end = END_DIRECTION.normalize();
    let end_yaw = end.x.atan2(end.z);
    let end_pitch = end.y.asin();
    // A slow sway across the boulevard, then a turn toward the crane on the right that
    // the camera passes at 10 s. Then the turn to the left for the end.
    let sway = smoothstep(4.4, 6.0, t) * (1.0 - smoothstep(7.4, 8.6, t));
    let crane = smoothstep(7.8, 9.4, t) - smoothstep(10.0, 11.6, t);
    let yaw = end_yaw * smoothstep(9.6, 12.9, t) + deg(4.0) * (0.7 * (t - 4.4)).cos() * sway
        - deg(9.0) * crane;
    let flight =
        deg(-90.0) + deg(74.0) * smoothstep(1.1, 4.6, t) + deg(10.0) * smoothstep(4.6, 7.6, t)
            - deg(2.0) * smoothstep(7.6, 10.2, t);
    // The tilt up ends exactly on the direction of shot 5.
    let tilt = ease(10.2, DURATION, t);
    let pitch = flight * (1.0 - tilt) + end_pitch * tilt;
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
    let mm = 35.0 - 7.0 * smoothstep(2.0, 4.6, t) + 7.0 * smoothstep(10.4, 12.8, t);
    let mut camera = look::lens(mm)
        .position(position)
        .look_at(position + forward * 1000.0)
        .clip(1.0, 6000.0);
    camera.up = up;
    // The focus: the ground on the dive, the towers ahead in the flight, the stars at the
    // end.
    let ground = position.y / (-pitch.sin()).max(0.2);
    let focus = ground
        + (260.0 - ground) * smoothstep(3.0, 4.6, t)
        + (400.0 - 260.0) * smoothstep(10.5, 12.5, t);
    let blur = 2.0 + 1.5 * smoothstep(3.0, 4.6, t) - 2.5 * smoothstep(10.5, 12.5, t);
    let shake = 0.0012 * smoothstep(3.5, 5.0, t) * (1.0 - smoothstep(10.0, 12.4, t));
    camera.focus(focus, blur).shake(t, shake, 41)
}

/// The position of the camera at `t` seconds: the dive, then the flight along the
/// boulevard.
fn flight_position(t: f32) -> Vec3 {
    let dive_end = 4.4;
    let height = if t < dive_end {
        let u = t / dive_end;
        let k = 1.0 - (1.0 - u) * (1.0 - u);
        (START_HEIGHT.ln() + (140.0f32.ln() - START_HEIGHT.ln()) * k).exp()
    } else {
        140.0
            + (78.0 - 140.0) * smoothstep(dive_end, 7.6, t)
            + (96.0 - 78.0) * smoothstep(7.6, 10.4, t)
            + (260.0 - 96.0) * smoothstep(10.2, DURATION, t)
    };
    // The speed along the ground, integrated in small steps.
    let speed = |s: f32| 46.0 * smoothstep(1.3, 4.4, s) - 28.0 * smoothstep(10.2, DURATION, s);
    let end = END_DIRECTION.normalize();
    let end_yaw = end.x.atan2(end.z);
    let step = 1.0f32 / 240.0;
    let mut ground = Vec2::ZERO;
    let mut s = 0.0;
    while s < t {
        let dt = step.min(t - s);
        let heading = 0.5 * end_yaw * smoothstep(9.6, 12.9, s + dt * 0.5);
        ground += vec2(heading.sin(), heading.cos()) * speed(s + dt * 0.5) * dt;
        s += dt;
    }
    // The sway and the move toward the crane, see `camera`.
    let sway = smoothstep(4.4, 6.0, t) * (1.0 - smoothstep(7.4, 8.6, t));
    let crane = smoothstep(8.0, 9.6, t) - smoothstep(10.2, 11.8, t);
    let side = 5.0 * (0.7 * (t - 4.4)).sin() * sway - 20.0 * crane;
    vec3(ground.x + side, height, ground.y)
}

/// The chip of shot 3: the die, the traces, the components and the pins. It is 0.04 m
/// square and 0.004 m thick, centered at the origin, with the top face up (+Y).
///
/// The top face has the plan of the city at [`CHIP_SCALE`]: the gold traces are the
/// arterials, the copper traces the small streets, the components the buildings, the black
/// slot the canal. The pins on the left and the right are rows of long buildings. Its +Z
/// direction is the top of the first picture of shot 4. That picture looks straight down
/// from 0.0418 m above the top face (y = 0.0433 m) through a 35 mm lens, so the chip
/// with its pins fills 94 % of the picture width.
///
/// One mesh with one material: a small texture atlas gives each part its color, metal and
/// glow.
pub fn chip() -> Node {
    let plan = Plan::new();
    let s = CHIP_SCALE;
    let half = CHIP_HALF * s;
    let top = 0.0015;
    let mut buf = MeshBuf::default();
    // A swatch of the atlas, see `chip_atlas`.
    let swatch = |k: u32| vec2(k as f32 * 0.125 + 0.0625, 0.5);
    // Adds the vertices of `mesh` with the UV of a swatch.
    let add = |buf: &mut MeshBuf, mesh: MeshBuf, k: u32| {
        let start = buf.len();
        buf.append(mesh);
        set_uvs(buf, start, swatch(k));
    };

    // The die body.
    let mut body = MeshBuf::default();
    body.block(vec3(-half, -top, -half), vec3(half, top, half));
    add(&mut buf, body, 0);
    // The traces and their pads, the same as on the streets of the city.
    let [gold, copper] = build::traces(&plan, s, top + 0.000_01);
    add(&mut buf, gold, 1);
    add(&mut buf, copper, 2);
    // The components: one box for each tier of the buildings and for each frame of the
    // construction sites.
    let boxes = plan
        .buildings
        .iter()
        .filter(|b| !b.pin)
        .flat_map(|b| {
            b.tiers
                .iter()
                .enumerate()
                .map(move |(i, tier)| (b.center, tier.size, i))
        })
        .chain(plan.sites.iter().map(|site| (site.center, site.size, 0)));
    for (center, size, tier) in boxes {
        if center.abs().max_element() > CHIP_HALF - 20.0 {
            continue;
        }
        let min = (center - size * 0.5) * s;
        let max = (center + size * 0.5) * s;
        let h = top + 0.000_06 + 0.000_04 * tier as f32;
        let start = buf.len();
        buf.block(vec3(min.x, top, min.y), vec3(max.x, h, max.y));
        set_uvs(&mut buf, start, swatch(4));
        // The top face is the fifth face of a box. It shows the component picture.
        let top_face = start + 16;
        for (k, uv) in [
            vec2(0.0, 1.0),
            vec2(1.0, 1.0),
            vec2(1.0, 0.0),
            vec2(0.0, 0.0),
        ]
        .into_iter()
        .enumerate()
        {
            buf.set_uv(top_face + k, vec2(0.755 + uv.x * 0.24, 0.02 + uv.y * 0.96));
        }
    }

    // The gull wing pins on the four sides, at the places of the long buildings.
    let mut pins = MeshBuf::default();
    let (pitch, reach, width) = (PIN_PITCH * s, PIN_REACH * s, 8.0 * BAY * s);
    for side in 0..4 {
        let rotation = Quat::from_rotation_y(side as f32 * std::f32::consts::FRAC_PI_2);
        for p in 0..PINS {
            let across = (p as f32 - (PINS - 1) as f32 * 0.5) * pitch;
            let at = |out: f32, y: f32| rotation * vec3(half + out, y, across);
            pins.bar(at(-0.0002, 0.0), at(reach * 0.27, 0.0), width);
            pins.bar(at(reach * 0.27, 0.0), at(reach * 0.64, -0.0018), width);
            pins.bar(at(reach * 0.64, -0.0018), at(reach, -0.0018), width);
        }
    }
    add(&mut buf, pins, 3);

    let (color, glow, metal_rough) = chip_atlas();
    let mut material = Material::standard(Rgb::WHITE)
        .map(color)
        .emissive_map(glow)
        .emissive(Rgb::WHITE, 1.0)
        .roughness(1.0)
        .metalness(1.0);
    material.metal_rough_map = Some(metal_rough);
    Node::mesh(buf.geometry(), material).named("chip")
}

fn set_uvs(buf: &mut MeshBuf, start: usize, uv: Vec2) {
    for i in start..buf.len() {
        buf.set_uv(i, uv);
    }
}

/// The atlas of the chip: the color, the glow and the metal roughness texture. Swatches of
/// 32 texels across: 0 die, 1 gold, 2 copper, 3 silver, 4 component side, 5 polished
/// slot. The right quarter holds the top of a component.
fn chip_atlas() -> (Arc<Texture>, Arc<Texture>, Arc<Texture>) {
    let (w, h) = (256, 64);
    let swatch = |u: f32| ((u * 8.0) as usize).min(5);
    let rim = |u: f32, v: f32| {
        let (x, y) = ((u - 0.75) * 4.0, v);
        x.min(y).min(1.0 - x).min(1.0 - y) < 0.07
    };
    let srgb = |c: u32| [(c >> 16) as u8, (c >> 8) as u8, c as u8, 255];
    let color = Texture::from_fn_data(w, h, true, |u, v| {
        if u >= 0.75 {
            // The roof picture times the component color.
            let c = if rim(u, v) {
                Rgb::srgb8(138, 142, 150)
            } else {
                Rgb::srgb8(46, 48, 53)
            };
            let out = Rgb::from(c.0 * Rgb::hex(COMPONENT).0).to_color();
            return [out.r, out.g, out.b, 255];
        }
        srgb([DIE, GOLD, COPPER, SILVER, 0x18191c, 0x020304][swatch(u)])
    });
    let glow = Texture::from_fn_data(w, h, true, |u, _| {
        let k = swatch(u);
        if (k == 1 || k == 2) && u < 0.75 {
            let c = (Rgb::hex(TRACE_GLOW) * if k == 1 { 0.1 } else { 0.07 }).to_color();
            [c.r, c.g, c.b, 255]
        } else {
            [0, 0, 0, 255]
        }
    });
    // Roughness in green, metalness in blue.
    let metal_rough = Texture::from_fn_data(w, h, false, |u, _| {
        if u >= 0.75 {
            return [255, 71, 153, 255];
        }
        let [rough, metal] = [
            [0.3, 0.0],
            [0.26, 1.0],
            [0.3, 1.0],
            [0.24, 1.0],
            [0.5, 0.0],
            [0.06, 0.0],
        ][swatch(u)];
        [255, (rough * 255.0) as u8, (metal * 255.0) as u8, 255]
    });
    (Arc::new(color), Arc::new(glow), Arc::new(metal_rough))
}
