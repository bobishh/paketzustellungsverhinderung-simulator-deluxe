//! Neighbourhood objects and mapped structures in street coordinates.
use crate::{EastNorth, Element, LatLon, landscape, project};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Playground,
    Bench,
    Tree,
    Stadium,
    Pitch { soccer: bool },
    Bridge,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Place {
    pub id: String,
    pub kind: Kind,
    pub position: EastNorth,
    pub outline: Vec<EastNorth>,
    pub holes: Vec<Vec<EastNorth>>,
}

pub(super) fn parse(element: &Element, origin: LatLon) -> Vec<Place> {
    let tag = |key: &str| element.tags.get(key).map(String::as_str);
    let kind = match (
        tag("leisure"),
        tag("amenity"),
        tag("natural"),
        tag("man_made"),
    ) {
        (_, _, _, Some("bridge")) => Kind::Bridge,
        (Some("stadium"), _, _, _) => Kind::Stadium,
        (Some("pitch"), _, _, _) => Kind::Pitch {
            soccer: tag("sport")
                .is_some_and(|value| value.split(';').any(|sport| sport == "soccer")),
        },
        (Some("playground"), _, _, _) if !matches!(tag("access"), Some("private" | "no")) => {
            Kind::Playground
        }
        (_, Some("bench"), _, _) if !matches!(tag("access"), Some("private" | "no")) => Kind::Bench,
        (_, _, Some("tree"), _) if !matches!(tag("access"), Some("private" | "no")) => Kind::Tree,
        _ => return vec![],
    };
    let id = format!("{}/{}", element.kind, element.id);
    let valid = |lat: f64, lon: f64| {
        lat.is_finite() && lat.abs() <= 85. && lon.is_finite() && lon.abs() <= 180.
    };
    if element.kind == "node" {
        return match element.lat.zip(element.lon) {
            Some((lat, lon))
                if valid(lat, lon)
                    && matches!(kind, Kind::Playground | Kind::Bench | Kind::Tree) =>
            {
                vec![Place {
                    id,
                    kind,
                    position: project(origin, LatLon { lat, lon }),
                    outline: vec![],
                    holes: vec![],
                }]
            }
            _ => vec![],
        };
    }
    if matches!(kind, Kind::Bench | Kind::Tree) {
        return vec![];
    }
    let project_ring = |ring: Vec<crate::LatLonJson>| {
        ring.iter().all(|p| valid(p.lat, p.lon)).then(|| {
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
        })
    };
    let members = |inner: bool| {
        landscape::stitch(
            element
                .members
                .iter()
                .filter(|m| {
                    m.role == if inner { "inner" } else { "outer" } || (!inner && m.role.is_empty())
                })
                .filter(|m| m.geometry.len() >= 2)
                .map(|m| m.geometry.clone())
                .collect(),
        )
    };
    let outer = if element.kind == "relation" {
        members(false)
    } else {
        landscape::stitch(element.geometry.clone().into_iter().collect())
    };
    let mut places = outer
        .into_iter()
        .filter_map(project_ring)
        .enumerate()
        .map(|(index, outline)| {
            let position = EastNorth {
                east: outline.iter().map(|p| p.east).sum::<f64>() / outline.len() as f64,
                north: outline.iter().map(|p| p.north).sum::<f64>() / outline.len() as f64,
            };
            Place {
                id: if index == 0 {
                    id.clone()
                } else {
                    format!("{id}/{index}")
                },
                kind,
                position,
                outline,
                holes: vec![],
            }
        })
        .collect::<Vec<_>>();
    for hole in members(true).into_iter().filter_map(project_ring) {
        if let Some(place) = places
            .iter_mut()
            .find(|p| landscape::in_ring(hole[0], &p.outline))
        {
            place.holes.push(hole);
        }
    }
    places
}
