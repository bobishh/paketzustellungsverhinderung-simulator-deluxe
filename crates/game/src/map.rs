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
    ui_widgets::{Button, TextInput},
    window::PrimaryWindow,
};
use paket_geo::{FIXTURE_LAT, FIXTURE_LON, LatLon, MAUSOHRWEG};
use paket_session::{Discovery, SharedStreet, invite_link, invite_origin};

use crate::{Mode, SessionResult, actors::Role, connect_session, start_ride};

const TILE: f64 = 256.0;
const TOOLBAR: f32 = if cfg!(target_arch = "wasm32") {
    0.0
} else {
    74.0
};
const SIDEBAR: f32 = if cfg!(target_arch = "wasm32") {
    360.0
} else {
    328.0
};
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
pub struct MapStatus;
#[derive(Component)]
pub struct LocationLabel;
#[derive(Component)]
pub struct NearbyLabel;
#[derive(Component)]
pub struct InviteInput;
#[derive(Component)]
pub struct RideSlot(usize);

#[derive(Clone, Copy, Component)]
pub enum Action {
    ZoomIn,
    ZoomOut,
    Select(LatLon),
    Role(Role),
    New,
    Join,
    Copy,
    Start,
    Solo,
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
        link: Option<String>,
        receiver: Mutex<mpsc::Receiver<SessionResult>>,
    },
    Ready {
        session: SharedStreet,
        link: String,
    },
}

impl Map {
    pub fn new(cache: PathBuf, offline: bool, center: LatLon) -> Self {
        let zoom = 16;
        let (tx, rx) = mpsc::channel();
        Self {
            center: project(center, zoom),
            zoom,
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
            connection: Connection::Idle,
            status: "",
            role: Role::Courier,
        }
    }

    pub(crate) fn with_role(mut self, role: Role) -> Self {
        self.role = role;
        self
    }

    fn point(&self) -> LatLon {
        self.exact_point
            .unwrap_or_else(|| unproject(self.center, self.zoom))
    }

    fn select(&mut self, point: LatLon) {
        self.center = project(point, self.zoom);
        self.exact_point = Some(point);
    }

    fn move_center(&mut self, delta: DVec2) {
        if delta != DVec2::ZERO {
            self.center += delta;
            self.exact_point = None;
        }
    }

