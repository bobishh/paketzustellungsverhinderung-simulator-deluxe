use crate::{
    Mode, RemoteActor, Ride,
    actors::Controls,
    actors::{ParkedVan, Player},
    dogs::Dog,
    jobs::{Jobs, Resident},
    roads::Roads,
    shift::Shift,
};
use bevy::prelude::*;
pub(crate) fn update(
    time: Res<Time>,
    roads: Res<Roads>,
    jobs: Res<Jobs>,
    shift: Res<Shift>,
    player: Query<(&Player, &Transform, &avian3d::prelude::LinearVelocity)>,
    ride: Res<Ride>,
    vans: Query<&Transform, With<ParkedVan>>,
    dogs: Query<&Transform, With<Dog>>,
    people: Query<(&Resident, &Transform)>,
    peers: Query<&Transform, With<RemoteActor>>,
    mut timer: Local<f32>,
    mut version: Local<Option<u64>>,
) {
    if !cfg!(target_arch = "wasm32") {
        return;
    }
    *timer += time.delta_secs();
    if *timer < 0.1 {
        return;
    }
    *timer = 0.;
    let Ok((p, t, velocity)) = player.single() else {
        return;
    };
    if *version != Some(roads.generation) {
        *version = Some(roads.generation);
        crate::platform::ui_event("paket-map-roads", &roads.radar());
    }
    let xy = |p: Vec3| [p.x, p.z];
    let data = serde_json::json!({"position":xy(t.translation),"heading":t.rotation.to_euler(EulerRot::YXZ).0,"target":jobs.current_target().map(xy),"van":p.parked.and_then(|e|vans.get(e).ok()).map(|v|xy(v.translation)),"dogs":dogs.iter().map(|t|xy(t.translation)).collect::<Vec<_>>(),"people":jobs.radar_people(&people),"peers":peers.iter().map(|t|xy(t.translation)).collect::<Vec<_>>()});
    crate::platform::ui_event("paket-radar", &data.to_string());
    let mut hud = jobs.hud_data(t.translation);
    hud["minute"] = serde_json::json!(shift.minute());
    hud["ready"] = serde_json::json!(shift.ready);
    hud["ended"] = serde_json::json!(shift.ended());
    hud["difficulty"] = serde_json::json!(shift.difficulty);
    hud["canChangeDifficulty"] = serde_json::json!(
        !shift.locked && ride.session.as_ref().map_or(!ride.guest, |s| s.is_host)
    );
    hud["form"] = serde_json::json!(match p.form {
        paket_session::Avatar::Van => "van",
        paket_session::Avatar::Dog => "dog",
        _ => "courier",
    });
    hud["photo"] = serde_json::json!(p.photo);
    hud["photoReady"] = serde_json::json!(jobs.photo_ready);
    hud["speed"] = serde_json::json!((velocity.0.xz().length() * 3.6).round());
    hud["loading"] = serde_json::json!(ride.waiting_for_tile);
    hud["dogPrompt"] = serde_json::json!(jobs.dog_status(t.translation));
    crate::platform::ui_event("paket-hud", &hud.to_string());
    let mut progress = jobs.progress();
    progress["workMs"] = serde_json::json!(shift.elapsed_ms());
    progress["ended"] = serde_json::json!(shift.ended());
    crate::platform::ui_event("paket-progress", &progress.to_string());
}

pub(crate) fn input(
    mut jobs: ResMut<Jobs>,
    mut shift: ResMut<Shift>,
    mut ride: ResMut<Ride>,
    mut controls: ResMut<Controls>,
    mut next: ResMut<NextState<Mode>>,
) {
    let values: Vec<serde_json::Value> =
        serde_json::from_str(&crate::platform::ui_commands()).unwrap_or_default();
    for c in values {
        match c["action"].as_str().unwrap_or("") {
            "delivery" => {
                if let Some(i) = c["index"].as_u64() {
                    jobs.select_delivery(i as usize);
                }
            }
            "delivery-step" => {
                if let Some(d) = c["delta"].as_i64() {
                    jobs.step_delivery(d.clamp(-1, 1));
                }
            }
            "menu" => {
                jobs.menu_open = c["open"].as_bool().unwrap_or(false);
                ride.mouse_look = !jobs.menu_open;
            }
            "look" => ride.mouse_look = c["active"].as_bool().unwrap_or(false),
            "sensitivity" => {
                if let Some(v) = c["value"].as_f64() {
                    if v.is_finite() {
                        controls.sensitivity = (v as f32).clamp(0.0008, 0.006);
                    }
                }
            }
            "difficulty"
                if !shift.locked && ride.session.as_ref().map_or(!ride.guest, |s| s.is_host) =>
            {
                if let Some(d) = c["value"].as_u64().filter(|d| *d < 3) {
                    shift.difficulty = d as u8;
                }
            }
            "map" if !jobs.interaction_active() => next.set(Mode::Map),
            _ => {}
        }
    }
}
