use bevy::prelude::*;
use paket_geo::Street;

use crate::{
    actors::{Hip, Player, WalkCycle},
    to_world,
};

#[derive(Component)]
pub(crate) struct Dog {
    start: Vec3,
    forward: Vec3,
    length: f32,
    distance: f32,
    direction: f32,
}

pub(crate) struct DogKit {
    sphere: Handle<Mesh>,
    cream: Handle<StandardMaterial>,
    brown: Handle<StandardMaterial>,
    dark: Handle<StandardMaterial>,
    collar: Handle<StandardMaterial>,
}

impl DogKit {
    pub(crate) fn new(
        meshes: &mut Assets<Mesh>,
        materials: &mut Assets<StandardMaterial>,
        images: &mut Assets<Image>,
    ) -> Self {
        use crate::surfaces::{Surface, material, matte};
        Self {
            sphere: meshes.add(crate::shapes::lit(Sphere::new(1.0).mesh().uv(24, 16))),
            cream: materials.add(material(
                images,
                Surface::Fur,
                Color::srgb(0.94, 0.92, 0.86),
            )),
            brown: materials.add(material(
                images,
                Surface::Fur,
                Color::srgb(0.49, 0.27, 0.13),
            )),
            dark: materials.add(matte(Color::srgb(0.045, 0.032, 0.025), 0.30)),
            collar: materials.add(matte(Color::srgb(0.25, 0.34, 0.32), 0.76)),
        }
    }

    fn part(
        &self,
        material: &Handle<StandardMaterial>,
        position: Vec3,
        scale: Vec3,
    ) -> (Mesh3d, MeshMaterial3d<StandardMaterial>, Transform) {
        (
            Mesh3d(self.sphere.clone()),
            MeshMaterial3d(material.clone()),
            Transform::from_translation(position).with_scale(scale),
        )
    }

    pub(crate) fn spawn(&self, commands: &mut Commands, street: &Street, parent: Entity) {
        let hash = street.id.bytes().fold(2166136261u32, |hash, byte| {
            (hash ^ u32::from(byte)).wrapping_mul(16777619)
        });
        let [left, right] = street.sidewalk_m;
        if !hash.is_multiple_of(3) || left.max(right) <= 0.0 {
            return;
        }
        let Some((a, b, length)) = street
            .centerline
            .windows(2)
            .map(|pair| {
                let a = to_world(pair[0], 0.045);
                let b = to_world(pair[1], 0.045);
                (a, b, a.distance(b))
            })
            .filter(|(_, _, length)| *length >= 10.0)
            .max_by(|a, b| a.2.total_cmp(&b.2))
        else {
            return;
        };
        let forward = (b - a) / length;
        let side = Vec3::new(-forward.z, 0.0, forward.x);
        let offset = if right > 0.0 && (left <= 0.0 || hash & 1 == 0) {
            street.width_m * 0.5 + right * 0.5
        } else {
            -(street.width_m * 0.5 + left * 0.5)
        };
        let patrol_length = (length - 4.0).min(16.0);
        let start = a + forward * ((length - patrol_length) * 0.5) + side * offset;
        let distance = patrol_length * (0.2 + (hash % 61) as f32 / 100.0);
        let dog = commands
            .spawn((
                Dog {
                    start,
                    forward,
                    length: patrol_length,
                    distance,
                    direction: 1.0,
                },
                Transform::from_translation(start + forward * distance)
                    .with_rotation(facing(forward)),
                Visibility::default(),
                ChildOf(parent),
            ))
            .id();
        self.model(commands, dog);
    }

