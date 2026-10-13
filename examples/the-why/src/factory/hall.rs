//! The factory hall: a floor of worn concrete, steel columns and trusses, a sawtooth roof with
//! glazing that faces the far end, corrugated walls, a glass wall at the far end, rows of lamps,
//! conveyors, a catwalk and a bridge crane. The hall runs along -Z. The near wall is at
//! `Z_NEAR` and the glass wall at `Z_FAR`.

use std::sync::Arc;

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
pub const CATWALK_Z: f32 = -66.0;
pub const CATWALK_Y: f32 = 7.0;
/// The distance between the parts on a conveyor.
const PART_SPACING: f32 = 2.25;
/// The speed of the conveyors in meters per second.
pub const BELT_SPEED: f32 = 0.5;

/// The nodes of the hall that change in each frame.
pub struct Hall {
    /// The parts on the conveyors, and their glowing seams.
    pub parts: NodeId,
    pub seams: NodeId,
    /// The lamps under the roof.
    pub lamps: NodeId,
    /// The panes of the roof glazing and the clerestory windows. They show the sky.
    windows: [NodeId; 2],
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
        let yellow = Material::standard(0xd9a400).roughness(0.6);

        // ---- The floor, with yellow lane lines beside the aisles.
        let length = Z_NEAR - Z_FAR;
        let center = f32::midpoint(Z_NEAR, Z_FAR);
        scene.add(
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
        scene.add(Node::instanced(lane, yellow.clone(), at(lanes)).shadows(false, true));

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
        let glazing = scene
            .add(Node::instanced(pane, Material::unlit(0xd8e4f4), at(panes)).shadows(false, false));
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
            Node::instanced(window, Material::unlit(0xc4d3e8), at(windows)).shadows(false, false),
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
        scene.add(
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

        // ---- A bridge crane on rails along the side walls.
        let crane = world_uvs(Geometry::merge([
            Geometry::cuboid(HALF_WIDTH * 2.0 - 2.0, 1.1, 0.7).translate(0.0, 0.0, -0.9),
            Geometry::cuboid(HALF_WIDTH * 2.0 - 2.0, 1.1, 0.7).translate(0.0, 0.0, 0.9),
            Geometry::cuboid(2.4, 1.4, 3.0).translate(-18.0, -0.2, 0.0),
        ]));
        scene.add(Node::mesh(crane, yellow.clone().roughness(0.5)).at(0.0, WALL - 3.0, -150.0));

        Self {
            parts,
            seams,
            lamps,
            windows: [glazing, clerestory],
        }
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

    /// Sets the glow of the lamps.
    pub fn light_lamps(&self, scene: &mut Scene, color: Rgb) {
        if let Some(m) = scene[self.lamps].material_mut() {
            m.color = color;
        }
    }
}
