//! The factory hall: a floor of worn concrete, steel columns and trusses, a sawtooth roof with
//! glazing that faces the far end, corrugated walls, a glass wall at the far end, rows of lamps,
//! conveyors and a catwalk. Machines move in it in one rhythm: bridge cranes, automated carts
//! (AGVs) in the aisles and signal towers beside the arms. The hall runs along -Z. The near
//! wall is at `Z_NEAR` and the glass wall at `Z_FAR`.

use std::sync::Arc;

use fframes_3d::math::{hash, smoothstep};
use fframes_3d::*;

use crate::assets::Assets;

/// The half width of the hall in meters.
pub const HALF_WIDTH: f32 = 60.0;
/// The near end wall.
pub const Z_NEAR: f32 = 20.0;
/// The glass wall at the far end.
pub const Z_FAR: f32 = -420.0;
/// The height of the walls. The sawtooth roof rises 4 m above it.
pub const WALL: f32 = 14.0;
/// The height of the highest point of the roof.
const ROOF: f32 = WALL + 4.0;
/// The length of one tooth of the sawtooth roof along Z.
const TOOTH: f32 = 12.0;
/// The X positions of the production lines.
pub const LINES: [f32; 8] = [-49.0, -35.0, -21.0, -7.0, 7.0, 21.0, 35.0, 49.0];
/// The height of the conveyor belts.
pub const BELT: f32 = 0.82;
/// The catwalk crosses the hall at this Z, at the height `CATWALK_Y`.
pub const CATWALK_Z: f32 = -84.0;
pub const CATWALK_Y: f32 = 8.0;
/// The distance between the parts on a conveyor.
const PART_SPACING: f32 = 2.25;
/// The speed of the conveyors in meters per second.
pub const BELT_SPEED: f32 = 0.5;
/// The height of the bridge girders of the cranes, under the lamps.
const CRANE_Y: f32 = 9.6;
/// The Z positions where the cranes start their cycle. No crane crosses the catwalk.
const CRANES: [f32; 7] = [4.0, -40.0, -104.0, -160.0, -222.0, -290.0, -352.0];
/// The aisles with carts: the X of their middle lines. The center aisle stays clear.
const AISLES: [f32; 6] = [-42.0, -28.0, -14.0, 14.0, 28.0, 42.0];
/// The speed of the carts in meters per second, and the distance between them in a lane.
const AGV_SPEED: f32 = 1.5;
const AGV_SPACING: f32 = 22.0;

/// A bridge crane: the bridge on the runways, the trolley on the bridge, the hook with its
/// load, the hoist ropes and the beacon on the trolley.
struct Crane {
    bridge: NodeId,
    trolley: NodeId,
    hook: NodeId,
    ropes: NodeId,
    beacon: NodeId,
    home: f32,
}

/// The time of the machines at each place: `clock(position)` in seconds. The machines of
/// shot 7 stop when the light passes them.
pub type Clock<'a> = &'a dyn Fn(Vec3) -> f32;

/// The nodes of the hall that change in each frame.
pub struct Hall {
    /// The parts on the conveyors, and their glowing seams.
    pub parts: NodeId,
    pub seams: NodeId,
    /// The lamps under the roof.
    pub lamps: NodeId,
    /// The panes of the roof glazing and the clerestory windows. They show the sky.
    windows: [NodeId; 2],
    /// The panes of the glass wall at the far end.
    glass: NodeId,
    /// The concrete floor.
    pub floor: NodeId,
    cranes: Vec<Crane>,
    /// The carts: bodies, glowing bumpers and loads.
    agvs: [NodeId; 3],
    /// The blue lights of the signal towers.
    beacon: NodeId,
    /// The signal towers: their position and the work phase of the arm beside them.
    towers: Vec<(Vec3, f32)>,
}

/// UVs from the position: each face takes the axis plane that it faces most. One unit of UV
/// is one meter.
pub fn world_uvs(mut g: Geometry) -> Geometry {
    g.uvs = g
        .positions
        .iter()
        .zip(&g.normals)
        .map(|(p, n)| {
            let a = n.abs();
            if a.y >= a.x && a.y >= a.z {
                vec2(p.x, p.z)
            } else if a.x >= a.z {
                vec2(p.z, -p.y)
            } else {
                vec2(p.x, -p.y)
            }
        })
        .collect();
    g
}

/// The cross section of an I-beam: `width` flange, `depth` web, in the XZ plane.
fn i_beam(width: f32, depth: f32, flange: f32, web: f32) -> Vec<Vec2> {
    let (w, d) = (width * 0.5, depth * 0.5);
    let (f, h) = (flange, web * 0.5);
    vec![
        vec2(-w, -d),
        vec2(w, -d),
        vec2(w, -d + f),
        vec2(h, -d + f),
        vec2(h, d - f),
        vec2(w, d - f),
        vec2(w, d),
        vec2(-w, d),
        vec2(-w, d - f),
        vec2(-h, d - f),
        vec2(-h, -d + f),
        vec2(-w, -d + f),
    ]
}

