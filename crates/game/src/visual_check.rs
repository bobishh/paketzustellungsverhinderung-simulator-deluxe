//! Opt-in native render smoke check: PAKET_VISUAL_CHECK=/absolute/output/directory.
//! Uses the actual street, materials and actors, then exits its own test instance.
use crate::{
    Mode, StreetMaterials,
    actors::{ParkedVan, Player},
};
use bevy::{
    prelude::*,
    render::view::screenshot::{Screenshot, ScreenshotCaptured},
};
use std::path::PathBuf;

#[derive(Resource)]
struct Capture {
    directory: PathBuf,
    elapsed: f32,
    shot: usize,
    pending: bool,
    dog: Option<Vec3>,
}

pub(crate) fn install(app: &mut App, directory: PathBuf) {
    app.insert_resource(Capture {
        directory,
        elapsed: 0.0,
        shot: 0,
        pending: false,
        dog: None,
    })
    .add_systems(
        Update,
        capture
            .after(crate::actors::camera_follow)
            .run_if(in_state(Mode::Ride)),
    );
}

fn capture(
    mut commands: Commands,
    time: Res<Time>,
    mut check: ResMut<Capture>,
    materials: Res<StreetMaterials>,
    player: Query<(&Player, &Transform), Without<Camera3d>>,
    vans: Query<&Transform, (With<ParkedVan>, Without<Camera3d>)>,
    mut camera: Query<&mut Transform, (With<Camera3d>, Without<Player>, Without<ParkedVan>)>,
) {
    let (Ok((_, actor)), Ok(mut camera)) = (player.single(), camera.single_mut()) else {
        return;
    };
    check.elapsed += time.delta_secs();
    let floor = actor.translation.with_y(-0.04);
    let right = actor.right().as_vec3();
    let forward = actor.forward().as_vec3();
    if check.dog.is_none() {
        let point = floor + right * 0.8 + forward * 0.9;
        let root = commands
            .spawn((
                Transform::from_translation(point).with_rotation(actor.rotation),
                Visibility::default(),
            ))
            .id();
        materials.dogs.model(&mut commands, root);
        check.dog = Some(point);
    }
    let (focus, offset) = match check.shot {
        0 => (
            floor + Vec3::Y * 0.9,
            forward * 3.0 + right * 2.3 + Vec3::Y * 1.1,
        ),
        1 => (
            vans.single().map_or(floor, |v| v.translation) + Vec3::Y * 0.9,
            forward * 5.0 + right * 4.8 + Vec3::Y * 2.0,
        ),
        2 => (
            check.dog.unwrap() + Vec3::Y * 0.31,
            forward * 1.25 + right * 1.05 + Vec3::Y * 0.43,
        ),
        _ => {
            commands.write_message(AppExit::Success);
            return;
        }
    };
    *camera = Transform::from_translation(focus + offset).looking_at(focus, Vec3::Y);
    // Leave time for map streaming, all pipeline compilation and the previous capture.
    if check.elapsed < 5.0 || check.pending {
        return;
    }
    if let Err(error) = std::fs::create_dir_all(&check.directory) {
        error!("visual check: {error}");
        commands.write_message(AppExit::Success);
        return;
    }
    let path = check.directory.join(format!("{:02}.png", check.shot));
    check.pending = true;
    commands.spawn(Screenshot::primary_window()).observe(
        move |event: On<ScreenshotCaptured>, mut check: ResMut<Capture>| {
            match event
                .image
                .clone()
                .try_into_dynamic()
                .map_err(|e| e.to_string())
                .and_then(|image| image.to_rgb8().save(&path).map_err(|e| e.to_string()))
            {
                Ok(()) => println!("Visual check saved: {}", path.display()),
                Err(error) => error!("visual check: {error}"),
            }
            check.pending = false;
            check.shot += 1;
            check.elapsed = 0.0;
        },
    );
}
