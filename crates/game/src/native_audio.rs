//! Native playback of the approved, derived recording cues.
#[path = "native_audio_mix.rs"]
mod mix;
use bevy::{
    audio::{AudioSinkPlayback, SpatialAudioSink, Volume},
    prelude::*,
    ui_widgets::Button,
};
use serde_json::Value;
use std::{
    collections::{HashMap, HashSet, VecDeque},
    sync::{
        Mutex, OnceLock,
        atomic::{AtomicBool, Ordering},
    },
};

static EVENTS: OnceLock<Mutex<VecDeque<(String, Value)>>> = OnceLock::new();
static LOADING: AtomicBool = AtomicBool::new(false);
pub(crate) fn accepts(name: &str) -> bool {
    matches!(
        name,
        "paket-mode" | "street-audio" | "game-sound" | "paket-bite" | "paket-hud" | "rich-round"
    )
}
pub(crate) fn event(name: &str, value: &Value) {
    if !accepts(name) {
        return;
    }
    if let Ok(mut events) = EVENTS.get_or_init(Mutex::default).lock() {
        if events.len() >= 128 {
            events.pop_front();
        }
        events.push_back((name.into(), value.clone()));
    }
}
pub(crate) fn set_loading(loading: bool) {
    LOADING.store(loading, Ordering::Relaxed);
}
#[derive(Clone)]
struct Cue {
    source: Handle<AudioSource>,
    gain: f32,
    looping: bool,
}
#[derive(Resource, Default)]
struct Audio {
    mixer: mix::Mixer,
    cues: HashMap<String, Vec<Cue>>,
    loops: HashMap<String, Entity>,
    variants: HashMap<String, usize>,
    foreground: bool,
}
#[derive(Component)]
struct Voice;
#[derive(Component)]
struct SoundButton;
#[derive(Component)]
struct SoundIcon;

