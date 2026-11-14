use std::collections::HashMap;

use paket_geo::{
    EastNorth, FIXTURE_LAT, FIXTURE_LON, LatLon, confirm_delivery, parse_overpass, project,
    tile_id, triangulate, within_delivery_range,
};

fn origin() -> LatLon {
    LatLon {
        lat: FIXTURE_LAT,
        lon: FIXTURE_LON,
    }
}

#[test]
fn real_berlin_footbridges_keep_their_bridge_layer() {
    let scene = parse_overpass(
        include_bytes!("../../../fixtures/berlin-bridges.json"),
        origin(),
    )
    .unwrap();
    assert_eq!(scene.streets.len(), 2);
    assert!(
        scene
            .streets
            .iter()
            .all(|s| s.bridge == Some(1) && !s.spawnable)
    );
    let normal =
        parse_overpass(include_bytes!("../../../fixtures/kreuzberg.json"), origin()).unwrap();
    assert!(normal.streets.iter().all(|s| s.bridge.is_none()));
}

#[test]
fn known_point_projects_within_a_decimeter() {
    let point = project(
        LatLon {
            lat: 52.52,
            lon: 13.405,
        },
        LatLon {
            lat: 52.521,
            lon: 13.406,
        },
    );
    assert!((point.east - 67.7362).abs() < 0.1, "east {}", point.east);
    assert!(
        (point.north - 111.3195).abs() < 0.1,
        "north {}",
        point.north
    );
    let back = project(origin(), origin());
    assert!(back.east.abs() < 1e-6 && back.north.abs() < 1e-6);
}

#[test]
fn height_uses_meters_then_levels_then_nine() {
    let mut tags = HashMap::from([("height".into(), "12".into())]);
    assert_eq!(paket_geo::building_height(&tags), 12.0);
    tags.remove("height");
    tags.insert("building:levels".into(), "4".into());
    assert_eq!(paket_geo::building_height(&tags), 12.0);
    tags.clear();
    assert_eq!(paket_geo::building_height(&tags), 9.0);
}

#[test]
fn fixture_keeps_stable_entrances_and_skips_nothing_playable() {
    let bytes = include_bytes!("../../../fixtures/kreuzberg.json");
    let first = parse_overpass(bytes, origin()).unwrap();
    let second = parse_overpass(bytes, origin()).unwrap();
    assert!(first.buildings.len() >= 8);
    assert!(!first.streets.is_empty());
    assert_eq!(first.skipped, 0);
    for (left, right) in first.buildings.iter().zip(second.buildings.iter()) {
        assert_eq!(left.id, right.id);
        assert!((left.entrance.east - right.entrance.east).abs() < 1e-6);
        assert!((left.entrance.north - right.entrance.north).abs() < 1e-6);
    }
    let tall = first
        .buildings
        .iter()
        .find(|building| building.id == "way/201")
        .unwrap();
    assert_eq!(tall.height, 12.0);
    let levels = first
        .buildings
        .iter()
        .find(|building| building.id == "way/202")
        .unwrap();
    assert_eq!(levels.height, 12.0);
    let plain = first
        .buildings
        .iter()
        .find(|building| building.id == "way/203")
        .unwrap();
    assert_eq!(plain.height, 9.0);
}

#[test]
fn street_width_and_sidewalk_follow_osm_tags() {
    let json = br#"{"elements":[
        {"type":"way","id":1,"tags":{"highway":"residential","width":"6.5 m","sidewalk":"right"},
         "geometry":[{"lat":52.4895,"lon":13.3908},{"lat":52.4896,"lon":13.3908}]},
        {"type":"way","id":2,"tags":{"highway":"footway","width":"unknown"},
         "geometry":[{"lat":52.4895,"lon":13.3908},{"lat":52.4895,"lon":13.3909}]}
    ]}"#;
    let scene = parse_overpass(json, origin()).unwrap();
    assert_eq!(scene.streets[0].width_m, 6.5);
    assert_eq!(scene.streets[0].sidewalk_m, [0.0, 1.4]);
    assert_eq!(scene.streets[1].width_m, 1.8);
    assert_eq!(scene.streets[1].sidewalk_m, [0.0, 0.0]);
}

#[test]
fn broken_ring_is_skipped() {
    let json = br#"{"elements":[{"type":"way","id":1,"tags":{"building":"yes"},"geometry":[{"lat":52.4895,"lon":13.3908},{"lat":52.4896,"lon":13.3908}]}]}"#;
    let scene = parse_overpass(json, origin()).unwrap();
    assert!(scene.buildings.is_empty());
    assert_eq!(scene.skipped, 1);
}

