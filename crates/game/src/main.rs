mod actors;
mod browser_ui;
mod buildings;
mod dogs;
mod jobs;
mod map;
mod platform;
mod press_scene;
mod roads;
mod shapes;
mod shift;
mod surfaces;
mod terrain;
mod traffic;
#[cfg(debug_assertions)]
mod visual_check;

use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::Duration;

use actors::{Models, ParkedVan, Player, Role};
use avian3d::prelude::*;
use bevy::asset::RenderAssetUsages;
use bevy::ecs::system::ParamSet;
use bevy::light::CascadeShadowConfigBuilder;
use bevy::light::{Atmosphere, SunDisk, atmosphere::ScatteringMedium};
use bevy::math::Affine2;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::pbr::{AtmosphereSettings, DistanceFog, FogFalloff};
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, PresentMode, PrimaryWindow};
use bevy::winit::{UpdateMode, WinitSettings};
use jobs::Jobs;
use paket_geo::{
    EastNorth, FIXTURE_LAT, FIXTURE_LON, LatLon, MAUSOHRWEG, Scene as MapScene, TILE_METERS,
    TileId, tile_center, tile_id,
};
use paket_session::{self, ActorPose, Avatar, Fact, SharedStreet, VanPose};

const KEEP_RADIUS: i32 = 3;
const FLOOR_METERS: f32 = 4_000.0;

fn main() {
    let args = Args::from_env();
    let cache = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../cache/osm");
    let _ = fs::create_dir_all(&cache);
    let direct = args.join.is_some() || args.random || args.lat.zip(args.lon).is_some();
    let assets = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets");
    let mut app = App::new();
    app.add_plugins((
        DefaultPlugins
            .set(AssetPlugin {
                file_path: if cfg!(target_arch = "wasm32") {
                    "assets".into()
                } else {
                    assets.display().to_string()
                },
                ..default()
            })
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title: "Paketzustellungsverhinderung Simulator Deluxe".into(),
                    #[cfg(target_arch = "wasm32")]
                    canvas: Some("#game".into()),
                    #[cfg(target_arch = "wasm32")]
                    fit_canvas_to_parent: true,
                    resize_constraints: bevy::window::WindowResizeConstraints {
                        min_width: 800.0,
                        min_height: 580.0,
                        ..default()
                    },
                    present_mode: PresentMode::AutoVsync,
                    ..default()
                }),
                ..default()
            }),
        PhysicsPlugins::default(),
    ))
    .insert_resource(WinitSettings {
        focused_mode: UpdateMode::reactive(Duration::from_secs_f64(1.0 / 60.0)),
        unfocused_mode: UpdateMode::reactive_low_power(Duration::from_secs_f64(1.0 / 30.0)),
    })
    .insert_resource(ClearColor(Color::srgb(0.53, 0.72, 0.9)))
    .insert_resource(GlobalAmbientLight {
        brightness: 330.0,
        ..default()
    })
    .insert_state(if direct { Mode::Ride } else { Mode::Map })
    .add_systems(OnEnter(Mode::Map), map::setup)
    .add_systems(OnExit(Mode::Map), map::cleanup)
    .add_systems(OnEnter(Mode::Travel), travel)
    .add_systems(
        Update,
        (map::input, map::update_lobby, map::tiles)
            .chain()
            .run_if(in_state(Mode::Map)),
    );
    // Bundle Latin, Cyrillic, kana and kanji instead of relying on system fonts:
    // WebAssembly text rendering cannot borrow the browser's font fallback.
    app.world_mut()
        .resource_mut::<Assets<Font>>()
        .insert(
            bevy::asset::AssetId::default(),
            Font::from_bytes(
                include_bytes!("../../../assets/fonts/NotoSansJP-Variable.ttf").to_vec(),
            ),
        )
        .expect("the default font asset can be replaced");
    install_ride(&mut app);
    if platform::arguments().iter().any(|a| a == "--press-shot") {
        press_scene::install(&mut app);
    }
    #[cfg(debug_assertions)]
    if let Ok(directory) = env::var("PAKET_VISUAL_CHECK") {
        visual_check::install(&mut app, directory.into());
    }
    if direct {
        let invite = args
            .join
            .as_ref()
            .map(|input| fs::read_to_string(input).unwrap_or_else(|_| input.clone()));
        let origin = invite
            .as_ref()
            .and_then(|input| paket_session::invite_origin(input).ok())
            .map(Into::into)
            .unwrap_or_else(|| resolve_origin(&args, &cache));
        let mut commands = app.world_mut().commands();
        let pending = connect_session(origin, invite.clone());
        start_ride(
            &mut commands,
            origin,
            cache,
            args.offline,
            Some(pending),
            None,
            invite,
            args.role,
        );
        app.world_mut().flush();
    } else {
        app.insert_resource(
            map::Map::new(cache.with_file_name("map"), args.offline, map::KREUZBERG)
                .with_role(args.role),
        );
    }
    app.run();
}

fn install_ride(app: &mut App) {
    app.init_resource::<actors::Controls>()
        .init_resource::<shift::Shift>()
        .add_systems(OnEnter(Mode::Ride), (setup, terrain::start))
        .add_systems(OnExit(Mode::Ride), cleanup_ride)
        .add_systems(
            FixedUpdate,
            (actors::drive, traffic::drive, stay_on_ground)
                .chain()
                .run_if(in_state(Mode::Ride)),
        )
        .add_systems(
            Update,
            (
                (
                    stream_tiles,
                    browser_ui::input,
                    (shift::tick, jobs::new_day).chain(),
                    jobs::service_update,
                    cursor_control,
                    actors::interact,
                    actors::camera_follow,
                    jobs::residents,
                    jobs::courier,
                    jobs::carry_visual,
                    jobs::dog,
                    sync_peers,
                    jobs::open_room,
                    hud,
                    guide,
                    action_prompt,
                )
                    .chain(),
                follow_floor,
                jobs::settle_thrown.before(jobs::residents),
                invite_control,
                return_to_map,
                dogs::walk,
                dogs::animate_tails,
                actors::animate_people,
                actors::animate_vehicles,
                terrain::finish,
                roads::rebuild,
                traffic::spawn,
                nearby_buildings,
                jobs::service_ui.after(jobs::courier),
                capture_frame,
                browser_ui::update.after(jobs::courier),
            )
                .run_if(in_state(Mode::Ride)),
        );
}

#[cfg(not(target_arch = "wasm32"))]
fn capture_frame(keys: Res<ButtonInput<KeyCode>>, mut commands: Commands) {
    if !keys.just_pressed(KeyCode::F12) {
        return;
    }
    let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../photos");
    if fs::create_dir_all(&directory).is_err() {
        return;
    }
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let path = directory.join(format!("street-{stamp}.png"));
    commands
        .spawn(bevy::render::view::screenshot::Screenshot::primary_window())
        .observe(bevy::render::view::screenshot::save_to_disk(path));
}

#[cfg(target_arch = "wasm32")]
fn capture_frame() {}

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq, States)]
enum Mode {
    #[default]
    Map,
    Ride,
    Travel,
}

#[derive(Resource)]
struct NextNeighbourhood(LatLon);

fn travel(
    mut commands: Commands,
    mut ride: ResMut<Ride>,
    destination: Res<NextNeighbourhood>,
    mut next: ResMut<NextState<Mode>>,
) {
    ride.origin = destination.0;
    ride.loaded.clear();
    ride.pending.clear();
    ride.retry.clear();
    ride.spawned.clear();
    ride.waiting_for_tile = false;
    commands.insert_resource(Jobs::default());
    commands.remove_resource::<NextNeighbourhood>();
    next.set(Mode::Ride);
}

type SessionResult = Result<SharedStreet, String>;

