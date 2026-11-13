use std::{
    collections::{HashMap, HashSet},
    fs,
    time::{SystemTime, UNIX_EPOCH},
};

use avian3d::prelude::*;
use bevy::{
    prelude::*,
    render::view::screenshot::{Screenshot, ScreenshotCaptured},
};
use paket_session::{Avatar, Fact, StreetAction};

use crate::{
    BuildingLink, Mode, RemoteActor, Ride,
    actors::{self, Models, ParkedVan, Player, Role},
};

const DROP_DISTANCE: f32 = 3.4;
const DELIVERIES_PER_SHIFT: usize = 12;
const PHOTO_DISTANCE: f32 = 9.0;
const HANDOVER_DISTANCE: f32 = 2.7;
#[path = "jobs/service.rs"]
mod service;
pub(crate) use service::{service_ui, service_update};

#[derive(Resource, Default)]
pub(crate) struct Jobs {
    day: u64,
    service: service::Service,
    parcels: Vec<Parcel>,
    selected: usize,
    navigation: usize,
    pub menu_open: bool,
    pub room: Option<Room>,
    open_doors: HashSet<String>,
    scratches: u8,
    marks: HashMap<String, Vec3>,
    seen: HashMap<String, HashSet<u64>>,
    sequence: u64,
    pub status: String,
    proof_pending: Option<f32>,
    proof_dir: Option<std::path::PathBuf>,
    caught: bool,
    pub prompt: String,
    pub photo_ready: bool,
    pub pursued: bool,
}

struct Parcel {
    address: String,
    label: String,
    point: Vec3,
    state: ParcelState,
    dropped_at: Option<String>,
    prop: Option<Entity>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum ParcelState {
    InVan,
    Carrying,
    Flying { position: Vec3 },
    Dropped { position: Vec3, wrong_address: bool },
    Filed(i32),
    Handed,
}

#[derive(Component)]
pub(crate) struct FlyingParcel {
    index: usize,
    still_for: f32,
}

#[derive(Clone)]
pub(crate) struct Room {
    address: String,
    center: Vec3,
    direction: Vec3,
}

impl Room {
    pub fn start(&self) -> (Vec3, Vec3) {
        (self.center, self.direction)
    }

    fn threshold(&self) -> Vec3 {
        self.center + self.direction * 1.66
    }
}

#[derive(Component)]
pub(crate) struct Resident {
    home: Vec3,
    heading: Vec3,
    watching: bool,
    address: String,
    doorstep: Vec3,
    state: ResidentState,
    answered: bool,
    last_seen: Vec3,
    memory_until: f32,
    stunned_until: f32,
    settled: Option<usize>,
    held: Option<Entity>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum ResidentState {
    Inside,
    Answering(f32),
    AtDoor(f32),
    Chasing,
    Returning,
    Entering,
}

#[derive(Component)]
pub(crate) struct Door {
    address: String,
    closed: Quat,
    opened: bool,
}

impl Jobs {
    pub fn delivering(&self) -> bool {
        self.parcels.iter().any(|p| {
            matches!(
                p.state,
                ParcelState::Carrying | ParcelState::Flying { .. } | ParcelState::Dropped { .. }
            )
        })
    }
    pub fn traffic_bonus(&mut self, pedestrian: bool) {
        let points = if pedestrian { 5 } else { 25 };
        self.service.bonus += points;
        self.status = format!(
            "{} · +{points} KPI",
            if pedestrian {
                "Pavement blocked"
            } else {
                "Traffic blocked during delivery"
            }
        );
    }
    pub fn day(&self) -> u64 {
        self.day.max(1)
    }
    /// Inspect another stop without teleporting or losing the parcel in hand.
    pub fn select_delivery(&mut self, index: usize) {
        if index >= self.parcels.len()
            || self.service.modal.is_some()
            || self.service.bulk.is_some()
        {
            return;
        }
        self.navigation = index;
        let busy = self.parcels.get(self.selected).is_some_and(|p| {
            matches!(
                p.state,
                ParcelState::Carrying | ParcelState::Flying { .. } | ParcelState::Dropped { .. }
            )
        });
        if !busy {
            self.selected = index;
        }
    }
    pub fn step_delivery(&mut self, delta: i64) {
        if !self.parcels.is_empty() {
            self.select_delivery(
                (self.navigation as i64 + delta).rem_euclid(self.parcels.len() as i64) as usize,
            );
        }
    }
    fn advance_after_delivery(&mut self, completed: usize) {
        if !self
            .parcels
            .get(completed)
            .is_some_and(|p| matches!(p.state, ParcelState::Filed(_) | ParcelState::Handed))
        {
            return;
        }
        if let Some(next) = (1..self.parcels.len())
            .map(|offset| (completed + offset) % self.parcels.len())
            .find(|&i| {
                !matches!(
                    self.parcels[i].state,
                    ParcelState::Filed(_) | ParcelState::Handed
                )
            })
        {
            self.selected = next;
            self.navigation = next;
        }
        self.caught = false;
        self.pursued = false;
        self.photo_ready = false;
    }
    pub fn hud_data(&self, position: Vec3) -> serde_json::Value {
        serde_json::json!({
            "selected":self.navigation,"active":self.selected,
            "routes":self.parcels.iter().enumerate().map(|(i,p)|serde_json::json!({
                "index":i,"address":p.label,"name":service::citizen(&p.address).name,
                "point":[p.point.x,p.point.z],
                "distance":actors::horizontal_distance(position,p.point),
                "parcelDistance":match p.state { ParcelState::Flying{position:p}|ParcelState::Dropped{position:p,..}=>Some(position.distance(p)),_=>None },
                "rung":self.service.visited.contains(&p.address),
                "state":match p.state { ParcelState::InVan=>"in-van",ParcelState::Carrying=>"carrying",ParcelState::Flying{..}=>"in-flight",ParcelState::Dropped{..}=>"photo-due",ParcelState::Filed(_)=>"filed",ParcelState::Handed=>"handed" }
            })).collect::<Vec<_>>(),
            "prompt":self.prompt,"status":self.status,"interaction":self.service.modal.is_some(),"menu":self.menu_open,
            "bulk":self.service.bulk.map(|(_,loaded)|loaded),
            "score":self.progress()["score"],"delivered":self.progress()["delivered"]
        })
    }
    pub fn radar_people(&self, residents: &Query<(&Resident, &Transform)>) -> Vec<[f32; 2]> {
        residents
            .iter()
            .filter(|(r, _)| r.watching)
            .map(|(_, t)| [t.translation.x, t.translation.z])
            .collect()
    }
    pub fn progress(&self) -> serde_json::Value {
        serde_json::json!({"delivered":self.parcels.iter().filter(|p|matches!(p.state,ParcelState::Filed(_)|ParcelState::Handed)).count(),"photos":self.parcels.iter().filter(|p|matches!(p.state,ParcelState::Filed(_))).count(),"score":self.parcels.iter().map(|p|if let ParcelState::Filed(n)=p.state{n}else{0}).sum::<i32>()+self.service.bonus,"rings":self.service.visited.len(),"markedVans":self.marks.len(),"special":self.service.rare_used&&self.service.bulk.is_none()})
    }
    pub fn interaction_active(&self) -> bool {
        self.service.modal.is_some() || self.menu_open
    }
    pub fn interaction_camera(&self) -> Option<Transform> {
        self.service.modal.as_ref().map(|m| m.camera)
    }
    pub fn seed(
        &mut self,
        mut addresses: Vec<BuildingLink>,
        commands: &mut Commands,
        models: &Models,
    ) {
        if self.parcels.len() >= DELIVERIES_PER_SHIFT || addresses.is_empty() {
            return;
        }
        addresses.sort_by(|a, b| {
            a.address.is_none().cmp(&b.address.is_none()).then_with(|| {
                a.entrance
                    .length_squared()
                    .total_cmp(&b.entrance.length_squared())
                    .then(a.id.cmp(&b.id))
            })
        });
        if self.room.is_none() {
            let room_address = &addresses[0];
            self.room = Some(Room {
                address: room_address.id.clone(),
                center: room_address.threshold.with_y(-0.04) - room_address.outward * 1.66,
                direction: room_address.outward,
            });
        }
        for address in addresses {
            if self.parcels.len() >= DELIVERIES_PER_SHIFT {
                break;
            }
            if self.parcels.iter().any(|p| p.address == address.id) {
                continue;
            }
            let index = self.parcels.len();
            self.parcels.push(Parcel {
                address: address.id.clone(),
                label: address
                    .address
                    .clone()
                    .unwrap_or_else(|| "Unnumbered building".into()),
                point: self.doorstep(&address),
                state: ParcelState::InVan,
                dropped_at: None,
                prop: None,
            });
            let home = (address.threshold - address.outward * 1.15).with_y(-0.04);
            let entity = commands
                .spawn((
                    Resident {
                        home,
                        heading: address.outward,
                        watching: false,
                        address: address.id.clone(),
                        doorstep: address.entrance.with_y(-0.04),
                        state: ResidentState::Inside,
                        answered: false,
                        last_seen: home,
                        memory_until: index as f32 * 0.15,
                        stunned_until: 0.0,
                        settled: None,
                        held: None,
                    },
                    Transform::from_translation(home).looking_to(address.outward, Vec3::Y),
                    Visibility::default(),
                    DespawnOnExit(Mode::Ride),
                ))
                .id();
            models.person(commands, entity, false);
        }
    }

