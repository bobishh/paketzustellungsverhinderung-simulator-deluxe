//! Builds façades and roofs from the dimensions and tags of each OSM footprint.
use bevy::{
    asset::RenderAssetUsages,
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
};
use paket_geo::{Building, Roof};

use crate::to_world;

const PLASTER: [[u8; 3]; 6] = [
    [205, 196, 175],
    [219, 208, 185],
    [181, 166, 147],
    [214, 197, 159],
    [172, 124, 108],
    [195, 194, 179],
];

pub(crate) const DOOR_WIDTH: f32 = 1.28;
pub(crate) const DOOR_HEIGHT: f32 = 2.18;
const WALL_THICKNESS: f32 = 0.13;

// Adjacent inside faces must use the same corner. Offsetting each wall on its
// own leaves a sky-visible slit at every concave corner of a courtyard.
fn inside_corners(points: &[Vec3], winding: f32) -> Vec<Vec3> {
    points
        .iter()
        .enumerate()
        .map(|(i, &p)| {
            let previous = (p - points[(i + points.len() - 1) % points.len()]).normalize_or_zero();
            let next = (points[(i + 1) % points.len()] - p).normalize_or_zero();
            let a = previous.cross(Vec3::Y) * winding;
            let b = next.cross(Vec3::Y) * winding;
            let inset = (a + b) * (WALL_THICKNESS / (1. + a.dot(b)).max(0.001));
            // Very acute OSM corners get a short bevel instead of a long spike.
            p + inset.clamp_length_max(0.5)
        })
        .collect()
}

pub(crate) fn doorway(building: &Building) -> (usize, Vec3, Vec3) {
    let target = to_world(building.entrance, 0.0);
    let points: Vec<_> = building
        .footprint
        .iter()
        .map(|p| to_world(*p, 0.0))
        .collect();
    let winding = points
        .iter()
        .zip(points.iter().cycle().skip(1))
        .map(|(a, b)| a.x * b.z - a.z * b.x)
        .sum::<f32>()
        .signum();
    points
        .iter()
        .zip(points.iter().cycle().skip(1))
        .enumerate()
        .filter(|(edge, (a, b))| {
            let half = a.distance(**b) * 0.5;
            half >= (DOOR_WIDTH + 0.4) * 0.5
                && !building.passages.iter().any(|p| {
                    p.edge == *edge
                        && f64::from(half + DOOR_WIDTH * 0.5 + 0.2) > p.start
                        && f64::from(half - DOOR_WIDTH * 0.5 - 0.2) < p.end
                })
        })
        .map(|(i, (a, b))| {
            (
                i,
                (*a + *b) * 0.5,
                (*b - *a).normalize().cross(Vec3::Y) * -winding,
            )
        })
        .min_by(|a, b| {
            a.1.distance_squared(target)
                .total_cmp(&b.1.distance_squared(target))
        })
        .unwrap_or((0, target, Vec3::NEG_Z))
}

// A shared wall partition drives both visible masonry and physics, so an arch
// cannot look open while retaining an invisible collider (or the reverse).
fn openings(building: &Building, edge: usize) -> Vec<(f32, f32, f32)> {
    let a = to_world(building.footprint[edge], 0.0);
    let b = to_world(
        building.footprint[(edge + 1) % building.footprint.len()],
        0.0,
    );
    let width = a.distance(b);
    let mut gaps: Vec<_> = building
        .passages
        .iter()
        .filter(|p| p.edge == edge)
        .map(|p| (p.start as f32, p.end as f32, p.height as f32))
        .collect();
    if !building.accessory && doorway(building).0 == edge && width > DOOR_WIDTH + 0.2 {
        gaps.push((
            (width - DOOR_WIDTH) * 0.5,
            (width + DOOR_WIDTH) * 0.5,
            DOOR_HEIGHT,
        ));
    }
    gaps
}
pub(crate) fn wall_sections(building: &Building, edge: usize) -> Vec<(f32, f32, f32, f32)> {
    let a = building.footprint[edge];
    let b = building.footprint[(edge + 1) % building.footprint.len()];
    let width = (b.east - a.east).hypot(b.north - a.north) as f32;
    let gaps = openings(building, edge);
    let mut cuts = vec![0.0, width];
    for &(start, end, _) in &gaps {
        cuts.extend([start.clamp(0.0, width), end.clamp(0.0, width)]);
    }
    cuts.sort_by(f32::total_cmp);
    cuts.dedup_by(|a, b| (*a - *b).abs() < 0.001);
    cuts.windows(2)
        .filter_map(|s| {
            let mid = (s[0] + s[1]) * 0.5;
            let bottom = gaps
                .iter()
                .filter(|&&(a, b, _)| mid >= a && mid <= b)
                .map(|p| p.2)
                .fold(0.0, f32::max);
            let top = building.height as f32;
            (s[1] - s[0] > 0.001 && top > bottom).then_some((s[0], s[1] - s[0], bottom, top))
        })
        .collect()
}