/// A straight bar of square section `size` from `a` to `b`.
fn bar(a: Vec3, b: Vec3, size: f32) -> Geometry {
    let length = a.distance(b);
    Geometry::cuboid(size, length, size)
        .rotate(Quat::from_rotation_arc(Vec3::Y, (b - a) / length))
        .translate(a.midpoint(b).x, a.midpoint(b).y, a.midpoint(b).z)
}

/// Copies at `positions` (translations only).
fn at(positions: impl IntoIterator<Item = Vec3>) -> Instances {
    Instances::new(positions.into_iter().map(Mat4::from_translation))
}

/// The Z positions of the sawtooth teeth, from the far end to the near end.
fn teeth() -> impl Iterator<Item = f32> {
    let count = ((Z_NEAR - Z_FAR) / TOOTH) as usize;
    (0..count).map(|i| Z_FAR + i as f32 * TOOTH)
}

impl Hall {
    pub fn build(scene: &mut Scene, assets: &Assets) -> Self {
        let concrete = assets.concrete.material(0x9a9c9e, 0.85, 0.0);
        let steel = assets.steel.material(0x8a929c, 0.7, 1.0);
        let painted = assets.painted_metal.material(0xb8c4d4, 0.8, 0.0);
        let corrugated = assets.corrugated.material(0x7d848c, 0.9, 1.0);
        let dark = Material::metal(0x24272c, 0.45);
        let paint = Material::standard(0xb4bcc6).roughness(0.7);
        let crane_paint = assets.painted_metal.material(0x6e7884, 0.6, 0.3);

        // ---- The floor, with painted lane lines beside the aisles.
        let length = Z_NEAR - Z_FAR;
        let center = f32::midpoint(Z_NEAR, Z_FAR);
        let floor = scene.add(
            Node::mesh(
                Geometry::floor(HALF_WIDTH * 2.0, length),
                concrete.clone().roughness(0.45).reflection(0.45),
            )
            .at(0.0, 0.0, center)
            .shadows(false, true),
        );
        let lane = Geometry::cuboid(0.12, 0.004, length - 4.0);
        let lanes: Vec<Vec3> = LINES
            .iter()
            .flat_map(|x| [x - 3.6, x + 3.6])
            .map(|x| vec3(x, 0.002, center))
            .collect();
        scene.add(Node::instanced(lane, paint.clone(), at(lanes)).shadows(false, true));

        // ---- The columns, with girders along Z on top.
        let column = world_uvs(
            Geometry::extrude(&i_beam(0.45, 0.6, 0.05, 0.035), WALL + 0.6, 0.0)
                .rotate_x(deg(-90.0)),
        );
        let column_x = [-42.0f32, -14.0, 14.0, 42.0];
        let rows = ((Z_NEAR - Z_FAR) / 24.0) as usize;
        let columns: Vec<Vec3> = (0..=rows)
            .flat_map(|i| column_x.map(|x| vec3(x, 0.0, Z_NEAR - 8.0 - i as f32 * 24.0)))
            .filter(|p| p.z > Z_FAR + 2.0)
            .collect();
        scene.add(Node::instanced(column, painted.clone(), at(columns)));
        let girder = world_uvs(
            Geometry::extrude(&i_beam(0.35, 0.9, 0.05, 0.03), length, 0.0)
                .rotate_x(deg(90.0))
                .rotate_z(deg(90.0)),
        );
        let girders: Vec<Vec3> = column_x
            .iter()
            .chain(&[-HALF_WIDTH + 0.3, HALF_WIDTH - 0.3])
            .map(|x| vec3(*x, WALL + 0.15, Z_NEAR))
            .collect();
        scene.add(Node::instanced(girder, painted.clone(), at(girders)));

        // ---- The sawtooth roof. Each roof plane falls toward +Z. At its high edge a vertical
        // band of glazing faces -Z, above the low edge of the plane before it. The light of
        // the sky comes in through the glazing.
        let rise = 4.0;
        let slope = (rise / TOOTH).atan();
        let roof_plane = world_uvs(
            Geometry::cuboid(HALF_WIDTH * 2.0, 0.12, (TOOTH * TOOTH + rise * rise).sqrt())
                .rotate_x(slope)
                .translate(0.0, WALL + rise * 0.5, TOOTH * 0.5),
        );
        let tooth_positions: Vec<Vec3> = teeth().map(|z| vec3(0.0, 0.0, z)).collect();
        scene.add(Node::instanced(
            roof_plane,
            corrugated.clone(),
            at(tooth_positions.clone()),
        ));
        // a strip of glazing at the top of the band, an opaque panel below it
        let glazed = 1.2;
        scene.add(Node::instanced(
            world_uvs(
                Geometry::cuboid(HALF_WIDTH * 2.0, rise - glazed, 0.1).translate(
                    0.0,
                    WALL + (rise - glazed) * 0.5,
                    0.0,
                ),
            ),
            corrugated.clone(),
            at(tooth_positions.clone()),
        ));
        let pane = Geometry::cuboid(1.9, glazed - 0.15, 0.02).translate(
            0.0,
            WALL + rise - glazed * 0.5,
            0.0,
        );
        let panes: Vec<Vec3> = teeth()
            .flat_map(|z| (0..60).map(move |i| vec3(-59.0 + i as f32 * 2.0, 0.0, z)))
            .collect();
        let glazing = scene.add(
            Node::instanced(
                pane,
                Material::unlit(0xd8e4f4).map(Arc::new(window_glass(1, 1))),
                at(panes),
            )
            .shadows(false, false),
        );
        let mullion = Geometry::cuboid(0.08, rise, 0.12).translate(0.0, WALL + rise * 0.5, 0.0);
        let mullions: Vec<Vec3> = teeth()
            .flat_map(|z| (0..=60).map(move |i| vec3(-60.0 + i as f32 * 2.0, 0.0, z)))
            .collect();
        scene.add(Node::instanced(mullion, dark.clone(), at(mullions)));

        // ---- A truss under the glazing of each tooth: two chords and diagonals.
        let mut members = vec![
            bar(
                vec3(-HALF_WIDTH, WALL, 0.0),
                vec3(HALF_WIDTH, WALL, 0.0),
                0.22,
            ),
            bar(
                vec3(-HALF_WIDTH, WALL - 1.8, 0.0),
                vec3(HALF_WIDTH, WALL - 1.8, 0.0),
                0.2,
            ),
        ];
        let panels = 40;
        for i in 0..panels {
            let x0 = -HALF_WIDTH + i as f32 * 3.0;
            let (top, bottom) = (WALL - 0.1, WALL - 1.7);
            if i % 2 == 0 {
                members.push(bar(vec3(x0, bottom, 0.0), vec3(x0 + 3.0, top, 0.0), 0.09));
            } else {
                members.push(bar(vec3(x0, top, 0.0), vec3(x0 + 3.0, bottom, 0.0), 0.09));
            }
            members.push(bar(vec3(x0, bottom, 0.0), vec3(x0, top, 0.0), 0.07));
        }
        let truss = world_uvs(Geometry::merge(members));
        scene.add(Node::instanced(
            truss,
            steel.clone(),
            at(tooth_positions.clone()),
        ));

        // ---- The side walls: corrugated metal below a band of clerestory windows.
        for side in [-1.0f32, 1.0] {
            let x = side * HALF_WIDTH;
            scene.add(
                Node::mesh(
                    world_uvs(Geometry::cuboid(0.2, 9.5, length)),
                    corrugated.clone(),
                )
                .at(x + side * 0.1, 4.75, center),
            );
            scene.add(
                Node::mesh(
                    world_uvs(Geometry::cuboid(0.2, 1.0, length)),
                    corrugated.clone(),
                )
                .at(x + side * 0.1, WALL - 0.5, center),
            );
        }
        let window = Geometry::cuboid(0.04, 3.2, 5.6);
        let windows: Vec<Vec3> = [-1.0f32, 1.0]
            .iter()
            .flat_map(|side| {
                (0..72).map(move |i| {
                    vec3(
                        side * (HALF_WIDTH + 0.1),
                        11.15,
                        Z_NEAR - 3.0 - i as f32 * 6.0,
                    )
                })
            })
            .filter(|p| p.z > Z_FAR + 3.0)
            .collect();
        let clerestory = scene.add(
            Node::instanced(
                window,
                Material::unlit(0xc4d3e8).map(Arc::new(window_glass(3, 2))),
                at(windows),
            )
            .shadows(false, false),
        );
        // the near end wall
        scene.add(
            Node::mesh(
                world_uvs(Geometry::cuboid(HALF_WIDTH * 2.0, WALL + 4.0, 0.3)),
                corrugated.clone(),
            )
            .at(0.0, ROOF * 0.5, Z_NEAR),
        );

        // ---- The glass wall at the far end: a grid of mullions and panes.
        let glass_pane = Geometry::cuboid(2.9, 2.9, 0.03);
        let glass_panes: Vec<Vec3> = (0..40)
            .flat_map(|i| {
                (0..6).map(move |j| vec3(-58.5 + i as f32 * 3.0, 1.5 + j as f32 * 3.0, Z_FAR))
            })
            .collect();
        let glass = scene.add(
            Node::instanced(
                glass_pane,
                Material::standard(0x0a0e14)
                    .roughness(0.05)
                    .opacity(0.35)
                    .env_intensity(1.5),
                at(glass_panes),
            )
            .shadows(false, false),
        );
        let wall_mullions = Geometry::merge(
            (0..=40)
                .map(|i| {
                    Geometry::cuboid(0.12, WALL + 4.0, 0.2).translate(
                        -60.0 + i as f32 * 3.0,
                        ROOF * 0.5,
                        0.0,
                    )
                })
                .chain((0..=6).map(|j| {
                    Geometry::cuboid(HALF_WIDTH * 2.0, 0.12, 0.2).translate(
                        0.0,
                        j as f32 * 3.0,
                        0.0,
                    )
                })),
        );
        scene.add(Node::mesh(wall_mullions, dark.clone()).at(0.0, 0.0, Z_FAR));

        // ---- Lamps under the trusses: cold glowing discs in housings.
        let housing = Geometry::lathe(
            &[
                vec2(0.0, 0.0),
                vec2(0.32, 0.0),
                vec2(0.3, 0.12),
                vec2(0.1, 0.35),
                vec2(0.0, 0.35),
            ],
            16,
        );
        let lamp_positions: Vec<Vec3> = teeth()
            .flat_map(|z| LINES.map(|x| vec3(x, WALL - 2.6, z + 0.6)))
            .collect();
        scene.add(
            Node::instanced(housing, dark.clone(), at(lamp_positions.clone()))
                .shadows(false, false),
        );
        let lamps = scene.add(
            Node::instanced(
                Geometry::cylinder(0.28, 0.28, 0.02, 16),
                Material::glow(Rgb::WHITE, 1.0),
                at(lamp_positions.iter().map(|p| *p - vec3(0.0, 0.01, 0.0))),
            )
            .shadows(false, false),
        );

        // ---- Pipes and cable trays along the walls and the girders.
        let pipe = Geometry::cylinder(0.18, 0.18, length, 12).rotate_x(deg(90.0));
        let pipes: Vec<Vec3> = [-58.6f32, -57.9, 57.9, 58.6]
            .iter()
            .map(|x| vec3(*x, 9.2 + (x.abs() - 57.9) * 1.2, center))
            .collect();
        scene.add(Node::instanced(pipe, steel.clone(), at(pipes)));
        let tray = world_uvs(Geometry::cuboid(0.6, 0.12, length));
        let trays: Vec<Vec3> = [-14.0f32, 14.0]
            .iter()
            .map(|x| vec3(*x, WALL - 2.2, center))
            .collect();
        scene.add(Node::instanced(tray, steel.clone(), at(trays)));

        // ---- The conveyors: a belt on a frame with legs.
        let belt_length = Z_NEAR - Z_FAR - 14.0;
        let belt_center = (Z_NEAR - 6.0 + Z_FAR + 8.0) * 0.5;
        let belt = Geometry::merge([
            Geometry::cuboid(1.2, 0.06, belt_length).translate(0.0, BELT - 0.03, 0.0),
            Geometry::cuboid(0.08, 0.16, belt_length).translate(-0.66, BELT - 0.02, 0.0),
            Geometry::cuboid(0.08, 0.16, belt_length).translate(0.66, BELT - 0.02, 0.0),
        ]);
        scene.add(Node::instanced(
            world_uvs(belt),
            dark.clone(),
            at(LINES.map(|x| vec3(x, 0.0, belt_center))),
        ));
        let leg = Geometry::merge([
            Geometry::cuboid(0.08, BELT - 0.1, 0.08).translate(-0.6, (BELT - 0.1) * 0.5, 0.0),
            Geometry::cuboid(0.08, BELT - 0.1, 0.08).translate(0.6, (BELT - 0.1) * 0.5, 0.0),
            Geometry::cuboid(1.2, 0.06, 0.06).translate(0.0, 0.3, 0.0),
        ]);
        let legs: Vec<Vec3> = LINES
            .iter()
            .flat_map(|x| {
                let count = (belt_length / 2.0) as usize;
                (0..count).map(move |i| {
                    vec3(
                        *x,
                        0.0,
                        belt_center + belt_length * 0.5 - 1.0 - i as f32 * 2.0,
                    )
                })
            })
            .collect();
        scene.add(Node::instanced(leg, painted.clone(), at(legs)));
        // the parts: battery modules with glowing seams
        let part = Geometry::rounded_box(0.9, 0.22, 1.4, 0.03, 2).translate(0.0, BELT + 0.11, 0.0);
        let parts = scene.add(Node::instanced(
            part,
            Material::metal(0x3a3f47, 0.35).clearcoat(0.4),
            Instances::new([]),
        ));
        let seam = Geometry::merge([
            Geometry::cuboid(0.92, 0.012, 0.03).translate(0.0, BELT + 0.17, -0.35),
            Geometry::cuboid(0.92, 0.012, 0.03).translate(0.0, BELT + 0.17, 0.35),
            Geometry::cuboid(0.03, 0.012, 1.3).translate(0.0, BELT + 0.225, 0.0),
        ]);
        let seams = scene.add(
            Node::instanced(seam, Material::glow(Rgb::WHITE, 1.0), Instances::new([]))
                .shadows(false, false),
        );

        // ---- The catwalk crosses the hall above the lines.
        let deck = world_uvs(Geometry::cuboid(HALF_WIDTH * 2.0 - 1.0, 0.1, 1.6));
        scene.add(Node::mesh(deck, steel.clone()).at(0.0, CATWALK_Y - 0.05, CATWALK_Z));
        // a round middle rail and a flat top rail
        let rail = Geometry::cylinder(0.025, 0.025, HALF_WIDTH * 2.0 - 1.0, 8).rotate_z(deg(90.0));
        let rails: Vec<Vec3> = [-0.78f32, 0.78]
            .iter()
            .map(|dz| vec3(0.0, CATWALK_Y + 0.5, CATWALK_Z + dz))
            .collect();
        scene.add(Node::instanced(rail, painted.clone(), at(rails)));
        let top_rail = Geometry::rounded_box(HALF_WIDTH * 2.0 - 1.0, 0.04, 0.08, 0.01, 1);
        let top_rails: Vec<Vec3> = [-0.78f32, 0.78]
            .iter()
            .map(|dz| vec3(0.0, CATWALK_Y + 1.08, CATWALK_Z + dz))
            .collect();
        scene.add(Node::instanced(top_rail, painted.clone(), at(top_rails)));
        let post = Geometry::cylinder(0.025, 0.025, 1.08, 8).translate(0.0, 0.54, 0.0);
        let posts: Vec<Vec3> = (0..60)
            .flat_map(|i| {
                [-0.78f32, 0.78].map(|dz| vec3(-59.0 + i as f32 * 2.0, CATWALK_Y, CATWALK_Z + dz))
            })
            .collect();
        scene.add(Node::instanced(post, painted.clone(), at(posts)));
        let hanger = Geometry::cylinder(0.03, 0.03, WALL - CATWALK_Y, 6).translate(
            0.0,
            (WALL - CATWALK_Y) * 0.5,
            0.0,
        );
        let hangers: Vec<Vec3> = (0..20)
            .flat_map(|i| {
                [-0.8f32, 0.8].map(|dz| vec3(-57.0 + i as f32 * 6.0, CATWALK_Y, CATWALK_Z + dz))
            })
            .collect();
        scene.add(Node::instanced(hanger, dark.clone(), at(hangers)));

        // ---- The runways of the bridge cranes along the side walls.
        let runway = world_uvs(
            Geometry::extrude(&i_beam(0.3, 0.6, 0.04, 0.03), length, 0.0)
                .rotate_x(deg(90.0))
                .rotate_z(deg(90.0)),
        );
        scene.add(Node::instanced(
            runway,
            steel.clone(),
            at([-1.0f32, 1.0].map(|side| vec3(side * 58.6, CRANE_Y - 0.9, Z_NEAR))),
        ));
        let cranes = CRANES
            .iter()
            .map(|home| Crane::build(scene, &crane_paint, &dark, *home))
            .collect();

        // ---- The carts: a low body, a glowing bumper, a lidar and a battery module on top.
        let body = Geometry::merge([
            Geometry::rounded_box(0.92, 0.28, 1.36, 0.05, 2).translate(0.0, 0.2, 0.0),
            Geometry::cylinder(0.06, 0.07, 0.08, 10).translate(0.0, 0.38, 0.55),
        ]);
        let bumper = Geometry::cuboid(0.94, 0.03, 1.38).translate(0.0, 0.1, 0.0);
        let load = Geometry::rounded_box(0.8, 0.22, 1.2, 0.03, 2).translate(0.0, 0.46, 0.0);
        let agvs = [
            scene.add(Node::instanced(
                body,
                crane_paint.clone(),
                Instances::new([]),
            )),
            scene.add(
                Node::instanced(bumper, Material::glow(Rgb::WHITE, 1.0), Instances::new([]))
                    .shadows(false, false),
            ),
            scene.add(Node::instanced(
                load,
                Material::metal(0x3a3f47, 0.35).clearcoat(0.4),
                Instances::new([]),
            )),
        ];

        // ---- A signal tower between every second pair of arms.
        let towers: Vec<(Vec3, f32)> = super::crowd::stations(&LINES, 10.0, Z_FAR + 8.0, 4.5)
            .into_iter()
            .enumerate()
            .filter(|(i, _)| i % 4 < 2)
            .map(|(_, s)| {
                let base = s.base.w_axis.truncate();
                let out = (base.x - nearest_line(base.x)).signum();
                (vec3(base.x + out * 0.8, 0.0, base.z - 2.25), s.phase)
            })
            .collect();
        let pole = Geometry::merge([
            Geometry::cuboid(0.24, 0.04, 0.24).translate(0.0, 0.02, 0.0),
            Geometry::cylinder(0.025, 0.025, 1.68, 8).translate(0.0, 0.86, 0.0),
            Geometry::cylinder(0.055, 0.055, 0.05, 12).translate(0.0, 2.02, 0.0),
        ]);
        scene.add(Node::instanced(
            pole,
            dark.clone(),
            at(towers.iter().map(|(p, _)| *p)),
        ));
        // two lights: blue (the arm welds) on top of white (the arm is in service)
        let light = |y: f32| Geometry::cylinder(0.05, 0.05, 0.1, 12).translate(0.0, y, 0.0);
        scene.add(
            Node::instanced(
                light(1.81),
                Material::glow(0xdfeaff, 1.6),
                at(towers.iter().map(|(p, _)| *p)),
            )
            .shadows(false, false),
        );
        let beacon = scene.add(
            Node::instanced(
                light(1.93),
                Material::glow(Rgb::WHITE, 1.0),
                Instances::new([]),
            )
            .shadows(false, false),
        );

        Self {
            parts,
            seams,
            lamps,
            windows: [glazing, clerestory],
            glass,
            floor,
            cranes,
            agvs,
            beacon,
            towers,
        }
    }

