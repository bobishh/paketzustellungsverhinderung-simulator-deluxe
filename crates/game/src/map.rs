use web_time::Instant;
mod cached;

use std::{
    collections::{HashMap, HashSet},
    fs,
    io::Read,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, mpsc},
    thread,
    time::Duration,
};

use bevy::{
    asset::RenderAssetUsages,
    image::{CompressedImageFormats, ImageSampler, ImageType},
    input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll},
    math::DVec2,
    picking::hover::Hovered,
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
    text::EditableText,
    ui_widgets::{Button, Checkbox, TextInput},
    window::PrimaryWindow,
};
use paket_geo::{FIXTURE_LAT, FIXTURE_LON, LatLon, MAUSOHRWEG};
use paket_session::{Discovery, invite_origin};

use crate::{Mode, RideStart, SessionResult, actors::Role, connect_session, start_ride};

const TILE: f64 = 256.0;
const TOOLBAR: f32 = 42.;
const SIDEBAR: f32 = 320.;
const RIDE_SLOTS: usize = 4;
pub(crate) const KREUZBERG: LatLon = LatLon {
    lat: FIXTURE_LAT,
    lon: FIXTURE_LON,
};

pub(crate) const CITIES: &[(&str, LatLon)] = &[
    ("Berlin · Kreuzberg", KREUZBERG),
    (
        "New York · East Village",
        LatLon {
            lat: 40.7265,
            lon: -73.9815,
        },
    ),
    (
        "São Paulo · Vila Madalena",
        LatLon {
            lat: -23.5505,
            lon: -46.6892,
        },
    ),
    (
        "Tokyo · Shimokitazawa",
        LatLon {
            lat: 35.6616,
            lon: 139.6682,
        },
    ),
    (
        "Cape Town · Gardens",
        LatLon {
            lat: -33.9335,
            lon: 18.4112,
        },
    ),
    (
        "Sydney · Surry Hills",
        LatLon {
            lat: -33.8845,
            lon: 151.2133,
        },
    ),
];

#[derive(Component)]
pub struct MapUi;

#[derive(Component)]
pub struct Viewport;
#[derive(Component)]
pub(super) struct MapLayout;
#[derive(Component)]
pub(super) struct MapPane;
#[derive(Component)]
pub(super) struct Sidebar;
#[derive(Component)]
pub(super) struct PublicCheck;

#[derive(Component)]
pub struct MapStatus;
#[derive(Component)]
pub struct LocationLabel;
#[derive(Component)]
pub(super) struct PickerHint;
#[derive(Component)]
pub struct NearbyLabel;
#[derive(Component)]
pub struct InviteInput;
#[derive(Component)]
pub(crate) struct AddressInput;
#[derive(Component)]
pub(super) struct SearchResults;
#[derive(Component)]
pub struct RideSlot(usize);

#[derive(Clone, Copy, Component)]
pub enum Action {
    ZoomIn,
    ZoomOut,
    Select(LatLon),
    Role(Role),
    Join,
    Start,
    Nearby(usize),
    Page(isize),
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
struct Tile {
    z: u8,
    x: i32,
    y: i32,
}

type TileResult = (Tile, Option<Image>);

#[derive(Resource)]
pub struct Map {
    center: DVec2,
    zoom: u8,
    raster_map: bool,
    cache: PathBuf,
    active: HashMap<(i32, i32), Entity>,
    images: HashMap<Tile, Handle<Image>>,
    failed: HashMap<Tile, Instant>,
    pending: HashSet<Tile>,
    tx: mpsc::Sender<TileResult>,
    rx: Mutex<mpsc::Receiver<TileResult>>,
    cached_map: Arc<cached::CachedMap>,
    offline: bool,
    drag_start: Option<Vec2>,
    dragged: bool,
    exact_point: Option<LatLon>,
    discovery: Option<Discovery>,
    nearby: Vec<(String, LatLon, String)>,
    page: usize,
    #[cfg(target_arch = "wasm32")]
    directory_requested: bool,
    connection: Connection,
    status: &'static str,
    role: Role,
}

#[derive(Default)]
enum Connection {
    #[default]
    Idle,
    Pending {
        origin: LatLon,
        receiver: Mutex<mpsc::Receiver<SessionResult>>,
    },
}

impl Map {
    pub fn new(cache: PathBuf, offline: bool, center: LatLon) -> Self {
        let zoom = 16;
        let (tx, rx) = mpsc::channel();
        Self {
            center: project(center, zoom),
            zoom,
            raster_map: true,
            cached_map: Arc::new(cached::CachedMap::read(&cache.with_file_name("osm"))),
            cache,
            active: HashMap::new(),
            images: HashMap::new(),
            failed: HashMap::new(),
            pending: HashSet::new(),
            tx,
            rx: Mutex::new(rx),
            offline,
            drag_start: None,
            dragged: false,
            exact_point: Some(center),
            discovery: Discovery::listen().ok(),
            nearby: Vec::new(),
            page: 0,
            #[cfg(target_arch = "wasm32")]
            directory_requested: false,
            connection: Connection::Idle,
            status: "",
            role: Role::Courier,
        }
    }

    pub(crate) fn with_role(mut self, role: Role) -> Self {
        self.role = role;
        self
    }

    pub(crate) fn point(&self) -> LatLon {
        self.exact_point
            .unwrap_or_else(|| unproject(self.center, self.zoom))
    }

    pub(crate) fn select(&mut self, point: LatLon) {
        self.center = project(point, self.zoom);
        self.exact_point = Some(point);
    }

    fn move_center(&mut self, delta: DVec2) {
        if delta != DVec2::ZERO {
            self.center += delta;
            self.exact_point = None;
        }
    }

