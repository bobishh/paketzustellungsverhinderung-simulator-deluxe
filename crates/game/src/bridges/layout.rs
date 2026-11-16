//! Resolve separately mapped sidewalks into the vehicle crossing they describe.
use bevy::prelude::*;
use geo::{LineString, SimplifyIdx};
use paket_geo::{EastNorth, Street, places::Place};
use std::collections::HashSet;

fn xy(p: EastNorth) -> Vec2 {
    Vec2::new(p.east as f32, p.north as f32)
}

fn point(p: Vec2) -> EastNorth {
    EastNorth {
        east: p.x.into(),
        north: p.y.into(),
    }
}

struct Crossing {
    start: f32,
    end: f32,
    sidewalks: [f32; 2],
    layer: i8,
    paths: Vec<usize>,
}

fn crossing(
    a: Vec2,
    b: Vec2,
    road: &Street,
    paths: &[&Street],
    outlines: &[Place],
) -> Option<Crossing> {
    let length = a.distance(b);
    if length < 5. {
        return None;
    }
    let heading = (b - a) / length;
    let normal = Vec2::new(heading.y, -heading.x);
    let coords = |p| {
        let d = xy(p) - a;
        (d.dot(heading), d.dot(normal))
    };
    let half = road.width_m * 0.5;
    let mut sides = [vec![], vec![]];
    for (index, path) in paths.iter().enumerate() {
        let Some(layer) = path.bridge else { continue };
        if road.bridge.is_some_and(|l| l != layer) {
            continue;
        }
        let (mut start, mut u) = coords(path.centerline[0]);
        let (mut end, mut v) = coords(*path.centerline.last().unwrap());
        if start > end {
            std::mem::swap(&mut start, &mut end);
            std::mem::swap(&mut u, &mut v);
        }
        let overlap = end.min(length) - start.max(0.);
        let approach_tolerance = if road.bridge.is_some() { 8. } else { 4. };
        if start < -approach_tolerance
            || end > length + approach_tolerance
            || overlap < 5.
            || overlap < (end - start) * 0.7
            || u * v <= 0.
            || u.abs().min(v.abs()) < (half - 1.).max(1.5)
            || u.abs().max(v.abs()) > (half + 8.).min(20.)
            || (u - v).abs() > (end - start) * 0.25
        {
            continue;
        }
        let width = ((u.abs() + v.abs() + path.width_m) * 0.5 - half).max(1.1);
        sides[usize::from(u > 0.)].push((start, end, width, layer, index));
    }

    let tagged = road
        .bridge
        .filter(|_| sides.iter().any(|side| !side.is_empty()))
        .map(|layer| Crossing {
            start: 0.,
            end: length,
            sidewalks: sides.each_ref().map(|side| {
                side.iter()
                    .map(|&(_, _, width, _, _)| width)
                    .fold(0., f32::max)
            }),
            layer,
            paths: sides.iter().flatten().map(|&(.., index)| index).collect(),
        });

    let mut best = None;
    // A parallel pair flanking a vehicle way is one crossing even when only
    // the OSM sidewalk ways carry a bridge tag.
    for &(a0, a1, left, layer, l) in &sides[0] {
        for &(b0, b1, right, r_layer, r) in &sides[1] {
            let start = a0.min(b0).max(0.);
            let end = a1.max(b1).min(length);
            if layer == r_layer
                && a1.min(b1) - a0.max(b0) >= (end - start) * 0.75
                && best
                    .as_ref()
                    .is_none_or(|c: &Crossing| end - start > c.end - c.start)
            {
                best = Some(Crossing {
                    start,
                    end,
                    sidewalks: [left, right],
                    layer,
                    paths: vec![l, r],
                });
            }
        }
    }

    for outline in outlines {
        let vertices = outline
            .outline
            .iter()
            .copied()
            .map(coords)
            .collect::<Vec<_>>();
        let mut hits = vertices
            .iter()
            .zip(vertices.iter().cycle().skip(1))
            .filter_map(|(&(x, y), &(u, v))| {
                ((y > 0.) != (v > 0.)).then(|| x + (u - x) * y / (y - v))
            })
            .collect::<Vec<_>>();
        hits.sort_by(f32::total_cmp);
        for pair in hits.chunks_exact(2) {
            let start = pair[0].max(0.);
            let end = pair[1].min(length);
            if !(5.0..=300.0).contains(&(end - start))
                || best.as_ref().is_some_and(|c| {
                    end - start + 1. < c.end - c.start || end < c.start || start > c.end
                })
            {
                continue;
            }
            let width = |sign: f32| vertices.iter().map(|v| v.1 * sign).fold(0., f32::max);
            let widths = [width(-1.), width(1.)];
            if widths.iter().any(|w| *w < half - 1. || *w > 25.) {
                continue;
            }
            let paths = sides
                .iter()
                .flatten()
                .filter_map(|&(a, b, _, _, i)| (a >= start - 3. && b <= end + 3.).then_some(i))
                .collect();
            best = Some(Crossing {
                start,
                end,
                sidewalks: widths.map(|w| (w - half).max(0.)),
                layer: road.bridge.unwrap_or(1),
                paths,
            });
        }
    }
    if let Some(mut tagged) = tagged {
        if let Some(best) = best {
            for (width, explicit) in tagged.sidewalks.iter_mut().zip(best.sidewalks) {
                *width = width.max(explicit);
            }
            tagged.paths.extend(best.paths);
        }
        Some(tagged)
    } else {
        best
    }
}

