use std::{
    fs,
    path::Path,
    sync::{Mutex, mpsc},
    thread,
};

use bevy::{
    asset::RenderAssetUsages,
    image::{CompressedImageFormats, ImageSampler, ImageType},
    light::NotShadowCaster,
    math::DVec2,
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
};
use paket_geo::{
    EastNorth, LatLon,
    landscape::{self, Area, Cover},
};

use crate::{Mode, Ride, to_world};

#[derive(Resource, Default)]
pub(crate) struct Landscape {
    pending: Option<Mutex<mpsc::Receiver<Result<Vec<Area>, String>>>>,
    water: Vec<Area>,
    root: Option<Entity>,
}

impl Landscape {
    #[cfg(test)]
    pub(crate) fn test_water(water: Vec<Area>) -> Self {
        Self { water, ..default() }
    }
    pub fn water_at(&self, point: EastNorth) -> bool {
        self.water.iter().any(|area| area.contains(point))
    }
}

pub(crate) fn start(mut commands: Commands, ride: Res<Ride>) {
    let (tx, rx) = mpsc::channel();
    let (cache, origin, online) = (ride.cache.clone(), ride.origin, ride.allow_network);
    #[cfg(not(target_arch = "wasm32"))]
    thread::spawn(move || {
        match landscape::load(&cache, origin, false) {
            Ok(exact) if !exact.is_empty() => {
                let _ = tx.send(Ok(exact));
                return;
            }
            Err(error) => eprintln!("OSM landscape cache: {error}"),
            _ => {}
        }
        let cached = from_map_tiles(&cache.with_file_name("map"), origin);
        if !cached.is_empty() {
            let _ = tx.send(Ok(cached));
        }
        if online {
            let _ = tx.send(landscape::load(&cache, origin, true));
        }
    });
    #[cfg(target_arch = "wasm32")]
    wasm_bindgen_futures::spawn_local(async move {
        let result = crate::platform::fetch_bytes(&format!(
            "/api/osm?lat={:.5}&lon={:.5}&landscape=1",
            origin.lat, origin.lon
        ))
        .await
        .and_then(|bytes| landscape::parse(&bytes, origin));
        let _ = tx.send(result);
    });
    commands.insert_resource(Landscape {
        pending: Some(Mutex::new(rx)),
        ..default()
    });
}

pub(crate) fn finish(
    mut commands: Commands,
    mut landscape: ResMut<Landscape>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    let Some(pending) = landscape.pending.as_mut() else {
        return;
    };
    let result = match pending.get_mut().unwrap().try_recv() {
        Ok(result) => result,
        Err(mpsc::TryRecvError::Empty) => return,
        Err(mpsc::TryRecvError::Disconnected) => {
            landscape.pending = None;
            return;
        }
    };
    let areas = match result {
        Ok(areas) => areas,
        Err(error) => {
            eprintln!("OSM landscape: {error}");
            return;
        }
    };
    if areas.is_empty() {
        return;
    }
    if let Some(previous) = landscape.root.take() {
        commands.entity(previous).despawn();
    }
    let root = commands
        .spawn((
            Transform::IDENTITY,
            Visibility::default(),
            DespawnOnExit(Mode::Ride),
        ))
        .id();
    landscape.root = Some(root);
    let water = materials.add(StandardMaterial {
        perceptual_roughness: 0.22,
        reflectance: 0.65,
        ..crate::surfaces::material(
            &mut images,
            crate::surfaces::Surface::Water,
            Color::srgb(0.15, 0.32, 0.35),
        )
    });
    let forest_floor = materials.add(crate::surfaces::material(
        &mut images,
        crate::surfaces::Surface::Grass,
        Color::srgb(0.70, 0.78, 0.62),
    ));
    for (cover, y, material) in [
        (Cover::Water, -0.016, water),
        (Cover::Woodland, -0.029, forest_floor),
    ] {
        let mut vertices = Vec::new();
        let mut triangles = Vec::new();
        for area in areas.iter().filter(|area| area.cover == cover) {
            let Some((points, indices)) = area.mesh() else {
                continue;
            };
            let start = vertices.len() as u32;
            let positions: Vec<_> = points.iter().map(|p| to_world(*p, y)).collect();
            for triangle in indices.chunks_exact(3) {
                let [a, b, c] = [
                    positions[triangle[0]],
                    positions[triangle[1]],
                    positions[triangle[2]],
                ];
                let [i, j, k] = [
                    start + triangle[0] as u32,
                    start + triangle[1] as u32,
                    start + triangle[2] as u32,
                ];
                triangles.extend(if (b - a).cross(c - a).y < 0.0 {
                    [i, k, j]
                } else {
                    [i, j, k]
                });
            }
            vertices.extend(positions);
        }
        if triangles.is_empty() {
            continue;
        }
        let mesh = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::default(),
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![Vec3::Y; vertices.len()])
        .with_inserted_attribute(
            Mesh::ATTRIBUTE_UV_0,
            vertices
                .iter()
                .map(|p| Vec2::new(p.x, p.z) / 3.0)
                .collect::<Vec<_>>(),
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, vertices)
        .with_inserted_indices(Indices::U32(triangles));
        commands.spawn((
            Mesh3d(meshes.add(crate::shapes::lit(mesh))),
            MeshMaterial3d(material),
            NotShadowCaster,
            ChildOf(root),
        ));
    }
    trees(&mut commands, &mut meshes, &mut materials, &areas, root);
    landscape.water = areas
        .into_iter()
        .filter(|area| area.cover == Cover::Water)
        .collect();
    println!(
        "OSM landscape ready: {} water surfaces",
        landscape.water.len()
    );
}

