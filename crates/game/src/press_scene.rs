//! Reproducible in-engine press still, using the same meshes and materials as play.
use crate::{
    Mode, Ride, StreetMaterials,
    actors::{HoldingParcel, Models, PARCEL_GRIP, ParkedVan, Player},
    elevation::Elevation,
    traffic::TrafficWorld,
};
use bevy::prelude::*;
use paket_geo::{FIXTURE_LAT, FIXTURE_LON, TileId};

// The west side of Chamissoplatz, facing the mapped playground.
const CHAMISSOPLATZ: Vec3 = Vec3::new(-9.2, 0., 158.);

#[derive(Resource, Default)]
pub(crate) struct Scene {
    base: Option<(Vec3, Quat)>,
    pub(crate) ready: bool,
}
pub(crate) fn install(app: &mut App) {
    app.init_resource::<Scene>().add_systems(
        PostUpdate,
        compose
            .before(bevy::ui::UiSystems::Prepare)
            .before(bevy::transform::TransformSystems::Propagate)
            .run_if(in_state(Mode::Ride)),
    );
}
fn compose(
    mut commands: Commands,
    mut scene: ResMut<Scene>,
    models: Res<Models>,
    materials: Res<StreetMaterials>,
    traffic: Res<TrafficWorld>,
    elevation: Res<Elevation>,
    ride: Res<Ride>,
    player: Query<(&Player, &Transform), Without<Camera3d>>,
    mut cameras: Query<(&mut Transform, &mut Projection), (With<Camera3d>, Without<Player>)>,
    mut visible: Query<&mut Visibility>,
    mut ui: Query<&mut Node>,
    vans: Query<Entity, With<ParkedVan>>,
    assets: Res<AssetServer>,
    roots: Query<&WorldAssetRoot>,
) {
    scene.ready = false;
    let Ok((player, actor)) = player.single() else {
        return;
    };
    if let Ok(mut v) = visible.get_mut(player.visual) {
        *v = Visibility::Hidden;
    }
    for van in &vans {
        if let Ok(mut v) = visible.get_mut(van) {
            *v = Visibility::Hidden;
        }
    }
    for mut node in &mut ui {
        node.display = Display::None;
    }
    let at_chamissoplatz = ride.origin.lat == FIXTURE_LAT && ride.origin.lon == FIXTURE_LON;
    if at_chamissoplatz
        && (!ride.loaded.contains_key(&TileId { x: 0, y: -1 }) || !elevation.ready())
    {
        return;
    }
    let (base, rotation) = *scene.base.get_or_insert_with(|| {
        let ground = |point: Vec3| point.with_y(elevation.heights.at(point));
        let (base, rotation) = if at_chamissoplatz {
            dress_chamissoplatz(&mut commands, &models, &traffic, ground);
            (ground(CHAMISSOPLATZ), Quat::from_rotation_y(1.28))
        } else {
            (
                ground(actor.translation + actor.forward().as_vec3() * 3.),
                actor.rotation,
            )
        };
        let forward = rotation * Vec3::NEG_Z;
        let right = rotation * Vec3::X;
        let courier_heading = rotation * Quat::from_rotation_y(0.91);
        let courier_position =
            ground(base + right * 0.58) + Vec3::Y * crate::roads::surface_rise(&elevation.heights);
        let courier = commands
            .spawn((
                Transform::from_translation(courier_position).with_rotation(courier_heading),
                Visibility::default(),
                HoldingParcel,
                DespawnOnExit(Mode::Ride),
            ))
            .id();
        models.citizens.courier.spawn(&mut commands, courier);
        let box_root = commands
            .spawn((
                Transform::from_translation(PARCEL_GRIP),
                Visibility::default(),
                ChildOf(courier),
            ))
            .id();
        models.parcel(&mut commands, box_root);
        let dog_position = ground(base + forward * 0.96 + right * 1.09)
            + Vec3::Y * crate::roads::surface_rise(&elevation.heights);
        let dog_pose =
            Transform::from_translation(dog_position).looking_to(forward - right * 0.35, Vec3::Y);
        let dog = commands
            .spawn((dog_pose, Visibility::default(), DespawnOnExit(Mode::Ride)))
            .id();
        materials
            .dogs
            .model(&mut commands, dog, paket_session::DogBreed::JackRussell);
        commands.spawn((
            PointLight {
                intensity: 180000.,
                range: 15.,
                color: Color::srgb(1., 0.91, 0.78),
                ..default()
            },
            Transform::from_translation(base + forward * 3. + right * 2. + Vec3::Y * 4.),
        ));
        (base, rotation)
    });
    scene.ready = !ride.loaded.is_empty()
        && !roots.is_empty()
        && roots
            .iter()
            .all(|root| assets.is_loaded_with_dependencies(root.0.id()));
    let forward = rotation * Vec3::NEG_Z;
    let right = rotation * Vec3::X;
    if let Ok((mut camera, mut projection)) = cameras.single_mut() {
        let focus = base - forward * 1.1 + right * 0.69 + Vec3::Y * 0.80;
        *camera = Transform::from_translation(base + forward * 4.5 + right * 0.25 + Vec3::Y * 1.36)
            .looking_at(focus, Vec3::Y);
        if let Projection::Perspective(lens) = &mut *projection {
            lens.fov = 39_f32.to_radians();
        }
    }
}

fn dress_chamissoplatz(
    commands: &mut Commands,
    models: &Models,
    traffic: &TrafficWorld,
    ground: impl Fn(Vec3) -> Vec3,
) {
    for (variant, (position, facing)) in [
        (Vec3::new(-3.3, 0., 154.4), Vec3::Z),
        (Vec3::new(-4.8, 0., 159.5), Vec3::NEG_Z),
    ]
    .into_iter()
    .enumerate()
    {
        let pedestrian = commands
            .spawn((
                Transform::from_translation(ground(position)).looking_to(facing, Vec3::Y),
                Visibility::default(),
                DespawnOnExit(Mode::Ride),
            ))
            .id();
        models.citizens.spawn(commands, pedestrian, variant);
    }
    let car = commands
        .spawn((
            Transform::from_translation(ground(Vec3::new(-7.1, 0., 151.2)) + Vec3::Y * 0.8)
                .looking_to(Vec3::new(-0.17, 0., 1.), Vec3::Y),
            Visibility::default(),
            DespawnOnExit(Mode::Ride),
        ))
        .id();
    traffic.car(commands, car, 3);
    let van = commands
        .spawn((
            Transform::from_translation(ground(Vec3::new(-6.6, 0., 160.7)))
                .looking_to(Vec3::new(-0.58, 0., 0.81), Vec3::Y),
            Visibility::default(),
            DespawnOnExit(Mode::Ride),
        ))
        .id();
    models.van(commands, van);
}