    /// Moves the cranes and the carts and sets the signal towers. `clock` gives the time of
    /// the machines at each place, `welding(time, phase)` whether an arm welds. Only the carts
    /// that `sees` draw.
    pub fn machines(
        &self,
        scene: &mut Scene,
        clock: Clock,
        welding: impl Fn(f32, f32) -> bool,
        sees: impl Fn(Vec3, f32) -> bool,
    ) {
        for crane in &self.cranes {
            crane.pose(scene, clock(vec3(0.0, CRANE_Y, crane.home)));
        }

        // The carts drive in two lanes in each aisle, in opposite directions.
        let length = Z_NEAR - Z_FAR;
        let mut carts = Vec::new();
        for (a, x) in AISLES.iter().enumerate() {
            for (lane, side) in [-1.0f32, 1.0].into_iter().enumerate() {
                let offset = AGV_SPACING * hash((a * 2 + lane) as u32 + 71);
                let count = (length / AGV_SPACING) as usize;
                for i in 0..count {
                    let at = |time: f32| {
                        let s =
                            (offset + i as f32 * AGV_SPACING + time * AGV_SPEED).rem_euclid(length);
                        vec3(
                            x + side * 1.6,
                            0.0,
                            Z_FAR + if side > 0.0 { s } else { length - s },
                        )
                    };
                    // A cart stops where its clock stops. The clock of the place where the
                    // cart is now gives the place where it stopped, close enough.
                    let now = at(clock(vec3(*x, 0.0, 0.0)));
                    let p = at(clock(now));
                    if sees(p + vec3(0.0, 0.3, 0.0), 1.0) {
                        let turn = if side > 0.0 {
                            0.0
                        } else {
                            std::f32::consts::PI
                        };
                        carts.push((
                            Mat4::from_rotation_translation(Quat::from_rotation_y(turn), p),
                            (a + i + lane) % 3 != 0,
                        ));
                    }
                }
            }
        }
        let set = |scene: &mut Scene, node: NodeId, instances: Instances| {
            if let Object::Mesh(mesh) = &mut scene[node].object {
                mesh.instances = Some(Arc::new(instances));
            }
        };
        let bumper = Rgb::hex(0x6fb8ff) * 3.0;
        set(
            scene,
            self.agvs[0],
            Instances::new(carts.iter().map(|c| c.0)),
        );
        set(
            scene,
            self.agvs[1],
            Instances::new(carts.iter().map(|c| c.0))
                .colors(std::iter::repeat_n(bumper, carts.len())),
        );
        set(
            scene,
            self.agvs[2],
            Instances::new(carts.iter().filter(|c| c.1).map(|c| c.0)),
        );

        // The blue light of a tower is on while its arm welds.
        let blue = self.towers.iter().map(|(p, phase)| {
            let on = if welding(clock(*p), *phase) {
                6.0
            } else {
                0.25
            };
            Rgb::hex(0x4f9dff) * on
        });
        set(
            scene,
            self.beacon,
            at(self.towers.iter().map(|(p, _)| *p)).colors(blue),
        );
    }

