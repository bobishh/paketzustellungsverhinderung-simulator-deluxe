//! Portable game interface. Browser code supplies services, never game widgets.
#[path = "ui_click.rs"]
mod click;
mod welcome;
use crate::Mode;
use bevy::{
    prelude::*,
    text::EditableText,
    ui_widgets::{Activate, Button},
};
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};
use welcome::Welcome;

static EVENTS: Mutex<Vec<(String, Value)>> = Mutex::new(Vec::new());
static PHOTO: Mutex<Option<Image>> = Mutex::new(None);
static COMMANDS: Mutex<Vec<Value>> = Mutex::new(Vec::new());

pub(crate) fn record(name: &str, text: &str) {
    if let Ok(value) = serde_json::from_str(text) {
        let mut queue = EVENTS.lock().unwrap();
        if queue.len() >= 128 {
            queue.remove(0);
        }
        queue.push((name.into(), value));
    }
}
pub(crate) fn save_photo(image: &Image) {
    *PHOTO.lock().unwrap() = Some(image.clone());
    record("paket-photo", r#"{"ready":true}"#);
}
pub(crate) fn queue(value: Value) {
    COMMANDS.lock().unwrap().push(value);
}
pub(crate) fn drain_commands() -> Vec<Value> {
    std::mem::take(&mut *COMMANDS.lock().unwrap())
}

#[derive(Component)]
pub(crate) struct Activated;
pub(crate) fn activate(event: On<Activate>, mut commands: Commands) {
    commands.entity(event.entity).insert(Activated);
}
#[derive(Clone, Component)]
struct Action(Value);
#[derive(Component)]
struct Interface;
#[derive(Clone, Copy, PartialEq, Eq, Component)]
pub(crate) enum Layer {
    Header,
    Hud,
    Lobby,
    Dialog,
    Intercom,
    Loading,
    Welcome,
}
#[derive(Component)]
struct Header;
#[derive(Component)]
struct HeaderBrand;
#[derive(Component)]
struct HeaderAction;
#[derive(Component)]
struct HeaderOffset(f32);
#[derive(Component)]
enum HudCard {
    Route,
    Instruction,
    Radar,
}

pub(crate) fn header_height(width: f32) -> f32 {
    if width < 600. { 90. } else { 62. }
}
#[derive(Resource, Default)]
struct HitRegions(Vec<Rect>);
#[derive(Resource, Default, Clone)]
struct UiGpu(Arc<AtomicBool>, Arc<AtomicBool>);
#[derive(Component)]
struct Modal;
#[derive(Component)]
struct RideHud;
#[derive(Component)]
struct RouteList;
#[derive(Component)]
struct WorldLoading;
#[derive(Component)]
struct Spinner;
#[derive(Component)]
pub(crate) struct Bound(pub(crate) &'static str);
#[derive(Component)]
struct Radar {
    overview: bool,
}
#[derive(Component)]
struct RadarMark {
    kind: &'static str,
    id: String,
    slot: Option<usize>,
}
struct RadarLayers {
    roads: Entity,
    markers: Entity,
    stops: Entity,
    signature: String,
    stops_signature: String,
}
#[derive(Component)]
struct MapStop {
    radar: Entity,
    index: usize,
}
#[derive(Component)]
struct Dynamic(&'static str);
#[derive(Resource, Default)]
struct LastPhoto(Option<Handle<Image>>);
#[derive(Resource, Default)]
pub(crate) struct Ui {
    pub(crate) values: HashMap<String, Value>,
    panel: String,
    built_panel: String,
    mode: String,
    notice: String,
    notice_until: f32,
    sensitivity: f64,
    sound: bool,
    public: bool,
    mode_since: f32,
    welcome: bool,
}
impl Ui {
    pub(crate) fn panel_open(&self) -> bool {
        !self.panel.is_empty() || self.welcome
    }
    pub(crate) fn value(&self, key: &str) -> &Value {
        self.values.get(key).unwrap_or(&Value::Null)
    }
}
const INK: Color = Color::srgb(0.055, 0.085, 0.08);
const PANEL: Color = Color::srgb(0.10, 0.15, 0.13);
const MUTED: Color = Color::srgb(0.65, 0.73, 0.67);
const PAPER: Color = Color::srgb(0.96, 0.94, 0.86);
const ORANGE: Color = Color::srgb(1.0, 0.46, 0.16);
pub(crate) fn text(value: impl Into<String>, size: f32) -> impl Bundle {
    colored_text(value, size, PAPER)
}
fn colored_text(value: impl Into<String>, size: f32, color: Color) -> impl Bundle {
    (
        Text::new(value),
        TextFont {
            font_size: FontSize::Px(size),
            ..default()
        },
        TextColor(color),
    )
}
pub(crate) fn button(value: Value, title: impl Into<String>) -> impl Bundle {
    let title = title.into();
    button_with_label(value, title.clone(), text(title, 14.))
}
fn button_with_label(value: Value, title: String, label: impl Bundle) -> impl Bundle {
    (
        Button,
        bevy::picking::hover::Hovered::default(),
        AccessibleLabel::new(title),
        Action(value),
        Node {
            min_height: Val::Px(38.),
            border: UiRect::all(px(1)),
            padding: UiRect::axes(Val::Px(12.), Val::Px(8.)),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            ..default()
        },
        BackgroundColor(PANEL),
        BorderColor::all(Color::srgb(0.23, 0.30, 0.25)),
        children![label],
    )
}
fn clicked(event: On<Activate>, actions: Query<&Action>) {
    if let Ok(action) = actions.get(event.entity) {
        crate::platform::ui_event("paket-ui-action", &action.0.to_string());
        queue(action.0.clone());
    }
}
pub(crate) fn add_button(
    parent: &mut ChildSpawnerCommands,
    value: Value,
    title: impl Into<String>,
) {
    parent.spawn(button(value, title)).observe(clicked);
}
fn row() -> Node {
    Node {
        column_gap: Val::Px(6.),
        row_gap: Val::Px(6.),
        flex_wrap: FlexWrap::Wrap,
        ..default()
    }
}
fn block() -> Node {
    Node {
        flex_direction: FlexDirection::Column,
        row_gap: Val::Px(7.),
        ..default()
    }
}
fn panel(title: &str, parent: &mut ChildSpawnerCommands) {
    parent.spawn(colored_text("DREHLSS / COURIER SHIFT", 11., MUTED));
    parent.spawn(text(title, 30.));
    add_button(
        parent,
        json!({"action":"ui-panel","panel":""}),
        "Close · Esc",
    );
}

pub(crate) fn install(app: &mut App) {
    click::install(app);
    let gpu = UiGpu::default();
    if let Some(render) = app.get_sub_app_mut(bevy::render::RenderApp) {
        render.insert_resource(gpu.clone()).add_systems(
            bevy::render::Render,
            report_ui_gpu.in_set(bevy::render::RenderSystems::Cleanup),
        );
    }
    app.insert_resource(gpu)
        .init_resource::<HitRegions>()
        .init_resource::<LastPhoto>()
        .init_resource::<crate::actors::Controls>()
        .insert_resource(Ui {
            sensitivity: 0.0024,
            sound: cfg!(target_arch = "wasm32")
                || !std::env::var("FEED_GAME_SOUND").is_ok_and(|value| value == "off"),
            welcome: !crate::platform::arguments()
                .iter()
                .any(|a| matches!(a.as_str(), "--lat" | "--join" | "--random")),
            ..default()
        })
        .add_systems(Startup, setup)
        .add_systems(First, receive)
        .add_systems(PreUpdate, shield.after(bevy::picking::PickingSystems::Last))
        .add_systems(
            PostUpdate,
            input_regions.after(bevy::ui::UiSystems::PostLayout),
        )
        .add_systems(
            Update,
            (
                actions
                    .before(crate::map::input)
                    .before(crate::browser_ui::input),
                photo,
                paint,
                panels,
                settings_labels,
                button_feedback,
                responsive,
                welcome::responsive,
                dynamic,
            )
                .chain(),
        );
}
fn button_feedback(
    ui: Res<Ui>,
    mut buttons: Query<
        (
            &Action,
            &bevy::picking::hover::Hovered,
            Has<bevy::ui::Pressed>,
            Has<welcome::Utility>,
            &mut BackgroundColor,
            &mut BorderColor,
        ),
        Without<MapStop>,
    >,
) {
    for (action, hovered, pressed, utility, mut background, mut border) in &mut buttons {
        let selected = action.0["action"] == "delivery"
            && action.0["index"] == ui.value("paket-hud")["selected"];
        let primary = action.0["action"] == "welcome-play";
        background.0 = if primary {
            if pressed {
                Color::srgb(0.72, 0.28, 0.06)
            } else if hovered.get() {
                Color::srgb(1., 0.57, 0.25)
            } else {
                ORANGE
            }
        } else if utility {
            Color::srgba(0.09, 0.07, 0.055, if hovered.get() { 0.90 } else { 0.66 })
        } else if selected {
            Color::srgb(0.27, 0.14, 0.07)
        } else if hovered.get() {
            Color::srgb(0.20, 0.29, 0.23)
        } else {
            PANEL
        };
        *border = BorderColor::all(if primary || selected || hovered.get() {
            ORANGE
        } else {
            Color::srgb(0.23, 0.30, 0.25)
        });
    }
}
fn responsive(
    ui: Res<Ui>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    mut nodes: Query<(
        &mut Node,
        Has<Header>,
        Has<HeaderBrand>,
        Option<&HeaderOffset>,
        Option<&HudCard>,
        Option<&Radar>,
        Has<HeaderAction>,
        Has<RouteList>,
    )>,
    actions: Query<(&Action, &Children), With<HeaderAction>>,
    mut texts: Query<(&mut Text, &mut TextFont, Has<HeaderBrand>)>,
) {
    let Ok(window) = windows.single() else {
        return;
    };
    let compact = window.width() < 1000.;
    let portrait = window.width() < 600.;
    let short = window.height() < 560.;
    for (mut node, header, brand, offset, card, radar, header_action, route_list) in &mut nodes {
        if let Some(offset) = offset {
            node.top = Val::Px(header_height(window.width()) + offset.0);
        }
        if route_list {
            let available =
                window.height() - header_height(window.width()) - if portrait { 340. } else { 42. };
            node.max_height = px((available - 256.).clamp(38., 480.));
        }
        if let Some(card) = card {
            match card {
                HudCard::Route => {
                    node.width = px(if portrait {
                        176.
                    } else if short {
                        220.
                    } else {
                        288.
                    });
                    node.left = px(if portrait || short { 12. } else { 24. });
                    node.height = Val::Auto;
                    node.max_height = px((window.height()
                        - header_height(window.width())
                        - if portrait { 340. } else { 42. })
                    .max(180.));
                    node.overflow = Overflow::clip();
                    node.padding = UiRect::all(px(if portrait || short { 10. } else { 14. }));
                    node.row_gap = px(if short { 5. } else { 10. });
                }
                HudCard::Instruction => {
                    node.left = px(if portrait {
                        12.
                    } else if short {
                        252.
                    } else {
                        332.
                    });
                    node.right = Val::Auto;
                    node.width = px(if portrait {
                        window.width() - 24.
                    } else if short {
                        window.width() - 452.
                    } else {
                        (window.width() - 356.).min(720.)
                    });
                    node.bottom = px(64);
                    node.height = px(if portrait {
                        240.
                    } else if short {
                        196.
                    } else {
                        220.
                    });
                }
                HudCard::Radar => {
                    node.width = px(if portrait {
                        138.
                    } else if short {
                        168.
                    } else {
                        248.
                    });
                    node.right = px(if portrait { 12. } else { 22. });
                    node.padding = UiRect::all(px(if portrait { 7. } else { 12. }));
                }
            }
        }
        if radar.is_some_and(|r| !r.overview) {
            node.width = px(if portrait {
                124.
            } else if short {
                144.
            } else {
                224.
            });
            node.height = node.width;
        }

        if header {
            node.display = if ui.welcome && ui.mode != "ride" {
                Display::None
            } else {
                Display::Flex
            };
            node.height = Val::Px(header_height(window.width()));
            node.padding = UiRect::axes(px(if compact { 12. } else { 20. }), px(10));
        }
        if header_action {
            node.padding = UiRect::axes(px(if compact { 6. } else { 12. }), px(8));
        }
        if brand {
            node.width = if portrait {
                Val::Percent(100.)
            } else {
                Val::Auto
            };
            node.height = if portrait { Val::Px(22.) } else { Val::Auto };
            node.flex_grow = if portrait { 0. } else { 1. };
        }
    }
    for (_, mut font, brand) in &mut texts {
        if brand {
            font.font_size = FontSize::Px(if compact { 16. } else { 18. });
        }
    }
    for (action, children) in &actions {
        let title = match (action.0["panel"].as_str(), action.0["kind"].as_str()) {
            (Some("share"), _) => {
                if compact {
                    "Share"
                } else {
                    "Share mesh"
                }
            }
            (Some("help"), _) => {
                if compact {
                    "Help"
                } else {
                    "How to play"
                }
            }
            (_, Some("fullscreen")) => {
                if compact {
                    "Full"
                } else {
                    "Fullscreen"
                }
            }
            (_, Some("sound")) => {
                if compact {
                    ""
                } else {
                    "Sound"
                }
            }
            _ => continue,
        };
        for child in children {
            if let Ok((mut text, ..)) = texts.get_mut(*child) {
                if text.0 != title {
                    text.0 = title.into();
                }
            }
        }
    }
}

fn setup(mut commands: Commands, assets: Res<AssetServer>) {
    commands
        .spawn((
            Interface,
            Layer::Header,
            Node {
                width: Val::Percent(100.),
                height: Val::Percent(100.),
                position_type: PositionType::Absolute,
                left: Val::Px(0.),
                top: Val::Px(0.),
                ..default()
            },
            Pickable::IGNORE,
            GlobalZIndex(20),
        ))
        .with_children(|root| {
            root.spawn((
                Header,
                Node {
                    width: Val::Percent(100.),
                    height: Val::Px(62.),
                    flex_wrap: FlexWrap::Wrap,
                    row_gap: Val::Px(4.),
                    padding: UiRect::axes(Val::Px(20.), Val::Px(10.)),
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(6.),
                    ..default()
                },
                BackgroundColor(INK),
            ))
            .with_children(|p| {
                p.spawn((
                    HeaderBrand,
                    text("DREHLSS / Courier shift", 18.),
                    Node {
                        flex_grow: 1.,
                        ..default()
                    },
                ));
                for (name, title) in [
                    ("profile", "Profile"),
                    ("share", "Share mesh"),
                    ("help", "How to play"),
                    ("settings", "Settings"),
                ] {
                    p.spawn((
                        HeaderAction,
                        button(json!({"action":"ui-panel","panel":name}), title),
                    ))
                    .observe(clicked);
                }
                p.spawn((
                    HeaderAction,
                    button(json!({"action":"service","kind":"sound"}), "Sound"),
                ))
                .observe(clicked)
                .with_children(|p| {
                    p.spawn((
                        ImageNode::new(assets.load("native/audio/speaker.png")),
                        Node {
                            width: Val::Px(24.),
                            height: Val::Px(24.),
                            ..default()
                        },
                    ));
                });
                p.spawn((
                    HeaderAction,
                    button(
                        json!({"action":"service","kind":"fullscreen"}),
                        "Fullscreen",
                    ),
                ))
                .observe(clicked);
            });
            root.spawn((
                RideHud,
                HudCard::Route,
                HeaderOffset(18.),
                Layer::Hud,
                Node {
                    position_type: PositionType::Absolute,
                    top: px(80),
                    left: px(24),
                    width: px(288),
                    padding: UiRect::all(px(14)),
                    row_gap: px(10),
                    flex_direction: FlexDirection::Column,
                    ..default()
                },
                BackgroundColor(INK),
            ))
            .with_children(|p| {
                p.spawn((
                    Bound("day"),
                    colored_text("DAY 1 / SHIFT ENDS 17:00", 11., MUTED),
                ));
                p.spawn(Node {
                    justify_content: JustifyContent::SpaceBetween,
                    align_items: AlignItems::Center,
                    ..default()
                })
                .with_children(|p| {
                    p.spawn((Bound("clock"), text("09:00", 28.)));
                    p.spawn((Bound("score"), colored_text("0 KPI", 20., ORANGE)));
                });
                p.spawn(row()).with_children(|p| {
                    add_button(p, json!({"action":"ui-panel","panel":"route"}), "Map · M");
                    p.spawn(colored_text("Tab / Shift+Tab", 11., MUTED));
                });
                p.spawn((
                    Bound("route-progress"),
                    colored_text("YOUR ROUTE", 11., MUTED),
                ));
                p.spawn((
                    RouteList,
                    bevy::ui_widgets::ScrollArea,
                    Node {
                        min_height: px(0),
                        overflow: Overflow::scroll_y(),
                        ..block()
                    },
                ))
                .with_children(|p| {
                    p.spawn((Dynamic("manifest"), block()));
                });
                p.spawn((
                    Bound("dispatcher"),
                    colored_text("", 12., ORANGE),
                    Node {
                        height: px(48),
                        flex_shrink: 0.,
                        overflow: Overflow::clip(),
                        ..default()
                    },
                ));
            });
            root.spawn((
                RideHud,
                HudCard::Instruction,
                Layer::Hud,
                Node {
                    position_type: PositionType::Absolute,
                    bottom: px(24),
                    left: px(332),
                    right: px(24),
                    height: px(220),
                    padding: UiRect::all(px(14)),
                    border: UiRect::left(px(3)),
                    row_gap: px(6),
                    overflow: Overflow::clip(),
                    ..block()
                },
                BackgroundColor(INK),
                BorderColor::all(ORANGE),
            ))
            .with_children(|p| {
                p.spawn((
                    Bound("stop-label"),
                    colored_text("CURRENT DELIVERY", 11., ORANGE),
                ));
                p.spawn((Bound("stop-address"), text("Finding addresses…", 20.)));
                p.spawn((Bound("stop-recipient"), colored_text("", 13., MUTED)));
                p.spawn((Bound("delivery"), text("", 14.)));
                p.spawn((
                    Bound("status"),
                    colored_text("", 13., ORANGE),
                    Node {
                        min_height: px(32),
                        ..default()
                    },
                ));
            });
            root.spawn((
                RideHud,
                HudCard::Radar,
                HeaderOffset(18.),
                Layer::Hud,
                Node {
                    position_type: PositionType::Absolute,
                    top: Val::Px(54.),
                    right: Val::Px(22.),
                    width: Val::Px(248.),
                    padding: UiRect::all(px(12)),
                    ..block()
                },
                BackgroundColor(INK),
            ))
            .with_children(|p| {
                p.spawn(colored_text("NEARBY / N ↑", 11., MUTED));
                p.spawn((
                    Radar { overview: false },
                    Node {
                        width: Val::Px(224.),
                        height: Val::Px(224.),
                        overflow: Overflow::clip(),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.10, 0.21, 0.16)),
                ));
                p.spawn(colored_text("▲ You   ◆ Stop   ● Van", 11., MUTED));
                p.spawn((Dynamic("peers"), block()));
            });
            root.spawn((
                RideHud,
                Layer::Hud,
                Node {
                    position_type: PositionType::Absolute,
                    bottom: Val::Px(12.),
                    right: Val::Px(12.),
                    ..row()
                },
            ))
            .with_children(|p| {
                add_button(
                    p,
                    json!({"action":"service","kind":"capture"}),
                    "Capture mouse",
                );
                add_button(p, json!({"action":"ui-panel","panel":"photo"}), "Photo");
            });
            welcome::spawn(root, &assets);
            root.spawn((
                WorldLoading,
                Layer::Loading,
                GlobalZIndex(70),
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(0.),
                    top: Val::Px(0.),
                    width: Val::Percent(100.),
                    height: Val::Percent(100.),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(16.),
                    display: Display::None,
                    ..default()
                },
                BackgroundColor(INK),
            ))
            .with_children(|p| {
                p.spawn(text("Loading neighbourhood", 28.));
                p.spawn((
                    Spinner,
                    Node {
                        width: Val::Px(36.),
                        height: Val::Px(36.),
                        border: UiRect::all(Val::Px(4.)),
                        border_radius: BorderRadius::MAX,
                        ..default()
                    },
                    BorderColor {
                        top: ORANGE,
                        left: PAPER,
                        right: PAPER,
                        bottom: PAPER,
                    },
                    UiTransform::default(),
                ));
                p.spawn((Bound("loading"), text("Starting neighbourhood…", 16.)));
                p.spawn(row()).with_children(|p| {
                    add_button(p, json!({"action":"retry-map"}), "Retry map");
                    add_button(
                        p,
                        json!({"action":"change-location"}),
                        "Choose neighbourhood",
                    );
                });
            });
            root.spawn((
                HeaderOffset(2.),
                Bound("notice"),
                BackgroundColor(Color::NONE),
                text("", 14.),
                Node {
                    position_type: PositionType::Absolute,
                    top: Val::Px(48.),
                    left: Val::Percent(26.),
                    max_width: Val::Percent(50.),
                    min_height: Val::Px(20.),
                    ..default()
                },
            ));
            root.spawn((
                text(
                    "Map data © OpenStreetMap contributors · Elevation: Mapzen · Search: Photon",
                    10.,
                ),
                Node {
                    position_type: PositionType::Absolute,
                    bottom: Val::Px(2.),
                    left: Val::Px(6.),
                    ..default()
                },
            ));
        });
}
fn receive(
    mut ui: ResMut<Ui>,
    mut controls: ResMut<crate::actors::Controls>,
    time: Res<Time>,
    mut fields: Query<
        (
            &mut EditableText,
            Has<crate::map::AddressInput>,
            Has<crate::map::InviteInput>,
        ),
        Or<(
            With<crate::map::AddressInput>,
            With<crate::map::InviteInput>,
        )>,
    >,
    mut font_context: ResMut<bevy::text::FontCx>,
    mut layout_context: ResMut<bevy::text::LayoutCx>,
) {
    ui.sensitivity = f64::from(controls.sensitivity);
    for (name, value) in std::mem::take(&mut *EVENTS.lock().unwrap()) {
        if name == "paket-mode" {
            ui.mode = value.as_str().unwrap_or("").into();
            if ui.mode == "ride" {
                ui.welcome = false;
            }
            ui.panel.clear();
            ui.built_panel = "__reset".into();
            ui.mode_since = time.elapsed_secs();
            ui.values.remove("paket-hud");
            ui.values.remove("paket-ride-loading");
        }
        if name == "paket-notice" {
            ui.notice = value.as_str().unwrap_or("").into();
            ui.notice_until = time.elapsed_secs() + 8.;
        }
        if name == "paket-settings" {
            ui.sound = value["sound"].as_bool().unwrap_or(ui.sound);
            if let Some(value) = value["sensitivity"].as_f64() {
                set_sensitivity(&mut ui, &mut controls, value);
            }
        }
        if name == "paket-services" {
            let target = ui.values.entry(name).or_insert(json!({}));
            if let (Some(target), Some(patch)) = (target.as_object_mut(), value.as_object()) {
                target.extend(patch.clone());
            }
        } else {
            ui.values.insert(name, value);
        }
    }
    for input in crate::platform::text_inputs() {
        if ui.mode != "map" || ui.panel_open() {
            continue;
        }
        let Some(name) = input["field"]
            .as_str()
            .filter(|v| matches!(*v, "address" | "invite"))
        else {
            continue;
        };
        let Some(raw) = input["value"].as_str() else {
            continue;
        };
        let value = raw
            .chars()
            .take(if name == "address" { 180 } else { 8192 })
            .collect::<String>();
        let start = utf16_byte_offset(
            &value,
            input["start"]
                .as_u64()
                .unwrap_or(value.encode_utf16().count() as u64) as usize,
        );
        let end = utf16_byte_offset(
            &value,
            input["end"]
                .as_u64()
                .unwrap_or(value.encode_utf16().count() as u64) as usize,
        );
        for (mut field, address, invite) in &mut fields {
            if !((name == "address" && address) || (name == "invite" && invite)) {
                continue;
            }
            if field.value().to_string() != value {
                field.editor_mut().set_text(&value);
            }
            field.pending_edits.clear();
            field.pending_paste = None;
            field
                .editor_mut()
                .driver(&mut font_context, &mut layout_context.0)
                .select_byte_range(start.min(end), start.max(end));
            if input["submit"] == true {
                queue(if address {
                    json!({"action":"service","kind":"search"})
                } else {
                    json!({"action":"join","link":value})
                });
            }
            break;
        }
    }
}
fn actions(
    mut ui: ResMut<Ui>,
    mut controls: ResMut<crate::actors::Controls>,
    keys: Res<ButtonInput<KeyCode>>,
    mut mouse: ResMut<ButtonInput<MouseButton>>,
    inputs: Query<&EditableText, With<crate::map::AddressInput>>,
    mode: Res<State<Mode>>,
) {
    if *mode.get() == Mode::Ride && keys.just_pressed(KeyCode::F1) {
        queue(json!({"action":"ui-panel","panel":if ui.panel=="settings"{""}else{"settings"}}));
    }
    if *mode.get() == Mode::Ride && keys.just_pressed(KeyCode::KeyM) {
        queue(json!({"action":"ui-panel","panel":if ui.panel=="route"{""}else{"route"}}));
    }
    if keys.just_pressed(KeyCode::Escape) && !ui.panel.is_empty() {
        queue(json!({"action":"ui-panel","panel":""}));
    }
    // Local interface actions are consumed before the game command dispatcher.
    let pending = drain_commands();
    for command in pending {
        match command["action"].as_str().unwrap_or("") {
            "welcome-play" => {
                ui.welcome = false;
                crate::platform::ui_event("paket-ui-action", &json!({"action":"play"}).to_string());
            }
            "ui-panel" => {
                ui.panel = command["panel"].as_str().unwrap_or("").into();
                queue(json!({"action":"menu","open":!ui.panel.is_empty()}));
                if !ui.panel.is_empty() {
                    crate::platform::service(&json!({"kind":"release"}));
                }
                if ui.panel == "profile" {
                    crate::platform::service(&json!({"kind":"profile"}));
                }
                mouse.clear_just_pressed(MouseButton::Left);
            }
            "change-location" => {
                // Close the settings menu before the ride dispatcher checks interactions.
                ui.panel.clear();
                queue(json!({"action":"menu","open":false}));
                queue(command);
                mouse.clear_just_pressed(MouseButton::Left);
            }
            "service" => {
                let mut request = command.clone();
                let kind = command["kind"].as_str().unwrap_or("");
                if kind == "copy-invite" {
                    let link = ui.value("paket-session")["link"].as_str().unwrap_or("");
                    if !link.is_empty() {
                        let copied = crate::platform::copy_text(link);
                        if !copied {
                            crate::platform::ui_event("paket-notice",&json!("Clipboard unavailable. Select the invitation field and copy it.").to_string());
                        }
                    }
                    continue;
                }
                if kind == "search" {
                    request["query"] = json!(
                        inputs
                            .single()
                            .map(|v| v.value().to_string())
                            .unwrap_or_default()
                    );
                }
                if kind == "public" {
                    ui.public = !ui.public;
                    request["listed"] = json!(ui.public);
                }
                if kind == "sound" {
                    ui.sound = !ui.sound;
                    request["enabled"] = json!(ui.sound);
                }
                if kind == "sensitivity" {
                    let value =
                        f64::from(controls.sensitivity) + command["delta"].as_f64().unwrap_or(0.);
                    set_sensitivity(&mut ui, &mut controls, value);
                    request["value"] = json!(ui.sensitivity);
                }
                crate::platform::service(&request);
                mouse.clear_just_pressed(MouseButton::Left);
            }
            _ => queue(command),
        }
    }
}
fn route_state(state: &str) -> &str {
    match state {
        "in-van" => "At van",
        "carrying" => "Carrying",
        "in-flight" => "In flight",
        "set-aside" => "Set aside",
        "photo-due" => "Photo needed",
        "filed" | "handed" => "Delivered",
        _ => state,
    }
}
fn value_text<'a>(v: &'a Value, key: &str) -> &'a str {
    v[key].as_str().unwrap_or("")
}
fn paint(
    mut ui: ResMut<Ui>,
    time: Res<Time>,
    mut labels: Query<(&Bound, &mut Text, Option<&mut BackgroundColor>)>,
    mut hud: Query<&mut Node, (With<RideHud>, Without<WorldLoading>, Without<Welcome>)>,
    mut loader: Query<&mut Node, (With<WorldLoading>, Without<RideHud>, Without<Welcome>)>,
    mut welcome: Query<&mut Node, (With<Welcome>, Without<RideHud>, Without<WorldLoading>)>,
    mut spins: Query<(&mut UiTransform, &mut Visibility), With<Spinner>>,
) {
    let ride = ui.mode == "ride";
    for mut node in &mut welcome {
        node.display = if !ride && ui.welcome {
            Display::Flex
        } else {
            Display::None
        };
    }
    for mut node in &mut hud {
        node.display = if ride { Display::Flex } else { Display::None };
    }
    let h = ui.value("paket-hud");
    for mut node in &mut loader {
        node.display = if ride && !h.is_object() {
            Display::Flex
        } else {
            Display::None
        };
    }
    for (mut spin, mut visible) in &mut spins {
        spin.rotation = Rot2::radians(time.elapsed_secs() * 4.);
        *visible = if matches!(
            ui.value("paket-ride-loading").as_str(),
            Some("failed" | "empty")
        ) || time.elapsed_secs() - ui.mode_since > 65.
        {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        };
    }
    let route = h["routes"]
        .as_array()
        .and_then(|r| r.get(h["selected"].as_u64().unwrap_or(0) as usize));
    let minute = h["minute"].as_u64().unwrap_or(540);
    for (binding, mut text, mut background) in &mut labels {
        if binding.0 == "notice" {
            if let Some(background) = background.as_mut() {
                background.0 = if time.elapsed_secs() < ui.notice_until {
                    INK
                } else {
                    Color::NONE
                };
            }
        }
        let next = match binding.0 {
            "map-loading" => {
                if ui.value("paket-map-loading")["failed"] == true {
                    "Some map tiles failed. Retry.".into()
                } else if ui.value("paket-map-loading")["pending"] == true {
                    "Loading map…".into()
                } else {
                    String::new()
                }
            }
            "clock" => format!("{:02}:{:02}", minute / 60, minute % 60),
            "day" => format!("DAY {} / SHIFT ENDS 17:00", h["day"].as_u64().unwrap_or(1)),
            "score" => format!("{} KPI", h["score"].as_i64().unwrap_or(0)),
            "stop-label" => format!(
                "DELIVERY {} / {}  ·  {:.0} m",
                h["selected"].as_u64().unwrap_or(0) + 1,
                h["routes"].as_array().map_or(0, Vec::len),
                route.and_then(|r| r["distance"].as_f64()).unwrap_or(0.)
            ),
            "stop-address" => route
                .map(|r| value_text(r, "address"))
                .unwrap_or("Finding addresses…")
                .into(),
            "stop-recipient" => route.map(|r| value_text(r, "name")).unwrap_or("").into(),
            "delivery" => crate::ui_guidance::delivery(h),
            "sensitivity" => format!("Mouse sensitivity: {:.4}", ui.sensitivity),
            "status" => value_text(h, "status").into(),
            "route-progress" => {
                let routes = h["routes"].as_array();
                let done = routes
                    .into_iter()
                    .flatten()
                    .filter(|r| matches!(r["state"].as_str(), Some("filed" | "handed")))
                    .count();
                format!(
                    "YOUR ROUTE  /  {done} OF {} DELIVERED",
                    routes.map_or(0, Vec::len)
                )
            }
            "dispatcher" => {
                let d = &h["dispatcher"];
                format!("{}\n{}", value_text(d, "message"), value_text(d, "timer"))
            }
            "peers" => ui
                .value("paket-members")
                .as_array()
                .map(|a| {
                    a.iter()
                        .enumerate()
                        .map(|(i, p)| {
                            format!(
                                "{} · {}",
                                i + 1,
                                p["name"].as_str().or(p["id"].as_str()).unwrap_or("Player")
                            )
                        })
                        .collect::<Vec<_>>()
                        .join("\n")
                })
                .unwrap_or_else(|| "No other players".into()),
            "notice" => {
                if time.elapsed_secs() < ui.notice_until {
                    ui.notice.clone()
                } else {
                    String::new()
                }
            }
            "service" => value_text(ui.value("paket-services"), "status").into(),
            "loading" => {
                if ui.value("paket-ride-loading") == "failed" {
                    "Neighbourhood could not load. Retry or choose another place.".into()
                } else if ui.value("paket-ride-loading") == "empty" {
                    "No playable streets here. Choose another place.".into()
                } else {
                    if time.elapsed_secs() - ui.mode_since > 65. {
                        "Loading took too long. Retry or choose another neighbourhood.".into()
                    } else {
                        "Starting neighbourhood…".into()
                    }
                }
            }
            _ => continue,
        };
        if text.0 != next {
            text.0 = next;
        }
    }
    if ride && h["ended"] == true && ui.panel.is_empty() {
        ui.panel = "vote".into();
    }
}
fn panels(
    mut commands: Commands,
    mut ui: ResMut<Ui>,
    roots: Query<Entity, With<Modal>>,
    mode: Res<State<Mode>>,
    photo: Res<LastPhoto>,
) {
    if ui.built_panel == ui.panel {
        return;
    }
    for entity in &roots {
        commands.entity(entity).despawn();
    }
    ui.built_panel = ui.panel.clone();
    if ui.panel.is_empty() {
        return;
    }
    let selected = ui.panel.clone();
    commands.spawn((
        Modal,
        Layer::Dialog,
        GlobalZIndex(39),
        Node {
            position_type: PositionType::Absolute,
            width: percent(100),
            height: percent(100),
            ..default()
        },
        BackgroundColor(Color::srgba(0.015, 0.025, 0.02, 0.72)),
    ));
    commands.spawn((Modal,HeaderOffset(18.),Layer::Dialog,bevy::ui_widgets::ScrollArea,GlobalZIndex(40),Node {position_type:PositionType::Absolute,left:Val::Percent(7.),top:Val::Px(80.),width:Val::Percent(86.),max_height:Val::Percent(82.),padding:UiRect::all(Val::Px(24.)),border:UiRect::all(px(1)),row_gap:px(12),overflow:Overflow::scroll_y(),..block()},BorderColor::all(Color::srgb(0.25,0.32,0.27)),BackgroundColor(INK))).with_children(|p|{
        panel(match selected.as_str(){"help"=>"How to deliver","settings"=>"Settings","profile"=>"Your career","share"=>"Share this mesh","photo"=>"Delivery photo","vote"=>"Day complete",_=>"Your route"},p);
        match selected.as_str(){
            "help"=>{p.spawn(text("WASD: move or drive · Shift: run · V: enter/leave van\nE: collect, exchange or hand over parcel · Q: return to van · X: set aside\nTab / Shift+Tab: choose delivery · R: intercom\nRing the named recipient before leaving a parcel. E places it; T throws it.\nF: camera. Centre the settled parcel within 9 m, then Space to photograph.\nDogs: E bite, Space jump, G mark van, B bark.\nM: route map · F1: settings · Esc: release cursor.\nThe shift continues while panels are open. Correct deliveries earn 0 KPI; questionable evidence earns more.",16.));}
            "settings"=>{
                p.spawn((Bound("sensitivity"),text("Mouse sensitivity",16.)));
                p.spawn(row()).with_children(|r|{add_button(r,json!({"action":"service","kind":"sensitivity","delta":-0.0003}),"−");add_button(r,json!({"action":"service","kind":"sensitivity","delta":0.0003}),"+");});
                p.spawn(text("Difficulty can change until the first doorbell, by the host.",14.));
                p.spawn(row()).with_children(|r|for(i,label)in["Trainee","Courier","Express"].iter().enumerate(){add_button(r,json!({"action":"difficulty","value":i}),*label);});
                add_button(p,json!({"action":"service","kind":"sound"}),if ui.sound{"Sound: on"}else{"Sound: off"});
                if *mode.get()==Mode::Ride {add_button(p,json!({"action":"change-location"}),"Change neighbourhood");}
            }
            "share"=>{p.spawn((Dynamic("share"),block()));}
            "profile"=>{p.spawn((Dynamic("profile"),block()));add_button(p,json!({"action":"service","kind":"new-face"}),"New portrait");if *mode.get()!=Mode::Ride {add_button(p,json!({"action":"service","kind":"signin"}),"Sign in with Identity");}}
            "photo"=>{if let Some(image)=&photo.0{p.spawn((ImageNode::new(image.clone()),Node{width:Val::Percent(100.),max_height:Val::Px(340.),..default()}));}p.spawn(text("A saved delivery photo is available for download after you file evidence.",16.));add_button(p,json!({"action":"service","kind":"download-photo"}),"Download photo");}
            "vote"=>{p.spawn((Dynamic("vote"),block()));}
            "route"=>{
                p.spawn(colored_text("Select a numbered stop. M or Esc returns to your shift.",14.,MUTED));
                p.spawn((Radar{overview:true},Node {width:percent(100),height:Val::Vh(46.),min_height:px(180),flex_shrink:0.,overflow:Overflow::clip(),..default()},BackgroundColor(Color::srgb(0.10,0.21,0.16))));
                p.spawn((Dynamic("routes"),row()));
            }
            _=>{p.spawn((Dynamic("routes"),block()));}
        }
    });
}
fn dynamic(
    mut commands: Commands,
    ui: Res<Ui>,
    regions: Query<(Entity, &Dynamic, Option<&Children>)>,
    mut old: Local<HashMap<Entity, String>>,
    radars: Query<(Entity, &ComputedNode, &Radar)>,
    all_children: Query<&Children>,
    mut stop_nodes: Query<(&MapStop, &mut Node, &mut BackgroundColor)>,
    mut radar_layers: Local<HashMap<Entity, RadarLayers>>,
    mut last_radar: Local<f32>,
    time: Res<Time>,
) {
    for (entity, kind, children) in &regions {
        let data = match kind.0 {
            "routes" | "manifest" => &ui.value("paket-hud")["routes"],
            "vote" => &ui.value("paket-hud")["vote"],
            "profile" => ui.value("paket-profile"),
            "share" => ui.value("paket-session"),
            "peers" => ui.value("paket-members"),
            _ => &Value::Null,
        };
        let signature = if matches!(kind.0, "routes" | "manifest") {
            data.as_array().map(|routes|routes.iter().map(|r|json!({"index":r["index"],"address":r["address"],"state":r["state"]})).collect::<Vec<_>>()).map(|v|json!(v).to_string()).unwrap_or_default()
        } else {
            data.to_string()
        };
        if old.get(&entity) == Some(&signature) {
            continue;
        }
        old.insert(entity, signature);
        if let Some(children) = children {
            for child in children {
                commands.entity(*child).despawn();
            }
        }
        commands.entity(entity).with_children(|p| match kind.0 {
            "routes" | "manifest" => {
                if let Some(routes) = data.as_array() {
                    for route in routes {
                        let index = route["index"].as_u64().unwrap_or(0) + 1;
                        let done = matches!(route["state"].as_str(), Some("filed" | "handed"));
                        let title = if kind.0 == "manifest" {
                            format!(
                                "{index:02}  {}{}",
                                if done { "[x] " } else { "" },
                                value_text(route, "address")
                            )
                        } else {
                            format!(
                                "{index} · {} · {}",
                                value_text(route, "address"),
                                route_state(value_text(route, "state"))
                            )
                        };
                        p.spawn((button(
                            json!({"action":"delivery","index":route["index"]}),
                            title,
                        ),))
                            .observe(clicked)
                            .insert(AccessibleLabel::new(format!("Route stop {index}")));
                    }
                }
            }
            "vote" => {
                p.spawn(text(
                    format!(
                        "Next shift begins in {} seconds",
                        data["remaining"].as_u64().unwrap_or(0)
                    ),
                    16.,
                ));
                for player in data["players"].as_array().into_iter().flatten() {
                    p.spawn(text(
                        format!(
                            "{} · {} parcels · {} KPI",
                            value_text(player, "name"),
                            player["delivered"],
                            player["score"]
                        ),
                        15.,
                    ));
                }
                if let Some(options) = data["options"].as_array() {
                    for (i, option) in options.iter().enumerate() {
                        add_button(
                            p,
                            json!({"action":"shift-vote","choice":i}),
                            format!("{} · {}", i + 1, option.as_str().unwrap_or("Neighbourhood")),
                        );
                    }
                }
            }
            "share" => {
                let link = value_text(data, "link");
                p.spawn(text(
                    if link.is_empty() {
                        "Play with friends to create an invitation."
                    } else {
                        link
                    },
                    14.,
                ));
                if !link.is_empty() {
                    p.spawn((
                        bevy::ui_widgets::TextInput,
                        EditableText::new(link),
                        bevy::text::TextReadWriteMode::ReadOnly,
                        AccessibleLabel::new("Invitation link"),
                        TextFont {
                            font_size: FontSize::Px(13.),
                            ..default()
                        },
                        TextColor(PAPER),
                        Node {
                            width: Val::Percent(100.),
                            min_height: Val::Px(36.),
                            overflow: Overflow::clip(),
                            padding: UiRect::all(Val::Px(6.)),
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.04, 0.08, 0.06)),
                    ));
                }
                add_button(
                    p,
                    json!({"action":"service","kind":"copy-invite"}),
                    "Copy invitation",
                );
            }
            "peers" => {
                let own = ui.value("paket-session")["device"].as_str().unwrap_or("");
                let mut count = 0;
                for member in data
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter(|m| m["id"].as_str() != Some(own))
                {
                    count += 1;
                    let slot = member["slot"].as_u64().unwrap_or(count) as usize;
                    p.spawn((
                        RadarMark {
                            kind: "roster",
                            id: value_text(member, "id").into(),
                            slot: Some(slot),
                        },
                        Text::new(format!("{} · {}", slot + 1, value_text(member, "name"))),
                        TextFont {
                            font_size: FontSize::Px(12.),
                            ..default()
                        },
                        TextColor(peer_color(slot)),
                    ));
                }
                if count == 0 {
                    p.spawn(text("No other players", 12.));
                }
            }
            "profile" => {
                portrait(p, data["face"].as_u64().unwrap_or(0) as u32);
                p.spawn(text(
                    format!(
                        "{}\nParcels: {} · KPI: {}\nTime: {} minutes\n{}",
                        data["name"].as_str().unwrap_or("Courier"),
                        data["stats"]["delivered"].as_u64().unwrap_or(0),
                        data["stats"]["score"].as_i64().unwrap_or(0),
                        data["stats"]["workMs"].as_u64().unwrap_or(0) / 60000,
                        data["linked"].as_bool().map_or(
                            "Career loads from this installation.",
                            |v| if v { "Identity linked" } else { "Guest career" }
                        )
                    ),
                    17.,
                ));
            }
            _ => {}
        });
    }
    if time.elapsed_secs() - *last_radar < 0.1 {
        return;
    }
    *last_radar = time.elapsed_secs();
    let data = ui.value("paket-radar");
    let position = point(&data["position"]);
    let roads = ui.value("paket-map-roads");
    let sports = ui.value("paket-map-sports");
    let geometry = json!([roads, sports]).to_string();
    for (entity, node, radar) in &radars {
        let extent = node.size() * node.inverse_scale_factor();
        if extent.min_element() <= 1. {
            continue;
        }
        let overview = radar.overview;
        let center = extent * 0.5;
        let radius = extent.min_element() * 0.5 - 12.;
        let (mut origin, mut scale) = (position, extent.x / 320.);
        if overview {
            let (mut min, mut max) = (position, position);
            for route in ui.value("paket-hud")["routes"]
                .as_array()
                .into_iter()
                .flatten()
            {
                let p = point(&route["point"]);
                min = min.min(p);
                max = max.max(p);
            }
            if data["van"].is_array() {
                let p = point(&data["van"]);
                min = min.min(p);
                max = max.max(p);
            }
            origin = (min + max) * 0.5;
            let span = (max - min + Vec2::splat(70.)).max(Vec2::splat(100.));
            scale = ((extent.x - 20.) / span.x).min((extent.y - 20.) / span.y);
        }
        let layers = radar_layers.entry(entity).or_insert_with(|| RadarLayers {
            roads: commands
                .spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        ..default()
                    },
                    ChildOf(entity),
                    UiTransform::default(),
                    Pickable::IGNORE,
                ))
                .id(),
            markers: commands
                .spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        width: Val::Percent(100.),
                        height: Val::Percent(100.),
                        ..default()
                    },
                    ChildOf(entity),
                    Pickable::IGNORE,
                ))
                .id(),
            stops: commands
                .spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        width: Val::Percent(100.),
                        height: Val::Percent(100.),
                        ..default()
                    },
                    ChildOf(entity),
                    Pickable::IGNORE,
                ))
                .id(),
            signature: String::new(),
            stops_signature: String::new(),
        });
        commands.entity(layers.roads).insert((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(center.x),
                top: Val::Px(center.y),
                ..default()
            },
            UiTransform {
                translation: Val2::px(-origin.x * scale, -origin.y * scale),
                scale: Vec2::splat(scale),
                ..default()
            },
        ));
        if layers.signature != geometry {
            layers.signature = geometry.clone();
            if let Ok(children) = all_children.get(layers.roads) {
                for child in children {
                    commands.entity(*child).despawn();
                }
            }
            commands.entity(layers.roads).with_children(|p| {
                for road in roads.as_array().into_iter().flatten().take(500) {
                    if let Some(points) = road["points"].as_array() {
                        for pair in points.windows(2) {
                            line(
                                p,
                                point(&pair[0]),
                                point(&pair[1]),
                                Color::srgb(0.50, 0.57, 0.47),
                                road["width"].as_f64().unwrap_or(3.) as f32,
                            );
                        }
                    }
                }
                for shape in sports.as_array().into_iter().flatten() {
                    for ring in shape.as_array().into_iter().flatten() {
                        if let Some(points) = ring.as_array() {
                            for pair in points.windows(2) {
                                line(
                                    p,
                                    point(&pair[0]),
                                    point(&pair[1]),
                                    Color::srgb(0.45, 0.73, 0.40),
                                    2.,
                                );
                            }
                        }
                    }
                }
            });
        }
        if let Ok(children) = all_children.get(layers.markers) {
            for child in children {
                commands.entity(*child).despawn();
            }
        }
        let xy = |at: Vec2| {
            let offset = (at - origin) * scale;
            center
                + if overview {
                    offset
                } else {
                    offset.clamp_length_max(radius)
                }
        };
        if overview {
            let routes = ui.value("paket-hud")["routes"].as_array();
            let signature = routes
                .map(|routes| {
                    json!(
                        routes
                            .iter()
                            .map(|r| json!([r["index"], r["point"], r["address"]]))
                            .collect::<Vec<_>>()
                    )
                    .to_string()
                })
                .unwrap_or_default();
            if layers.stops_signature != signature {
                layers.stops_signature = signature;
                if let Ok(children) = all_children.get(layers.stops) {
                    for child in children {
                        commands.entity(*child).despawn();
                    }
                }
                commands.entity(layers.stops).with_children(|p| {
                    for route in routes.into_iter().flatten() {
                        let index = route["index"].as_u64().unwrap_or(0) as usize;
                        let at = xy(point(&route["point"]));
                        p.spawn((
                            Button,
                            MapStop {
                                radar: entity,
                                index,
                            },
                            Action(json!({"action":"delivery","index":index})),
                            AccessibleLabel::new(format!("Map stop {}", index + 1)),
                            Node {
                                position_type: PositionType::Absolute,
                                left: Val::Px(at.x - 12.),
                                top: Val::Px(at.y - 12.),
                                width: Val::Px(24.),
                                height: Val::Px(24.),
                                border_radius: BorderRadius::MAX,
                                align_items: AlignItems::Center,
                                justify_content: JustifyContent::Center,
                                ..default()
                            },
                            BackgroundColor(ORANGE),
                            children![text((index + 1).to_string(), 12.)],
                        ))
                        .observe(clicked);
                    }
                });
            }
            for (stop, mut node, mut color) in &mut stop_nodes {
                if stop.radar == entity {
                    if let Some(route) = routes.and_then(|r| r.get(stop.index)) {
                        let at = xy(point(&route["point"]));
                        node.left = Val::Px(at.x - 12.);
                        node.top = Val::Px(at.y - 12.);
                        color.0 = if route["index"] == ui.value("paket-hud")["selected"] {
                            ORANGE
                        } else {
                            Color::srgb(0.36, 0.48, 0.39)
                        };
                    }
                }
            }
        }
        commands.entity(layers.markers).with_children(|p| {
            if !overview {
                p.spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(center.x - radius),
                        top: Val::Px(center.y - radius),
                        width: Val::Px(radius * 2.),
                        height: Val::Px(radius * 2.),
                        border: UiRect::all(Val::Px(1.)),
                        border_radius: BorderRadius::MAX,
                        ..default()
                    },
                    BorderColor::all(Color::srgba(0.8, 0.87, 0.72, 0.4)),
                    RadarMark {
                        kind: "ring",
                        id: String::new(),
                        slot: None,
                    },
                ));
            }
            for (key, color, size) in [
                ("van", PAPER, 7.),
                ("target", ORANGE, 20.),
                ("position", Color::WHITE, 8.),
            ] {
                if data[key].is_array() {
                    let at = xy(point(&data[key]));
                    p.spawn((
                        Node {
                            position_type: PositionType::Absolute,
                            left: Val::Px(at.x - size / 2.),
                            top: Val::Px(at.y - size / 2.),
                            width: Val::Px(size),
                            height: Val::Px(size),
                            border_radius: if key == "target" {
                                BorderRadius::ZERO
                            } else {
                                BorderRadius::MAX
                            },
                            ..default()
                        },
                        BackgroundColor(color),
                        RadarMark {
                            kind: key,
                            id: String::new(),
                            slot: None,
                        },
                        UiTransform {
                            rotation: Rot2::radians(if key == "target" {
                                std::f32::consts::FRAC_PI_4
                            } else {
                                0.
                            }),
                            ..default()
                        },
                    ));
                }
            }
            for (key, color) in [
                ("people", Color::srgb(0.65, 0.87, 0.79)),
                ("dogs", Color::srgb(0.93, 0.73, 0.46)),
            ] {
                for value in data[key].as_array().into_iter().flatten() {
                    let at = xy(point(value));
                    p.spawn((
                        Node {
                            position_type: PositionType::Absolute,
                            left: Val::Px(at.x),
                            top: Val::Px(at.y),
                            width: Val::Px(4.),
                            height: Val::Px(4.),
                            border_radius: BorderRadius::MAX,
                            ..default()
                        },
                        BackgroundColor(color),
                    ));
                }
            }
            for peer in data["peers"].as_array().into_iter().flatten() {
                let at = if overview {
                    xy(point(&peer["position"]))
                } else {
                    let delta = point(&peer["position"]) - position;
                    center
                        + if delta.length() >= 1. {
                            delta.normalize() * radius
                        } else {
                            Vec2::ZERO
                        }
                };
                let slot = ui
                    .value("paket-members")
                    .as_array()
                    .and_then(|members| members.iter().find(|m| m["id"] == peer["id"]))
                    .and_then(|m| m["slot"].as_u64())
                    .unwrap_or(0) as usize;
                p.spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(at.x - 8.),
                        top: Val::Px(at.y - 8.),
                        width: Val::Px(16.),
                        height: Val::Px(16.),
                        border_radius: BorderRadius::MAX,
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::Center,
                        ..default()
                    },
                    BackgroundColor(peer_color(slot)),
                    RadarMark {
                        kind: "peer",
                        id: value_text(peer, "id").into(),
                        slot: Some(slot),
                    },
                    children![(
                        Text::new((slot + 1).to_string()),
                        TextFont {
                            font_size: FontSize::Px(10.),
                            ..default()
                        },
                        TextColor(INK)
                    )],
                ));
            }
        });
    }
    radar_layers.retain(|entity, _| radars.contains(*entity));
    old.retain(|entity, _| regions.contains(*entity));
}
fn point(value: &Value) -> Vec2 {
    Vec2::new(
        value[0].as_f64().unwrap_or(0.) as f32,
        value[1].as_f64().unwrap_or(0.) as f32,
    )
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn native_service(value: &Value) {
    match value["kind"].as_str().unwrap_or("") {
        "search"=>{
            let query=value["query"].as_str().unwrap_or("").trim().to_string();
            if query.len()<3 {record("paket-services",r#"{"status":"Type at least three characters."}"#);return;}
            record("paket-services",r#"{"status":"Finding that address…"}"#);
            std::thread::spawn(move||{
                let escaped=query.bytes().map(|b|if b.is_ascii_alphanumeric()||b==b'-'||b==b'_' {char::from(b).to_string()}else{format!("%{b:02X}")}).collect::<String>();
                let result=ureq::get(&format!("https://paketzustellungsverhinderung-simulator-deluxe.meta-uber-engineer.dev/api/address?q={escaped}")).set("User-Agent","Paket native location picker").timeout(std::time::Duration::from_secs(15)).call().ok().and_then(|r|r.into_string().ok());
                let data=result.and_then(|s|serde_json::from_str::<Value>(&s).ok()).filter(Value::is_array);
                let status=match data.as_ref().and_then(Value::as_array) {Some(a)if !a.is_empty()=>"Select a result.",Some(_)=>"No results. Add a city or postal code.",_=>"Address search unavailable. Retry or choose a point on the map."};
                record("paket-services",&json!({"places":data.unwrap_or(json!([])),"status":status}).to_string());
            });
        }
        "sound"=>crate::native_audio::event("paket-sound-setting",&value["enabled"]),
        "location"=>record("paket-notice",&json!("Location permission is unavailable on this platform. Search an address or choose on the map.").to_string()),
        "public"=>record("paket-notice",&json!("Native meshes use local discovery. Share the invitation to connect.").to_string()),
        "resume"=>record("paket-notice",&json!("Paste your saved invitation to return to a native mesh.").to_string()),
        "signin"=>record("paket-notice",&json!("Identity sign-in is available in the web version.").to_string()),

        _=>{}
    }
}

fn input_regions(
    ui: Res<Ui>,
    nodes: Query<
        (
            Entity,
            &ComputedNode,
            &UiGlobalTransform,
            Option<&Action>,
            Option<&InheritedVisibility>,
            Option<&AccessibleLabel>,
            Option<&Children>,
            Option<&bevy::ui::CalculatedClip>,
            Option<&EditableText>,
            Option<&bevy::text::TextReadWriteMode>,
            Has<crate::map::AddressInput>,
            Has<crate::map::InviteInput>,
        ),
        Or<(With<Button>, With<bevy::ui_widgets::TextInput>)>,
    >,
    texts: Query<(
        Entity,
        &Text,
        &ComputedNode,
        &UiGlobalTransform,
        Option<&InheritedVisibility>,
        Option<&bevy::ui::CalculatedClip>,
    )>,
    mut hit_regions: ResMut<HitRegions>,
    time: Res<Time<Real>>,
    gpu: Res<UiGpu>,
    mut last_emit: Local<f32>,
    mut boot_ready: Local<(u8, bool)>,
    mut last: Local<String>,
    image_nodes: Query<(
        Entity,
        &ImageNode,
        &ComputedNode,
        &UiGlobalTransform,
        Option<&InheritedVisibility>,
        Option<&bevy::ui::CalculatedClip>,
    )>,
    images: Res<Assets<Image>>,
    assets: Res<AssetServer>,
    marker_nodes: Query<(
        Entity,
        &RadarMark,
        &ComputedNode,
        &UiGlobalTransform,
        Option<&InheritedVisibility>,
        Option<&bevy::ui::CalculatedClip>,
        Option<&BackgroundColor>,
        Option<&TextColor>,
    )>,
    parents: Query<(Option<&ChildOf>, Option<&Layer>)>,
    press: Option<Res<crate::press_scene::Scene>>,
    cards: Query<(&HudCard, &ComputedNode, &UiGlobalTransform)>,
) {
    let mut regions = Vec::new();
    hit_regions.0.clear();
    for (
        entity,
        node,
        transform,
        action,
        visibility,
        label,
        children,
        clip,
        editable,
        read_mode,
        address,
        invite,
    ) in &nodes
    {
        if !layer_visible(&ui, entity, &parents) {
            continue;
        }
        if visibility.is_some_and(|v| !v.get()) {
            continue;
        }
        let size = node.size() * node.inverse_scale_factor();
        if size.min_element() <= 0. {
            continue;
        }
        if clip.is_some_and(|clip| !clip.contains_point(transform.translation)) {
            continue;
        }
        let center = transform.translation * node.inverse_scale_factor();
        let start = center - size * 0.5;
        let label = label
            .map(|v| v.0.clone())
            .or_else(|| {
                children.and_then(|children| {
                    children
                        .into_iter()
                        .find_map(|child| texts.get(*child).ok().map(|(_, v, ..)| v.0.clone()))
                })
            })
            .unwrap_or_default();
        let action_name = action.map(|a| a.0.to_string()).unwrap_or_default();
        let capture = action.is_some_and(|a| a.0["kind"] == "capture");
        let field = if address {
            Some("address")
        } else if invite {
            Some("invite")
        } else {
            None
        };
        let value = editable.map(|v| v.value().to_string());
        let selection = editable.map(|editable| {
            let range = editable.editor().raw_selection().text_range();
            let value = value.as_deref().unwrap_or("");
            [
                value
                    .get(..range.start)
                    .unwrap_or(value)
                    .encode_utf16()
                    .count(),
                value
                    .get(..range.end)
                    .unwrap_or(value)
                    .encode_utf16()
                    .count(),
            ]
        });
        let read_only =
            read_mode.is_some_and(|mode| *mode != bevy::text::TextReadWriteMode::Editable);

        hit_regions.0.push(Rect::from_corners(start, start + size));
        regions.push(json!({"x":start.x,"y":start.y,"width":size.x,"height":size.y,"capture":capture,"label":label,"action":action_name,"field":field,"value":value,"selection":selection,"readOnly":read_only}));
    }
    if time.elapsed_secs() - *last_emit < 0.1 {
        return;
    }
    *last_emit = time.elapsed_secs();
    let drawn_texts = texts
        .iter()
        .filter_map(|(entity, text, node, transform, visibility, clip)| {
            if !layer_visible(&ui, entity, &parents) {
                return None;
            }
            if text.0.is_empty()
                || node.size().min_element() <= 0.
                || visibility.is_some_and(|v| !v.get())
                || clip.is_some_and(|v| !v.contains_point(transform.translation))
            {
                return None;
            }
            let size = node.size() * node.inverse_scale_factor();
            let start = transform.translation * node.inverse_scale_factor() - size * 0.5;
            Some(json!({"text":text.0,"x":start.x,"y":start.y,"width":size.x,"height":size.y}))
        })
        .collect::<Vec<_>>();
    let drawn_images=image_nodes.iter().filter_map(|(entity,image,node,transform,visibility,clip)|{
        if !layer_visible(&ui,entity,&parents){return None;}
        if node.size().min_element()<=0.||visibility.is_some_and(|v|!v.get())||clip.is_some_and(|v|!v.contains_point(transform.translation)){return None;}
        let size=node.size()*node.inverse_scale_factor();let start=transform.translation*node.inverse_scale_factor()-size*0.5;
        Some(json!({"path":assets.get_path(image.image.id()).map(|p|p.to_string()),"loaded":images.get(image.image.id()).is_some(),"x":start.x,"y":start.y,"width":size.x,"height":size.y}))
    }).take(128).collect::<Vec<_>>();
    if gpu.1.load(Ordering::Relaxed) && !boot_ready.1 {
        boot_ready.1 = true;
        crate::platform::ui_event(
            "paket-bootstrap-error",
            &json!("The renderer could not prepare the interface. Retry loading the game.")
                .to_string(),
        );
    }
    if !boot_ready.1 {
        let content_ready = if let Some(scene) = press.as_ref().filter(|_| ui.mode == "ride") {
            scene.ready
        } else {
            !drawn_texts.is_empty()
        };
        let ready = content_ready && gpu.0.load(Ordering::Relaxed);
        boot_ready.0 = if ready {
            boot_ready.0.saturating_add(1)
        } else {
            0
        };
        if boot_ready.0 >= 3 {
            boot_ready.1 = true;
            crate::platform::ui_event("paket-ui-ready", "true");
        }
    }
    let drawn_markers = marker_nodes
        .iter()
        .filter_map(
            |(entity, mark, node, transform, visibility, clip, background, text_color)| {
                if !layer_visible(&ui, entity, &parents)
                    || node.size().min_element() <= 0.
                    || visibility.is_some_and(|v| !v.get())
                    || clip.is_some_and(|v| !v.contains_point(transform.translation))
                {
                    return None;
                }
                let size = node.size() * node.inverse_scale_factor();
                let start = transform.translation * node.inverse_scale_factor() - size * 0.5;
                let color = background
                    .map(|v| v.0)
                    .or_else(|| text_color.map(|v| v.0))
                    .map(|v| {
                        let c = v.to_srgba();
                        [c.red, c.green, c.blue, c.alpha]
                    });
                Some(
                    json!({"kind":mark.kind,"id":mark.id,"slot":mark.slot,"color":color,
            "x":start.x,"y":start.y,"width":size.x,"height":size.y}),
                )
            },
        )
        .collect::<Vec<_>>();
    let drawn_panels = cards
        .iter()
        .filter_map(|(card, node, transform)| {
            if ui.mode != "ride" || node.size().min_element() <= 0. {
                return None;
            }
            let kind = match card {
                HudCard::Route => "route",
                HudCard::Instruction => "instructions",
                HudCard::Radar => "radar",
            };
            let size = node.size() * node.inverse_scale_factor();
            let start = transform.translation * node.inverse_scale_factor() - size * 0.5;
            Some(json!({"kind":kind,"x":start.x,"y":start.y,"width":size.x,"height":size.y}))
        })
        .collect::<Vec<_>>();
    let value=json!({"captureAllowed":ui.mode=="ride"&&ui.value("paket-hud")["ready"]==true&&ui.panel.is_empty()&&ui.value("paket-hud")["interaction"]!=true,"regions":regions,"mode":ui.mode,"panel":ui.panel,"status":ui.value("paket-services")["status"],"texts":drawn_texts,"images":drawn_images,"markers":drawn_markers,"panels":drawn_panels}).to_string();
    if *last != value {
        *last = value.clone();
        crate::platform::ui_event("paket-ui-regions", &value);
    }
}

fn photo(mut photo: ResMut<LastPhoto>, mut images: ResMut<Assets<Image>>) {
    if let Some(image) = PHOTO.lock().unwrap().take() {
        photo.0 = Some(images.add(image));
    }
}

fn settings_labels(
    ui: Res<Ui>,
    mut commands: Commands,
    buttons: Query<(Entity, &Action, &AccessibleLabel, &Children)>,
    mut labels: Query<&mut Text>,
) {
    for (entity, action, label, children) in &buttons {
        let title = match action.0["kind"].as_str() {
            Some("sound") => format!("Sound: {}", if ui.sound { "on" } else { "off" }),
            Some("public") => format!("Public mesh: {}", if ui.public { "on" } else { "off" }),
            Some("location") => if ui.value("paket-services")["locationPending"] == true {
                "Locating…"
            } else {
                "My location"
            }
            .into(),
            Some("search") => if ui.value("paket-services")["searchPending"] == true {
                "Searching…"
            } else {
                "Find address or place"
            }
            .into(),
            Some("refresh") => if ui.value("paket-services")["directoryPending"] == true {
                "Refreshing…"
            } else {
                "Refresh"
            }
            .into(),
            _ => continue,
        };
        if label.0 != title {
            commands
                .entity(entity)
                .insert(AccessibleLabel::new(title.clone()));
            for child in children {
                if let Ok(mut text) = labels.get_mut(*child) {
                    text.0 = title.clone();
                }
            }
        }
    }
}
fn portrait(parent: &mut ChildSpawnerCommands, seed: u32) {
    let skin = [
        Color::srgb(0.82, 0.61, 0.47),
        Color::srgb(0.72, 0.47, 0.33),
        Color::srgb(0.52, 0.33, 0.22),
        Color::srgb(0.91, 0.70, 0.57),
    ][seed as usize % 4];
    let hair = [
        Color::srgb(0.16, 0.14, 0.11),
        Color::srgb(0.29, 0.21, 0.16),
        Color::srgb(0.66, 0.46, 0.30),
        Color::srgb(0.52, 0.47, 0.38),
    ][(seed as usize >> 3) % 4];
    parent
        .spawn((
            Node {
                width: Val::Px(96.),
                height: Val::Px(110.),
                position_type: PositionType::Relative,
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(Color::srgb(0.22, 0.33, 0.26)),
        ))
        .with_children(|p| {
            for (left, top, width, height, color, rounded) in [
                (10., 80., 76., 40., ORANGE, false),
                (27., 18., 44., 65., skin, true),
                (25., 12., 48., 22., hair, true),
                (34., 44., 6., 5., INK, false),
                (60., 44., 6., 5., INK, false),
                (42., 68., 17., 3., INK, false),
            ] {
                p.spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(left),
                        top: Val::Px(top),
                        width: Val::Px(width),
                        height: Val::Px(height),
                        border_radius: if rounded {
                            BorderRadius::MAX
                        } else {
                            BorderRadius::ZERO
                        },
                        ..default()
                    },
                    BackgroundColor(color),
                ));
            }
        });
}

fn peer_color(slot: usize) -> Color {
    let hues = [
        195., 305., 145., 265., 55., 220., 335., 175., 285., 105., 240., 315., 75., 205., 155.,
        255., 355.,
    ];
    Color::hsl(hues[slot % hues.len()], 0.78, 0.70)
}

fn line(parent: &mut ChildSpawnerCommands, a: Vec2, b: Vec2, color: Color, width: f32) {
    let delta = b - a;
    parent.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px((a.x + b.x) * 0.5 - delta.length() * 0.5),
            top: Val::Px((a.y + b.y) * 0.5 - width * 0.5),
            width: Val::Px(delta.length()),
            height: Val::Px(width.max(1.)),
            ..default()
        },
        UiTransform {
            rotation: Rot2::radians(delta.y.atan2(delta.x)),
            ..default()
        },
        BackgroundColor(color),
    ));
}