fn connect_session(origin: LatLon, invite: Option<String>) -> Receiver<SessionResult> {
    let (tx, rx) = mpsc::channel();
    #[cfg(not(target_arch = "wasm32"))]
    thread::spawn(move || {
        let result = match invite {
            Some(input) => paket_session::join(&input),
            None => paket_session::host(origin.into()),
        };
        let _ = tx.send(result);
    });
    #[cfg(target_arch = "wasm32")]
    wasm_bindgen_futures::spawn_local(async move {
        let _ = tx.send(paket_session::connect_browser(origin.into(), invite).await);
    });
    rx
}

fn start_ride(
    commands: &mut Commands,
    origin: LatLon,
    cache: PathBuf,
    offline: bool,
    pending: Option<Receiver<SessionResult>>,
    session: Option<SharedStreet>,
    invite: Option<String>,
    role: Role,
) {
    let invite = invite.map(|input| {
        if input.trim().starts_with('{') {
            paket_session::invite_link(input.trim())
        } else {
            input.trim().to_owned()
        }
    });
    let invite_status = if session.is_some() {
        "Copy invite link"
    } else if pending.is_some() {
        "Preparing invite…"
    } else {
        "Solo ride"
    };
    let guest = session
        .as_ref()
        .map_or(invite.is_some(), |session| !session.is_host);
    platform::ui_event("paket-mode", "\"ride\"");
    commands.init_resource::<Jobs>();
    commands.insert_resource(shift::Shift::new(guest));
    commands.insert_resource(Ride {
        origin,
        role,
        guest,
        session,
        connecting: pending.map(Mutex::new),
        invite,
        invite_status,
        loaded: HashMap::new(),
        pending: HashMap::new(),
        retry: HashMap::new(),
        spawned: HashMap::new(),
        cache,
        allow_network: !offline,
        pose_timer: 0.0,
        sequence: 0,
        waiting_for_tile: false,
        mouse_look: true,
    });
}

fn cleanup_ride(
    mut commands: Commands,
    continuing: Option<Res<NextNeighbourhood>>,
    mut cursor: Query<&mut CursorOptions, With<PrimaryWindow>>,
) {
    #[cfg(target_arch = "wasm32")]
    platform::capture_cursor(false);
    if let Ok(mut cursor) = cursor.single_mut() {
        cursor.visible = true;
        cursor.grab_mode = CursorGrabMode::None;
    }
    commands.remove_resource::<terrain::Landscape>();
    if continuing.is_none() {
        commands.remove_resource::<Ride>();
    }
    commands.remove_resource::<Models>();
    commands.remove_resource::<Jobs>();
    commands.remove_resource::<StreetMaterials>();
    commands.remove_resource::<roads::Roads>();
    commands.remove_resource::<traffic::TrafficWorld>();
}

fn cursor_control(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut ride: ResMut<Ride>,
    jobs: Res<Jobs>,
    mut window: Query<(&Window, &mut CursorOptions), With<PrimaryWindow>>,
) {
    if keys.just_pressed(KeyCode::Escape) {
        ride.mouse_look = false;
    }
    if mouse.just_pressed(MouseButton::Left) && !jobs.interaction_active() {
        ride.mouse_look = true;
    }
    if let Ok((window, mut cursor)) = window.single_mut() {
        let captured = window.focused && ride.mouse_look && !jobs.interaction_active();
        #[cfg(target_arch = "wasm32")]
        platform::capture_cursor(captured);
        #[cfg(target_arch = "wasm32")]
        let mode = CursorGrabMode::None; // The browser requires a fresh canvas click.
        #[cfg(not(target_arch = "wasm32"))]
        let mode = if captured {
            CursorGrabMode::Locked
        } else {
            CursorGrabMode::None
        };
        if cursor.grab_mode != mode {
            cursor.grab_mode = mode;
        }
        if cursor.visible == captured {
            cursor.visible = !captured;
        }
    }
}

#[derive(Component)]
struct MapButton;

fn return_to_map(
    mut commands: Commands,
    buttons: Query<(), (With<MapButton>, Added<bevy::ui::Pressed>)>,
    ride: Res<Ride>,
    mut next: ResMut<NextState<Mode>>,
) {
    if !buttons.is_empty() {
        change_location(&mut commands, &ride, &mut next);
    }
}

fn change_location(commands: &mut Commands, ride: &Ride, next: &mut NextState<Mode>) {
    // Every route to the selector must create its required resource before OnEnter.
    commands.insert_resource(
        map::Map::new(
            ride.cache.with_file_name("map"),
            !ride.allow_network,
            ride.origin,
        )
        .with_role(ride.role),
    );
    next.set(Mode::Map);
}

#[derive(Component)]
struct InviteButton;

#[derive(Component)]
struct InviteLabel;

#[derive(Component)]
struct ShareToggle;

#[derive(Component)]
struct SharePanel;

#[derive(Component)]
struct ShareLink;

fn invite_control(
    mut ride: ResMut<Ride>,
    buttons: Query<(), (With<InviteButton>, Added<bevy::ui::Pressed>)>,
    toggle: Query<(), (With<ShareToggle>, Added<bevy::ui::Pressed>)>,
    mut panel: Query<&mut Node, With<SharePanel>>,
    mut text: ParamSet<(
        Query<&mut Text, With<InviteLabel>>,
        Query<&mut Text, With<ShareLink>>,
    )>,
) {
    let result = ride
        .connecting
        .as_ref()
        .and_then(|rx| rx.lock().ok()?.try_recv().ok());
    if let Some(result) = result {
        ride.connecting = None;
        match result {
            Ok(session) => {
                if ride.invite.is_none() {
                    ride.invite = Some(paket_session::invite_link(&session.invitation));
                    let _ = fs::write("invitation.json", &session.invitation);
                }
                ride.session = Some(session);
                ride.invite_status = "Copy invite link";
            }
            Err(error) => {
                eprintln!("session: {error}");
                ride.invite_status = "Session unavailable";
            }
        }
    }
    if !buttons.is_empty() {
        if let Some(link) = &ride.invite {
            ride.invite_status = if platform::copy_text(link) {
                "Invite copied"
            } else {
                "Clipboard unavailable"
            };
        }
    }
    if !toggle.is_empty() {
        if let Ok(mut panel) = panel.single_mut() {
            panel.display = if panel.display == Display::None {
                Display::Flex
            } else {
                Display::None
            };
        }
    }
    if let Ok(mut text) = text.p0().single_mut() {
        if text.0 != ride.invite_status {
            **text = ride.invite_status.into();
        }
    }
    if let Some(link) = &ride.invite {
        if let Ok(mut text) = text.p1().single_mut() {
            if text.0 != *link {
                **text = link.clone();
            }
        }
    }
}

struct Args {
    role: Role,
    lat: Option<f64>,
    lon: Option<f64>,
    random: bool,
    offline: bool,
    join: Option<String>,
}

impl Args {
    fn from_env() -> Self {
        let mut args = Self {
            role: Role::Courier,
            lat: None,
            lon: None,
            random: false,
            offline: false,
            join: None,
        };
        let mut tokens = platform::arguments().into_iter();
        while let Some(token) = tokens.next() {
            match token.as_str() {
                "--lat" => args.lat = tokens.next().and_then(|value| value.parse().ok()),
                "--lon" => args.lon = tokens.next().and_then(|value| value.parse().ok()),
                "--random" => args.random = true,
                "--offline" => args.offline = true,
                "--dog" => args.role = Role::Dog,
                "--join" => args.join = tokens.next(),
                "--help" => {
                    println!(
                        "paket-game [--lat LAT --lon LON | --random] [--offline] [--dog] [--join LINK_OR_JSON_FILE]\n\
                         opens the OpenStreetMap area selector by default"
                    );
                    std::process::exit(0);
                }
                other => eprintln!("ignored argument {other}"),
            }
        }
        args
    }
}

