//! Natural surfaces from OpenStreetMap. A surface keeps its outer ring and islands.
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::Path,
    time::Duration,
};

use serde::Deserialize;

use crate::{EastNorth, LatLon, project, unproject};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cover {
    Water,
    Woodland,
}

#[derive(Debug)]
pub struct Area {
    pub cover: Cover,
    pub rings: Vec<Vec<EastNorth>>,
}

impl Area {
    pub fn mesh(&self) -> Option<(Vec<EastNorth>, Vec<usize>)> {
        let mut points = Vec::new();
        let mut holes = Vec::new();
        for ring in &self.rings {
            if !points.is_empty() {
                holes.push(points.len());
            }
            points.extend_from_slice(ring);
        }
        let coordinates: Vec<_> = points.iter().flat_map(|p| [p.east, p.north]).collect();
        let indices = earcutr::earcut(&coordinates, &holes, 2).ok()?;
        (!indices.is_empty()).then_some((points, indices))
    }

    pub fn contains(&self, p: EastNorth) -> bool {
        self.rings.first().is_some_and(|outer| in_ring(p, outer))
            && self.rings.iter().skip(1).all(|hole| !in_ring(p, hole))
    }
}

fn in_ring(p: EastNorth, ring: &[EastNorth]) -> bool {
    ring.iter()
        .zip(ring.iter().cycle().skip(1))
        .fold(false, |inside, (a, b)| {
            inside
                ^ ((a.north > p.north) != (b.north > p.north)
                    && p.east
                        < (b.east - a.east) * (p.north - a.north) / (b.north - a.north) + a.east)
        })
}

#[derive(Clone, Copy, PartialEq, Deserialize)]
struct Point {
    lat: f64,
    lon: f64,
}

#[derive(Deserialize)]
struct Document {
    elements: Vec<Element>,
}

#[derive(Deserialize)]
struct Element {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    tags: HashMap<String, String>,
    #[serde(default)]
    geometry: Vec<Point>,
    #[serde(default)]
    members: Vec<Member>,
    id: u64,
}

#[derive(Deserialize)]
struct Member {
    #[serde(rename = "ref")]
    id: u64,
    role: String,
    #[serde(default)]
    geometry: Vec<Point>,
}

fn cover(tags: &HashMap<String, String>) -> Option<Cover> {
    let has = |key, value| tags.get(key).is_some_and(|tag| tag == value);
    if has("natural", "water") || has("waterway", "riverbank") {
        Some(Cover::Water)
    } else if has("natural", "wood") || has("landuse", "forest") {
        Some(Cover::Woodland)
    } else {
        None
    }
}

pub fn parse(bytes: &[u8], origin: LatLon) -> Result<Vec<Area>, String> {
    let document: Document = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
    let project_ring = |ring: Vec<Point>| {
        ring.into_iter()
            .map(|p| {
                project(
                    origin,
                    LatLon {
                        lat: p.lat,
                        lon: p.lon,
                    },
                )
            })
            .collect::<Vec<_>>()
    };
    let mut areas = Vec::new();
    let mut relation_ways = HashSet::new();
    for relation in document.elements.iter().filter(|e| e.kind == "relation") {
        let Some(cover) = cover(&relation.tags) else {
            continue;
        };
        let rings = |role: &str| {
            stitch(
                relation
                    .members
                    .iter()
                    .filter(|member| {
                        member.role == role || (role == "outer" && member.role.is_empty())
                    })
                    .filter(|member| member.geometry.len() >= 2)
                    .map(|member| member.geometry.clone())
                    .collect(),
            )
        };
        let outer = rings("outer");
        if outer.is_empty() {
            continue;
        }
        relation_ways.extend(relation.members.iter().map(|member| member.id));
        let mut polys: Vec<_> = outer
            .into_iter()
            .map(|ring| vec![project_ring(ring)])
            .collect();
        for hole in rings("inner").into_iter().map(project_ring) {
            if let Some(polygon) = polys.iter_mut().find(|rings| in_ring(hole[0], &rings[0])) {
                polygon.push(hole);
            }
        }
        areas.extend(polys.into_iter().map(|rings| Area { cover, rings }));
    }
    for way in document
        .elements
        .into_iter()
        .filter(|e| e.kind == "way" && !relation_ways.contains(&e.id))
    {
        let Some(cover) = cover(&way.tags) else {
            continue;
        };
        areas.extend(stitch(vec![way.geometry]).into_iter().map(|ring| Area {
            cover,
            rings: vec![project_ring(ring)],
        }));
    }
    Ok(areas)
}