    fn connect(&mut self, link: Option<String>) -> Result<(), String> {
        let origin = match &link {
            Some(link) => invite_origin(link)?.into(),
            None => self.point(),
        };
        self.select(origin);
        self.status = if link.is_some() {
            "Connecting to mesh…"
        } else {
            "Creating mesh…"
        };
        self.connection = Connection::Pending {
            origin,
            receiver: Mutex::new(connect_session(origin, link.clone())),
            link,
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
        Node {
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
        (true, false) => Color::srgb(0.69, 0.30, 0.11),
        (true, true) => Color::srgb(0.80, 0.37, 0.15),
        (false, false) => Color::srgb(0.19, 0.27, 0.26),
        (false, true) => Color::srgb(0.26, 0.37, 0.35),
    }
}

pub fn setup(mut commands: Commands) {
    crate::platform::ui_event("paket-mode", "\"map\"");
    let muted = Color::srgb(0.64, 0.74, 0.71);
    commands.spawn((Camera2d, MapUi));
    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..default()
            },
            BackgroundColor(Color::srgb(0.08, 0.13, 0.13)),
            MapUi,
        ))
        .with_children(|root| {
            root.spawn(Node {
                flex_grow: 1.0,
                min_width: Val::Px(0.0),
                flex_direction: FlexDirection::Column,
                ..default()
            })
            .with_children(|column| {
                column.spawn((
                    Node {
                        flex_grow: 1.0,
                        overflow: Overflow::clip(),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.17, 0.24, 0.23)),
                    Viewport,
                    children![(
                        Node {
                            position_type: PositionType::Absolute,
                            left: Val::Percent(50.0),
                            top: Val::Percent(50.0),
                            width: Val::Px(34.0),
                            height: Val::Px(34.0),
                            border: UiRect::all(Val::Px(2.0)),
                            border_radius: BorderRadius::MAX,
                            ..default()
                        },
                        UiTransform::from_translation(Val2::percent(-50.0, -50.0)),
                        BorderColor::all(Color::srgb(1.0, 0.45, 0.19)),
                        ZIndex(1),
                    )],
                ));
                column.spawn((
                    if cfg!(target_arch = "wasm32") {
                        Visibility::Hidden
                    } else {
                        Visibility::Inherited
                    },
                    Node {
                        height: Val::Px(TOOLBAR),
                        flex_shrink: 0.0,
                        padding: UiRect::all(Val::Px(12.0)),
                        column_gap: Val::Px(8.0),
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    children![
                        button(Action::ZoomOut, "−", false),
                        button(Action::ZoomIn, "+", false),
                        button(Action::Select(CITIES[1].1), "New York", false),
                        button(Action::Select(KREUZBERG), "Kreuzberg", false),
                        label("© OpenStreetMap contributors", 12.0, muted),
                    ],
                ));
            });
            root.spawn((
                if cfg!(target_arch = "wasm32") {
                    Visibility::Hidden
                } else {
                    Visibility::Inherited
                },
                Node {
                    width: Val::Px(SIDEBAR),
                    flex_shrink: 0.0,
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(Val::Px(16.0)),
                    row_gap: Val::Px(8.0),
                    overflow: Overflow::scroll_y(),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.10, 0.16, 0.16)),
            ))
            .with_children(|panel| {
                panel.spawn(label("Choose your shift", 28.0, Color::WHITE));
                for (name, point) in CITIES {
                    panel.spawn(button(Action::Select(*point), name, false));
                }
                panel.spawn(label(
                    "Click or drag the map to choose a start.",
                    14.0,
                    muted,
                ));
                panel.spawn((label("", 16.0, Color::WHITE), LocationLabel));
                panel.spawn((
                    Node {
                        column_gap: Val::Px(8.0),
                        ..default()
                    },
                    children![
                        button(Action::Role(Role::Courier), "Courier", true),
                        button(Action::Role(Role::Dog), "Dog", false)
                    ],
                ));

                panel.spawn((
                    Node {
                        column_gap: Val::Px(8.0),
                        ..default()
                    },
                    children![
                        button(Action::New, "With friends", true),
                        button(Action::Solo, "Play solo", false)
                    ],
                ));
                panel.spawn(label("Friend’s invite link", 14.0, muted));
                panel.spawn((
                    TextInput,
                    InviteInput,
                    EditableText::new(""),
                    TextFont {
                        font_size: FontSize::Px(15.0),
                        ..default()
                    },
                    TextColor(Color::WHITE),
                    Node {
                        width: Val::Percent(100.0),
                        height: Val::Px(35.0),
                        padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
                        overflow: Overflow::clip(),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.06, 0.10, 0.10)),
                ));
                panel.spawn((
                    Node {
                        column_gap: Val::Px(8.0),
                        ..default()
                    },
                    children![
                        button(Action::Join, "Join", false),
                        button(Action::Copy, "Copy link", false),
                        button(Action::Start, "Start ride", true)
                    ],
                ));
                panel.spawn((
                    label("", 14.0, Color::srgb(1.0, 0.70, 0.47)),
                    Node {
                        min_height: Val::Px(34.0),
                        ..default()
                    },
                    MapStatus,
                ));
                panel.spawn(label("Friends nearby", 18.0, Color::WHITE));
                panel.spawn((label("", 14.0, muted), NearbyLabel));
                for index in 0..RIDE_SLOTS {
                    panel.spawn((
                        Button,
                        Hovered::default(),
                        Action::Nearby(index),
                        Node {
                            display: Display::None,
                            padding: UiRect::all(Val::Px(8.0)),
                            ..default()
                        },
                        BackgroundColor(button_color(false, false)),
                        children![(label("", 15.0, Color::WHITE), RideSlot(index))],
                    ));
                }
                panel.spawn((
                    Node {
                        column_gap: Val::Px(8.0),
                        ..default()
                    },
                    children![
                        button(Action::Page(-1), "Previous", false),
                        button(Action::Page(1), "Next", false),
                    ],
                ));
            });
        });
}

