use std::f32::consts::{FRAC_PI_2, TAU};

use bevy::prelude::*;

#[derive(Resource)]
pub(crate) struct BikeModel(Vec<(Handle<Mesh>, Handle<StandardMaterial>)>);

impl BikeModel {
    pub(crate) fn new(meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>) -> Self {
        let mut frame = Parts::default();
        let mut hardware = Parts::default();
        let mut rubber = Parts::default();
        let mut saddle = Parts::default();
        let front = Vec3::new(0.0, 0.33, -0.59);
        let rear = Vec3::new(0.0, 0.33, 0.50);
        let crank = Vec3::new(0.0, 0.31, 0.05);
        let seat = Vec3::new(0.0, 0.82, 0.23);
        let head = Vec3::new(0.0, 0.85, -0.39);
        let fork = Vec3::new(0.0, 0.69, -0.45);

        for (a, b, radius) in [
            (seat, head, 0.022),
            (crank, fork, 0.026),
            (crank, seat, 0.021),
            (head, fork, 0.029),
        ] {
            frame.tube(a, b, radius);
        }
        for side in [-1.0, 1.0] {
            let axle = Vec3::X * 0.051 * side;
            frame.tube(crank + axle * 0.6, rear + axle, 0.012);
            frame.tube(seat, rear + axle, 0.011);
            frame.tube(fork, front + axle, 0.014);
        }

        for hub in [front, rear] {
            rubber.ring(hub, 0.286, 0.33);
            hardware.ring(hub, 0.267, 0.287);
            hardware.tube(hub - Vec3::X * 0.051, hub + Vec3::X * 0.051, 0.022);
            for spoke in 0..20 {
                let angle = TAU * spoke as f32 / 20.0;
                let (sin, cos) = angle.sin_cos();
                let rim = hub + Vec3::new(0.0, 0.272 * sin, 0.272 * cos);
                let flange = hub + Vec3::X * if spoke % 2 == 0 { -0.034 } else { 0.034 };
                hardware.tube(flange, rim, 0.0024);
            }
        }

        let stem = Vec3::new(0.0, 0.99, -0.33);
        let bar = Vec3::new(0.0, 1.02, -0.45);
        hardware.tube(head, stem, 0.015);
        hardware.tube(stem, bar, 0.017);
        hardware.tube(seat, Vec3::new(0.0, 0.94, 0.27), 0.015);
        for side in [-1.0, 1.0] {
            let bend = Vec3::new(side * 0.18, 1.02, -0.42);
            let grip = Vec3::new(side * 0.32, 1.03, -0.37);
            hardware.tube(bar, bend, 0.012);
            hardware.tube(bend, grip, 0.012);
            rubber.tube(bend.lerp(grip, 0.28), grip, 0.018);
            let pedal = crank + Vec3::new(side * 0.13, side * 0.11, 0.0);
            hardware.tube(crank + Vec3::X * side * 0.08, pedal, 0.012);
            rubber.part(
                Cuboid::new(0.083, 0.026, 0.065).mesh().build(),
                Transform::from_translation(pedal),
            );
        }
        saddle.part(
            Sphere::new(1.0).mesh().uv(20, 12),
            Transform::from_xyz(0.0, 0.954, 0.23).with_scale(Vec3::new(0.076, 0.029, 0.139)),
        );

        let surfaces = [
            (frame, Color::srgb(0.81, 0.29, 0.10), 0.22, 0.35),
            (hardware, Color::srgb(0.49, 0.52, 0.54), 0.65, 0.38),
            (rubber, Color::srgb(0.085, 0.086, 0.092), 0.0, 0.9),
            (saddle, Color::srgb(0.29, 0.16, 0.09), 0.0, 0.82),
        ];
        Self(
            surfaces
                .into_iter()
                .map(|(parts, base_color, metallic, perceptual_roughness)| {
                    (
                        meshes.add(parts.0.expect("bike material has geometry")),
                        materials.add(StandardMaterial {
                            base_color,
                            metallic,
                            perceptual_roughness,
                            ..default()
                        }),
                    )
                })
                .collect(),
        )
    }

    pub(crate) fn spawn(&self, commands: &mut Commands, parent: Entity, vertical_offset: f32) {
        for (mesh, material) in &self.0 {
            commands.spawn((
                Mesh3d(mesh.clone()),
                MeshMaterial3d(material.clone()),
                Transform::from_xyz(0.0, vertical_offset, 0.0),
                ChildOf(parent),
            ));
        }
    }
}

#[derive(Default)]
struct Parts(Option<Mesh>);

impl Parts {
    fn part(&mut self, mesh: Mesh, transform: Transform) {
        let mesh = mesh.transformed_by(transform);
        match &mut self.0 {
            Some(parts) => parts
                .merge(&mesh)
                .expect("bike primitives use the same attributes"),
            None => self.0 = Some(mesh),
        }
    }

    fn tube(&mut self, a: Vec3, b: Vec3, radius: f32) {
        let direction = b - a;
        self.part(
            Cylinder::new(radius, direction.length())
                .mesh()
                .resolution(10)
                .build(),
            Transform::from_translation((a + b) * 0.5)
                .with_rotation(Quat::from_rotation_arc(Vec3::Y, direction.normalize())),
        );
    }

    fn ring(&mut self, center: Vec3, inner: f32, outer: f32) {
        self.part(
            Torus::new(inner, outer)
                .mesh()
                .major_resolution(40)
                .minor_resolution(10)
                .build(),
            Transform::from_translation(center).with_rotation(Quat::from_rotation_z(FRAC_PI_2)),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bike_material_meshes_have_valid_indexed_triangles() {
        let mut meshes = Assets::default();
        let mut materials = Assets::default();
        let bike = BikeModel::new(&mut meshes, &mut materials);
        assert_eq!(bike.0.len(), 4);
        for (handle, _) in bike.0 {
            let mesh = meshes.get(&handle).unwrap();
            let indices = mesh.indices().unwrap();
            assert_eq!(indices.len() % 3, 0);
            assert!(indices.iter().all(|index| index < mesh.count_vertices()));
        }
    }
}