fn stitch(mut lines: Vec<Vec<Point>>) -> Vec<Vec<Point>> {
    let mut rings = Vec::new();
    while let Some(mut chain) = lines.pop() {
        if chain.len() < 2 {
            continue;
        }
        while chain.first() != chain.last() {
            let endpoint = *chain.last().unwrap();
            let Some(index) = lines
                .iter()
                .position(|line| line.first() == Some(&endpoint) || line.last() == Some(&endpoint))
            else {
                break;
            };
            let mut next = lines.swap_remove(index);
            if next.last() == Some(&endpoint) {
                next.reverse();
            }
            chain.extend(next.into_iter().skip(1));
        }
        if chain.len() >= 4 && chain.first() == chain.last() {
            chain.pop();
            rings.push(chain);
        }
    }
    rings
}

#[cfg(not(target_arch = "wasm32"))]
pub fn load(cache: &Path, origin: LatLon, network: bool) -> Result<Vec<Area>, String> {
    let path = cache.join(format!(
        "{:.5}_{:.5}_landscape.json",
        origin.lat, origin.lon
    ));
    if let Ok(bytes) = fs::read(&path) {
        return parse(&bytes, origin);
    }
    if !network {
        return Ok(Vec::new());
    }
    let sw = unproject(
        origin,
        EastNorth {
            east: -1650.0,
            north: -1650.0,
        },
    );
    let ne = unproject(
        origin,
        EastNorth {
            east: 1650.0,
            north: 1650.0,
        },
    );
    let bbox = format!("{:.6},{:.6},{:.6},{:.6}", sw.lat, sw.lon, ne.lat, ne.lon);
    let query = format!(
        "[out:json][timeout:16];(wr[\"natural\"~\"^(water|wood)$\"]({bbox});wr[\"landuse\"=\"forest\"]({bbox});wr[\"waterway\"=\"riverbank\"]({bbox}););out geom;"
    );
    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(19))
        .build();
    let mut error = String::new();
    for endpoint in [
        "https://overpass.kumi.systems/api/interpreter",
        "https://overpass-api.de/api/interpreter",
        "https://overpass.openstreetmap.fr/api/interpreter",
    ] {
        match agent
            .post(endpoint)
            .set("User-Agent", "Paket DREHLSS/0.1 (OpenStreetMap land cover)")
            .send_form(&[("data", &query)])
            .and_then(|response| response.into_string().map_err(Into::into))
        {
            Ok(body) => {
                let areas = parse(body.as_bytes(), origin)?;
                if !areas.is_empty() {
                    fs::create_dir_all(cache).map_err(|e| e.to_string())?;
                    let temporary = path.with_extension(format!("{}.tmp", std::process::id()));
                    fs::write(&temporary, body)
                        .and_then(|_| fs::rename(&temporary, &path))
                        .map_err(|e| e.to_string())?;
                    return Ok(areas);
                }
                error = format!("empty landscape from {endpoint}");
            }
            Err(failure) => error = failure.to_string(),
        }
    }
    Err(error)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn relation_joins_ways_and_leaves_water_island_empty() {
        let json = br#"{"elements":[{"type":"relation","id":1,"tags":{"natural":"water"},"members":[
            {"ref":2,"role":"outer","geometry":[{"lat":0,"lon":0},{"lat":0,"lon":4},{"lat":4,"lon":4}]},
            {"ref":3,"role":"outer","geometry":[{"lat":0,"lon":0},{"lat":4,"lon":0},{"lat":4,"lon":4}]},
            {"ref":4,"role":"inner","geometry":[{"lat":1,"lon":1},{"lat":1,"lon":2},{"lat":2,"lon":2},{"lat":2,"lon":1},{"lat":1,"lon":1}]}
        ]}]}"#;
        let origin = LatLon { lat: 0.0, lon: 0.0 };
        let areas = parse(json, origin).unwrap();
        assert_eq!(areas.len(), 1);
        assert!(areas[0].mesh().is_some());
        assert!(areas[0].contains(project(origin, LatLon { lat: 3.0, lon: 3.0 })));
        assert!(!areas[0].contains(project(origin, LatLon { lat: 1.5, lon: 1.5 })));
    }
}