    fn doorstep(&self, building: &BuildingLink) -> Vec3 {
        building.entrance
    }

    pub fn dog_indoors(&self, position: Vec3) -> bool {
        self.room.as_ref().is_some_and(|room| {
            let delta = position - room.center;
            delta.dot(room.direction).abs() < 1.49
                && delta.dot(room.direction.cross(Vec3::Y)).abs() < 1.24
        })
    }

    pub fn emit(&mut self, ride: &Ride, event: StreetAction) {
        if let Some(session) = &ride.session {
            self.sequence += 1;
            let _ = session.publish(Fact::StreetAction {
                origin: session.origin,
                day: self.day(),
                device: session.device.clone(),
                sequence: self.sequence,
                event,
            });
        }
    }

    pub fn receive(
        &mut self,
        local: &str,
        device: String,
        sequence: u64,
        event: StreetAction,
        now: f32,
        player: Option<&mut Player>,
    ) {
        if device == local {
            return;
        }
        let events = self.seen.entry(device).or_default();
        if !events.insert(sequence) {
            return;
        }
        match event {
            StreetAction::Door { building_id } => {
                self.open_doors.insert(building_id);
            }
            StreetAction::Bite { target } if target == local => {
                if let Some(player) = player {
                    if player.form == Avatar::Courier {
                        player.stunned_until = now + 1.5;
                        self.status = "Dog bite. Give it a second.".into();
                    }
                }
            }
            StreetAction::MarkVan { owner, east, north }
                if east.is_finite() && north.is_finite() =>
            {
                self.marks.insert(owner, Vec3::new(east, 0.0, -north));
            }
            _ => {}
        }
    }

    pub fn needs_van(&self) -> bool {
        self.navigation == self.selected
            && self
                .parcels
                .get(self.selected)
                .is_some_and(|parcel| matches!(parcel.state, ParcelState::InVan))
    }

    pub(crate) fn complete(&self) -> bool {
        !self.parcels.is_empty()
            && self
                .parcels
                .iter()
                .all(|parcel| matches!(parcel.state, ParcelState::Filed(_) | ParcelState::Handed))
    }

    pub fn current_target(&self) -> Option<Vec3> {
        if let Some((target, true)) = self.service.bulk {
            return Some(target);
        }
        self.parcels
            .get(self.navigation)
            .and_then(|parcel| match parcel.state {
                ParcelState::Flying { position } | ParcelState::Dropped { position, .. } => {
                    Some(position)
                }
                ParcelState::Filed(_) | ParcelState::Handed => None,
                _ => Some(parcel.point),
            })
    }

    pub fn dog_status(&self, here: Vec3) -> &'static str {
        let Some(room) = &self.room else {
            return "Finding a house…";
        };
        if !self.open_doors.contains(&room.address) {
            "Door shut · E scratch"
        } else if actors::horizontal_distance(here, room.center) < 2.4 {
            "Door open · get outside"
        } else {
            "E bite · G pee on van"
        }
    }

    pub fn manifest(&self) -> String {
        let score: i32 = self
            .parcels
            .iter()
            .map(|parcel| match parcel.state {
                ParcelState::Filed(value) => value,
                _ => 0,
            })
            .sum();
        let mut lines = vec![format!("Route · {} KPI", score + self.service.bonus)];
        for (index, parcel) in self.parcels.iter().enumerate() {
            let state = match parcel.state {
                ParcelState::InVan => "in van",
                ParcelState::Carrying => "carrying",
                ParcelState::Flying { .. } => "in flight",
                ParcelState::Dropped { .. } => "photo due",
                ParcelState::Filed(_) => "filed",
                ParcelState::Handed => "handed over / 0 KPI",
            };
            let cursor = if index == self.navigation { ">" } else { " " };
            lines.push(format!(
                "{cursor} {}  {}  ·  {state}",
                index + 1,
                format!(
                    "{} · {}",
                    parcel.label,
                    service::citizen(&parcel.address).name
                )
            ));
        }
        lines.join("\n")
    }

