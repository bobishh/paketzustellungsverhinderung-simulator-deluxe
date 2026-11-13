//! Elevated OSM bridge decks. Their render mesh is also the physical driving surface.
use crate::{
    Mode, StreetMaterials,
    actors::{ParkedVan, Player},
    roads::Roads,
};
use avian3d::prelude::*;
use bevy::{
    asset::RenderAssetUsages,
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
};
use paket_geo::{Street, Surface};

// Start below the physical ground (-0.05), avoiding a vertical lip at the ramp.
const GROUND: f32 = -0.055;
const GRADE: f32 = 0.14;
const THICKNESS: f32 = 0.32;

#[derive(Component)]
pub(crate) struct BridgeDeck;

struct Span {
    points: Vec<Vec3>,
    sides: Vec<Vec3>,
    left: f32,
    right: f32,
    road_half: f32,
    surface: Surface,
}

#[derive(Resource, Default)]
pub(crate) struct Bridges {
    generation: u64,
    root: Option<Entity>,
    spans: Vec<Span>,
    concrete: Option<Handle<StandardMaterial>>,
    metal: Option<Handle<StandardMaterial>>,
}

impl Bridges {
    pub fn height_at(&self, point: Vec3) -> Option<f32> {
        self.spans
            .iter()
            .flat_map(|span| {
                (0..span.points.len() - 1).filter_map(move |i| {
                    let a = span.points[i];
                    let b = span.points[i + 1];
                    let u = span.sides[i];
                    let v = span.sides[i + 1];
                    let [al, ar, br, bl] = [
                        a - u * span.left,
                        a + u * span.right,
                        b + v * span.right,
                        b - v * span.left,
                    ];
                    // Query the same triangles as the collider, including the
                    // outside of a bend where a centreline projection misses.
                    triangle_height(point, [al, ar, br])
                        .or_else(|| triangle_height(point, [al, br, bl]))
                })
            })
            .max_by(f32::total_cmp)
    }
}

fn triangle_height(point: Vec3, [a, b, c]: [Vec3; 3]) -> Option<f32> {
    let edge = (b - a).xz();
    let other = (c - a).xz();
    let offset = (point - a).xz();
    let determinant = edge.perp_dot(other);
    if determinant.abs() < 1e-6 {
        return None;
    }
    let u = offset.perp_dot(other) / determinant;
    let v = edge.perp_dot(offset) / determinant;
    (u >= -1e-5 && v >= -1e-5 && u + v <= 1.00001)
        .then_some(a.y + u * (b.y - a.y) + v * (c.y - a.y))
}