fn viewport_size(window: &Window) -> Vec2 {
    Vec2::new(
        (window.width() - SIDEBAR).max(0.0),
        (window.height() - TOOLBAR).max(0.0),
    )
}

pub fn input(
    mut commands: Commands,
    mut map: ResMut<Map>,
    windows: Query<&Window, With<PrimaryWindow>>,
    buttons: Res<ButtonInput<MouseButton>>,
    motion: Res<AccumulatedMouseMotion>,
    scroll: Res<AccumulatedMouseScroll>,
    actions: Query<&Action, Added<bevy::ui::Pressed>>,
    mut invites: Query<&mut EditableText, With<InviteInput>>,
    mut next: ResMut<NextState<Mode>>,
) {
    let window = windows.single().ok();
    let cursor = window.and_then(Window::cursor_position);
    let over_map = cursor
        .zip(window)
        .is_some_and(|(cursor, window)| cursor.cmplt(viewport_size(window)).all());
    let initial_point = map.point();
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

    let mut requested: Vec<Action> = actions.iter().copied().collect();
    let external: Vec<serde_json::Value> =
        serde_json::from_str(&crate::platform::ui_commands()).unwrap_or_default();
    for command in external {
        match command["action"].as_str() {
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
            Some("new") => requested.push(Action::New),
            Some("solo") => requested.push(Action::Solo),
            Some("start") => requested.push(Action::Start),
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
        match *action {
            Action::ZoomIn => map.zoom(1),
            Action::ZoomOut => map.zoom(-1),
            Action::Select(point) => map.select(point),
            Action::Role(role) => map.role = role,
            Action::New => {
                *invite = EditableText::new("");
                let _ = map.connect(None);
                return;
            }
            Action::Join => {
                let link = invite.value().to_string().trim().to_owned();
                if let Connection::Ready { link: hosted, .. } = &map.connection
                    && hosted == &link
                {
                    map.status = "Mesh ready. Start ride when you're ready.";
                } else if map.connect(Some(link)).is_err() {
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
                    let _ = map.connect(Some(link));
                }
                return;
            }
            Action::Copy => {
                let link = invite.value().to_string();
                map.status = if invite_origin(&link).is_err() {
                    "Create a mesh or paste an invite first."
                } else if crate::platform::copy_text(&link) {
                    "Invite copied."
                } else {
                    "Clipboard unavailable."
                };
            }
            Action::Solo | Action::Start => {
                let origin = map.point();
                let (origin, session, link) = match (&map.connection, action) {
                    (_, Action::Solo) => (origin, None, None),
                    (Connection::Idle, _) => {
                        map.status = "Create or join a mesh first.";
                        return;
                    }
                    (Connection::Pending { .. }, _) => return,
                    (Connection::Ready { .. }, _) => {
                        let Connection::Ready { session, link } =
                            std::mem::take(&mut map.connection)
                        else {
                            unreachable!();
                        };
                        (session.origin.into(), Some(session), Some(link))
                    }
                };
                start_ride(
                    &mut commands,
                    origin,
                    map.cache.with_file_name("osm"),
                    map.offline,
                    None,
                    session,
                    link,
                    map.role,
                );
                next.set(Mode::Ride);
                return;
            }
        }
    }
    let selected = map.point();
    if (selected.lat - initial_point.lat)
        .abs()
        .max((selected.lon - initial_point.lon).abs())
        > 1e-9
        && !matches!(map.connection, Connection::Idle)
    {
        map.connection = Connection::Idle;
        *invite = EditableText::new("");
        map.status = "Location changed. Create a mesh for this start.";
    }
}

pub fn update_lobby(
    mut map: ResMut<Map>,
    mut invites: Query<&mut EditableText, With<InviteInput>>,
    mut labels: Query<(
        &mut Text,
        Has<LocationLabel>,
        Has<MapStatus>,
        Has<NearbyLabel>,
        Option<&RideSlot>,
    )>,
    mut actions: Query<(&Action, &mut Node, &Hovered, &mut BackgroundColor)>,
) {
    let pending = match &map.connection {
        Connection::Pending {
            origin,
            link,
            receiver,
        } => receiver
            .lock()
            .ok()
            .and_then(|rx| match rx.try_recv() {
                Ok(result) => Some(result),
                Err(mpsc::TryRecvError::Disconnected) => Some(Err("session stopped".into())),
                Err(mpsc::TryRecvError::Empty) => None,
            })
            .map(|result| (*origin, link.clone(), result)),
        _ => None,
    };
    if let Some((origin, link, result)) = pending {
        match result {
            Ok(session) => {
                let link = link.unwrap_or_else(|| {
                    let _ = fs::write("invitation.json", &session.invitation);
                    invite_link(&session.invitation)
                });
                if let Ok(mut input) = invites.single_mut() {
                    *input = EditableText::new(&link);
                }
                map.select(origin);
                map.connection = Connection::Ready { session, link };
                map.status = "Mesh ready.";
            }
            Err(error) => {
                eprintln!("session: {error}");
                map.connection = Connection::Idle;
                map.status = "Could not connect. Check the invite or try again.";
            }
        }
    }
    let own = match &map.connection {
        Connection::Ready { session, .. } => Some(session.device.clone()),
        _ => None,
    };
    if let Some(discovery) = &mut map.discovery {
        map.nearby = discovery
            .poll()
            .into_iter()
            .filter(|(id, _)| Some(*id) != own.as_deref())
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
    crate::platform::ui_event("paket-lobby",&serde_json::json!({"lat":point.lat,"lon":point.lon,"label":location_name(point),"status":map.status,"ready":matches!(map.connection,Connection::Ready{..}),"pending":matches!(map.connection,Connection::Pending{..}),"link":match &map.connection {Connection::Ready{link,..}=>link.as_str(),_=>""}}).to_string());
    for (mut text, location, status, nearby, slot) in &mut labels {
        let value = if location {
            location_name(point)
        } else if status {
            map.status.into()
        } else if nearby {
            if map.discovery.is_none() {
                if cfg!(target_arch = "wasm32") {
                    "Share a ride link to invite friends.".into()
                } else {
                    "Local discovery unavailable.".into()
                }
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
            matches!(action, Action::New | Action::Start)
                || matches!(action, Action::Role(role) if *role == map.role),
            interaction.0,
        );
        if color.0 != value {
            color.0 = value;
        }
    }
}

fn location_name(point: LatLon) -> String {
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
    mut commands: Commands,
    mut map: ResMut<Map>,
    mut images: ResMut<Assets<Image>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    viewport: Query<Entity, With<Viewport>>,
    mut nodes: Query<(&mut Node, &mut ImageNode)>,
) {
    let (Ok(window), Ok(viewport)) = (windows.single(), viewport.single()) else {
        return;
    };
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

    let half = viewport_size(window).as_dvec2() / 2.0;
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
            world.entity_mut(entity).insert(bevy::ui::Pressed);
            app.update();
            app.world_mut()
                .entity_mut(entity)
                .remove::<bevy::ui::Pressed>();
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
            link: None,
            receiver: Mutex::new(rx),
        };
        press(&mut app, |action| matches!(action, Action::Start));
        assert!(matches!(
            app.world().resource::<Map>().connection,
            Connection::Pending { .. }
        ));
        tx.send(Err("test connection failure".into())).unwrap();
        app.update();
        assert_eq!(
            app.world().resource::<Map>().status,
            "Could not connect. Check the invite or try again."
        );
        press(&mut app, |action| matches!(action, Action::Role(Role::Dog)));
        press(&mut app, |action| matches!(action, Action::Solo));
        app.update();
        assert_eq!(*app.world().resource::<State<Mode>>().get(), Mode::Ride);
        assert_eq!(app.world().resource::<crate::Ride>().origin, CITIES[1].1);
        assert_eq!(app.world().resource::<crate::Ride>().role, Role::Dog);
    }
}
