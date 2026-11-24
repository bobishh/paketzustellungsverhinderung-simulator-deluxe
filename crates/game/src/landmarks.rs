//! City landmarks with their own silhouettes and real-world placement.
use avian3d::prelude::{Collider, RigidBody};
use bevy::{
    mesh::{MeshBuilder, VertexAttributeValues, primitives::ConicalFrustumMeshBuilder},
    prelude::*,
};
use paket_geo::{LatLon, architecture::Landmark, project};

use crate::{Mode, Player, Ride, elevation::Elevation, surfaces, to_world};

#[derive(Component)]
pub(super) struct LandmarkModel(Landmark);

const VIEW_DISTANCE: f32 = 1500.;

fn nearby(origin: LatLon, here: Vec3) -> impl Iterator<Item = (Landmark, Vec3)> {
    Landmark::ALL.into_iter().filter_map(move |tower| {
        let point = to_world(project(origin, tower.location()), 0.);
        (point.xz().distance_squared(here.xz()) <= VIEW_DISTANCE.powi(2)).then_some((tower, point))
    })
}

pub(super) fn update(
    mut commands: Commands,
    ride: Res<Ride>,
    elevation: Res<Elevation>,
    player: Query<&Transform, With<Player>>,
    mut landmarks: Query<(Entity, &LandmarkModel, &mut Transform), Without<Player>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    if !elevation.ready() {
        return;
    }
    let here = player
        .single()
        .map_or(Vec3::ZERO, |transform| transform.translation);
    let mut loaded = [false; Landmark::ALL.len()];
    for (entity, tower, mut transform) in &mut landmarks {
        if transform.translation.xz().distance_squared(here.xz()) > (VIEW_DISTANCE + 200.).powi(2) {
            commands.entity(entity).despawn();
            continue;
        }
        let height = elevation.heights.at(transform.translation);
        if transform.translation.y != height {
            transform.translation.y = height;
        }
        loaded[tower.0 as usize] = true;
    }
    for (tower, point) in nearby(ride.origin, here) {
        if loaded[tower as usize] {
            continue;
        }
        let mut model = Form::default();
        let foundations = match tower {
            Landmark::EiffelTower => eiffel(&mut model),
            Landmark::BerlinTvTower => berlin(&mut model),
        };
        let unlit = cfg!(target_arch = "wasm32")
            && !crate::platform::arguments()
                .iter()
                .any(|arg| arg == "--press-shot");
        commands.spawn((
            LandmarkModel(tower),
            DespawnOnExit(Mode::Ride),
            Mesh3d(meshes.add(model.0.expect("landmark mesh has geometry"))),
            MeshMaterial3d(materials.add(StandardMaterial {
                unlit,
                ..surfaces::matte(Color::WHITE, 0.84)
            })),
            RigidBody::Static,
            Collider::compound(foundations),
            Transform::from_translation(point.with_y(elevation.heights.at(point))),
        ));
    }
}

type Foundations = Vec<(Vec3, Quat, Collider)>;

#[derive(Default)]
struct Form(Option<Mesh>);

impl Form {
    fn part(&mut self, mut mesh: Mesh, transform: Transform, rgb: [u8; 3]) {
        mesh.transform_by(transform);
        let color = Color::srgb_u8(rgb[0], rgb[1], rgb[2]).to_linear();
        let Some(VertexAttributeValues::Float32x3(normals)) =
            mesh.attribute(Mesh::ATTRIBUTE_NORMAL)
        else {
            unreachable!("landmark primitives have normals");
        };
        // Subtle directional paint shading also reads on the lightly rendered web path.
        let incoming = Vec3::new(0.35, 0.8, 0.48).normalize();
        let colors: Vec<[f32; 4]> = normals
            .iter()
            .map(|&normal| {
                let shade = 0.72 + 0.28 * Vec3::from_array(normal).dot(incoming).max(0.);
                [
                    color.red * shade,
                    color.green * shade,
                    color.blue * shade,
                    1.,
                ]
            })
            .collect();
        mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
        if let Some(existing) = &mut self.0 {
            existing
                .merge(&mesh)
                .expect("landmark primitives share attributes");
        } else {
            self.0 = Some(mesh);
        }
    }

    fn block(&mut self, center: Vec3, size: Vec3, rotation: Quat, color: [u8; 3]) {
        self.part(
            Cuboid::from_size(size).into(),
            Transform::from_translation(center).with_rotation(rotation),
            color,
        );
    }

    fn beam(&mut self, a: Vec3, b: Vec3, width: f32, color: [u8; 3]) {
        let axis = b - a;
        self.block(
            (a + b) * 0.5,
            Vec3::new(width, axis.length(), width),
            Quat::from_rotation_arc(Vec3::Y, axis.normalize()),
            color,
        );
    }

    fn taper(&mut self, low: f32, high: f32, bottom: f32, top: f32, color: [u8; 3]) {
        self.part(
            ConicalFrustumMeshBuilder::new(top, bottom, high - low, 24).build(),
            Transform::from_xyz(0., (low + high) * 0.5, 0.),
            color,
        );
    }
}

