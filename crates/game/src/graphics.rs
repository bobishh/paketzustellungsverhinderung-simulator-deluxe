//! Render the adaptive street separately from the full-resolution game interface.
use crate::{Mode, actors::FollowCamera};
use bevy::{
    camera::{CameraUpdateSystems, ImageRenderTarget, RenderTarget},
    image::ImageSampler,
    picking::Pickable,
    prelude::*,
    render::render_resource::{Extent3d, TextureFormat},
    window::PrimaryWindow,
};

#[derive(Resource)]
struct SceneSurface {
    image: Handle<Image>,
    physical_size: UVec2,
}

#[derive(Component)]
struct SceneImage;

/// One window camera owns UI pixels; only the gameplay view changes resolution.
pub(crate) fn install(app: &mut App) {
    app.add_systems(Startup, setup)
        .add_systems(PostUpdate, resize.before(CameraUpdateSystems));
}

fn setup(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    let mut image = Image::new_target_texture(1, 1, TextureFormat::Rgba8UnormSrgb, None);
    image.sampler = ImageSampler::linear();
    let image = images.add(image);
    commands.insert_resource(SceneSurface {
        image: image.clone(),
        physical_size: UVec2::ONE,
    });
    commands.spawn((
        Camera2d,
        Camera {
            order: 100,
            #[cfg(target_arch = "wasm32")]
            clear_color: ClearColorConfig::Custom(Color::NONE),
            #[cfg(not(target_arch = "wasm32"))]
            clear_color: ClearColorConfig::Default,
            ..default()
        },
        Msaa::Off,
        IsDefaultUiCamera,
        Name::new("Full-resolution interface"),
    ));
    // Composite through the same UI pass as the HUD, beneath all game widgets.
    // Clearing the window each frame also prevents old loading artwork persisting.
    commands.spawn((
        ImageNode::new(image),
        Node {
            position_type: PositionType::Absolute,
            width: percent(100),
            height: percent(100),
            display: Display::None,
            ..default()
        },
        GlobalZIndex(-1000),
        Pickable::IGNORE,
        SceneImage,
        Name::new("Adaptive street composite"),
    ));
}

fn resize(
    windows: Query<&Window, With<PrimaryWindow>>,
    mode: Res<State<Mode>>,
    mut surface: ResMut<SceneSurface>,
    mut images: ResMut<Assets<Image>>,
    mut cameras: Query<(&mut Camera, &mut RenderTarget), (With<Camera3d>, With<FollowCamera>)>,
    mut composite: Query<&mut Node, With<SceneImage>>,
) {
    let Ok(window) = windows.single() else {
        return;
    };
    let active = *mode.get() == Mode::Ride && cameras.iter().any(|(camera, _)| camera.is_active);
    for mut node in &mut composite {
        let desired = if active { Display::Flex } else { Display::None };
        if node.display != desired {
            node.display = desired;
        }
    }
    if !active {
        return;
    }
    let scale = scene_scale();
    let size = (window.physical_size().as_vec2() * scale)
        .round()
        .max(Vec2::ONE)
        .as_uvec2();
    if surface.physical_size != size {
        if let Some(mut image) = images.get_mut(&surface.image) {
            image.resize(Extent3d {
                width: size.x,
                height: size.y,
                depth_or_array_layers: 1,
            });
            surface.physical_size = size;
        }
    }
    // Camera projections, pointer rays and world labels keep window logical pixels.
    // The physical render texture alone shrinks with quality; UI never does.
    let scale_factor = window.scale_factor() * scale;
    for (mut camera, mut target) in &mut cameras {
        if camera.order != -1 {
            camera.order = -1;
        }
        let unchanged = matches!(&*target, RenderTarget::Image(image)
            if image.handle == surface.image && image.scale_factor == scale_factor);
        if !unchanged {
            *target = RenderTarget::Image(ImageRenderTarget {
                handle: surface.image.clone(),
                scale_factor,
            });
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn scene_scale() -> f32 {
    1.0
}

#[cfg(target_arch = "wasm32")]
fn scene_scale() -> f32 {
    let scale = requested_scene_scale();
    if scale.is_finite() {
        scale.clamp(0.2, 1.0)
    } else {
        1.0
    }
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen(
    inline_js = "export function sceneScale(){return globalThis.paketSceneScale ?? 1;}"
)]
extern "C" {
    #[wasm_bindgen::prelude::wasm_bindgen(js_name = sceneScale)]
    fn requested_scene_scale() -> f32;
}
