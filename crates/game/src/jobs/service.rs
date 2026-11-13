//! Doorbells, resident schedules and the small delivery conversations.
use super::*;
use bevy::{picking::hover::Hovered, ui_widgets::Button};

#[derive(Default)]
pub(super) struct Service {
    pub modal: Option<Modal>,
    pub visited: HashSet<String>,
    pub bonus: i32,
    pub dialogue_result: Option<i32>,
    conversations: HashSet<usize>,
    bell_awards: HashSet<String>,
    pub rare_used: bool,
    pub bulk: Option<(Vec3, bool)>,
    revision: u64,
    pub dog_ready: f32,
}
pub(super) struct Modal {
    pub camera: Transform,
    pub kind: ModalKind,
}
pub(super) enum ModalKind {
    Intercom {
        address: String,
        pressed: u8,
        started: Option<f32>,
        budget: f32,
        rare: bool,
    },
    Dialogue {
        recipient: Entity,
        name: String,
        expires: f32,
    },
}
pub(super) struct Citizen {
    pub name: String,
    pub delay: f32,
    away: [(u32, u32); 2],
}
impl Citizen {
    pub fn home_at(&self, minute: u32) -> bool {
        !self.away.iter().any(|&(a, b)| (a..b).contains(&minute))
    }
    fn schedule(&self) -> String {
        let clock = |m: u32| format!("{:02}:{:02}", m / 60, m % 60);
        format!(
            "Away {}–{} / {}–{}",
            clock(self.away[0].0),
            clock(self.away[0].1),
            clock(self.away[1].0),
            clock(self.away[1].1)
        )
    }
}
fn hash(id: &str) -> u32 {
    id.bytes().fold(2166136261u32, |h, b| {
        (h ^ u32::from(b)).wrapping_mul(16777619)
    })
}
pub(super) fn citizen(id: &str) -> Citizen {
    let h = hash(id);
    let first = [
        "Mika", "Anna", "Deniz", "Elena", "Alex", "Yuki", "Samira", "Bruno", "Nora", "Luca",
        "Maya", "Omar",
    ];
    let last = [
        "Schneider",
        "Kowalski",
        "Yilmaz",
        "Nguyen",
        "Müller",
        "Santos",
        "Novak",
        "Weber",
        "Tanaka",
        "Fischer",
        "Haddad",
        "Petrov",
    ];
    let away = 540 + (h % 8) * 20;
    Citizen {
        name: format!(
            "{} {}",
            first[(h % 12) as usize],
            last[((h / 12) % 12) as usize]
        ),
        delay: 4.0 + ((h / 144) % 18) as f32,
        away: [(away, away + 65), (790 + (h % 7) * 15, 850 + (h % 7) * 15)],
    }
}
#[derive(Clone, Copy, Component)]
pub(crate) enum Action {
    Bell(u8),
    Reply(u8),
    Fault,
    Close,
}
#[derive(Component)]
pub(crate) struct Panel;
#[derive(Component)]
pub(crate) struct TimerLabel;
#[derive(Component)]
pub(crate) struct Marker(u8);