fn eiffel(form: &mut Form) -> Foundations {
    let iron = [143, 122, 93];
    let brace = [128, 109, 85];
    let deck = [112, 95, 74];
    let rotation = Quat::from_rotation_y(45_f32.to_radians());
    let corner = |angle: usize, edge: f32, height: f32| {
        let x = if angle == 0 || angle == 3 {
            edge
        } else {
            -edge
        };
        let z = if angle < 2 { edge } else { -edge };
        rotation * Vec3::new(x, height, z)
    };
    let levels = [
        (0., 51.5, 11.),
        (24., 38., 9.),
        (57., 27., 6.8),
        (82., 20.6, 5.2),
        (115., 15.3, 4.0),
        (147., 11.0, 3.4),
        (182., 7.9, 2.7),
        (219., 5.7, 2.0),
        (250., 4.5, 1.65),
        (276., 4.2, 1.4),
        (300., 2.4, 1.15),
    ];
    let mut foundations = Vec::new();
    for side in 0..4 {
        let next = (side + 1) % 4;
        let foot = corner(side, 51.5, -1.);
        form.block(foot, Vec3::new(22., 5., 22.), rotation, [174, 161, 138]);
        foundations.push((foot, rotation, Collider::cuboid(22., 5., 22.)));
        for (index, pair) in levels.windows(2).enumerate() {
            let [(y0, r0, t0), (y1, r1, t1)] = [pair[0], pair[1]];
            let a = corner(side, r0, y0);
            let b = corner(side, r1, y1);
            form.beam(a, b, (t0 + t1) * 0.5, iron);
            // Below the first platform the four legs are independent.
            if index >= 2 {
                let c = corner(next, r0, y0);
                let d = corner(next, r1, y1);
                form.beam(b, d, t1 * 0.42, iron);
                form.beam(a, d, t1 * 0.36, brace);
                form.beam(c, b, t1 * 0.36, brace);
            } else {
                // Lattice within the leg, leaving the space below the arch open.
                let inward = -a.with_y(0.).normalize();
                let diagonals = [a, b, a + inward * (t0 * 0.65), b + inward * (t1 * 0.65)];
                form.beam(diagonals[2], diagonals[1], 1.4, brace);
                form.beam(diagonals[0], diagonals[3], 1.4, brace);
            }
        }
        let left = corner(side, 39., 21.);
        let right = corner(next, 39., 21.);
        let arch = [0., 0.16, 0.32, 0.5, 0.68, 0.84, 1.].map(|t| {
            left.lerp(right, t)
                .with_y(21. + (t * std::f32::consts::PI).sin() * 26.)
        });
        for points in arch.windows(2) {
            form.beam(points[0], points[1], 1.8, iron);
            let top = corner(side, 29., 53.).lerp(
                corner(next, 29., 53.),
                (points[0] - left).dot(right - left) / left.distance_squared(right),
            );
            form.beam(points[0], top, 0.48, brace);
        }
    }
    for (height, width, thickness) in [(57., 60., 4.5), (115., 36., 3.7), (276., 15., 5.8)] {
        form.block(
            Vec3::Y * height,
            Vec3::new(width, thickness, width),
            rotation,
            deck,
        );
        form.block(
            Vec3::Y * (height + thickness * 0.5),
            Vec3::new(width + 1.5, 0.9, width + 1.5),
            rotation,
            iron,
        );
    }
    form.taper(292., 310., 3.3, 1.45, deck);
    form.taper(310., 324., 1.45, 0.7, iron);
    form.taper(324., 330., 0.7, 0.33, deck);
    foundations
}

fn berlin(form: &mut Form) -> Foundations {
    let concrete = [210, 203, 185];
    let steel = [191, 200, 200];
    let glass = [73, 90, 97];
    let red = [171, 68, 54];
    let white = [220, 217, 201];
    form.taper(-1., 26., 8.0, 6.9, concrete);
    form.taper(26., 200., 6.9, 4.45, concrete);
    form.taper(200., 250., 4.45, 3.2, concrete);
    form.part(
        Sphere::new(16.)
            .mesh()
            .ico(4)
            .expect("supported sphere subdivision"),
        Transform::from_xyz(0., 216., 0.),
        steel,
    );
    // Wrap the observation deck and café windows around the steel sphere.
    for (bottom, top, bottom_radius, top_radius) in
        [(204.5, 208.0, 12.1, 14.3), (211.0, 214.0, 15.55, 16.08)]
    {
        form.taper(bottom, top, bottom_radius, top_radius, glass);
    }
    form.taper(249., 268., 3.3, 2.05, steel);
    for (i, pair) in [268., 281., 294., 307., 320., 333., 346., 358., 368.]
        .windows(2)
        .enumerate()
    {
        let radius = |y| 2.05 - (y - 268.) * 0.015;
        form.taper(
            pair[0],
            pair[1],
            radius(pair[0]),
            radius(pair[1]),
            if i % 2 == 0 { red } else { white },
        );
    }

    vec![(Vec3::Y * 10., Quat::IDENTITY, Collider::cylinder(7.8, 22.))]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_tower_appears_at_its_site_and_reaches_its_real_height() {
        for (tower, height) in [
            (Landmark::EiffelTower, 330.),
            (Landmark::BerlinTvTower, 368.),
        ] {
            assert_eq!(
                nearby(tower.location(), Vec3::ZERO).collect::<Vec<_>>(),
                [(tower, Vec3::ZERO)]
            );
            let mut model = Form::default();
            let foundations = match tower {
                Landmark::EiffelTower => eiffel(&mut model),
                Landmark::BerlinTvTower => berlin(&mut model),
            };
            let mesh = model.0.unwrap();
            let Some(VertexAttributeValues::Float32x3(vertices)) =
                mesh.attribute(Mesh::ATTRIBUTE_POSITION)
            else {
                panic!("landmark has no positions");
            };
            let top = vertices.iter().map(|p| p[1]).fold(0_f32, f32::max);
            assert!((height - top).abs() < 0.01);
            assert!(mesh.count_vertices() < 10_000);
            assert!(!foundations.is_empty());
        }
        assert_eq!(
            nearby(
                LatLon {
                    lat: 40.732,
                    lon: -73.996
                },
                Vec3::ZERO
            )
            .count(),
            0
        );
    }
}
