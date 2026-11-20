//! Native playback of the approved, derived recording cues.
#[path = "native_audio_mix.rs"]
mod mix;
use bevy::{
    audio::{AudioSinkPlayback, SpatialAudioSink, Volume},
    prelude::*,
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
        "paket-sound-setting"
            | "paket-mode"
            | "street-audio"
            | "game-sound"
            | "paket-bite"
            | "paket-hud"
            | "rich-round"
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
pub(crate) fn install(app: &mut App) {
    app.init_resource::<Audio>()
        .add_systems(Startup, setup)
        .add_systems(Update, render.after(crate::browser_ui::sound));
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
    audio.cues.insert(
        "dog-paw".into(),
        (1..=3)
            .map(|index| Cue {
                source: assets.load(format!("audio/dog-paw-{index}.wav")),
                gain: 0.14,
                looping: false,
            })
            .collect(),
    );
    audio.foreground = true;
    audio.mixer.muted = std::env::var("FEED_GAME_SOUND").is_ok_and(|value| value == "off");
    commands.spawn((SpatialListener::new(2.), Transform::default()));
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
        crate::platform::ui_event(
            "paket-settings",
            &serde_json::json!({"sound":!audio.mixer.muted}).to_string(),
        );
    }
    if let Ok(mut events) = EVENTS.get_or_init(Mutex::default).lock() {
        for (name, value) in events.drain(..) {
            if name == "paket-sound-setting" {
                if let Some(enabled) = value.as_bool() {
                    audio.mixer.muted = !enabled;
                }
            } else {
                audio.mixer.event(&name, &value, real.elapsed_secs_f64());
            }
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