// Zoom-16 tiles already on disk provide a coarse preview while vector land cover loads.
fn from_map_tiles(cache: &Path, origin: LatLon) -> Vec<Area> {
    const ZOOM: u8 = 16;
    const SIDE: i32 = 256;
    const BLOCK: i32 = 8;
    let center = crate::map::project(origin, ZOOM).as_ivec2() / SIDE;
    let mut areas = Vec::new();
    for y in (center.y - 3)..=(center.y + 3) {
        for x in (center.x - 3)..=(center.x + 3) {
            let path = cache
                .join(ZOOM.to_string())
                .join(x.to_string())
                .join(format!("{y}.png"));
            let Ok(bytes) = fs::read(path) else {
                continue;
            };
            let Ok(image) = Image::from_buffer(
                &bytes,
                ImageType::Extension("png"),
                CompressedImageFormats::NONE,
                true,
                ImageSampler::nearest(),
                RenderAssetUsages::default(),
            ) else {
                continue;
            };
            let Ok(image) = image.try_into_dynamic() else {
                continue;
            };
            let image = image.into_rgb8();
            if image.dimensions() != (SIDE as u32, SIDE as u32) {
                continue;
            }
            let classify = |bx: i32, by: i32| {
                let mut water = 0;
                let mut woods = 0;
                for dy in [1, 3, 5, 7] {
                    for dx in [1, 3, 5, 7] {
                        match image.get_pixel((bx + dx) as u32, (by + dy) as u32).0 {
                            [170, 211, 223] => water += 1,
                            [173, 209, 158] => woods += 1,
                            _ => {}
                        }
                    }
                }
                if water >= 10 {
                    Some(Cover::Water)
                } else if woods >= 10 {
                    Some(Cover::Woodland)
                } else {
                    None
                }
            };
            let point = |bx, by| {
                paket_geo::project(
                    origin,
                    crate::map::unproject(
                        DVec2::new(f64::from(x * SIDE + bx), f64::from(y * SIDE + by)),
                        ZOOM,
                    ),
                )
            };
            for by in (0..SIDE).step_by(BLOCK as usize) {
                let mut left = 0;
                while left < SIDE {
                    let Some(cover) = classify(left, by) else {
                        left += BLOCK;
                        continue;
                    };
                    let mut right = left + BLOCK;
                    while right < SIDE && classify(right, by) == Some(cover) {
                        right += BLOCK;
                    }
                    areas.push(Area {
                        cover,
                        rings: vec![vec![
                            point(left, by),
                            point(right, by),
                            point(right, by + BLOCK),
                            point(left, by + BLOCK),
                        ]],
                    });
                    left = right;
                }
            }
        }
    }
    areas
}

