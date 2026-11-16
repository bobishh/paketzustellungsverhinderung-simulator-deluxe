use bevy::prelude::*;
use paket_geo::Street;

use crate::{actors::Player, to_world};

#[derive(Component)]
pub(crate) struct Dog {
    start: Vec3,
    forward: Vec3,
    length: f32,
    distance: f32,
    direction: f32,
    mood: Mood,
}

#[derive(Component)]
pub(crate) struct DeliveryDog;

#[derive(Default)]
enum Mood {
    #[default]
    Patrol,
    Warning(f32),
    Leap {
        start: Vec3,
        end: Vec3,
        began: f32,
        hit: bool,
    },
    Recover(f32),
}
impl Dog {
    pub(crate) fn reset_day(&mut self) {
        self.mood = Mood::Patrol;
    }
}
pub(crate) fn leap_height(progress: f32) -> f32 {
    (progress.clamp(0., 1.) * std::f32::consts::PI).sin() * 0.65
}

pub(crate) struct DogKit(crate::characters::Model);

impl DogKit {
    pub(crate) fn new(assets: &AssetServer, graphs: &mut Assets<AnimationGraph>) -> Self {
        Self(crate::characters::Model::shiba(assets, graphs))
    }

    pub(crate) fn spawn(&self, commands: &mut Commands, street: &Street, parent: Entity) {
        let hash = street.id.bytes().fold(2166136261u32, |hash, byte| {
            (hash ^ u32::from(byte)).wrapping_mul(16777619)
        });
        let [left, right] = street.sidewalk_m;
        if !hash.is_multiple_of(3) || left.max(right) <= 0.0 {
            return;
        }
        let Some((a, b, length)) = street
            .centerline
            .windows(2)
            .map(|pair| {
                let a = to_world(pair[0], 0.045);
                let b = to_world(pair[1], 0.045);
                (a, b, a.distance(b))
            })
            .filter(|(_, _, length)| *length >= 10.0)
            .max_by(|a, b| a.2.total_cmp(&b.2))
        else {
            return;
        };
        let forward = (b - a) / length;
        let side = Vec3::new(-forward.z, 0.0, forward.x);
        let offset = if right > 0.0 && (left <= 0.0 || hash & 1 == 0) {
            street.width_m * 0.5 + right * 0.5
        } else {
            -(street.width_m * 0.5 + left * 0.5)
        };
        let patrol_length = (length - 4.0).min(16.0);
        let start = a + forward * ((length - patrol_length) * 0.5) + side * offset;
        let distance = patrol_length * (0.2 + (hash % 61) as f32 / 100.0);
        let dog = commands
            .spawn((
                Dog {
                    start,
                    forward,
                    length: patrol_length,
                    distance,
                    direction: 1.0,
                    mood: Mood::Patrol,
                },
                Transform::from_translation(start + forward * distance)
                    .with_rotation(facing(forward)),
                Visibility::default(),
                ChildOf(parent),
            ))
            .id();
        self.model(commands, dog);
    }

    pub(crate) fn at_door(&self, commands: &mut Commands, position: Vec3, outward: Vec3) {
        let start = position + outward * 2.4 + Vec3::Y * 0.045;
        let forward = outward.cross(Vec3::Y);
        let dog = commands
            .spawn((
                Dog {
                    start,
                    forward,
                    length: 2.,
                    distance: 0.,
                    direction: 1.,
                    mood: Mood::Patrol,
                },
                Transform::from_translation(start).with_rotation(facing(forward)),
                Visibility::default(),
                DespawnOnExit(crate::Mode::Ride),
                DeliveryDog,
            ))
            .id();
        self.model(commands, dog);
    }

    pub(crate) fn model(&self, commands: &mut Commands, root: Entity) {
        self.0.spawn(commands, root);
    }
}

fn facing(direction: Vec3) -> Quat {
    Quat::from_rotation_y((-direction.x).atan2(-direction.z))
}

