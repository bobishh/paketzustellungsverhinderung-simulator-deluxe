use std::time::Duration;

use bevy::{
    animation::transition::AnimationTransitions, prelude::*,
    world_serialization::WorldInstanceReady,
};

pub(crate) struct Kit {
    models: [Model; 4],
}

struct Model {
    scene: Handle<WorldAsset>,
    clips: Clips,
}

#[derive(Component, Clone)]
struct Clips {
    graph: Handle<AnimationGraph>,
    idle: AnimationNodeIndex,
    walk: AnimationNodeIndex,
}

#[derive(Component)]
pub(crate) struct Movement {
    root: Entity,
    clips: Clips,
    previous: Option<Vec2>,
    speed: f32,
}

impl Kit {
    pub(crate) fn new(assets: &AssetServer, graphs: &mut Assets<AnimationGraph>) -> Self {
        let models = [
            "street/man-casual.glb",
            "street/woman-casual.glb",
            "street/man-hoodie.glb",
            "street/woman-formal.glb",
        ]
        .map(|path| {
            let scene = assets.load(GltfAssetLabel::Scene(0).from_asset(path));
            let mut graph = AnimationGraph::new();
            let [idle, walk] = [0, 1].map(|index| {
                graph.add_clip(
                    assets.load(GltfAssetLabel::Animation(index).from_asset(path)),
                    1.0,
                    graph.root,
                )
            });
            Model {
                scene,
                clips: Clips {
                    graph: graphs.add(graph),
                    idle,
                    walk,
                },
            }
        });
        Self { models }
    }

    pub(crate) fn spawn(&self, commands: &mut Commands, parent: Entity, variant: usize) {
        let model = &self.models[variant % self.models.len()];
        commands
            .spawn((
                ChildOf(parent),
                Transform::from_rotation(Quat::from_rotation_y(std::f32::consts::PI))
                    .with_scale(Vec3::splat(0.95)),
                WorldAssetRoot(model.scene.clone()),
                model.clips.clone(),
            ))
            .observe(animate_when_ready);
    }
}

fn animate_when_ready(
    ready: On<WorldInstanceReady>,
    mut commands: Commands,
    poses: Query<&Clips>,
    children: Query<&Children>,
    mut players: Query<&mut AnimationPlayer>,
) {
    let Ok(clips) = poses.get(ready.entity) else {
        return;
    };
    for child in children.iter_descendants(ready.entity) {
        if let Ok(mut player) = players.get_mut(child) {
            let mut transitions = AnimationTransitions::new();
            transitions
                .play(&mut player, clips.idle, Duration::ZERO)
                .repeat();
            commands.entity(child).insert((
                AnimationGraphHandle(clips.graph.clone()),
                transitions,
                Movement {
                    root: ready.entity,
                    clips: clips.clone(),
                    previous: None,
                    speed: 0.0,
                },
            ));
        }
    }
}

pub(crate) fn animate(
    time: Res<Time>,
    roots: Query<&GlobalTransform>,
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
        // Traffic moves at the physics tick rate, which can differ from the animation tick rate.
        let sample = if sample <= 6.0 { sample } else { 0.0 };
        movement.speed += (sample - movement.speed) * (1.0 - (-18.0 * time.delta_secs()).exp());
        let walking = movement.speed > 0.12;
        let clip = if walking {
            movement.clips.walk
        } else {
            movement.clips.idle
        };
        if transitions.get_main_animation() != Some(clip) {
            transitions
                .play(&mut player, clip, Duration::from_millis(150))
                .repeat();
        }
        if walking && let Some(walk) = player.animation_mut(clip) {
            walk.set_speed((movement.speed / 1.4).clamp(0.6, 1.8));
        }
    }
}
