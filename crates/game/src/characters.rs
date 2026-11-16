use std::time::Duration;

use bevy::{
    animation::transition::AnimationTransitions, prelude::*,
    world_serialization::WorldInstanceReady,
};

use crate::actors::{HoldingParcel, Player};

pub(crate) struct Kit {
    residents: [Model; 4],
    pub courier: Model,
}

pub(crate) struct Model {
    scene: Handle<WorldAsset>,
    clips: Clips,
    scale: f32,
    lift: f32,
}

#[derive(Clone, Copy)]
struct Locomotion {
    idle: AnimationNodeIndex,
    walk: AnimationNodeIndex,
    run: Option<AnimationNodeIndex>,
}

#[derive(Clone, Copy)]
struct CourierClips {
    carrying: Locomotion,
    recoil: AnimationNodeIndex,
}

#[derive(Component, Clone)]
struct Clips {
    graph: Handle<AnimationGraph>,
    movement: Locomotion,
    courier: Option<CourierClips>,
    stride_speed: f32,
    run_threshold: f32,
}

#[derive(Component)]
pub(crate) struct Movement {
    root: Entity,
    parent: Entity,
    clips: Clips,
    previous: Option<Vec2>,
    speed: f32,
}

#[derive(Clone, Copy)]
enum Rig {
    Resident,
    Courier,
    Dog,
}

impl Kit {
    pub(crate) fn new(assets: &AssetServer, graphs: &mut Assets<AnimationGraph>) -> Self {
        let residents = [
            "street/man-casual.glb",
            "street/woman-casual.glb",
            "street/man-hoodie.glb",
            "street/woman-formal.glb",
        ]
        .map(|path| Model::load(assets, graphs, path, Rig::Resident));
        let courier = Model::load(assets, graphs, "street/courier.glb", Rig::Courier);
        Self { residents, courier }
    }

    pub(crate) fn spawn(&self, commands: &mut Commands, parent: Entity, variant: usize) {
        self.residents[variant % self.residents.len()].spawn(commands, parent);
    }
}

impl Model {
    pub(crate) fn shiba(assets: &AssetServer, graphs: &mut Assets<AnimationGraph>) -> Self {
        Self::load(assets, graphs, "street/shiba.glb", Rig::Dog)
    }

    fn load(
        assets: &AssetServer,
        graphs: &mut Assets<AnimationGraph>,
        path: &'static str,
        rig: Rig,
    ) -> Self {
        let scene = assets.load(GltfAssetLabel::Scene(0).from_asset(path));
        let mut graph = AnimationGraph::new();
        let mut clip = |index| {
            graph.add_clip(
                assets.load(GltfAssetLabel::Animation(index).from_asset(path)),
                1.0,
                graph.root,
            )
        };
        let movement = Locomotion {
            idle: clip(0),
            walk: clip(1),
            run: (!matches!(rig, Rig::Resident)).then(|| clip(2)),
        };
        let courier = matches!(rig, Rig::Courier).then(|| CourierClips {
            carrying: Locomotion {
                idle: clip(3),
                walk: clip(4),
                run: Some(clip(5)),
            },
            recoil: clip(6),
        });
        let (scale, lift, stride_speed, run_threshold) = match rig {
            Rig::Resident => (0.95, 0., 1.4, f32::INFINITY),
            Rig::Courier => (0.95, 0., 2.15, 4.2),
            Rig::Dog => (0.22, 0.03, 1.5, 3.1),
        };
        Self {
            scene,
            clips: Clips {
                graph: graphs.add(graph),
                movement,
                courier,
                stride_speed,
                run_threshold,
            },
            scale,
            lift,
        }
    }

    pub(crate) fn spawn(&self, commands: &mut Commands, parent: Entity) {
        commands
            .spawn((
                ChildOf(parent),
                Transform::from_xyz(0., self.lift, 0.)
                    .with_rotation(Quat::from_rotation_y(std::f32::consts::PI))
                    .with_scale(Vec3::splat(self.scale)),
                WorldAssetRoot(self.scene.clone()),
                self.clips.clone(),
            ))
            .observe(animate_when_ready);
    }
}

fn animate_when_ready(
    ready: On<WorldInstanceReady>,
    mut commands: Commands,
    poses: Query<(&Clips, &ChildOf)>,
    children: Query<&Children>,
    mut players: Query<&mut AnimationPlayer>,
) {
    let Ok((clips, parent)) = poses.get(ready.entity) else {
        return;
    };
    for child in children.iter_descendants(ready.entity) {
        if let Ok(mut player) = players.get_mut(child) {
            let mut transitions = AnimationTransitions::new();
            transitions
                .play(&mut player, clips.movement.idle, Duration::ZERO)
                .repeat();
            commands.entity(child).insert((
                AnimationGraphHandle(clips.graph.clone()),
                transitions,
                Movement {
                    root: ready.entity,
                    parent: parent.parent(),
                    clips: clips.clone(),
                    previous: None,
                    speed: 0.,
                },
            ));
        }
    }
}

