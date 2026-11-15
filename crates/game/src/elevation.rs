//! Shared terrain heights for physics and scenery. Zero is the ride origin's elevation.
use avian3d::prelude::*;
use bevy::{
    asset::RenderAssetUsages,
    math::DVec2,
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
};
use paket_geo::LatLon;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex, mpsc},
};

const ZOOM: u8 = 13;
const TILE: i32 = 256;
const RADIUS: f32 = 1600.;
pub(crate) const STEP: f32 = 20.;
const SIDE: usize = (RADIUS as usize * 2 / STEP as usize) + 1;
const FADE: f32 = 200.;
type Tile = (i32, i32);

#[derive(Resource, Default)]
pub(crate) struct Elevation {
    pub heights: Heights,
    pending: Option<(
        usize,
        Mutex<mpsc::Receiver<(Tile, Option<image::RgbImage>)>>,
    )>,
    tiles: HashMap<Tile, image::RgbImage>,
}

#[derive(Clone, Default)]
pub(crate) struct Heights(Option<Arc<[f32]>>);

impl Heights {
    pub fn available(&self) -> bool {
        self.0.is_some()
    }

    pub fn at(&self, point: Vec3) -> f32 {
        let Some(grid) = &self.0 else {
            return 0.;
        };
        let x = (point.x + RADIUS) / STEP;
        let z = (point.z + RADIUS) / STEP;
        if x < 0. || z < 0. || x >= (SIDE - 1) as f32 || z >= (SIDE - 1) as f32 {
            return 0.;
        }
        let i = z.floor() as usize * SIDE + x.floor() as usize;
        let (a, b, c, d) = (grid[i], grid[i + 1], grid[i + SIDE], grid[i + SIDE + 1]);
        let (u, v) = (x.fract(), z.fract());
        // Interpolate the same triangles used by the ground collider.
        if u + v <= 1. {
            a + (b - a) * u + (c - a) * v
        } else {
            d + (c - d) * (1. - u) + (b - d) * (1. - v)
        }
    }
}

impl Elevation {
    pub fn ready(&self) -> bool {
        self.pending.is_none()
    }
}

fn pixel(origin: LatLon, east: f64, north: f64) -> DVec2 {
    let ll = paket_geo::unproject(origin, paket_geo::EastNorth { east, north });
    crate::map::project(ll, ZOOM) - DVec2::splat(0.5)
}

fn tile(pixel: DVec2) -> Tile {
    let p = (pixel / f64::from(TILE)).floor().as_ivec2();
    (p.x, p.y)
}

fn wrapped(x: i32) -> i32 {
    x.rem_euclid(1 << ZOOM)
}

fn decode(bytes: &[u8]) -> Option<image::RgbImage> {
    use image::ImageDecoder;
    let decoder = image::codecs::png::PngDecoder::new(std::io::Cursor::new(bytes)).ok()?;
    if decoder.dimensions() != (TILE as u32, TILE as u32) {
        return None;
    }
    image::DynamicImage::from_decoder(decoder)
        .ok()
        .map(|img| img.into_rgb8())
}

pub(crate) fn start(mut commands: Commands, ride: Res<crate::Ride>) {
    let mut elevation = Elevation::default();
    if !ride.allow_network {
        commands.insert_resource(elevation);
        return;
    }
    let (radius, origin) = (f64::from(RADIUS), ride.origin);
    let nw = tile(pixel(origin, -radius, radius));
    let se = tile(pixel(origin, radius, -radius) + DVec2::ONE);
    if (se.0 - nw.0 + 1) * (se.1 - nw.1 + 1) > 16 {
        commands.insert_resource(elevation);
        return;
    }
    let (send, receive) = mpsc::channel();
    let mut count = 0;
    for y in nw.1..=se.1 {
        if !(0..1 << ZOOM).contains(&y) {
            continue;
        }
        for x in nw.0..=se.0 {
            count += 1;
            let send = send.clone();
            #[cfg(target_arch = "wasm32")]
            wasm_bindgen_futures::spawn_local(async move {
                let png =
                    crate::platform::fetch_bytes(&format!("/elevation/{}/{y}.png", wrapped(x)))
                        .await
                        .ok()
                        .and_then(|bytes| decode(&bytes));
                let _ = send.send(((x, y), png));
            });
            #[cfg(not(target_arch = "wasm32"))]
            {
                let cache = ride.cache.with_file_name("elevation");
                std::thread::spawn(move || {
                    use std::io::Read;
                    let file = cache.join(format!("{}-{y}.png", wrapped(x)));
                    let png = std::fs::read(&file).ok().and_then(|bytes| decode(&bytes)).or_else(|| {
                        let url = format!("https://s3.amazonaws.com/elevation-tiles-prod/terrarium/{ZOOM}/{}/{y}.png",wrapped(x));
                        let response = ureq::get(&url).timeout(std::time::Duration::from_secs(4)).call().ok()?;
                        let mut bytes = Vec::new();
                        response.into_reader().take(512_000).read_to_end(&mut bytes).ok()?;
                        let image = decode(&bytes)?;
                        if std::fs::create_dir_all(&cache).is_ok() { let _ = std::fs::write(file, bytes); }
                        Some(image)
                    });
                    let _ = send.send(((x, y), png));
                });
            }
        }
    }
    elevation.pending = Some((count, Mutex::new(receive)));
    commands.insert_resource(elevation);
}

