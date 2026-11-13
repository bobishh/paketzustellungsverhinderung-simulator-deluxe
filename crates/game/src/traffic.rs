//! Small neighbourhood traffic: vehicles yield to obstacles and pedestrians use the edges.
use crate::{
    Mode,
    actors::{Models, ParkedVan, Player},
    jobs::Jobs,
    roads::Roads,
    shift::Shift,
};
use avian3d::prelude::*;
use bevy::prelude::*;
use std::collections::HashSet;
#[derive(Component)]
pub(crate) struct Car;
#[derive(Component)]
pub(crate) struct Pedestrian;
#[derive(Component)]
pub(crate) struct Traffic {
    path: Vec<Vec3>,
    next: usize,
    direction: isize,
    pedestrian: bool,
    stopped: f32,
    awarded: bool,
    day: u64,
}
#[derive(Resource)]
pub(crate) struct TrafficWorld {
    seen: HashSet<String>,
    cars: usize,
    people: usize,
    cube: Handle<Mesh>,
    wheel: Handle<Mesh>,
    paint: Vec<Handle<StandardMaterial>>,
    rubber: Handle<StandardMaterial>,
    glass: Handle<StandardMaterial>,
    lights: Handle<StandardMaterial>,
    brakes: Handle<StandardMaterial>,
}
impl TrafficWorld {
    pub fn new(meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>) -> Self {
        let mut material = |color| {
            materials.add(StandardMaterial {
                base_color: color,
                perceptual_roughness: 0.6,
                ..default()
            })
        };
        Self {
            seen: HashSet::new(),
            cars: 0,
            people: 0,
            cube: meshes.add(crate::shapes::lit(Cuboid::new(1., 1., 1.).mesh().build())),
            wheel: meshes.add(crate::shapes::lit(
                Cylinder::new(0.32, 0.20).mesh().resolution(16).build(),
            )),
            paint: vec![
                material(Color::srgb(0.55, 0.15, 0.12)),
                material(Color::srgb(0.2, 0.32, 0.39)),
                material(Color::srgb(0.79, 0.76, 0.64)),
                material(Color::srgb(0.22, 0.25, 0.23)),
            ],
            rubber: material(Color::srgb(0.035, 0.04, 0.04)),
            glass: material(Color::srgb(0.12, 0.21, 0.23)),
            lights: material(Color::srgb(1., 0.88, 0.59)),
            brakes: material(Color::srgb(0.85, 0.08, 0.025)),
        }
    }
    fn car(&self, commands: &mut Commands, root: Entity, index: usize) {
        let paint = &self.paint[index % self.paint.len()];
        for (p, size, m) in [
            (Vec3::ZERO, Vec3::new(1.7, 0.65, 3.8), paint),
            (
                Vec3::new(0., 0.48, 0.15),
                Vec3::new(1.48, 0.65, 1.75),
                &self.glass,
            ),
            (Vec3::new(0., 0.84, 0.2), Vec3::new(1.5, 0.07, 1.8), paint),
            (
                Vec3::new(0., -0.2, -1.94),
                Vec3::new(1.75, 0.18, 0.12),
                &self.rubber,
            ),
            (
                Vec3::new(0., -0.2, 1.94),
                Vec3::new(1.75, 0.18, 0.12),
                &self.rubber,
            ),
        ] {
            commands.spawn((
                Mesh3d(self.cube.clone()),
                MeshMaterial3d(m.clone()),
                Transform::from_translation(p).with_scale(size),
                ChildOf(root),
            ));
        }
        for x in [-0.86, 0.86] {
            for z in [-1.25, 1.25] {
                commands.spawn((
                    Mesh3d(self.wheel.clone()),
                    MeshMaterial3d(self.rubber.clone()),
                    Transform::from_xyz(x, -0.4, z)
                        .with_rotation(Quat::from_rotation_z(std::f32::consts::FRAC_PI_2)),
                    ChildOf(root),
                ));
            }
        }
        for x in [-0.55, 0.55] {
            for (z, m) in [(-1.93, &self.lights), (1.93, &self.brakes)] {
                commands.spawn((
                    Mesh3d(self.cube.clone()),
                    MeshMaterial3d(m.clone()),
                    Transform::from_xyz(x, 0.02, z).with_scale(Vec3::new(0.38, 0.22, 0.06)),
                    ChildOf(root),
                ));
            }
        }
    }
}
fn path(points: &[paket_geo::EastNorth], offset: f32, y: f32) -> Vec<Vec3> {
    let world = points
        .iter()
        .map(|p| Vec3::new(p.east as f32, y, -p.north as f32))
        .collect::<Vec<_>>();
    world
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let a = world[i.saturating_sub(1)];
            let b = world[(i + 1).min(world.len() - 1)];
            let right = (b - a).normalize_or_zero().cross(Vec3::Y);
            *p + right * offset
        })
        .collect()
}
pub(crate) fn spawn(
    mut commands: Commands,
    roads: Res<Roads>,
    mut world: ResMut<TrafficWorld>,
    models: Res<Models>,
    player: Query<&Transform, With<Player>>,
    shift: Res<Shift>,
) {
    let Ok(player) = player.single() else {
        return;
    };
    for street in roads.streets() {
        if street.centerline.len() < 2 || world.seen.contains(&street.id) {
            continue;
        }
        let route = path(&street.centerline, 0., 0.8);
        if route.windows(2).map(|w| w[0].distance(w[1])).sum::<f32>() < 35.
            || route.iter().all(|p| p.distance(player.translation) > 230.)
        {
            continue;
        }
        world.seen.insert(street.id.clone());
        for pedestrian in [false, true] {
            if (pedestrian && world.people >= 20)
                || (!pedestrian && (world.cars >= 12 || !street.spawnable || street.width_m < 4.5))
            {
                continue;
            }
            let offset = if pedestrian {
                if street.bridge.is_some() {
                    (street.width_m * 0.5 + street.sidewalk_m[1] - 0.45).max(0.)
                } else {
                    street.width_m * 0.5 + 0.8
                }
            } else {
                street.width_m * 0.22
            };
            let points = path(
                &street.centerline,
                offset,
                if pedestrian { 0.9 } else { 0.8 },
            );
            let start = points[0].lerp(points[1], if pedestrian { 0.2 } else { 0.4 });
            let rotation = Transform::IDENTITY
                .looking_to(points[1] - points[0], Vec3::Y)
                .rotation;
            let collider = if pedestrian {
                Collider::cuboid(0.55, 1.5, 0.55)
            } else {
                Collider::cuboid(1.8, 1.05, 3.9)
            };
            let root = commands
                .spawn((
                    Traffic {
                        path: points,
                        next: 1,
                        direction: 1,
                        pedestrian,
                        stopped: 0.,
                        awarded: false,
                        day: shift.day(),
                    },
                    Transform::from_translation(start).with_rotation(rotation),
                    Visibility::default(),
                    RigidBody::Kinematic,
                    collider,
                    LinearVelocity::ZERO,
                    DespawnOnExit(Mode::Ride),
                ))
                .id();
            if pedestrian {
                commands.entity(root).insert(Pedestrian);
                let visual = commands
                    .spawn((
                        Transform::from_xyz(0., -0.9, 0.),
                        Visibility::default(),
                        ChildOf(root),
                    ))
                    .id();
                models.person(&mut commands, visual, false);
                world.people += 1;
            } else {
                commands.entity(root).insert(Car);
                world.car(&mut commands, root, world.cars);
                world.cars += 1;
            }
        }
    }
}
fn obstruction_bonus(stopped: f32, awarded: bool, delivering: bool, own_van: bool) -> bool {
    stopped >= 4. && !awarded && delivering && own_van
}
pub(crate) fn drive(
    time: Res<Time>,
    shift: Res<Shift>,
    mut jobs: ResMut<Jobs>,
    spatial: SpatialQuery,
    player: Query<&Player>,
    vans: Query<Entity, With<ParkedVan>>,
    bridges: Option<Res<crate::bridges::Bridges>>,
    decks: Query<Entity, With<crate::bridges::BridgeDeck>>,
    mut traffic: Query<(Entity, &mut Traffic, &mut Transform, &mut LinearVelocity)>,
) {
    let van = player
        .single()
        .ok()
        .filter(|p| p.form == paket_session::Avatar::Courier)
        .and_then(|p| p.parked)
        .filter(|e| vans.contains(*e));
    for (entity, mut actor, mut transform, mut velocity) in &mut traffic {
        velocity.0 = Vec3::ZERO;
        if let Some(bridges) = &bridges {
            let ground = bridges.height_at(transform.translation).unwrap_or(0.);
            transform.translation.y = ground + if actor.pedestrian { 0.9 } else { 0.8 };
        }
        if shift.ended() {
            continue;
        }
        if actor.day != shift.day() {
            actor.day = shift.day();
            actor.awarded = false;
            actor.stopped = 0.;
        }
        if (transform.translation - actor.path[actor.next])
            .with_y(0.)
            .length()
            < 1.2
        {
            let next = actor.next as isize + actor.direction;
            if next < 0 || next >= actor.path.len() as isize {
                actor.direction = -actor.direction;
            }
            actor.next = (actor.next as isize + actor.direction) as usize;
        }
        let direction = (actor.path[actor.next] - transform.translation)
            .with_y(0.)
            .normalize_or_zero();
        let Ok(dir) = Dir3::new(direction) else {
            continue;
        };
        let speed = if actor.pedestrian { 1.35 } else { 5.5 };
        let shape = if actor.pedestrian {
            Collider::cuboid(0.55, 1.45, 0.55)
        } else {
            Collider::cuboid(1.85, 1., 4.)
        };
        let target = Transform::IDENTITY.looking_to(direction, Vec3::Y).rotation;
        let filter =
            SpatialQueryFilter::from_excluded_entities(std::iter::once(entity).chain(decks.iter()));
        let hit = spatial.cast_shape(
            &shape,
            transform.translation,
            target,
            dir,
            &ShapeCastConfig {
                max_distance: if actor.pedestrian { 0.7 } else { 3.2 },
                ..default()
            },
            &filter,
        );
        if let Some(hit) = hit {
            actor.stopped += time.delta_secs();
            if obstruction_bonus(
                actor.stopped,
                actor.awarded,
                jobs.delivering(),
                Some(hit.entity) == van,
            ) {
                jobs.traffic_bonus(actor.pedestrian);
                actor.awarded = true;
            }
            if actor.pedestrian && actor.stopped > 1. {
                // Take a short sidestep only when it is clear, rather than walking through a van.
                let side = direction.cross(Vec3::Y);
                let side_dir = Dir3::new(side).unwrap();
                if spatial
                    .cast_shape(
                        &shape,
                        transform.translation,
                        target,
                        side_dir,
                        &ShapeCastConfig {
                            max_distance: 0.8,
                            ..default()
                        },
                        &filter,
                    )
                    .is_none()
                {
                    velocity.0 = side * 0.7;
                }
            }
        } else {
            actor.stopped = 0.;
            velocity.0 = direction * speed;
        }
        transform.rotation = transform
            .rotation
            .slerp(target, 1. - (-5. * time.delta_secs()).exp());
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn a_car_waits_for_the_parked_van_scores_once_and_resumes_when_clear() {
        use bevy::time::TimeUpdateStrategy;
        use std::time::Duration;
        let mut ride = crate::ride_tests::headless_ride(crate::actors::Role::Courier);
        crate::ride_tests::press(&mut ride, KeyCode::KeyE);
        let jobs = ride.world_mut().remove_resource::<Jobs>().unwrap();
        assert!(jobs.delivering());
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            TransformPlugin,
            bevy::diagnostic::DiagnosticsPlugin,
            PhysicsPlugins::default(),
        ))
        .init_asset::<Mesh>()
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
            20,
        )))
        .insert_resource(jobs)
        .init_resource::<Shift>()
        .add_systems(FixedUpdate, drive);
        let van = app
            .world_mut()
            .spawn((
                ParkedVan,
                RigidBody::Static,
                Collider::cuboid(2., 2., 5.),
                Transform::from_xyz(0., 1., -5.),
            ))
            .id();
        app.world_mut().spawn(Player {
            form: paket_session::Avatar::Courier,
            photo: false,
            pitch: 0.,
            stunned_until: 0.,
            visual: Entity::PLACEHOLDER,
            parked: Some(van),
        });
        let car = app
            .world_mut()
            .spawn((
                Traffic {
                    path: vec![Vec3::new(0., 0.8, 0.), Vec3::new(0., 0.8, -100.)],
                    next: 1,
                    direction: 1,
                    pedestrian: false,
                    stopped: 0.,
                    awarded: false,
                    day: 1,
                },
                RigidBody::Kinematic,
                Collider::cuboid(1.8, 1.05, 3.9),
                Transform::from_xyz(0., 0.8, 0.),
                LinearVelocity::ZERO,
            ))
            .id();
        app.finish();
        app.cleanup();
        for _ in 0..300 {
            app.update();
        }
        assert!(app.world().get::<Traffic>(car).unwrap().awarded);
        assert_eq!(app.world().resource::<Jobs>().progress()["score"], 25);
        assert!(app.world().get::<Transform>(car).unwrap().translation.z > -0.5);
        for _ in 0..100 {
            app.update();
        }
        assert_eq!(app.world().resource::<Jobs>().progress()["score"], 25);
        app.world_mut().entity_mut(van).despawn();
        for _ in 0..100 {
            app.update();
        }
        assert!(app.world().get::<Transform>(car).unwrap().translation.z < -5.);
    }
    #[test]
    fn obstruction_only_scores_a_real_delivery_once() {
        assert!(!obstruction_bonus(3.9, false, true, true));
        assert!(!obstruction_bonus(5., false, false, true));
        assert!(!obstruction_bonus(5., false, true, false));
        assert!(!obstruction_bonus(5., true, true, true));
        assert!(obstruction_bonus(5., false, true, true));
    }
}
