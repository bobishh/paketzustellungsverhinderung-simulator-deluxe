//! World-scale painted walls. A ride keeps one kit as its tiles stream in.
use bevy::{math::Affine2, prelude::*};
use paket_geo::{
    Building, LatLon,
    architecture::{Region, WallFinish, region_at},
};

use crate::surfaces;

use WallFinish::*;

#[derive(Clone, Copy)]
struct Finish {
    material: WallFinish,
    tint: [u8; 3],
}

const fn finish(material: WallFinish, tint: [u8; 3]) -> Finish {
    Finish { material, tint }
}

#[derive(Clone, Copy)]
pub(crate) struct Windows {
    pub spacing: f32,
    pub width: f32,
    pub height: f32,
    pub trim: [u8; 3],
}

impl Windows {
    pub const DEFAULT: Self = Self {
        spacing: 3.2,
        width: 1.42,
        height: 1.50,
        trim: [212, 207, 192],
    };

    const fn new(spacing: f32, width: f32, height: f32, trim: [u8; 3]) -> Self {
        Self {
            spacing,
            width,
            height,
            trim,
        }
    }
}

struct Kit {
    walls: [Finish; 3],
    windows: Windows,
}

fn neighbourhood(region: Option<Region>) -> Kit {
    use Region::*;
    let (walls, windows) = match region {
        Some(BritishIsles) => (
            [
                finish(Brick, [182, 119, 99]),
                finish(StockBrick, [205, 183, 144]),
                finish(Stucco, [230, 224, 201]),
            ],
            Windows::new(2.9, 1.20, 1.70, [231, 220, 196]),
        ),
        Some(Nordic) => (
            [
                finish(Timber, [179, 93, 74]),
                finish(Lime, [232, 214, 171]),
                finish(Panels, [191, 196, 190]),
            ],
            Windows::new(3.2, 1.42, 1.42, [223, 222, 207]),
        ),
        Some(CentralEurope) => (
            [
                finish(Lime, [219, 205, 177]),
                finish(Brick, [183, 118, 98]),
                finish(Panels, [194, 194, 182]),
            ],
            Windows::new(3.1, 1.34, 1.61, [226, 213, 184]),
        ),
        Some(AtlanticEurope) => (
            [
                finish(Ashlar, [230, 214, 181]),
                finish(Lime, [220, 209, 186]),
                finish(StockBrick, [196, 158, 132]),
            ],
            Windows::new(2.8, 1.18, 1.80, [230, 216, 184]),
        ),
        Some(Mediterranean) => (
            [
                finish(Stucco, [237, 212, 174]),
                finish(Ashlar, [220, 192, 158]),
                finish(Ceramic, [180, 212, 203]),
            ],
            Windows::new(3.0, 1.16, 1.60, [226, 211, 177]),
        ),
        Some(EasternEurope) => (
            [
                finish(Panels, [191, 193, 187]),
                finish(Lime, [226, 204, 168]),
                finish(Brick, [186, 127, 101]),
            ],
            Windows::new(3.0, 1.48, 1.45, [215, 213, 200]),
        ),
        Some(WestAsiaNorthAfrica) => (
            [
                finish(SandRender, [226, 181, 128]),
                finish(Stucco, [237, 218, 176]),
                finish(Ashlar, [210, 179, 145]),
            ],
            Windows::new(3.3, 1.10, 1.36, [226, 201, 159]),
        ),
        Some(WestAfrica) => (
            [
                finish(Adobe, [208, 147, 102]),
                finish(SandRender, [230, 175, 124]),
                finish(Concrete, [203, 196, 176]),
            ],
            Windows::new(3.0, 1.38, 1.24, [229, 204, 158]),
        ),
        Some(EastAfrica) => (
            [
                finish(Rubble, [220, 201, 162]),
                finish(Lime, [234, 217, 179]),
                finish(Concrete, [190, 190, 182]),
            ],
            Windows::new(3.3, 1.27, 1.48, [187, 150, 115]),
        ),
        Some(SouthernAfrica) => (
            [
                finish(StockBrick, [204, 173, 139]),
                finish(Stucco, [229, 222, 196]),
                finish(Rubble, [186, 173, 155]),
            ],
            Windows::new(3.5, 1.65, 1.35, [232, 223, 198]),
        ),
        Some(NorthAmerica) => (
            [
                finish(Brick, [181, 105, 87]),
                finish(Timber, [203, 216, 206]),
                finish(Ashlar, [219, 210, 184]),
            ],
            Windows::new(2.8, 1.26, 1.65, [215, 209, 191]),
        ),
        Some(LatinHighlands) => (
            [
                finish(Stucco, [228, 188, 134]),
                finish(Adobe, [192, 133, 102]),
                finish(Basalt, [157, 147, 139]),
            ],
            Windows::new(3.1, 1.19, 1.56, [223, 200, 162]),
        ),
        Some(TropicalAmerica) => (
            [
                finish(Stucco, [218, 180, 145]),
                finish(Mosaic, [172, 193, 181]),
                finish(Concrete, [203, 193, 176]),
            ],
            Windows::new(3.6, 1.77, 1.37, [231, 223, 190]),
        ),
        Some(SouthAsia) => (
            [
                finish(Lime, [227, 195, 160]),
                finish(Ashlar, [215, 163, 129]),
                finish(Concrete, [196, 190, 180]),
            ],
            Windows::new(3.4, 1.42, 1.42, [234, 211, 177]),
        ),
        Some(EastAsia) => (
            [
                finish(StockBrick, [161, 164, 156]),
                finish(Ceramic, [217, 208, 188]),
                finish(Panels, [190, 196, 191]),
            ],
            Windows::new(3.1, 1.59, 1.36, [208, 197, 178]),
        ),
        Some(JapanKorea) => (
            [
                finish(Ceramic, [218, 208, 195]),
                finish(Concrete, [187, 190, 182]),
                finish(Metal, [163, 174, 170]),
            ],
            Windows::new(3.0, 1.54, 1.28, [195, 194, 180]),
        ),
        Some(SoutheastAsia) => (
            [
                finish(Stucco, [232, 205, 153]),
                finish(Timber, [168, 132, 107]),
                finish(Ceramic, [170, 203, 183]),
            ],
            Windows::new(3.1, 1.46, 1.43, [210, 185, 143]),
        ),
        Some(Oceania) => (
            [
                finish(Timber, [211, 217, 197]),
                finish(Brick, [189, 124, 104]),
                finish(Metal, [172, 184, 176]),
            ],
            Windows::new(3.5, 1.69, 1.40, [227, 219, 193]),
        ),
        None => (
            [
                finish(Lime, [208, 197, 175]),
                finish(Stucco, [224, 212, 188]),
                finish(Panels, [195, 194, 179]),
            ],
            Windows::DEFAULT,
        ),
    };
    Kit { walls, windows }
}