fn sample(tiles: &HashMap<Tile, image::RgbImage>, p: DVec2) -> Option<f32> {
    let i = p.floor().as_ivec2();
    let rgb = |x: i32, y: i32| {
        let image = tiles.get(&(x.div_euclid(TILE), y.div_euclid(TILE)))?;
        let [r, g, b] = image
            .get_pixel(x.rem_euclid(TILE) as u32, y.rem_euclid(TILE) as u32)
            .0;
        Some(f32::from(r) * 256. + f32::from(g) + f32::from(b) / 256. - 32768.)
    };
    let f = p.fract().as_vec2();
    let a = rgb(i.x, i.y)?.lerp(rgb(i.x + 1, i.y)?, f.x);
    let b = rgb(i.x, i.y + 1)?.lerp(rgb(i.x + 1, i.y + 1)?, f.x);
    Some(a.lerp(b, f.y))
}

pub(crate) fn finish(ride: Res<crate::Ride>, mut elevation: ResMut<Elevation>) {
    let Elevation {
        pending,
        tiles,
        heights,
    } = &mut *elevation;
    let Some((count, receive)) = pending else {
        return;
    };
    loop {
        if *count == 0 {
            break;
        }
        match receive.get_mut().unwrap().try_recv() {
            Ok((id, png)) => {
                *count -= 1;
                if let Some(png) = png {
                    tiles.insert(id, png);
                }
            }
            Err(mpsc::TryRecvError::Empty) => return,
            Err(mpsc::TryRecvError::Disconnected) => break,
        }
    }
    if let Some(base) = sample(tiles, pixel(ride.origin, 0., 0.)) {
        let grid = (0..SIDE)
            .flat_map(|z| (0..SIDE).map(move |x| (x, z)))
            .map(|(x, z)| {
                let x = x as f32 * STEP - RADIUS;
                let z = z as f32 * STEP - RADIUS;
                let fade = ((RADIUS - x.abs().max(z.abs())) / FADE).clamp(0., 1.);
                sample(tiles, pixel(ride.origin, x.into(), (-z).into())).map(|e| (e - base) * fade)
            })
            .collect::<Option<Vec<_>>>();
        // A single incomplete patch falls back before any geometry is spawned,
        // so buildings and streets cannot use incompatible heights.
        heights.0 = grid.map(Arc::from);
    }
    tiles.clear();
    *pending = None;
}

pub(crate) fn floor(heights: &Heights, center: Vec3, half: f32) -> (Mesh, Collider) {
    let side = (half * 2. / STEP) as usize + 1;
    let mut position = Vec::with_capacity(side * side);
    let mut uv = Vec::with_capacity(side * side);
    let mut indices = Vec::with_capacity((side - 1) * (side - 1) * 6);
    for z in 0..side {
        for x in 0..side {
            let p = Vec3::new(x as f32 * STEP - half, 0., z as f32 * STEP - half);
            position.push(p.with_y(heights.at(p + center) - 0.05));
            uv.push((p + center).xz() / 3.);
            if z > 0 && x > 0 {
                let d = (z * side + x) as u32;
                let (a, b, c) = (d - side as u32 - 1, d - side as u32, d - 1);
                indices.extend([a, c, b, b, c, d]);
            }
        }
    }
    // Parry stores the grid by column (east) then row (south).
    let columns = (0..side)
        .map(|x| (0..side).map(|z| position[z * side + x].y).collect())
        .collect();
    let collider = Collider::heightfield(columns, Vec3::new(half * 2., 1., half * 2.));
    let mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, position)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uv)
    .with_inserted_indices(Indices::U32(indices))
    .with_computed_smooth_normals();
    (mesh, collider)
}

// Overlay triangles are split before sampling so long roads follow hills between OSM nodes.
pub(crate) fn drape(mesh: Mesh, heights: &Heights, origin: Vec3) -> Mesh {
    use bevy::mesh::VertexAttributeValues;
    if !heights.available() {
        return mesh;
    }
    let Some(VertexAttributeValues::Float32x3(vertices)) = mesh.attribute(Mesh::ATTRIBUTE_POSITION)
    else {
        return mesh;
    };
    let Some(indices) = mesh.indices() else {
        return mesh;
    };
    let indices = indices.iter().collect::<Vec<_>>();
    let mut points = Vec::new();
    let mut pending = Vec::new();
    for t in indices.chunks_exact(3) {
        pending.push((
            [
                Vec3::from(vertices[t[0]]),
                Vec3::from(vertices[t[1]]),
                Vec3::from(vertices[t[2]]),
            ],
            0,
        ));
        while let Some((triangle, depth)) = pending.pop() {
            let [a, b, c] = triangle;
            let edges = [
                a.distance_squared(b),
                b.distance_squared(c),
                c.distance_squared(a),
            ];
            let (longest, length) = edges
                .into_iter()
                .enumerate()
                .max_by(|(_, a), (_, b)| a.total_cmp(b))
                .unwrap();
            if depth < 10 && length > STEP * STEP {
                let a = triangle[longest];
                let b = triangle[(longest + 1) % 3];
                let c = triangle[(longest + 2) % 3];
                let mid = a.midpoint(b);
                pending.push(([a, mid, c], depth + 1));
                pending.push(([mid, b, c], depth + 1));
            } else {
                points.extend(
                    triangle.map(|p| p + Vec3::Y * (heights.at(p + origin) - origin.y + 0.025)),
                );
            }
        }
    }
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(
        Mesh::ATTRIBUTE_UV_0,
        points
            .iter()
            .map(|p| (p + origin).xz() / 3.)
            .collect::<Vec<_>>(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, points)
    .with_computed_flat_normals()
}
