use bevy::prelude::*;

const ICON: &[u8] = include_bytes!("../../../assets/icon.png");

#[cfg(target_os = "macos")]
pub(crate) fn install(app: &mut App) {
    app.add_systems(Startup, set_dock_icon);
}

#[cfg(target_os = "macos")]
fn set_dock_icon(_: bevy::ecs::system::NonSendMarker) {
    use objc2::{AnyThread, MainThreadMarker};
    use objc2_app_kit::{NSApplication, NSImage};
    use objc2_foundation::NSData;

    let Some(main_thread) = MainThreadMarker::new() else {
        return;
    };
    let data = NSData::with_bytes(ICON);
    let Some(icon) = NSImage::initWithData(NSImage::alloc(), &data) else {
        warn!("Could not decode the application icon");
        return;
    };
    let application = NSApplication::sharedApplication(main_thread);
    // AppKit requires a non-null image and access on the main thread.
    unsafe { application.setApplicationIconImage(Some(&icon)) };
}

#[cfg(any(target_os = "windows", target_os = "linux"))]
pub(crate) fn install(app: &mut App) {
    use bevy::window::{PrimaryWindow, WindowCreated};
    use winit::window::Icon;

    let Ok(decoded) = image::load_from_memory_with_format(ICON, image::ImageFormat::Png) else {
        warn!("Could not decode the application icon");
        return;
    };
    let pixels = decoded.resize_exact(64, 64, image::imageops::FilterType::Lanczos3);
    let Ok(icon) = Icon::from_rgba(pixels.into_rgba8().into_raw(), 64, 64) else {
        warn!("Could not create the window icon");
        return;
    };
    app.add_systems(
        Update,
        move |_: bevy::ecs::system::NonSendMarker,
              mut created: MessageReader<WindowCreated>,
              primary: Query<(), With<PrimaryWindow>>| {
            for event in created
                .read()
                .filter(|event| primary.contains(event.window))
            {
                bevy::winit::WINIT_WINDOWS.with_borrow(|windows| {
                    if let Some(window) = windows.get_window(event.window) {
                        window.set_window_icon(Some(icon.clone()));
                    }
                });
            }
        },
    );
}
