//! Split mapped water at terrain breaklines before sampling height.
use bevy::math::Vec3;

pub(super) fn conform(triangle: [Vec3; 3], step: f32, radius: f32) -> Vec<[Vec3; 3]> {
    let mut result = Vec::new();
    let mut inside = triangle.to_vec();
    // Heights are flat outside the sampled patch. Keep those pieces coarse,
    // even when an OSM coastline extends far beyond the loaded neighbourhood.
    for (normal, offset) in [
        (Vec3::X, radius),
        (Vec3::NEG_X, radius),
        (Vec3::Z, radius),
        (Vec3::NEG_Z, radius),
    ] {
        append(&mut result, &clip(&inside, -normal, -offset));
        inside = clip(&inside, normal, offset);
    }
    if inside.len() < 3 {
        return result;
    }
    let min = inside
        .iter()
        .copied()
        .fold(Vec3::splat(f32::INFINITY), Vec3::min);
    let max = inside
        .iter()
        .copied()
        .fold(Vec3::splat(f32::NEG_INFINITY), Vec3::max);
    for z in (min.z / step).floor() as i32..(max.z / step).ceil() as i32 {
        for x in (min.x / step).floor() as i32..(max.x / step).ceil() as i32 {
            let (x, z) = (x as f32 * step, z as f32 * step);
            let mut cell = inside.clone();
            for (normal, offset) in [
                (Vec3::X, -x),
                (Vec3::NEG_X, x + step),
                (Vec3::Z, -z),
                (Vec3::NEG_Z, z + step),
            ] {
                cell = clip(&cell, normal, offset);
            }
            // This is the same anti-diagonal as Heights::at and floor's heightfield.
            let diagonal = Vec3::new(1., 0., 1.);
            append(&mut result, &clip(&cell, -diagonal, x + z + step));
            append(&mut result, &clip(&cell, diagonal, -(x + z + step)));
        }
    }
    result
}

// Convex half-plane clipping preserves the original OSM triangle and its winding.
fn clip(polygon: &[Vec3], normal: Vec3, offset: f32) -> Vec<Vec3> {
    let mut result = Vec::new();
    let Some(&last) = polygon.last() else {
        return result;
    };
    let mut previous = last;
    let mut previous_distance = normal.dot(previous) + offset;
    for &point in polygon {
        let distance = normal.dot(point) + offset;
        if (distance >= 0.) != (previous_distance >= 0.) {
            result.push(previous.lerp(point, previous_distance / (previous_distance - distance)));
        }
        if distance >= 0. {
            result.push(point);
        }
        previous = point;
        previous_distance = distance;
    }
    result
}

fn append(triangles: &mut Vec<[Vec3; 3]>, polygon: &[Vec3]) {
    if polygon.len() < 3 {
        return;
    }
    for pair in polygon[1..].windows(2) {
        let triangle = [polygon[0], pair[0], pair[1]];
        if (triangle[1] - triangle[0])
            .cross(triangle[2] - triangle[0])
            .length_squared()
            > 1e-10
        {
            triangles.push(triangle);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const STEP: f32 = 20.;

    fn height(p: Vec3) -> f32 {
        let x = (p.x / STEP).floor();
        let z = (p.z / STEP).floor();
        let (u, v) = (p.x / STEP - x, p.z / STEP - z);
        let a = ((x as i32 + z as i32).rem_euclid(2) as f32) * 4.;
        let (b, c, d) = (4. - a, 4. - a, a);
        if u + v <= 1. {
            a + (b - a) * u + (c - a) * v
        } else {
            d + (c - d) * (1. - u) + (b - d) * (1. - v)
        }
    }

    #[test]
    fn water_follows_the_ground_inside_triangles_not_only_at_sampled_vertices() {
        // Given a four-metre terrain ridge inside one 20 m cell and a differently aligned OSM triangle.
        let input = [
            Vec3::new(3., 0., 2.),
            Vec3::new(19., 0., 18.),
            Vec3::new(19., 0., 3.),
        ];
        let mut minimum_gap = f32::INFINITY;
        let mut maximum_error = 0f32;
        // When that water is draped, every interior point must follow the same terrain plane.
        let triangles = conform(input, STEP, 40.);
        assert!(!triangles.is_empty());
        for triangle in triangles {
            let [a, b, c] = triangle.map(|p| p.with_y(height(p)));
            for i in 0..=10 {
                for j in 0..=10 - i {
                    let point = a + (b - a) * (i as f32 / 10.) + (c - a) * (j as f32 / 10.);
                    let gap = point.y - height(point);
                    minimum_gap = minimum_gap.min(gap);
                    maximum_error = maximum_error.max(gap.abs());
                }
            }
        }
        assert!(
            maximum_error < 0.0001,
            "water crosses terrain planes: minimum gap {minimum_gap:.4} m, maximum error {maximum_error:.4} m"
        );
    }

    #[test]
    fn water_also_follows_breaklines_across_negative_and_multiple_cells() {
        let triangles = conform(
            [
                Vec3::new(-37., 0., -33.),
                Vec3::new(42., 0., -18.),
                Vec3::new(7., 0., 45.),
            ],
            STEP,
            80.,
        );
        assert!(!triangles.is_empty());
        for triangle in triangles {
            let [a, b, c] = triangle.map(|p| p.with_y(height(p)));
            for i in 0..=6 {
                for j in 0..=6 - i {
                    let point = a + (b - a) * (i as f32 / 6.) + (c - a) * (j as f32 / 6.);
                    assert!(
                        (point.y - height(point)).abs() < 0.0001,
                        "water crossed a terrain breakline at {point:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn clipping_preserves_water_area_and_an_island_across_negative_grid_cells() {
        // Given an already triangulated water polygon with a square island.
        let ring = [
            (-25., -25.),
            (25., -25.),
            (25., 25.),
            (-25., 25.),
            (-3., -3.),
            (3., -3.),
            (3., 3.),
            (-3., 3.),
        ];
        let vertices = ring.map(|(x, z)| Vec3::new(x, 0., z));
        let faces = [
            [0, 1, 5],
            [0, 5, 4],
            [1, 2, 6],
            [1, 6, 5],
            [2, 3, 7],
            [2, 7, 6],
            [3, 0, 4],
            [3, 4, 7],
        ];
        let mut area = 0.;
        for face in faces {
            for [a, b, c] in conform(face.map(|i| vertices[i]), STEP, 40.) {
                area += (b - a).cross(c - a).length() * 0.5;
                let middle = (a + b + c) / 3.;
                assert!(
                    middle.x.abs() >= 3. - 0.0001 || middle.z.abs() >= 3. - 0.0001,
                    "an island was filled with water"
                );
            }
        }
        assert!(
            (area - (50. * 50. - 6. * 6.)).abs() < 0.01,
            "water footprint changed: {area}"
        );
    }
    #[test]
    fn a_large_coastal_triangle_only_tessellates_the_elevation_patch() {
        let triangle = [
            Vec3::new(-1e6, 0., -1e6),
            Vec3::new(1e6, 0., -1e6),
            Vec3::new(0., 0., 1e6),
        ];
        let triangles = conform(triangle, STEP, 40.);
        assert!(
            triangles.len() < 100,
            "coastline tessellation escaped the sampled terrain patch"
        );
        let area = |[a, b, c]: [Vec3; 3]| (b - a).cross(c - a).length() * 0.5;
        let total = triangles.into_iter().map(area).sum::<f32>();
        assert!(
            (total / area(triangle) - 1.).abs() < 1e-6,
            "clipping discarded water outside the patch"
        );
    }
}