#[derive(Resource)]
struct Ride {
    origin: LatLon,
    role: Role,
    guest: bool,
    session: Option<SharedStreet>,
    connecting: Option<Mutex<Receiver<SessionResult>>>,
    invite: Option<String>,
    invite_status: &'static str,
    loaded: HashMap<TileId, Vec<String>>,
    pending: HashMap<TileId, Mutex<Receiver<Result<MapScene, String>>>>,
    retry: HashMap<TileId, f32>,
    spawned: HashMap<String, (Entity, usize)>,
    cache: PathBuf,
    allow_network: bool,
    pose_timer: f32,
    sequence: u64,
    waiting_for_tile: bool,
    mouse_look: bool,
}

#[derive(Component, Clone)]
struct BuildingLink {
    id: String,
    threshold: Vec3,
    address: Option<String>,
    entrance: Vec3,
    outward: Vec3,
    accessory: bool,
}

#[derive(Component)]
struct RemoteActor {
    device: String,
    form: Avatar,
    position: Vec3,
    velocity: Vec3,
    rotation: Quat,
    sequence: u64,
    received: f32,
    parked: Option<VanPose>,
    visual: Option<Entity>,
    van_visual: Option<Entity>,
}

impl RemoteActor {
    fn van_position(&self, transform: &Transform) -> Option<Vec3> {
        if self.form == Avatar::Van {
            return Some(transform.translation);
        }
        self.parked
            .map(|van| Vec3::new(van.east, -0.04, -van.north))
    }
}

#[derive(Component)]
struct Hud;

#[derive(Component)]
struct CompassNeedle;

#[derive(Component)]
struct CompassDistance;

#[derive(Component)]
struct PhotoReticle;

#[derive(Component)]
struct ActionPrompt;

#[derive(Component)]
struct WorldFloor;

#[cfg(target_arch = "wasm32")]
fn resolve_origin(args: &Args, _: &std::path::Path) -> LatLon {
    args.lat
        .zip(args.lon)
        .map_or(MAUSOHRWEG, |(lat, lon)| LatLon { lat, lon })
}

#[cfg(not(target_arch = "wasm32"))]
fn resolve_origin(args: &Args, cache: &PathBuf) -> LatLon {
    if let (Some(lat), Some(lon)) = (args.lat, args.lon) {
        return LatLon { lat, lon };
    }
    if args.random && !args.offline {
        for index in 0..8 {
            let origin = jitter(index);
            if let Ok(scene) = paket_geo::load_tile(cache, origin, TileId { x: 0, y: 0 }, true) {
                if paket_geo::tile_is_playable(&scene) {
                    println!("random origin {:.5}, {:.5}", origin.lat, origin.lon);
                    return origin;
                }
            }
        }
        eprintln!("random search found no streets; using the bundled block");
    }
    LatLon {
        lat: FIXTURE_LAT,
        lon: FIXTURE_LON,
    }
}

fn jitter(index: u32) -> LatLon {
    let cities = paket_geo::seed_cities();
    let city = cities[index as usize % cities.len()];
    let salt = f64::from(index + 1);
    LatLon {
        lat: city.lat + (salt * 12.9898).sin() * 0.008,
        lon: city.lon + (salt * 78.233).cos() * 0.012,
    }
}

fn setup(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut scattering: ResMut<Assets<ScatteringMedium>>,
) {
    commands.insert_resource(roads::Roads::default());
    commands.insert_resource(traffic::TrafficWorld::new(&mut meshes, &mut materials));
    commands.insert_resource(Models::new(&mut meshes, &mut materials, &mut images));
    let earth = scattering.add(ScatteringMedium::earth(256, 256));
    commands.spawn((Atmosphere::earth(earth), DespawnOnExit(Mode::Ride)));
    commands.insert_resource(StreetMaterials {
        buildings: materials.add(surfaces::material(
            &mut images,
            surfaces::Surface::Plaster,
            Color::WHITE,
        )),
        asphalt: materials.add(surfaces::material(
            &mut images,
            surfaces::Surface::Asphalt,
            Color::WHITE,
        )),
        paving: materials.add(surfaces::material(
            &mut images,
            surfaces::Surface::Paving,
            Color::WHITE,
        )),
        gravel: materials.add(surfaces::material(
            &mut images,
            surfaces::Surface::Gravel,
            Color::WHITE,
        )),
        dogs: dogs::DogKit::new(&mut meshes, &mut materials, &mut images),
    });
    commands.spawn((
        DespawnOnExit(Mode::Ride),
        Camera3d::default(),
        actors::FollowCamera::default(),
        Msaa::Sample4,
        Projection::Perspective(PerspectiveProjection {
            fov: 64_f32.to_radians(),
            ..default()
        }),
        DistanceFog {
            color: Color::srgb(0.65, 0.73, 0.76),
            falloff: FogFalloff::ExponentialSquared { density: 0.0023 },
            ..default()
        },
        AtmosphereSettings::default(),
        Transform::from_xyz(0.0, 14.0, 22.0).looking_at(Vec3::new(0.0, 1.0, -6.0), Vec3::Y),
    ));
    commands.spawn((
        DespawnOnExit(Mode::Ride),
        DirectionalLight {
            illuminance: 22_000.0,
            color: Color::srgb(1.0, 0.94, 0.84),
            shadow_maps_enabled: true,
            ..default()
        },
        SunDisk::EARTH,
        CascadeShadowConfigBuilder {
            first_cascade_far_bound: 18.0,
            maximum_distance: 180.0,
            ..default()
        }
        .build(),
        Transform::from_xyz(40.0, 70.0, 24.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    commands.spawn((
        Mesh3d(meshes.add(shapes::lit(Mesh::from(Cuboid::new(
            FLOOR_METERS,
            4.0,
            FLOOR_METERS,
        ))))),
        MeshMaterial3d(materials.add(StandardMaterial {
            uv_transform: Affine2::from_scale(Vec2::splat(FLOOR_METERS / 3.0)),
            ..surfaces::material(&mut images, surfaces::Surface::Grass, Color::WHITE)
        })),
        Transform::from_xyz(0.0, -2.05, 0.0),
        RigidBody::Static,
        Collider::cuboid(FLOOR_METERS, 4.0, FLOOR_METERS),
        WorldFloor,
        DespawnOnExit(Mode::Ride),
    ));
    commands.spawn((
        Text::new("© OpenStreetMap contributors"),
        TextFont {
            font_size: FontSize::Px(16.0),
            ..default()
        },
        TextColor(Color::srgb(0.96, 0.95, 0.9)),
        BackgroundColor(Color::srgba(0.035, 0.055, 0.06, 0.86)),
        Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(16.0),
            left: Val::Px(16.0),
            padding: UiRect::all(Val::Px(10.0)),
            ..default()
        },
        Hud,
        if cfg!(target_arch = "wasm32") {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        },
        DespawnOnExit(Mode::Ride),
    ));
    commands.spawn((
        DespawnOnExit(Mode::Ride),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(12.0),
            width: Val::Percent(100.0),
            justify_content: JustifyContent::Center,
            ..default()
        },
        children![(
            Node {
                width: Val::Px(150.0),
                padding: UiRect::all(Val::Px(8.0)),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.05, 0.05, 0.05, 0.72)),
            children![
                (
                    Text::new("▲"),
                    TextFont {
                        font_size: FontSize::Px(52.0),
                        ..default()
                    },
                    TextColor(Color::srgb(1.0, 0.55, 0.08)),
                    UiTransform::default(),
                    CompassNeedle,
                ),
                (
                    Text::new("—"),
                    TextFont {
                        font_size: FontSize::Px(22.0),
                        ..default()
                    },
                    TextColor(Color::srgb(0.98, 0.96, 0.9)),
                    CompassDistance,
                ),
            ],
        )],
    ));
    let panel_color = BackgroundColor(Color::srgba(0.05, 0.08, 0.09, 0.93));
    commands.spawn((
        bevy::ui_widgets::Button,
        ShareToggle,
        DespawnOnExit(Mode::Ride),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(12.0),
            right: Val::Px(12.0),
            padding: UiRect::all(Val::Px(10.0)),
            ..default()
        },
        panel_color,
        children![Text::new("Share")],
    ));
    commands.spawn((
        SharePanel,
        DespawnOnExit(Mode::Ride),
        Node {
            display: Display::None,
            position_type: PositionType::Absolute,
            top: Val::Px(58.0),
            right: Val::Px(12.0),
            width: Val::Px(380.0),
            max_width: Val::Percent(90.0),
            padding: UiRect::all(Val::Px(12.0)),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(8.0),
            ..default()
        },
        panel_color,
        children![
            (
                Text::new(""),
                ShareLink,
                TextFont {
                    font_size: FontSize::Px(13.0),
                    ..default()
                },
                TextLayout::linebreak(bevy::text::LineBreak::AnyCharacter),
            ),
            (
                bevy::ui_widgets::Button,
                InviteButton,
                Node {
                    padding: UiRect::all(Val::Px(8.0)),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.24, 0.34, 0.35)),
                children![(Text::new("Preparing invite…"), InviteLabel)],
            )
        ],
    ));
    commands.spawn((
        bevy::ui_widgets::Button,
        MapButton,
        if cfg!(target_arch = "wasm32") {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        },
        DespawnOnExit(Mode::Ride),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(12.0),
            left: Val::Px(12.0),
            padding: UiRect::all(Val::Px(10.0)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.05, 0.08, 0.09, 0.85)),
        children![Text::new("Change neighbourhood")],
    ));
    commands.spawn((
        PhotoReticle,
        DespawnOnExit(Mode::Ride),
        Node {
            display: Display::None,
            position_type: PositionType::Absolute,
            top: Val::Percent(50.0),
            left: Val::Percent(50.0),
            ..default()
        },
        Text::new("[ + ]"),
        TextFont {
            font_size: FontSize::Px(24.0),
            ..default()
        },
        TextColor(Color::WHITE),
        UiTransform::from_translation(Val2::percent(-50.0, -50.0)),
    ));
    commands.spawn((
        ActionPrompt,
        if cfg!(target_arch = "wasm32") {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        },
        DespawnOnExit(Mode::Ride),
        Node {
            display: Display::None,
            position_type: PositionType::Absolute,
            top: Val::Percent(64.0),
            left: Val::Percent(50.0),
            max_width: Val::Px(620.0),
            padding: UiRect::axes(Val::Px(14.0), Val::Px(10.0)),
            ..default()
        },
        Text::new(""),
        TextFont {
            font_size: FontSize::Px(18.0),
            ..default()
        },
        TextColor(Color::WHITE),
        BackgroundColor(Color::srgba(0.035, 0.055, 0.06, 0.9)),
        UiTransform::from_translation(Val2::percent(-50.0, 0.0)),
    ));
}

