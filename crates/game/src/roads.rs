//! Union road ribbons, then cut lower-priority surfaces out of them before triangulating.
//! Adjacent OSM ways and overlapping tiles must never produce coplanar drawable faces.
use crate::{Mode, StreetMaterials};
use bevy::{
    asset::RenderAssetUsages,
    light::NotShadowCaster,
    mesh::{Indices, PrimitiveTopology, VertexAttributeValues},
    prelude::*,
};
use geo::{BooleanOps, Coord, LineString, MultiPolygon, Polygon};
use paket_geo::{Street, Surface};
use std::{
    collections::BTreeMap,
    sync::{Mutex, mpsc},
    thread,
};

#[derive(Resource, Default)]
pub(crate) struct Roads {
    streets: BTreeMap<String, Street>,
    dirty: bool,
    pending: Option<Mutex<mpsc::Receiver<Vec<(usize, Mesh)>>>>,
    roots: Vec<Entity>,
    pub generation: u64,
}

impl Roads {
    pub fn streets(&self) -> impl Iterator<Item = &Street> {
        self.streets.values()
    }
    pub fn insert(&mut self, street: &Street) {
        if !self.streets.contains_key(&street.id) {
            self.streets.insert(street.id.clone(), street.clone());
            self.dirty = true;
            self.generation += 1;
        }
    }
    pub fn remove(&mut self, id: &str) {
        self.dirty |= self.streets.remove(id).is_some();
        self.generation += 1;
    }
    pub fn radar(&self) -> String {
        serde_json::to_string(&self.streets.values().map(|s|serde_json::json!({"points":s.centerline.iter().map(|p|[p.east,-p.north]).collect::<Vec<_>>(),"width":s.width_m})).collect::<Vec<_>>()).unwrap()
    }
}

pub(crate) fn rebuild(
    mut commands: Commands,
    mut roads: ResMut<Roads>,
    mut meshes: ResMut<Assets<Mesh>>,
    materials: Res<StreetMaterials>,
) {
    if let Some(pending) = &mut roads.pending {
        match pending.get_mut().unwrap().try_recv() {
            Ok(result) => {
                roads.pending = None;
                // Publish one consistent batch. Never leave new strips on top of old ones.
                for root in roads.roots.drain(..) {
                    commands.entity(root).despawn();
                }
                for (kind, mesh) in result {
                    let material =
                        [&materials.asphalt, &materials.paving, &materials.gravel][kind].clone();
                    let root = commands
                        .spawn((
                            Mesh3d(meshes.add(crate::shapes::lit(mesh))),
                            MeshMaterial3d(material),
                            NotShadowCaster,
                            DespawnOnExit(Mode::Ride),
                        ))
                        .id();
                    roads.roots.push(root);
                }
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                roads.pending = None;
            }
            Err(mpsc::TryRecvError::Empty) => {}
        }
    }
    if roads.dirty && roads.pending.is_none() {
        let streets = roads.streets.values().cloned().collect::<Vec<_>>();
        let (tx, rx) = mpsc::channel();
        roads.dirty = false;
        roads.pending = Some(Mutex::new(rx));
        let work = move || {
            let _ = tx.send(build(&streets));
        };
        #[cfg(not(target_arch = "wasm32"))]
        thread::spawn(work);
        #[cfg(target_arch = "wasm32")]
        wasm_bindgen_futures::spawn_local(async move {
            work();
        });
    }
}

fn union(mut pieces: Vec<MultiPolygon<f64>>) -> MultiPolygon<f64> {
    if pieces.is_empty() {
        return MultiPolygon(vec![]);
    }
    // A balanced reduction is much cheaper than repeatedly unioning into one growing city.
    while pieces.len() > 1 {
        pieces = pieces
            .chunks(2)
            .map(|pair| {
                if pair.len() == 2 {
                    pair[0].union(&pair[1])
                } else {
                    pair[0].clone()
                }
            })
            .collect();
    }
    pieces.pop().unwrap()
}

fn layers(streets: &[Street]) -> Vec<(usize, MultiPolygon<f64>)> {
    // Road width decides which surface continues through a junction. A tiny
    // asphalt footpath must not cut a broad paved street into two pieces.
    let mut layers: [Vec<MultiPolygon<f64>>; 10] = std::array::from_fn(|_| Vec::new());
    for street in streets.iter().filter(|s| s.bridge.is_none()) {
        let kind = match street.surface {
            Surface::Asphalt => 0,
            Surface::Paving => 1,
            Surface::Gravel => 2,
        };
        for sidewalk in [false, true] {
            let layer = if sidewalk {
                9
            } else {
                kind + if street.width_m >= 5.0 {
                    0
                } else if street.width_m >= 3.0 {
                    3
                } else {
                    6
                }
            };
            let mesh = crate::street_mesh(std::slice::from_ref(street), sidewalk, 0.0, 1.0);
            let Some(VertexAttributeValues::Float32x3(points)) =
                mesh.attribute(Mesh::ATTRIBUTE_POSITION)
            else {
                continue;
            };
            let Some(indices) = mesh.indices() else {
                continue;
            };
            let indices: Vec<_> = indices.iter().collect();
            for triangle in indices.chunks_exact(3) {
                let ring: Vec<_> = triangle
                    .iter()
                    .map(|&i| Coord {
                        x: points[i][0] as f64,
                        y: points[i][2] as f64,
                    })
                    .collect();
                layers[layer].push(MultiPolygon(vec![Polygon::new(LineString(ring), vec![])]));
            }
        }
    }
    let mut occupied = MultiPolygon(vec![]);
    let mut result = Vec::new();
    for (index, pieces) in layers.into_iter().enumerate() {
        let surface = union(pieces);
        let cut = surface.difference(&occupied);
        occupied = occupied.union(&surface);
        result.push((if index == 9 { 1 } else { index % 3 }, cut));
    }
    result
}

