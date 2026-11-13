//! Small, seamless material maps with complete mip chains. All detail is in world units.
use bevy::{
    asset::RenderAssetUsages,
    image::{ImageAddressMode, ImageSampler, ImageSamplerDescriptor},
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};

#[derive(Clone, Copy)]
pub(crate) enum Surface {
    Water,
    Asphalt,
    Paving,
    Gravel,
    Grass,
    Plaster,
    Fabric,
    Fur,
}

pub(crate) fn matte(color: Color, roughness: f32) -> StandardMaterial {
    StandardMaterial {
        base_color: color,
        perceptual_roughness: roughness,
        reflectance: 0.28,
        ..default()
    }
}

pub(crate) fn material(images: &mut Assets<Image>, kind: Surface, tint: Color) -> StandardMaterial {
    const SIZE: usize = 256;
    let mut colors = Vec::with_capacity(SIZE * SIZE * 4);
    let mut heights = Vec::with_capacity(SIZE * SIZE);
    for y in 0..SIZE {
        for x in 0..SIZE {
            let grain = noise(x as u32, y as u32);
            let broad = ((x as f32 * std::f32::consts::TAU / SIZE as f32).sin()
                * (y as f32 * std::f32::consts::TAU / SIZE as f32).cos())
                * 0.5;
            let (base, variation, height) = match kind {
                Surface::Water => {
                    let u = x as f32 * std::f32::consts::TAU / SIZE as f32;
                    let v = y as f32 * std::f32::consts::TAU / SIZE as f32;
                    (
                        [245., 248., 250.],
                        0.0,
                        (u * 8. + (v * 3.).sin()).sin() * 0.3
                            + (v * 12. + (u * 2.).sin()).sin() * 0.10,
                    )
                }
                Surface::Asphalt => ([77., 80., 79.], grain * 10. + broad * 4., grain * 0.20),
                Surface::Paving => {
                    let row = y / 32;
                    let u = (x + (row % 2) * 32) % 64;
                    let v = y % 32;
                    let edge = u.min(63 - u).min(v.min(31 - v)) as f32;
                    let bevel = (edge / 2.0).min(1.0);
                    let stone = noise(((x + (row % 2) * 32) / 64) as u32, row as u32);
                    (
                        [149., 143., 128.],
                        grain * 4. + stone * 12. - (1. - bevel) * 32.,
                        bevel * 0.8,
                    )
                }
                Surface::Gravel => ([122., 115., 99.], grain * 18. + broad * 6., grain * 0.35),
                Surface::Grass => ([86., 104., 61.], grain * 8. + broad * 7., grain * 0.13),
                Surface::Plaster => ([244., 243., 240.], grain * 5., grain * 0.12),
                Surface::Fabric => {
                    let weave = if (x / 2 + y / 2) % 2 == 0 { 1.0 } else { -1.0 };
                    ([239., 239., 239.], weave * 5. + grain * 2., weave * 0.14)
                }
                Surface::Fur => ([244., 243., 240.], grain * 4., grain * 0.08),
            };
            for channel in base {
                colors.push((channel + variation).clamp(0., 255.) as u8);
            }
            colors.push(255);
            heights.push(height);
        }
    }
    let mut normals = Vec::with_capacity(colors.len());
    for y in 0..SIZE {
        for x in 0..SIZE {
            let at = |dx: usize, dy: usize| heights[((y + dy) % SIZE) * SIZE + (x + dx) % SIZE];
            let n =
                Vec3::new(at(SIZE - 1, 0) - at(1, 0), at(0, SIZE - 1) - at(0, 1), 1.0).normalize();
            normals.extend([
                ((n.x * 0.5 + 0.5) * 255.) as u8,
                ((n.y * 0.5 + 0.5) * 255.) as u8,
                ((n.z * 0.5 + 0.5) * 255.) as u8,
                255,
            ]);
        }
    }
    StandardMaterial {
        base_color_texture: Some(images.add(mipmapped(SIZE, colors, true))),
        normal_map_texture: Some(images.add(mipmapped(SIZE, normals, false))),
        ..matte(tint, 0.91)
    }
}

fn noise(x: u32, y: u32) -> f32 {
    let n = x.wrapping_mul(374761393) ^ y.wrapping_mul(668265263);
    let n = (n ^ (n >> 13)).wrapping_mul(1274126177);
    (n & 65535) as f32 / 32767.5 - 1.0
}

fn mipmapped(side: usize, mut pixels: Vec<u8>, srgb: bool) -> Image {
    let mut image = Image::new_uninit(
        Extent3d {
            width: side as u32,
            height: side as u32,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        if srgb {
            TextureFormat::Rgba8UnormSrgb
        } else {
            TextureFormat::Rgba8Unorm
        },
        RenderAssetUsages::default(),
    );
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::Repeat,
        address_mode_v: ImageAddressMode::Repeat,
        anisotropy_clamp: 16,
        ..ImageSamplerDescriptor::linear()
    });
    let mut data = pixels.clone();
    let mut size = side;
    while size > 1 {
        let next = size / 2;
        let mut small = vec![0; next * next * 4];
        for y in 0..next {
            for x in 0..next {
                for c in 0..4 {
                    // Colour is averaged in linear light; normal maps stay linear.
                    let mut sum = 0.0;
                    for dy in 0..2 {
                        for dx in 0..2 {
                            let value =
                                pixels[((y * 2 + dy) * size + x * 2 + dx) * 4 + c] as f32 / 255.;
                            sum += if srgb && c < 3 {
                                value.powf(2.2)
                            } else {
                                value
                            };
                        }
                    }
                    let mean = sum * 0.25;
                    small[(y * next + x) * 4 + c] = ((if srgb && c < 3 {
                        mean.powf(1. / 2.2)
                    } else {
                        mean
                    }) * 255.)
                        .round() as u8;
                }
            }
        }
        data.extend(&small);
        pixels = small;
        size = next;
        image.texture_descriptor.mip_level_count += 1;
    }
    image.data = Some(data);
    image
}