pub(crate) fn service_update(
    mut commands: Commands,
    time: Res<Time<Virtual>>,
    keys: Res<ButtonInput<KeyCode>>,
    ride: Res<Ride>,
    mut shift: ResMut<crate::shift::Shift>,
    mut jobs: ResMut<Jobs>,
    actions: Query<&Action, Added<bevy::ui::Pressed>>,
    player: Query<(Entity, &Player, &Transform)>,
    buildings: Query<&BuildingLink>,
    residents: Query<(Entity, &Resident, &Transform)>,
    vans: Query<&Transform, With<ParkedVan>>,
    models: Res<Models>,
    spatial: SpatialQuery,
) {
    let Ok((actor_id, player, actor)) = player.single() else {
        return;
    };
    let now = time.elapsed_secs();
    if jobs.menu_open || now < player.stunned_until {
        return;
    }
    if shift.ended() || !shift.ready {
        if jobs.service.modal.take().is_some() {
            jobs.service.revision += 1;
        }
        return;
    }
    if player.form != Avatar::Courier {
        return;
    }
    let mut action = actions.iter().next().copied();
    if keys.just_pressed(KeyCode::Escape) {
        action = Some(Action::Close);
    }
    if matches!(
        jobs.service.modal.as_ref().map(|m| &m.kind),
        Some(ModalKind::Dialogue { .. })
    ) {
        for (i, key) in [KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3]
            .iter()
            .enumerate()
        {
            if keys.just_pressed(*key) {
                action = Some(Action::Reply(i as u8));
            }
        }
        if matches!(jobs.service.modal.as_ref().map(|m|&m.kind),Some(ModalKind::Dialogue{expires,..}) if now>=*expires)
        {
            action = Some(Action::Reply(0));
        }
    }
    if let Some(action) = action {
        if let Some(mut modal) = jobs.service.modal.take() {
            let mut keep = true;
            match (&mut modal.kind, action) {
                (ModalKind::Dialogue { .. }, Action::Reply(choice)) => {
                    jobs.service.dialogue_result = Some(if choice == 1 { 15 } else { 0 });
                    keep = false;
                }
                (ModalKind::Dialogue { .. }, Action::Close) => {
                    jobs.service.dialogue_result = Some(0);
                    keep = false;
                }
                (
                    ModalKind::Intercom {
                        address,
                        pressed,
                        started,
                        budget,
                        ..
                    },
                    Action::Bell(i),
                ) if i < 6 => {
                    shift.locked = true;
                    let start = *started.get_or_insert(now);
                    *pressed |= 1 << i;
                    if i == 0 {
                        jobs.service.visited.insert(address.clone());
                        let citizen = citizen(address);
                        if citizen.home_at(shift.minute()) {
                            jobs.open_doors.insert(address.clone());
                            jobs.emit(
                                &ride,
                                StreetAction::Door {
                                    building_id: address.clone(),
                                },
                            );
                            jobs.status = format!(
                                "{} is on the way · about {:.0} seconds",
                                citizen.name, citizen.delay
                            );
                        } else {
                            jobs.status =
                                format!("No answer from {}. {}", citizen.name, citizen.schedule());
                        }
                    }
                    if *pressed == 63
                        && now - start <= *budget
                        && jobs.service.bell_awards.insert(address.clone())
                    {
                        jobs.service.bonus += 40;
                        jobs.status = "Every bell in time! +40 KPI".into();
                    }
                }
                (
                    ModalKind::Intercom {
                        address,
                        rare: true,
                        ..
                    },
                    Action::Fault,
                ) if !jobs.service.rare_used && jobs.service.visited.contains(address) => {
                    let target = buildings
                        .iter()
                        .filter(|b| {
                            !b.accessory
                                && b.id != *address
                                && b.entrance.distance(actor.translation) > 12.0
                        })
                        .min_by_key(|b| hash(&format!("{}{}", address, b.id)))
                        .map(|b| b.entrance);
                    if let Some(target) = target {
                        jobs.service.rare_used = true;
                        jobs.service.bulk = Some((target, false));
                        jobs.status = "NO ACCESS · Collect the whole batch at your van [E].".into();
                        keep = false;
                    }
                }
                (_, Action::Close) => keep = false,
                _ => {}
            }
            if keep {
                jobs.service.modal = Some(modal);
            }
            jobs.service.revision += 1;
        }
    }
    if jobs.service.modal.is_some() {
        return;
    }
    if let Some((target, collected)) = jobs.service.bulk {
        if keys.just_pressed(KeyCode::KeyE) {
            if !collected
                && player
                    .parked
                    .and_then(|e| vans.get(e).ok())
                    .is_some_and(|v| {
                        actors::horizontal_distance(v.translation, actor.translation) < 3.7
                    })
            {
                jobs.service.bulk = Some((target, true));
                jobs.status =
                    "Whole batch loaded. Follow the orange marker to the dumping spot.".into();
            } else if collected && actors::horizontal_distance(target, actor.translation) < 3.4 {
                for (i, parcel) in jobs.parcels.iter_mut().enumerate() {
                    if !matches!(parcel.state, ParcelState::Filed(_) | ParcelState::Handed) {
                        if let Some(prop) = parcel.prop.take() {
                            commands.entity(prop).despawn();
                        }
                        let root = commands
                            .spawn((
                                Transform::from_translation(
                                    target
                                        + Vec3::new(
                                            (i % 2) as f32 * 0.6,
                                            0.17,
                                            (i / 2) as f32 * 0.5,
                                        ),
                                ),
                                Visibility::default(),
                                DespawnOnExit(Mode::Ride),
                            ))
                            .id();
                        models.parcel(&mut commands, root);
                        parcel.state = ParcelState::Filed(0);
                    }
                }
                jobs.service.bonus += 500;
                jobs.service.bulk = None;
                jobs.status = "DELIVERY OF THE DAY · Whole batch abandoned · +500 KPI".into();
            }
        }
        return;
    }
    let Some(parcel) = jobs.parcels.get(jobs.selected) else {
        return;
    };
    let index = jobs.selected;
    let address = parcel.address.clone();
    let active = matches!(
        parcel.state,
        ParcelState::Carrying | ParcelState::Dropped { .. }
    );
    let in_reach = match parcel.state {
        ParcelState::Dropped { position, .. } => {
            actors::horizontal_distance(position, actor.translation) < DROP_DISTANCE
        }
        _ => true,
    };
    if active
        && in_reach
        && !jobs.service.conversations.contains(&index)
        && jobs.service.dialogue_result.is_none()
    {
        if let Some((entity, npc, target)) = residents.iter().find(|(_, n, t)| {
            n.watching
                && n.settled != Some(index)
                && (n.address == address || parcel.dropped_at.as_ref() == Some(&n.address))
                && now >= n.stunned_until
                && actors::horizontal_distance(t.translation, actor.translation) < HANDOVER_DISTANCE
                && visible(
                    &spatial,
                    actor.translation + Vec3::Y * 0.5,
                    t.translation + Vec3::Y * 1.2,
                    actor_id,
                )
        }) {
            let camera = Transform::from_translation(actor.translation + Vec3::Y * 0.65)
                .looking_at(target.translation + Vec3::Y * 1.45, Vec3::Y);
            jobs.service.modal = Some(Modal {
                camera,
                kind: ModalKind::Dialogue {
                    recipient: entity,
                    name: citizen(&npc.address).name,
                    expires: now + [9., 6., 4.][shift.difficulty as usize],
                },
            });
            jobs.service.conversations.insert(index);
            jobs.service.revision += 1;
            return;
        }
    }
    if keys.just_pressed(KeyCode::KeyR) {
        let address = jobs
            .parcels
            .get(jobs.navigation)
            .map_or(address, |p| p.address.clone());
        if let Some(b) = buildings.iter().find(|b| {
            b.id == address && actors::horizontal_distance(b.entrance, actor.translation) < 4.5
        }) {
            let camera =
                Transform::from_translation(b.threshold + b.outward * 1.6 + Vec3::Y * 1.55)
                    .looking_at(b.threshold + Vec3::Y * 1.45, Vec3::Y);
            jobs.service.modal = Some(Modal {
                camera,
                kind: ModalKind::Intercom {
                    address,
                    pressed: 0,
                    started: None,
                    budget: [10., 7., 4.5][shift.difficulty as usize],
                    rare: hash(&b.id) % 7 == 0 && !jobs.service.rare_used,
                },
            });
            jobs.service.revision += 1;
        } else {
            jobs.status = "Approach the orange delivery marker to use the intercom [R].".into();
        }
    }
}