fn layer_visible(
    ui: &Ui,
    mut entity: Entity,
    parents: &Query<(Option<&ChildOf>, Option<&Layer>)>,
) -> bool {
    let top = if ui.mode == "ride" && !ui.value("paket-hud").is_object() {
        Layer::Loading
    } else if !ui.panel.is_empty() {
        Layer::Dialog
    } else if ui.value("paket-hud")["interaction"] == true {
        Layer::Intercom
    } else if ui.welcome && ui.mode != "ride" {
        Layer::Welcome
    } else if ui.mode == "ride" {
        Layer::Hud
    } else {
        Layer::Lobby
    };
    for _ in 0..12 {
        let Ok((parent, layer)) = parents.get(entity) else {
            return false;
        };
        if let Some(layer) = layer {
            return *layer == top
                || (*layer == Layer::Header && top != Layer::Loading && !ui.welcome);
        }
        let Some(parent) = parent else {
            return false;
        };
        entity = parent.parent();
    }
    false
}

fn shield(
    regions: Res<HitRegions>,
    windows: Query<&Window>,
    mut mouse: ResMut<ButtonInput<MouseButton>>,
) {
    if let Some(cursor) = windows.single().ok().and_then(Window::cursor_position) {
        if regions.0.iter().any(|rect| rect.contains(cursor)) {
            mouse.clear_just_pressed(MouseButton::Left);
        }
    }
}
fn report_ui_gpu(cache: Res<bevy::render::render_resource::PipelineCache>, gpu: Res<UiGpu>) {
    use bevy::render::render_resource::{CachedPipelineState, PipelineDescriptor};
    let waiting: std::collections::HashSet<_> = cache.waiting_pipelines().collect();
    let mut ready = false;
    let mut failed = false;
    // Street shaders compile independently. Only the UI shader gates the menu.
    for (index, pipeline) in cache.pipelines().enumerate() {
        let PipelineDescriptor::RenderPipelineDescriptor(descriptor) = &pipeline.descriptor else {
            continue;
        };
        if descriptor.label.as_deref() != Some("ui_pipeline") {
            continue;
        }
        ready |= matches!(pipeline.state, CachedPipelineState::Ok(_));
        failed |=
            !waiting.contains(&index) && matches!(pipeline.state, CachedPipelineState::Err(_));
    }
    gpu.0.store(ready, Ordering::Relaxed);
    gpu.1.store(failed, Ordering::Relaxed);
}

fn set_sensitivity(ui: &mut Ui, controls: &mut crate::actors::Controls, value: f64) {
    if value.is_finite() {
        controls.sensitivity = value.clamp(0.0008, 0.006) as f32;
        ui.sensitivity = f64::from(controls.sensitivity);
    }
}

fn utf16_byte_offset(value: &str, index: usize) -> usize {
    let mut units = 0;
    for (byte, ch) in value.char_indices() {
        if units >= index || units + ch.len_utf16() > index {
            return byte;
        }
        units += ch.len_utf16();
    }
    value.len()
}

#[cfg(test)]
mod marker_tests {
    use super::*;

    #[test]
    fn every_admitted_mesh_slot_has_a_distinct_stable_marker_color() {
        let colors: Vec<_> = (0..17).map(peer_color).collect();
        for (slot, color) in colors.iter().enumerate() {
            assert_eq!(*color, peer_color(slot));
            for other in &colors[..slot] {
                assert_ne!(color, other);
            }
        }
        assert_eq!(peer_color(17), colors[0]);
    }
}
