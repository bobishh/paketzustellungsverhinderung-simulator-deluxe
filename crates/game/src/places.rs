//! Small public-space props; geometry and collisions share the same parts.
use crate::{Mode, to_world};
use avian3d::prelude::*;
use bevy::prelude::*;
use paket_geo::{
    EastNorth,
    places::{Kind, Place},
};

pub(crate) struct Kit {
    cube: Handle<Mesh>,
    crown: Handle<Mesh>,
    wood: Handle<StandardMaterial>,
    metal: Handle<StandardMaterial>,
    blue: Handle<StandardMaterial>,
    orange: Handle<StandardMaterial>,
    leaves: Handle<StandardMaterial>,
    sand: Handle<StandardMaterial>,
}
impl Kit {
    pub fn new(meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>) -> Self {
        let mut material =
            |r, g, b| materials.add(crate::surfaces::matte(Color::srgb(r, g, b), 0.85));
        Self {
            cube: meshes.add(Cuboid::new(1., 1., 1.)),
            crown: meshes.add(Sphere::new(1.).mesh().ico(2).unwrap()),
            wood: material(0.43, 0.26, 0.12),
            metal: material(0.22, 0.28, 0.27),
            blue: material(0.12, 0.44, 0.58),
            orange: material(0.91, 0.39, 0.12),
            leaves: material(0.24, 0.40, 0.16),
            sand: material(0.72, 0.63, 0.43),
        }
    }
    fn part(
        &self,
        commands: &mut Commands,
        root: Entity,
        at: Vec3,
        size: Vec3,
        angle: f32,
        material: &Handle<StandardMaterial>,
    ) {
        commands.spawn((
            ChildOf(root),
            Mesh3d(self.cube.clone()),
            MeshMaterial3d(material.clone()),
            Transform::from_translation(at)
                .with_rotation(Quat::from_rotation_x(angle))
                .with_scale(size),
            RigidBody::Static,
            Collider::cuboid(1., 1., 1.),
        ));
    }
    fn bench(&self, commands: &mut Commands, root: Entity, offset: Vec3) {
        for x in [-0.65, 0.65] {
            self.part(
                commands,
                root,
                offset + Vec3::new(x, 0.25, 0.),
                Vec3::new(0.12, 0.5, 0.48),
                0.,
                &self.metal,
            );
        }
        for x in [-0.65, 0.65] {
            self.part(
                commands,
                root,
                offset + Vec3::new(x, 0.76, 0.27),
                Vec3::new(0.08, 0.6, 0.08),
                0.,
                &self.metal,
            );
        }
        for z in [-0.18, 0., 0.18] {
            self.part(
                commands,
                root,
                offset + Vec3::new(0., 0.52, z),
                Vec3::new(1.8, 0.08, 0.14),
                0.,
                &self.wood,
            );
        }
        for y in [0.78, 0.97] {
            self.part(
                commands,
                root,
                offset + Vec3::new(0., y, 0.27),
                Vec3::new(1.8, 0.14, 0.08),
                0.,
                &self.wood,
            );
        }
    }
    pub fn spawn(
        &self,
        commands: &mut Commands,
        meshes: &mut Assets<Mesh>,
        place: &Place,
    ) -> Entity {
        let root = commands
            .spawn((
                DespawnOnExit(Mode::Ride),
                Transform::from_translation(to_world(place.position, 0.)),
                Visibility::default(),
            ))
            .id();
        match place.kind {
            Kind::Bench => self.bench(commands, root, Vec3::ZERO),
            Kind::Tree => {
                self.part(
                    commands,
                    root,
                    Vec3::new(0., 1.7, 0.),
                    Vec3::new(0.35, 3.4, 0.35),
                    0.,
                    &self.wood,
                );
                commands.spawn((
                    ChildOf(root),
                    Mesh3d(self.crown.clone()),
                    MeshMaterial3d(self.leaves.clone()),
                    Transform::from_xyz(0., 4.2, 0.).with_scale(Vec3::new(2., 2.4, 1.8)),
                ));
            }
            Kind::Playground => {
                if !place.outline.is_empty() {
                    let area = paket_geo::landscape::Area {
                        cover: paket_geo::landscape::Cover::Woodland,
                        rings: vec![place.outline.clone()],
                    };
                    if let Some((points, indices)) = area.mesh() {
                        use bevy::{
                            asset::RenderAssetUsages,
                            mesh::{Indices, PrimitiveTopology},
                        };
                        let positions: Vec<_> = points
                            .iter()
                            .map(|p| {
                                to_world(
                                    EastNorth {
                                        east: p.east - place.position.east,
                                        north: p.north - place.position.north,
                                    },
                                    -0.022,
                                )
                            })
                            .collect();
                        let mut triangles = Vec::new();
                        for t in indices.chunks_exact(3) {
                            let [a, b, c] = [t[0], t[1], t[2]];
                            triangles.extend(
                                if (positions[b] - positions[a])
                                    .cross(positions[c] - positions[a])
                                    .y
                                    > 0.
                                {
                                    [a as u32, b as u32, c as u32]
                                } else {
                                    [a as u32, c as u32, b as u32]
                                },
                            );
                        }
                        let mesh = Mesh::new(
                            PrimitiveTopology::TriangleList,
                            RenderAssetUsages::default(),
                        )
                        .with_inserted_attribute(
                            Mesh::ATTRIBUTE_NORMAL,
                            vec![Vec3::Y; positions.len()],
                        )
                        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
                        .with_inserted_indices(Indices::U32(triangles));
                        commands.spawn((
                            ChildOf(root),
                            Mesh3d(meshes.add(mesh)),
                            MeshMaterial3d(self.sand.clone()),
                        ));
                    }
                }
                // Each prop needs a clear footprint inside the actual playground boundary.
                for (offset, half, kind) in [
                    (Vec3::new(-2.4, 0., 0.), Vec2::new(1.6, 1.3), 0),
                    (Vec3::new(2.1, 0., 0.), Vec2::new(0.9, 2.3), 1),
                    (Vec3::new(0., 0., 3.4), Vec2::new(1., 0.5), 2),
                ] {
                    if !fits(place, offset, half) {
                        continue;
                    }
                    match kind {
                        0 => {
                            for x in [-1.4, 1.4] {
                                for z in [-0.9, 0.9] {
                                    self.part(
                                        commands,
                                        root,
                                        offset + Vec3::new(x, 1.25, z * 0.5),
                                        Vec3::new(0.12, 2.65, 0.12),
                                        -z.atan2(2.5),
                                        &self.blue,
                                    );
                                }
                            }
                            self.part(
                                commands,
                                root,
                                offset + Vec3::new(0., 2.5, 0.),
                                Vec3::new(3., 0.15, 0.15),
                                0.,
                                &self.orange,
                            );
                            for x in [-0.7, 0.7] {
                                for side in [-0.23, 0.23] {
                                    self.part(
                                        commands,
                                        root,
                                        offset + Vec3::new(x + side, 1.55, 0.),
                                        Vec3::new(0.025, 1.9, 0.025),
                                        0.,
                                        &self.metal,
                                    );
                                }
                                self.part(
                                    commands,
                                    root,
                                    offset + Vec3::new(x, 0.58, 0.),
                                    Vec3::new(0.6, 0.09, 0.35),
                                    0.,
                                    &self.wood,
                                );
                            }
                        }
                        1 => {
                            for x in [-0.55, 0.55] {
                                self.part(
                                    commands,
                                    root,
                                    offset + Vec3::new(x, 0.85, -1.1),
                                    Vec3::new(0.12, 1.7, 0.12),
                                    0.,
                                    &self.blue,
                                );
                            }
                            self.part(
                                commands,
                                root,
                                offset + Vec3::new(0., 1.65, -1.),
                                Vec3::new(1.2, 0.12, 0.65),
                                0.,
                                &self.orange,
                            );
                            self.part(
                                commands,
                                root,
                                offset + Vec3::new(0., 0.9, 0.45),
                                Vec3::new(0.95, 0.09, 3.1),
                                0.51,
                                &self.blue,
                            );
                            for x in [-0.52, 0.52] {
                                self.part(
                                    commands,
                                    root,
                                    offset + Vec3::new(x, 1.05, 0.45),
                                    Vec3::new(0.09, 0.22, 3.1),
                                    0.51,
                                    &self.orange,
                                );
                            }
                            for y in [0.3, 0.6, 0.9, 1.2] {
                                self.part(
                                    commands,
                                    root,
                                    offset + Vec3::new(0., y, -1.42),
                                    Vec3::new(1., 0.08, 0.18),
                                    0.,
                                    &self.wood,
                                );
                            }
                        }
                        _ => self.bench(commands, root, offset),
                    }
                }
            }
        }
        root
    }
}
fn fits(place: &Place, offset: Vec3, half: Vec2) -> bool {
    use geo::{Contains, Coord, LineString, Polygon, Rect};
    // Point-only playgrounds have no boundary for inferred equipment.
    if place.outline.len() < 3 {
        return false;
    }
    let polygon = Polygon::new(
        LineString::from(
            place
                .outline
                .iter()
                .map(|p| (p.east, p.north))
                .collect::<Vec<_>>(),
        ),
        vec![],
    );
    let center = Coord {
        x: place.position.east + offset.x as f64,
        y: place.position.north - offset.z as f64,
    };
    let footprint = Rect::new(
        Coord {
            x: center.x - half.x as f64,
            y: center.y - half.y as f64,
        },
        Coord {
            x: center.x + half.x as f64,
            y: center.y + half.y as f64,
        },
    );
    polygon.contains(&footprint)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn playground(size: f64) -> Place {
        Place {
            id: "way/test".into(),
            kind: Kind::Playground,
            position: EastNorth {
                east: 0.,
                north: 0.,
            },
            outline: vec![
                EastNorth {
                    east: -size,
                    north: -size,
                },
                EastNorth {
                    east: size,
                    north: -size,
                },
                EastNorth {
                    east: size,
                    north: size,
                },
                EastNorth {
                    east: -size,
                    north: size,
                },
            ],
        }
    }
    #[test]
    fn equipment_stays_inside_the_mapped_playground() {
        assert!(fits(
            &playground(10.),
            Vec3::new(-2.4, 0., 0.),
            Vec2::new(1.6, 1.3)
        ));
        assert!(!fits(
            &playground(1.),
            Vec3::new(-2.4, 0., 0.),
            Vec2::new(1.6, 1.3)
        ));
        let mut point = playground(10.);
        point.outline.clear();
        assert!(!fits(&point, Vec3::ZERO, Vec2::ONE));
    }
    #[test]
    fn playground_parts_have_collisions_and_despawn_with_the_tile() {
        let mut world = World::new();
        let mut meshes = Assets::<Mesh>::default();
        let mut materials = Assets::<StandardMaterial>::default();
        let kit = Kit::new(&mut meshes, &mut materials);
        let root = kit.spawn(&mut world.commands(), &mut meshes, &playground(10.));
        world.flush();
        let parts = world.query::<&Collider>().iter(&world).count();
        assert!(parts > 20, "swings, slide and bench should be solid");
        world.despawn(root);
        assert_eq!(world.query::<&Collider>().iter(&world).count(), 0);
    }
}