    pub fn radar(
        &self,
        own: Option<&str>,
        position: &Transform,
        vans: impl Iterator<Item = (String, Vec3)>,
    ) -> String {
        let current: HashMap<_, _> = vans.collect();
        let nearest = self
            .marks
            .iter()
            .filter(|(owner, _)| Some(owner.as_str()) != own)
            .map(|(owner, marked)| current.get(owner).unwrap_or(marked))
            .map(|point| (*point - position.translation).with_y(0.0))
            .min_by(|a, b| a.length_squared().total_cmp(&b.length_squared()));
        nearest.map_or_else(String::new, |delta| {
            let angle = delta
                .dot(position.right().as_vec3())
                .atan2(delta.dot(position.forward().as_vec3()))
                .to_degrees();
            format!("Marked van · {:.0} m · {:+.0}°", delta.length(), angle)
        })
    }
}

/// Refill the same neighbourhood without replacing the world or the mesh connection.
pub(crate) fn new_day(
    mut commands: Commands,
    mut jobs: ResMut<Jobs>,
    mut shift: ResMut<crate::shift::Shift>,
    mut residents: Query<(Entity, &mut Resident, &mut Transform), Without<Door>>,
    mut doors: Query<(Entity, &mut Door, &mut Transform), Without<Resident>>,
    mut players: Query<(Entity, &mut Player)>,
    mut dogs: Query<&mut crate::dogs::Dog>,
    ride: Res<Ride>,
    mut next: ResMut<NextState<Mode>>,
) {
    if !shift.ready || jobs.day == shift.day() {
        return;
    }
    let initial = jobs.day == 0;
    if initial {
        jobs.day = shift.day();
        if shift.world.is_none_or(|p| {
            (p.lat - ride.origin.lat).abs() + (p.lon - ride.origin.lon).abs() < 1e-8
        }) {
            return;
        }
    }
    let day = shift.day();
    let mut progress = jobs.progress();
    progress["ended"] = serde_json::json!(true);
    if !initial {
        crate::platform::ui_event("paket-progress", &progress.to_string());
    }
    if let Some(world) = shift.world {
        if (world.lat - ride.origin.lat).abs() + (world.lon - ride.origin.lon).abs() > 1e-8 {
            *jobs = Jobs::default();
            jobs.day = day;
            jobs.menu_open = true;
            commands.insert_resource(crate::NextNeighbourhood(world.into()));
            crate::platform::ui_event(
                if initial { "paket-travel" } else { "paket-day" },
                &serde_json::json!({"day":day,"travel":true}).to_string(),
            );
            next.set(Mode::Travel);
            return;
        }
    }
    for parcel in &mut jobs.parcels {
        if let Some(prop) = parcel.prop.take() {
            commands.entity(prop).try_despawn();
        }
        parcel.state = ParcelState::InVan;
        parcel.dropped_at = None;
    }
    if !jobs.parcels.is_empty() {
        jobs.parcels.rotate_left(1);
    }
    for (entity, mut resident, mut transform) in &mut residents {
        if let Some(prop) = resident.held.take() {
            commands.entity(prop).try_despawn();
        }
        commands.entity(entity).remove::<actors::HoldingParcel>();
        resident.state = ResidentState::Inside;
        resident.watching = false;
        resident.answered = false;
        resident.memory_until = 0.;
        resident.stunned_until = 0.;
        resident.settled = None;
        resident.last_seen = resident.home;
        *transform =
            Transform::from_translation(resident.home).looking_to(resident.heading, Vec3::Y);
    }
    for (entity, mut door, mut transform) in &mut doors {
        door.opened = false;
        transform.rotation = door.closed;
        commands.entity(entity).remove::<ColliderDisabled>();
    }
    for (entity, mut player) in &mut players {
        player.photo = false;
        player.stunned_until = 0.;
        commands.entity(entity).remove::<actors::HoldingParcel>();
        commands.entity(player.visual).insert(Visibility::Inherited);
    }
    for mut dog in &mut dogs {
        dog.reset_day();
    }
    let revision = jobs.service.revision + 1;
    jobs.service = service::Service::default();
    jobs.service.revision = revision;
    jobs.day = day;
    jobs.selected = 0;
    jobs.navigation = 0;
    jobs.open_doors.clear();
    jobs.marks.clear();
    jobs.seen.clear();
    jobs.scratches = 0;
    jobs.proof_pending = None;
    jobs.caught = false;
    jobs.pursued = false;
    jobs.photo_ready = false;
    jobs.prompt.clear();
    jobs.status.clear();
    shift.locked = false;
    crate::platform::ui_event("paket-day", &serde_json::json!({"day":day}).to_string());
}

pub(crate) fn spawn_door(
    commands: &mut Commands,
    models: &Models,
    building: &BuildingLink,
    parent: Entity,
) {
    let rotation = Transform::IDENTITY
        .looking_to(building.outward, Vec3::Y)
        .rotation;
    let hinge = building.threshold + rotation * Vec3::X * 0.60;
    let root = commands
        .spawn((
            Door {
                address: building.id.clone(),
                closed: rotation,
                opened: false,
            },
            Transform::from_translation(hinge).with_rotation(rotation),
            Visibility::default(),
            ChildOf(parent),
            RigidBody::Static,
            Collider::compound(vec![(
                Vec3::new(-0.60, 1.09, 0.0),
                Quat::IDENTITY,
                Collider::cuboid(1.20, 2.18, 0.10),
            )]),
        ))
        .id();
    models.door(commands, root);
}

pub(crate) fn open_room(
    mut commands: Commands,
    jobs: Res<Jobs>,
    time: Res<Time>,
    mut doors: Query<(Entity, &mut Door, &mut Transform)>,
) {
    for (entity, mut door, mut transform) in &mut doors {
        if jobs.open_doors.contains(&door.address) {
            if !door.opened {
                door.opened = true;
                commands.entity(entity).insert(ColliderDisabled);
            }
            transform.rotation = transform.rotation.slerp(
                door.closed * Quat::from_rotation_y(-1.65),
                1.0 - (-7.0 * time.delta_secs()).exp(),
            );
        }
    }
}

pub(crate) fn visible(spatial: &SpatialQuery, eye: Vec3, point: Vec3, exclude: Entity) -> bool {
    let delta = point - eye;
    let Ok(direction) = Dir3::new(delta) else {
        return true;
    };
    spatial
        .cast_ray(
            eye,
            direction,
            (delta.length() - 0.16).max(0.0),
            true,
            &SpatialQueryFilter::from_excluded_entities([exclude]),
        )
        .is_none()
}

pub(crate) fn residents(
    mut commands: Commands,
    time: Res<Time>,
    clock: Res<Time<Virtual>>,
    spatial: SpatialQuery,
    keys: Res<ButtonInput<KeyCode>>,
    actor: Query<(Entity, &Player, &Transform)>,
    mut jobs: ResMut<Jobs>,
    shift: Res<crate::shift::Shift>,
    mut npcs: Query<(Entity, &mut Resident, &mut Transform), Without<Player>>,
) {
    let Ok((player_entity, player, actor)) = actor.single() else {
        return;
    };
    let now = clock.elapsed_secs();
    let selected = jobs.selected;
    let parcel = jobs.parcels.get(selected);
    let active = parcel
        .is_some_and(|p| matches!(p.state, ParcelState::Carrying | ParcelState::Dropped { .. }));
    let correct_drop = parcel.is_some_and(|p| {
        matches!(
            p.state,
            ParcelState::Dropped {
                wrong_address: false,
                ..
            }
        )
    });
    let recipient = parcel.map(|p| p.address.clone());
    let dropped_at = parcel.and_then(|p| p.dropped_at.clone());
    let noisy = (player.form == Avatar::Van && keys.just_pressed(KeyCode::KeyH))
        || (player.form == Avatar::Dog && keys.just_pressed(KeyCode::KeyB));
    let mut witnessed = false;
    let mut pursued = false;
    for (entity, mut npc, mut transform) in &mut npcs {
        let citizen = service::citizen(&npc.address);
        if !citizen.home_at(shift.minute()) && npc.state == ResidentState::Inside {
            continue;
        }
        if jobs.service.modal.as_ref().is_some_and(|m| matches!(m.kind, service::ModalKind::Dialogue { recipient, .. } if recipient == entity)) { continue; }
        let concerned = active
            && npc.settled != Some(selected)
            && (recipient.as_ref() == Some(&npc.address)
                || dropped_at.as_ref() == Some(&npc.address));
        if !concerned && npc.state == ResidentState::Chasing {
            npc.watching = false;
            npc.state = ResidentState::Returning;
        }
        if now < npc.stunned_until {
            continue;
        }
        if npc.state == ResidentState::Inside {
            let hears = noisy && actors::horizontal_distance(actor.translation, npc.home) < 18.0;
            if (!npc.answered && jobs.open_doors.contains(&npc.address)) || hears {
                npc.answered = true;
                npc.state = ResidentState::Answering(now + citizen.delay);
                jobs.open_doors.insert(npc.address.clone());
            } else {
                continue;
            }
        }
        if let ResidentState::Answering(until) = npc.state {
            if now < until {
                continue;
            }
            npc.state = ResidentState::AtDoor(now + 8.0);
        }
        let delta = (actor.translation - transform.translation).with_y(0.0);
        let distance = delta.length();
        let hiding = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
        // Closed walls block sight. A recipient needs an open door and time to reach it.
        let outside = (transform.translation - npc.home).dot(npc.heading) > 1.4;
        let sees = outside
            && player.form == Avatar::Courier
            && concerned
            && distance < if hiding { 4.0 } else { 11.0 }
            && (npc.watching || transform.forward().dot(delta.normalize_or_zero()) > 0.1)
            && visible(
                &spatial,
                transform.translation + Vec3::Y * 1.5,
                actor.translation + Vec3::Y * 0.45,
                player_entity,
            );
        npc.watching = sees;
        if sees {
            npc.last_seen = actor.translation.with_y(-0.04);
            npc.memory_until = now + 3.0;
            if correct_drop {
                // An ordinary delivery is not a reason to stalk the courier.
                npc.state = ResidentState::AtDoor(now + 8.0);
            } else {
                npc.state = ResidentState::Chasing;
                witnessed |= distance <= HANDOVER_DISTANCE;
            }
        }
        if npc.state == ResidentState::Chasing && !sees && now > npc.memory_until {
            npc.state = ResidentState::Returning;
        }
        pursued |= npc.state == ResidentState::Chasing;
        if let ResidentState::AtDoor(until) = npc.state {
            if now > until {
                npc.state = ResidentState::Returning;
            }
        }
        if npc.state == ResidentState::Returning
            && actors::horizontal_distance(transform.translation, npc.doorstep) <= 0.22
        {
            npc.state = ResidentState::Entering;
        }
        let target = match npc.state {
            ResidentState::Inside | ResidentState::Answering(_) => npc.home,
            ResidentState::AtDoor(_) => npc.doorstep,
            ResidentState::Chasing => npc.last_seen,
            // Always route through the doorway, never take a diagonal shortcut through a wall.
            ResidentState::Returning => npc.doorstep,
            ResidentState::Entering => npc.home,
        };
        let offset = (target - transform.translation).with_y(0.0);
        let stop = if npc.state == ResidentState::Chasing && sees {
            1.5
        } else {
            0.10
        };
        if offset.length() > stop {
            let direction = offset.normalize();
            let speed = if npc.state == ResidentState::Chasing {
                2.1
            } else {
                1.2
            };
            let step = (speed * time.delta_secs()).min(offset.length());
            if visible(
                &spatial,
                transform.translation + Vec3::Y * 0.8,
                transform.translation + Vec3::Y * 0.8 + direction * (step + 0.18),
                player_entity,
            ) {
                transform.translation += direction * step;
                let facing = Transform::IDENTITY.looking_to(direction, Vec3::Y).rotation;
                transform.rotation = transform
                    .rotation
                    .slerp(facing, 1.0 - (-10.0 * time.delta_secs()).exp());
            }
        } else if npc.state == ResidentState::Entering {
            npc.state = ResidentState::Inside;
            if let Some(parcel) = npc.held.take() {
                commands.entity(parcel).despawn();
                commands.entity(entity).remove::<actors::HoldingParcel>();
            }
            transform.look_to(npc.heading, Vec3::Y);
        }
    }
    if witnessed && !jobs.caught {
        jobs.caught = true;
        jobs.status = "Caught. Hand it over for 0 KPI, or lose the recipient.".into();
    }
    jobs.pursued = pursued;
}

pub(crate) fn courier(
    mut commands: Commands,
    ride: Res<Ride>,
    mut jobs: ResMut<Jobs>,
    shift: Res<crate::shift::Shift>,
    models: Res<Models>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    clock: Res<Time<Virtual>>,
    mut actors: Query<(Entity, &mut Player, &Transform)>,
    buildings: Query<&BuildingLink>,
    vans: Query<&Transform, With<ParkedVan>>,
    camera: Query<&Transform, With<Camera3d>>,
    mut residents: Query<(Entity, &mut Resident, &Transform)>,
    spatial: SpatialQuery,
) {
    if ride.role != Role::Courier {
        return;
    }
    let Ok((actor_entity, mut player, actor)) = actors.single_mut() else {
        return;
    };
    jobs.prompt.clear();
    jobs.photo_ready = false;
    if !shift.ready || shift.ended() {
        jobs.prompt = if shift.ended() {
            "17:00 · Next day starting… Same neighbourhood, fresh parcels."
        } else {
            "Waiting for the shared shift clock…"
        }
        .into();
        return;
    }
    if jobs.interaction_active() {
        return;
    }
    if let Some((_, loaded)) = jobs.service.bulk {
        jobs.prompt = if loaded {
            "Follow the marker · [E] Dump the whole batch"
        } else {
            "Return to your van · [E] Collect the whole batch"
        }
        .into();
        return;
    }
    if jobs
        .proof_pending
        .is_some_and(|deadline| clock.elapsed_secs() > deadline)
    {
        jobs.proof_pending = None;
        jobs.status = "Photo timed out. Try again.".into();
    }
    if keys.just_pressed(KeyCode::Tab)
        && !keys.any_pressed([
            KeyCode::SuperLeft,
            KeyCode::SuperRight,
            KeyCode::ControlLeft,
            KeyCode::ControlRight,
            KeyCode::AltLeft,
            KeyCode::AltRight,
        ])
    {
        jobs.step_delivery(
            if keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]) {
                -1
            } else {
                1
            },
        );
    }
    if player.form != Avatar::Courier || clock.elapsed_secs() < player.stunned_until {
        return;
    }
    let index = jobs.selected;
    let Some(parcel) = jobs.parcels.get(index) else {
        return;
    };
    let state = parcel.state;
    if matches!(state, ParcelState::Flying { .. }) {
        jobs.prompt = "Parcel in flight · Wait for it to land, then [F] take a photo.".into();
        return;
    }
    if keys.just_pressed(KeyCode::KeyT) && matches!(state, ParcelState::Carrying) && !player.photo {
        if !jobs.service.visited.contains(&parcel.address) {
            jobs.status =
                "Ring the recipient first: R at the marked intercom, then click their bell.".into();
            return;
        }
        let forward = actor.forward().as_vec3();
        let hand = actor.translation + Vec3::Y * 0.48;
        let shape = Collider::cuboid(0.48, 0.30, 0.37);
        if spatial
            .cast_shape(
                &shape,
                hand,
                actor.rotation,
                Dir3::new(forward).unwrap(),
                &ShapeCastConfig::from_max_distance(0.85),
                &SpatialQueryFilter::from_excluded_entities([actor_entity]),
            )
            .is_some()
        {
            jobs.status = "No room to throw. Step away from the wall or van.".into();
            return;
        }
        let start = hand + forward * 0.7;
        let root = commands
            .spawn((
                Transform::from_translation(start).with_rotation(actor.rotation),
                Visibility::default(),
                DespawnOnExit(Mode::Ride),
                RigidBody::Dynamic,
                shape,
                SweptCcd::default(),
                LinearVelocity(forward * 9.0 + Vec3::Y * 3.8),
                AngularVelocity(actor.right().as_vec3() * 6.0),
                LinearDamping(0.35),
                AngularDamping(2.0),
                Friction::new(0.8),
                Restitution::new(0.18),
                FlyingParcel {
                    index,
                    still_for: 0.0,
                },
            ))
            .id();
        models.parcel(&mut commands, root);
        jobs.parcels[index].prop = Some(root);
        jobs.parcels[index].dropped_at = None;
        jobs.parcels[index].state = ParcelState::Flying { position: start };
        jobs.status = "Parcel launched. Wait for it to land, then F and Space for proof.".into();
        return;
    }
    if keys.just_pressed(KeyCode::KeyQ) && matches!(state, ParcelState::Carrying) {
        if player
            .parked
            .and_then(|e| vans.get(e).ok())
            .is_some_and(|van| {
                actors::horizontal_distance(actor.translation, van.translation) <= 3.7
            })
        {
            jobs.parcels[index].state = ParcelState::InVan;
            jobs.selected = jobs.navigation;
            jobs.caught = false;
            jobs.pursued = false;
            jobs.status = "Parcel returned. Choose a stop, then E to collect its parcel.".into();
        } else {
            jobs.status = "Go back to your van to return this parcel [Q].".into();
        }
        return;
    }
    let active = matches!(state, ParcelState::Carrying | ParcelState::Dropped { .. });
    let receiver = residents
        .iter()
        .filter(|(_, npc, transform)| {
            active
                && npc.watching
                && npc.settled != Some(index)
                && (npc.address == parcel.address
                    || parcel.dropped_at.as_ref() == Some(&npc.address))
                && clock.elapsed_secs() >= npc.stunned_until
                && actors::horizontal_distance(actor.translation, transform.translation)
                    <= HANDOVER_DISTANCE
                && visible(
                    &spatial,
                    actor.translation + Vec3::Y * 0.5,
                    transform.translation + Vec3::Y * 1.2,
                    actor_entity,
                )
        })
        .min_by(|a, b| {
            a.2.translation
                .distance_squared(actor.translation)
                .total_cmp(&b.2.translation.distance_squared(actor.translation))
        })
        .map(|(entity, _, _)| entity);
    let within_reach = match state {
        ParcelState::Carrying => true,
        ParcelState::Dropped { position, .. } => {
            actors::horizontal_distance(actor.translation, position) <= DROP_DISTANCE
                && visible(
                    &spatial,
                    actor.translation + Vec3::Y * 0.4,
                    position,
                    actor_entity,
                )
        }
        _ => false,
    };
    if receiver.is_some() && within_reach {
        jobs.prompt = "[E] Hand over parcel  |  0 KPI  |  No photo needed".into();
    } else if receiver.is_some() {
        jobs.prompt = "Parcel left behind  |  Return to the box to hand it over".into();
    } else if jobs.pursued {
        jobs.prompt = "SPOTTED  |  Break sight for 3 seconds, or approach and [E] hand over".into();
    } else {
        jobs.prompt = match state {
            ParcelState::Carrying => {
                "[E] Leave parcel at a door  |  [T] Throw  |  Wrong address + photo = 100 KPI"
            }
            ParcelState::Flying { .. } => "Wait for the parcel to land",
            ParcelState::Dropped { .. } if within_reach => "[F] Camera  |  [E] Pick parcel back up",
            ParcelState::Dropped { .. } => {
                "[F] Camera  |  Frame the parcel, then [Space] take photo"
            }
            ParcelState::InVan => {
                "Collect a parcel at your van with [E]  |  Choose a delivery with [Tab / Shift+Tab]"
            }
            ParcelState::Filed(_) | ParcelState::Handed => {
                if jobs.complete() {
                    "SHIFT COMPLETE  |  All parcels delivered  |  [M] Choose a new ride"
                } else {
                    "Delivery complete  |  [Tab / Shift+Tab] Choose your next parcel"
                }
            }
        }
        .into();
    }
    // Giving up also works from camera mode, and cancels an in-flight photo.
    if (keys.just_pressed(KeyCode::KeyE) || jobs.service.dialogue_result.is_some())
        && within_reach
        && let Some(receiver) = receiver
    {
        let bonus = jobs.service.dialogue_result.take().unwrap_or(0);
        jobs.service.bonus += bonus;
        let prop = jobs.parcels[index].prop.take().unwrap_or_else(|| {
            let root = commands
                .spawn((Transform::default(), Visibility::default()))
                .id();
            models.parcel(&mut commands, root);
            root
        });
        commands
            .entity(prop)
            .insert((ChildOf(receiver), Transform::from_xyz(0.0, 1.04, -0.39)));
        for (entity, mut npc, _) in &mut residents {
            if npc.watching || npc.state == ResidentState::Chasing || entity == receiver {
                npc.settled = Some(index);
                npc.watching = false;
                npc.state = ResidentState::Returning;
                npc.memory_until = 0.0;
            }
            if entity == receiver {
                npc.held = Some(prop);
                commands.entity(entity).insert(actors::HoldingParcel);
            }
        }
        jobs.parcels[index].state = ParcelState::Handed;
        player.photo = false;
        commands.entity(player.visual).insert(Visibility::Inherited);
        jobs.proof_pending = None;
        jobs.pursued = false;
        jobs.status = if jobs.complete() {
            "Handed to recipient | 0 KPI | Shift complete."
        } else {
            "Handed to recipient | 0 KPI | No photo required."
        }
        .into();
        jobs.prompt = jobs.status.clone();
        if bonus > 0 {
            jobs.status = format!("Name mangled, parcel handed over · +{bonus} KPI");
        }
        jobs.advance_after_delivery(index);
        return;
    }
    if keys.just_pressed(KeyCode::KeyE) && !player.photo {
        let parcel = &jobs.parcels[index];
        match parcel.state {
            ParcelState::InVan => {
                if player
                    .parked
                    .and_then(|entity| vans.get(entity).ok())
                    .is_some_and(|van| {
                        actors::horizontal_distance(actor.translation, van.translation) <= 3.7
                    })
                {
                    jobs.parcels[index].state = ParcelState::Carrying;
                    jobs.caught = false;
                    jobs.status = "Parcel collected. Follow the marker, press R and click the recipient’s bell.".into();
                } else {
                    jobs.status = "Get a parcel from your van.".into();
                }
            }
            ParcelState::Carrying => {
                if !jobs.service.visited.contains(&parcel.address) {
                    jobs.status =
                        "Ring your recipient first. Approach the marked intercom and press R."
                            .into();
                    return;
                }
                let nearest = buildings
                    .iter()
                    .filter(|building| !building.accessory)
                    .filter_map(|building| {
                        let distance =
                            actors::horizontal_distance(actor.translation, jobs.doorstep(building));
                        (distance <= DROP_DISTANCE).then_some((building, distance))
                    })
                    .min_by(|a, b| a.1.total_cmp(&b.1));
                if let Some((building, _)) = nearest {
                    let wrong_address = building.id != parcel.address;
                    let position = (jobs.doorstep(building) + building.outward * 0.25).with_y(0.17);
                    let root = commands
                        .spawn((
                            Transform::from_translation(position),
                            Visibility::default(),
                            DespawnOnExit(Mode::Ride),
                        ))
                        .id();
                    models.parcel(&mut commands, root);
                    jobs.parcels[index].prop = Some(root);
                    jobs.parcels[index].dropped_at = Some(building.id.clone());
                    jobs.parcels[index].state = ParcelState::Dropped {
                        position,
                        wrong_address,
                    };
                    jobs.status = "Parcel down. [F] camera, aim at the box, [Space] photo.".into();
                    if jobs.open_doors.insert(building.id.clone()) {
                        jobs.emit(
                            &ride,
                            StreetAction::Door {
                                building_id: building.id.clone(),
                            },
                        );
                    }
                } else {
                    jobs.status = "Move closer to a doorstep.".into();
                }
            }
            ParcelState::Dropped { .. } if within_reach => {
                if let Some(prop) = jobs.parcels[index].prop.take() {
                    commands.entity(prop).despawn();
                }
                jobs.parcels[index].state = ParcelState::Carrying;
                jobs.parcels[index].dropped_at = None;
                jobs.proof_pending = None;
                jobs.status = "Parcel picked up. [E] at a door or beside the recipient.".into();
            }
            _ => {}
        }
    }
    if !player.photo {
        return;
    }
    let ParcelState::Dropped {
        position,
        wrong_address,
    } = jobs.parcels[index].state
    else {
        jobs.prompt = if matches!(
            jobs.parcels[index].state,
            ParcelState::Filed(_) | ParcelState::Handed
        ) {
            "Delivery complete  |  [F] Put camera away  |  [Tab / Shift+Tab] Next parcel"
        } else {
            "[F] Put camera away  |  Leave the parcel at a door first"
        }
        .into();
        return;
    };
    let Ok(camera) = camera.single() else {
        return;
    };
    let delta = position - camera.translation;
    jobs.photo_ready = delta.length() <= PHOTO_DISTANCE
        && camera.forward().dot(delta.normalize_or_zero()) >= 0.976
        && visible(&spatial, camera.translation, position, actor_entity);
    let photo_hint = if jobs.proof_pending.is_some() {
        "Saving photo..."
    } else if jobs.photo_ready {
        "PARCEL IN FRAME  |  [Space / click] Take photo  |  [F] Back"
    } else if delta.length() > PHOTO_DISTANCE {
        "Move closer to the parcel (under 9 m)  |  [F] Back"
    } else {
        "Aim at the box with the centre crosshair  |  [F] Back"
    };
    if receiver.is_some() && within_reach {
        jobs.prompt = format!("[E] Hand over for 0 KPI\n{photo_hint}");
    } else {
        jobs.prompt = photo_hint.into();
    }
    if !ride.mouse_look {
        jobs.prompt
            .push_str("\nClick the game to enable mouse aiming");
    }
    if jobs.proof_pending.is_some()
        || !((ride.mouse_look && mouse.just_pressed(MouseButton::Left))
            || keys.just_pressed(KeyCode::Space))
    {
        return;
    }
    if !jobs.photo_ready {
        jobs.status = "Photo not taken: centre the parcel within 9 m, with a clear view.".into();
        return;
    }
    let score = photo_score(wrong_address, jobs.caught);
    jobs.proof_pending = Some(clock.elapsed_secs() + 4.0);
    let token = jobs.proof_pending;
    jobs.status = "Saving photo…".into();
    commands
        .spawn((Screenshot::primary_window(), DespawnOnExit(Mode::Ride)))
        .observe(
            move |captured: On<ScreenshotCaptured>,
                  jobs: Option<ResMut<Jobs>>,
                  mut players: Query<&mut Player>,
                  mut commands: Commands| {
                let Some(mut jobs) = jobs else {
                    return;
                };
                if jobs.proof_pending != token
                    || !matches!(jobs.parcels[index].state, ParcelState::Dropped { .. })
                {
                    return;
                }
                jobs.proof_pending = None;
                #[cfg(not(target_arch = "wasm32"))]
                let saved = {
                    let stamp = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_millis();
                    let default_directory =
                        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../photos");
                    let directory = jobs.proof_dir.as_deref().unwrap_or(&default_directory);
                    let path =
                        directory.join(format!("proof-{}-{stamp}-{index}.png", std::process::id()));
                    fs::create_dir_all(directory).is_ok()
                        && captured
                            .image
                            .clone()
                            .try_into_dynamic()
                            .is_ok_and(|image| image.save(&path).is_ok())
                };
                #[cfg(target_arch = "wasm32")]
                let saved = crate::platform::save_photo(&captured.image, index);
                if saved {
                    jobs.parcels[index].state = ParcelState::Filed(score);
                    if let Ok(mut player) = players.single_mut() {
                        player.photo = false;
                        commands.entity(player.visual).insert(Visibility::Inherited);
                    }
                    jobs.status = if jobs.complete() {
                        format!("Photo filed · {score:+} KPI · shift complete")
                    } else {
                        format!("Photo filed · {score:+} KPI")
                    };
                    jobs.advance_after_delivery(index);
                } else {
                    jobs.status = "Photo could not be saved. Try again.".into();
                }
            },
        );
}

