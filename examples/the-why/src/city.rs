//! Shot 4, "The perfect city" (29.0 to 42.0 s of the film, 13 s): Manhattan at night.
//!
//! The camera of shot 3 dives straight down into a chip. This shot continues the dive. The
//! chip is Midtown Manhattan: its traces are the streets, the avenues and the diagonal of
//! Broadway, its components are the roofs of the real buildings. The buildings rise, the
//! Empire State Building first, the windows light up, and the camera levels out beside the
//! crown of the Empire State Building. It turns up Broadway and glides toward Times Square,
//! where the screens play for nobody, and tilts up into the stars, where shot 5 starts.
//!
//! The buildings, the streets, the shores and the parks are real: `fetch-nyc.sh` downloads
//! the NYC 3D Building Model and the open data of the city, and `nyc.py` makes
//! `dynamic_media/nyc/manhattan.bin` from them. The world is in meters with the origin at the Empire State Building, +Z
//! uptown along the avenues and +X to the west. The chip of shot 3 ([`chip`]) has the same
//! plan at 1/45000 scale.
//!
//! Seconds of the shot:
//! - 0.0: straight down on the chip from 1880 m above Broadway at 29th Street. The image
//!   top is +Z (uptown). The chip with its pins fills 94 % of the picture width.
//! - 0.55 to 4.5: the buildings rise from the dive outward, the Empire State Building
//!   first. The gold of the traces goes dark and they glow with cold light, then the
//!   street lamps and the traffic come on.
//! - 1.4 to 4.0: the camera turns toward the Empire State Building and levels out at its
//!   crown, 350 m above the street and 300 m from it.
//! - 4.8 to 8.0: it turns up Broadway, toward Times Square, comes down to 170 m and
//!   speeds up from 80 to 110 m/s.
//! - 8.0 to 10.2: the glide. The video screens of Times Square play at the end of
//!   Broadway. The lights at Broadway and 42nd Street turn red at 7.5 s and green for the
//!   empty street at 8 s.
//! - 10.2 to 13.0: the camera climbs and tilts up into the stars.

#[path = "city/build.rs"]
mod build;
#[path = "city/data.rs"]
pub(crate) mod data;
#[path = "city/flight.rs"]
mod flight;
#[path = "city/outer.rs"]
mod outer;
#[path = "city/textures.rs"]
pub(crate) mod textures;
#[path = "city/traffic.rs"]
mod traffic;

use std::collections::HashSet;
use std::path::Path;
use std::sync::Arc;

use fframes::Svgr;
use fframes_3d::math::{hash, smoothstep};
use fframes_3d::*;

use crate::assets::{Assets, Pbr};
use crate::look;
use build::{BAY, CraneParts, FLOOR, MeshBuf, RISE_TIME, RoofLight, STYLES};
use data::Manhattan;
use flight::near_flight;
pub use flight::{
    camera, exposure as flight_exposure, motion as flight_motion, samples as flight_samples,
};
use traffic::{Lane, Signal};

/// The direction of the camera at the end of the shot. Shot 5 starts with it.
pub const END_DIRECTION: Vec3 = Vec3::new(0.145, 0.906, 0.397);
/// The height of the camera above the street at the first frame, in meters. The chip
/// with its pins (1818 m) fills 94 % of the picture width.
pub const START_HEIGHT: f32 = 1880.0;
/// The length of the shot in seconds.
pub const DURATION: f32 = 13.0;
/// The size of the chip relative to the city.
pub const CHIP_SCALE: f32 = 1.0 / 45000.0;

/// The point on Broadway at 29th Street straight below the first frame, in meters. The
/// center of the chip.
const DIVE: Vec2 = Vec2::new(96.0, -330.0);
/// Half the side of the part of the city that is the top face of the chip, in meters.
const CHIP_HALF: f32 = 810.0;
/// The pins of the chip: their distance, their length out of the chip, and their number
/// on each side.
const PIN_PITCH: f32 = 36.0;
const PIN_REACH: f32 = 99.0;
const PINS: usize = 44;
/// Seventh Avenue at 46th Street, ahead of the glide through Times Square. Its light for
/// the cross street throws its color on the street.
const KEY_CROSSING: Vec2 = Vec2::new(504.0, 1010.0);
/// Times Square, the area where the video screens hang.
const TIMES_SQUARE: (Vec2, Vec2) = (Vec2::new(400.0, 680.0), Vec2::new(640.0, 1180.0));
/// The steam vents in Seventh Avenue, under the flight.
const VENTS: [Vec2; 3] = [
    Vec2::new(505.0, 560.0),
    Vec2::new(504.0, 812.0),
    Vec2::new(502.0, 965.0),
];
/// The puffs of steam over one vent.
const PUFFS: u32 = 60;
/// The most video screens in Times Square.
const SCREEN_COUNT: usize = 20;

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
/// The height of the water of the rivers and the harbor.
const WATER: f32 = -1.5;
/// The glow of the city on the night sky and in the far haze, linear: warm grey.
const SKY_GLOW: Rgb = Rgb::linear(0.055, 0.047, 0.042);
/// The point lights that light the mist over Broadway ahead of the camera.
const AVENUE_LIGHTS: usize = 2;

