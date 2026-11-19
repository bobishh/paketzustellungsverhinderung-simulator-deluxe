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