pub(crate) fn settle_thrown(
    time: Res<Time>,
    mut commands: Commands,
    mut jobs: ResMut<Jobs>,
    buildings: Query<&BuildingLink>,
    mut flying: Query<(
        Entity,
        &mut FlyingParcel,
        &Transform,
        &LinearVelocity,
        &AngularVelocity,
    )>,
) {
    for (entity, mut flight, transform, linear, angular) in &mut flying {
        let Some(parcel) = jobs.parcels.get(flight.index) else {
            continue;
        };
        if parcel.prop != Some(entity) || !matches!(parcel.state, ParcelState::Flying { .. }) {
            continue;
        }
        let position = transform.translation;
        if !position.is_finite() || position.y < -10.0 {
            jobs.parcels[flight.index].state = ParcelState::InVan;
            jobs.parcels[flight.index].prop = None;
            commands.entity(entity).despawn();
            jobs.status = "Parcel recovered to your van. Collect it with E.".into();
            continue;
        }
        jobs.parcels[flight.index].state = ParcelState::Flying { position };
        if linear.0.length_squared() < 0.09 && angular.0.length_squared() < 0.09 {
            flight.still_for += time.delta_secs();
        } else {
            flight.still_for = 0.0;
        }
        if flight.still_for < 0.3 {
            continue;
        }
        let nearby = buildings
            .iter()
            .filter(|b| !b.accessory)
            .filter(|b| position.distance(jobs.doorstep(b).with_y(0.17)) <= DROP_DISTANCE)
            .min_by(|a, b| {
                position
                    .distance_squared(a.entrance)
                    .total_cmp(&position.distance_squared(b.entrance))
            })
            .map(|b| b.id.clone());
        let parcel = &mut jobs.parcels[flight.index];
        let wrong_address = nearby.as_ref() != Some(&parcel.address);
        parcel.dropped_at = nearby.clone();
        parcel.state = ParcelState::Dropped {
            position,
            wrong_address,
        };
        // A resting proof prop must not occlude its own photo ray or move after filing.
        commands.entity(entity).remove::<(
            FlyingParcel,
            RigidBody,
            Collider,
            LinearVelocity,
            AngularVelocity,
        )>();
        if let Some(address) = nearby {
            jobs.open_doors.insert(address);
        }
        jobs.status = "Parcel landed. F opens the camera; frame the box and press Space.".into();
    }
}

