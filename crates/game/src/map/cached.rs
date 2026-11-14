use std::{collections::HashSet, fs, path::Path};

use bevy::math::DVec2;
use paket_geo::{LatLon, Scene, Surface, parse_overpass, unproject};

use super::{KREUZBERG, MAUSOHRWEG, TILE, Tile, project};

#[derive(Default)]
pub(super) struct CachedMap {
    areas: Vec<Area>,
}

struct Area {
    origin: LatLon,
    scene: Scene,
}

impl CachedMap {
    pub(super) fn read(cache: &Path) -> Self {
        let mut areas = Vec::new();
        let Ok(entries) = fs::read_dir(cache) else {
            return Self { areas };
        };
        let files: Vec<_> = entries.flatten().map(|entry| entry.path()).collect();
        for origin in [MAUSOHRWEG, KREUZBERG] {
            let prefix = format!("{:.5}_{:.5}_", origin.lat, origin.lon);
            let mut seen = HashSet::new();
            let mut scene = Scene {
                buildings: vec![],
                streets: vec![],
                places: vec![],
                skipped: 0,
            };
            for path in &files {
                if !path
                    .file_name()
                    .is_some_and(|name| name.to_string_lossy().starts_with(&prefix))
                {
                    continue;
                }
                let Ok(bytes) = fs::read(path) else {
                    continue;
                };
                let Ok(tile) = parse_overpass(&bytes, origin) else {
                    continue;
                };
                scene.buildings.extend(
                    tile.buildings
                        .into_iter()
                        .filter(|item| seen.insert(item.id.clone())),
                );
                scene.streets.extend(
                    tile.streets
                        .into_iter()
                        .filter(|item| seen.insert(item.id.clone())),
                );
            }
            if !scene.streets.is_empty() {
                areas.push(Area { origin, scene });
            }
        }
        Self { areas }
    }

    pub(super) fn raster(&self, tile: Tile) -> Option<Vec<u8>> {
        let size = TILE as usize;
        let offset = DVec2::new(tile.x as f64, tile.y as f64) * TILE;
        let mut pixels = [222, 227, 214, 255].repeat(size * size);
        let mut drew = false;
        for area in &self.areas {
            let pixel = |point| project(unproject(area.origin, point), tile.z) - offset;
            for street in &area.scene.streets {
                let color = match street.surface {
                    Surface::Asphalt if street.width_m > 3.5 => [250, 245, 231, 255],
                    Surface::Asphalt => [241, 239, 229, 255],
                    Surface::Paving => [227, 218, 202, 255],
                    Surface::Gravel => [208, 197, 169, 255],
                };
                let radius = (street.width_m / 2.0).ceil().clamp(1.0, 4.0) as i32;
                for pair in street.centerline.windows(2) {
                    drew |= line(&mut pixels, pixel(pair[0]), pixel(pair[1]), radius, color);
                }
            }
            if tile.z >= 15 {
                for building in &area.scene.buildings {
                    let color = if building.accessory {
                        [187, 182, 171, 255]
                    } else {
                        [172, 153, 136, 255]
                    };
                    for (a, b) in building
                        .footprint
                        .iter()
                        .zip(building.footprint.iter().cycle().skip(1))
                    {
                        drew |= line(&mut pixels, pixel(*a), pixel(*b), 0, color);
                    }
                }
            }
        }
        drew.then_some(pixels)
    }
}

fn line(pixels: &mut [u8], a: DVec2, b: DVec2, radius: i32, color: [u8; 4]) -> bool {
    let size = TILE as i32;
    let margin = f64::from(radius);
    if a.min(b).max_element().is_nan()
        || a.max(b).min_element() < -margin
        || a.min(b).max_element() > TILE + margin
    {
        return false;
    }
    let delta = b - a;
    // Limit the walk to the part of the line that crosses this tile.
    let mut from: f64 = 0.0;
    let mut to: f64 = 1.0;
    for axis in 0..2 {
        if delta[axis].abs() > f64::EPSILON {
            let low = (-margin - a[axis]) / delta[axis];
            let high = (TILE + margin - a[axis]) / delta[axis];
            from = from.max(low.min(high));
            to = to.min(low.max(high));
        }
    }
    if from > to {
        return false;
    }
    let length = (delta.abs().max_element() * (to - from)).ceil().max(1.0) as i32;
    for i in 0..=length {
        let t = from + (to - from) * f64::from(i) / f64::from(length);
        let p = (a + delta * t).round().as_ivec2();
        for y in (p.y - radius).max(0)..=(p.y + radius).min(size - 1) {
            for x in (p.x - radius).max(0)..=(p.x + radius).min(size - 1) {
                let index = (y * size + x) as usize * 4;
                pixels[index..index + 4].copy_from_slice(&color);
            }
        }
    }
    true
}