pub(crate) fn install(app: &mut App) {
    app.init_resource::<Audio>()
        .add_systems(Startup, setup)
        .add_observer(toggle)
        .add_systems(
            Update,
            (render.after(crate::browser_ui::sound), paint.after(render)),
        );
}
fn setup(mut commands: Commands, assets: Res<AssetServer>, mut audio: ResMut<Audio>) {
    let bank: Value =
        serde_json::from_str(include_str!("../../../assets/native/audio/manifest.json"))
            .expect("verified derived audio manifest");
    for (name, records) in bank["cues"].as_object().into_iter().flatten() {
        let cues = records
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|cue| {
                Some(Cue {
                    source: assets.load(cue["file"].as_str()?.to_string()),
                    gain: cue["gain"].as_f64()? as f32,
                    looping: cue["loop"] == true,
                })
            })
            .collect();
        audio.cues.insert(name.clone(), cues);
    }
    audio.foreground = true;
    audio.mixer.muted = std::env::var("FEED_GAME_SOUND").is_ok_and(|value| value == "off");
    commands.spawn((SpatialListener::new(2.), Transform::default()));
    commands.spawn((
        Button,
        SoundButton,
        bevy::picking::hover::Hovered::default(),
        AccessibleLabel::new("Mute sound (F8)"),
        Node {
            position_type: PositionType::Absolute,
            top: px(8),
            right: px(118),
            width: px(44),
            height: px(44),
            display: Display::None,
            border_radius: BorderRadius::MAX,
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        },
        BackgroundColor(Color::srgba(0.05, 0.11, 0.12, 0.9)),
        GlobalZIndex(60),
        children![(
            ImageNode::new(assets.load("native/audio/speaker.png")),
            SoundIcon,
            Node {
                width: px(28),
                height: px(28),
                ..default()
            }
        )],
    ));
}
fn toggle(
    event: On<bevy::ui_widgets::Activate>,
    buttons: Query<(), With<SoundButton>>,
    mut audio: ResMut<Audio>,
    mut mouse: ResMut<ButtonInput<MouseButton>>,
) {
    if buttons.contains(event.entity) {
        audio.mixer.muted = !audio.mixer.muted;
        mouse.clear_just_pressed(MouseButton::Left);
    }
}
fn render(
    mut commands: Commands,
    real: Res<Time<Real>>,
    clock: Res<Time<Virtual>>,
    windows: Query<&Window>,
    mode: Res<State<crate::Mode>>,
    ride: Option<Res<crate::Ride>>,
    roots: Query<&WorldAssetRoot>,
    assets: Res<AssetServer>,
    mut audio: ResMut<Audio>,
    mut voices: Query<(Entity, &mut Transform, Option<&mut SpatialAudioSink>), With<Voice>>,
    mut lifecycle: MessageReader<bevy::window::AppLifecycle>,
    keys: Res<ButtonInput<KeyCode>>,
) {
    for state in lifecycle.read() {
        match state {
            bevy::window::AppLifecycle::Suspended => audio.foreground = false,
            bevy::window::AppLifecycle::Running => audio.foreground = true,
            _ => {}
        }
    }
    if keys.just_pressed(KeyCode::F8) {
        audio.mixer.muted = !audio.mixer.muted;
    }
    if let Ok(mut events) = EVENTS.get_or_init(Mutex::default).lock() {
        for (name, value) in events.drain(..) {
            audio.mixer.event(&name, &value, real.elapsed_secs_f64());
        }
    }
    let ready = ride.as_ref().is_some_and(|ride| !ride.loaded.is_empty())
        && !roots.is_empty()
        && roots
            .iter()
            .all(|root| assets.is_loaded_with_dependencies(root.0.id()));
    let allowed = *mode.get() == crate::Mode::Ride
        && audio.foreground
        && !clock.is_paused()
        && windows.single().is_ok_and(|window| window.focused)
        && ready
        && !LOADING.load(Ordering::Relaxed);
    let mix = audio.mixer.frame(allowed);
    if !mix.active {
        for (entity, _, _) in &mut voices {
            commands.entity(entity).try_despawn();
        }
        audio.loops.clear();
        return;
    }
    let wanted: HashSet<_> = mix
        .loops
        .iter()
        .filter(|sound| sound.gain > 0.001)
        .map(|s| s.name.clone())
        .collect();
    audio.loops.retain(|name, entity| {
        if !wanted.contains(name) {
            commands.entity(*entity).try_despawn();
            false
        } else {
            true
        }
    });
    for sound in mix.loops.into_iter().filter(|sound| sound.gain > 0.001) {
        let Some(cue) = audio
            .cues
            .get(&sound.name)
            .and_then(|c| c.first())
            .cloned()
            .filter(|c| c.looping)
        else {
            continue;
        };
        if let Some(entity) = audio.loops.get(&sound.name).copied() {
            if let Ok((_, mut transform, sink)) = voices.get_mut(entity) {
                transform.translation = Vec3::new(sound.pan * 2., 0., -1.);
                if let Some(mut sink) = sink {
                    sink.set_volume(Volume::Linear((cue.gain * sound.gain * 0.5).clamp(0., 1.)));
                    sink.set_speed(sound.rate.clamp(0.5, 2.));
                    sink.set_emitter_position(transform.translation);
                }
            }
        } else {
            let entity = spawn(&mut commands, &cue, &sound);
            audio.loops.insert(sound.name, entity);
        }
    }
    let mut count = voices.iter().count();
    for sound in mix.effects {
        if count >= 16 {
            break;
        }
        let Some(cues) = audio.cues.get(&sound.name).cloned() else {
            continue;
        };
        if cues.is_empty() {
            continue;
        }
        let variant = audio.variants.entry(sound.name.clone()).or_default();
        let index = sound.variant.unwrap_or(*variant) % cues.len();
        *variant = variant.wrapping_add(1);
        let cue = &cues[index];
        if !cue.looping {
            spawn(&mut commands, cue, &sound);
            count += 1;
        }
    }
}
fn spawn(commands: &mut Commands, cue: &Cue, sound: &mix::Sound) -> Entity {
    let playback = if cue.looping {
        PlaybackSettings::LOOP
    } else {
        PlaybackSettings::DESPAWN
    };
    commands
        .spawn((
            AudioPlayer::new(cue.source.clone()),
            playback
                .with_spatial(true)
                .with_speed(sound.rate.clamp(0.5, 2.))
                .with_volume(Volume::Linear((cue.gain * sound.gain * 0.5).clamp(0., 1.))),
            Transform::from_xyz(sound.pan.clamp(-1., 1.) * 2., 0., -1.),
            Voice,
        ))
        .id()
}
fn paint(
    mut commands: Commands,
    audio: Res<Audio>,
    mode: Res<State<crate::Mode>>,
    assets: Res<AssetServer>,
    mut buttons: Query<(Entity, &mut Node), With<SoundButton>>,
    mut icons: Query<&mut ImageNode, With<SoundIcon>>,
) {
    let path = if audio.mixer.muted {
        "native/audio/muted.png"
    } else {
        "native/audio/speaker.png"
    };
    let image = assets.load(path);
    for mut icon in &mut icons {
        if icon.image != image {
            icon.image = image.clone();
        }
    }
    for (entity, mut node) in &mut buttons {
        node.display = if *mode.get() == crate::Mode::Ride {
            Display::Flex
        } else {
            Display::None
        };
        if audio.is_changed() {
            commands
                .entity(entity)
                .insert(AccessibleLabel::new(if audio.mixer.muted {
                    "Enable sound (F8)"
                } else {
                    "Mute sound (F8)"
                }));
        }
    }
}
