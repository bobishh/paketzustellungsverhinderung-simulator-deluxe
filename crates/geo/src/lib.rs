//! Map geometry for a ride that can start anywhere.
//!
//! Tiles are only a fetch size. The session origin stays fixed while the
//! bicycle crosses them.

pub mod landscape;

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

pub const EARTH_RADIUS_M: f64 = 6_378_137.0;
/// How much map one OSM request covers. Not a limit on the ride.
pub const TILE_METERS: f64 = 180.0;
pub const DELIVERY_METERS: f64 = 3.0;
pub const FIXTURE_LAT: f64 = 52.4895;
pub const FIXTURE_LON: f64 = 13.3908;
pub const MAUSOHRWEG: LatLon = LatLon {
    lat: 52.4275793,
    lon: 13.741161,
};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LatLon {
    pub lat: f64,
    pub lon: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EastNorth {
    pub east: f64,
    pub north: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TileId {
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Building {
    pub id: String,
    pub address: Option<String>,
    pub footprint: Vec<EastNorth>,
    pub triangles: Vec<[u32; 3]>,
    pub height: f64,
    pub entrance: EastNorth,
    /// Ground-level openings where an OSM building passage crosses a facade.
    pub passages: Vec<Passage>,
    pub accessory: bool,
    pub open_sided: bool,
    pub roof: Roof,
    pub roof_height: Option<f32>,
    pub wall_color: Option<[u8; 3]>,
    pub roof_color: Option<[u8; 3]>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Passage {
    pub edge: usize,
    pub start: f64,
    pub end: f64,
    pub height: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Roof {
    Flat,
    Gabled,
    Hipped,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Surface {
    Asphalt,
    Paving,
    Gravel,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Street {
    pub id: String,
    pub centerline: Vec<EastNorth>,
    pub width_m: f32,
    pub sidewalk_m: [f32; 2],
    pub name: Option<String>,
    pub surface: Surface,
    pub spawnable: bool,
    /// OSM bridge layer; relative stacking order, not an absolute elevation.
    pub bridge: Option<i8>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Scene {
    pub buildings: Vec<Building>,
    pub streets: Vec<Street>,
    pub skipped: usize,
}

#[derive(Debug, Deserialize)]
struct Overpass {
    elements: Vec<Element>,
}

#[derive(Debug, Deserialize)]
struct Element {
    #[serde(rename = "type")]
    kind: String,
    id: u64,
    #[serde(default)]
    tags: HashMap<String, String>,
    geometry: Option<Vec<LatLonJson>>,
}

#[derive(Debug, Deserialize)]
struct LatLonJson {
    lat: f64,
    lon: f64,
}

pub fn project(origin: LatLon, point: LatLon) -> EastNorth {
    let lat0 = origin.lat.to_radians();
    EastNorth {
        east: (point.lon - origin.lon).to_radians() * EARTH_RADIUS_M * lat0.cos(),
        north: (point.lat - origin.lat).to_radians() * EARTH_RADIUS_M,
    }
}

pub fn unproject(origin: LatLon, point: EastNorth) -> LatLon {
    let lat0 = origin.lat.to_radians();
    LatLon {
        lat: origin.lat + (point.north / EARTH_RADIUS_M).to_degrees(),
        lon: origin.lon + (point.east / (EARTH_RADIUS_M * lat0.cos())).to_degrees(),
    }
}

pub fn tile_id(point: EastNorth) -> TileId {
    TileId {
        x: div_floor(point.east, TILE_METERS),
        y: div_floor(point.north, TILE_METERS),
    }
}

pub fn tile_center(id: TileId) -> EastNorth {
    EastNorth {
        east: (f64::from(id.x) + 0.5) * TILE_METERS,
        north: (f64::from(id.y) + 0.5) * TILE_METERS,
    }
}

pub fn neighboring_tiles(point: EastNorth, margin: f64) -> Vec<TileId> {
    let here = tile_id(point);
    let mut tiles = vec![here];
    let local_x = point.east - f64::from(here.x) * TILE_METERS;
    let local_y = point.north - f64::from(here.y) * TILE_METERS;
    let mut push = |x, y| tiles.push(TileId { x, y });
    if local_x < margin {
        push(here.x - 1, here.y);
    }
    if local_x > TILE_METERS - margin {
        push(here.x + 1, here.y);
    }
    if local_y < margin {
        push(here.x, here.y - 1);
    }
    if local_y > TILE_METERS - margin {
        push(here.x, here.y + 1);
    }
    tiles
}

pub fn parse_overpass(bytes: &[u8], origin: LatLon) -> Result<Scene, String> {
    let doc: Overpass = serde_json::from_slice(bytes).map_err(|error| error.to_string())?;
    let mut buildings = Vec::new();
    let mut streets = Vec::new();
    let mut passage_ways = Vec::new();
    let mut skipped = 0;
    for element in doc.elements {
        if element.kind != "way" {
            continue;
        }
        let Some(geometry) = element.geometry else {
            continue;
        };
        let points: Vec<EastNorth> = geometry
            .into_iter()
            .map(|point| {
                project(
                    origin,
                    LatLon {
                        lat: point.lat,
                        lon: point.lon,
                    },
                )
            })
            .collect();
        let id = format!("way/{}", element.id);
        if element
            .tags
            .get("building")
            .is_some_and(|kind| kind != "no")
        {
            match building_from_ring(&id, &element.tags, points, origin) {
                Some(building) => buildings.push(building),
                None => skipped += 1,
            }
        } else if let Some(highway) = element.tags.get("highway")
            && !matches!(
                highway.as_str(),
                "platform" | "steps" | "construction" | "proposed" | "raceway"
            )
            && !element.tags.get("area").is_some_and(|area| area == "yes")
        {
            let line = dedup(points);
            if line.len() >= 2 {
                let (default_width, default_sidewalk) = match highway.as_str() {
                    "motorway" | "trunk" => (9.0, 0.0),
                    "primary" | "secondary" | "tertiary" => (8.0, 1.6),
                    "residential" | "unclassified" => (5.8, 1.3),
                    "living_street" | "service" | "track" => (3.6, 0.0),
                    "pedestrian" => (4.0, 0.0),
                    _ => (1.8, 0.0),
                };
                let width_m = element
                    .tags
                    .get("width")
                    .and_then(|width| parse_meters(width))
                    .filter(|width| (0.25..=100.0).contains(width))
                    .map_or(default_width, |width| width as f32);
                let sidewalk_m = match element.tags.get("sidewalk").map(String::as_str) {
                    Some("no" | "none" | "separate") => [0.0, 0.0],
                    Some("left") => [1.4, 0.0],
                    Some("right") => [0.0, 1.4],
                    Some("both" | "yes") => [1.4, 1.4],
                    _ => [default_sidewalk; 2],
                };
                let surface = match element.tags.get("surface").map(String::as_str) {
                    Some("asphalt" | "concrete" | "paved") => Surface::Asphalt,
                    Some("paving_stones" | "sett" | "cobblestone" | "concrete:plates") => {
                        Surface::Paving
                    }
                    Some(
                        "gravel" | "fine_gravel" | "compacted" | "dirt" | "ground" | "sand"
                        | "unpaved" | "grass",
                    ) => Surface::Gravel,
                    _ if matches!(highway.as_str(), "path" | "track") => Surface::Gravel,
                    _ if matches!(highway.as_str(), "footway" | "pedestrian") => Surface::Paving,
                    _ => Surface::Asphalt,
                };
                let spawnable = !matches!(
                    highway.as_str(),
                    "motorway" | "motorway_link" | "trunk" | "trunk_link" | "footway"
                ) && !matches!(
                    element.tags.get("bicycle").map(String::as_str),
                    Some("no" | "dismount")
                );
                if element
                    .tags
                    .get("tunnel")
                    .is_some_and(|v| v == "building_passage")
                {
                    let height = element
                        .tags
                        .get("maxheight")
                        .and_then(|v| parse_meters(v))
                        .unwrap_or(3.2)
                        .clamp(2.2, 6.0);
                    passage_ways.push((line.clone(), f64::from(width_m), height));
                }
                streets.push(Street {
                    id,
                    centerline: line,
                    width_m,
                    sidewalk_m,
                    name: element.tags.get("name").cloned(),
                    surface,
                    spawnable,
                    bridge: element
                        .tags
                        .get("bridge")
                        .filter(|v| !matches!(v.as_str(), "no" | "false" | "0"))
                        .map(|_| {
                            element
                                .tags
                                .get("layer")
                                .and_then(|v| v.parse::<i8>().ok())
                                .unwrap_or(1)
                        }),
                });
            }
        }
    }
    assign_passages(&mut buildings, &passage_ways);
    assign_entrances(&mut buildings, &streets);
    Ok(Scene {
        buildings,
        streets,
        skipped,
    })
}

pub fn building_height(tags: &HashMap<String, String>) -> f64 {
    if let Some(meters) = tags.get("height").and_then(|value| parse_meters(value)) {
        return meters;
    }
    if let Some(levels) = tags
        .get("building:levels")
        .and_then(|value| value.trim().parse::<f64>().ok())
        .filter(|levels| levels.is_finite() && *levels > 0.0)
    {
        return levels * 3.0;
    }
    match tags.get("building").map(String::as_str) {
        Some("carport" | "shed" | "garage" | "roof" | "hut" | "allotment_house") => 2.6,
        Some("garages" | "outbuilding" | "bungalow") => 3.2,
        Some("house" | "detached" | "semidetached_house" | "terrace") => 5.8,
        Some("retail" | "service" | "industrial" | "warehouse") => 5.5,
        _ => 9.0,
    }
}

pub fn parse_meters(value: &str) -> Option<f64> {
    let trimmed = value.trim().trim_end_matches(['m', 'M']).trim();
    let meters = trimmed.parse::<f64>().ok()?;
    meters
        .is_finite()
        .then_some(meters)
        .filter(|meters| *meters > 0.0)
}

pub fn within_delivery_range(distance_m: f64) -> bool {
    distance_m.is_finite() && distance_m <= DELIVERY_METERS
}

/// Records the first in-range confirmation. A repeat or a far confirmation leaves the record unchanged.
pub fn confirm_delivery(recorded: &mut Option<String>, building_id: &str, distance_m: f64) -> bool {
    if recorded.is_some() || !within_delivery_range(distance_m) {
        return false;
    }
    *recorded = Some(building_id.to_string());
    true
}

pub fn tile_is_playable(scene: &Scene) -> bool {
    !scene.buildings.is_empty() && !scene.streets.is_empty()
}

pub fn cache_file(cache_dir: &Path, origin: LatLon, tile: TileId) -> PathBuf {
    cache_dir.join(format!(
        "{:.5}_{:.5}_{}_{}.json",
        origin.lat, origin.lon, tile.x, tile.y
    ))
}

#[cfg(not(target_arch = "wasm32"))]
pub fn load_tile(
    cache_dir: &Path,
    origin: LatLon,
    tile: TileId,
    allow_network: bool,
) -> Result<Scene, String> {
    let path = cache_file(cache_dir, origin, tile);
    if path.exists() {
        let bytes = fs::read(&path).map_err(|error| error.to_string())?;
        return parse_overpass(&bytes, origin);
    }
    if !allow_network {
        return Err(format!("no cached tile {} {}", tile.x, tile.y));
    }
    let bytes = fetch_tile(origin, tile)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    fs::write(&path, &bytes).map_err(|error| error.to_string())?;
    parse_overpass(&bytes, origin)
}

#[cfg(not(target_arch = "wasm32"))]
pub fn fetch_tile(origin: LatLon, tile: TileId) -> Result<Vec<u8>, String> {
    let south_west = unproject(
        origin,
        EastNorth {
            east: f64::from(tile.x) * TILE_METERS,
            north: f64::from(tile.y) * TILE_METERS,
        },
    );
    let north_east = unproject(
        origin,
        EastNorth {
            east: f64::from(tile.x + 1) * TILE_METERS,
            north: f64::from(tile.y + 1) * TILE_METERS,
        },
    );
    let query = format!(
        "[out:json][timeout:25];(way[\"building\"]({south:.6},{west:.6},{north:.6},{east:.6});way[\"highway\"]({south:.6},{west:.6},{north:.6},{east:.6}););out geom;",
        south = south_west.lat,
        west = south_west.lon,
        north = north_east.lat,
        east = north_east.lon,
    );
    let mut last_error = String::from("no overpass endpoint");
    for endpoint in [
        "https://overpass.openstreetmap.fr/api/interpreter",
        "https://overpass.kumi.systems/api/interpreter",
        "https://overpass-api.de/api/interpreter",
    ] {
        match ureq::post(endpoint)
            .set(
                "User-Agent",
                "paket/0.1 (local game; OSM credit shown in the window)",
            )
            .timeout(std::time::Duration::from_secs(25))
            .send_form(&[("data", query.as_str())])
        {
            Ok(response) => {
                return response
                    .into_string()
                    .map(|body| body.into_bytes())
                    .map_err(|error| error.to_string());
            }
            Err(error) => last_error = format!("{endpoint}: {error}"),
        }
    }
    Err(last_error)
}

pub fn seed_cities() -> &'static [LatLon] {
    &[
        LatLon {
            lat: 52.5200,
            lon: 13.4050,
        },
        LatLon {
            lat: 25.0330,
            lon: 121.5654,
        },
        LatLon {
            lat: 48.8566,
            lon: 2.3522,
        },
        LatLon {
            lat: 40.7128,
            lon: -74.0060,
        },
        LatLon {
            lat: 35.6762,
            lon: 139.6503,
        },
    ]
}

fn building_from_ring(
    id: &str,
    tags: &HashMap<String, String>,
    mut points: Vec<EastNorth>,
    origin: LatLon,
) -> Option<Building> {
    points = dedup(points);
    if points.len() >= 2 {
        let first = points[0];
        let last = *points.last().unwrap();
        let gap = (first.east - last.east).hypot(first.north - last.north);
        if gap < 1e-3 {
            points.pop();
        } else if gap > 2.0 {
            return None;
        }
    }
    let triangles = triangulate(&points)?;
    let entrance = centroid(&points);
    let kind = tags.get("building").map(String::as_str).unwrap_or("yes");
    let area = polygon_area(&points).abs();
    let hessenwinkel = project(MAUSOHRWEG, unproject(origin, entrance));
    let small_home_area = if hessenwinkel.east.hypot(hessenwinkel.north) < 1300.0 {
        240.0
    } else {
        110.0
    };
    let accessory = matches!(
        kind,
        "roof"
            | "carport"
            | "shed"
            | "garage"
            | "garages"
            | "hut"
            | "outbuilding"
            | "allotment_house"
    ) || (kind == "yes" && area < 32.0);
    let roof = match tags.get("roof:shape").map(String::as_str) {
        Some("gabled") => Roof::Gabled,
        Some("hipped") => Roof::Hipped,
        _ => Roof::Flat,
    };
    let mut height = building_height(tags);
    if kind == "yes" && !tags.contains_key("height") && !tags.contains_key("building:levels") {
        height = if accessory {
            2.6
        } else if area < small_home_area {
            5.8
        } else {
            height
        };
    }
    let roof_height = tags
        .get("roof:height")
        .and_then(|height| parse_meters(height))
        .map(|height| height as f32);
    Some(Building {
        id: id.to_string(),
        address: tags
            .get("addr:housenumber")
            .map(|number| {
                tags.get("addr:street").map_or_else(
                    || format!("No. {number}"),
                    |street| format!("{street} {number}"),
                )
            })
            .or_else(|| tags.get("name").cloned()),
        footprint: points,
        triangles,
        height,
        entrance,
        passages: Vec::new(),
        accessory,
        open_sided: matches!(kind, "carport" | "roof"),
        roof,
        roof_height,
        wall_color: color_tag(tags, "building:colour"),
        roof_color: color_tag(tags, "roof:colour"),
    })
}

fn color_tag(tags: &HashMap<String, String>, key: &str) -> Option<[u8; 3]> {
    let value = tags.get(key)?.trim();
    let rgb = match value.to_ascii_lowercase().as_str() {
        "white" => 0xe6e2d7,
        "grey" | "gray" => 0x82827e,
        "red" => 0x9f5545,
        "brown" => 0x795b47,
        "black" => 0x3c3d3b,
        "beige" => 0xc8b894,
        "yellow" => 0xd5b778,
        _ => {
            let hex = value.strip_prefix('#')?;
            if hex.len() != 6 {
                return None;
            }
            u32::from_str_radix(hex, 16).ok()?
        }
    };
    Some([(rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8])
}

fn assign_passages(buildings: &mut [Building], ways: &[(Vec<EastNorth>, f64, f64)]) {
    let cross = |a: EastNorth, b: EastNorth| a.east * b.north - a.north * b.east;
    let sub = |a: EastNorth, b: EastNorth| EastNorth {
        east: a.east - b.east,
        north: a.north - b.north,
    };
    for building in buildings {
        for (edge, (a, b)) in building
            .footprint
            .iter()
            .zip(building.footprint.iter().cycle().skip(1))
            .enumerate()
        {
            let wall = sub(*b, *a);
            let length = wall.east.hypot(wall.north);
            if length < 0.01 {
                continue;
            }
            for (line, width, height) in ways {
                for segment in line.windows(2) {
                    let road = sub(segment[1], segment[0]);
                    let road_length = road.east.hypot(road.north);
                    let determinant = cross(wall, road);
                    if determinant.abs() < 1e-8 || road_length < 0.01 {
                        continue;
                    }
                    let delta = sub(segment[0], *a);
                    let t = cross(delta, road) / determinant;
                    let u = cross(delta, wall) / determinant;
                    // OSM passage endpoints can stop at the facade rather than cross it.
                    if !(0.0..=1.0).contains(&t)
                        || u < -0.4 / road_length
                        || u > 1.0 + 0.4 / road_length
                    {
                        continue;
                    }
                    let sin = (determinant / (length * road_length)).abs();
                    let half = (width.max(2.0) * 0.5 + 0.16) / sin.max(0.15);
                    building.passages.push(Passage {
                        edge,
                        start: (t * length - half).max(0.0),
                        end: (t * length + half).min(length),
                        height: height.min((building.height - 0.3).max(0.0)),
                    });
                }
            }
        }
    }
}

fn assign_entrances(buildings: &mut [Building], streets: &[Street]) {
    let segments: Vec<_> = streets
        .iter()
        .flat_map(|street| street.centerline.windows(2))
        .map(|pair| (pair[0], pair[1]))
        .collect();
    if segments.is_empty() {
        return;
    }
    let distance = |point: EastNorth| {
        segments
            .iter()
            .map(|(a, b)| {
                let dx = b.east - a.east;
                let dy = b.north - a.north;
                let length = dx * dx + dy * dy;
                let t = if length > 1e-10 {
                    (((point.east - a.east) * dx + (point.north - a.north) * dy) / length)
                        .clamp(0.0, 1.0)
                } else {
                    0.0
                };
                (point.east - a.east - dx * t).hypot(point.north - a.north - dy * t)
            })
            .fold(f64::INFINITY, f64::min)
    };
    for building in buildings {
        // A doorway belongs in a facade, not at the nearest footprint corner.
        building.entrance = building
            .footprint
            .iter()
            .zip(building.footprint.iter().cycle().skip(1))
            .enumerate()
            .filter(|(edge, (a, b))| {
                let half = (b.east - a.east).hypot(b.north - a.north) * 0.5;
                half >= 0.9
                    && !building
                        .passages
                        .iter()
                        .any(|p| p.edge == *edge && half + 0.85 > p.start && half - 0.85 < p.end)
            })
            .map(|(_, (a, b))| EastNorth {
                east: (a.east + b.east) * 0.5,
                north: (a.north + b.north) * 0.5,
            })
            .min_by(|a, b| distance(*a).total_cmp(&distance(*b)))
            .unwrap_or(building.entrance);
    }
}

fn centroid(points: &[EastNorth]) -> EastNorth {
    let count = points.len() as f64;
    EastNorth {
        east: points.iter().map(|point| point.east).sum::<f64>() / count,
        north: points.iter().map(|point| point.north).sum::<f64>() / count,
    }
}

fn dedup(points: Vec<EastNorth>) -> Vec<EastNorth> {
    let mut out = Vec::new();
    for point in points {
        if out.last().is_none_or(|previous: &EastNorth| {
            (previous.east - point.east).abs() > 1e-6 || (previous.north - point.north).abs() > 1e-6
        }) {
            out.push(point);
        }
    }
    out
}

pub fn triangulate(points: &[EastNorth]) -> Option<Vec<[u32; 3]>> {
    if points.len() < 3
        || !polygon_area(points).abs().is_finite()
        || polygon_area(points).abs() < 1e-4
    {
        return None;
    }
    let mut ring: Vec<usize> = (0..points.len()).collect();
    if polygon_area(points) < 0.0 {
        ring.reverse();
    }
    let mut triangles = Vec::new();
    let mut guard = 0;
    while ring.len() > 3 && guard < points.len() * points.len() {
        guard += 1;
        let mut clipped = false;
        for index in 0..ring.len() {
            let prev = ring[(index + ring.len() - 1) % ring.len()];
            let current = ring[index];
            let next = ring[(index + 1) % ring.len()];
            if !is_ear(points, &ring, prev, current, next) {
                continue;
            }
            triangles.push([prev as u32, current as u32, next as u32]);
            ring.remove(index);
            clipped = true;
            break;
        }
        if !clipped {
            return None;
        }
    }
    if ring.len() == 3 {
        triangles.push([ring[0] as u32, ring[1] as u32, ring[2] as u32]);
    }
    (!triangles.is_empty()).then_some(triangles)
}

fn polygon_area(points: &[EastNorth]) -> f64 {
    let mut area = 0.0;
    for index in 0..points.len() {
        let next = points[(index + 1) % points.len()];
        area += points[index].east * next.north - next.east * points[index].north;
    }
    area * 0.5
}

fn is_ear(points: &[EastNorth], ring: &[usize], prev: usize, current: usize, next: usize) -> bool {
    let a = points[prev];
    let b = points[current];
    let c = points[next];
    let cross = (b.east - a.east) * (c.north - a.north) - (b.north - a.north) * (c.east - a.east);
    if cross <= 1e-9 {
        return false;
    }
    !ring.iter().any(|other| {
        *other != prev
            && *other != current
            && *other != next
            && point_in_triangle(points[*other], a, b, c)
    })
}

fn point_in_triangle(p: EastNorth, a: EastNorth, b: EastNorth, c: EastNorth) -> bool {
    let ab = (b.east - a.east) * (p.north - a.north) - (b.north - a.north) * (p.east - a.east);
    let bc = (c.east - b.east) * (p.north - b.north) - (c.north - b.north) * (p.east - b.east);
    let ca = (a.east - c.east) * (p.north - c.north) - (a.north - c.north) * (p.east - c.east);
    ab >= -1e-9 && bc >= -1e-9 && ca >= -1e-9
}

fn div_floor(value: f64, size: f64) -> i32 {
    (value / size).floor() as i32
}