#[test]
fn square_triangulates_and_concave_l_does_too() {
    let square = [
        EastNorth {
            east: 0.0,
            north: 0.0,
        },
        EastNorth {
            east: 10.0,
            north: 0.0,
        },
        EastNorth {
            east: 10.0,
            north: 4.0,
        },
        EastNorth {
            east: 0.0,
            north: 4.0,
        },
    ];
    assert_eq!(triangulate(&square).unwrap().len(), 2);
    let ell = [
        EastNorth {
            east: 0.0,
            north: 0.0,
        },
        EastNorth {
            east: 6.0,
            north: 0.0,
        },
        EastNorth {
            east: 6.0,
            north: 2.0,
        },
        EastNorth {
            east: 2.0,
            north: 2.0,
        },
        EastNorth {
            east: 2.0,
            north: 5.0,
        },
        EastNorth {
            east: 0.0,
            north: 5.0,
        },
    ];
    assert!(triangulate(&ell).unwrap().len() >= 4);
}

#[test]
fn delivery_accepts_only_the_first_close_confirmation() {
    assert!(!within_delivery_range(3.01));
    assert!(within_delivery_range(3.0));
    let mut recorded = None;
    assert!(!confirm_delivery(&mut recorded, "way/1", 3.2));
    assert!(recorded.is_none());
    assert!(confirm_delivery(&mut recorded, "way/1", 2.9));
    assert_eq!(recorded.as_deref(), Some("way/1"));
    assert!(!confirm_delivery(&mut recorded, "way/1", 0.1));
    assert_eq!(recorded.as_deref(), Some("way/1"));
}

#[test]
fn tile_ids_cover_the_plane_without_a_maximum() {
    assert_eq!(
        tile_id(EastNorth {
            east: 10.0,
            north: -10.0
        })
        .y,
        -1
    );
    assert_eq!(
        tile_id(EastNorth {
            east: 10.0,
            north: 10.0
        })
        .x,
        0
    );
    assert_eq!(
        tile_id(EastNorth {
            east: 400.0,
            north: 20.0
        })
        .x,
        2
    );
}

#[test]
fn mausohrweg_style_outbuildings_keep_their_scale() {
    for (kind, height) in [
        ("carport", 2.6),
        ("shed", 2.6),
        ("garage", 2.6),
        ("house", 5.8),
    ] {
        let tags = HashMap::from([("building".into(), kind.into())]);
        assert_eq!(paket_geo::building_height(&tags), height, "{kind}");
    }
}

#[test]
fn platforms_and_steps_do_not_become_bicycle_roads() {
    let json = br#"{"elements":[
        {"type":"way","id":1,"tags":{"highway":"platform"},"geometry":[{"lat":52.4895,"lon":13.3908},{"lat":52.4896,"lon":13.3908}]},
        {"type":"way","id":2,"tags":{"highway":"steps"},"geometry":[{"lat":52.4895,"lon":13.3908},{"lat":52.4896,"lon":13.3908}]}
    ]}"#;
    assert!(parse_overpass(json, origin()).unwrap().streets.is_empty());
}

#[test]
fn neighbourhood_includes_real_playgrounds_benches_and_trees() {
    let origin = paket_geo::LatLon { lat: 0., lon: 0. };
    let scene = paket_geo::parse_overpass(br#"{"elements":[
      {"type":"way","id":1,"tags":{"leisure":"playground"},"geometry":[{"lat":0,"lon":0},{"lat":0,"lon":0.0002},{"lat":0.0002,"lon":0.0002},{"lat":0.0002,"lon":0},{"lat":0,"lon":0}]},
      {"type":"node","id":2,"lat":0.0001,"lon":0.0001,"tags":{"amenity":"bench"}},
      {"type":"node","id":3,"lat":0.0001,"lon":0.0003,"tags":{"natural":"tree"}},
      {"type":"node","id":4,"lat":0,"lon":0,"tags":{"leisure":"playground","access":"private"}}
    ]}"#, origin).unwrap();
    assert_eq!(scene.places.len(), 3);
    assert_eq!(scene.places[0].id, "way/1");
    assert_eq!(scene.places[0].kind, paket_geo::places::Kind::Playground);
    assert_eq!(scene.places[0].outline.len(), 4);
    assert_eq!(scene.places[1].kind, paket_geo::places::Kind::Bench);
    assert_eq!(scene.places[2].kind, paket_geo::places::Kind::Tree);
}