pub(crate) fn animate(
    time: Res<Time>,
    clock: Res<Time<Virtual>>,
    roots: Query<&GlobalTransform>,
    held: Query<(), With<HoldingParcel>>,
    players: Query<&Player>,
    mut actors: Query<(
        &mut Movement,
        &mut AnimationPlayer,
        &mut AnimationTransitions,
    )>,
) {
    for (mut movement, mut player, mut transitions) in &mut actors {
        let Ok(root) = roots.get(movement.root) else {
            continue;
        };
        let position = root.translation().xz();
        let sample = movement.previous.replace(position).map_or(0.0, |previous| {
            previous.distance(position) / time.delta_secs().max(0.001)
        });
        // Physics positions arrive on fixed ticks.
        let sample = if sample <= 14.0 { sample } else { 0.0 };
        movement.speed += (sample - movement.speed) * (1.0 - (-18.0 * time.delta_secs()).exp());
        let mut motion = movement.clips.movement;
        let recoil = movement.clips.courier.and_then(|courier| {
            if held.contains(movement.parent) {
                motion = courier.carrying;
            }
            players
                .iter()
                .any(|p| p.visual == movement.parent && p.stunned_until > clock.elapsed_secs())
                .then_some(courier.recoil)
        });
        let walking = movement.speed > 0.12;
        let running = movement.speed > movement.clips.run_threshold;
        let clip = recoil.unwrap_or(if running {
            motion.run.unwrap_or(motion.walk)
        } else if walking {
            motion.walk
        } else {
            motion.idle
        });
        if transitions.get_main_animation() != Some(clip) {
            let active = transitions.play(&mut player, clip, Duration::from_millis(130));
            if recoil.is_none() {
                active.repeat();
            }
        }
        if walking
            && recoil.is_none()
            && let Some(active) = player.animation_mut(clip)
        {
            let pace = if running {
                movement.clips.stride_speed * 1.8
            } else {
                movement.clips.stride_speed
            };
            active.set_speed((movement.speed / pace).clamp(0.55, 1.8));
        }
    }
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn inspect_courier(
    time: Res<Time>,
    mut elapsed: Local<f32>,
    player: Query<(&Player, &GlobalTransform)>,
    children: Query<&Children>,
    nodes: Query<(&Name, &GlobalTransform)>,
    parcel: Query<&GlobalTransform, With<crate::jobs::ParcelInHands>>,
    roads: Res<crate::roads::Roads>,
    bridges: Res<crate::bridges::Bridges>,
    elevation: Res<crate::elevation::Elevation>,
) {
    *elapsed += time.delta_secs();
    if *elapsed < 0.08 {
        return;
    }
    *elapsed = 0.;
    let Ok((player, transform)) = player.single() else {
        return;
    };
    let mut markers = [None; 4];
    for entity in children.iter_descendants(player.visual) {
        let Ok((name, global)) = nodes.get(entity) else {
            continue;
        };
        let index = match name.as_str() {
            "SoleLeft" => 0,
            "SoleRight" => 1,
            "PalmLeft" => 2,
            "PalmRight" => 3,
            _ => continue,
        };
        markers[index] = Some(global.translation());
    }
    let [Some(left), Some(right), Some(left_hand), Some(right_hand)] = markers else {
        return;
    };
    let soles = [left, right].map(|foot| {
        let floor = bridges.height_at(foot).unwrap_or_else(|| {
            let heights = &elevation.heights;
            let ground = heights.at(foot) - crate::elevation::FLOOR_RECESS;
            ground
                + roads.pavement(foot)
                    * (crate::elevation::FLOOR_RECESS + crate::roads::surface_rise(heights))
        });
        foot.y - floor
    });
    let parcel = parcel.single().ok();
    let grip = [left_hand, right_hand].map(|hand| {
        parcel.map_or(0., |parcel| {
            let delta = parcel.affine().inverse().transform_point3(hand).abs();
            let half = crate::actors::PARCEL_HALF_SIZE;
            Vec3::new(
                delta.x - half.x,
                (delta.y - half.y).max(0.),
                (delta.z - half.z).max(0.),
            )
            .length()
        })
    });
    let p = transform.translation();
    let data = serde_json::json!({
        "seconds": time.elapsed_secs_f64(),
        "position": [p.x, p.z], "soles": soles, "grip": grip, "carrying": parcel.is_some(),
    });
    crate::platform::ui_event("paket-courier-pose", &data.to_string());
}