#[derive(Resource)]
struct StreetMaterials {
    buildings: Handle<StandardMaterial>,
    asphalt: Handle<StandardMaterial>,
    paving: Handle<StandardMaterial>,
    gravel: Handle<StandardMaterial>,
    dogs: dogs::DogKit,
}

fn stream_tiles(
    mut commands: Commands,
    time: Res<Time>,
    mut ride: ResMut<Ride>,
    mut meshes: ResMut<Assets<Mesh>>,
    models: Res<Models>,
    street_materials: Res<StreetMaterials>,
    mut jobs: ResMut<Jobs>,
    mut roads: ResMut<roads::Roads>,
    player: Query<&Transform, With<Player>>,
) {
    let dt = time.delta_secs();
    for wait in ride.retry.values_mut() {
        *wait -= dt;
    }
    ride.retry.retain(|_, wait| *wait > 0.0);
    let player = player.single().ok();
    let here = player.map_or(Vec3::ZERO, |bike| bike.translation);
    let forward = player.map_or(Vec3::NEG_Z, |bike| bike.forward().as_vec3());
    let around = tile_id(east_north(here));
    let in_range =
        |tile: TileId| (tile.x - around.x).abs().max((tile.y - around.y).abs()) <= KEEP_RADIUS;
    let expired: Vec<_> = ride
        .loaded
        .keys()
        .copied()
        .filter(|tile| !in_range(*tile))
        .collect();
    for tile in expired {
        for id in ride.loaded.remove(&tile).unwrap_or_default() {
            if let Some((entity, count)) = ride.spawned.get_mut(&id) {
                *count -= 1;
                if *count == 0 {
                    commands.entity(*entity).despawn();
                    ride.spawned.remove(&id);
                    roads.remove(&id);
                }
            }
        }
    }
    ride.retry.retain(|tile, _| in_range(*tile));

    let mut wanted = Vec::new();
    for y in (around.y - 2)..=(around.y + 2) {
        for x in (around.x - 2)..=(around.x + 2) {
            let tile = TileId { x, y };
            let offset = to_world(tile_center(tile), here.y) - here;
            let edge = (offset.abs() - Vec3::splat(TILE_METERS as f32 * 0.5)).max(Vec3::ZERO);
            let near = edge.length() < 65.0;
            let distance = offset.length();
            let ahead = offset.dot(forward);
            if near || (player.is_some() && distance < 380.0 && ahead > distance * 0.5) {
                let priority = if tile == around {
                    -1000.0
                } else {
                    distance - ahead.max(0.0) * 0.55
                };
                wanted.push((tile, priority));
            }
        }
    }
    wanted.sort_by(|a, b| a.1.total_cmp(&b.1));
    for (tile, _) in wanted {
        if ride.pending.len() >= 1 {
            break;
        }
        if ride.loaded.contains_key(&tile)
            || ride.pending.contains_key(&tile)
            || ride.retry.contains_key(&tile)
        {
            continue;
        }
        let cache = ride.cache.clone();
        let origin = ride.origin;
        let network = ride.allow_network;
        let (tx, rx) = mpsc::channel();
        platform::load_scene(cache, origin, tile, network, tx);
        ride.pending.insert(tile, Mutex::new(rx));
    }
    let ready: Vec<(TileId, Result<MapScene, String>)> = ride
        .pending
        .iter()
        .filter_map(|(tile, rx)| {
            rx.lock()
                .ok()
                .and_then(|receiver| receiver.try_recv().ok())
                .map(|scene| (*tile, scene))
        })
        .collect();
    for (tile, scene) in ready {
        ride.pending.remove(&tile);
        if !in_range(tile) {
            continue;
        }
        let Ok(scene) = scene else {
            let message = scene.err().unwrap_or_else(|| "tile failed".into());
            eprintln!("osm tile {},{}: {message}", tile.x, tile.y);
            ride.retry.insert(tile, 12.0);
            continue;
        };
        println!(
            "osm tile {},{}  {} buildings  {} streets",
            tile.x,
            tile.y,
            scene.buildings.len(),
            scene.streets.len()
        );
        let origin_tile = TileId { x: 0, y: 0 };
        if tile == origin_tile || ride.loaded.contains_key(&origin_tile) {
            if !scene.buildings.is_empty() {
                let addresses = scene
                    .buildings
                    .iter()
                    .filter(|building| !building.accessory)
                    .map(building_link)
                    .collect();
                jobs.seed(addresses, &mut commands, &models);
            }
            if player.is_none() {
                let start = match ride.role {
                    Role::Dog => jobs.room.as_ref().map(jobs::Room::start),
                    Role::Courier => street_start(&scene, ride.origin, ride.guest),
                };
                if let Some((position, direction)) = start {
                    actors::spawn(
                        &mut commands,
                        &models,
                        &street_materials.dogs,
                        ride.role,
                        position,
                        direction,
                    );
                }
            }
        }
        let features = spawn_tile(
            &mut commands,
            &mut meshes,
            &street_materials,
            &models,
            &scene,
            &mut ride.spawned,
            &mut roads,
        );
        ride.loaded.insert(tile, features);
    }
}

