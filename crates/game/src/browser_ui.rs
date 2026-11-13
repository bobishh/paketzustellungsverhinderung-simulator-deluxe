use crate::{
    RemoteActor,
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
    player: Query<(&Player, &Transform)>,
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
    let Ok((p, t)) = player.single() else {
        return;
    };
    if *version != Some(roads.generation) {
        *version = Some(roads.generation);
        crate::platform::ui_event("paket-map-roads", &roads.radar());
    }
    let xy = |p: Vec3| [p.x, p.z];
    let data = serde_json::json!({"position":xy(t.translation),"heading":t.rotation.to_euler(EulerRot::YXZ).0,"target":jobs.current_target().map(xy),"van":p.parked.and_then(|e|vans.get(e).ok()).map(|v|xy(v.translation)),"dogs":dogs.iter().map(|t|xy(t.translation)).collect::<Vec<_>>(),"people":jobs.radar_people(&people),"peers":peers.iter().map(|t|xy(t.translation)).collect::<Vec<_>>()});
    crate::platform::ui_event("paket-radar", &data.to_string());
    let mut progress = jobs.progress();
    progress["workMs"] = serde_json::json!(shift.elapsed_ms());
    progress["ended"] = serde_json::json!(shift.ended());
    crate::platform::ui_event("paket-progress", &progress.to_string());
}
