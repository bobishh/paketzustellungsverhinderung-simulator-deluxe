//! Place the objective arrow on the ground or above the roof while driving.
use crate::{
    Mode,
    actors::{FollowCamera, ParkedVan, Player},
    elevation::Elevation,
    jobs::Jobs,
    shift::Shift,
};
use avian3d::prelude::{SpatialQuery, SpatialQueryFilter};
use bevy::{
    asset::RenderAssetUsages,
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
};
use paket_session::Avatar;

#[derive(Component)]
pub(crate) struct NavigationArrow;

pub(crate) fn update(
    mut commands: Commands,
    jobs: Res<Jobs>,
    shift: Res<Shift>,
    player: Query<(Entity, &Player, &Transform), Without<NavigationArrow>>,
    vans: Query<&Transform, (With<ParkedVan>, Without<NavigationArrow>)>,
    camera: Query<&Transform, (With<FollowCamera>, Without<NavigationArrow>)>,
    elevation: Res<Elevation>,
    landscape: Option<Res<crate::terrain::Landscape>>,
    bridges: Option<Res<crate::bridges::Bridges>>,
    spatial: SpatialQuery,
    mut arrows: Query<(&mut Transform, &mut Visibility), With<NavigationArrow>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    time: Res<Time>,
    mut last_emit: Local<f32>,
) {
    let Ok((player_id, player, actor)) = player.single() else {
        return;
    };
    if arrows.is_empty() {
        let material = materials.add(StandardMaterial {
            base_color: Color::srgb(1., 0.78, 0.12),
            perceptual_roughness: 0.3,
            ..default()
        });
        commands.spawn((
            NavigationArrow,
            Name::new("Objective arrow"),
            Mesh3d(meshes.add(arrow_mesh())),
            MeshMaterial3d(material),
            Transform::default(),
            Visibility::Hidden,
            DespawnOnExit(Mode::Ride),
        ));
        return;
    }
    let target = if shift.ready && !shift.ended() && !jobs.interaction_active() && !player.photo {
        jobs.navigation_target(player, player.parked.and_then(|id| vans.get(id).ok()))
    } else {
        None
    };
    let mut projection = serde_json::json!({"visible":false,"reason":"nearby-or-inactive"});
    for (mut transform, mut visible) in &mut arrows {
        *visible = Visibility::Hidden;
        let Some(target) =
            target.filter(|point| (*point - actor.translation).with_y(0.).length() > 4.5)
        else {
            continue;
        };
        let point = if player.form == Avatar::Van {
            // The van collider reaches the roof. Leave clearance for the mesh on slopes.
            Some(
                actor.translation
                    + Vec3::Y
                        * (crate::actors::half_height(Avatar::Van, Default::default()) + 0.55),
            )
        } else {
            let forward = actor.forward().as_vec3().with_y(0.).normalize_or_zero();
            let right = actor.right().as_vec3().with_y(0.).normalize_or_zero();
            let filter = SpatialQueryFilter::from_excluded_entities([player_id]);
            // Keep the marker off the silhouette and use the open side of narrow streets.
            [2., -2., 1.3, -1.3, 0.].into_iter().find_map(|side| {
                let point = actor.translation + forward * 3. + right * side;
                let deck = bridges
                    .as_ref()
                    .and_then(|bridges| bridges.height_at(point));
                if deck.is_none()
                    && landscape
                        .as_ref()
                        .is_some_and(|landscape| landscape.water_at(crate::east_north(point)))
                {
                    return None;
                }
                let ground = elevation.heights.at(point);
                let point = point.with_y(deck.unwrap_or(ground).max(ground) + 0.35);
                let from = actor.translation + Vec3::Y * 0.35;
                let offset = point + Vec3::Y * 0.35 - from;
                let clear = Dir3::new(offset).is_ok_and(|direction| {
                    spatial
                        .cast_ray(from, direction, offset.length(), true, &filter)
                        .is_none()
                });
                clear.then_some(point)
            })
        };
        let Some(point) = point else {
            projection = serde_json::json!({"visible":false,"reason":"obstructed","target":target.to_array()});
            continue;
        };
        let direction = (target - point).with_y(0.).normalize_or_zero();
        if direction.length_squared() < 0.5 {
            continue;
        }
        transform.translation = point;
        transform.scale = Vec3::splat(1.3);
        transform.rotation = if player.form == Avatar::Van {
            camera.single().map_or_else(
                |_| arrow_rotation(direction),
                |camera| roof_arrow_rotation(direction, camera.translation - point),
            )
        } else {
            arrow_rotation(direction)
        };
        *visible = Visibility::Visible;
        projection = serde_json::json!({"visible":true,"position":point.to_array(),"target":target.to_array(),"direction":(transform.rotation*Vec3::Y).to_array()});
    }
    if time.elapsed_secs() - *last_emit >= 0.1 {
        *last_emit = time.elapsed_secs();
        crate::platform::ui_event("paket-navigation", &projection.to_string());
    }
}