macro_rules! swatches {
    ($( $finish:ident $name:literal $width:literal $height:literal $rgb:expr; )+) => {
        const SWATCHES: &[(WallFinish, Swatch)] = &[
            $(($finish, Swatch {
                pixels: include_bytes!(concat!("../../../assets/facades/", $name, ".png")),
                metres: Vec2::new($width, $height),
                tint: $rgb,
            }),)+
        ];
    }
}

struct Swatch {
    pixels: &'static [u8],
    metres: Vec2,
    tint: [u8; 3],
}

swatches! {
    Lime "lime" 3.2 3.2 [218, 208, 183];
    Stucco "stucco" 3.2 3.2 [229, 218, 197];
    Concrete "concrete" 3.2 2.4 [192, 192, 185];
    Panels "panels" 2.5 2.5 [199, 196, 185];
    Brick "brick" 1.05 1.0 [190, 125, 105];
    StockBrick "stock-brick" 1.0 0.83 [209, 181, 146];
    Ashlar "ashlar" 1.36 1.42 [223, 210, 180];
    Rubble "rubble" 2.1 1.65 [195, 185, 164];
    Ceramic "ceramic" 1.26 1.54 [216, 210, 196];
    Mosaic "mosaic" 1.24 1.24 [177, 202, 204];
    Timber "timber" 2.4 1.23 [191, 156, 124];
    Metal "metal" 2.4 2.8 [179, 185, 184];
    SandRender "sand-render" 3.2 3.2 [225, 182, 127];
    Adobe "adobe" 2.0 1.6 [198, 147, 110];
    Granite "granite" 2.15 2.15 [169, 160, 146];
    Basalt "basalt" 2.1 1.65 [153, 148, 140];
}

#[derive(Resource)]
pub(crate) struct Library(Vec<Handle<StandardMaterial>>);

impl Library {
    pub(crate) fn new(
        images: &mut Assets<Image>,
        materials: &mut Assets<StandardMaterial>,
        unlit: bool,
    ) -> Self {
        Self(
            SWATCHES
                .iter()
                .map(|(_, swatch)| {
                    let image =
                        image::load_from_memory_with_format(swatch.pixels, image::ImageFormat::Png)
                            .expect("valid bundled facade texture")
                            .into_rgba8();
                    debug_assert_eq!(image.width(), image.height());
                    debug_assert!(image.width().is_power_of_two());
                    let texture = images.add(surfaces::mipmapped(
                        image.width() as usize,
                        image.into_raw(),
                        true,
                    ));
                    materials.add(StandardMaterial {
                        base_color_texture: Some(texture),
                        uv_transform: Affine2::from_scale(swatch.metres.recip()),
                        unlit,
                        ..surfaces::matte(Color::WHITE, 0.94)
                    })
                })
                .collect(),
        )
    }

    pub(crate) fn material(&self, finish: WallFinish) -> Handle<StandardMaterial> {
        self.0[swatch_index(finish)].clone()
    }
}

fn swatch_index(finish: WallFinish) -> usize {
    SWATCHES
        .iter()
        .position(|(value, _)| *value == finish)
        .expect("wall finish has a swatch")
}

pub(crate) struct Neighbourhood(Kit);

impl Neighbourhood {
    pub(crate) fn at(origin: LatLon) -> Self {
        Self(neighbourhood(region_at(origin)))
    }

    pub(crate) fn building(&self, building: &Building) -> (WallFinish, [u8; 3], Windows) {
        let hash = building.id.bytes().fold(2166136261u32, |hash, byte| {
            (hash ^ u32::from(byte)).wrapping_mul(16777619)
        });
        let selected = match hash % 7 {
            0..=2 => self.0.walls[0],
            3..=4 => self.0.walls[1],
            _ => self.0.walls[2],
        };
        let (material, tint) = building
            .wall_finish
            .map_or((selected.material, selected.tint), |material| {
                (material, SWATCHES[swatch_index(material)].1.tint)
            });
        let tint = building.wall_color.unwrap_or_else(|| {
            let light = ((hash >> 8) % 19) as i16 - 9;
            tint.map(|channel| (i16::from(channel) + light).clamp(0, 255) as u8)
        });
        (material, tint, self.0.windows)
    }
}