pub(crate) fn walk(
    time: Res<Time>,
    clock: Res<Time<Virtual>>,
    mut commands: Commands,
    mut player: Query<(Entity, &mut Player, &Transform), Without<Dog>>,
    mut dogs: Query<(Entity, &mut Dog, &mut Transform)>,
    mut jobs: ResMut<crate::jobs::Jobs>,
    spatial: avian3d::prelude::SpatialQuery,
    elevation: Res<crate::elevation::Elevation>,
) {
    let Ok((actor, mut player, rider)) = player.single_mut() else {
        return;
    };
    let dt = time.delta_secs();
    let now = time.elapsed_secs();
    for (entity, mut dog, mut transform) in &mut dogs {
        if !matches!(dog.mood, Mood::Leap { .. }) {
            transform.translation.y = elevation.heights.at(transform.translation) + 0.045;
        }
        if !matches!(dog.mood, Mood::Leap { .. })
            && jobs.dog_delivery_step(
                &mut commands,
                &spatial,
                (actor, &mut player),
                entity,
                &mut transform,
                clock.elapsed_secs(),
                dt,
            )
        {
            dog.mood = Mood::Recover(now + 3.);
            continue;
        }
        let delta = (rider.translation - transform.translation).with_y(0.);
        let visible = player.form == paket_session::Avatar::Courier
            && !jobs.interaction_active()
            && delta.length() < 5.0
            && crate::jobs::visible(
                &spatial,
                rider.translation + Vec3::Y * 0.5,
                transform.translation + Vec3::Y * 0.4,
                actor,
            );
        match &mut dog.mood {
            Mood::Patrol if visible => {
                dog.mood = Mood::Warning(now + 2.8);
                jobs.status = "Grrrr… Dog watching you. Step back before it jumps.".into();
                continue;
            }
            Mood::Warning(deadline) => {
                if !visible {
                    dog.mood = Mood::Recover(now + 2.);
                    continue;
                }
                transform.rotation = facing(delta);
                if now >= *deadline {
                    dog.mood = Mood::Leap {
                        start: transform.translation,
                        end: rider.translation.with_y(transform.translation.y)
                            - delta.normalize_or_zero() * 0.45,
                        began: now,
                        hit: false,
                    };
                }
                continue;
            }
            Mood::Leap {
                start,
                end,
                began,
                hit,
            } => {
                let t = ((now - *began) / 0.72).clamp(0., 1.);
                transform.translation = start.lerp(*end, t) + Vec3::Y * leap_height(t);
                transform.rotation = facing(*end - *start)
                    * Quat::from_rotation_x(-0.38 * (t * std::f32::consts::PI).sin());
                if !*hit && t >= 0.55 && visible && delta.length() < 1.25 {
                    *hit = true;
                    player.stunned_until = now + 1.2;
                    jobs.status = "OUCH · Dog bite! Move away while it recovers.".into();
                }
                if t >= 1. {
                    dog.mood = Mood::Recover(now + 8.);
                }
                continue;
            }
            Mood::Recover(until) => {
                if now >= *until {
                    dog.mood = Mood::Patrol;
                }
                let home = dog.start + dog.forward * dog.distance;
                let home = home.with_y(elevation.heights.at(home) + 0.045);
                let delta = home - transform.translation;
                transform.translation += delta.clamp_length_max(dt * 1.5);
                if delta.length() > 0.1 {
                    transform.rotation = facing(delta);
                }
                continue;
            }
            _ => {}
        }
        let speed = 0.85;
        let next = (dog.distance + dog.direction * speed * dt).clamp(0.0, dog.length);
        dog.distance = next;
        transform.translation = dog.start + dog.forward * dog.distance;
        transform.translation.y = elevation.heights.at(transform.translation) + 0.045;
        transform.rotation = transform.rotation.slerp(
            facing(dog.forward * dog.direction),
            1.0 - (-14.0 * dt).exp(),
        );
        if (next == 0.0 || next == dog.length) && speed < 1.0 {
            dog.direction = if next == 0.0 { 1.0 } else { -1.0 };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn npc_warns_before_a_jump_and_recovers_after_biting() {
        let mut app = crate::ride_tests::headless_ride(crate::actors::Role::Courier);
        let (entity, position, right) = {
            let w = app.world_mut();
            let (e, t) = w
                .query_filtered::<(Entity, &Transform), With<Player>>()
                .single(w)
                .unwrap();
            (e, t.translation, t.right().as_vec3())
        };
        let old = {
            let w = app.world_mut();
            w.query_filtered::<Entity, With<Dog>>()
                .iter(w)
                .collect::<Vec<_>>()
        };
        for e in old {
            app.world_mut().despawn(e);
        }
        let point = (position + right * 2.5).with_y(0.045);
        let dog = app
            .world_mut()
            .spawn((
                Dog {
                    start: point,
                    forward: Vec3::X,
                    length: 10.,
                    distance: 0.,
                    direction: 1.,
                    mood: Mood::Patrol,
                },
                Transform::from_translation(point),
            ))
            .id();
        for _ in 0..140 {
            app.update();
        }
        assert!(matches!(
            app.world().get::<Dog>(dog).unwrap().mood,
            Mood::Warning(_)
        ));
        assert_eq!(app.world().get::<Player>(entity).unwrap().stunned_until, 0.);
        for _ in 0..100 {
            app.update();
        }
        assert!(matches!(
            app.world().get::<Dog>(dog).unwrap().mood,
            Mood::Recover(_)
        ));
        assert!(
            app.world().get::<Player>(entity).unwrap().stunned_until
                > app.world().resource::<Time>().elapsed_secs()
        );
    }
    #[test]
    fn leap_lands_on_the_ground() {
        assert!(leap_height(0.).abs() < 1e-6);
        assert!((leap_height(0.5) - 0.65).abs() < 1e-6);
        assert!(leap_height(1.).abs() < 1e-6);
    }
}