fn east_north(translation: Vec3) -> EastNorth {
    EastNorth {
        east: f64::from(translation.x),
        north: f64::from(-translation.z),
    }
}

fn to_world(point: EastNorth, y: f32) -> Vec3 {
    Vec3::new(point.east as f32, y, -point.north as f32)
}

fn stay_on_ground(
    mut actor: Query<(
        &Player,
        &mut Transform,
        &mut LinearVelocity,
        &mut AngularVelocity,
    )>,
) {
    let Ok((player, mut transform, mut linear, mut angular)) = actor.single_mut() else {
        return;
    };
    if transform.translation.is_finite() && transform.translation.y >= 0.08 {
        return;
    }
    if !transform.translation.is_finite() {
        transform.translation = Vec3::ZERO;
    }
    transform.translation.y = actors::half_height(player.form) + 0.15;
    linear.0 = Vec3::ZERO;
    angular.0 = Vec3::ZERO;
}

fn sync_peers(
    mut commands: Commands,
    time: Res<Time>,
    clock: Res<Time<Virtual>>,
    mut ride: ResMut<Ride>,
    mut jobs: ResMut<Jobs>,
    mut shift: ResMut<shift::Shift>,
    models: Res<Models>,
    street: Res<StreetMaterials>,
    mut players: Query<(&mut Player, &Transform, &LinearVelocity), Without<RemoteActor>>,
    vans: Query<&Transform, (With<ParkedVan>, Without<RemoteActor>)>,
    mut remotes: Query<(Entity, &mut RemoteActor, &mut Transform), Without<Player>>,
) {
    if ride.session.is_none() {
        return;
    }
    ride.pose_timer += time.delta_secs();
    if ride.pose_timer >= 0.1 {
        ride.pose_timer %= 0.1;
        ride.sequence += 1;
        if let (Some(session), Ok((player, transform, velocity))) =
            (ride.session.as_ref(), players.single())
        {
            let parked = player.parked.and_then(|entity| vans.get(entity).ok());
            let _ = session.publish(Fact::Actor {
                origin: session.origin,
                device: session.device.clone(),
                sequence: ride.sequence,
                pose: actors::pose(player, transform, velocity, parked),
            });
        }
    }
    let Some(session) = &ride.session else {
        return;
    };
    let now = clock.elapsed_secs();
    let mut snapshots = HashMap::<String, RemoteActor>::new();
    for fact in session.poll() {
        let (device, sequence, pose) = match fact {
            Fact::Shift {
                device,
                day,
                elapsed_ms,
                difficulty,
                world,
                choices,
                remaining_ms,
                ..
            } => {
                if !session.is_host && device == session.host_device() {
                    shift.receive_snapshot(
                        day,
                        elapsed_ms,
                        difficulty,
                        world,
                        choices,
                        remaining_ms,
                    );
                }
                continue;
            }
            Fact::Ballot {
                device,
                day,
                sequence,
                courier,
                score,
                delivered,
                finished,
                choice,
                ..
            } => {
                shift.receive_ballot(
                    device, day, sequence, courier, score, delivered, finished, choice,
                );
                continue;
            }
            Fact::Actor {
                device,
                sequence,
                pose,
                ..
            } => (device, sequence, pose),
            Fact::Pose {
                device,
                sequence,
                east,
                north,
                up,
                heading,
                velocity_east,
                velocity_north,
                ..
            } => (
                device,
                sequence,
                ActorPose {
                    avatar: Avatar::Courier,
                    east,
                    north,
                    up: up - 0.7,
                    heading,
                    velocity_east,
                    velocity_north,
                    parked_van: None,
                },
            ),
            Fact::StreetAction {
                day,
                device,
                sequence,
                event,
                ..
            } => {
                if day != jobs.day() || day != shift.day() {
                    continue;
                }
                let player = players.single_mut().ok().map(|(player, _, _)| player);
                jobs.receive(
                    &session.device,
                    device,
                    sequence,
                    event,
                    now,
                    player.map(Mut::into_inner),
                );
                continue;
            }
            Fact::Delivery { .. } => continue,
        };
        let position = Vec3::new(pose.east, pose.up, -pose.north);
        let velocity = Vec3::new(pose.velocity_east, 0.0, -pose.velocity_north);
        if device == session.device
            || !position.is_finite()
            || !velocity.is_finite()
            || !pose.heading.is_finite()
        {
            continue;
        }
        if snapshots
            .get(&device)
            .is_some_and(|current| current.sequence > sequence)
        {
            continue;
        }
        let parked = pose
            .parked_van
            .filter(|van| van.east.is_finite() && van.north.is_finite() && van.heading.is_finite());
        snapshots.insert(
            device.clone(),
            RemoteActor {
                device,
                form: pose.avatar,
                position,
                velocity,
                rotation: Quat::from_rotation_y(pose.heading),
                sequence,
                received: now,
                parked,
                visual: None,
                van_visual: None,
            },
        );
    }
    let smoothing = 1.0 - (-14.0 * time.delta_secs()).exp();
    for (entity, mut remote, mut transform) in &mut remotes {
        if let Some(mut snapshot) = snapshots.remove(&remote.device)
            && (snapshot.sequence == 0 || snapshot.sequence > remote.sequence)
        {
            if remote.form == snapshot.form {
                snapshot.visual = remote.visual;
            } else if let Some(visual) = remote.visual {
                commands.entity(visual).despawn();
            }
            snapshot.van_visual = remote.van_visual;
            *remote = snapshot;
        }
        let age = now - remote.received;
        if age > 10.0 {
            commands.entity(entity).despawn();
            continue;
        }
        transform.translation = transform
            .translation
            .lerp(remote.position + remote.velocity * age.min(0.15), smoothing);
        transform.rotation = transform.rotation.slerp(remote.rotation, smoothing);
        remote_visuals(
            &mut commands,
            &models,
            &street.dogs,
            entity,
            &transform,
            &mut remote,
        );
    }
    for (_, mut remote) in snapshots {
        let transform = Transform::from_translation(remote.position).with_rotation(remote.rotation);
        let entity = commands
            .spawn((transform, Visibility::default(), DespawnOnExit(Mode::Ride)))
            .id();
        remote_visuals(
            &mut commands,
            &models,
            &street.dogs,
            entity,
            &transform,
            &mut remote,
        );
        commands.entity(entity).insert(remote);
    }
}

fn remote_visuals(
    commands: &mut Commands,
    models: &Models,
    dogs: &dogs::DogKit,
    entity: Entity,
    transform: &Transform,
    remote: &mut RemoteActor,
) {
    if remote.visual.is_none() {
        remote.visual = Some(models.avatar(commands, entity, remote.form, dogs, 0.0));
    }
    if let Some(van) = remote.parked {
        let visual = *remote
            .van_visual
            .get_or_insert_with(|| models.avatar(commands, entity, Avatar::Van, dogs, 0.0));
        let inverse = transform.rotation.inverse();
        let ground = Vec3::new(van.east, -0.04, -van.north);
        commands.entity(visual).insert(
            Transform::from_translation(inverse * (ground - transform.translation))
                .with_rotation(inverse * Quat::from_rotation_y(van.heading)),
        );
    } else if let Some(visual) = remote.van_visual.take() {
        commands.entity(visual).despawn();
    }
}

