//! The facade kit selected for a ride's starting point.
use crate::{Building, LatLon, project};
use serde::Deserialize;
use std::sync::OnceLock;

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Region {
    BritishIsles,
    Nordic,
    CentralEurope,
    AtlanticEurope,
    Mediterranean,
    EasternEurope,
    WestAsiaNorthAfrica,
    WestAfrica,
    EastAfrica,
    SouthernAfrica,
    NorthAmerica,
    LatinHighlands,
    TropicalAmerica,
    SouthAsia,
    EastAsia,
    JapanKorea,
    SoutheastAsia,
    Oceania,
}

/// Landmark geometry replaces the corresponding generic OSM building.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Landmark {
    EiffelTower,
    BerlinTvTower,
}

impl Landmark {
    pub const ALL: [Self; 2] = [Self::EiffelTower, Self::BerlinTvTower];

    pub const fn location(self) -> LatLon {
        match self {
            Self::EiffelTower => LatLon {
                lat: 48.85826,
                lon: 2.29450,
            },
            Self::BerlinTvTower => LatLon {
                lat: 52.52082,
                lon: 13.40942,
            },
        }
    }

    pub const fn osm_id(self) -> &'static str {
        match self {
            Self::EiffelTower => "way/5013364",
            Self::BerlinTvTower => "way/556435241",
        }
    }
    pub(super) fn replaces_vector_part(self, building: &Building, origin: LatLon) -> bool {
        // The vector fallback flattens tower sub-parts into independent buildings
        // and assigns synthetic IDs. Use the tight footprint and main-structure
        // height to preserve nearby low pavilions and ticket offices.
        let radius = match self {
            Self::EiffelTower => 60.0,
            Self::BerlinTvTower => 20.0,
        };
        if building.height < 50.0 {
            return false;
        }
        let center = project(origin, self.location());
        let (east, north) = building
            .footprint
            .iter()
            .fold((0.0, 0.0), |(east, north), point| {
                (east + point.east, north + point.north)
            });
        let count = building.footprint.len() as f64;
        (east / count - center.east).hypot(north / count - center.north) < radius
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WallFinish {
    Lime,
    Stucco,
    Concrete,
    Panels,
    Brick,
    StockBrick,
    Ashlar,
    Rubble,
    Ceramic,
    Mosaic,
    Timber,
    Metal,
    SandRender,
    Adobe,
    Granite,
    Basalt,
}

impl WallFinish {
    pub(super) fn from_osm(value: &str) -> Option<Self> {
        Some(match value.trim() {
            "brick" => Self::Brick,
            "sandstone" | "limestone" => Self::Ashlar,
            "stone" => Self::Rubble,
            "granite" => Self::Granite,
            "basalt" | "lava_stone" => Self::Basalt,
            "concrete" => Self::Concrete,
            "concrete_panels" => Self::Panels,
            "plaster" | "render" => Self::Lime,
            "stucco" => Self::Stucco,
            "tiles" | "ceramic" => Self::Ceramic,
            "mosaic" => Self::Mosaic,
            "wood" | "timber" => Self::Timber,
            "metal" | "steel" => Self::Metal,
            "adobe" | "mudbrick" => Self::Adobe,
            "earth" | "rammed_earth" => Self::SandRender,
            _ => return None,
        })
    }
}

type Ring = Vec<[i32; 2]>;

#[derive(Deserialize)]
struct Area(Region, [i32; 4], Vec<Ring>);

pub fn region_at(point: LatLon) -> Option<Region> {
    if !(-90.0..=90.0).contains(&point.lat) || !(-180.0..=180.0).contains(&point.lon) {
        return None;
    }
    static AREAS: OnceLock<Vec<Area>> = OnceLock::new();
    let areas = AREAS.get_or_init(|| {
        serde_json::from_str(include_str!("architecture-regions.json"))
            .expect("valid bundled facade regions")
    });
    let (x, y) = (point.lon * 1000., point.lat * 1000.);
    // Generalized coastlines can cut out waterfronts and tiny islands.
    // Keep a modest tolerance, then use the neutral kit on open water.
    const COAST_MARGIN: f64 = 55.0;
    let longitude_scale = point.lat.to_radians().cos().max(0.01);
    let mut nearest = None;
    let mut nearest_squared = COAST_MARGIN.powi(2);
    let longitude_margin = COAST_MARGIN / longitude_scale;
    let areas = areas
        .iter()
        .take_while(|Area(_, [west, ..], _)| f64::from(*west) <= x + longitude_margin);
    for Area(region, [west, south, east, north], rings) in areas {
        let (west, south, east, north) = (
            f64::from(*west),
            f64::from(*south),
            f64::from(*east),
            f64::from(*north),
        );
        if x > east + longitude_margin || y < south - COAST_MARGIN || y > north + COAST_MARGIN {
            continue;
        }
        let [outer, holes @ ..] = rings.as_slice() else {
            continue;
        };
        if x >= west && x <= east && y >= south && y <= north && in_ring(outer, x, y) {
            if !holes.iter().any(|ring| in_ring(ring, x, y)) {
                return Some(*region);
            }
        }
        for (&[ax, ay], &[bx, by]) in outer.iter().zip(outer.iter().cycle().skip(1)) {
            let ax = (f64::from(ax) - x) * longitude_scale;
            let ay = f64::from(ay) - y;
            let dx = (f64::from(bx) - x) * longitude_scale - ax;
            let dy = f64::from(by) - y - ay;
            let length_squared = dx * dx + dy * dy;
            if length_squared == 0.0 {
                continue;
            }
            let t = (-(ax * dx + ay * dy) / length_squared).clamp(0.0, 1.0);
            let distance_squared = (ax + t * dx).powi(2) + (ay + t * dy).powi(2);
            if distance_squared < nearest_squared {
                nearest = Some(*region);
                nearest_squared = distance_squared;
            }
        }
    }
    nearest
}

fn in_ring(ring: &[[i32; 2]], x: f64, y: f64) -> bool {
    let mut inside = false;
    for (&[ax, ay], &[bx, by]) in ring.iter().zip(ring.iter().cycle().skip(1)) {
        let (ay, by) = (f64::from(ay), f64::from(by));
        if (ay > y) != (by > y) && x < f64::from(ax) + f64::from(bx - ax) * (y - ay) / (by - ay) {
            inside = !inside;
        }
    }
    inside
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn playable_cities_cover_each_facade_region() {
        use Region::*;
        for (city, lat, lon, region) in [
            ("London", 51.5074, -0.1278, BritishIsles),
            ("Oslo", 59.9139, 10.7522, Nordic),
            ("Berlin", 52.4895, 13.3908, CentralEurope),
            ("Paris", 48.85826, 2.29450, AtlanticEurope),
            ("Lisbon", 38.7223, -9.1393, Mediterranean),
            ("Kyiv", 50.4501, 30.5234, EasternEurope),
            ("Fez", 34.0331, -5.0003, WestAsiaNorthAfrica),
            ("Djenné", 13.9053, -4.5559, WestAfrica),
            ("Zanzibar", -6.1659, 39.1921, EastAfrica),
            ("Cape Town", -33.925, 18.424, SouthernAfrica),
            ("New York", 40.732, -73.996, NorthAmerica),
            ("Cuenca", -2.9001, -79.0059, LatinHighlands),
            ("São Paulo", -23.55, -46.63, TropicalAmerica),
            ("Jaipur", 26.9124, 75.7873, SouthAsia),
            ("Shanghai", 31.2304, 121.4737, EastAsia),
            ("Tokyo", 35.661, 139.668, JapanKorea),
            ("Hội An", 15.8801, 108.3380, SoutheastAsia),
            ("Sydney", -33.879, 151.216, Oceania),
            ("Papeete", -17.533, -149.567, Oceania),
            ("Hong Kong", 22.2819, 114.1577, EastAsia),
            ("Lagos", 6.523, 3.376, WestAfrica),
            ("Tunis", 36.8065, 10.1815, WestAsiaNorthAfrica),
        ] {
            assert_eq!(region_at(LatLon { lat, lon }), Some(region), "{city}");
        }
    }

    #[test]
    fn osm_towers_are_not_generic_buildings_and_explicit_finishes_survive() {
        let raw = br##"{"elements":[
            {"type":"way","id":5013364,"tags":{"building":"tower"},"geometry":[
                {"lat":0,"lon":0},{"lat":0.0001,"lon":0},
                {"lat":0.0001,"lon":0.0001},{"lat":0,"lon":0}]},
            {"type":"way","id":556435241,"tags":{"building":"tower"},"geometry":[
                {"lat":0,"lon":0},{"lat":0.0001,"lon":0},
                {"lat":0.0001,"lon":0.0001},{"lat":0,"lon":0}]},
            {"type":"way","id":42,"tags":{"building":"yes","building:material":"brick","building:colour":"#a76150"},"geometry":[
                {"lat":0,"lon":0},{"lat":0.0001,"lon":0},
                {"lat":0.0001,"lon":0.0001},{"lat":0,"lon":0}]}
        ]}"##;
        let scene = crate::parse_overpass(raw, LatLon { lat: 0., lon: 0. }).unwrap();
        assert_eq!(scene.buildings.len(), 1);
        assert_eq!(scene.buildings[0].id, "way/42");
        assert_eq!(scene.buildings[0].wall_color, Some([167, 97, 80]));
        assert_eq!(scene.buildings[0].wall_finish, Some(WallFinish::Brick));
    }

    #[test]
    fn fallback_tower_parts_do_not_hide_the_custom_landmark() {
        let raw = br#"{"elements":[
            {"type":"way","id":1144217341501222,"tags":{"building":"yes","height":"205"},"geometry":[
                {"lat":52.52078,"lon":13.40937},{"lat":52.52087,"lon":13.40937},
                {"lat":52.52087,"lon":13.40946},{"lat":52.52078,"lon":13.40937}]},
            {"type":"way","id":1144217341501223,"tags":{"building":"yes","height":"8"},"geometry":[
                {"lat":52.52078,"lon":13.40937},{"lat":52.52087,"lon":13.40937},
                {"lat":52.52087,"lon":13.40946},{"lat":52.52078,"lon":13.40937}]}
        ]}"#;
        let scene = crate::parse_overpass(raw, Landmark::BerlinTvTower.location()).unwrap();
        assert_eq!(scene.buildings.len(), 1);
        assert_eq!(scene.buildings[0].height, 8.0);
    }

    #[test]
    fn ocean_and_invalid_points_have_no_region() {
        for (lat, lon) in [
            (0., 0.),
            (30., -45.),
            (-90., 0.),
            (f64::NAN, 15.),
            (90., 190.),
        ] {
            assert_eq!(region_at(LatLon { lat, lon }), None);
        }
    }
}