    /// Moves the parts along the conveyors to time `t`. Only parts between `z0` and `z1` are
    /// drawn, because the haze hides the others.
    pub fn conveyors(&self, scene: &mut Scene, t: f32, z0: f32, z1: f32, seam: Rgb) {
        let shift = (t * BELT_SPEED).rem_euclid(PART_SPACING);
        let count = ((z0 - z1) / PART_SPACING) as usize;
        let transforms: Vec<Mat4> = LINES
            .iter()
            .flat_map(|x| {
                (0..count).map(move |i| {
                    Mat4::from_translation(vec3(*x, 0.0, z0 - i as f32 * PART_SPACING + shift))
                })
            })
            .collect();
        let set = |scene: &mut Scene, node: NodeId, instances: Instances| {
            if let Object::Mesh(mesh) = &mut scene[node].object {
                mesh.instances = Some(Arc::new(instances));
            }
        };
        set(
            scene,
            self.seams,
            Instances::new(transforms.clone()).colors(std::iter::repeat_n(seam, transforms.len())),
        );
        set(scene, self.parts, Instances::new(transforms));
    }

    /// Makes the windows show the night sky.
    pub fn night(&self, scene: &mut Scene) {
        for node in self.windows {
            if let Some(m) = scene[node].material_mut() {
                m.color = Rgb::hex(0x070b14);
            }
        }
    }