pub(crate) fn resolve(streets: &[Street], outlines: &[Place]) -> Vec<Street> {
    let paths = streets
        .iter()
        .filter(|s| !s.motor_road && s.bridge.is_some())
        .collect::<Vec<_>>();
    if paths.is_empty() && outlines.is_empty() {
        return streets.to_vec();
    }
    let mut removed = HashSet::new();
    let mut result = Vec::new();
    for road in streets.iter().filter(|s| s.motor_road) {
        // OSM often inserts extra nodes in the road while mapping the side
        // paths from bank to bank. Match against whole nearly straight runs.
        let line = LineString::from_iter(road.centerline.iter().map(|p| (p.east, p.north)));
        let runs = line.simplify_idx(&0.75);
        let crossings = runs
            .windows(2)
            .map(|run| {
                crossing(
                    xy(road.centerline[run[0]]),
                    xy(road.centerline[run[1]]),
                    road,
                    &paths,
                    outlines,
                )
            })
            .collect::<Vec<_>>();
        if road.bridge.is_some() || crossings.iter().all(Option::is_none) {
            let mut street = road.clone();
            for c in crossings.into_iter().flatten() {
                removed.extend(c.paths);
                for (w, ext) in street.sidewalk_m.iter_mut().zip(c.sidewalks) {
                    *w = w.max(ext);
                }
            }
            result.push(street);
            continue;
        }
        let mut pieces: Vec<Street> = Vec::new();
        for (run, c) in runs.windows(2).zip(crossings) {
            let start = road.centerline[run[0]];
            let end = road.centerline[run[1]];
            let mut append = |a, b, bridge, sidewalk_m| {
                if xy(a).distance_squared(xy(b)) < 0.0001 {
                    return;
                }
                if let Some(last) = pieces.last_mut()
                    && last.bridge == bridge
                    && last.sidewalk_m == sidewalk_m
                {
                    last.centerline.push(b);
                } else {
                    pieces.push(Street {
                        centerline: vec![a, b],
                        bridge,
                        sidewalk_m,
                        ..road.clone()
                    });
                }
            };
            if let Some(c) = c {
                let a = xy(start);
                let d = (xy(end) - a).normalize_or_zero();
                let lo = point(a + d * c.start);
                let hi = point(a + d * c.end);
                removed.extend(c.paths);
                append(start, lo, None, road.sidewalk_m);
                append(lo, hi, Some(c.layer), c.sidewalks);
                append(hi, end, None, road.sidewalk_m);
            } else {
                for pair in road.centerline[run[0]..=run[1]].windows(2) {
                    append(pair[0], pair[1], None, road.sidewalk_m);
                }
            }
        }
        result.extend(pieces);
    }
    let removed_ids = removed
        .into_iter()
        .map(|i| paths[i].id.as_str())
        .collect::<HashSet<_>>();
    result.extend(
        streets
            .iter()
            .filter(|s| !s.motor_road && !removed_ids.contains(s.id.as_str()))
            .cloned(),
    );
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn humboldtstrasse_has_one_road_deck_wide_enough_for_both_sidewalks() {
        let scene = paket_geo::parse_overpass(
            include_bytes!("../../../../fixtures/humboldt-bridge.json"),
            paket_geo::LatLon {
                lat: 52.27,
                lon: 10.5325,
            },
        )
        .unwrap();
        let layout = resolve(&scene.streets, &[]);
        let decks = layout
            .iter()
            .filter(|s| s.bridge.is_some())
            .collect::<Vec<_>>();
        assert_eq!(decks.len(), 1);
        let deck = decks[0];
        assert!(deck.motor_road);
        assert!(deck.sidewalk_m.iter().all(|&w| w > 1.6));
        assert!(deck.width_m >= 8.);
        assert!(scene.streets.iter().any(|s| s.motor_road && s.spawnable));

        let footways = scene
            .streets
            .into_iter()
            .filter(|s| !s.motor_road)
            .collect::<Vec<_>>();
        assert_eq!(resolve(&footways, &[]).len(), 2);
    }

    #[test]
    fn jasperallee_attaches_its_single_mapped_sidewalk_to_the_road_bridge() {
        let scene = paket_geo::parse_overpass(
            include_bytes!("../../../../fixtures/jasperallee-bridge.json"),
            paket_geo::LatLon {
                lat: 52.266,
                lon: 10.534,
            },
        )
        .unwrap();
        let layout = resolve(&scene.streets, &[]);
        assert_eq!(layout.len(), 1);
        assert!(layout[0].motor_road && layout[0].bridge.is_some());
        assert!(layout[0].sidewalk_m.iter().any(|&width| width > 1.6));
    }

    #[test]
    fn mapped_outline_elevates_an_untagged_road_without_extending_the_deck_onto_land() {
        let center = point(Vec2::ZERO);
        let road = Street {
            id: "road".into(),
            centerline: vec![point(Vec2::new(-50., 0.)), point(Vec2::new(50., 0.))],
            width_m: 8.,
            sidewalk_m: [0., 0.],
            name: None,
            surface: paket_geo::Surface::Asphalt,
            spawnable: true,
            motor_road: true,
            bridge: None,
        };
        let outline = Place {
            id: "deck".into(),
            kind: paket_geo::places::Kind::Bridge,
            position: center,
            outline: [
                Vec2::new(-20., -7.),
                Vec2::new(20., -7.),
                Vec2::new(20., 7.),
                Vec2::new(-20., 7.),
            ]
            .into_iter()
            .map(point)
            .collect(),
            holes: vec![],
        };
        let layout = resolve(&[road], &[outline]);
        assert_eq!(layout.len(), 3);
        assert_eq!(
            layout[1].centerline,
            vec![point(Vec2::new(-20., 0.)), point(Vec2::new(20., 0.))]
        );
        assert_eq!(layout[1].sidewalk_m, [3., 3.]);
        assert_eq!(
            [layout[0].bridge, layout[1].bridge, layout[2].bridge],
            [None, Some(1), None]
        );
    }
}