fn photo_score(wrong_address: bool, caught: bool) -> i32 {
    match (wrong_address, caught) {
        (true, false) => 100,
        (true, true) => 25,
        (false, _) => 0,
    }
}

#[derive(Component)]
pub(crate) struct ParcelInHands;

pub(crate) fn carry_visual(
    mut commands: Commands,
    jobs: Res<Jobs>,
    models: Res<Models>,
    player: Query<&Player>,
    props: Query<Entity, With<ParcelInHands>>,
) {
    let Ok(player) = player.single() else {
        return;
    };
    let carrying = player.form == Avatar::Courier
        && jobs
            .parcels
            .get(jobs.selected)
            .is_some_and(|parcel| matches!(parcel.state, ParcelState::Carrying));
    if carrying && props.is_empty() {
        let root = commands
            .spawn((
                ParcelInHands,
                ChildOf(player.visual),
                Transform::from_xyz(0.0, 1.04, -0.39),
                Visibility::default(),
            ))
            .id();
        models.parcel(&mut commands, root);
        commands.entity(player.visual).insert(actors::HoldingParcel);
    } else if !carrying {
        for entity in &props {
            commands.entity(entity).despawn();
        }
        commands
            .entity(player.visual)
            .remove::<actors::HoldingParcel>();
    }
}

