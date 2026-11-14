//! Public neighbourhood objects from OSM, in the same coordinates as streets.
use crate::{EastNorth, Element, LatLon, project};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Playground,
    Bench,
    Tree,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Place {
    pub id: String,
    pub kind: Kind,
    pub position: EastNorth,
    pub outline: Vec<EastNorth>,
}

pub(super) fn parse(element: &Element, origin: LatLon) -> Option<Place> {
    let tag = |key: &str| element.tags.get(key).map(String::as_str);
    if matches!(tag("access"), Some("private" | "no")) {
        return None;
    }
    let kind = if tag("leisure") == Some("playground") {
        Kind::Playground
    } else if tag("amenity") == Some("bench") {
        Kind::Bench
    } else if tag("natural") == Some("tree") {
        Kind::Tree
    } else {
        return None;
    };
    let valid = |lat: f64, lon: f64| {
        lat.is_finite() && lat.abs() <= 85. && lon.is_finite() && lon.abs() <= 180.
    };
    let mut outline = Vec::new();
    let position = if element.kind == "node" {
        let (lat, lon) = (element.lat?, element.lon?);
        if !valid(lat, lon) {
            return None;
        }
        project(origin, LatLon { lat, lon })
    } else if element.kind == "way" && kind == Kind::Playground {
        let geometry = element.geometry.as_ref()?;
        if geometry.len() < 4 || geometry.iter().any(|p| !valid(p.lat, p.lon)) {
            return None;
        }
        outline = geometry
            .iter()
            .map(|p| {
                project(
                    origin,
                    LatLon {
                        lat: p.lat,
                        lon: p.lon,
                    },
                )
            })
            .collect();
        if outline.first() != outline.last() {
            return None;
        }
        outline.pop();
        EastNorth {
            east: outline.iter().map(|p| p.east).sum::<f64>() / outline.len() as f64,
            north: outline.iter().map(|p| p.north).sum::<f64>() / outline.len() as f64,
        }
    } else {
        return None;
    };
    Some(Place {
        id: format!("{}/{}", element.kind, element.id),
        kind,
        position,
        outline,
    })
}