fn follow_floor(
    bike: Query<&Transform, With<Player>>,
    mut floor: Query<&mut Transform, (With<WorldFloor>, Without<Player>)>,
) {
    let (Ok(bike), Ok(mut floor)) = (bike.single(), floor.single_mut()) else {
        return;
    };
    let translation = Vec3::new(
        (bike.translation.x / 200.0).round() * 200.0,
        floor.translation.y,
        (bike.translation.z / 200.0).round() * 200.0,
    );
    if translation != floor.translation {
        floor.translation = translation;
    }
}

fn guide(
    jobs: Res<Jobs>,
    actors: Query<(&Player, &Transform)>,
    mut needle: Query<(&mut Text, &mut UiTransform), With<CompassNeedle>>,
    mut label: Query<&mut Text, (With<CompassDistance>, Without<CompassNeedle>)>,
    mut crosshair: Query<(&mut Node, &mut TextColor), With<PhotoReticle>>,
    vans: Query<&Transform, With<ParkedVan>>,
) {
    let Ok((player, actor)) = actors.single() else {
        return;
    };
    if let Ok((mut crosshair, mut color)) = crosshair.single_mut() {
        crosshair.display = if player.photo {
            Display::Flex
        } else {
            Display::None
        };
        color.0 = if jobs.photo_ready {
            Color::srgb(0.38, 1.0, 0.63)
        } else {
            Color::WHITE
        };
    }
    let (Ok((mut needle, mut spin)), Ok(mut label)) = (needle.single_mut(), label.single_mut())
    else {
        return;
    };
    let target = match player.form {
        Avatar::Dog => None,
        Avatar::Courier if jobs.needs_van() => player
            .parked
            .and_then(|entity| vans.get(entity).ok())
            .map(|van| van.translation),
        _ => jobs.current_target(),
    };
    if let Some(target) = target {
        let delta = (target - actor.translation).with_y(0.0);
        spin.rotation = Rot2::radians(actor.right().dot(delta).atan2(actor.forward().dot(delta)));
        needle.0 = "▲".into();
        label.0 = format!("{:.0} m", delta.length());
    } else {
        needle.0.clear();
        label.0.clear();
    }
}

fn action_prompt(
    jobs: Res<Jobs>,
    mut prompt: Query<(&mut Text, &mut Node, &mut TextColor), With<ActionPrompt>>,
) {
    let Ok((mut text, mut node, mut color)) = prompt.single_mut() else {
        return;
    };
    node.display = if jobs.prompt.is_empty() {
        Display::None
    } else {
        Display::Flex
    };
    if text.0 != jobs.prompt {
        text.0.clone_from(&jobs.prompt);
    }
    color.0 = if jobs.photo_ready {
        Color::srgb(0.38, 1.0, 0.63)
    } else if jobs.pursued {
        Color::srgb(1.0, 0.63, 0.27)
    } else {
        Color::srgb(0.96, 0.95, 0.9)
    };
}

fn hud(
    ride: Res<Ride>,
    jobs: Res<Jobs>,
    shift: Res<shift::Shift>,
    keys: Res<ButtonInput<KeyCode>>,
    mut text: Query<&mut Text, With<Hud>>,
    actor: Query<(&Player, &Transform, &LinearVelocity)>,
    peers: Query<(&RemoteActor, &Transform)>,
    vans: Query<&Transform, With<ParkedVan>>,
) {
    let Ok(mut text) = text.single_mut() else {
        return;
    };
    let mut display = format!("{}\n", shift.label());
    if let Ok((player, position, velocity)) = actor.single() {
        match player.form {
            Avatar::Courier => {
                if let Some(van) = player.parked.and_then(|entity| vans.get(entity).ok()) {
                    let distance =
                        actors::horizontal_distance(position.translation, van.translation);
                    display.push_str(&format!(
                        "Van {:.0} m | {}\n",
                        distance,
                        if distance <= 3.5 {
                            "V enter / E parcel"
                        } else {
                            "walk to the van"
                        }
                    ));
                }
                display.push_str(&jobs.manifest());
                if keys.pressed(KeyCode::F1) {
                    display.push_str("\nWASD move | Shift run | Ctrl crouch\nTab / Shift+Tab parcel | E deliver | F camera | click photo");
                }
            }
            Avatar::Van => {
                display.push_str(&format!(
                    "{:02.0} km/h | {}\n",
                    velocity.0.xz().length() * 3.6,
                    if velocity.0.dot(position.forward().as_vec3()) < -0.2 {
                        "REVERSE"
                    } else {
                        "DRIVE"
                    }
                ));
                display.push_str(&jobs.manifest());
                if keys.pressed(KeyCode::F1) {
                    display.push_str("\nW gas | S brake/reverse | A/D steer\nSpace handbrake | V exit | H call residents");
                }
            }
            Avatar::Dog => {
                display.push_str(jobs.dog_status(position.translation));
                if keys.pressed(KeyCode::F1) {
                    display.push_str("\nWASD move | Shift run | B alert residents");
                }
            }
        }
        let radar = jobs.radar(
            ride.session.as_ref().map(|session| session.device.as_str()),
            position,
            peers.iter().filter_map(|(peer, transform)| {
                Some((peer.device.clone(), peer.van_position(transform)?))
            }),
        );
        if !radar.is_empty() {
            display.push('\n');
            display.push_str(&radar);
        }
    }
    if !jobs.status.is_empty() {
        display.push('\n');
        display.push_str(&jobs.status);
    }
    if ride.waiting_for_tile {
        display.push_str("\nLoading the next streets...");
    }
    if ride.session.is_some() {
        display.push_str(&format!("\nPlayers: {}", 1 + peers.iter().count()));
    }
    display.push_str("\nHold F1: controls | Esc: cursor | M: map");
    if keys.pressed(KeyCode::F1) {
        display.push_str("\nMouse look | Wheel zoom | -/+ sensitivity | F12 screenshot");
    }
    display.push_str("\nOpenStreetMap contributors");
    let display = display
        .replace('·', "|")
        .replace('→', ">")
        .replace('–', "-")
        .replace('↑', "^")
        .replace('↓', "v")
        .replace('…', "...");
    if text.0 != display {
        text.0 = display;
    }
}