    /// Makes the panes of the glass wall glow in `color`: the light outside comes through.
    pub fn light_glass(&self, scene: &mut Scene, color: Rgb) {
        if let Some(m) = scene[self.glass].material_mut() {
            m.emissive = color;
        }
    }

    /// Adds point lights at the `count` lamps nearest to `near`, so that they light the floor
    /// and the arms under them and glow in the haze.
    pub fn lamp_lights(&self, scene: &mut Scene, near: Vec3, count: usize, light: Light) {
        let mut lamps: Vec<Vec3> = teeth()
            .flat_map(|z| LINES.map(|x| vec3(x, WALL - 2.75, z + 0.6)))
            .collect();
        lamps.sort_by(|a, b| a.distance(near).total_cmp(&b.distance(near)));
        for p in lamps.into_iter().take(count) {
            scene.add(Node::light(light.clone()).at(p.x, p.y, p.z));
        }
    }

    /// Sets the glow of the lamps.
    pub fn light_lamps(&self, scene: &mut Scene, color: Rgb) {
        if let Some(m) = scene[self.lamps].material_mut() {
            m.color = color;
        }
    }
}

/// The light through a window of `lites_u` by `lites_v` lites of glass in a frame: brighter
/// at the top, with dirt along the bars and the sill. `v` runs down.
fn window_glass(lites_u: u32, lites_v: u32) -> Texture {
    // value noise: the bilinear blend of a lattice of random values
    let value = |u: f32, v: f32, cells: f32, seed: u32| {
        let (x, y) = (u * cells, v * cells);
        let (i, j) = (x.floor(), y.floor());
        let (fx, fy) = (x - i, y - j);
        let at = |di: f32, dj: f32| hash(((i + di) * 131.0 + (j + dj) * 7.0) as u32 + seed);
        let top = at(0.0, 0.0) + (at(1.0, 0.0) - at(0.0, 0.0)) * fx;
        let bottom = at(0.0, 1.0) + (at(1.0, 1.0) - at(0.0, 1.0)) * fx;
        top + (bottom - top) * fy
    };
    Texture::from_fn(384, 224, |u, v| {
        // the bars of the frame: 6 cm in a window of 5.6 by 3.2 m
        let lite_u = (u * lites_u as f32).fract();
        let lite_v = (v * lites_v as f32).fract();
        let bar = lite_u < 0.011 || lite_u > 0.989 || lite_v < 0.019 || lite_v > 0.981;
        let edge = smoothstep(0.0, 0.06, lite_u.min(1.0 - lite_u))
            * smoothstep(0.0, 0.1, lite_v.min(1.0 - lite_v));
        let dirt = 0.5 * value(u, v, 9.0, 11) + 0.3 * value(u, v, 31.0, 23);
        let grime = (1.0 - edge) * (0.15 + 0.4 * dirt) + lite_v * 0.12 * dirt;
        let l = if bar {
            0.22
        } else {
            (1.0 - 0.1 * v) * (1.0 - grime)
        };
        Rgb::linear(l, l, l).to_color()
    })
}