    fn join(&mut self, link: String, listed: bool) -> Result<(), String> {
        let origin = invite_origin(&link)?.into();
        self.select(origin);
        self.status = "Connecting to shift…";
        self.connection = Connection::Pending {
            origin,
            receiver: Mutex::new(connect_session(origin, Some(link), listed)),
        };
        Ok(())
    }

    fn zoom(&mut self, delta: i8) {
        let next = self.zoom.saturating_add_signed(delta).clamp(3, 19);
        self.center *= 2f64.powi(i32::from(next) - i32::from(self.zoom));
        self.zoom = next;
    }
}

fn world_size(z: u8) -> f64 {
    TILE * f64::from(1u32 << z)
}

pub(crate) fn project(point: LatLon, z: u8) -> DVec2 {
    let size = world_size(z);
    let lat = point.lat.clamp(-85.0, 85.0).to_radians();
    DVec2::new(
        (point.lon + 180.0) / 360.0 * size,
        (1.0 - lat.tan().asinh() / std::f64::consts::PI) * 0.5 * size,
    )
}

pub(crate) fn unproject(point: DVec2, z: u8) -> LatLon {
    let size = world_size(z);
    LatLon {
        lon: point.x.rem_euclid(size) / size * 360.0 - 180.0,
        lat: ((1.0 - 2.0 * point.y / size) * std::f64::consts::PI)
            .sinh()
            .atan()
            .to_degrees()
            .clamp(-85.0, 85.0),
    }
}

fn label(value: impl Into<String>, size: f32, color: Color) -> impl Bundle {
    (
        Text::new(value),
        TextFont {
            font_size: FontSize::Px(size),
            ..default()
        },
        TextColor(color),
    )
}

fn button(action: Action, title: &str, primary: bool) -> impl Bundle {
    (
        Button,
        Hovered::default(),
        action,
        AccessibleLabel::new(title),
        Node {
            min_height: Val::Px(38.),
            flex_shrink: 0.,
            justify_content: JustifyContent::Center,
            padding: UiRect::axes(Val::Px(11.0), Val::Px(8.0)),
            ..default()
        },
        BackgroundColor(button_color(primary, false)),
        children![label(title, 15.0, Color::WHITE)],
    )
}

fn button_color(primary: bool, hovered: bool) -> Color {
    match (primary, hovered) {
        (true, false) => Color::srgb(0.85, 0.34, 0.09),
        (true, true) => Color::srgb(0.80, 0.37, 0.15),
        (false, false) => Color::srgb(0.15, 0.22, 0.18),
        (false, true) => Color::srgb(0.26, 0.37, 0.35),
    }
}

pub fn setup(mut commands: Commands) {
    crate::platform::ui_event("paket-mode", "\"map\"");
    let muted = Color::srgb(0.68, 0.76, 0.70);
    commands
        .spawn((
            MapUi,
            MapLayout,
            bevy::input_focus::tab_navigation::TabGroup::default(),
            crate::game_ui::Layer::Lobby,
            Node {
                width: Val::Percent(100.),
                height: Val::Percent(100.),
                padding: UiRect::top(Val::Px(46.)),
                ..default()
            },
        ))
        .with_children(|root| {
            root.spawn((
                MapPane,
                Node {
                    flex_grow: 1.,
                    min_width: Val::Px(0.),
                    flex_direction: FlexDirection::Column,
                    ..default()
                },
            ))
            .with_children(|column| {
                column
                    .spawn((
                        Viewport,
                        Node {
                            flex_grow: 1.,
                            min_height: Val::Px(0.),
                            overflow: Overflow::clip(),
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.17, 0.24, 0.23)),
                    ))
                    .with_children(|viewport| {
                        viewport.spawn((
                            Node {
                                position_type: PositionType::Absolute,
                                left: Val::Percent(50.),
                                top: Val::Percent(50.),
                                width: Val::Px(24.),
                                height: Val::Px(24.),
                                border: UiRect::all(Val::Px(2.)),
                                ..default()
                            },
                            UiTransform::from_translation(Val2::percent(-50., -50.)),
                            BorderColor::all(Color::srgb(1., 0.45, 0.19)),
                            BackgroundColor(Color::srgba(0.05, 0.08, 0.08, 0.68)),
                            children![(
                                Node {
                                    width: px(6),
                                    height: px(6),
                                    margin: UiRect::all(Val::Auto),
                                    ..default()
                                },
                                BackgroundColor(Color::srgb(1., 0.45, 0.19)),
                            )],
                            ZIndex(2),
                        ));
                    });
                column
                    .spawn((
                        BackgroundColor(Color::srgb(0.055, 0.085, 0.08)),
                        Node {
                            height: Val::Px(TOOLBAR),
                            flex_shrink: 0.,
                            column_gap: Val::Px(6.),
                            align_items: AlignItems::Center,
                            padding: UiRect::axes(px(10.), px(0.)),
                            ..default()
                        },
                    ))
                    .with_children(|p| {
                        add_button(p, Action::ZoomOut, "−");
                        add_button(p, Action::ZoomIn, "+");
                        p.spawn(Node {
                            flex_grow: 1.,
                            min_width: px(0),
                            flex_direction: FlexDirection::Column,
                            overflow: Overflow::clip(),
                            ..default()
                        })
                        .with_children(|p| {
                            p.spawn((
                                crate::game_ui::Bound("map-loading"),
                                label("Loading map…", 11., muted),
                            ));
                            p.spawn((
                                crate::game_ui::Bound("map-attribution"),
                                label("© OpenStreetMap contributors", 10., muted),
                            ));
                        });
                        crate::game_ui::add_button(
                            p,
                            serde_json::json!({"action":"retry-map","map-toolbar":true}),
                            "Retry map",
                        );
                    });
            });
            root.spawn((
                Sidebar,
                Node {
                    width: Val::Px(SIDEBAR),
                    border: UiRect::left(px(1.)),
                    flex_shrink: 0.,
                    min_height: Val::Px(0.),
                    padding: UiRect::all(Val::Px(18.)),
                    row_gap: Val::Px(10.),
                    flex_direction: FlexDirection::Column,
                    ..default()
                },
                BackgroundColor(Color::srgb(0.075, 0.115, 0.10)),
                BorderColor::all(Color::srgb(0.28, 0.37, 0.31)),
            ))
            .with_children(|p| {
                p.spawn((
                    LocationLabel,
                    label("Choose a neighbourhood", 20., Color::srgb(0.96, 0.94, 0.86)),
                    Node {
                        flex_shrink: 0.,
                        ..default()
                    },
                ));
                p.spawn((
                    bevy::ui_widgets::ScrollArea,
                    Node {
                        flex_grow: 1.,
                        flex_basis: Val::Px(0.),
                        min_height: Val::Px(0.),
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(6.),
                        overflow: Overflow::scroll_y(),
                        ..default()
                    },
                ))
                .with_children(|p| {
                    p.spawn((
                        PickerHint,
                        label("Click or drag the map to set your start.", 12., muted),
                    ));
                    p.spawn(label("Street address or place", 12., muted));
                    text_input(p, true);
                    p.spawn(Node {
                        column_gap: px(6),
                        flex_shrink: 0.,
                        ..default()
                    })
                    .with_children(|p| {
                        crate::game_ui::add_button(
                            p,
                            serde_json::json!({"action":"service","kind":"search"}),
                            "Find address or place",
                        );
                        crate::game_ui::add_button(
                            p,
                            serde_json::json!({"action":"service","kind":"location"}),
                            "My location",
                        );
                    });
                    p.spawn(label(
                        "Only your rounded neighbourhood is shared.",
                        10.,
                        muted,
                    ));
                    p.spawn((
                        SearchResults,
                        Node {
                            flex_direction: FlexDirection::Column,
                            row_gap: Val::Px(4.),
                            ..default()
                        },
                    ));
                    p.spawn(Node {
                        flex_wrap: FlexWrap::Wrap,
                        column_gap: Val::Px(5.),
                        row_gap: Val::Px(4.),
                        ..default()
                    })
                    .with_children(|p| {
                        for (name, point) in CITIES {
                            add_button(
                                p,
                                Action::Select(*point),
                                name.split('·').next().unwrap_or(name).trim(),
                            );
                        }
                    });
                    p.spawn(label("Friend’s invitation", 13., muted));
                    text_input(p, false);
                    p.spawn(Node {
                        column_gap: px(6.),
                        flex_shrink: 0.,
                        ..default()
                    })
                    .with_children(|p| {
                        add_button(p, Action::Join, "Join shift");
                        crate::game_ui::add_button(
                            p,
                            serde_json::json!({"action":"service","kind":"resume"}),
                            "Return to your mesh",
                        );
                    });
                    p.spawn(label(
                        if cfg!(target_arch = "wasm32") {
                            "Public shifts"
                        } else {
                            "Nearby shifts"
                        },
                        16.,
                        Color::WHITE,
                    ));
                    #[cfg(target_arch = "wasm32")]
                    crate::game_ui::add_button(
                        p,
                        serde_json::json!({"action":"service","kind":"refresh"}),
                        "Refresh",
                    );
                    p.spawn((NearbyLabel, label("", 12., muted)));
                    for index in 0..RIDE_SLOTS {
                        p.spawn((
                            Button,
                            Hovered::default(),
                            Action::Nearby(index),
                            Node {
                                display: Display::None,
                                padding: UiRect::all(Val::Px(7.)),
                                ..default()
                            },
                            BackgroundColor(button_color(false, false)),
                            children![(label("", 13., Color::WHITE), RideSlot(index))],
                        ))
                        .observe(activated);
                    }
                    p.spawn(Node {
                        column_gap: Val::Px(6.),
                        ..default()
                    })
                    .with_children(|p| {
                        add_button(p, Action::Page(-1), "Previous");
                        add_button(p, Action::Page(1), "Next");
                    });
                });
                p.spawn((
                    MapStatus,
                    label("", 11., Color::srgb(1., 0.70, 0.47)),
                    Node {
                        height: Val::Px(28.),
                        flex_shrink: 0.,
                        overflow: Overflow::clip(),
                        ..default()
                    },
                ));
                p.spawn(Node {
                    flex_direction: FlexDirection::Column,
                    flex_shrink: 0.,
                    row_gap: Val::Px(6.),
                    ..default()
                })
                .with_children(|p| {
                    p.spawn(Node {
                        column_gap: Val::Px(6.),
                        ..default()
                    })
                    .with_children(|p| {
                        add_button(p, Action::Role(Role::Courier), "Courier");
                        add_button(p, Action::Role(Role::Dog), "Dog");
                    });
                    p.spawn(Node {
                        column_gap: px(8),
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::SpaceBetween,
                        ..default()
                    })
                    .with_children(|p| {
                        p.spawn((
                            Checkbox,
                            AccessibleLabel::new("Make public"),
                            bevy::input_focus::tab_navigation::TabIndex(0),
                            Node {
                                min_height: px(38),
                                align_items: AlignItems::Center,
                                column_gap: px(6),
                                ..default()
                            },
                        ))
                        .observe(bevy::ui_widgets::checkbox_self_update)
                        .with_children(|p| {
                            p.spawn((
                                Node {
                                    width: px(17),
                                    height: px(17),
                                    border: UiRect::all(px(1)),
                                    align_items: AlignItems::Center,
                                    justify_content: JustifyContent::Center,
                                    ..default()
                                },
                                BorderColor::all(Color::srgb(0.60, 0.69, 0.62)),
                            ))
                            .with_child((
                                PublicCheck,
                                Visibility::Hidden,
                                Node {
                                    width: px(9),
                                    height: px(9),
                                    ..default()
                                },
                                BackgroundColor(Color::srgb(1., 0.55, 0.23)),
                            ));
                            p.spawn(label("Make public", 13., Color::WHITE));
                        });
                        add_button(p, Action::Start, "Start shift");
                    });
                });
            });
        });
}
fn text_input(parent: &mut ChildSpawnerCommands, address: bool) {
    let mut field = parent.spawn((
        TextInput,
        bevy::input_focus::tab_navigation::TabIndex(if address { 0 } else { 1 }),
        AccessibleLabel::new(if address {
            "Street address or place"
        } else {
            "Invitation link"
        }),
        EditableText::new(""),
        TextFont {
            font_size: FontSize::Px(14.),
            ..default()
        },
        TextColor(Color::WHITE),
        Node {
            width: Val::Percent(100.),
            height: Val::Px(36.),
            min_height: Val::Px(38.),
            flex_shrink: 0.,
            padding: UiRect::all(Val::Px(7.)),
            overflow: Overflow::clip(),
            ..default()
        },
        BackgroundColor(Color::srgb(0.055, 0.10, 0.075)),
    ));
    if address {
        field.insert(AddressInput);
    } else {
        field.insert(InviteInput);
    }
}
pub(crate) fn responsive(
    windows: Query<&Window, With<PrimaryWindow>>,
    checkboxes: Query<Has<bevy::ui::Checked>, With<Checkbox>>,
    mut check: Query<&mut Visibility, With<PublicCheck>>,
    mut nodes: Query<(
        &mut Node,
        Has<MapLayout>,
        Has<MapPane>,
        Has<Sidebar>,
        Has<PickerHint>,
    )>,
) {
    let Ok(window) = windows.single() else {
        return;
    };
    let portrait = window.width() < 600.;
    let header = crate::game_ui::header_height(window.width());
    for (mut node, layout, pane, sidebar, hint) in &mut nodes {
        if hint {
            node.display = if window.height() < 560. {
                Display::None
            } else {
                Display::Flex
            };
        }
        if layout {
            node.height = Val::Px(window.height());
            node.flex_direction = if portrait {
                FlexDirection::Column
            } else {
                FlexDirection::Row
            };
            node.padding.top = Val::Px(header);
        }
        if pane {
            node.width = if portrait {
                Val::Percent(100.)
            } else {
                Val::Auto
            };
            node.height = if portrait {
                Val::Px(portrait_map_height(window) + TOOLBAR)
            } else {
                Val::Px((window.height() - header).max(0.))
            };
            node.flex_grow = if portrait { 0. } else { 1. };
            node.flex_shrink = if portrait { 0. } else { 1. };
        }
        if sidebar {
            let short = window.height() < 560.;
            node.padding = UiRect::all(px(if short { 8. } else { 18. }));
            node.row_gap = px(if short { 4. } else { 10. });
            node.width = if portrait {
                Val::Percent(100.)
            } else {
                Val::Px(SIDEBAR)
            };
            node.height = Val::Px(
                (window.height()
                    - header
                    - if portrait {
                        portrait_map_height(window) + TOOLBAR
                    } else {
                        0.
                    })
                .max(0.),
            );
            node.flex_grow = 0.;
            node.flex_shrink = 0.;
            node.overflow = Overflow::clip();
        }
    }
    if let (Ok(checked), Ok(mut mark)) = (checkboxes.single(), check.single_mut()) {
        let visibility = if checked {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *mark != visibility {
            *mark = visibility;
        }
    }
}
fn portrait_map_height(window: &Window) -> f32 {
    ((window.height() - crate::game_ui::header_height(window.width())) * 0.30).clamp(140., 220.)
}
fn add_button(parent: &mut ChildSpawnerCommands, action: Action, title: &str) {
    parent
        .spawn(button(action, title, false))
        .observe(activated);
}

fn viewport_size(window: &Window) -> Vec2 {
    if window.width() < 600. {
        Vec2::new(window.width(), portrait_map_height(window))
    } else {
        Vec2::new(
            (window.width() - SIDEBAR).max(0.),
            (window.height() - TOOLBAR - crate::game_ui::header_height(window.width())).max(0.),
        )
    }
}

pub fn input(
    ui: Res<crate::game_ui::Ui>,
    mut commands: Commands,
    mut map: ResMut<Map>,
    windows: Query<&Window, With<PrimaryWindow>>,
    buttons: Res<ButtonInput<MouseButton>>,
    motion: Res<AccumulatedMouseMotion>,
    scroll: Res<AccumulatedMouseScroll>,
    actions: Query<(Entity, &Action), With<crate::game_ui::Activated>>,
    mut invites: Query<&mut EditableText, With<InviteInput>>,
    mut next: ResMut<NextState<Mode>>,
    public: Query<Has<bevy::ui::Checked>, With<Checkbox>>,
) {
    let window = windows.single().ok();
    let cursor = window.and_then(Window::cursor_position).map(|v| {
        v - Vec2::new(
            0.,
            window
                .map(|w| crate::game_ui::header_height(w.width()))
                .unwrap_or(46.),
        )
    });
    let over_map = cursor.zip(window).is_some_and(|(cursor, window)| {
        cursor.cmpge(Vec2::ZERO).all() && cursor.cmplt(viewport_size(window)).all()
    });
    let connecting = matches!(map.connection, Connection::Pending { .. });
    let over_map = over_map && !connecting && !ui.panel_open();
    if connecting {
        map.drag_start = None;
    }
    if buttons.just_pressed(MouseButton::Left) && over_map {
        map.drag_start = cursor;
        map.dragged = false;
    }
    if let Some(start) = map.drag_start {
        if buttons.pressed(MouseButton::Left)
            && cursor.is_some_and(|point| point.distance(start) > 3.0)
        {
            map.dragged = true;
        }
        if buttons.pressed(MouseButton::Left) && map.dragged {
            map.move_center(-motion.delta.as_dvec2());
        }
        if buttons.just_released(MouseButton::Left) {
            if !map.dragged
                && over_map
                && let Some(window) = window
            {
                map.move_center((start - viewport_size(window) * 0.5).as_dvec2());
            }
            map.drag_start = None;
        }
    }
    if over_map && scroll.delta.y.abs() > 0.01 {
        map.zoom(if scroll.delta.y > 0.0 { 1 } else { -1 });
    }
    let size = world_size(map.zoom);
    map.center.x = map.center.x.rem_euclid(size);
    map.center.y = map.center.y.clamp(0.0, size - 1.0);
    let Ok(mut invite) = invites.single_mut() else {
        return;
    };

    let mut requested: Vec<Action> = actions.iter().map(|(_, action)| *action).collect();
    for (entity, _) in &actions {
        commands
            .entity(entity)
            .remove::<crate::game_ui::Activated>();
    }
    let external: Vec<serde_json::Value> =
        serde_json::from_str(&crate::platform::ui_commands()).unwrap_or_default();
    for command in external {
        match command["action"].as_str() {
            Some("retry-map") => map.failed.clear(),
            Some("vector-map") => map.raster_map = false,
            Some("raster-map") => map.raster_map = true,
            Some("select") => {
                if let (Some(lat), Some(lon)) = (command["lat"].as_f64(), command["lon"].as_f64()) {
                    if lat.is_finite() && lon.is_finite() && lat.abs() <= 85. && lon.abs() <= 180. {
                        requested.push(Action::Select(LatLon { lat, lon }));
                    }
                }
            }
            Some("role") => requested.push(Action::Role(if command["role"] == "dog" {
                Role::Dog
            } else {
                Role::Courier
            })),
            Some("zoom-in") => requested.push(Action::ZoomIn),
            Some("zoom-out") => requested.push(Action::ZoomOut),
            Some("join") => {
                *invite = EditableText::new(command["link"].as_str().unwrap_or(""));
                requested.push(Action::Join);
            }
            _ => {}
        }
    }
    for action in &requested {
        if connecting {
            continue;
        }
        match *action {
            Action::ZoomIn => map.zoom(1),
            Action::ZoomOut => map.zoom(-1),
            Action::Select(point) => {
                map.status = "";
                crate::platform::service(&serde_json::json!({"kind":"selection"}));
                map.select(point);
            }
            Action::Role(role) => map.role = role,
            Action::Join => {
                if map
                    .join(
                        invite.value().to_string().trim().to_owned(),
                        public.single().unwrap_or(false),
                    )
                    .is_err()
                {
                    map.status = "Paste a valid Paket invite link.";
                }
                return;
            }
            Action::Page(delta) => {
                map.page = map.page.saturating_add_signed(delta);
            }
            Action::Nearby(index) => {
                if let Some((link, _, _)) = map.nearby.get(map.page * RIDE_SLOTS + index).cloned() {
                    *invite = EditableText::new(&link);
                    if map.join(link, false).is_err() {
                        map.status = "This invitation is invalid. Refresh the public list.";
                    }
                }
                return;
            }
            Action::Start => {
                let session = if map.offline {
                    RideStart::Local
                } else {
                    RideStart::Host {
                        listed: public.single().unwrap_or(false),
                    }
                };
                start_ride(
                    &mut commands,
                    map.point(),
                    map.cache.with_file_name("osm"),
                    map.offline,
                    session,
                    map.role,
                );
                next.set(Mode::Ride);
                return;
            }
        }
    }
}

pub fn update_lobby(
    ui: Res<crate::game_ui::Ui>,
    mut commands: Commands,
    results: Query<(Entity, Option<&Children>), With<SearchResults>>,
    mut previous_results: Local<String>,
    mut map: ResMut<Map>,
    mut next: ResMut<NextState<Mode>>,
    mut labels: Query<(
        &mut Text,
        Has<LocationLabel>,
        Has<MapStatus>,
        Has<NearbyLabel>,
        Option<&RideSlot>,
    )>,
    mut actions: Query<(&Action, &mut Node, &Hovered, &mut BackgroundColor)>,
) {
    #[cfg(target_arch = "wasm32")]
    if !map.directory_requested && !ui.welcome_open() {
        map.directory_requested = true;
        crate::platform::service(&serde_json::json!({"kind":"refresh"}));
    }
    let services = ui.value("paket-services");
    let search = services["places"].to_string();
    if search != *previous_results {
        *previous_results = search;
        for (entity, children) in &results {
            if let Some(children) = children {
                for child in children {
                    commands.entity(*child).despawn();
                }
            }
            commands.entity(entity).with_children(|p| {
                for place in services["places"].as_array().into_iter().flatten().take(5) {
                    if let (Some(lat), Some(lon)) = (place["lat"].as_f64(), place["lon"].as_f64()) {
                        add_button(
                            p,
                            Action::Select(LatLon { lat, lon }),
                            place["name"].as_str().unwrap_or("Place"),
                        );
                    }
                }
            });
        }
    }
    #[cfg(target_arch = "wasm32")]
    if let Some(meshes) = services["meshes"].as_array() {
        map.nearby = meshes
            .iter()
            .filter_map(|v| {
                Some((
                    v["link"].as_str()?.into(),
                    LatLon {
                        lat: v["lat"].as_f64()?,
                        lon: v["lon"].as_f64()?,
                    },
                    v["name"].as_str().unwrap_or("Mesh").into(),
                ))
            })
            .collect();
    }
    let pending = match &map.connection {
        Connection::Pending { origin, receiver } => receiver
            .lock()
            .ok()
            .and_then(|rx| match rx.try_recv() {
                Ok(result) => Some(result),
                Err(mpsc::TryRecvError::Disconnected) => Some(Err("session stopped".into())),
                Err(mpsc::TryRecvError::Empty) => None,
            })
            .map(|result| (*origin, result)),
        Connection::Idle => None,
    };
    if let Some((origin, result)) = pending {
        map.connection = Connection::Idle;
        match result {
            Ok(session) => {
                start_ride(
                    &mut commands,
                    origin,
                    map.cache.with_file_name("osm"),
                    map.offline,
                    RideStart::Joined(session),
                    map.role,
                );
                next.set(Mode::Ride);
                return;
            }
            Err(error) => {
                eprintln!("session: {error}");
                map.status = "Could not join. Keep the host’s game open and try again.";
            }
        }
    }
    if let Some(discovery) = &mut map.discovery {
        map.nearby = discovery
            .poll()
            .into_iter()
            .map(|(id, ride)| {
                (
                    ride.link.clone(),
                    ride.origin.into(),
                    id.chars().take(6).collect(),
                )
            })
            .collect();
    }
    let last_page = map.nearby.len().saturating_sub(1) / RIDE_SLOTS;
    map.page = map.page.min(last_page);
    let first = map.page * RIDE_SLOTS;
    let point = map.point();
    crate::platform::ui_event("paket-lobby",&serde_json::json!({"lat":point.lat,"lon":point.lon,"zoom":map.zoom,"label":location_name(point),"status":map.status,"pending":matches!(map.connection,Connection::Pending{..})}).to_string());
    for (mut text, location, status, nearby, slot) in &mut labels {
        let value = if location {
            location_name(point)
        } else if status {
            if map.status.is_empty() {
                services["status"].as_str().unwrap_or("").into()
            } else {
                map.status.into()
            }
        } else if nearby {
            if cfg!(target_arch = "wasm32") {
                services["directoryStatus"].as_str().unwrap_or("").into()
            } else if map.discovery.is_none() {
                "Local discovery unavailable.".into()
            } else if map.nearby.is_empty() {
                "No rides on this network yet.".into()
            } else if last_page > 0 {
                format!(
                    "{}–{} of {}",
                    first + 1,
                    (first + RIDE_SLOTS).min(map.nearby.len()),
                    map.nearby.len()
                )
            } else {
                "Select a ride to connect.".into()
            }
        } else if let Some(slot) = slot {
            map.nearby
                .get(first + slot.0)
                .map_or_else(String::new, |(_, point, id)| {
                    format!("{} · {}", location_name(*point), id)
                })
        } else {
            continue;
        };
        if text.0 != value {
            text.0 = value;
        }
    }
    for (action, mut node, interaction, mut color) in &mut actions {
        let visible = match action {
            Action::Nearby(index) => first + index < map.nearby.len(),
            Action::Page(-1) => map.page > 0,
            Action::Page(1) => map.page < last_page,
            _ => true,
        };
        let display = if visible {
            Display::Flex
        } else {
            Display::None
        };
        if node.display != display {
            node.display = display;
        }
        let value = button_color(
            matches!(action, Action::Start)
                || matches!(action, Action::Role(role) if *role == map.role),
            interaction.0,
        );
        if color.0 != value {
            color.0 = value;
        }
    }
}

pub(crate) fn location_name(point: LatLon) -> String {
    let near = |preset: LatLon| {
        (point.lat - preset.lat).abs() < 1e-6 && (point.lon - preset.lon).abs() < 1e-6
    };
    if let Some((name, _)) = CITIES.iter().find(|(_, p)| near(*p)) {
        return (*name).into();
    }
    if near(MAUSOHRWEG) {
        "Mausohrweg · Berlin".into()
    } else if near(KREUZBERG) {
        "Kreuzberg · Berlin".into()
    } else {
        format!("{:.5}, {:.5}", point.lat, point.lon)
    }
}

pub fn tiles(
    mut loading: Local<Option<(Entity, bool, bool, bool)>>,
    #[cfg(target_arch = "wasm32")] mut browser_viewport: Local<
        Option<(Entity, Option<(Vec2, Vec2, DVec2, u8)>)>,
    >,
    #[cfg(target_arch = "wasm32")] ui: Res<crate::game_ui::Ui>,
    mut commands: Commands,
    mut map: ResMut<Map>,
    mut images: ResMut<Assets<Image>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut viewports: Query<
        (Entity, &mut Node, &mut BackgroundColor),
        (With<Viewport>, Without<ImageNode>),
    >,
    mut nodes: Query<(&mut Node, &mut ImageNode)>,
) {
    let (Ok(window), Ok((viewport, _node, mut background))) =
        (windows.single(), viewports.single_mut())
    else {
        return;
    };
    let size = viewport_size(window);
    #[cfg(target_arch = "wasm32")]
    {
        let visible = !ui.welcome_open();
        let offset = Vec2::new(0., crate::game_ui::header_height(window.width()));
        let view =
            (visible && size.x > 0. && size.y > 0.).then_some((offset, size, map.center, map.zoom));
        if *browser_viewport != Some((viewport, view)) {
            *browser_viewport = Some((viewport, view));
            let detail = view.map(|(offset, size, center, zoom)| {
                let point = unproject(center, zoom);
                serde_json::json!({"x":offset.x,"y":offset.y,"width":size.x,"height":size.y,
                    "lat":point.lat,"lon":point.lon,"zoom":zoom})
            });
            crate::platform::ui_event(
                "paket-map-viewport",
                &serde_json::to_string(&detail).unwrap(),
            );
        }
    }
    let fill = if map.raster_map {
        Color::srgb(0.17, 0.24, 0.23)
    } else {
        Color::NONE
    };
    if background.0 != fill {
        background.0 = fill;
        *loading = None;
    }
    if !map.raster_map {
        for (_, entity) in map.active.drain() {
            commands.entity(entity).despawn();
        }
        let state = (viewport, true, false, false);
        if *loading != Some(state) {
            *loading = Some(state);
            crate::platform::ui_event(
                "paket-map-loading",
                r#"{"ready":true,"pending":false,"failed":false,"vector":true}"#,
            );
        }
        return;
    }
    let finished: Vec<_> = map
        .rx
        .lock()
        .map(|rx| rx.try_iter().collect())
        .unwrap_or_default();
    for (key, image) in finished {
        map.pending.remove(&key);
        if let Some(image) = image {
            map.images.insert(key, images.add(image));
        } else {
            map.failed.insert(key, Instant::now());
        }
    }

    let half = size.as_dvec2() / 2.0;
    let top_left = map.center - half;
    let from = (top_left / TILE).floor().as_ivec2();
    let to = ((map.center + half) / TILE).floor().as_ivec2();
    let count = 1i32 << map.zoom;
    let mut wanted = HashSet::new();
    let mut visible_keys = HashSet::new();
    let mut missing = Vec::new();
    for y in from.y.max(0)..=to.y.min(count - 1) {
        for x in from.x..=to.x {
            let key = Tile {
                z: map.zoom,
                x: x.rem_euclid(count),
                y,
            };
            wanted.insert((x, y));
            visible_keys.insert(key);
            let offset = DVec2::new(f64::from(x), f64::from(y)) * TILE - top_left;
            if let Some(handle) = map.images.get(&key).cloned() {
                if let Some(&entity) = map.active.get(&(x, y)) {
                    if let Ok((mut node, mut image)) = nodes.get_mut(entity) {
                        node.left = Val::Px(offset.x as f32);
                        node.top = Val::Px(offset.y as f32);
                        image.image = handle;
                    }
                } else {
                    let entity = commands
                        .spawn((
                            Node {
                                position_type: PositionType::Absolute,
                                left: Val::Px(offset.x as f32),
                                top: Val::Px(offset.y as f32),
                                width: Val::Px(TILE as f32),
                                height: Val::Px(TILE as f32),
                                ..default()
                            },
                            ImageNode::new(handle),
                            ChildOf(viewport),
                        ))
                        .id();
                    map.active.insert((x, y), entity);
                }
            } else {
                if let Some(entity) = map.active.remove(&(x, y)) {
                    commands.entity(entity).despawn();
                }
                if !map.pending.contains(&key) && !map.failed.contains_key(&key) {
                    missing.push((
                        key,
                        (offset + DVec2::splat(TILE / 2.0) - half).length_squared(),
                    ));
                }
            }
        }
    }
    map.active.retain(|position, entity| {
        if wanted.contains(position) {
            true
        } else {
            commands.entity(*entity).despawn();
            false
        }
    });
    map.images.retain(|key, _| visible_keys.contains(key));
    map.failed
        .retain(|key, time| visible_keys.contains(key) && time.elapsed() < Duration::from_secs(15));
    missing.sort_by(|a, b| a.1.total_cmp(&b.1));
    for (key, _) in missing
        .into_iter()
        .take(4usize.saturating_sub(map.pending.len()))
    {
        if map.pending.insert(key) {
            let (cache, offline, tx) = (map.cache.clone(), map.offline, map.tx.clone());
            let cached_map = Arc::clone(&map.cached_map);
            #[cfg(target_arch = "wasm32")]
            wasm_bindgen_futures::spawn_local(async move {
                let image = crate::platform::fetch_bytes(&format!(
                    "https://tile.openstreetmap.org/{}/{}/{}.png",
                    key.z, key.x, key.y
                ))
                .await
                .ok()
                .and_then(|bytes| {
                    Image::from_buffer(
                        &bytes,
                        ImageType::Extension("png"),
                        CompressedImageFormats::NONE,
                        true,
                        ImageSampler::linear(),
                        RenderAssetUsages::default(),
                    )
                    .ok()
                });
                let _ = tx.send((key, image));
            });
            #[cfg(not(target_arch = "wasm32"))]
            thread::spawn(move || {
                let image = load(&cache, key, offline)
                    .and_then(|bytes| {
                        Image::from_buffer(
                            &bytes,
                            ImageType::Extension("png"),
                            CompressedImageFormats::NONE,
                            true,
                            ImageSampler::linear(),
                            RenderAssetUsages::default(),
                        )
                        .ok()
                    })
                    .or_else(|| {
                        let pixels = cached_map.raster(key)?;
                        let mut image = Image::new(
                            Extent3d {
                                width: TILE as u32,
                                height: TILE as u32,
                                depth_or_array_layers: 1,
                            },
                            TextureDimension::D2,
                            pixels,
                            TextureFormat::Rgba8UnormSrgb,
                            RenderAssetUsages::default(),
                        );
                        image.sampler = ImageSampler::linear();
                        Some(image)
                    });
                let _ = tx.send((key, image));
            });
        }
    }

    let ready = visible_keys.iter().any(|key| map.images.contains_key(key));
    let pending = visible_keys
        .iter()
        .any(|key| !map.images.contains_key(key) && !map.failed.contains_key(key));
    let failed = visible_keys.iter().any(|key| map.failed.contains_key(key));
    let state = (viewport, ready, pending, failed);
    if *loading != Some(state) {
        *loading = Some(state);
        crate::platform::ui_event(
            "paket-map-loading",
            &serde_json::json!({"ready":ready,"pending":pending,"failed":failed}).to_string(),
        );
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn load(cache: &Path, tile: Tile, offline: bool) -> Option<Vec<u8>> {
    let path = cache
        .join(tile.z.to_string())
        .join(tile.x.to_string())
        .join(format!("{}.png", tile.y));
    let cached = fs::read(&path).ok();
    let fresh = fs::metadata(&path)
        .ok()
        .and_then(|meta| meta.modified().ok())
        .and_then(|time| time.elapsed().ok())
        .is_some_and(|age| age < Duration::from_secs(7 * 86400));
    if offline || (fresh && cached.is_some()) {
        return cached;
    }
    let url = format!(
        "https://tile.openstreetmap.org/{}/{}/{}.png",
        tile.z, tile.x, tile.y
    );
    let response = ureq::get(&url)
        .set(
            "User-Agent",
            concat!(
                "Paket/",
                env!("CARGO_PKG_VERSION"),
                " (native neighborhood selector)"
            ),
        )
        .timeout(Duration::from_secs(8))
        .call();
    let Ok(response) = response else {
        return cached;
    };
    let mut bytes = Vec::new();
    response
        .into_reader()
        .take(1_000_001)
        .read_to_end(&mut bytes)
        .ok()?;
    if bytes.len() > 1_000_000 || !bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return cached;
    }
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
        let temporary = path.with_extension("tmp");
        if fs::write(&temporary, &bytes).is_ok() {
            let _ = fs::rename(temporary, path);
        }
    }
    Some(bytes)
}

pub fn cleanup(mut commands: Commands, entities: Query<Entity, With<MapUi>>) {
    for entity in &entities {
        commands.entity(entity).despawn();
    }
    commands.remove_resource::<Map>();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn changing_zoom_keeps_the_selected_point() {
        let point = LatLon {
            lat: 52.4895,
            lon: 13.3908,
        };
        let mut map = Map::new(PathBuf::new(), true, point);
        map.zoom(5);
        let selected = unproject(map.center, map.zoom);
        assert!((selected.lat - point.lat).abs() < 1e-9);
        assert!((selected.lon - point.lon).abs() < 1e-9);
    }

    #[test]
    fn crossing_the_map_edge_wraps_longitude() {
        let point = unproject(DVec2::new(world_size(4) + 256.0, world_size(4) / 2.0), 4);
        assert_eq!(point.lat, 0.0);
        assert_eq!(point.lon, -157.5);
    }

    #[test]
    fn lobby_selects_location_and_keeps_pending_connection_until_its_result() {
        let mut app = App::new();
        app.add_plugins(bevy::state::app::StatesPlugin)
            .init_state::<Mode>()
            .init_resource::<crate::game_ui::Ui>()
            .init_resource::<ButtonInput<MouseButton>>()
            .init_resource::<AccumulatedMouseMotion>()
            .init_resource::<AccumulatedMouseScroll>()
            .insert_resource(Map::new(PathBuf::new(), true, KREUZBERG))
            .add_systems(Startup, setup)
            .add_systems(Update, (input, update_lobby).chain());
        app.update();

        fn press(app: &mut App, select: impl Fn(&Action) -> bool) {
            let world = app.world_mut();
            let mut query = world.query::<(Entity, &Action)>();
            let entity = query
                .iter(world)
                .find(|(_, action)| select(action))
                .unwrap()
                .0;
            world.trigger(bevy::ui_widgets::Activate { entity });
            app.update();
            app.world_mut()
                .entity_mut(entity)
                .remove::<crate::game_ui::Activated>();
        }

        press(
            &mut app,
            |action| matches!(action, Action::Select(point) if *point == CITIES[1].1),
        );
        assert_eq!(app.world().resource::<Map>().point(), CITIES[1].1);
        press(&mut app, |action| matches!(action, Action::Join));
        assert_eq!(
            app.world().resource::<Map>().status,
            "Paste a valid Paket invite link."
        );

        let (tx, rx) = mpsc::channel();
        app.world_mut().resource_mut::<Map>().connection = Connection::Pending {
            origin: CITIES[1].1,
            receiver: Mutex::new(rx),
        };
        press(&mut app, |action| matches!(action, Action::Start));
        press(
            &mut app,
            |action| matches!(action, Action::Select(point) if *point == CITIES[2].1),
        );
        assert_eq!(app.world().resource::<Map>().point(), CITIES[1].1);
        assert!(matches!(
            app.world().resource::<Map>().connection,
            Connection::Pending { .. }
        ));
        tx.send(Err("test connection failure".into())).unwrap();
        app.update();
        assert_eq!(
            app.world().resource::<Map>().status,
            "Could not join. Keep the host’s game open and try again."
        );
        press(&mut app, |action| matches!(action, Action::Role(Role::Dog)));
        press(&mut app, |action| matches!(action, Action::Start));
        app.update();
        assert_eq!(*app.world().resource::<State<Mode>>().get(), Mode::Ride);
        assert_eq!(app.world().resource::<crate::Ride>().origin, CITIES[1].1);
        assert_eq!(app.world().resource::<crate::Ride>().role, Role::Dog);
    }
}

fn activated(
    event: On<bevy::ui_widgets::Activate>,
    actions: Query<&Action>,
    mut commands: Commands,
) {
    if let Ok(action) = actions.get(event.entity) {
        let value = match action {
            Action::Start => serde_json::json!({"action":"start"}),
            Action::Join | Action::Nearby(_) => serde_json::json!({"action":"join"}),
            Action::Role(role) => {
                serde_json::json!({"action":"role","role":if matches!(role,Role::Dog){"dog"}else{"courier"}})
            }
            _ => serde_json::json!({"action":"map"}),
        };
        crate::platform::ui_event("paket-ui-action", &value.to_string());
        commands
            .entity(event.entity)
            .insert(crate::game_ui::Activated);
    }
}