fn text(value: impl Into<String>, size: f32) -> impl Bundle {
    (
        Text::new(value),
        TextFont {
            font_size: FontSize::Px(size),
            ..default()
        },
        TextColor(Color::srgb(0.95, 0.94, 0.89)),
    )
}
fn button(parent: &mut ChildSpawnerCommands, action: Action, title: String, active: bool) {
    parent.spawn((
        Button,
        Hovered::default(),
        action,
        Node {
            padding: UiRect::all(Val::Px(12.)),
            margin: UiRect::bottom(Val::Px(5.)),
            ..default()
        },
        BackgroundColor(if active {
            Color::srgb(0.24, 0.42, 0.31)
        } else {
            Color::srgb(0.19, 0.24, 0.24)
        }),
        children![text(title, 17.)],
    ));
}
pub(crate) fn service_ui(
    mut commands: Commands,
    jobs: Res<Jobs>,
    shift: Res<crate::shift::Shift>,
    time: Res<Time<Virtual>>,
    panels: Query<Entity, With<Panel>>,
    mut timers: Query<&mut Text, With<TimerLabel>>,
    mut last: Local<Option<u64>>,
    player: Query<&Transform, With<Player>>,
    camera: Query<(&Camera, &GlobalTransform), With<Camera3d>>,
    residents: Query<(&Resident, &Transform)>,
    mut markers: Query<(&Marker, &mut Node, &mut Text), (Without<TimerLabel>, Without<Panel>)>,
) {
    let now = time.elapsed_secs();
    if *last != Some(jobs.service.revision) {
        *last = Some(jobs.service.revision);
        for root in &panels {
            commands.entity(root).despawn();
        }
        if let Some(modal) = &jobs.service.modal {
            commands
                .spawn((
                    Panel,
                    DespawnOnExit(Mode::Ride),
                    ZIndex(50),
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Percent(50.),
                        top: Val::Percent(50.),
                        width: Val::Px(440.),
                        max_width: Val::Percent(94.),
                        padding: UiRect::all(Val::Px(24.)),
                        flex_direction: FlexDirection::Column,
                        ..default()
                    },
                    UiTransform::from_translation(Val2::percent(-50., -50.)),
                    BackgroundColor(Color::srgba(0.055, 0.075, 0.075, 0.97)),
                ))
                .with_children(|p| {
                    match &modal.kind {
                        ModalKind::Intercom {
                            address,
                            pressed,
                            rare,
                            ..
                        } => {
                            p.spawn(text("INTERCOM", 24.));
                            p.spawn(text(format!("For {}", citizen(address).name), 17.));
                            p.spawn((
                                TimerLabel,
                                text("Ring your recipient. All 6 bells in time: +40 KPI", 14.),
                            ));
                            for i in 0..6 {
                                let name = if i == 0 {
                                    citizen(address).name
                                } else {
                                    citizen(&format!("{address}/{i}")).name
                                };
                                button(
                                    p,
                                    Action::Bell(i),
                                    format!(
                                        "{}  {name}",
                                        if pressed & (1 << i) != 0 {
                                            "●"
                                        } else {
                                            "○"
                                        }
                                    ),
                                    pressed & (1 << i) != 0,
                                );
                            }
                            if *rare && jobs.service.visited.contains(address) {
                                button(
                                    p,
                                    Action::Fault,
                                    "★ Report broken intercom · special delivery".into(),
                                    false,
                                );
                            }
                        }
                        ModalKind::Dialogue { name, .. } => {
                            p.spawn(text("Who is this parcel for?", 24.));
                            p.spawn((TimerLabel, text("", 17.)));
                            button(
                                p,
                                Action::Reply(0),
                                format!("1  {name}. Here you go."),
                                false,
                            );
                            button(
                                p,
                                Action::Reply(1),
                                format!(
                                    "2  {}… Schnaffel?",
                                    name.split_whitespace().next().unwrap_or("Er")
                                ),
                                false,
                            );
                            button(p, Action::Reply(2), "3  Whoever lives here.".into(), false);
                        }
                    }
                    p.spawn(text(&jobs.status, 13.));
                    button(p, Action::Close, "Close · Esc".into(), false);
                });
        }
    }
    for mut label in &mut timers {
        if let Some(modal) = &jobs.service.modal {
            label.0 = match &modal.kind {
                ModalKind::Dialogue { expires, .. } => format!(
                    "Answer in {:.1}s · mangled name +15 KPI",
                    (expires - now).max(0.)
                ),
                ModalKind::Intercom {
                    started: Some(start),
                    budget,
                    pressed,
                    ..
                } => format!(
                    "{} / 6 bells · {:.1}s",
                    pressed.count_ones(),
                    (budget - (now - start)).max(0.)
                ),
                _ => "Ring your recipient. All 6 bells in time: +40 KPI".into(),
            };
        }
    }
    if markers.is_empty() {
        for i in 0..2 {
            commands.spawn((
                Marker(i),
                DespawnOnExit(Mode::Ride),
                Node {
                    position_type: PositionType::Absolute,
                    padding: UiRect::all(Val::Px(7.)),
                    display: Display::None,
                    ..default()
                },
                UiTransform::from_translation(Val2::percent(-50., -100.)),
                ZIndex(5),
                BackgroundColor(if i == 0 {
                    Color::srgba(0.69, 0.25, 0.04, 0.92)
                } else {
                    Color::srgba(0.05, 0.28, 0.25, 0.93)
                }),
                text("", 14.),
            ));
        }
    }
    let (Ok((camera, ct)), Ok(actor), Some(parcel)) = (
        camera.single(),
        player.single(),
        jobs.parcels.get(jobs.navigation),
    ) else {
        return;
    };
    for (kind, mut node, mut label) in &mut markers {
        let citizen = citizen(&parcel.address);
        let (position, value) = if kind.0 == 0 {
            let target = jobs
                .service
                .bulk
                .filter(|(_, loaded)| *loaded)
                .map_or(parcel.point, |(p, _)| p);
            (
                target + Vec3::Y * 2.7,
                format!(
                    "{} · {:.0} m\n{}\n[R] Intercom",
                    parcel.label,
                    actors::horizontal_distance(target, actor.translation),
                    citizen.name
                ),
            )
        } else if let Some((npc, t)) = residents.iter().find(|(n, _)| n.address == parcel.address) {
            let state = if !citizen.home_at(shift.minute()) {
                "AWAY".into()
            } else {
                match npc.state {
                    ResidentState::Inside => "At home".into(),
                    ResidentState::Answering(deadline) => {
                        format!("Coming · {:.0}s", (deadline - now).max(0.))
                    }
                    _ => "Recipient here".into(),
                }
            };
            (
                t.translation + Vec3::Y * 2.25,
                format!("{} · {state}", citizen.name),
            )
        } else {
            node.display = Display::None;
            continue;
        };
        if jobs.interaction_active()
            || actors::horizontal_distance(actor.translation, position) > 100.
        {
            node.display = Display::None;
            continue;
        }
        if let Ok(p) = camera.world_to_viewport(ct, position) {
            node.left = Val::Px(p.x);
            node.top = Val::Px(p.y);
            node.display = Display::Flex;
            label.0 = value;
        } else {
            node.display = Display::None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn doorbell_is_required_and_the_all_bells_bonus_is_awarded_once() {
        use crate::ride_tests::{headless_ride, press};
        let mut app = headless_ride(Role::Courier);
        press(&mut app, KeyCode::KeyE);
        let (address, point) = {
            let jobs = app.world().resource::<Jobs>();
            (jobs.parcels[0].address.clone(), jobs.parcels[0].point)
        };
        let actor = {
            let w = app.world_mut();
            w.query_filtered::<Entity, With<Player>>()
                .single(w)
                .unwrap()
        };
        app.world_mut()
            .get_mut::<Transform>(actor)
            .unwrap()
            .translation = point.with_y(0.83);
        press(&mut app, KeyCode::KeyE);
        assert_eq!(
            app.world().resource::<Jobs>().parcels[0].state,
            ParcelState::Carrying
        );
        for _ in 0..2 {
            press(&mut app, KeyCode::KeyR);
            for i in 0..6 {
                let button = {
                    let w = app.world_mut();
                    w.query::<(Entity, &Action)>()
                        .iter(w)
                        .find(|(_, a)| matches!(a,Action::Bell(n) if *n==i))
                        .unwrap()
                        .0
                };
                app.world_mut().entity_mut(button).insert(bevy::ui::Pressed);
                app.update();
            }
            assert!(
                app.world()
                    .resource::<Jobs>()
                    .service
                    .visited
                    .contains(&address)
            );
            assert_eq!(app.world().resource::<Jobs>().service.bonus, 40);
            let close = {
                let w = app.world_mut();
                w.query::<(Entity, &Action)>()
                    .iter(w)
                    .find(|(_, a)| matches!(a, Action::Close))
                    .unwrap()
                    .0
            };
            app.world_mut().entity_mut(close).insert(bevy::ui::Pressed);
            app.update();
        }
        assert!(app.world().resource::<crate::shift::Shift>().locked);
    }
    #[test]
    fn schedules_are_repeatable_and_residents_have_different_away_slots() {
        let a = citizen("way/1");
        let b = citizen("way/2");
        assert_eq!(a.name, citizen("way/1").name);
        assert_ne!(a.away, b.away);
        assert!(!a.home_at(a.away[0].0));
        assert!(a.home_at(1019));
        assert!((4.0..=21.0).contains(&a.delay));
    }
}