/// The X of the production line nearest to `x`.
fn nearest_line(x: f32) -> f32 {
    LINES
        .into_iter()
        .min_by(|a, b| (a - x).abs().total_cmp(&(b - x).abs()))
        .unwrap_or(0.0)
}

impl Crane {
    fn build(scene: &mut Scene, paint: &Material, dark: &Material, home: f32) -> Self {
        let span = HALF_WIDTH * 2.0 - 3.0;
        let bridge = scene.add(Node::group().at(0.0, CRANE_Y, home));
        let girders = world_uvs(Geometry::merge([
            Geometry::cuboid(span, 1.0, 0.55).translate(0.0, 0.0, -1.0),
            Geometry::cuboid(span, 1.0, 0.55).translate(0.0, 0.0, 1.0),
            Geometry::cuboid(1.1, 0.7, 5.6).translate(-span * 0.5, -0.5, 0.0),
            Geometry::cuboid(1.1, 0.7, 5.6).translate(span * 0.5, -0.5, 0.0),
        ]));
        scene.add_to(bridge, Node::mesh(girders, paint.clone()));
        // light stripes mark the ends of the girders
        let stripes = Geometry::merge((0..6).flat_map(|i| {
            [-1.0f32, 1.0].map(|side| {
                Geometry::cuboid(0.4, 1.02, 2.62).translate(
                    side * (span * 0.5 - 1.5 - i as f32 * 0.8),
                    0.0,
                    0.0,
                )
            })
        }));
        scene.add_to(
            bridge,
            Node::mesh(stripes, Material::standard(0xd8dde2).roughness(0.6)),
        );
        let trolley = scene.add_to(bridge, Node::group());
        let frame = Geometry::merge([
            Geometry::cuboid(2.6, 0.6, 2.9).translate(0.0, 0.8, 0.0),
            Geometry::cylinder(0.36, 0.36, 1.9, 16)
                .rotate_z(deg(90.0))
                .translate(0.0, 1.3, 0.0),
            Geometry::cuboid(0.7, 0.6, 0.6).translate(1.3, 1.2, 0.8),
        ]);
        scene.add_to(trolley, Node::mesh(world_uvs(frame), paint.clone()));
        let beacon = scene.add_to(
            trolley,
            Node::mesh(
                Geometry::cylinder(0.08, 0.08, 0.16, 12).translate(-1.0, 1.25, -1.1),
                Material::glow(0x4f9dff, 1.0),
            )
            .shadows(false, false),
        );
        // four ropes of 1 m that the hook scales to its depth
        let ropes = scene.add_to(
            trolley,
            Node::mesh(
                Geometry::merge([-0.25f32, 0.25].into_iter().flat_map(|x| {
                    [-0.15f32, 0.15]
                        .map(|z| Geometry::cylinder(0.014, 0.014, 1.0, 6).translate(x, -0.5, z))
                })),
                Material::metal(0x9aa0a8, 0.4),
            )
            .shadows(false, false),
        );
        let hook = scene.add_to(trolley, Node::group());
        let block = Geometry::merge([
            Geometry::rounded_box(0.7, 0.8, 0.45, 0.06, 2).translate(0.0, -0.4, 0.0),
            Geometry::torus(0.16, 0.04, 8, 16)
                .rotate_y(deg(90.0))
                .translate(0.0, -0.95, 0.0),
            Geometry::cuboid(3.0, 0.16, 0.16).translate(0.0, -1.15, 0.0),
        ]);
        scene.add_to(hook, Node::mesh(world_uvs(block), dark.clone()));
        // the load: a battery pack of four modules on the spreader beam
        let pack = Geometry::merge((0..4).map(|i| {
            Geometry::rounded_box(0.62, 0.5, 1.5, 0.03, 2).translate(
                -1.0 + i as f32 * 0.66,
                -1.5,
                0.0,
            )
        }));
        scene.add_to(
            hook,
            Node::mesh(pack, Material::metal(0x3a3f47, 0.35).clearcoat(0.4)),
        );
        scene.add_to(
            hook,
            Node::mesh(
                Geometry::merge((0..4).map(|i| {
                    Geometry::cuboid(0.03, 0.02, 1.4).translate(-1.0 + i as f32 * 0.66, -1.24, 0.0)
                })),
                Material::glow(0x8fd0ff, 4.0),
            )
            .shadows(false, false),
        );
        Self {
            bridge,
            trolley,
            hook,
            ropes,
            beacon,
            home,
        }
    }