pub(crate) fn mesh(building: &Building) -> Mesh {
    let mut mesh = BuildingMesh::default();
    let points: Vec<_> = building
        .footprint
        .iter()
        .map(|point| to_world(*point, 0.0))
        .collect();
    let winding = points
        .iter()
        .zip(points.iter().cycle().skip(1))
        .map(|(a, b)| a.x * b.z - a.z * b.x)
        .sum::<f32>()
        .signum();
    let hash = building.id.bytes().fold(0usize, |hash, byte| {
        hash.wrapping_mul(31).wrapping_add(byte.into())
    });
    let wall = linear(building.wall_color.unwrap_or(PLASTER[hash % PLASTER.len()]));
    let roof = linear(building.roof_color.unwrap_or(match building.roof {
        Roof::Flat => [86, 84, 77],
        _ if hash % 4 == 0 => [75, 76, 72],
        _ => [133, 79, 61],
    }));
    let eave = building.height as f32;
    let corners: Vec<_> = points.iter().map(|point| point.with_y(eave)).collect();
    let inside = inside_corners(&points, winding);
    let (door_edge, _, _) = doorway(building);

    for (index, &a) in points.iter().enumerate() {
        let b = points[(index + 1) % points.len()];
        let along = (b - a).normalize_or_zero();
        let outward = along.cross(Vec3::Y) * -winding;
        if building.open_sided {
            let size = 0.13;
            let offset = (along + outward) * size * 0.5;
            let base = a + offset;
            for side in [along, outward, -along, -outward] {
                let width = Vec3::Y.cross(side) * size;
                let start = base + side * size * 0.5 - width * 0.5;
                mesh.wall_panel(start, width, eave, side, wall);
            }
            mesh.wall_panel(a + Vec3::Y * (eave - 0.16), b - a, 0.16, outward, roof);
        } else {
            let width = a.distance(b);
            let door = !building.accessory && index == door_edge && width > DOOR_WIDTH + 0.2;
            let interior = linear([214, 210, 196]);
            let sections = wall_sections(building, index);
            let back = |distance: f32| {
                if distance < 0.001 {
                    inside[index]
                } else if width - distance < 0.001 {
                    inside[(index + 1) % points.len()]
                } else {
                    a + along * distance - outward * WALL_THICKNESS
                }
            };
            for (section, &(start, span, bottom, top)) in sections.iter().enumerate() {
                let base = a + along * start + Vec3::Y * bottom;
                mesh.wall_panel(base, along * span, top - bottom, outward, wall);
                let inner_start = back(start).with_y(bottom);
                let inner_end = back(start + span).with_y(bottom);
                mesh.wall_panel(
                    inner_start,
                    inner_end - inner_start,
                    top - bottom,
                    -outward,
                    interior,
                );
                if bottom > 0. {
                    mesh.face(
                        [base, base + along * span, inner_end, inner_start],
                        Vec3::NEG_Y,
                        interior,
                    );
                }
                // Close only the exposed height differences at opening edges;
                // shared masonry cuts must not add overlapping coplanar faces.
                for (distance, neighbour, normal) in [
                    (
                        start,
                        section.checked_sub(1).and_then(|i| sections.get(i)),
                        -along,
                    ),
                    (start + span, sections.get(section + 1), along),
                ] {
                    let exposed_top = neighbour.map_or(bottom, |s| s.2).min(top);
                    if exposed_top > bottom {
                        let front = (a + along * distance).with_y(bottom);
                        let rear = back(distance).with_y(bottom);
                        let up = Vec3::Y * (exposed_top - bottom);
                        mesh.face([front, rear, rear + up, front + up], normal, interior);
                    }
                }
            }
            if !building.accessory {
                mesh.facade(a, b, eave, outward, door, &openings(building, index));
            }
        }
    }
    if !building.open_sided {
        for &[a, b, c] in &building.triangles {
            mesh.face(
                [
                    points[a as usize].with_y(0.004),
                    points[b as usize].with_y(0.004),
                    points[c as usize].with_y(0.004),
                ],
                Vec3::Y,
                linear([159, 145, 119]),
            );
            mesh.face(
                [
                    corners[a as usize],
                    corners[b as usize],
                    corners[c as usize],
                ],
                Vec3::NEG_Y,
                linear([218, 215, 204]),
            );
        }
    }

    let pitched = !building.open_sided
        && building.roof != Roof::Flat
        && mesh.pitched_roof(&corners, building.roof, building.roof_height, wall, roof);
    if !pitched {
        for &[a, b, c] in &building.triangles {
            mesh.face(
                [
                    corners[a as usize],
                    corners[b as usize],
                    corners[c as usize],
                ],
                Vec3::Y,
                roof,
            );
            if building.open_sided {
                mesh.face(
                    [
                        corners[a as usize],
                        corners[b as usize],
                        corners[c as usize],
                    ],
                    Vec3::NEG_Y,
                    roof,
                );
            }
        }
    }
    mesh.finish()
}

