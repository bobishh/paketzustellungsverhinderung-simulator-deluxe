//! Rounded hard-surface meshes: broad flat panels, small bevels that catch the light.
use bevy::{
    asset::RenderAssetUsages,
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
};

pub(crate) fn rounded_box(size: Vec3, radius: f32) -> Mesh {
    let half = size * 0.5;
    let inner = half - Vec3::splat(radius.min(half.min_element() * 0.9));
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut uv = Vec::new();
    let mut indices = Vec::new();
    for (normal, u, v) in [
        (Vec3::X, Vec3::NEG_Z, Vec3::Y),
        (Vec3::NEG_X, Vec3::Z, Vec3::Y),
        (Vec3::Y, Vec3::X, Vec3::NEG_Z),
        (Vec3::NEG_Y, Vec3::X, Vec3::Z),
        (Vec3::Z, Vec3::X, Vec3::Y),
        (Vec3::NEG_Z, Vec3::NEG_X, Vec3::Y),
    ] {
        let extent = |axis: Vec3| half.dot(axis.abs());
        let r = radius.min(half.min_element() * 0.9);
        let steps = |h: f32| [-h, -h + r * 0.3, -h + r, h - r, h - r * 0.3, h];
        let us = steps(extent(u));
        let vs = steps(extent(v));
        let start = positions.len() as u32;
        for (y, &b) in vs.iter().enumerate() {
            for (x, &a) in us.iter().enumerate() {
                let point = normal * extent(normal) + u * a + v * b;
                let nearest = point.clamp(-inner, inner);
                let n = (point - nearest).normalize();
                positions.push(nearest + n * r);
                normals.push(n);
                uv.push(Vec2::new(
                    a / size.dot(u.abs()) + 0.5,
                    b / size.dot(v.abs()) + 0.5,
                ));
                if x < 5 && y < 5 {
                    let i = start + (y * 6 + x) as u32;
                    indices.extend([i, i + 1, i + 7, i, i + 7, i + 6]);
                }
            }
        }
    }
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uv)
    .with_inserted_indices(Indices::U32(indices));
    mesh.generate_tangents()
        .expect("rounded box has nondegenerate UVs");
    mesh
}

pub(crate) fn lit(mut mesh: Mesh) -> Mesh {
    if let Err(error) = mesh.generate_tangents() {
        warn!("material tangent generation: {error:?}");
    }
    mesh
}

/// One mesh for the deliberately utilitarian delivery-company stencil.
pub(crate) fn company_mark() -> Mesh {
    let letters = [
        [30, 17, 17, 17, 17, 17, 30], // D
        [30, 17, 17, 30, 20, 18, 17], // R
        [31, 16, 16, 30, 16, 16, 31], // E
        [17, 17, 17, 31, 17, 17, 17], // H
        [16, 16, 16, 16, 16, 16, 31], // L
        [15, 16, 16, 14, 1, 1, 30],   // S
        [15, 16, 16, 14, 1, 1, 30],   // S
    ];
    let pixel = Mesh::from(Cuboid::new(0.040, 0.040, 0.006));
    let mut combined: Option<Mesh> = None;
    for (letter, rows) in letters.iter().enumerate() {
        for (y, row) in rows.iter().enumerate() {
            for x in 0..5 {
                if row & (1 << (4 - x)) == 0 {
                    continue;
                }
                let mesh = pixel.clone().transformed_by(Transform::from_xyz(
                    (letter as f32 * 6.0 + x as f32 - 20.0) * 0.039,
                    (3.0 - y as f32) * 0.039,
                    0.0,
                ));
                if let Some(combined) = &mut combined {
                    combined.merge(&mesh).unwrap();
                } else {
                    combined = Some(mesh);
                }
            }
        }
    }
    combined.unwrap()
}