/// The nodes of the buildings that rise together.
struct RiseNodes {
    root: NodeId,
    /// The facades with the brightness of their windows: the five styles, then the five
    /// sharper styles of the buildings along the flight through Times Square.
    walls: Vec<(NodeId, f32)>,
    /// The roofs inside the chip square, then outside it.
    roofs: [NodeId; 2],
    crowns: NodeId,
    /// The top of the Empire State Building and the crown of the Chrysler Building.
    landmarks: [NodeId; 2],
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
    /// The work lights on the frame.
    work: NodeId,
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
    streets: NodeId,
    /// The gold and the copper traces of the chip.
    traces: [NodeId; 2],
    /// The pins and the seal ring of the chip on the city.
    chip_rim: [NodeId; 2],
    lamp_light: NodeId,
    storefronts: NodeId,
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
    /// The point light of the key crossing, in the color of its signal for the street,
    /// with the index of that signal.
    signal_light: (NodeId, usize),
    /// The lamps of Broadway, by distance along it, and the point lights that move along
    /// them with the camera.
    avenue_lamps: Vec<(f32, Vec3)>,
    avenue_lights: [NodeId; AVENUE_LIGHTS],
    /// The video screens, with the start time of the rise of their tower.
    screens: Vec<(NodeId, f32)>,
    screen_ids: Vec<NodeId>,
    /// The LED edges of the screens, with the start time of the rise of their tower.
    screen_edges: Vec<(NodeId, f32)>,
    sites: Vec<SiteNodes>,
    stars: NodeId,
    /// The glow of the city on the sky at the horizon. It moves with the camera.
    sky: NodeId,
    /// The light carpet of the far city.
    carpet: NodeId,
    /// The water of the rivers and the harbor.
    water: NodeId,
    /// The steam of the street vents.
    steam: NodeId,
    /// The screens left on in dark rooms along Times Square, which flicker.
    monitors: NodeId,

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
    /// The roofs on the chip: inside the chip square, then outside it.
    chip_roofs: [Material; 2],
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

/// Reads the Manhattan of `nyc.py`.
fn manhattan() -> Manhattan {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("dynamic_media/nyc/manhattan.bin");
    let bytes = std::fs::read(&path)
        .unwrap_or_else(|e| panic!("{}: {e}. Run examples/the-why/fetch-nyc.sh", path.display()));
    Manhattan::load(&bytes).unwrap_or_else(|e| panic!("{e}"))
}

impl CitySet {
    pub fn new(assets: &Assets) -> Self {
        let mut city = manhattan();
        city.face_walls_out();
        let mut scene =
            Scene::new()
                .environment(environment(true))
                .background(Background::Gradient {
                    top: Rgb::hex(0x061128),
                    bottom: Rgb::hex(0x091733),
                });

        // The chip and the city states of the materials that change.
        let roof_texture = textures::roof();
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
                Material::standard(COMPONENT)
                    .roughness(0.28)
                    .metalness(0.6)
                    .map(roof_texture.clone()),
                Material::standard(0x000000).env_intensity(0.0),
            ],
            roof: Material::standard(0x7c7f86)
                .roughness(0.85)
                .map(roof_texture)
                .env_intensity(0.2),
        };

        // The water of the rivers and the harbor mirrors the city.
        scene.reflector = Some(Reflector::new(vec3(0.0, WATER, 0.0), Vec3::Y).scale(0.2));
        let water = scene.add(
            Node::mesh(
                // So wide that it reaches the horizon: no edge between the water and the sky.
                Geometry::floor_segments(330_000.0, 330_000.0, 33, 33).translate(0.0, WATER, 0.0),
                Material::standard(0x020409)
                    .roughness(0.18)
                    .reflection(0.85)
                    .env_intensity(0.6),
            )
            .shadows(false, false),
        );
        let ground = scene.add(
            Node::mesh(build::area(&city.land, 0.0), materials.die.clone()).shadows(false, false),
        );
        scene.add(Node::mesh(build::new_jersey(), materials.ground.clone()).shadows(false, false));
        // The far city: the light of its streets and windows on the land.
        let land_map = outer::LandMap::new(&city);
        let mut carpet_light = Material::unlit(Rgb::BLACK)
            .map(outer::carpet(&city, &land_map))
            .additive();
        carpet_light.fog = false;
        let carpet = scene.add(
            Node::mesh(outer::carpet_ground(&city, 0.4).geometry(), carpet_light)
                .shadows(false, false),
        );
        let crossings = build::crossings(&city.streets);
        let streets = scene.add(
            Node::mesh(build::streets(&city), materials.street_die.clone()).shadows(false, false),
        );
        let [gold, copper] =
            build::traces(&city, &crossings, (DIVE, CHIP_HALF), Vec2::ZERO, 1.0, 0.2);
        let traces = [
            scene.add(Node::mesh(gold.geometry(), materials.gold.clone()).shadows(false, false)),
            scene
                .add(Node::mesh(copper.geometry(), materials.copper.clone()).shadows(false, false)),
        ];
        let (pins, ring) = build::pins(DIVE, CHIP_HALF, PIN_PITCH, PIN_REACH, PINS);
        let chip_rim = [
            scene.add(Node::mesh(pins, Material::metal(SILVER, 0.24)).shadows(false, false)),
            scene.add(Node::mesh(ring, materials.gold.clone()).shadows(false, false)),
        ];
        let parks = scene.add(
            Node::mesh(build::area(&city.parks, 0.05), materials.park_die.clone())
                .shadows(false, false),
        );
        let lamp_light = scene.add(
            Node::mesh(
                build::lamp_light(&city, near_flight),
                Material::unlit(Rgb::BLACK)
                    .map(textures::lamp_pools())
                    .additive(),
            )
            .shadows(false, false),
        );
        // The shop windows and lobbies at street level. They come on with the street lamps.
        let storefronts = scene.add(
            Node::mesh(
                build::storefronts(&city, near_flight).geometry(),
                Material::unlit(Rgb::BLACK)
                    .map(textures::storefront())
                    .additive(),
            )
            .shadows(false, false),
        );
        let markings = scene.add(
            Node::mesh(
                build::markings(&city, &crossings, near_flight),
                Material::unlit(Rgb::BLACK).additive(),
            )
            .shadows(false, false),
        );

        // The street lamps.
        let (poles, heads) = build::lamps(&city, &crossings, near_flight);
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
        // Every third lamp under the flight can carry a point light. They are sorted by the
        // distance along the track.
        let mut avenue_lamps: Vec<(f32, Vec3)> = heads
            .iter()
            .filter_map(|&h| {
                let (off, along) = flight::track().project(vec2(h.x, h.z));
                (off < 30.0).then_some((along, h))
            })
            .collect();
        avenue_lamps.sort_by(|a, b| a.0.total_cmp(&b.0));
        let avenue_lamps: Vec<(f32, Vec3)> = avenue_lamps.into_iter().step_by(3).collect();
        let avenue_lights = [(); AVENUE_LIGHTS]
            .map(|()| scene.add(Node::light(Light::point(Rgb::hex(0xdbe8ff), 0.0, 70.0))));