fn plans(streets: &[&Street]) -> Vec<Span> {
    let paths = streets
        .iter()
        .map(|s| {
            s.centerline
                .iter()
                .map(|p| crate::to_world(*p, 0.))
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let lengths = paths
        .iter()
        .map(|p| p.windows(2).map(|w| w[0].distance(w[1])).sum::<f32>())
        .collect::<Vec<_>>();
    let ends = paths
        .iter()
        .flat_map(|p| [p[0], *p.last().unwrap()])
        .collect::<Vec<_>>();
    let mut links = vec![vec![]; ends.len()];
    for a in 0..ends.len() {
        for b in a + 1..ends.len() {
            if a / 2 != b / 2
                && streets[a / 2].bridge == streets[b / 2].bridge
                && ends[a].distance(ends[b]) < 0.25
            {
                links[a].push(b);
                links[b].push(a);
            }
        }
    }
    // Distance to the bank follows connected ways. A map split in the middle
    // of a bridge must not create a dip back down to the water.
    let mut bank = links
        .iter()
        .map(|l| if l.is_empty() { 0. } else { f32::INFINITY })
        .collect::<Vec<_>>();
    for _ in 0..ends.len() {
        let mut changed = false;
        for i in 0..ends.len() {
            let distance = links[i]
                .iter()
                .fold(bank[i ^ 1] + lengths[i / 2], |d, &j| d.min(bank[j]))
                .min(bank[i]);
            if distance < bank[i] {
                bank[i] = distance;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    streets
        .iter()
        .enumerate()
        .filter_map(|(i, s)| {
            if lengths[i] < 0.1 {
                return None;
            }
            // Layer is ordering, not metres. Use a consistent game-scale clearance.
            let clearance = 2.4 * f32::from(s.bridge.unwrap_or(1).clamp(1, 4));
            let mut distance = 0.;
            let mut points = vec![];
            for (segment, pair) in paths[i].windows(2).enumerate() {
                let length = pair[0].distance(pair[1]);
                let steps = (length / 2.).ceil().max(1.) as usize;
                for step in 0..=steps {
                    if segment > 0 && step == 0 {
                        continue;
                    }
                    let t = step as f32 / steps as f32;
                    let along = distance + length * t;
                    let from_bank = (bank[i * 2] + along).min(bank[i * 2 + 1] + lengths[i] - along);
                    points.push(
                        pair[0]
                            .lerp(pair[1], t)
                            .with_y(GROUND + (from_bank * GRADE).min(clearance)),
                    );
                }
                distance += length;
            }
            let sides = points
                .iter()
                .enumerate()
                .map(|(i, &p)| {
                    let previous = (p - points[i.saturating_sub(1)])
                        .with_y(0.)
                        .normalize_or_zero();
                    let next = (points[(i + 1).min(points.len() - 1)] - p)
                        .with_y(0.)
                        .normalize_or_zero();
                    let a = if previous == Vec3::ZERO {
                        next
                    } else {
                        previous
                    }
                    .cross(Vec3::Y);
                    let b = if next == Vec3::ZERO { previous } else { next }.cross(Vec3::Y);
                    ((a + b) / (1. + a.dot(b)).max(0.2)).clamp_length_max(2.)
                })
                .collect();
            Some(Span {
                points,
                sides,
                left: s.width_m * 0.5 + s.sidewalk_m[0],
                right: s.width_m * 0.5 + s.sidewalk_m[1],
                road_half: s.width_m * 0.5,
                surface: s.surface,
            })
        })
        .collect()
}

#[derive(Default)]
struct Geometry {
    positions: Vec<Vec3>,
    normals: Vec<Vec3>,
    uv: Vec<Vec2>,
    indices: Vec<u32>,
}
impl Geometry {
    fn quad(&mut self, mut p: [Vec3; 4], facing: Vec3) {
        let mut n = (p[1] - p[0]).cross(p[2] - p[0]);
        if n.length_squared() < 1e-10 {
            return;
        }
        if n.dot(facing) < 0. {
            p.reverse();
            n = -n;
        }
        let start = self.positions.len() as u32;
        self.positions.extend(p);
        self.normals.extend([n.normalize(); 4]);
        self.uv.extend(p.map(|p| Vec2::new(p.x, p.z) / 2.));
        self.indices
            .extend([start, start + 1, start + 2, start, start + 2, start + 3]);
    }
    fn mesh(self) -> Mesh {
        Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::default(),
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, self.positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, self.normals)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, self.uv)
        .with_inserted_indices(Indices::U32(self.indices))
    }
}

fn strip(span: &Span, from: f32, to: f32) -> Mesh {
    let mut mesh = Geometry::default();
    for i in 0..span.points.len() - 1 {
        let a = span.points[i];
        let b = span.points[i + 1];
        let u = span.sides[i];
        let v = span.sides[i + 1];
        mesh.quad(
            [a + u * from, a + u * to, b + v * to, b + v * from],
            Vec3::Y,
        );
    }
    mesh.mesh()
}

fn beam(
    commands: &mut Commands,
    root: Entity,
    cube: &Handle<Mesh>,
    material: &Handle<StandardMaterial>,
    a: Vec3,
    b: Vec3,
    width: f32,
    height: f32,
) {
    let delta = b - a;
    if delta.length_squared() < 0.0001 {
        return;
    }
    commands.spawn((
        Mesh3d(cube.clone()),
        MeshMaterial3d(material.clone()),
        Transform::from_translation((a + b) * 0.5)
            .with_rotation(Quat::from_rotation_arc(Vec3::Z, delta.normalize()))
            .with_scale(Vec3::new(width, height, delta.length())),
        ChildOf(root),
    ));
}

pub(crate) fn rebuild(
    mut commands: Commands,
    roads: Res<Roads>,
    mut bridges: ResMut<Bridges>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    street_materials: Res<StreetMaterials>,
    mut actors: Query<(&mut Transform, Option<&Player>), Or<(With<Player>, With<ParkedVan>)>>,
) {
    if bridges.generation == roads.generation {
        return;
    }
    bridges.generation = roads.generation;
    if let Some(root) = bridges.root.take() {
        commands.entity(root).try_despawn();
    }
    let streets = roads
        .streets()
        .filter(|s| s.bridge.is_some())
        .collect::<Vec<_>>();
    let previous = Bridges {
        spans: std::mem::take(&mut bridges.spans),
        ..default()
    };
    bridges.spans = plans(&streets);
    if bridges.spans.is_empty() {
        return;
    }
    let concrete = bridges
        .concrete
        .get_or_insert_with(|| {
            materials.add(StandardMaterial {
                base_color: Color::srgb(0.52, 0.53, 0.48),
                perceptual_roughness: 0.9,
                ..default()
            })
        })
        .clone();
    let metal = bridges
        .metal
        .get_or_insert_with(|| {
            materials.add(StandardMaterial {
                base_color: Color::srgb(0.16, 0.22, 0.20),
                metallic: 0.65,
                perceptual_roughness: 0.4,
                ..default()
            })
        })
        .clone();
    let cube = meshes.add(Cuboid::new(1., 1., 1.));
    let root = commands
        .spawn((
            Transform::IDENTITY,
            Visibility::default(),
            DespawnOnExit(Mode::Ride),
        ))
        .id();
    bridges.root = Some(root);
    for span in &bridges.spans {
        let deck = strip(span, -span.left, span.right);
        let collider = Collider::trimesh_from_mesh(&deck).expect("nondegenerate bridge deck");
        commands.spawn((
            BridgeDeck,
            RigidBody::Static,
            collider,
            Transform::IDENTITY,
            ChildOf(root),
        ));
        for (from, to, material) in [
            (
                -span.road_half,
                span.road_half,
                match span.surface {
                    Surface::Asphalt => &street_materials.asphalt,
                    Surface::Paving => &street_materials.paving,
                    Surface::Gravel => &street_materials.gravel,
                },
            ),
            (-span.left, -span.road_half, &street_materials.paving),
            (span.road_half, span.right, &street_materials.paving),
        ] {
            if to - from > 0.01 {
                commands.spawn((
                    Mesh3d(meshes.add(crate::shapes::lit(strip(span, from, to)))),
                    MeshMaterial3d(material.clone()),
                    Transform::IDENTITY,
                    ChildOf(root),
                ));
            }
        }
        let mut sides = Geometry::default();
        let down = Vec3::Y * THICKNESS;
        for i in 0..span.points.len() - 1 {
            let a = span.points[i];
            let b = span.points[i + 1];
            let u = span.sides[i];
            let v = span.sides[i + 1];
            let [al, ar, bl, br] = [
                a - u * span.left,
                a + u * span.right,
                b - v * span.left,
                b + v * span.right,
            ];
            sides.quad([al, bl, bl - down, al - down], -u);
            sides.quad([ar, ar - down, br - down, br], u);
            sides.quad([al - down, bl - down, br - down, ar - down], Vec3::NEG_Y);
            for (p, q) in [(al, bl), (ar, br)] {
                beam(
                    &mut commands,
                    root,
                    &cube,
                    &concrete,
                    p + Vec3::Y * 0.1,
                    q + Vec3::Y * 0.1,
                    0.20,
                    0.20,
                );
                // A solid collision rail keeps both vans and pedestrians on the deck.
                let delta = q - p;
                commands.spawn((
                    RigidBody::Static,
                    Collider::cuboid(0.18, 1.1, delta.length()),
                    Transform::from_translation((p + q) * 0.5 + Vec3::Y * 0.55)
                        .with_rotation(Quat::from_rotation_arc(Vec3::Z, delta.normalize())),
                    ChildOf(root),
                ));
                for height in [0.5, 1.0] {
                    beam(
                        &mut commands,
                        root,
                        &cube,
                        &metal,
                        p + Vec3::Y * height,
                        q + Vec3::Y * height,
                        0.075,
                        0.08,
                    );
                }
                beam(
                    &mut commands,
                    root,
                    &cube,
                    &metal,
                    p + Vec3::Y * 0.05,
                    p + Vec3::Y * 1.04,
                    0.075,
                    0.075,
                );
            }
            if i % 9 == 4 && a.y > 1.5 {
                for offset in [-span.left * 0.65, span.right * 0.65] {
                    let top = a + u * offset - down;
                    beam(
                        &mut commands,
                        root,
                        &cube,
                        &concrete,
                        top.with_y(-0.8),
                        top,
                        0.65,
                        0.65,
                    );
                }
            }
        }
        for (i, facing) in [
            (0, span.points[0] - span.points[1]),
            (
                span.points.len() - 1,
                *span.points.last().unwrap() - span.points[span.points.len() - 2],
            ),
        ] {
            let a = span.points[i] - span.sides[i] * span.left;
            let b = span.points[i] + span.sides[i] * span.right;
            sides.quad([a, b, b - down, a - down], facing);
        }
        commands.spawn((
            Mesh3d(meshes.add(sides.mesh())),
            MeshMaterial3d(concrete.clone()),
            Transform::IDENTITY,
            ChildOf(root),
        ));
    }
    // The first tile can spawn a player on a bridge before its deck is built.
    for (mut transform, player) in &mut actors {
        let half = player.map_or(0., |p| crate::actors::half_height(p.form));
        // Streaming a neighbouring tile must not pull somebody standing
        // underneath an already loaded overpass onto its roof.
        if previous
            .height_at(transform.translation)
            .is_some_and(|height| transform.translation.y - half < height - 0.15)
        {
            continue;
        }
        if let Some(height) = bridges.height_at(transform.translation) {
            if transform.translation.y - half < height - 0.05 {
                transform.translation.y = height + half + 0.03;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use paket_geo::{
        EastNorth,
        landscape::{Area, Cover},
    };
    fn road(id: &str, from: f64, to: f64) -> Street {
        Street {
            id: id.into(),
            centerline: vec![
                EastNorth {
                    east: from,
                    north: 0.,
                },
                EastNorth {
                    east: to,
                    north: 0.,
                },
            ],
            width_m: 7.,
            sidewalk_m: [1.2, 1.2],
            name: None,
            surface: Surface::Asphalt,
            spawnable: true,
            bridge: Some(1),
        }
    }
    #[test]
    fn connected_bridge_ways_have_one_continuous_deck() {
        let a = road("a", -30., 0.);
        let b = road("b", 0., 30.);
        let spans = plans(&[&a, &b]);
        assert_eq!(spans[0].points.last().unwrap().y, spans[1].points[0].y);
        assert!(spans[1].points[0].y > 2.3);
        assert!((spans[0].points[0].y - GROUND).abs() < 0.001);
        assert!((spans[1].points.last().unwrap().y - GROUND).abs() < 0.001);
        let bridges = Bridges { spans, ..default() };
        assert!(bridges.height_at(Vec3::ZERO).is_some_and(|h| h > 2.3));
        assert!(bridges.height_at(Vec3::new(0., 0., 6.)).is_none());
    }

    #[test]
    fn bent_deck_height_matches_the_physical_surface_at_the_outer_corner() {
        let mut street = road("bend", -30., 0.);
        street.centerline.push(EastNorth {
            east: 0.,
            north: 30.,
        });
        let bridges = Bridges {
            spans: plans(&[&street]),
            ..default()
        };
        // Outer corner: beyond both straight centreline segments, but on the miter.
        assert!(
            bridges
                .height_at(Vec3::new(3., 0., 3.))
                .is_some_and(|h| h > 2.3)
        );
        assert!(bridges.height_at(Vec3::new(6., 0., 6.)).is_none());
    }

    fn driving_app(form: paket_session::Avatar) -> (App, Entity) {
        use bevy::time::TimeUpdateStrategy;
        use std::time::Duration;
        let mut seed = crate::ride_tests::headless_ride(crate::actors::Role::Courier);
        let materials = seed
            .world_mut()
            .remove_resource::<StreetMaterials>()
            .unwrap();
        let mut ride = seed.world_mut().remove_resource::<crate::Ride>().unwrap();
        for x in -1..=1 {
            for y in -1..=1 {
                ride.loaded.insert(paket_geo::TileId { x, y }, vec![]);
            }
        }
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            TransformPlugin,
            bevy::diagnostic::DiagnosticsPlugin,
            PhysicsPlugins::default(),
        ))
        .init_asset::<Mesh>()
        .init_asset::<StandardMaterial>()
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
            20,
        )))
        .init_resource::<ButtonInput<KeyCode>>()
        .init_resource::<Bridges>()
        .init_resource::<crate::jobs::Jobs>()
        .insert_resource(materials)
        .insert_resource(ride)
        .add_systems(Update, rebuild)
        .add_systems(FixedUpdate, crate::actors::drive);
        let mut roads = Roads::default();
        roads.insert(&road("bridge", -30., 30.));
        app.insert_resource(roads);
        app.insert_resource(crate::terrain::Landscape::test_water(vec![Area {
            cover: Cover::Water,
            rings: vec![vec![
                EastNorth {
                    east: -23.,
                    north: -20.,
                },
                EastNorth {
                    east: 23.,
                    north: -20.,
                },
                EastNorth {
                    east: 23.,
                    north: 20.,
                },
                EastNorth {
                    east: -23.,
                    north: 20.,
                },
            ]],
        }]));
        app.world_mut().spawn((
            RigidBody::Static,
            Collider::cuboid(200., 0.2, 100.),
            Transform::from_xyz(0., -0.14, 0.),
        ));
        let collider = if form == paket_session::Avatar::Van {
            Collider::cuboid(1.86, 1.88, 4.2)
        } else {
            Collider::capsule(0.3, 1.14)
        };
        let player = app
            .world_mut()
            .spawn((
                Player {
                    form,
                    photo: false,
                    pitch: 0.,
                    stunned_until: 0.,
                    visual: Entity::PLACEHOLDER,
                    parked: None,
                },
                crate::actors::Controller::default(),
                RigidBody::Dynamic,
                collider,
                Transform::from_xyz(-35., crate::actors::half_height(form), 0.)
                    .looking_to(Vec3::X, Vec3::Y),
                LinearVelocity::ZERO,
                AngularVelocity::ZERO,
                LockedAxes::new().lock_rotation_x().lock_rotation_z(),
                SweptCcd::default(),
                Friction {
                    combine_rule: CoefficientCombine::Min,
                    ..Friction::ZERO
                },
            ))
            .id();
        app.finish();
        app.cleanup();
        for _ in 0..4 {
            app.update();
        }
        (app, player)
    }

    #[test]
    fn van_and_courier_cross_the_water_on_the_physical_deck() {
        for form in [paket_session::Avatar::Van, paket_session::Avatar::Courier] {
            let (mut app, player) = driving_app(form);
            app.world_mut()
                .resource_mut::<ButtonInput<KeyCode>>()
                .press(KeyCode::KeyW);
            let mut peak = 0f32;
            for _ in 0..1250 {
                app.update();
                let p = app.world().get::<Transform>(player).unwrap().translation;
                peak = peak.max(p.y);
                if p.x > 34. {
                    break;
                }
            }
            let end = app.world().get::<Transform>(player).unwrap().translation;
            assert!(end.x > 34., "{form:?} stopped before crossing: {end:?}");
            assert!(
                peak > 3.2 && peak < 5.,
                "{form:?} did not follow the deck: peak {peak}"
            );
            assert!(
                end.y < 1.5,
                "{form:?} did not descend the far ramp: {end:?}"
            );
        }
    }

    #[test]
    fn guardrails_are_solid_and_water_outside_the_bridge_still_blocks() {
        use bevy::ecs::system::RunSystemOnce;
        let (mut app, player) = driving_app(paket_session::Avatar::Van);
        let rail = app
            .world_mut()
            .run_system_once(|spatial: SpatialQuery| {
                spatial.cast_ray(
                    Vec3::new(0., 3.1, 0.),
                    Dir3::Z,
                    8.,
                    true,
                    &SpatialQueryFilter::default(),
                )
            })
            .unwrap();
        assert!(rail.is_some_and(|h| h.distance > 4. && h.distance < 5.));
        app.world_mut().entity_mut(player).insert((
            Transform::from_xyz(-25., 0.94, 10.).looking_to(Vec3::X, Vec3::Y),
            Position(Vec3::new(-25., 0.94, 10.)),
        ));
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyW);
        for _ in 0..150 {
            app.update();
        }
        let end = app.world().get::<Transform>(player).unwrap().translation;
        assert!(
            end.x < -22.8,
            "van drove on water outside the deck: {end:?}"
        );
    }
}