fn nearby_buildings(
    player: Query<&Transform, With<Player>>,
    mut buildings: Query<(&BuildingLink, &mut Visibility)>,
) {
    let Ok(player) = player.single() else {
        return;
    };
    for (building, mut visibility) in &mut buildings {
        let detail_distance = if building.accessory { 72.0 } else { 255.0 };
        let visible =
            actors::horizontal_distance(player.translation, building.entrance) < detail_distance;
        let target = if visible {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *visibility != target {
            *visibility = target;
        }
    }
}

fn building_link(building: &paket_geo::Building) -> BuildingLink {
    let (_, threshold, outward) = buildings::doorway(building);
    BuildingLink {
        id: building.id.clone(),
        threshold,
        address: building.address.clone(),
        entrance: threshold + outward * 1.25,
        outward,
        accessory: building.accessory,
    }
}

fn spawn_tile(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    street_materials: &StreetMaterials,
    models: &Models,
    scene: &MapScene,
    spawned: &mut HashMap<String, (Entity, usize)>,
    roads: &mut roads::Roads,
) -> Vec<String> {
    let mut features = Vec::new();
    for street in &scene.streets {
        features.push(street.id.clone());
        if let Some((_, references)) = spawned.get_mut(&street.id) {
            *references += 1;
            continue;
        }
        roads.insert(street);
        let road = commands
            .spawn((
                DespawnOnExit(Mode::Ride),
                Transform::IDENTITY,
                Visibility::default(),
            ))
            .id();
        street_materials.dogs.spawn(commands, street, road);
        spawned.insert(street.id.clone(), (road, 1));
    }
    for building in &scene.buildings {
        features.push(building.id.clone());
        if let Some((_, references)) = spawned.get_mut(&building.id) {
            *references += 1;
            continue;
        }
        let mesh = meshes.add(shapes::lit(buildings::mesh(building)));
        let collider = building_collider(building);
        let mut entity = commands.spawn((
            DespawnOnExit(Mode::Ride),
            Mesh3d(mesh),
            MeshMaterial3d(street_materials.buildings.clone()),
            RigidBody::Static,
            building_link(building),
        ));
        if let Some(collider) = collider {
            entity.insert(collider);
        }
        let entity = entity.id();
        if !building.accessory && !building.open_sided {
            jobs::spawn_door(commands, models, &building_link(building), entity);
        }
        spawned.insert(building.id.clone(), (entity, 1));
    }
    features
}

fn street_start(scene: &MapScene, origin: LatLon, guest: bool) -> Option<(Vec3, Vec3)> {
    let mausohrweg =
        (origin.lat - MAUSOHRWEG.lat).abs() < 1e-6 && (origin.lon - MAUSOHRWEG.lon).abs() < 1e-6;
    let segments: Vec<_> = scene
        .streets
        .iter()
        .filter(|street| street.spawnable)
        .flat_map(|street| {
            let preferred = mausohrweg && street.name.as_deref() == Some("Mausohrweg");
            street.centerline.windows(2).filter_map(move |pair| {
                let start = to_world(pair[0], 0.8);
                let delta = to_world(pair[1], 0.8) - start;
                let length = delta.length_squared();
                (length > 0.01).then_some((preferred, start, delta, length))
            })
        })
        .collect();
    let nearest = |target: Vec3| {
        segments
            .iter()
            .map(|&(preferred, start, delta, length)| {
                let along = ((target - start).dot(delta) / length).clamp(0.0, 1.0);
                let facing = if preferred && delta.x > 0.0 {
                    -delta
                } else {
                    delta
                };
                (preferred, start + delta * along, facing.normalize())
            })
            .min_by(|a, b| {
                b.0.cmp(&a.0).then_with(|| {
                    a.1.distance_squared(target)
                        .total_cmp(&b.1.distance_squared(target))
                })
            })
    };
    let (_, point, facing) = nearest(Vec3::ZERO)?;
    if guest {
        for direction in [facing, -facing] {
            if let Some((_, candidate, heading)) = nearest(point + direction * 5.0)
                && candidate.distance_squared(point) >= 4.0
            {
                return Some((candidate, heading));
            }
        }
        // Both ends of this short way are close. Shift within the road instead.
        return Some((point + facing.cross(Vec3::Y) * 0.65, facing));
    }
    Some((point, facing))
}

fn building_collider(building: &paket_geo::Building) -> Option<Collider> {
    if building.open_sided {
        return None;
    }
    let points: Vec<_> = building
        .footprint
        .iter()
        .map(|p| to_world(*p, 0.0))
        .collect();
    let mut parts = Vec::new();
    for (index, (a, b)) in points.iter().zip(points.iter().cycle().skip(1)).enumerate() {
        let length = a.distance(*b);
        if length < 0.01 {
            continue;
        }
        let along = (*b - *a) / length;
        let rotation = Quat::from_rotation_y(-along.z.atan2(along.x));
        let mut wall = |start: f32, width: f32, bottom: f32, top: f32| {
            if width > 0.01 && top > bottom {
                parts.push((
                    *a + along * (start + width * 0.5) + Vec3::Y * ((bottom + top) * 0.5),
                    rotation,
                    Collider::cuboid(width, top - bottom, 0.16),
                ));
            }
        };
        for (start, width, bottom, top) in buildings::wall_sections(building, index) {
            wall(start, width, bottom, top);
        }
    }
    (!parts.is_empty()).then(|| Collider::compound(parts))
}

fn street_mesh(
    streets: &[paket_geo::Street],
    sidewalk: bool,
    elevation: f32,
    repeat_meters: f32,
) -> Mesh {
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut indices = Vec::new();
    for street in streets {
        let half_width = street.width_m * 0.5;
        let [left, right] = street.sidewalk_m;
        let spans: &[(f32, f32)] = if sidewalk {
            &[
                (-half_width - left, -half_width),
                (half_width, half_width + right),
            ]
        } else {
            &[(-half_width, half_width)]
        };
        let points: Vec<_> = street
            .centerline
            .iter()
            .map(|point| to_world(*point, elevation))
            .collect();
        let sides: Vec<_> = points
            .windows(2)
            .map(|pair| {
                let delta = pair[1] - pair[0];
                Vec3::new(-delta.z, 0.0, delta.x).normalize_or_zero()
            })
            .collect();
        let miter = |index: usize| {
            let before = sides[index.saturating_sub(1)];
            let after = sides[index.min(sides.len() - 1)];
            let bisector = (before + after).normalize_or_zero();
            let cosine = bisector.dot(after);
            if cosine > 0.45 {
                bisector / cosine
            } else {
                after
            }
        };
        for (index, pair) in points.windows(2).enumerate() {
            let [a, b] = [pair[0], pair[1]];
            if a.distance_squared(b) < 1e-4 {
                continue;
            }
            let side_a = miter(index);
            let side_b = miter(index + 1);
            for &(from, to) in spans {
                if from == to {
                    continue;
                }
                let a_left = a + side_a * from;
                let a_right = a + side_a * to;
                let b_left = b + side_b * from;
                let b_right = b + side_b * to;
                push_triangle(
                    &mut positions,
                    &mut normals,
                    &mut indices,
                    a_left,
                    b_right,
                    b_left,
                );
                push_triangle(
                    &mut positions,
                    &mut normals,
                    &mut indices,
                    a_left,
                    a_right,
                    b_right,
                );
            }
        }
    }
    let uv: Vec<[f32; 2]> = positions
        .iter()
        .map(|point| [point.x / repeat_meters, point.z / repeat_meters])
        .collect();
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uv)
    .with_inserted_indices(Indices::U32(indices))
}

fn push_triangle(
    positions: &mut Vec<Vec3>,
    normals: &mut Vec<Vec3>,
    indices: &mut Vec<u32>,
    a: Vec3,
    b: Vec3,
    c: Vec3,
) {
    let normal = (b - a).cross(c - a).normalize_or_zero();
    let start = positions.len() as u32;
    positions.extend([a, b, c]);
    normals.extend([normal, normal, normal]);
    indices.extend([start, start + 1, start + 2]);
}

#[cfg(test)]
mod ride_tests {
    use super::*;

    #[test]
    fn location_selector_creates_map_before_leaving_the_3d_scene() {
        use bevy::ecs::system::RunSystemOnce;
        let mut app = headless_ride(Role::Courier);
        app.world_mut()
            .run_system_once(
                |mut commands: Commands, ride: Res<Ride>, mut next: ResMut<NextState<Mode>>| {
                    change_location(&mut commands, &ride, &mut next)
                },
            )
            .unwrap();
        app.update();
        assert_eq!(*app.world().resource::<State<Mode>>().get(), Mode::Map);
        assert!(app.world().contains_resource::<map::Map>());
        assert!(!app.world().contains_resource::<Ride>());
        assert_eq!(
            app.world_mut()
                .query_filtered::<Entity, With<Camera3d>>()
                .iter(app.world())
                .count(),
            0
        );
    }