fn arrow_rotation(direction: Vec3) -> Quat {
    Quat::from_rotation_y((-direction.x).atan2(-direction.z))
        * Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)
}

fn roof_arrow_rotation(direction: Vec3, view: Vec3) -> Quat {
    // Lift the tip when it points away from the camera so forward and back read clearly.
    let pitch = -0.5 * direction.dot(view.with_y(0.).normalize_or_zero());
    let head = (direction + Vec3::Y * pitch).normalize();
    let Some(side) = head.cross(view).try_normalize() else {
        return arrow_rotation(direction);
    };
    Quat::from_mat3(&Mat3::from_cols(side, head, side.cross(head)))
}

fn arrow_mesh() -> Mesh {
    let outline = [
        Vec2::new(-0.17, -0.55),
        Vec2::new(0.17, -0.55),
        Vec2::new(0.17, 0.10),
        Vec2::new(0.50, 0.10),
        Vec2::new(0.0, 0.72),
        Vec2::new(-0.50, 0.10),
        Vec2::new(-0.17, 0.10),
    ];
    let face = [0, 1, 2, 0, 2, 6, 6, 2, 4, 2, 3, 4, 5, 6, 4];
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut uvs = Vec::new();
    let mut indices = Vec::new();
    for (z, normal) in [(0.09, Vec3::Z), (-0.09, Vec3::NEG_Z)] {
        for point in outline {
            positions.push(point.extend(z));
            normals.push(normal);
            uvs.push(Vec2::new(point.x + 0.5, (point.y + 0.55) / 1.27));
        }
    }
    indices.extend(face);
    for triangle in face.chunks_exact(3) {
        indices.extend([triangle[2] + 7, triangle[1] + 7, triangle[0] + 7]);
    }
    for i in 0..outline.len() {
        let a = outline[i];
        let b = outline[(i + 1) % outline.len()];
        let edge = b - a;
        let normal = Vec3::new(edge.y, -edge.x, 0.0).normalize();
        let first = positions.len() as u32;
        positions.extend([
            a.extend(0.09),
            a.extend(-0.09),
            b.extend(-0.09),
            b.extend(0.09),
        ]);
        normals.extend([normal; 4]);
        uvs.extend([Vec2::ZERO, Vec2::X, Vec2::ONE, Vec2::Y]);
        indices.extend([first, first + 1, first + 2, first, first + 2, first + 3]);
    }
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
    .with_inserted_indices(Indices::U32(indices))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_ground_arrow_points_to_the_objective_in_every_world_direction() {
        for direction in [
            Vec3::NEG_Z,
            Vec3::X,
            Vec3::Z,
            Vec3::NEG_X,
            Vec3::new(0.6, 0.0, -0.8),
        ] {
            let rotation = arrow_rotation(direction);
            assert!(
                (rotation * Vec3::Y).dot(direction) > 0.999,
                "the head points toward the real target"
            );
            assert!(
                (rotation * Vec3::Z).dot(Vec3::Y) > 0.999,
                "the broad face stays parallel to the ground"
            );
        }
    }
}