pub(crate) fn dog(
    ride: Res<Ride>,
    mut jobs: ResMut<Jobs>,
    keys: Res<ButtonInput<KeyCode>>,
    clock: Res<Time<Virtual>>,
    mut players: Query<(&Player, &Transform, &mut LinearVelocity)>,
    remotes: Query<(&RemoteActor, &Transform)>,
    mut npcs: Query<(&mut Resident, &Transform)>,
) {
    let Ok((player, actor, mut velocity)) = players.single_mut() else {
        return;
    };
    if player.form != Avatar::Dog {
        return;
    }
    if keys.just_pressed(KeyCode::Space) && clock.elapsed_secs() >= jobs.service.dog_ready {
        velocity.0.y = 4.0;
        jobs.service.dog_ready = clock.elapsed_secs() + 1.0;
    }
    if keys.just_pressed(KeyCode::KeyE) && clock.elapsed_secs() >= jobs.service.dog_ready {
        if let Some(room) = &jobs.room
            && !jobs.open_doors.contains(&room.address)
            && actors::horizontal_distance(room.threshold(), actor.translation) < 1.5
        {
            let address = room.address.clone();
            jobs.scratches += 1;
            if jobs.scratches >= 3 {
                jobs.open_doors.insert(address.clone());
                jobs.emit(
                    &ride,
                    StreetAction::Door {
                        building_id: address,
                    },
                );
                jobs.status = "Door latch gave way.".into();
            } else {
                jobs.status = format!("Door rattles · {}/3", jobs.scratches);
            }
        } else if let Some((remote, _)) = remotes.iter().find(|(remote, target)| {
            remote.form == Avatar::Courier
                && actors::horizontal_distance(target.translation, actor.translation) < 1.55
        }) {
            velocity.0.y = 4.0;
            jobs.service.dog_ready = clock.elapsed_secs() + 2.5;
            jobs.service.bonus += 20;
            jobs.emit(
                &ride,
                StreetAction::Bite {
                    target: remote.device.clone(),
                },
            );
            jobs.status = "Courier bit. Run.".into();
        } else if let Some((mut npc, _)) = npcs.iter_mut().find(|(_, target)| {
            actors::horizontal_distance(target.translation, actor.translation) < 1.55
        }) {
            velocity.0.y = 4.0;
            jobs.service.dog_ready = clock.elapsed_secs() + 2.5;
            npc.stunned_until = clock.elapsed_secs() + 3.0;
            jobs.status = "Resident stopped.".into();
        }
    }
    if keys.just_pressed(KeyCode::KeyG) {
        if let Some((remote, point)) = remotes
            .iter()
            .filter_map(|(remote, transform)| {
                let point = remote.van_position(transform)?;
                (actors::horizontal_distance(point, actor.translation) <= 2.7)
                    .then_some((remote, point))
            })
            .min_by(|a, b| {
                actors::horizontal_distance(a.1, actor.translation)
                    .total_cmp(&actors::horizontal_distance(b.1, actor.translation))
            })
        {
            if !jobs.marks.contains_key(&remote.device) {
                jobs.service.bonus += 25;
            }
            jobs.marks.insert(remote.device.clone(), point);
            jobs.emit(
                &ride,
                StreetAction::MarkVan {
                    owner: remote.device.clone(),
                    east: point.x,
                    north: -point.z,
                },
            );
            jobs.status = "Van marked on everyone's radar.".into();
        } else {
            jobs.status = "Get closer to a courier van.".into();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::ride_tests::{headless_ride, press};

    #[test]
    fn next_day_refills_parcels_without_restarting_the_world_or_skipping_a_day() {
        let mut app = headless_ride(Role::Courier);
        let origin = app.world().resource::<Ride>().origin;
        let count = app.world().resource::<Jobs>().parcels.len();
        {
            let mut jobs = app.world_mut().resource_mut::<Jobs>();
            for p in &mut jobs.parcels {
                p.state = ParcelState::Filed(100);
            }
            jobs.service.visited.insert("old-door".into());
            jobs.service.bonus = 25;
        }
        app.update();
        assert!(app.world().resource::<crate::shift::Shift>().ended());
        app.world_mut()
            .resource_mut::<crate::shift::Shift>()
            .receive_snapshot(2, 0, 1, Some(origin.into()), vec![], 0);
        app.update();
        app.update();
        let jobs = app.world().resource::<Jobs>();
        assert_eq!(jobs.day(), 2);
        assert_eq!(jobs.parcels.len(), count);
        assert!(jobs.parcels.iter().all(|p| p.state == ParcelState::InVan));
        assert!(jobs.service.visited.is_empty());
        assert_eq!(jobs.progress()["score"], 0);
        assert_eq!(app.world().resource::<crate::shift::Shift>().minute(), 540);
        assert_eq!(app.world().resource::<Ride>().origin, origin);
    }

    fn player(app: &mut App) -> Entity {
        let world = app.world_mut();
        world
            .query_filtered::<Entity, With<Player>>()
            .single(world)
            .unwrap()
    }

    fn meet_resident(app: &mut App, address: String, distance: f32) -> Entity {
        let player = player(app);
        let until = app.world().resource::<Time<Virtual>>().elapsed_secs() + 30.0;
        let (resident, doorstep, heading) = {
            let world = app.world_mut();
            let (entity, npc) = world
                .query::<(Entity, &Resident)>()
                .iter(world)
                .find(|(_, npc)| npc.address == address)
                .unwrap();
            (entity, npc.doorstep, npc.heading)
        };
        app.world_mut()
            .resource_mut::<Jobs>()
            .open_doors
            .insert(address);
        let mut entity = app.world_mut().entity_mut(resident);
        *entity.get_mut::<Transform>().unwrap() =
            Transform::from_translation(doorstep).looking_to(heading, Vec3::Y);
        let mut npc = entity.get_mut::<Resident>().unwrap();
        npc.state = ResidentState::AtDoor(until);
        npc.answered = true;
        *app.world_mut().get_mut::<Transform>(player).unwrap() =
            Transform::from_translation((doorstep + heading * distance).with_y(0.83))
                .looking_to(-heading, Vec3::Y);
        app.update();
        resident
    }

    fn ring_selected(app: &mut App) {
        let actor = player(app);
        let previous = *app.world().get::<Transform>(actor).unwrap();
        let point =
            app.world().resource::<Jobs>().parcels[app.world().resource::<Jobs>().selected].point;
        app.world_mut()
            .get_mut::<Transform>(actor)
            .unwrap()
            .translation = point.with_y(0.83);
        press(app, KeyCode::KeyR);
        assert!(app.world().resource::<Jobs>().interaction_active());
        let button = {
            let world = app.world_mut();
            world
                .query::<(Entity, &service::Action)>()
                .iter(world)
                .find(|(_, a)| matches!(a, service::Action::Bell(0)))
                .unwrap()
                .0
        };
        app.world_mut().entity_mut(button).insert(bevy::ui::Pressed);
        app.update();
        let close = {
            let world = app.world_mut();
            world
                .query::<(Entity, &service::Action)>()
                .iter(world)
                .find(|(_, a)| matches!(a, service::Action::Close))
                .unwrap()
                .0
        };
        app.world_mut().entity_mut(close).insert(bevy::ui::Pressed);
        app.update();
        *app.world_mut().get_mut::<Transform>(actor).unwrap() = previous;
    }

    #[test]
    fn thrown_parcel_requires_a_bell_hits_walls_lands_and_can_be_recovered() {
        let mut app = headless_ride(Role::Courier);
        let actor = player(&mut app);
        press(&mut app, KeyCode::KeyE);
        press(&mut app, KeyCode::KeyT);
        assert_eq!(
            app.world().resource::<Jobs>().parcels[0].state,
            ParcelState::Carrying
        );
        assert!(app.world().resource::<Jobs>().status.contains("Ring"));
        ring_selected(&mut app);
        // Isolate a real physical wall and floor from the neighbourhood's incidental layout.
        let blockers: Vec<_> = app.world_mut().query_filtered::<Entity, (With<Collider>, Without<Player>, Without<crate::WorldFloor>)>()
            .iter(app.world()).collect();
        for entity in blockers {
            app.world_mut().entity_mut(entity).remove::<Collider>();
        }
        let residents: Vec<_> = app
            .world_mut()
            .query_filtered::<Entity, With<Resident>>()
            .iter(app.world())
            .collect();
        for entity in residents {
            app.world_mut().entity_mut(entity).remove::<Resident>();
        }
        let start = *app.world().get::<Transform>(actor).unwrap();
        let forward = start.forward().as_vec3();
        app.world_mut().spawn((
            RigidBody::Static,
            Collider::cuboid(12.0, 6.0, 0.2),
            Transform::from_translation((start.translation + forward * 4.0).with_y(3.0))
                .with_rotation(start.rotation),
        ));
        app.update();
        press(&mut app, KeyCode::KeyT);
        assert!(matches!(
            app.world().resource::<Jobs>().parcels[0].state,
            ParcelState::Flying { .. }
        ));
        press(&mut app, KeyCode::Tab);
        assert_eq!(app.world().resource::<Jobs>().selected, 0);
        assert_eq!(app.world().resource::<Jobs>().navigation, 1);
        for _ in 0..400 {
            app.update();
            if matches!(
                app.world().resource::<Jobs>().parcels[0].state,
                ParcelState::Dropped { .. }
            ) {
                break;
            }
        }
        let jobs = app.world().resource::<Jobs>();
        let ParcelState::Dropped { position, .. } = jobs.parcels[0].state else {
            panic!("throw did not settle: {:?}", jobs.parcels[0].state);
        };
        assert!(
            (position - start.translation).dot(forward) < 3.9,
            "box passed through the wall"
        );
        let prop = jobs.parcels[0].prop.unwrap();
        assert!(
            app.world().get::<Collider>(prop).is_none(),
            "resting prop must not block its own photo"
        );
        app.world_mut()
            .get_mut::<Transform>(actor)
            .unwrap()
            .translation = (position - forward * 0.8).with_y(0.87);
        app.update();
        press(&mut app, KeyCode::KeyE);
        assert_eq!(
            app.world().resource::<Jobs>().parcels[0].state,
            ParcelState::Carrying
        );
        assert!(app.world().get_entity(prop).is_err());
    }

    #[test]
    fn bite_causes_one_startle_jump_and_blocks_camera_until_recovery() {
        let mut app = headless_ride(Role::Courier);
        let actor = player(&mut app);
        for _ in 0..10 {
            app.update();
        }
        let before = app.world().get::<Transform>(actor).unwrap().translation.y;
        let now = app.world().resource::<Time<Virtual>>().elapsed_secs();
        app.world_mut()
            .get_mut::<Player>(actor)
            .unwrap()
            .stunned_until = now + 1.2;
        press(&mut app, KeyCode::KeyF);
        assert!(!app.world().get::<Player>(actor).unwrap().photo);
        assert!(app.world().get::<LinearVelocity>(actor).unwrap().0.y > 2.0);
        let mut highest = before;
        for _ in 0..110 {
            app.update();
            highest = highest.max(app.world().get::<Transform>(actor).unwrap().translation.y);
        }
        assert!(highest > before + 0.3, "bite must visibly lift the courier");
        assert!(
            app.world().get::<Transform>(actor).unwrap().translation.y < before + 0.15,
            "shock must not repeatedly relaunch the courier"
        );
        press(&mut app, KeyCode::KeyF);
        assert!(app.world().get::<Player>(actor).unwrap().photo);
    }

    #[test]
    fn automatic_route_skips_completed_stops_and_wraps_without_losing_pending_photos() {
        let mut app = crate::ride_tests::headless_ride(Role::Courier);
        let mut jobs = app.world_mut().resource_mut::<Jobs>();
        assert!(jobs.parcels.len() >= 3);
        for parcel in &mut jobs.parcels {
            parcel.state = ParcelState::Handed;
        }
        let last = jobs.parcels.len() - 1;
        jobs.parcels[0].state = ParcelState::InVan;
        jobs.selected = last;
        jobs.navigation = last;
        jobs.parcels[last].state = ParcelState::Dropped {
            position: Vec3::ZERO,
            wrong_address: false,
        };
        jobs.advance_after_delivery(last);
        assert_eq!(
            jobs.selected, last,
            "a dropped box still requires a saved photo"
        );
        jobs.parcels[last].state = ParcelState::Filed(0);
        jobs.advance_after_delivery(last);
        assert_eq!(jobs.selected, 0, "wrap to the unfinished first stop");
        assert_eq!(jobs.navigation, 0);
        jobs.parcels[0].state = ParcelState::Handed;
        jobs.advance_after_delivery(0);
        assert!(jobs.complete());
        assert_eq!(jobs.selected, 0, "a finished route has no next parcel");
    }

    #[test]
    fn route_preview_preserves_the_held_parcel_and_can_swap_at_the_van() {
        let mut app = crate::ride_tests::headless_ride(Role::Courier);
        press(&mut app, KeyCode::KeyV);
        press(&mut app, KeyCode::KeyV);
        press(&mut app, KeyCode::KeyE);
        assert_eq!(
            app.world().resource::<Jobs>().parcels[0].state,
            ParcelState::Carrying
        );
        press(&mut app, KeyCode::Tab);
        let jobs = app.world().resource::<Jobs>();
        assert_eq!(jobs.selected, 0);
        assert_eq!(jobs.navigation, 1);
        assert_eq!(jobs.current_target(), Some(jobs.parcels[1].point));
        assert_eq!(jobs.parcels[0].state, ParcelState::Carrying);
        assert_eq!(jobs.parcels[1].state, ParcelState::InVan);
        press(&mut app, KeyCode::KeyQ);
        let jobs = app.world().resource::<Jobs>();
        assert_eq!(jobs.selected, 1);
        assert_eq!(jobs.parcels[0].state, ParcelState::InVan);
        press(&mut app, KeyCode::KeyE);
        assert_eq!(
            app.world().resource::<Jobs>().parcels[1].state,
            ParcelState::Carrying
        );
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::ShiftLeft);
        press(&mut app, KeyCode::Tab);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .reset(KeyCode::ShiftLeft);
        assert_eq!(app.world().resource::<Jobs>().navigation, 0);
        assert_eq!(app.world().resource::<Jobs>().selected, 1);
    }

    #[test]
    fn streamed_neighbourhoods_fill_twelve_stops_without_duplicates() {
        use bevy::ecs::system::RunSystemOnce;
        let mut app = crate::ride_tests::headless_ride(Role::Courier);
        let scene = paket_geo::parse_overpass(
            include_bytes!("../../../fixtures/koertestrasse.json"),
            paket_geo::LatLon {
                lat: 52.4895,
                lon: 13.4084,
            },
        )
        .unwrap();
        let addresses: Vec<_> = scene
            .buildings
            .iter()
            .filter(|b| !b.accessory)
            .map(crate::building_link)
            .collect();
        app.world_mut()
            .run_system_once(
                move |mut commands: Commands, mut jobs: ResMut<Jobs>, models: Res<Models>| {
                    jobs.seed(addresses.clone(), &mut commands, &models);
                    jobs.seed(addresses.clone(), &mut commands, &models);
                },
            )
            .unwrap();
        let jobs = app.world().resource::<Jobs>();
        assert_eq!(jobs.parcels.len(), 12);
        assert_eq!(
            jobs.parcels
                .iter()
                .map(|p| &p.address)
                .collect::<HashSet<_>>()
                .len(),
            12
        );
    }

    fn frame_dropped_parcel(app: &mut App, index: usize) {
        let player = player(app);
        let jobs = app.world().resource::<Jobs>();
        let ParcelState::Dropped { position, .. } = jobs.parcels[index].state else {
            panic!("parcel must have been dropped");
        };
        let outward = jobs.room.as_ref().unwrap().direction;
        let eye = position + outward * 3.0 + Vec3::Y * 1.36;
        let forward = -outward;
        let mut entity = app.world_mut().entity_mut(player);
        *entity.get_mut::<Transform>().unwrap() =
            Transform::from_translation(eye - Vec3::Y * 0.69 - forward * 0.12)
                .looking_to(forward, Vec3::Y);
        let mut actor = entity.get_mut::<Player>().unwrap();
        actor.photo = true;
        actor.pitch = (position - eye).normalize().y.asin();
    }

    fn captured_image() -> Image {
        use bevy::{
            asset::RenderAssetUsages,
            render::render_resource::{Extent3d, TextureDimension, TextureFormat},
        };
        Image::new(
            Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            vec![255; 4],
            TextureFormat::Rgba8UnormSrgb,
            RenderAssetUsages::default(),
        )
    }

    #[test]
    fn handing_over_carried_parcel_closes_delivery_once_and_recipient_goes_home() {
        let mut app = headless_ride(Role::Courier);
        press(&mut app, KeyCode::KeyE);
        let address = app.world().resource::<Jobs>().parcels[0].address.clone();
        let resident = meet_resident(&mut app, address, 1.8);
        app.update();
        assert!(app.world().resource::<Jobs>().interaction_active());
        press(&mut app, KeyCode::Digit1);
        assert_eq!(
            app.world().resource::<Jobs>().parcels[0].state,
            ParcelState::Handed
        );
        assert!(app.world().resource::<Jobs>().manifest().contains("0 KPI"));
        let prop = app.world().get::<Resident>(resident).unwrap().held.unwrap();
        assert_eq!(app.world().get::<ChildOf>(prop).unwrap().parent(), resident);
        press(&mut app, KeyCode::KeyE);
        assert_eq!(
            app.world().resource::<Jobs>().parcels[0].state,
            ParcelState::Handed
        );
        for _ in 0..400 {
            app.update();
        }
        let npc = app.world().get::<Resident>(resident).unwrap();
        assert_eq!(npc.state, ResidentState::Inside);
        assert!(npc.held.is_none());
        assert!(app.world().get_entity(prop).is_err());
        assert!(!app.world().resource::<Jobs>().pursued);
        assert_eq!(app.world().resource::<Jobs>().selected, 1);
        assert_eq!(app.world().resource::<Jobs>().navigation, 1);
    }

    #[test]
    fn handover_of_dropped_parcel_cancels_pending_photo_and_restores_normal_camera() {
        let mut app = headless_ride(Role::Courier);
        let actor = player(&mut app);
        press(&mut app, KeyCode::Tab);
        press(&mut app, KeyCode::KeyE);
        ring_selected(&mut app);
        let (destination, address) = {
            let jobs = app.world().resource::<Jobs>();
            (jobs.parcels[0].point, jobs.parcels[0].address.clone())
        };
        app.world_mut()
            .get_mut::<Transform>(actor)
            .unwrap()
            .translation = destination.with_y(0.83);
        press(&mut app, KeyCode::KeyE);
        let prop = app.world().resource::<Jobs>().parcels[1].prop.unwrap();
        frame_dropped_parcel(&mut app, 1);
        press(&mut app, KeyCode::Space);
        assert!(app.world().resource::<Jobs>().photo_ready);
        let screenshot = {
            let world = app.world_mut();
            world
                .query_filtered::<Entity, With<Screenshot>>()
                .single(world)
                .unwrap()
        };
        let resident = meet_resident(&mut app, address, 1.8);
        app.update();
        press(&mut app, KeyCode::Digit1);
        assert_eq!(
            app.world().resource::<Jobs>().parcels[1].state,
            ParcelState::Handed
        );
        assert!(!app.world().get::<Player>(actor).unwrap().photo);
        assert_eq!(
            app.world().get::<Resident>(resident).unwrap().held,
            Some(prop)
        );
        assert!(app.world().resource::<Jobs>().proof_pending.is_none());
        app.world_mut().trigger(ScreenshotCaptured {
            entity: screenshot,
            image: captured_image(),
        });
        assert_eq!(
            app.world().resource::<Jobs>().parcels[1].state,
            ParcelState::Handed,
            "a late screenshot must not replace the zero-point handover"
        );
    }

    #[test]
    fn unrelated_resident_and_correct_delivery_do_not_start_a_chase() {
        let mut app = headless_ride(Role::Courier);
        let actor = player(&mut app);
        press(&mut app, KeyCode::KeyE);
        ring_selected(&mut app);
        let unrelated = app.world().resource::<Jobs>().parcels[1].address.clone();
        let npc = meet_resident(&mut app, unrelated, 1.8);
        assert!(!app.world().get::<Resident>(npc).unwrap().watching);
        assert!(!app.world().resource::<Jobs>().pursued);
        let (destination, address) = {
            let jobs = app.world().resource::<Jobs>();
            (jobs.parcels[0].point, jobs.parcels[0].address.clone())
        };
        app.world_mut()
            .get_mut::<Transform>(actor)
            .unwrap()
            .translation = destination.with_y(0.83);
        press(&mut app, KeyCode::KeyE);
        let npc = meet_resident(&mut app, address, 1.8);
        assert!(app.world().get::<Resident>(npc).unwrap().watching);
        assert!(matches!(
            app.world().get::<Resident>(npc).unwrap().state,
            ResidentState::AtDoor(_)
        ));
        assert!(!app.world().resource::<Jobs>().pursued);
        assert!(app.world().resource::<Jobs>().prompt.contains("Hand over"));
    }

    #[test]
    fn dropped_parcel_must_be_reachable_before_handover() {
        let mut app = headless_ride(Role::Courier);
        let actor = player(&mut app);
        press(&mut app, KeyCode::Tab);
        press(&mut app, KeyCode::KeyE);
        ring_selected(&mut app);
        let (destination, address) = {
            let jobs = app.world().resource::<Jobs>();
            (jobs.parcels[0].point, jobs.parcels[0].address.clone())
        };
        app.world_mut()
            .get_mut::<Transform>(actor)
            .unwrap()
            .translation = destination.with_y(0.83);
        press(&mut app, KeyCode::KeyE);
        let npc = meet_resident(&mut app, address, 1.8);
        // Move the dropped parcel away: proximity to its recipient cannot teleport the box.
        let state = ParcelState::Dropped {
            position: destination + Vec3::X * 30.0,
            wrong_address: true,
        };
        app.world_mut().resource_mut::<Jobs>().parcels[1].state = state;
        press(&mut app, KeyCode::KeyE);
        assert_eq!(app.world().resource::<Jobs>().parcels[1].state, state);
        assert!(app.world().get::<Resident>(npc).unwrap().held.is_none());
    }

    #[test]
    fn courier_can_drive_choose_a_parcel_and_file_wrong_address_photo() {
        use bevy::{
            asset::RenderAssetUsages,
            render::render_resource::{Extent3d, TextureDimension, TextureFormat},
        };
        let mut app = headless_ride(Role::Courier);
        let player = player(&mut app);
        press(&mut app, KeyCode::KeyV);
        assert_eq!(app.world().get::<Player>(player).unwrap().form, Avatar::Van);
        press(&mut app, KeyCode::KeyV);
        assert_eq!(
            app.world().get::<Player>(player).unwrap().form,
            Avatar::Courier
        );
        press(&mut app, KeyCode::Tab);
        press(&mut app, KeyCode::KeyE);
        assert!(matches!(
            app.world().resource::<Jobs>().parcels[1].state,
            ParcelState::Carrying
        ));

        ring_selected(&mut app);
        let destination = app.world().resource::<Jobs>().parcels[0].point;
        let mut entity = app.world_mut().entity_mut(player);
        *entity.get_mut::<Transform>().unwrap() =
            Transform::from_translation(destination.with_y(0.83));
        press(&mut app, KeyCode::KeyE);
        let jobs = app.world().resource::<Jobs>();
        let ParcelState::Dropped {
            position,
            wrong_address,
        } = jobs.parcels[1].state
        else {
            panic!("{}", jobs.status);
        };
        assert!(wrong_address);
        assert!(
            jobs.open_doors
                .contains(&jobs.room.as_ref().unwrap().address)
        );

        let outward = jobs.room.as_ref().unwrap().direction;
        let eye = position + outward * 3.0 + Vec3::Y * 1.36;
        let direction = (position - eye).normalize();
        let forward = -outward;
        let mut entity = app.world_mut().entity_mut(player);
        *entity.get_mut::<Transform>().unwrap() =
            Transform::from_translation(eye - Vec3::Y * 0.69 - forward * 0.12)
                .looking_to(forward, Vec3::Y);
        let mut actor = entity.get_mut::<Player>().unwrap();
        actor.photo = true;
        actor.pitch = direction.y.asin();
        let output = std::env::temp_dir().join(format!(
            "paket-proof-test-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        app.world_mut().resource_mut::<Jobs>().proof_dir = Some(output.clone());
        press(&mut app, KeyCode::Space);
        let world = app.world_mut();
        let screenshot = world
            .query_filtered::<Entity, With<Screenshot>>()
            .single(world)
            .unwrap_or_else(|_| panic!("{}", world.resource::<Jobs>().status));
        world.trigger(ScreenshotCaptured {
            entity: screenshot,
            image: Image::new(
                Extent3d {
                    width: 1,
                    height: 1,
                    depth_or_array_layers: 1,
                },
                TextureDimension::D2,
                vec![255; 4],
                TextureFormat::Rgba8UnormSrgb,
                RenderAssetUsages::default(),
            ),
        });
        assert!(matches!(
            app.world().resource::<Jobs>().parcels[1].state,
            ParcelState::Filed(_)
        ));
        let jobs = app.world().resource::<Jobs>();
        assert_eq!(jobs.selected, 2 % jobs.parcels.len());
        assert_eq!(jobs.navigation, jobs.selected);
        assert!(!app.world().get::<Player>(player).unwrap().photo);
        assert_eq!(fs::read_dir(&output).unwrap().count(), 1);
        fs::remove_dir_all(output).unwrap();
    }

    #[test]
    fn dog_starts_inside_and_a_couriers_door_event_releases_it() {
        let mut app = headless_ride(Role::Dog);
        let player = player(&mut app);
        let position = app.world().get::<Transform>(player).unwrap().translation;
        let jobs = app.world().resource::<Jobs>();
        assert!(jobs.dog_indoors(position));
        let address = jobs.room.as_ref().unwrap().address.clone();
        let door = {
            let world = app.world_mut();
            world
                .query::<(Entity, &Door)>()
                .iter(world)
                .find(|(_, door)| door.address == address)
                .unwrap()
                .0
        };
        assert!(app.world().get::<Collider>(door).is_some());
        app.world_mut().resource_mut::<Jobs>().receive(
            "dog",
            "courier".into(),
            1,
            StreetAction::Door {
                building_id: address,
            },
            0.0,
            None,
        );
        app.update();
        assert!(app.world().get::<ColliderDisabled>(door).is_some());
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyW);
        for _ in 0..90 {
            app.update();
        }
        let position = app.world().get::<Transform>(player).unwrap().translation;
        assert!(
            !app.world().resource::<Jobs>().dog_indoors(position),
            "dog must physically pass through the building doorway"
        );
    }

    #[test]
    fn residents_wait_inside_then_answer_their_own_door() {
        let mut app = headless_ride(Role::Courier);
        let (entity, address, threshold) = {
            let world = app.world_mut();
            let (entity, npc, transform) = world
                .query::<(Entity, &Resident, &Transform)>()
                .iter(world)
                .next()
                .unwrap();
            assert_eq!(npc.state, ResidentState::Inside);
            assert!((transform.translation - npc.doorstep).dot(npc.heading) < -2.0);
            (
                entity,
                npc.address.clone(),
                npc.doorstep - npc.heading * 1.25,
            )
        };
        app.world_mut()
            .resource_mut::<Jobs>()
            .open_doors
            .insert(address);
        app.world_mut()
            .resource_mut::<crate::shift::Shift>()
            .receive(210 * 6000, 1);
        let delay = service::citizen(&app.world().get::<Resident>(entity).unwrap().address).delay;
        for _ in 0..((delay + 3.0) / 0.016) as usize {
            app.update();
        }
        let npc = app.world().get::<Resident>(entity).unwrap();
        let position = app.world().get::<Transform>(entity).unwrap().translation;
        assert!(
            (position - threshold).dot(npc.heading) > 0.8,
            "resident should use the doorway: {position:?}"
        );
        assert!(matches!(npc.state, ResidentState::AtDoor(_)));
        for _ in 0..650 {
            app.update();
        }
        let npc = app.world().get::<Resident>(entity).unwrap();
        assert_eq!(npc.state, ResidentState::Inside);
    }

    #[test]
    fn bite_replays_do_not_extend_stun_and_radar_tracks_the_moving_van() {
        let mut jobs = Jobs::default();
        let mut actor = Player {
            form: Avatar::Courier,
            photo: false,
            pitch: 0.0,
            stunned_until: 0.0,
            visual: Entity::PLACEHOLDER,
            parked: None,
        };
        for now in [1.0, 2.0] {
            jobs.receive(
                "courier",
                "dog".into(),
                1,
                StreetAction::Bite {
                    target: "courier".into(),
                },
                now,
                Some(&mut actor),
            );
        }
        assert_eq!(actor.stunned_until, 2.5);
        jobs.receive(
            "courier",
            "dog".into(),
            2,
            StreetAction::MarkVan {
                owner: "partner".into(),
                east: 2.0,
                north: 0.0,
            },
            2.0,
            None,
        );
        let display = jobs.radar(
            Some("courier"),
            &Transform::IDENTITY,
            [("partner".into(), Vec3::new(8.0, 0.0, 0.0))].into_iter(),
        );
        assert!(display.contains("8 m"));
    }

    #[test]
    fn proof_rewards_a_wrong_address_and_penalizes_being_seen() {
        assert_eq!(photo_score(true, false), 100);
        assert!(photo_score(true, true) < photo_score(true, false));
        assert_eq!(photo_score(false, false), 0);
    }
}