        // The trees of the parks. They rise with the buildings around them.
        let park_root = scene.add(Node::group().scaled3(1.0, 0.002, 1.0));
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
                Instances::new(build::trees(&city, DIVE, 2500.0)),
            )
            .shadows(false, false),
        );
        // The bridges with the lights on their decks and cables, and the lamps of the park
        // paths.
        let (bridge_steel, bridge_lights) = outer::bridges(&city, &land_map);
        scene.add(
            Node::mesh(
                bridge_steel.geometry(),
                Material::standard(0x4a5058)
                    .roughness(0.6)
                    .env_intensity(0.6)
                    .double_sided(),
            )
            .shadows(false, false),
        );
        let small_lights = scene.add(Node::particles(
            bridge_lights
                .iter()
                .map(|&p| Particle::new(p, 0.9, Rgb::hex(0xfff0d8) * 6.0))
                .chain(
                    outer::park_lamps(&city, 3500.0)
                        .iter()
                        .map(|&p| Particle::new(p, 0.35, Rgb::hex(0xffe6c0) * 4.0)),
                )
                .collect(),
            Blending::Additive,
        ));

        // The buildings, in groups that rise together. Two low buildings near Broadway make
        // room for construction sites. Five walls of Times Square carry the screens.
        let sites_at = construction_lots(&city);
        let skip: HashSet<usize> = sites_at.iter().map(|s| s.0).collect();
        let screen_list = place_screens(&city);
        // The facades of the five styles, far and near.
        let facades: Vec<textures::Facade> = std::thread::scope(|s| {
            let handles: Vec<_> = [textures::FAR, textures::NEAR, textures::DISTANT]
                .into_iter()
                .flat_map(|layout| {
                    STYLES
                        .iter()
                        .enumerate()
                        .map(move |(i, &style)| (i, style, layout))
                })
                .map(|(i, style, layout)| {
                    s.spawn(move || textures::Facade::new(style, 31 + i as u32 * 97, layout))
                })
                .collect();
            handles.into_iter().filter_map(|h| h.join().ok()).collect()
        });
        let mullion_steel = Material::metal(0x8c949e, 0.32).env_intensity(1.1);
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
        let tanks = Arc::new(build::water_tower());
        // The wood of the water towers catches the glow of the city, so their shapes read
        // against the dark roofs.
        let tank_wood = Material::standard(0x7a6450)
            .roughness(0.8)
            .env_intensity(1.0)
            .emissive(0x2a3448, 0.6);
        let mut groups = Vec::new();
        let mut roof_light_list = Vec::new();
        let mut screens = Vec::new();
        let mut screen_edges = Vec::new();
        // The facades: sharp along the glide through Times Square, sparse far from the
        // flight.
        let lod = |b: &data::Building| {
            let off = flight::track().project(b.center).0;
            if (450.0..1750.0).contains(&b.center.y) && off < 110.0 {
                build::Lod::Near
            } else if off > 380.0 {
                build::Lod::Distant
            } else {
                build::Lod::Mid
            }
        };
        // The walls that face the flight from near get steel mullions.
        let facing_flight = |center: Vec3, normal: Vec3| {
            let ground = vec2(center.x, center.z);
            let (off, along) = flight::track().project(ground);
            let toward = flight::track().at(along).0 - ground;
            off < 70.0 && center.y < 170.0 && vec2(normal.x, normal.z).dot(toward) > 0.0
        };
        for g in build::rise_groups(
            &city,
            DIVE,
            CHIP_HALF,
            &skip,
            &screen_list,
            lod,
            facing_flight,
        ) {
            let root = scene.add(Node::group().scaled3(1.0, 0.002, 1.0));
            let lights = [1.5, 1.4, 1.3, 1.3, 1.1];
            let mut walls = Vec::new();
            for ((mesh, facade), light) in g
                .walls
                .into_iter()
                .chain(g.near_walls)
                .chain(g.distant_walls)
                .zip(&facades)
                .zip(lights.iter().cycle())
            {
                if mesh.len() > 0 {
                    let id =
                        scene.add_to(root, Node::mesh(mesh.geometry(), facade_material(facade)));
                    walls.push((id, *light));
                }
            }
            if g.mullions.len() > 0 {
                scene.add_to(
                    root,
                    Node::mesh(g.mullions.geometry(), mullion_steel.clone()),
                );
            }
            let mut roofs = [root; 2];
            for ((id, mesh), material) in roofs.iter_mut().zip(g.roofs).zip(&materials.chip_roofs) {
                *id = scene.add_to(
                    root,
                    Node::mesh(mesh.geometry(), material.clone().double_sided()),
                );
            }
            scene.add_to(root, Node::mesh(g.details.geometry(), details.clone()));
            let crowns = scene.add_to(
                root,
                Node::mesh(g.crowns.geometry(), Material::glow(COLD, 0.0)).shadows(false, false),
            );
            let landmarks = [(g.floodlit, true), (g.chrome, false)].map(|(mesh, additive)| {
                let mut material = Material::unlit(Rgb::BLACK).map(facades[3].color.clone());
                if additive {
                    material = material.additive();
                }
                scene.add_to(
                    root,
                    Node::mesh(mesh.geometry(), material).shadows(false, false),
                )
            });
            if !g.water_towers.is_empty() {
                scene.add_to(
                    root,
                    Node::instanced(
                        tanks.clone(),
                        tank_wood.clone(),
                        Instances::new(g.water_towers),
                    ),
                );
            }
            if g.screen_edges.len() > 0 {
                let id = scene.add_to(
                    root,
                    Node::mesh(
                        g.screen_edges.geometry(),
                        Material::unlit(Rgb::BLACK).additive(),
                    )
                    .shadows(false, false),
                );
                screen_edges.push((id, g.start));
            }
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
                landmarks,
                start: g.start,
            });
        }
        // The buildings outside Midtown and in New Jersey rise with the last group.
        if let Some(last) = groups.last_mut() {
            let (walls, roofs) = outer::blocks(&city, &STYLES);
            for (k, mesh) in walls.into_iter().enumerate() {
                let id = scene.add_to(
                    last.root,
                    Node::mesh(mesh.geometry(), facade_material(&facades[10 + k]))
                        .shadows(false, false)
                        .named("outer"),
                );
                last.walls.push((id, 1.3));
            }
            scene.add_to(
                last.root,
                Node::mesh(
                    roofs.geometry(),
                    Material::standard(0x1c1e22)
                        .roughness(0.9)
                        .env_intensity(0.3)
                        .double_sided(),
                )
                .shadows(false, false)
                .named("outer"),
            );
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
        for (k, &(_, min, max)) in sites_at.iter().enumerate() {
            let seed = 500 + k as u32 * 17;
            let center = (min + max) * 0.5;
            let start = build::group_start(build::group_of(build::rise_start(center - DIVE, seed)));
            let floors = 16 + (hash(seed) * 6.0) as u32;
            let crane_height = floors as f32 * FLOOR + 30.0 + hash(seed + 1) * 12.0;
            // The crane stands at the side of the frame that faces the flight.
            let (_, along) = flight::track().project(center);
            let toward = (flight::track().at(along).0 - center).normalize_or(Vec2::X);
            let crane_at = center + toward * ((max - min).max_element() * 0.5 + 8.0);
            let root = scene.add(Node::group().scaled3(1.0, 0.002, 1.0));
            let (frame, slabs, glass, work) = build::site_frame(min, max, floors, seed);
            scene.add_to(root, Node::mesh(frame, steel.clone()));
            scene.add_to(
                root,
                Node::mesh(
                    slabs,
                    surface(&assets.concrete, 0x7a7c80, 0.9).env_intensity(0.3),
                ),
            );
            scene.add_to(root, Node::mesh(glass, facade_material(&facades[0])));
            let work = scene.add_to(
                root,
                Node::particles(
                    work.iter()
                        .map(|&p| Particle::new(p, 0.6, Rgb::hex(0xeef5ff) * 7.0))
                        .collect(),
                    Blending::Additive,
                ),
            );
            let parts = CraneParts::new(crane_height);
            let base = vec3(crane_at.x, 0.0, crane_at.y);
            let crane = scene.add_to(root, Node::group().with_position(base).named("crane"));
            scene.add_to(crane, Node::mesh(parts.mast, crane_paint.clone()));
            let slew = scene.add_to(crane, Node::group().at(0.0, crane_height, 0.0));
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
            let top = floors as f32 * FLOOR;
            let placed = scene.add_to(
                root,
                Node::mesh(parts.load, steel.clone())
                    .at(center.x, top + 3.5, center.y)
                    .hidden(),
            );
            // A flood light on the jib that lights the top of the frame.
            let lamp = scene.add_to(
                slew,
                Node::light(Light::spot(0xe6f0ff, 0.0, deg(28.0), 0.6)).at(16.0, -0.5, 0.0),
            );
            let to_frame = center - crane_at;
            sites.push(SiteNodes {
                root,
                slew,
                trolley,
                cable,
                hook,
                placed,
                lamp,
                work,
                start,
                base,
                place: (-to_frame.y).atan2(to_frame.x),
                radius: to_frame.length(),
                reach: parts.reach,
                height: crane_height,
                top,
                seed,
            });
        }

        // The traffic lights and the traffic.
        let signals = traffic::signals(&city.streets, &city.routes, near_flight, TIMES_SQUARE);
        let lanes = traffic::lanes(&city.routes, near_flight, &signals);
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
        let mut signal_poles = MeshBuf::default();
        for s in &signals {
            let p = s.head;
            signal_poles.block(
                vec3(p.x - 0.14, 0.0, p.z - 0.14),
                vec3(p.x + 0.14, 6.5, p.z + 0.14),
            );
            signal_poles.block(
                vec3(p.x - 0.35, 5.2, p.z - 0.35),
                vec3(p.x + 0.35, 6.6, p.z + 0.35),
            );
        }
        scene.add(
            Node::mesh(
                signal_poles.geometry(),
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
        // The crossing ahead of the glide through Times Square gets a point light in the
        // color of the street.
        let key = signals
            .iter()
            .enumerate()
            .filter(|(_, s)| s.key && !s.avenue)
            .min_by(|a, b| {
                let d = |s: &Signal| s.crossing.distance(KEY_CROSSING);
                d(a.1).total_cmp(&d(b.1))
            })
            .map_or(0, |(i, _)| i);
        let head = signals.get(key).map_or(Vec3::ZERO, |s| s.head);
        let signal_light = (
            scene.add(Node::light(Light::point(Rgb::WHITE, 0.0, 60.0)).with_position(head)),
            key,
        );

        // No moon: on the mirror glass it makes glints brighter than the windows.
        scene.add(Node::light(Light::hemisphere(0x0a1424, 0x2a4470, 0.35)));

        let stars = scene.add(crate::stars::stars());
        let mut glow = Material::unlit(Rgb::WHITE).additive().double_sided();
        glow.fog = false;
        glow.depth_write = false;
        let sky =
            scene.add(Node::mesh(outer::sky_dome(SKY_GLOW).geometry(), glow).shadows(false, false));

        // The steam stacks of the street vents: orange cones with white bands, as the
        // steam company puts them over the vents in the avenues.
        let stacks = Geometry::merge(
            VENTS.map(|v| Geometry::cylinder(0.38, 0.55, 3.4, 12).translate(v.x, 1.7, v.y)),
        );
        let bands = Geometry::merge(VENTS.iter().flat_map(|v| {
            [1.1, 2.3].map(|y| Geometry::cylinder(0.5, 0.53, 0.35, 12).translate(v.x, y, v.y))
        }));
        scene.add(Node::mesh(
            stacks,
            Material::standard(0xff5a12)
                .roughness(0.6)
                .emissive(0xff5a12, 0.15),
        ));
        scene.add(Node::mesh(
            bands,
            Material::standard(0xe8ecf0)
                .roughness(0.5)
                .emissive(0xe8ecf0, 0.1),
        ));
        let steam = scene.add(Node::particles(Vec::new(), Blending::Normal));
        let monitors = scene.add(Node::particles(
            build::window_points(
                &city,
                |p| (450.0..1250.0).contains(&p.y) && flight::track().project(p).0 < 150.0,
                700,
            )
            .into_iter()
            .map(|p| Particle::new(p, 0.3, Rgb::BLACK))
            .collect(),
            Blending::Additive,
        ));

        Self {
            scene,
            groups,
            ground,
            parks,
            park_root,
            streets,
            traces,
            chip_rim,
            lamp_light,
            storefronts,
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
            signal_light,
            avenue_lamps,
            avenue_lights,
            screens,
            screen_ids,
            screen_edges,
            sites,
            stars,
            sky,
            carpet,
            water,
            steam,
            monitors,
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

    /// Motion blur samples at `t`: 3 to 6, more in the frames that move fast. Those frames
    /// also close the shutter sooner, see [`CitySet::moment`].
    pub fn shutter(&self, t: f32) -> usize {
        flight::samples(t)
    }

    /// Nothing over the picture.
    pub fn overlay(&self, _t: f32) -> Svgr<'_> {
        Svgr::empty()
    }

    /// The video screens on the walls of Times Square, up to 20: flat panels in the shape of
    /// the film, tall ones, wide bands, small ones, bent ones and ones around a corner. Each
    /// is an unlit mesh named "screen". Its UVs show the middle of a 4:3 film in the shape
    /// of the screen (V = 0 at the top). To show a video, set the `map` of its material in
    /// the scene from [`CitySet::moment`]. The color of the material is the brightness: it
    /// comes on with the windows of the tower.
    pub fn screens(&self) -> &[NodeId] {
        &self.screen_ids
    }

    /// The scene and the camera at `t` seconds into the shot. For the motion blur samples
    /// of a fast frame, the time moves toward the frame time, so its shutter closes sooner
    /// (see `flight::exposure_time`).
    pub fn moment(&self, t: f32) -> (Scene, Camera) {
        let t = flight::exposure_time(t);
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
        // The pins of the chip leave the picture in the first half second.
        for id in self.chip_rim {
            scene[id].visible = t < 0.8;
        }

        // The buildings rise. Their windows and the crowns of the landmarks light up when
        // they stand.
        let roofs = m
            .chip_roofs
            .each_ref()
            .map(|chip| blend(chip, &m.roof, city));
        for g in &self.groups {
            let rise = ease(g.start, g.start + RISE_TIME, t);
            scene[g.root].scale.y = rise.max(0.002);
            let windows = smoothstep(g.start + 0.3, g.start + RISE_TIME + 0.4, t);
            for &(w, light) in &g.walls {
                if let Some(mat) = scene[w].material_mut() {
                    mat.emissive_intensity = windows * light * textures::WINDOW_LIGHT;
                }
            }
            if let Some(mat) = scene[g.crowns].material_mut() {
                mat.color = Rgb::WHITE * (2.0 * windows);
            }
            for (&id, (color, light)) in g.landmarks.iter().zip([(0x7f9dff, 1.0), (0xe8f0ff, 4.0)])
            {
                if let Some(mat) = scene[id].material_mut() {
                    mat.color = Rgb::hex(color) * (light * windows);
                }
            }
            for (&id, roof) in g.roofs.iter().zip(&roofs) {
                set_material(&mut scene, id, roof.clone().double_sided());
            }
        }
        for &(id, start) in &self.screens {
            if let Some(mat) = scene[id].material_mut() {
                mat.color = Rgb::WHITE * (1.8 * smoothstep(start + 1.0, start + 2.2, t));
            }
        }
        for &(id, start) in &self.screen_edges {
            if let Some(mat) = scene[id].material_mut() {
                mat.color = Rgb::WHITE * (2.4 * smoothstep(start + 1.0, start + 2.2, t));
            }
        }
        scene[self.park_root].scale.y = ease(1.4, 1.4 + RISE_TIME, t).max(0.002);

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
            mat.color = Rgb::hex(0x9fc0ff) * (0.5 * lamps);
        }
        if let Some(mat) = scene[self.storefronts].material_mut() {
            mat.color = Rgb::WHITE * (2.2 * lamps);
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
        // The point lights of the lamps under the flight ahead of the camera. They fade in
        // far ahead and out behind, so they can move from lamp to lamp.
        let here = flight::distance(t);
        let first = self.avenue_lamps.partition_point(|&(s, _)| s < here - 40.0);
        for (k, &id) in self.avenue_lights.iter().enumerate() {
            let Some(&(s, lamp)) = self.avenue_lamps.get(first + k) else {
                scene[id].visible = false;
                continue;
            };
            let fade = smoothstep(here - 40.0, here - 10.0, s)
                * (1.0 - smoothstep(here + 200.0, here + 300.0, s));
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

        // The traffic lights.
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
        let (id, i) = self.signal_light;
        scene[id].visible = ahead(scene[id].position);
        if let Some(Light::Point(p)) = scene[id].light_mut() {
            p.color = colors.get(i).copied().unwrap_or(Rgb::BLACK);
            p.intensity = 260.0;
        }

        // The cranes lift beams and place them on the frames. Only the nearest crane ahead
        // keeps its flood light.
        let nearest = self
            .sites
            .iter()
            .filter(|s| ahead(s.base))
            .map(|s| s.base.distance(camera.position))
            .fold(f32::MAX, f32::min);
        for s in &self.sites {
            let rise = ease(s.start, s.start + RISE_TIME, t);
            scene[s.root].scale.y = rise.max(0.002);
            // The work lights come on when the frame stands, not on the chip.
            scene[s.work].visible = rise > 0.3;
            lights.extend(crane(&mut scene, s, t, rise, blink));
            scene[s.lamp].visible = s.base.distance(camera.position) <= nearest;
        }
        set_particles(&mut scene, self.roof_lights, lights);

        // The stars stay around the camera.
        scene[self.stars].position = camera.position;
        set_particles(&mut scene, self.steam, steam(t));
        // The screens in the dark rooms flicker with their films.
        if let Object::Particles(p) = &mut scene[self.monitors].object {
            let on = smoothstep(3.0, 4.0, t);
            for (i, item) in p.items.iter_mut().enumerate() {
                let i = i as u32;
                let k = t * (5.0 + 7.0 * hash(i * 7 + 1)) + hash(i * 7 + 2) * 50.0;
                let (a, b) = (hash(i * 131 + k as u32), hash(i * 131 + k as u32 + 1));
                let flicker = 0.35 + 0.65 * (a + (b - a) * k.fract());
                let tone = if hash(i * 7 + 3) < 0.7 {
                    0x8fb8ff
                } else {
                    0xd8e4ff
                };
                item.color = Rgb::hex(tone) * (2.2 * flicker * on);
            }
        }

        // The small lights streak with the motion of the camera.
        let before = self::camera(t - look::SHUTTER * 0.5 * flight::exposure(t));
        for id in [
            self.lamp_heads,
            self.small_lights,
            self.roof_lights,
            self.signal_heads,
        ] {
            streaks(&mut scene, id, &camera, &before);
        }
        streaks(
            &mut scene,
            self.stars,
            &camera,
            &Camera {
                position: camera.position,
                ..before
            },
        );
        // The eye adapts to the dark while the city leaves the picture: the stars come
        // out in the tilt. At the cut they are as bright as in shot 5, which has 2.2 times
        // the exposure.
        let adapt = 1.0 + 1.2 * smoothstep(10.8, 12.7, t);
        if let Object::Particles(p) = &mut scene[self.stars].object {
            for item in &mut p.items {
                item.color = item.color * adapt;
            }
        }

        // The far city: its light, the glow of the sky over it and the haze that hides its
        // end. None of it on the chip.
        scene[self.sky].position = camera.position;
        // The mirror of the water costs a pass of the whole scene. In the canyon of Times
        // Square and in the stars no water is in view, so it is off there.
        let mirror = water_mirror(t);
        if mirror <= 0.0 {
            scene.reflector = None;
        } else {
            keep_out_of_mirror(&mut scene);
        }
        if let Some(mat) = scene[self.water].material_mut() {
            mat.reflection = 0.85 * mirror;
        }
        let far = smoothstep(1.0, 2.6, t);
        if let Some(mat) = scene[self.carpet].material_mut() {
            mat.color = Rgb::WHITE * (0.9 * far);
        }
        if let Some(mat) = scene[self.sky].material_mut() {
            mat.color = Rgb::WHITE * far;
        }
        // The far haze has the color of the sky at the horizon: the background there and the
        // glow over it, so the land fades into the sky without an edge.
        let horizon = Rgb::hex(0x081530).0 + SKY_GLOW.0;
        scene.fog = (far > 0.0).then_some(Fog::Exp2 {
            color: Rgb::from(horizon * far),
            density: 1.0 / 9000.0,
        });
        // Haze: none on the chip. In the city a dense mist lies between the towers, and the
        // tops stand clear.
        let air = smoothstep(0.6, 3.4, t);
        scene.atmosphere = Some(Atmosphere {
            density: 0.003 * air,
            color: Rgb::WHITE,
            base: 10.0,
            falloff: 0.016,
            anisotropy: 0.5,
            // The mist takes the warm grey of the city light, like the glow of the sky.
            ambient: Rgb::hex(0x5c6274) * 0.3,
            noise: 0.65,
            noise_size: 55.0,
            offset: vec3(t * 6.0, 0.0, t * 2.5),
            distance: 4000.0,
            steps: 12,
        });
        (scene, camera)
    }
}

/// The mirror of the water shows only the lit facades, the bridges and the lights: roofs,
/// streets and other small parts cost time and are not seen in it. The renderer leaves the
/// materials that reflect out of the mirror pass, so these get a reflection too small to
/// see.
fn keep_out_of_mirror(scene: &mut Scene) {
    let ids: Vec<NodeId> = scene.nodes().map(|(id, _)| id).collect();
    for id in ids {
        if let Some(m) = scene[id].material_mut()
            && m.emissive_map.is_none()
            && m.reflection == 0.0
            && m.blending != Blending::Additive
        {
            m.reflection = 1e-4;
        }
    }
}

/// How much the water mirrors the city at `t`, `0..1`: from the end of the dive until the
/// camera drops into Times Square, and in the climb out of it.
fn water_mirror(t: f32) -> f32 {
    smoothstep(0.9, 1.4, t) * (1.0 - smoothstep(6.6, 7.1, t))
        + smoothstep(10.9, 11.4, t) * (1.0 - smoothstep(12.0, 12.4, t))
}

/// The steam of the vents at `t`: puffs that rise, grow, drift with the wind and fade.
fn steam(t: f32) -> Vec<Particle> {
    let mut out = Vec::new();
    for (v, at) in VENTS.iter().enumerate() {
        for i in 0..PUFFS {
            let seed = v as u32 * 997 + i;
            let life = 3.5 + 1.5 * hash(seed);
            let age = ((t + 30.0) / life + hash(seed + 1)).fract();
            let a = age * life;
            let wobble = vec3(
                (a * 1.3 + hash(seed + 2) * 6.0).sin(),
                0.0,
                (a * 1.1 + hash(seed + 3) * 6.0).cos(),
            ) * (0.4 * a);
            let p = vec3(at.x, 3.4 + 6.5 * a - 0.5 * a * a, at.y)
                + vec3(1.2, 0.0, -0.5) * (a * a * 0.5)
                + wobble;
            let opacity = 0.3 * (1.0 - age).powf(1.6) * smoothstep(0.0, 0.08, age);
            out.push(Particle::new(p, 0.6 + 1.3 * a, Rgb::hex(0xa8b8d0) * 0.35).opacity(opacity));
        }
    }
    out
}

/// The brightness (`0..1`) of the film on the video screens, `made_of_motion.mp4`, every
/// quarter second of the shot. The film owner plays it in sync with the shot. The screens
/// throw this light into Times Square.
const FILM_LIGHT: [f32; 56] = [
    0.80, 0.77, 0.78, 0.82, 0.82, 0.82, 0.82, 0.82, 0.32, 0.56, 0.53, 0.52, 0.52, 0.52, 0.53, 0.21,
    0.11, 0.16, 0.16, 0.18, 0.18, 0.18, 0.19, 0.24, 0.37, 0.71, 0.67, 0.75, 0.75, 0.75, 0.73, 0.73,
    0.73, 0.72, 0.08, 0.09, 0.08, 0.75, 0.75, 0.74, 0.08, 0.13, 0.12, 0.74, 0.74, 0.08, 0.09, 0.08,
    0.74, 0.75, 0.75, 0.78, 0.77, 0.76, 0.77, 0.78,
];

/// The brightness of the film at `t`.
fn film_light(t: f32) -> f32 {
    let k = (t * 4.0).clamp(0.0, (FILM_LIGHT.len() - 1) as f32);
    let i = (k as usize).min(FILM_LIGHT.len() - 2);
    FILM_LIGHT[i] + (FILM_LIGHT[i + 1] - FILM_LIGHT[i]) * (k - i as f32)
}

/// The video screens of Times Square: the places on the walls that the camera sees well
/// when it drops into the square and glides through it, without overlaps and at most
/// four on one building. A screen counts most when it fills about a fifth of the picture
/// while the film is bright.
fn place_screens(city: &Manhattan) -> Vec<build::Screen> {
    // The heights of the roofs on a grid of 2 m cells, to test the lines of sight.
    let (origin, cell, size) = (vec2(300.0, 500.0), 2.0, (240usize, 450usize));
    let mut heights = vec![0.0f32; size.0 * size.1];
    let index = |p: Vec2| {
        let q = ((p - origin) / cell).floor();
        (q.x >= 0.0 && q.y >= 0.0 && (q.x as usize) < size.0 && (q.y as usize) < size.1)
            .then(|| q.y as usize * size.0 + q.x as usize)
    };
    for b in city.buildings.iter().filter(|b| !b.far) {
        for t in &b.roofs {
            let top = t.iter().map(|p| p.y).fold(0.0, f32::max);
            let flat = t.map(|p| vec2(p.x, p.z));
            let (min, max) = flat.iter().fold((Vec2::MAX, Vec2::MIN), |(lo, hi), &p| {
                (lo.min(p), hi.max(p))
            });
            let mut x = (min.x / cell).floor() * cell + cell * 0.5;
            while x <= max.x {
                let mut z = (min.y / cell).floor() * cell + cell * 0.5;
                while z <= max.y {
                    let q = vec2(x, z);
                    let s = [0, 1, 2].map(|k| (flat[(k + 1) % 3] - flat[k]).perp_dot(q - flat[k]));
                    let inside = s.iter().all(|&v| v >= 0.0) || s.iter().all(|&v| v <= 0.0);
                    if let Some(i) = index(q).filter(|_| inside) {
                        heights[i] = heights[i].max(top);
                    }
                    z += cell;
                }
                x += cell;
            }
        }
    }
    // Whether the air between `eye` and `target` is free. The last 3 m before the wall
    // do not count.
    let clear = |eye: Vec3, target: Vec3| {
        let length = eye.distance(target);
        let steps = (length / 2.0) as usize;
        (1..steps).all(|k| {
            let p = eye.lerp(target, k as f32 / steps as f32);
            length * (1.0 - k as f32 / steps as f32) < 3.0
                || index(vec2(p.x, p.z)).is_none_or(|i| p.y > heights[i])
        })
    };
    let (width, height) = (1920.0, 804.0);
    let times: Vec<f32> = (0..22).map(|k| 6.4 + k as f32 * 0.25).collect();
    let views: Vec<(Camera, f32)> = times
        .iter()
        .map(|&t| (camera(t), 0.35 + 0.65 * film_light(t)))
        .collect();
    let score = |screen: &build::Screen| {
        let height_of = screen.top - screen.bottom;
        let mut sum = 0.0;
        for (view, light) in &views {
            let (front, facing) = screen.at(0.5, screen.bottom + height_of * 0.5);
            if (view.position - front).dot(facing) < 2.0 {
                continue;
            }
            let Some(center) = view.project(front + facing, width, height) else {
                continue;
            };
            if !(0.0..width).contains(&center.x) || !(0.0..height).contains(&center.y) {
                continue;
            }
            let mut seen = 0;
            for a in [0.05, 0.5, 0.95] {
                for b in [0.05, 0.5, 0.95] {
                    let (p, n) = screen.at(a, screen.bottom + height_of * b);
                    let p = p + n;
                    let inside = view.project(p, width, height).is_some_and(|q| {
                        (20.0..width - 20.0).contains(&q.x) && (20.0..height - 20.0).contains(&q.y)
                    });
                    let facing_view = (view.position - p).dot(n) > 0.0;
                    seen += usize::from(inside && facing_view && clear(view.position, p));
                }
            }
            if seen >= 5 {
                // The size in the picture: the larger of the width and the height. A fifth
                // of the picture is enough.
                let mid = screen.bottom + height_of * 0.5;
                let ends = [
                    screen.at(0.0, mid).0,
                    screen.at(1.0, mid).0,
                    screen.at(0.5, screen.bottom).0,
                    screen.at(0.5, screen.top).0,
                ]
                .map(|p| view.project(p, width, height));
                if let [Some(l), Some(r), Some(lo), Some(hi)] = ends {
                    let size = (r.x - l.x).abs().max((hi.y - lo.y).abs()).min(width * 0.22);
                    sum += size * seen as f32 / 9.0 * light;
                }
            }
        }
        sum
    };
    let candidates = build::screen_walls(city, TIMES_SQUARE);
    let threads = std::thread::available_parallelism().map_or(4, std::num::NonZero::get);
    let chunk = candidates.len().div_ceil(threads).max(1);
    let mut places: Vec<(f32, build::Screen)> = std::thread::scope(|s| {
        let handles: Vec<_> = candidates
            .chunks(chunk)
            .map(|part| {
                s.spawn(|| {
                    part.iter()
                        .map(|c| (score(c), c.clone()))
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        handles
            .into_iter()
            .filter_map(|h| h.join().ok())
            .flatten()
            .filter(|(score, _)| *score > 0.0)
            .collect()
    });
    places.sort_by(|a, b| b.0.total_cmp(&a.0));
    let overlap = |a: &build::Screen, b: &build::Screen| {
        let (a0, a1) = a.bounds();
        let (b0, b1) = b.bounds();
        let gap = Vec3::splat(1.5);
        (a0 - gap).cmple(b1).all() && (b0 - gap).cmple(a1).all()
    };
    // The best places, with a mix of kinds like in the real square.
    let quota = |kind: build::ScreenKind| match kind {
        build::ScreenKind::Standard => 6,
        build::ScreenKind::Small => 4,
        build::ScreenKind::Tall | build::ScreenKind::Band => 3,
        build::ScreenKind::Curved | build::ScreenKind::Corner => 2,
    };
    let best = places.first().map_or(0.0, |p| p.0);
    let mut out: Vec<build::Screen> = Vec::new();
    for (score, place) in &places {
        let same = out.iter().filter(|o| o.building == place.building).count();
        let kind = out.iter().filter(|o| o.kind == place.kind).count();
        if out.len() < SCREEN_COUNT
            && same < 4
            && kind < quota(place.kind)
            && *score > best * 0.05
            && !out.iter().any(|o| overlap(o, place))
        {
            out.push(place.clone());
        }
    }
    out
}

/// Two low buildings near the track, ahead of the flight, that become construction sites:
/// their index and their footprint box.
fn construction_lots(city: &Manhattan) -> Vec<(usize, Vec2, Vec2)> {
    let mut lots: Vec<(f32, usize, Vec2, Vec2)> = Vec::new();
    for (i, b) in city.buildings.iter().enumerate() {
        let (off, along) = flight::track().project(b.center);
        let (p, d) = flight::track().at(along);
        let side = (b.center - p).perp_dot(d).signum() * off;
        if b.far
            || b.height > 25.0
            || !(350.0..800.0).contains(&along)
            || !(70.0..220.0).contains(&side.abs())
        {
            continue;
        }
        let (mut min, mut max) = (Vec2::MAX, Vec2::MIN);
        for p in b.roofs.iter().flatten() {
            min = min.min(vec2(p.x, p.z));
            max = max.max(vec2(p.x, p.z));
        }
        let size = max - min;
        if size.min_element() > 25.0 {
            lots.push((size.x * size.y * side.signum(), i, min, max));
        }
    }
    // The largest lot on each side of the flight.
    let left = lots
        .iter()
        .filter(|l| l.0 > 0.0)
        .max_by(|a, b| a.0.total_cmp(&b.0));
    let right = lots
        .iter()
        .filter(|l| l.0 < 0.0)
        .min_by(|a, b| a.0.total_cmp(&b.0));
    [left, right]
        .into_iter()
        .flatten()
        .map(|&(_, i, min, max)| (i, min, max))
        .collect()
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

/// Gives the lights of a particle node the path that the motion of the camera from
/// `before` to `now` gives them on the picture. In one step of the shutter each light
/// draws a line, so that in fast moves it is a streak and not a row of dots.
fn streaks(scene: &mut Scene, id: NodeId, now: &Camera, before: &Camera) {
    let origin = scene[id].position;
    let turn = orientation(now) * orientation(before).transpose();
    if let Object::Particles(p) = &mut scene[id].object {
        for item in &mut p.items {
            let at = origin + item.position;
            item.tail = now.position + turn * (at - before.position) - at;
        }
    }
}

/// The rotation from the space of `camera` to the world.
fn orientation(camera: &Camera) -> Mat3 {
    let forward = camera.forward();
    let right = forward.cross(camera.up).normalize();
    Mat3::from_cols(right, right.cross(forward), -forward)
}

/// The chip of shot 3: the die, the traces, the components and the pins. It is 0.04 m
/// square and 0.004 m thick, centered at the origin, with the top face up (+Y).
///
/// The top face is Midtown Manhattan around Broadway at 29th Street at [`CHIP_SCALE`]: the
/// gold traces are the avenues and Broadway, the copper traces the streets, the components
/// the roofs of the buildings. Its +Z direction (uptown) is the top of the first picture of
/// shot 4. That picture looks straight down from 0.0418 m above the top face (y = 0.0433 m)
/// through a 35 mm lens, so the chip with its pins fills 94 % of the picture width.
///
/// One mesh with one material: a small texture atlas gives each part its color, metal and
/// glow.
pub fn chip() -> Node {
    let city = manhattan();
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
    let crossings = build::crossings(&city.streets);
    let [gold, copper] = build::traces(
        &city,
        &crossings,
        (DIVE, CHIP_HALF),
        DIVE,
        s,
        top + 0.000_01,
    );
    add(&mut buf, gold, 1);
    add(&mut buf, copper, 2);

    // The components: the roofs of the buildings, flat on the die. The top face of each
    // shows the component picture over the box of the building.
    for b in &city.buildings {
        if b.far || (b.center - DIVE).abs().max_element() > CHIP_HALF - 20.0 {
            continue;
        }
        let (mut min, mut max) = (Vec2::MAX, Vec2::MIN);
        for p in b.roofs.iter().flatten() {
            min = min.min(vec2(p.x, p.z));
            max = max.max(vec2(p.x, p.z));
        }
        let size = (max - min).max(Vec2::splat(1.0));
        let h = top + 0.000_04 + 0.000_04 * (b.height / 150.0).min(1.0);
        for t in &b.roofs {
            let flat = t.map(|p| {
                let q = (vec2(p.x, p.z) - DIVE) * s;
                vec3(q.x, h, q.y)
            });
            let uv = t.map(|p| {
                let k = (vec2(p.x, p.z) - min) / size;
                vec2(0.755 + k.x * 0.24, 0.02 + k.y * 0.96)
            });
            if (flat[1] - flat[0]).cross(flat[2] - flat[0]).y > 0.0 {
                buf.triangle(flat, uv);
            } else {
                buf.triangle([flat[0], flat[2], flat[1]], [uv[0], uv[2], uv[1]]);
            }
        }
    }

    // The gull wing pins on the four sides, at the places of the pins on the city.
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
    // The gold seal ring at the edge of the top face.
    let mut ring = MeshBuf::default();
    let w = 18.0 * s;
    for (min, max) in [
        (vec2(-half, -half), vec2(-half + w, half)),
        (vec2(half - w, -half), vec2(half, half)),
        (vec2(-half, -half), vec2(half, -half + w)),
        (vec2(-half, half - w), vec2(half, half)),
    ] {
        ring.ground_rect(min, max, top + 0.000_03);
    }
    add(&mut buf, ring, 1);

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
