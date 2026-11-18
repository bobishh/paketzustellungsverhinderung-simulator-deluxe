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
pub(crate) fn frame(time: Res<Time<Real>>) {
    crate::platform::render_frame(time.delta_secs() * 1000.);
}

pub(crate) fn update(
    time: Res<Time>,
    clock: Res<Time<Virtual>>,
    roads: Res<Roads>,
    elevation: Res<crate::elevation::Elevation>,
    sports: Query<&crate::places::SportsOutline>,
    jobs: Res<Jobs>,
    shift: Res<Shift>,
    player: Query<(&Player, &Transform, &avian3d::prelude::LinearVelocity)>,
    ride: Res<Ride>,
    vans: Query<&Transform, With<ParkedVan>>,
    dogs: Query<&Transform, With<Dog>>,
    people: Query<(&Resident, &Transform)>,
    peers: Query<(&RemoteActor, &Transform)>,
    mut timer: Local<f32>,
    mut version: Local<Option<(u64, u64)>>,
    mut outlines: Local<String>,
) {
    *timer += time.delta_secs();
    if *timer < 0.1 {
        return;
    }
    *timer = 0.;
    let Ok((p, t, velocity)) = player.single() else {
        return;
    };
    if *version != Some((roads.generation, shift.day())) {
        *version = Some((roads.generation, shift.day()));
        crate::platform::ui_event("paket-map-roads", &roads.radar());
    }
    let shapes =
        serde_json::to_string(&sports.iter().map(|shape| &shape.0).collect::<Vec<_>>()).unwrap();
    if *outlines != shapes {
        crate::platform::ui_event("paket-map-sports", &shapes);
        *outlines = shapes;
    }
    let xy = |p: Vec3| [p.x, p.z];
    let data = serde_json::json!({"position":xy(t.translation),"altitude":t.translation.y,"terrain":elevation.heights.available(),"heading":t.rotation.to_euler(EulerRot::YXZ).0,"target":jobs.current_target().map(xy),"van":p.parked.and_then(|e|vans.get(e).ok()).map(|v|xy(v.translation)),"dogs":dogs.iter().map(|t|xy(t.translation)).collect::<Vec<_>>(),"people":jobs.radar_people(&people),"peers":peers.iter().map(|(p,t)|serde_json::json!({"id":p.device,"position":xy(t.translation),"form":p.form,"age":(clock.elapsed_secs()-p.received).max(0.)})).collect::<Vec<_>>()});
    crate::platform::ui_event("paket-radar", &data.to_string());
    let mut hud = jobs.hud_data(t.translation);
    hud["navigation"] = serde_json::json!(
        jobs.navigation_target(p, p.parked.and_then(|id| vans.get(id).ok()))
            .map(|target| {
                let (bearing, distance) = crate::jobs::navigation_bearing(t, target);
                serde_json::json!({"bearing": bearing.to_degrees(), "distance": distance})
            })
    );
    hud["dispatcher"] = jobs.dispatcher(clock.elapsed_secs());
    hud["intercom"] = jobs.browser_intercom(clock.elapsed_secs());
    hud["minute"] = serde_json::json!(shift.minute());
    hud["day"] = serde_json::json!(shift.day());
    hud["vote"] = shift.hud(&ride);
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
    hud["stunSeconds"] = serde_json::json!((p.stunned_until - clock.elapsed_secs()).max(0.0));
    hud["vanDistance"] = serde_json::json!(
        p.parked
            .and_then(|e| vans.get(e).ok())
            .map(|v| crate::actors::horizontal_distance(v.translation, t.translation))
    );
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
    mut commands: Commands,
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
            "shift-vote" => {
                if let Some(i) = c["choice"].as_u64().filter(|i| *i < 3) {
                    shift.vote(i as u8);
                }
            }
            "intercom" => {
                if let (Some(revision), Some(kind)) = (c["revision"].as_u64(), c["kind"].as_str()) {
                    let choice = c["choice"]
                        .as_u64()
                        .and_then(|n| u8::try_from(n).ok())
                        .unwrap_or(0);
                    jobs.intercom_action(revision, kind, choice);
                }
            }
            "open-intercom" => jobs.request_intercom(),
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
            "retry-map" => ride.retry.clear(),
            "change-location" if !jobs.interaction_active() => {
                crate::change_location(&mut commands, &ride, &mut next)
            }
            _ => {}
        }
    }
}

// Publish committed street motion to the browser recording player at 10 Hz.
pub(crate) fn sound(
    time: Res<Time>,
    ride: Res<Ride>,
    jobs: Res<Jobs>,
    keys: Res<ButtonInput<KeyCode>>,
    player: Query<(&Player, &Transform, &avian3d::prelude::LinearVelocity)>,
    cars: Query<(&Transform, &avian3d::prelude::LinearVelocity), With<crate::traffic::Car>>,
    mut timer: Local<f32>,
) {
    *timer += time.delta_secs();
    if *timer < 0.1 {
        return;
    }
    *timer = 0.;
    let Ok((person, actor, velocity)) = player.single() else {
        return;
    };
    let active = !jobs.menu_open && !ride.waiting_for_tile;
    let controls = active && ride.mouse_look && !jobs.interaction_active();
    let pressed = |a, b| {
        if keys.pressed(a) || keys.pressed(b) {
            1.0
        } else {
            0.0
        }
    };
    let throttle = if controls {
        pressed(KeyCode::KeyW, KeyCode::ArrowUp) - pressed(KeyCode::KeyS, KeyCode::ArrowDown)
    } else {
        0.0
    };
    let traffic = cars
        .iter()
        .map(|(car, velocity)| {
            let offset = (car.translation - actor.translation).with_y(0.0);
            let distance = offset.length();
            (distance, offset, velocity.0.xz().length() * 3.6)
        })
        .filter(|(distance, _, _)| distance.is_finite() && *distance < 42.0)
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(distance, offset, speed)| {
            serde_json::json!({"distance":distance,"pan":actor.right().dot(offset.normalize_or_zero()),"speed":speed})
        });
    let form = match person.form {
        paket_session::Avatar::Van => "van",
        paket_session::Avatar::Dog => "dog",
        _ => "courier",
    };
    crate::platform::ui_event("street-audio", &serde_json::json!({
        "active":active,"position":actor.translation.to_array(),"speed":velocity.0.xz().length()*3.6,
        "verticalSpeed":velocity.0.y,"form":form,"throttle":throttle,"traffic":traffic
    }).to_string());
}