fn trees(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    areas: &[Area],
    root: Entity,
) {
    let trunk = Cylinder::new(0.18, 3.8).mesh().resolution(5).build();
    let pine = Cone::new(2.8, 7.0).mesh().resolution(7).build();
    let crown = Sphere::new(1.0).mesh().ico(2).unwrap();
    let mut groups = [None, None, None, None];
    let mut add = |group: usize, mesh: &Mesh, transform| {
        let mesh = mesh.clone().transformed_by(transform);
        if let Some(combined) = &mut groups[group] {
            let _: Result<(), _> = Mesh::merge(combined, &mesh);
        } else {
            groups[group] = Some(mesh);
        }
    };
    let cover_at = |p: EastNorth, cover| {
        areas
            .iter()
            .any(|area| area.cover == cover && area.contains(p))
    };
    let mut count = 0;
    // Make nearby woods dense enough to block the sightline between their buildings.
    let mut cells: Vec<_> = (-44i32..=44)
        .flat_map(|x| (-44i32..=44).map(move |z| (x, z)))
        .collect();
    cells.sort_unstable_by_key(|&(x, z)| (x * x + z * z, x, z));
    for (x, z) in cells {
        let hash = (x as u32).wrapping_mul(374761393) ^ (z as u32).wrapping_mul(668265263);
        let hash = (hash ^ (hash >> 13)).wrapping_mul(1274126177);
        let point = EastNorth {
            east: f64::from(x * 13) + f64::from(hash % 9) - 4.0,
            north: f64::from(z * 13) + f64::from((hash >> 8) % 9) - 4.0,
        };
        if !cover_at(point, Cover::Woodland) || cover_at(point, Cover::Water) {
            continue;
        }
        let base = to_world(point, 0.0);
        let scale = 0.73 + (hash % 42) as f32 * 0.016;
        add(
            0,
            &trunk,
            Transform::from_translation(base + Vec3::Y * 1.85 * scale)
                .with_scale(Vec3::splat(scale)),
        );
        if hash % 3 == 0 {
            add(
                1,
                &pine,
                Transform::from_translation(base + Vec3::Y * 5.5 * scale)
                    .with_scale(Vec3::splat(scale)),
            );
        } else {
            for lobe in 0..5 {
                let angle = lobe as f32 * 2.4 + (hash % 100) as f32;
                let offset = Vec3::new(
                    angle.cos() * 1.35,
                    4.5 + lobe as f32 * 0.43,
                    angle.sin() * 1.35,
                );
                add(
                    2 + ((hash + lobe) % 2) as usize,
                    &crown,
                    Transform::from_translation(base + offset * scale)
                        .with_rotation(Quat::from_rotation_y(angle))
                        .with_scale(Vec3::new(2.0, 2.2, 1.85) * scale),
                );
            }
        }
        count += 1;
        if count == 950 {
            break;
        }
    }
    println!("OSM landscape ready: {count} woodland trees");
    for (mesh, color) in groups.into_iter().zip([
        Color::srgb(0.29, 0.21, 0.14),
        Color::srgb(0.17, 0.32, 0.19),
        Color::srgb(0.28, 0.43, 0.19),
        Color::srgb(0.34, 0.46, 0.23),
    ]) {
        if let Some(mesh) = mesh {
            commands.spawn((
                Mesh3d(meshes.add(mesh)),
                MeshMaterial3d(materials.add(color)),
                ChildOf(root),
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "uses locally cached OpenStreetMap tiles"]
    fn mausohrweg_cache_contains_woods_and_water() {
        let origin = paket_geo::MAUSOHRWEG;
        let cache = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../cache/map");
        let areas = from_map_tiles(&cache, origin);
        for cover in [Cover::Water, Cover::Woodland] {
            let covered: Vec<_> = areas.iter().filter(|area| area.cover == cover).collect();
            assert!(!covered.is_empty(), "missing {cover:?}");
            assert!(covered.iter().all(|area| area.mesh().is_some()));
            println!("{cover:?}: {} projected strips", covered.len());
        }
    }
}
