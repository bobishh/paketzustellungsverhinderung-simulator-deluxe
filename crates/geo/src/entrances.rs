use super::{Building, EastNorth, Street, polygon_area};

const MAX_APPROACH_METERS: f64 = 65.0;
const COURIER_RADIUS: f64 = 0.3;

type Segment = (EastNorth, EastNorth);

pub(super) fn assign(buildings: &mut [Building], streets: &[Street], public: &[bool]) {
    let streets: Vec<_> = streets
        .iter()
        .zip(public)
        .flat_map(|(street, public)| {
            street
                .centerline
                .windows(2)
                .map(|pair| ((pair[0], pair[1]), *public))
        })
        .collect();
    let walls: Vec<_> = buildings
        .iter()
        .enumerate()
        .filter(|(_, building)| !building.open_sided)
        .flat_map(|(index, building)| {
            edges(building)
                .enumerate()
                .map(move |(edge, segment)| (index, edge, segment))
        })
        .collect();
    let entrances: Vec<_> = buildings
        .iter()
        .enumerate()
        .map(|(index, building)| {
            let winding = polygon_area(&building.footprint).signum();
            let mut nearest = (f64::INFINITY, building.entrance);
            let mut accessible = None;
            let mut best = MAX_APPROACH_METERS * MAX_APPROACH_METERS;
            for (edge, (a, b)) in edges(building).enumerate() {
                let wall = subtract(b, a);
                let half = wall.east.hypot(wall.north) * 0.5;
                if half < 0.9
                    || building
                        .passages
                        .iter()
                        .any(|p| p.edge == edge && half + 0.85 > p.start && half - 0.85 < p.end)
                {
                    continue;
                }
                let midpoint = EastNorth {
                    east: (a.east + b.east) * 0.5,
                    north: (a.north + b.north) * 0.5,
                };
                for &(street, public) in &streets {
                    let target = nearest_point(midpoint, street);
                    let approach = subtract(target, midpoint);
                    let distance = dot(approach, approach);
                    if distance < nearest.0 {
                        nearest = (distance, midpoint);
                    }
                    if !public || distance >= best || winding * cross(wall, approach) >= 0.0 {
                        continue;
                    }
                    let blocked = walls.iter().any(|&(other, side, segment)| {
                        if (index, edge) == (other, side) {
                            return false;
                        }
                        crossing((midpoint, target), segment).is_some_and(|offset| {
                            let span = subtract(segment.1, segment.0);
                            let offset = offset * span.east.hypot(span.north);
                            !buildings[other].passages.iter().any(|p| {
                                p.edge == side
                                    && p.height >= 1.8
                                    && offset >= p.start + COURIER_RADIUS
                                    && offset <= p.end - COURIER_RADIUS
                            })
                        })
                    });
                    if !blocked {
                        best = distance;
                        accessible = Some(midpoint);
                    }
                }
            }
            (accessible.unwrap_or(nearest.1), accessible.is_some())
        })
        .collect();
    for (building, (entrance, accessible)) in buildings.iter_mut().zip(entrances) {
        building.entrance = entrance;
        building.street_entrance = accessible;
    }
}

fn edges(building: &Building) -> impl Iterator<Item = Segment> + '_ {
    let points = &building.footprint;
    points
        .iter()
        .copied()
        .zip(points.iter().copied().cycle().skip(1))
}

fn subtract(a: EastNorth, b: EastNorth) -> EastNorth {
    EastNorth {
        east: a.east - b.east,
        north: a.north - b.north,
    }
}

fn cross(a: EastNorth, b: EastNorth) -> f64 {
    a.east * b.north - a.north * b.east
}

fn dot(a: EastNorth, b: EastNorth) -> f64 {
    a.east * b.east + a.north * b.north
}

fn nearest_point(point: EastNorth, (a, b): Segment) -> EastNorth {
    let span = subtract(b, a);
    let length = dot(span, span);
    let t = (dot(subtract(point, a), span) / length.max(1e-10)).clamp(0.0, 1.0);
    EastNorth {
        east: a.east + span.east * t,
        north: a.north + span.north * t,
    }
}

fn crossing((a, b): Segment, (c, d): Segment) -> Option<f64> {
    if a.east.max(b.east) < c.east.min(d.east)
        || c.east.max(d.east) < a.east.min(b.east)
        || a.north.max(b.north) < c.north.min(d.north)
        || c.north.max(d.north) < a.north.min(b.north)
    {
        return None;
    }
    let ray = subtract(b, a);
    let wall = subtract(d, c);
    let determinant = cross(ray, wall);
    if determinant.abs() < 1e-8 {
        return None;
    }
    let start = subtract(c, a);
    let along_ray = cross(start, wall) / determinant;
    let along_wall = cross(start, ray) / determinant;
    // A shared facade blocks the approach at the threshold itself.
    let unit = -1e-8..=1.0 + 1e-8;
    (unit.contains(&along_ray) && unit.contains(&along_wall)).then_some(along_wall)
}