    /// Poses the crane at `time`. Every 16 s the bridge moves 16 m along the hall and back,
    /// the trolley crosses, and the hook goes down and up. Each move takes 4 s.
    fn pose(&self, scene: &mut Scene, time: f32) {
        let seed = self.home.to_bits();
        let t = (time + 16.0 * hash(seed)).rem_euclid(16.0);
        let there = smoothstep(0.0, 4.0, t) - smoothstep(8.0, 12.0, t);
        let across = smoothstep(4.0, 8.0, t) - smoothstep(12.0, 16.0, t);
        let drop = 3.0 + 3.0 * (smoothstep(2.0, 5.0, t) - smoothstep(10.0, 13.0, t));
        let side = if hash(seed + 1) > 0.5 { 1.0 } else { -1.0 };
        scene[self.bridge].position.z = self.home - 16.0 * there;
        scene[self.trolley].position.x = side * (24.0 + 18.0 * across);
        scene[self.hook].position.y = -drop;
        scene[self.ropes].scale.y = drop;
        // the beacon flashes twice a second while the crane moves
        let moving = (there * (1.0 - there) + across * (1.0 - across)) > 0.01;
        let flash = if moving && (time * 2.0).fract() < 0.25 {
            8.0
        } else {
            0.6
        };
        if let Some(m) = scene[self.beacon].material_mut() {
            m.color = Rgb::hex(0x4f9dff) * flash;
        }
    }
}