fn linear([r, g, b]: [u8; 3]) -> [f32; 4] {
    let color = Color::srgb_u8(r, g, b).to_linear();
    [color.red, color.green, color.blue, 1.0]
}

#[derive(Default)]
struct BuildingMesh {
    positions: Vec<Vec3>,
    normals: Vec<Vec3>,
    colors: Vec<[f32; 4]>,
    uv: Vec<Vec2>,
    indices: Vec<u32>,
}

impl BuildingMesh {
    fn finish(self) -> Mesh {
        Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::default(),
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, self.positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, self.normals)
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, self.colors)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, self.uv)
        .with_inserted_indices(Indices::U32(self.indices))
    }

    fn face<const N: usize>(&mut self, mut points: [Vec3; N], outward: Vec3, color: [f32; 4]) {
        let mut normal = (points[1] - points[0]).cross(points[2] - points[0]);
        if normal.dot(outward) < 0.0 {
            points.reverse();
            normal = -normal;
        }
        if normal.length_squared() < 1e-10 {
            return;
        }
        let start = self.positions.len() as u32;
        let normal = normal.normalize();
        let u = (points[1] - points[0]).normalize();
        let v = normal.cross(u);
        self.uv
            .extend(points.iter().map(|p| Vec2::new(p.dot(u), p.dot(v)) / 1.5));
        self.positions.extend(points);
        self.normals.extend([normal; N]);
        self.colors.extend([color; N]);
        for i in 1..(N as u32 - 1) {
            self.indices.extend([start, start + i, start + i + 1]);
        }
    }

    fn wall_panel(
        &mut self,
        start: Vec3,
        width: Vec3,
        height: f32,
        outward: Vec3,
        color: [f32; 4],
    ) {
        let up = Vec3::Y * height;
        self.face(
            [start, start + width, start + width + up, start + up],
            outward,
            color,
        );
    }

    fn facade(
        &mut self,
        a: Vec3,
        b: Vec3,
        height: f32,
        outward: Vec3,
        _door: bool,
        openings: &[(f32, f32, f32)],
    ) {
        let edge = b - a;
        let width = edge.length();
        if width < 2.7 || height < 2.7 {
            return;
        }
        let along = edge / width;
        let trim = linear([212, 207, 192]);
        let glass = linear([53, 72, 78]);
        let plinth = linear([117, 112, 103]);
        let mut boundaries = vec![0.0, width];
        for &(start, end, _) in openings {
            boundaries.extend([start, end]);
        }
        boundaries.sort_by(f32::total_cmp);
        for span in boundaries.windows(2) {
            let mid = (span[0] + span[1]) * 0.5;
            if !openings.iter().any(|&(s, e, _)| mid > s && mid < e) {
                self.wall_panel(
                    a + along * span[0] + outward * 0.018,
                    along * (span[1] - span[0]),
                    0.35,
                    outward,
                    plinth,
                );
            }
        }
        let bays = (width / 3.2).floor().max(1.0) as usize;
        let floors = (height / 2.8).round().max(1.0) as usize;
        let floor_height = height / floors as f32;
        let window_width = (width / bays as f32 * 0.42).min(1.42);
        for floor in 0..floors {
            for bay in 0..bays {
                let x = (bay as f32 + 0.5) * width / bays as f32 - window_width * 0.5;
                let y = (floor as f32 + 0.30) * floor_height;
                if openings.iter().any(|&(start, end, top)| {
                    y < top + 0.12 && x < end + 0.12 && x + window_width > start - 0.12
                }) {
                    continue;
                }
                let corner = a + along * x + Vec3::Y * y;
                let window_height = (floor_height * 0.46).min(1.5);
                self.wall_panel(
                    corner + outward * 0.025,
                    along * window_width,
                    window_height,
                    outward,
                    trim,
                );
                self.wall_panel(
                    corner + outward * 0.044 + along * 0.07 + Vec3::Y * 0.07,
                    along * (window_width - 0.14),
                    window_height - 0.14,
                    outward,
                    glass,
                );
                // Frames and projecting sills give the facade depth at street level.
                self.wall_panel(
                    corner + outward * 0.053 + along * (window_width * 0.5 - 0.025),
                    along * 0.05,
                    window_height,
                    outward,
                    trim,
                );
                self.wall_panel(
                    corner + outward * 0.054 + Vec3::Y * (window_height * 0.58),
                    along * window_width,
                    0.045,
                    outward,
                    trim,
                );
                let sill = corner - along * 0.06 + outward * 0.11 - Vec3::Y * 0.035;
                self.wall_panel(sill, along * (window_width + 0.12), 0.065, outward, trim);
                self.face(
                    [
                        sill + Vec3::Y * 0.065,
                        sill + along * (window_width + 0.12) + Vec3::Y * 0.065,
                        sill + along * (window_width + 0.12) + Vec3::Y * 0.065 - outward * 0.10,
                        sill + Vec3::Y * 0.065 - outward * 0.10,
                    ],
                    Vec3::Y,
                    trim,
                );
            }
        }
    }

    fn pitched_roof(
        &mut self,
        ring: &[Vec3],
        shape: Roof,
        rise: Option<f32>,
        wall: [f32; 4],
        roof: [f32; 4],
    ) -> bool {
        let mut outline = ring.to_vec();
        let mut index = 0;
        while outline.len() > 4 && index < outline.len() {
            let a = outline[(index + outline.len() - 1) % outline.len()];
            let b = outline[(index + 1) % outline.len()];
            let edge = b - a;
            let along = (outline[index] - a).dot(edge) / edge.length_squared();
            if (0.0..=1.0).contains(&along) && (outline[index] - a - edge * along).length() < 0.30 {
                outline.remove(index);
                index = index.saturating_sub(1);
            } else {
                index += 1;
            }
        }
        let Ok(mut corners) = <[Vec3; 4]>::try_from(outline.as_slice()) else {
            return false;
        };
        if (corners[1] - corners[0]).length_squared() < (corners[2] - corners[1]).length_squared() {
            corners.rotate_left(1);
        }
        let [a, b, c, d] = corners;
        if (b - a)
            .normalize_or_zero()
            .dot((c - b).normalize_or_zero())
            .abs()
            > 0.24
            || (b - a).normalize_or_zero().dot((c - d).normalize_or_zero()) < 0.90
        {
            return false;
        }
        let left = (a + d) * 0.5;
        let right = (b + c) * 0.5;
        let rise = Vec3::Y * rise.unwrap_or_else(|| (a.distance(d) * 0.26).clamp(0.7, 3.6));
        let inset = if shape == Roof::Hipped { 0.25 } else { 0.0 };
        let p = left.lerp(right, inset) + rise;
        let q = right.lerp(left, inset) + rise;
        self.face([a, b, q, p], Vec3::Y, roof);
        self.face([d, c, q, p], Vec3::Y, roof);
        if shape == Roof::Hipped {
            self.face([a, p, d], Vec3::Y, roof);
            self.face([b, c, q], Vec3::Y, roof);
        } else {
            self.face([a, d, p], left - right, wall);
            self.face([b, c, q], right - left, wall);
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::mesh::VertexAttributeValues;
    use paket_geo::{EastNorth, Passage};

    fn building(outline: &[(f64, f64)]) -> Building {
        let footprint = outline
            .iter()
            .map(|&(east, north)| EastNorth { east, north })
            .collect::<Vec<_>>();
        Building {
            id: "wall-regression".into(),
            address: None,
            triangles: paket_geo::triangulate(&footprint).unwrap(),
            entrance: footprint[0],
            footprint,
            height: 9.,
            passages: vec![],
            accessory: true,
            open_sided: false,
            roof: Roof::Flat,
            roof_height: None,
            wall_color: None,
            roof_color: None,
        }
    }

    // Match the renderer's back-face culling: a wall's outside cannot hide a hole
    // in its inside surface. Return the first visible triangle along the ray.
    fn visible_hit(mesh: &Mesh, origin: Vec3, direction: Vec3) -> Option<f32> {
        let VertexAttributeValues::Float32x3(points) =
            mesh.attribute(Mesh::ATTRIBUTE_POSITION).unwrap()
        else {
            panic!("positions")
        };
        let indices = mesh.indices().unwrap().iter().collect::<Vec<_>>();
        indices
            .chunks_exact(3)
            .filter_map(|t| {
                let [a, b, c] = [t[0], t[1], t[2]].map(|i| Vec3::from_array(points[i]));
                let e1 = b - a;
                let e2 = c - a;
                let p = direction.cross(e2);
                let det = e1.dot(p);
                if det < 1e-7 {
                    return None;
                }
                let s = origin - a;
                let u = s.dot(p) / det;
                let q = s.cross(e1);
                let v = direction.dot(q) / det;
                let distance = e2.dot(q) / det;
                (u >= -1e-5 && v >= -1e-5 && u + v <= 1. + 1e-5 && distance > 1e-4)
                    .then_some(distance)
            })
            .min_by(f32::total_cmp)
    }

    #[test]
    fn concave_inside_corners_do_not_show_the_sky_in_either_winding() {
        let mut outline = vec![
            (0., 0.),
            (10., 0.),
            (10., 4.),
            (4., 4.),
            (4., 10.),
            (0., 10.),
        ];
        for _ in 0..2 {
            let mesh = mesh(&building(&outline));
            for dx in [-0.02, 0., 0.02] {
                let distance = visible_hit(
                    &mesh,
                    Vec3::new(3., 1.6, -3.),
                    Vec3::new(1. + dx, 0., -1.).normalize(),
                );
                assert!(
                    distance.is_some_and(|d| d < 1.5),
                    "open inside corner: {distance:?}"
                );
            }
            outline.reverse();
        }
    }

    #[test]
    fn passage_has_visible_jambs_and_soffit_but_stays_open() {
        let mut building = building(&[(0., 0.), (10., 0.), (10., 10.), (0., 10.)]);
        building.passages.push(Passage {
            edge: 0,
            start: 4.,
            end: 6.,
            height: 3.,
        });
        let mesh = mesh(&building);
        let inside_opening = Vec3::new(5., 1., -0.065);
        for direction in [Vec3::X, Vec3::NEG_X] {
            let hit = visible_hit(&mesh, inside_opening, direction);
            assert!(
                hit.is_some_and(|d| (d - 1.).abs() < 0.01),
                "missing jamb: {hit:?}"
            );
        }
        let hit = visible_hit(&mesh, inside_opening, Vec3::Y);
        assert!(
            hit.is_some_and(|d| (d - 2.).abs() < 0.01),
            "missing soffit: {hit:?}"
        );
        assert_eq!(
            visible_hit(&mesh, Vec3::new(5., 1., -1.), Vec3::Z),
            None,
            "arch was sealed"
        );
    }

    #[test]
    fn real_courtyard_inside_corners_are_opaque() {
        let scene = paket_geo::parse_overpass(
            include_bytes!("../../../fixtures/koertestrasse.json"),
            paket_geo::LatLon {
                lat: 52.4895,
                lon: 13.4084,
            },
        )
        .unwrap();
        let mut checked = 0;
        for building in scene.buildings.iter().filter(|b| !b.open_sided) {
            let points = building
                .footprint
                .iter()
                .map(|&p| to_world(p, 0.))
                .collect::<Vec<_>>();
            let winding = points
                .iter()
                .zip(points.iter().cycle().skip(1))
                .map(|(a, b)| a.x * b.z - a.z * b.x)
                .sum::<f32>()
                .signum();
            let mesh = mesh(building);
            for (i, &corner) in points.iter().enumerate() {
                let incoming =
                    (corner - points[(i + points.len() - 1) % points.len()]).normalize_or_zero();
                let outgoing = (points[(i + 1) % points.len()] - corner).normalize_or_zero();
                if incoming.cross(outgoing).y * winding < 0.2 {
                    continue;
                }
                let inside = (incoming - outgoing).normalize();
                let eye = corner.with_y(building.height as f32 * 0.8) + inside * 0.6;
                let hit = visible_hit(&mesh, eye, -inside);
                assert!(
                    hit.is_some_and(|d| d < 0.65),
                    "{} corner {i} leaks: {hit:?}",
                    building.id
                );
                checked += 1;
            }
        }
        assert!(checked > 10, "fixture must exercise real courtyards");
    }
}