fn build(streets: &[Street]) -> Vec<(usize, Mesh)> {
    let mut vertices: [Vec<Vec3>; 3] = std::array::from_fn(|_| Vec::new());
    let mut indices: [Vec<u32>; 3] = std::array::from_fn(|_| Vec::new());
    for (kind, surface) in layers(streets) {
        for polygon in surface.0 {
            let mut coords = Vec::new();
            let mut holes = Vec::new();
            for (index, ring) in std::iter::once(polygon.exterior())
                .chain(polygon.interiors())
                .enumerate()
            {
                if index > 0 {
                    holes.push(coords.len() / 2);
                }
                for point in ring.0.iter().take(ring.0.len().saturating_sub(1)) {
                    coords.extend([point.x, point.y]);
                }
            }
            let Ok(triangles) = earcutr::earcut(&coords, &holes, 2) else {
                continue;
            };
            let base = vertices[kind].len() as u32;
            let points: Vec<_> = coords
                .chunks_exact(2)
                .map(|p| Vec3::new(p[0] as f32, 0.026, p[1] as f32))
                .collect();
            for t in triangles.chunks_exact(3) {
                let [a, b, c] = [points[t[0]], points[t[1]], points[t[2]]];
                let [i, j, k] = [base + t[0] as u32, base + t[1] as u32, base + t[2] as u32];
                indices[kind].extend(if (b - a).cross(c - a).y < 0.0 {
                    [i, k, j]
                } else {
                    [i, j, k]
                });
            }
            vertices[kind].extend(points);
        }
    }
    vertices
        .into_iter()
        .zip(indices)
        .enumerate()
        .filter(|(_, (_, indices))| !indices.is_empty())
        .map(|(kind, (points, indices))| {
            let repeat = [3.0, 1.28, 2.0][kind];
            let mesh = Mesh::new(
                PrimitiveTopology::TriangleList,
                RenderAssetUsages::default(),
            )
            .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![Vec3::Y; points.len()])
            .with_inserted_attribute(
                Mesh::ATTRIBUTE_UV_0,
                points
                    .iter()
                    .map(|p| Vec2::new(p.x, p.z) / repeat)
                    .collect::<Vec<_>>(),
            )
            .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, points)
            .with_inserted_indices(Indices::U32(indices));
            (kind, mesh)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use geo::Area;
    #[test]
    fn a_narrow_asphalt_path_does_not_cut_a_wide_paved_road() {
        use geo::{Contains, Point};
        use paket_geo::EastNorth;
        let road = Street {
            id: "road".into(),
            centerline: vec![
                EastNorth {
                    east: -10.,
                    north: 0.,
                },
                EastNorth {
                    east: 10.,
                    north: 0.,
                },
            ],
            width_m: 8.,
            sidewalk_m: [1., 1.],
            name: None,
            surface: Surface::Paving,
            spawnable: true,
            bridge: None,
        };
        let path = Street {
            id: "path".into(),
            centerline: vec![
                EastNorth {
                    east: 0.,
                    north: -10.,
                },
                EastNorth {
                    east: 0.,
                    north: 10.,
                },
            ],
            width_m: 1.8,
            sidewalk_m: [0., 0.],
            name: None,
            surface: Surface::Asphalt,
            spawnable: false,
            bridge: None,
        };
        let layers = layers(&[road, path]);
        let center = Point::new(0., 0.);
        assert!(
            layers
                .iter()
                .any(|(kind, p)| *kind == 1 && p.contains(&center))
        );
        assert!(
            !layers
                .iter()
                .any(|(kind, p)| *kind == 0 && p.contains(&center))
        );
    }
    #[test]
    fn intersecting_surfaces_and_sidewalks_never_share_area() {
        let scene = paket_geo::parse_overpass(
            include_bytes!("../../../fixtures/kreuzberg.json"),
            paket_geo::LatLon {
                lat: paket_geo::FIXTURE_LAT,
                lon: paket_geo::FIXTURE_LON,
            },
        )
        .unwrap();
        let surfaces = layers(&scene.streets);
        assert!(surfaces.iter().any(|(_, p)| p.unsigned_area() > 1.0));
        for (i, (_, a)) in surfaces.iter().enumerate() {
            for (_, b) in surfaces.iter().skip(i + 1) {
                assert!(a.intersection(b).unsigned_area() < 0.001);
            }
        }
    }
}