    #[test]
    fn voted_neighbourhood_rebuilds_the_scene_and_keeps_the_shared_day() {
        let mut app = headless_ride(Role::Courier);
        app.add_systems(OnEnter(Mode::Travel), travel);
        let destination = map::CITIES[1].1;
        app.world_mut()
            .resource_mut::<shift::Shift>()
            .receive_snapshot(2, 0, 1, Some(destination.into()), vec![], 0);
        for _ in 0..4 {
            app.update();
        }
        assert_eq!(*app.world().resource::<State<Mode>>().get(), Mode::Ride);
        assert_eq!(app.world().resource::<Ride>().origin, destination);
        assert_eq!(app.world().resource::<shift::Shift>().day(), 2);
        assert_eq!(app.world().resource::<Jobs>().day(), 2);
        assert_eq!(
            app.world_mut()
                .query_filtered::<Entity, With<Camera3d>>()
                .iter(app.world())
                .count(),
            1
        );
    }

    #[test]
    fn koertestrasse_courtyard_passages_have_no_invisible_walls() {
        use bevy::ecs::system::RunSystemOnce;
        use bevy::time::TimeUpdateStrategy;
        let origin = LatLon {
            lat: 52.4895,
            lon: 13.4084,
        };
        let scene = paket_geo::parse_overpass(
            include_bytes!("../../../fixtures/koertestrasse.json"),
            origin,
        )
        .unwrap();
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            TransformPlugin,
            bevy::diagnostic::DiagnosticsPlugin,
            PhysicsPlugins::default(),
        ))
        .init_asset::<Mesh>()
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
            20,
        )));
        for b in &scene.buildings {
            if let Some(collider) = building_collider(b) {
                app.world_mut()
                    .spawn((RigidBody::Static, Transform::IDENTITY, collider));
            }
            if !b.passages.is_empty() {
                let (edge, _, _) = buildings::doorway(b);
                let a = b.footprint[edge];
                let z = b.footprint[(edge + 1) % b.footprint.len()];
                let mid = (z.east - a.east).hypot(z.north - a.north) * 0.5;
                assert!(
                    !b.passages
                        .iter()
                        .any(|p| p.edge == edge && mid + 0.64 > p.start && mid - 0.64 < p.end),
                    "door blocks arch in {}",
                    b.id
                );
            }
        }
        app.finish();
        app.cleanup();
        for _ in 0..3 {
            app.update();
        }
        // These two real OSM paths are the street gateway and passage in no. 35.
        for id in ["way/98862005", "way/595148187"] {
            let street = scene.streets.iter().find(|s| s.id == id).unwrap();
            let probes: Vec<_> = street
                .centerline
                .windows(2)
                .map(|p| (to_world(p[0], 0.95), to_world(p[1], 0.95)))
                .collect();
            let hits = app
                .world_mut()
                .run_system_once(move |spatial: SpatialQuery| {
                    probes
                        .iter()
                        .map(|&(a, b)| {
                            spatial.cast_shape(
                                &Collider::capsule(0.28, 1.1),
                                a,
                                Quat::IDENTITY,
                                Dir3::new(b - a).unwrap(),
                                &ShapeCastConfig::from_max_distance(a.distance(b)),
                                &SpatialQueryFilter::default(),
                            )
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap();
            assert!(
                hits.iter().all(Option::is_none),
                "courier cannot walk through {id}: {hits:?}"
            );
        }
    }

    pub(crate) fn headless_ride(role: Role) -> App {
        use bevy::time::TimeUpdateStrategy;
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            TransformPlugin,
            bevy::diagnostic::DiagnosticsPlugin,
            bevy::state::app::StatesPlugin,
            PhysicsPlugins::default(),
        ))
        .init_asset::<Mesh>()
        .init_asset::<StandardMaterial>()
        .init_asset::<ScatteringMedium>()
        .init_asset::<Image>()
        .init_resource::<ButtonInput<KeyCode>>()
        .init_resource::<ButtonInput<MouseButton>>()
        .init_resource::<bevy::input::mouse::AccumulatedMouseMotion>()
        .init_resource::<bevy::input::mouse::AccumulatedMouseScroll>()
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
            16,
        )))
        .insert_state(Mode::Ride);
        install_ride(&mut app);
        app.finish();
        app.cleanup();
        let origin = LatLon {
            lat: FIXTURE_LAT,
            lon: FIXTURE_LON,
        };
        let scene =
            paket_geo::parse_overpass(include_bytes!("../../../fixtures/kreuzberg.json"), origin)
                .unwrap();
        {
            let mut commands = app.world_mut().commands();
            start_ride(
                &mut commands,
                origin,
                PathBuf::new(),
                true,
                None,
                None,
                None,
                role,
            );
        }
        app.world_mut().flush();
        let (tx, rx) = mpsc::channel();
        tx.send(Ok(scene)).unwrap();
        app.world_mut()
            .resource_mut::<Ride>()
            .pending
            .insert(TileId { x: 0, y: 0 }, Mutex::new(rx));
        app.update();
        app
    }

    pub(crate) fn press(app: &mut App, key: KeyCode) {
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(key);
        app.update();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .reset(key);
    }

    #[test]
    fn mouse_look_needs_no_button_and_escape_releases_it() {
        use bevy::input::mouse::AccumulatedMouseMotion;
        let mut app = headless_ride(Role::Courier);
        let player = {
            let world = app.world_mut();
            world
                .query_filtered::<Entity, With<Player>>()
                .single(world)
                .unwrap()
        };
        let start = app.world().get::<Transform>(player).unwrap().rotation;
        app.world_mut()
            .resource_mut::<AccumulatedMouseMotion>()
            .delta = Vec2::new(100.0, 0.0);
        app.update();
        let turned = app.world().get::<Transform>(player).unwrap().rotation;
        assert!(start.angle_between(turned) > 0.2);
        press(&mut app, KeyCode::Escape);
        app.update();
        let released = app.world().get::<Transform>(player).unwrap().rotation;
        assert!(turned.angle_between(released) < 0.01);
        assert!(!app.world().resource::<Ride>().mouse_look);
    }

    #[test]
    fn guest_spawns_separately_from_the_host_on_the_same_road() {
        let origin = LatLon {
            lat: FIXTURE_LAT,
            lon: FIXTURE_LON,
        };
        let scene =
            paket_geo::parse_overpass(include_bytes!("../../../fixtures/kreuzberg.json"), origin)
                .unwrap();
        let (host, _) = street_start(&scene, origin, false).unwrap();
        let (guest, _) = street_start(&scene, origin, true).unwrap();
        assert!((guest.distance(host) - 5.0).abs() < 1e-4);
        assert_eq!(host.y, guest.y);
    }

    #[test]
    fn chase_camera_pulls_in_before_a_wall() {
        let mut app = headless_ride(Role::Courier);
        let player = {
            let world = app.world_mut();
            world
                .query_filtered::<Entity, With<Player>>()
                .single(world)
                .unwrap()
        };
        *app.world_mut().get_mut::<Transform>(player).unwrap() =
            Transform::from_xyz(5000., 0.87, 5000.);
        app.world_mut().spawn((
            RigidBody::Static,
            Collider::cuboid(10., 8., 0.3),
            Transform::from_xyz(5000., 3., 5002.),
        ));
        for _ in 0..6 {
            app.update();
        }
        let world = app.world_mut();
        let camera = world
            .query_filtered::<&Transform, With<Camera3d>>()
            .single(world)
            .unwrap();
        assert!(
            camera.translation.z < 5001.7,
            "camera crossed the wall: {:?}",
            camera.translation
        );
        assert!(
            camera.translation.z > 5000.0,
            "camera should remain behind the player"
        );
    }
}