    pub(crate) fn model(&self, commands: &mut Commands, root: Entity) {
        commands
            .entity(root)
            .insert(WalkCycle::default())
            .with_children(|dog| {
                // Parson Russell proportions: high shoulders, long legs, tapered muzzle.
                for (material, position, size) in [
                    (
                        &self.cream,
                        Vec3::new(0., 0.36, 0.035),
                        Vec3::new(0.129, 0.128, 0.273),
                    ),
                    (
                        &self.cream,
                        Vec3::new(0., 0.345, -0.150),
                        Vec3::new(0.126, 0.151, 0.138),
                    ),
                    (
                        &self.cream,
                        Vec3::new(0., 0.348, 0.195),
                        Vec3::new(0.111, 0.112, 0.125),
                    ),
                    (
                        &self.brown,
                        Vec3::new(0.042, 0.437, 0.126),
                        Vec3::new(0.085, 0.055, 0.103),
                    ),
                    (
                        &self.cream,
                        Vec3::new(0., 0.444, -0.231),
                        Vec3::new(0.086, 0.113, 0.100),
                    ),
                    (
                        &self.collar,
                        Vec3::new(0., 0.457, -0.254),
                        Vec3::new(0.092, 0.084, 0.035),
                    ),
                    (
                        &self.brown,
                        Vec3::new(0., 0.531, -0.310),
                        Vec3::new(0.095, 0.097, 0.092),
                    ),
                    (
                        &self.cream,
                        Vec3::new(0., 0.513, -0.383),
                        Vec3::new(0.044, 0.068, 0.078),
                    ),
                    (
                        &self.cream,
                        Vec3::new(0., 0.486, -0.400),
                        Vec3::new(0.065, 0.045, 0.068),
                    ),
                    (
                        &self.dark,
                        Vec3::new(0., 0.500, -0.467),
                        Vec3::new(0.031, 0.022, 0.020),
                    ),
                ] {
                    dog.spawn(self.part(material, position, size));
                }
                let (mesh, material, tail) = self.part(
                    &self.cream,
                    Vec3::new(0., 0.464, 0.287),
                    Vec3::new(0.026, 0.116, 0.027),
                );
                dog.spawn((
                    mesh,
                    material,
                    Tail,
                    tail.with_rotation(Quat::from_rotation_x(0.59)),
                ));
                // A white blaze and a collar tag distinguish the face from the muzzle.
                dog.spawn(self.part(
                    &self.cream,
                    Vec3::new(0., 0.574, -0.372),
                    Vec3::new(0.021, 0.050, 0.017),
                ));
                dog.spawn(self.part(
                    &self.collar,
                    Vec3::new(0., 0.384, -0.287),
                    Vec3::new(0.019, 0.025, 0.008),
                ));
                for side in [-1.0, 1.0] {
                    let (mesh, material, ear) = self.part(
                        &self.brown,
                        Vec3::new(side * 0.082, 0.545, -0.294),
                        Vec3::new(0.035, 0.064, 0.046),
                    );
                    dog.spawn((
                        mesh,
                        material,
                        ear.with_rotation(
                            Quat::from_rotation_z(side * 0.63) * Quat::from_rotation_x(-0.51),
                        ),
                    ));
                    dog.spawn(self.part(
                        &self.dark,
                        Vec3::new(side * 0.061, 0.552, -0.373),
                        Vec3::new(0.013, 0.013, 0.013),
                    ));
                    dog.spawn(self.part(
                        &self.cream,
                        Vec3::new(side * 0.062, 0.557, -0.382),
                        Vec3::splat(0.0038),
                    ));
                    for end in [-1.0, 1.0] {
                        dog.spawn((
                            Hip(side * end),
                            Transform::from_xyz(side * 0.083, 0.34, end * 0.174),
                            Visibility::default(),
                        ))
                        .with_children(|leg| {
                            leg.spawn(self.part(
                                &self.cream,
                                Vec3::new(0.0, -0.086, 0.0),
                                Vec3::new(0.036, 0.103, 0.043),
                            ));
                            leg.spawn(self.part(
                                &self.cream,
                                Vec3::new(0.0, -0.223, 0.006),
                                Vec3::new(0.025, 0.100, 0.028),
                            ));
                            leg.spawn(self.part(
                                &self.cream,
                                Vec3::new(0.0, -0.313, -0.013),
                                Vec3::new(0.037, 0.028, 0.051),
                            ));
                        });
                    }
                }
            });
    }
}

fn facing(direction: Vec3) -> Quat {
    Quat::from_rotation_y((-direction.x).atan2(-direction.z))
}

pub(crate) fn walk(
    time: Res<Time>,
    player: Query<&Transform, (With<Player>, Without<Dog>)>,
    mut dogs: Query<(&mut Dog, &mut Transform)>,
) {
    let rider = player.single().ok().map(|bike| bike.translation);
    let dt = time.delta_secs();
    for (mut dog, mut transform) in &mut dogs {
        let mut speed = 0.85;
        if let Some(rider) = rider {
            let away = (transform.translation - rider).with_y(0.0);
            if away.length_squared() < 3.5_f32.powi(2) {
                let escape = away.dot(dog.forward);
                if escape.abs() > 0.35 {
                    dog.direction = escape.signum();
                }
                speed = 1.8;
            }
        }
        let next = (dog.distance + dog.direction * speed * dt).clamp(0.0, dog.length);
        dog.distance = next;
        transform.translation = dog.start + dog.forward * dog.distance;
        transform.rotation = transform.rotation.slerp(
            facing(dog.forward * dog.direction),
            1.0 - (-14.0 * dt).exp(),
        );
        if (next == 0.0 || next == dog.length) && speed < 1.0 {
            dog.direction = if next == 0.0 { 1.0 } else { -1.0 };
        }
    }
}

#[derive(Component)]
pub(crate) struct Tail;

pub(crate) fn animate_tails(
    time: Res<Time>,
    mut tails: Query<(&mut Transform, Entity), With<Tail>>,
) {
    for (mut tail, entity) in &mut tails {
        let wag = (time.elapsed_secs() * 8.0 + (entity.to_bits() % 97) as f32).sin() * 0.25;
        tail.rotation = Quat::from_rotation_z(wag) * Quat::from_rotation_x(0.59);
    }
}
