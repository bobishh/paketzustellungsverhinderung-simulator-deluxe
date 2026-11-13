//! Reproducible in-engine press still, using the same meshes and materials as play.
use crate::{
    Mode, StreetMaterials,
    actors::{HoldingParcel, Models, ParkedVan, Player},
};
use bevy::prelude::*;
#[derive(Resource, Default)]
struct Scene {
    base: Option<(Vec3, Quat)>,
}
pub(crate) fn install(app: &mut App) {
    app.init_resource::<Scene>().add_systems(
        Update,
        compose
            .after(crate::actors::camera_follow)
            .after(crate::jobs::service_ui)
            .after(crate::action_prompt)
            .run_if(in_state(Mode::Ride)),
    );
}
fn compose(
    mut commands: Commands,
    mut scene: ResMut<Scene>,
    models: Res<Models>,
    materials: Res<StreetMaterials>,
    player: Query<(&Player, &Transform), Without<Camera3d>>,
    mut cameras: Query<(&mut Transform, &mut Projection), (With<Camera3d>, Without<Player>)>,
    mut visible: Query<&mut Visibility>,
    mut ui: Query<&mut Node>,
    vans: Query<Entity, With<ParkedVan>>,
) {
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
    let (base, rotation) = *scene.base.get_or_insert_with(|| {
        let forward = actor.forward().as_vec3();
        let right = actor.right().as_vec3();
        let base = (actor.translation + forward * 3.0).with_y(0.04);
        let rotation = actor.rotation;
        let courier = commands
            .spawn((
                Transform::from_translation(base)
                    .with_rotation(rotation * Quat::from_rotation_z(-0.07)),
                Visibility::default(),
                HoldingParcel,
                DespawnOnExit(Mode::Ride),
            ))
            .id();
        models.person(&mut commands, courier, true);
        models.grimace(&mut commands, courier);
        let box_root = commands
            .spawn((
                Transform::from_xyz(0., 1.04, -0.40),
                Visibility::default(),
                ChildOf(courier),
            ))
            .id();
        models.parcel(&mut commands, box_root);
        let dog_position = base + right * 0.48 + forward * 0.08 + Vec3::Y * 0.25;
        let dog = commands
            .spawn((
                Transform::from_translation(dog_position).looking_to(-right, Vec3::Y),
                Visibility::default(),
                DespawnOnExit(Mode::Ride),
            ))
            .id();
        materials.dogs.model(&mut commands, dog);
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
    let forward = rotation * Vec3::NEG_Z;
    let right = rotation * Vec3::X;
    if let Ok((mut camera, mut projection)) = cameras.single_mut() {
        let focus = base + right * 0.95 + Vec3::Y * 0.95;
        *camera = Transform::from_translation(focus + forward * 4.0 - right * 0.4 + Vec3::Y * 0.38)
            .looking_at(focus, Vec3::Y);
        if let Projection::Perspective(lens) = &mut *projection {
            lens.fov = 48_f32.to_radians();
        }
    }
}
