use avian3d::prelude::*;
use bevy::{
    input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll},
    prelude::*,
    world_serialization::WorldInstanceReady,
};
use paket_session::{ActorPose, Avatar, VanPose};

use crate::{Mode, Ride, StreetMaterials, east_north};
use paket_geo::tile_id;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Role {
    #[default]
    Courier,
    Dog,
}

#[derive(Component)]
pub(crate) struct Player {
    pub form: Avatar,
    pub photo: bool,
    pub pitch: f32,
    pub stunned_until: f32,
    pub visual: Entity,
    pub parked: Option<Entity>,
}

#[derive(Component)]
pub(crate) struct ParkedVan;

#[derive(Component, Default)]
pub(crate) struct Controller {
    steering: f32,
    van_look: f32,
    look_idle: f32,
}

#[derive(Resource)]
pub(crate) struct Controls {
    pub(crate) sensitivity: f32,
    zoom: f32,
}

impl Default for Controls {
    fn default() -> Self {
        Self {
            sensitivity: 0.0024,
            zoom: 1.0,
        }
    }
}

#[derive(Component, Default)]
pub(crate) struct FollowCamera {
    form: Option<Avatar>,
}

#[derive(Component, Default)]
pub(crate) struct VehicleVisual {
    previous: Option<Vec3>,
    heading: Option<f32>,
    roll: f32,
    steer: f32,
    radius: f32,
}

#[derive(Component)]
pub(crate) struct VehicleWheels(Vec<(Entity, bool)>);

#[derive(Component)]
pub(crate) struct HoldingParcel;

#[derive(Component)]
pub(crate) struct CourierGround(f32);

#[derive(Component)]
pub(crate) struct VanGround;

pub(crate) const PARCEL_GRIP: Vec3 = Vec3::new(0., 1.055, -0.38);
pub(crate) const PARCEL_HALF_SIZE: Vec3 = Vec3::new(0.24, 0.15, 0.185);

#[derive(Resource)]
pub(crate) struct Models {
    box_mesh: Handle<Mesh>,
    decal: Handle<Mesh>,
    branding: Handle<StandardMaterial>,
    pub citizens: crate::characters::Kit,
    delivery_van: Handle<WorldAsset>,
    fabric: Handle<StandardMaterial>,
    dark: Handle<StandardMaterial>,
    glass: Handle<StandardMaterial>,
    metal: Handle<StandardMaterial>,
    cardboard: Handle<StandardMaterial>,
    tape: Handle<StandardMaterial>,
    label: Handle<StandardMaterial>,
}

impl Models {
    pub fn new(
        meshes: &mut Assets<Mesh>,
        materials: &mut Assets<StandardMaterial>,
        images: &mut Assets<Image>,
        citizens: crate::characters::Kit,
        assets: &AssetServer,
    ) -> Self {
        use crate::surfaces::{Surface, material, matte};
        Self {
            box_mesh: meshes.add(crate::shapes::rounded_box(Vec3::ONE, 0.065)),
            decal: meshes.add(Rectangle::new(1.8, 0.72)),
            branding: materials.add(StandardMaterial {
                base_color_texture: Some(assets.load("branding/drehlss-van.png")),
                alpha_mode: AlphaMode::Blend,
                perceptual_roughness: 0.8,
                ..default()
            }),
            citizens,
            delivery_van: assets
                .load(GltfAssetLabel::Scene(0).from_asset("street/delivery-van.glb")),
            fabric: materials.add(material(
                images,
                Surface::Fabric,
                Color::srgb(0.16, 0.20, 0.24),
            )),
            dark: materials.add(matte(Color::srgb(0.055, 0.075, 0.085), 0.72)),
            glass: materials.add(StandardMaterial {
                metallic: 0.25,
                reflectance: 0.6,
                ..matte(Color::srgb(0.10, 0.19, 0.23), 0.16)
            }),
            metal: materials.add(StandardMaterial {
                metallic: 0.72,
                ..matte(Color::srgb(0.51, 0.55, 0.55), 0.34)
            }),
            cardboard: materials.add(matte(Color::srgb(0.54, 0.36, 0.18), 0.95)),
            tape: materials.add(matte(Color::srgb(0.68, 0.51, 0.29), 0.55)),
            label: materials.add(matte(Color::srgb(0.92, 0.90, 0.82), 0.85)),
        }
    }

    pub fn avatar(
        &self,
        commands: &mut Commands,
        parent: Entity,
        form: Avatar,
        dogs: &crate::dogs::DogKit,
        y: f32,
    ) -> Entity {
        let root = commands
            .spawn((
                Transform::from_xyz(0.0, y, 0.0),
                Visibility::default(),
                ChildOf(parent),
            ))
            .id();
        match form {
            Avatar::Courier => {
                commands.entity(root).insert(CourierGround(y));
                self.citizens.courier.spawn(commands, root);
            }
            Avatar::Dog => dogs.model(commands, root),
            Avatar::Van => self.van(commands, root),
        }
        root
    }

    fn part(
        &self,
        commands: &mut Commands,
        parent: Entity,
        mesh: &Handle<Mesh>,
        material: &Handle<StandardMaterial>,
        transform: Transform,
    ) {
        commands.spawn((
            Mesh3d(mesh.clone()),
            MeshMaterial3d(material.clone()),
            transform,
            ChildOf(parent),
        ));
    }

    pub fn parcel(&self, commands: &mut Commands, parent: Entity) {
        for (point, size, material) in [
            (Vec3::ZERO, PARCEL_HALF_SIZE * 2.0, &self.cardboard),
            (
                Vec3::new(0.0, 0.152, 0.0),
                Vec3::new(0.065, 0.006, 0.37),
                &self.tape,
            ),
            (
                Vec3::new(0.0, 0.0, -0.187),
                Vec3::new(0.065, 0.30, 0.006),
                &self.tape,
            ),
            (
                Vec3::new(0.13, 0.154, -0.02),
                Vec3::new(0.14, 0.006, 0.17),
                &self.label,
            ),
        ] {
            self.part(
                commands,
                parent,
                &self.box_mesh,
                material,
                Transform::from_translation(point).with_scale(size),
            );
        }
        for x in 0..7 {
            self.part(
                commands,
                parent,
                &self.box_mesh,
                &self.dark,
                Transform::from_xyz(0.078 + x as f32 * 0.016, 0.159, -0.04)
                    .with_scale(Vec3::new(0.006, 0.003, 0.07)),
            );
        }
    }

    pub fn door(&self, commands: &mut Commands, parent: Entity) {
        for (point, size, material) in [
            (
                Vec3::new(-0.60, 1.09, 0.),
                Vec3::new(1.20, 2.18, 0.10),
                &self.fabric,
            ),
            (
                Vec3::new(-0.60, 1.56, -0.057),
                Vec3::new(0.84, 0.67, 0.018),
                &self.glass,
            ),
            (
                Vec3::new(-0.60, 1.56, 0.057),
                Vec3::new(0.84, 0.67, 0.018),
                &self.glass,
            ),
            (
                Vec3::new(-1.06, 1.02, -0.08),
                Vec3::new(0.055, 0.17, 0.06),
                &self.metal,
            ),
            (
                Vec3::new(-1.06, 1.02, 0.08),
                Vec3::new(0.055, 0.17, 0.06),
                &self.metal,
            ),
            (
                Vec3::new(-0.60, 0.81, -0.064),
                Vec3::new(0.36, 0.055, 0.02),
                &self.metal,
            ),
        ] {
            self.part(
                commands,
                parent,
                &self.box_mesh,
                material,
                Transform::from_translation(point).with_scale(size),
            );
        }
    }

    pub(crate) fn van(&self, commands: &mut Commands, parent: Entity) {
        let body = commands
            .spawn((
                Transform::default(),
                Visibility::default(),
                ChildOf(parent),
                VanGround,
            ))
            .id();
        spawn_vehicle(commands, body, self.delivery_van.clone(), Vec3::ZERO, 0.32);
        for side in [-1., 1.] {
            self.part(
                commands,
                body,
                &self.decal,
                &self.branding,
                Transform::from_xyz(side * 0.936, 1.35, 0.90)
                    .with_rotation(Quat::from_rotation_y(side * std::f32::consts::FRAC_PI_2)),
            );
        }
    }
}

pub(crate) fn ground_couriers(
    roads: Res<crate::roads::Roads>,
    bridges: Res<crate::bridges::Bridges>,
    elevation: Res<crate::elevation::Elevation>,
    actors: Query<&Transform, Without<CourierGround>>,
    mut visuals: Query<(&ChildOf, &CourierGround, &mut Transform)>,
) {
    for (parent, ground, mut visual) in &mut visuals {
        let Ok(actor) = actors.get(parent.parent()) else {
            continue;
        };
        let point = actor.translation;
        let lift = if bridges.height_at(point).is_some() {
            0.0
        } else {
            roads.pavement(point)
                * (crate::elevation::FLOOR_RECESS + crate::roads::surface_rise(&elevation.heights))
        };
        visual.translation.y = ground.0 + lift + 0.009;
    }
}

pub(crate) fn ground_vans(
    roads: Res<crate::roads::Roads>,
    bridges: Res<crate::bridges::Bridges>,
    elevation: Res<crate::elevation::Elevation>,
    parents: Query<&GlobalTransform>,
    mut vans: Query<(&ChildOf, &mut Transform, &InheritedVisibility), With<VanGround>>,
) {
    for (parent, mut van, visibility) in &mut vans {
        if !visibility.get() {
            continue;
        }
        let Ok(parent) = parents.get(parent.parent()) else {
            continue;
        };
        let point = parent.translation();
        let heights = &elevation.heights;
        let pavement = roads.pavement(point);
        let road_lift =
            pavement * (crate::elevation::FLOOR_RECESS + crate::roads::surface_rise(heights));
        let surface = |point| {
            bridges
                .height_at(point)
                .unwrap_or_else(|| heights.at(point) - crate::elevation::FLOOR_RECESS + road_lift)
        };
        let rotation = parent.rotation();
        let right = rotation * Vec3::X * 0.80;
        let rear = rotation * Vec3::Z * 1.35;
        let cross_slope = (surface(point + right) - surface(point - right)) / 1.60;
        let grade = (surface(point + rear) - surface(point - rear)) / 2.70;
        let up = Vec3::new(
            -cross_slope.clamp(-0.18, 0.18),
            1.,
            -grade.clamp(-0.24, 0.24),
        );
        van.rotation = Quat::from_rotation_arc(Vec3::Y, up.normalize());
        van.translation.y = (surface(point) - point.y).clamp(-0.18, 0.18) + 0.008;
    }
}

pub(crate) fn spawn_vehicle(
    commands: &mut Commands,
    parent: Entity,
    scene: Handle<WorldAsset>,
    offset: Vec3,
    radius: f32,
) {
    commands
        .spawn((
            ChildOf(parent),
            WorldAssetRoot(scene),
            Transform::from_translation(offset),
            VehicleVisual {
                radius,
                ..default()
            },
        ))
        .observe(find_wheels);
}

fn find_wheels(
    ready: On<WorldInstanceReady>,
    mut commands: Commands,
    children: Query<&Children>,
    names: Query<&Name>,
) {
    let wheels = children
        .iter_descendants(ready.entity)
        .filter_map(|entity| {
            let name = names.get(entity).ok()?.as_str();
            match name {
                "WheelFrontLeft" | "WheelFrontRight" => Some((entity, true)),
                "WheelRear" | "WheelRearLeft" | "WheelRearRight" => Some((entity, false)),
                _ => None,
            }
        })
        .collect();
    commands.entity(ready.entity).insert(VehicleWheels(wheels));
}

pub(crate) fn animate_vehicles(
    time: Res<Time>,
    mut vehicles: Query<(&mut VehicleVisual, &GlobalTransform, &VehicleWheels)>,
    mut wheels: Query<&mut Transform>,
) {
    for (mut vehicle, transform, parts) in &mut vehicles {
        let here = transform.translation();
        let forward = transform.forward().as_vec3();
        let distance = vehicle
            .previous
            .replace(here)
            .map_or(0., |p| (here - p).dot(forward));
        let yaw = transform.rotation().to_euler(EulerRot::YXZ).0;
        let turn = vehicle
            .heading
            .replace(yaw)
            .map_or(0., |p| (yaw - p).sin().atan2((yaw - p).cos()));
        if distance.abs() < 2.0 {
            vehicle.roll = (vehicle.roll - distance / vehicle.radius) % std::f32::consts::TAU;
        }
        let target = if distance.abs() > 0.002 {
            (turn * 2.55 / distance).atan().clamp(-0.55, 0.55)
        } else {
            0.0
        };
        vehicle.steer += (target - vehicle.steer) * blend(14.0, time.delta_secs());
        for &(entity, front) in &parts.0 {
            if let Ok(mut wheel) = wheels.get_mut(entity) {
                wheel.rotation = Quat::from_rotation_y(if front { vehicle.steer } else { 0.0 })
                    * Quat::from_rotation_x(vehicle.roll);
            }
        }
    }
}

pub(crate) fn half_height(form: Avatar) -> f32 {
    match form {
        Avatar::Courier => 0.87,
        Avatar::Dog => 0.30,
        Avatar::Van => 0.94,
    }
}

fn collider(form: Avatar) -> Collider {
    match form {
        Avatar::Courier => Collider::capsule(0.30, 1.14),
        Avatar::Dog => Collider::capsule(0.19, 0.22),
        Avatar::Van => Collider::cuboid(1.86, 1.88, 4.2),
    }
}

pub(crate) fn spawn(
    commands: &mut Commands,
    models: &Models,
    dogs: &crate::dogs::DogKit,
    role: Role,
    point: Vec3,
    direction: Vec3,
) {
    let form = if role == Role::Dog {
        Avatar::Dog
    } else {
        Avatar::Courier
    };
    let rotation = heading(direction);
    let ground = point;
    let entity = commands
        .spawn((
            RigidBody::Dynamic,
            collider(form),
            Transform::from_translation(ground + Vec3::Y * half_height(form))
                .with_rotation(rotation),
            Visibility::default(),
            LockedAxes::new().lock_rotation_x().lock_rotation_z(),
            Controller::default(),
            SweptCcd::default(),
            Friction {
                combine_rule: CoefficientCombine::Min,
                ..Friction::ZERO
            },
            LinearDamping(0.0),
            DespawnOnExit(Mode::Ride),
        ))
        .id();
    let parked = (role == Role::Courier).then(|| {
        let van = commands
            .spawn((
                Transform::from_translation(ground + direction * 2.6 + rotation * Vec3::X * 1.45)
                    .with_rotation(rotation),
                Visibility::default(),
                RigidBody::Static,
                Collider::compound(vec![(
                    Vec3::Y * half_height(Avatar::Van),
                    Quat::IDENTITY,
                    collider(Avatar::Van),
                )]),
                ParkedVan,
                DespawnOnExit(Mode::Ride),
            ))
            .id();
        models.van(commands, van);
        van
    });
    let visual = models.avatar(commands, entity, form, dogs, -half_height(form));
    commands.entity(entity).insert(Player {
        form,
        photo: false,
        pitch: 0.0,
        stunned_until: 0.0,
        visual,
        parked,
    });
}

fn heading(dir: Vec3) -> Quat {
    Quat::from_rotation_y((-dir.x).atan2(-dir.z))
}

pub(crate) fn horizontal_distance(a: Vec3, b: Vec3) -> f32 {
    (a - b).xz().length()
}

pub(crate) fn interact(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    ride: Res<Ride>,
    motion: Res<AccumulatedMouseMotion>,
    scroll: Res<AccumulatedMouseScroll>,
    mut controls: ResMut<Controls>,
    time: Res<Time>,
    models: Res<Models>,
    street: Res<StreetMaterials>,
    mut actor: Query<(
        Entity,
        &mut Player,
        &mut Controller,
        &mut Transform,
        &mut LinearVelocity,
        &mut AngularVelocity,
    )>,
    mut vans: Query<(&mut Transform, &mut Visibility), (With<ParkedVan>, Without<Player>)>,
    spatial: SpatialQuery,
    jobs: Res<crate::jobs::Jobs>,
) {
    if jobs.interaction_active() {
        return;
    }
    let Ok((entity, mut player, mut controller, mut transform, mut velocity, mut spin)) =
        actor.single_mut()
    else {
        return;
    };
    if time.elapsed_secs() < player.stunned_until {
        return;
    }
    if player.form == Avatar::Courier && keys.just_pressed(KeyCode::KeyF) {
        player.photo = !player.photo;
        player.pitch = -0.12;
        commands.entity(player.visual).insert(if player.photo {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        });
    }
    let looking = ride.mouse_look && !keys.just_pressed(KeyCode::Escape);
    #[cfg(target_arch = "wasm32")]
    let looking = looking && crate::platform::pointer_locked();
    if looking {
        let precision = if player.photo { 0.60 } else { 1.0 };
        let yaw = -motion.delta.x * controls.sensitivity * precision;
        if player.form == Avatar::Van {
            controller.van_look = (controller.van_look + yaw).clamp(-2.2, 2.2);
            if motion.delta.length_squared() > 0.1 {
                controller.look_idle = 0.0;
            }
        } else {
            transform.rotate_y(yaw);
        }
        let (low, high) = if player.photo {
            (-1.2, 1.2)
        } else {
            (-0.75, 0.48)
        };
        player.pitch =
            (player.pitch - motion.delta.y * controls.sensitivity * precision).clamp(low, high);
        controls.zoom = (controls.zoom - scroll.delta.y * 0.055).clamp(0.70, 1.45);
        if keys.just_pressed(KeyCode::Minus) {
            controls.sensitivity = (controls.sensitivity / 1.15).max(0.0008);
        }
        if keys.just_pressed(KeyCode::Equal) {
            controls.sensitivity = (controls.sensitivity * 1.15).min(0.006);
        }
    }
    controller.look_idle += time.delta_secs();
    if player.form == Avatar::Van && controller.look_idle > 1.2 && velocity.0.xz().length() > 1.0 {
        controller.van_look *= (-2.0 * time.delta_secs()).exp();
    }
    if !keys.just_pressed(KeyCode::KeyV) || player.photo {
        return;
    }
    let Some(parked) = player.parked else {
        return;
    };
    let Ok((mut van, mut visible)) = vans.get_mut(parked) else {
        return;
    };
    let next = if player.form == Avatar::Van {
        if velocity.0.xz().length() > 2.0 {
            return;
        }
        let filter = SpatialQueryFilter::from_excluded_entities([entity, parked]);
        let ground = transform.translation - Vec3::Y * half_height(Avatar::Van);
        let Some(exit) = [transform.right(), -transform.right()]
            .into_iter()
            .find(|direction| {
                spatial
                    .cast_ray(ground + Vec3::Y * 0.9, *direction, 1.8, true, &filter)
                    .is_none()
            })
        else {
            return;
        };
        van.translation = ground;
        van.rotation = transform.rotation;
        *visible = Visibility::Inherited;
        commands.entity(parked).remove::<ColliderDisabled>();
        transform.translation = ground + exit * 1.55 + Vec3::Y * half_height(Avatar::Courier);
        Avatar::Courier
    } else {
        if horizontal_distance(transform.translation, van.translation) > 3.5 {
            return;
        }
        *visible = Visibility::Hidden;
        commands.entity(parked).insert(ColliderDisabled);
        transform.translation = van.translation + Vec3::Y * half_height(Avatar::Van);
        transform.rotation = van.rotation;
        Avatar::Van
    };
    velocity.0 = Vec3::ZERO;
    spin.0 = Vec3::ZERO;
    commands.entity(entity).insert(collider(next));
    commands.entity(player.visual).despawn();
    player.visual = models.avatar(
        &mut commands,
        entity,
        next,
        &street.dogs,
        -half_height(next),
    );
    player.form = next;
    player.pitch = 0.0;
    *controller = Controller::default();
}

/// Exponential response gives identical damping at 30, 60 and 144 Hz.
fn blend(response: f32, dt: f32) -> f32 {
    1.0 - (-response * dt).exp()
}

fn van_velocity(
    velocity: Vec3,
    forward: Vec3,
    right: Vec3,
    throttle: f32,
    steer: f32,
    handbrake: bool,
    steering: &mut f32,
    dt: f32,
) -> (Vec3, f32) {
    let old_speed = velocity.dot(forward);
    let target = if handbrake || throttle == 0.0 {
        0.0
    } else if throttle < 0.0 {
        -4.5
    } else {
        16.0
    };
    let braking = old_speed * throttle < -0.1;
    let rate = if handbrake {
        18.0
    } else if braking {
        12.0
    } else if throttle == 0.0 {
        1.8
    } else {
        5.8
    };
    let speed = old_speed + (target - old_speed).clamp(-rate * dt, rate * dt);
    // Bicycle steering: reverse turns correctly and high-speed steering becomes gentler.
    let max_angle = 0.52 / (1.0 + speed.abs() * 0.052);
    *steering += (steer * max_angle - *steering) * blend(if steer == 0.0 { 11.0 } else { 7.5 }, dt);
    let yaw = (-speed / 2.55 * steering.tan()).clamp(-1.55, 1.55);
    let grip = if handbrake { 3.2 } else { 11.0 };
    (
        forward * speed + right * velocity.dot(right) * (-grip * dt).exp(),
        yaw,
    )
}

pub(crate) fn drive(
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    clock: Res<Time<Virtual>>,
    mut ride: ResMut<Ride>,
    jobs: Res<crate::jobs::Jobs>,
    terrain: Option<Res<crate::terrain::Landscape>>,
    bridges: Option<Res<crate::bridges::Bridges>>,
    window: Query<&Window, With<bevy::window::PrimaryWindow>>,
    mut actor: Query<(
        &Player,
        &mut Controller,
        &Transform,
        &mut LinearVelocity,
        &mut AngularVelocity,
    )>,
    mut reacted_to_bite: Local<f32>,
) {
    let Ok((player, mut controller, transform, mut linear, mut angular)) = actor.single_mut()
    else {
        return;
    };
    let dt = time.delta_secs();
    let active = ride.mouse_look
        && !jobs.interaction_active()
        && window.single().map_or(true, |window| window.focused);
    let axis = |positive, negative| {
        if active {
            f32::from(keys.pressed(positive)) - f32::from(keys.pressed(negative))
        } else {
            0.0
        }
    };
    let van = player.form == Avatar::Van;
    let throttle = (axis(KeyCode::KeyW, KeyCode::KeyS)
        + if van {
            axis(KeyCode::ArrowUp, KeyCode::ArrowDown)
        } else {
            0.0
        })
    .clamp(-1.0, 1.0);
    let steer = (axis(KeyCode::KeyD, KeyCode::KeyA)
        + if van {
            axis(KeyCode::ArrowRight, KeyCode::ArrowLeft)
        } else {
            0.0
        })
    .clamp(-1.0, 1.0);
    let forward = transform.forward().as_vec3();
    let right = transform.right().as_vec3();
    let mut horizontal = if van {
        let (velocity, turn) = van_velocity(
            linear.0.with_y(0.0),
            forward,
            right,
            throttle,
            steer,
            keys.pressed(KeyCode::Space) || !active,
            &mut controller.steering,
            dt,
        );
        angular.0.y = turn;
        velocity
    } else {
        let crouching = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
        let sprint = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
        let speed = match player.form {
            Avatar::Dog if sprint => 7.2,
            Avatar::Dog => 4.6,
            _ if player.photo || crouching => 1.65,
            _ if sprint => 5.4,
            _ => 3.3,
        };
        angular.0.y = axis(KeyCode::ArrowLeft, KeyCode::ArrowRight) * 1.7;
        let target = (forward * throttle + right * steer).clamp_length_max(1.0) * speed;
        linear.0.with_y(0.0).lerp(
            target,
            blend(if target == Vec3::ZERO { 22.0 } else { 15.0 }, dt),
        )
    };
    if clock.elapsed_secs() < player.stunned_until {
        if player.stunned_until > *reacted_to_bite {
            *reacted_to_bite = player.stunned_until;
            linear.0.y = linear.0.y.max(3.8);
            crate::platform::ui_event("paket-bite", "{}");
        }
        horizontal = Vec3::ZERO;
        angular.0.y = 0.0;
    }
    let current = tile_id(east_north(transform.translation));
    let next = tile_id(east_north(
        transform.translation + horizontal.normalize_or_zero(),
    ));
    ride.waiting_for_tile = current != next && !ride.loaded.contains_key(&next);
    if ride.waiting_for_tile
        || terrain.is_some_and(|terrain| {
            water_blocks_move(
                &terrain,
                bridges.as_deref(),
                transform.translation,
                transform.translation + horizontal * dt,
            )
        })
    {
        horizontal = Vec3::ZERO;
    }
    linear.0 = horizontal.with_y(linear.0.y);
}

fn water_blocks_move(
    terrain: &crate::terrain::Landscape,
    bridges: Option<&crate::bridges::Bridges>,
    from: Vec3,
    to: Vec3,
) -> bool {
    let deck = |p| bridges.and_then(|b| b.height_at(p)).is_some();
    terrain.water_at(east_north(to))
        && !deck(to)
        && (!terrain.water_at(east_north(from)) || deck(from))
}

pub(crate) fn camera_follow(
    time: Res<Time>,
    clock: Res<Time<Virtual>>,
    controls: Res<Controls>,
    keys: Res<ButtonInput<KeyCode>>,
    actor: Query<(Entity, &Player, &Controller, &Transform, &LinearVelocity)>,
    jobs: Res<crate::jobs::Jobs>,
    spatial: SpatialQuery,
    mut visuals: Query<&mut Visibility>,
    mut cameras: Query<
        (&mut Transform, &mut FollowCamera, &mut Projection),
        (With<Camera3d>, Without<Player>),
    >,
) {
    let (
        Ok((entity, player, controller, actor, velocity)),
        Ok((mut camera, mut rig, mut projection)),
    ) = (actor.single(), cameras.single_mut())
    else {
        return;
    };
    let dt = time.delta_secs();
    let shock = (player.stunned_until - clock.elapsed_secs()).clamp(0.0, 1.0);
    let jolt = Quat::from_rotation_z((clock.elapsed_secs() * 35.0).sin() * shock * 0.035)
        * Quat::from_rotation_x((clock.elapsed_secs() * 28.0).sin() * shock * 0.02);
    let forward = actor.forward().as_vec3();
    if let Some(view) = jobs.interaction_camera() {
        *camera = view;
        if let Ok(mut visible) = visuals.get_mut(player.visual) {
            *visible = Visibility::Hidden;
        }
        return;
    }
    let changed = rig.form != Some(player.form);
    rig.form = Some(player.form);
    let indoors = player.form == Avatar::Dog && jobs.dog_indoors(actor.translation);
    let target_fov: f32 = if player.photo {
        48.0
    } else if player.form == Avatar::Van {
        64.0 + (velocity.0.xz().length() / 16.0).clamp(0., 1.) * 5.0
    } else {
        64.0
    };
    if let Projection::Perspective(lens) = &mut *projection {
        lens.fov += (target_fov.to_radians() - lens.fov) * blend(9.0, dt);
    }
    if player.photo || indoors {
        if let Ok(mut visible) = visuals.get_mut(player.visual) {
            *visible = Visibility::Hidden;
        }
        let eye = actor.translation + Vec3::Y * if indoors { 0.22 } else { 0.69 };
        *camera = Transform::from_translation(eye + forward * if indoors { 0.18 } else { 0.12 })
            .with_rotation(actor.rotation * Quat::from_rotation_x(player.pitch) * jolt);
        return;
    }
    let crouching = player.form == Avatar::Courier
        && (keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight));
    let (back, height, shoulder) = match player.form {
        Avatar::Van => (7.1, 0.70, 0.0),
        Avatar::Dog => (2.5, 0.26, 0.14),
        Avatar::Courier => (3.6, if crouching { 0.22 } else { 0.61 }, 0.45),
    };
    let pivot = actor.translation + Vec3::Y * height;
    let look = actor.rotation
        * Quat::from_rotation_y(controller.van_look)
        * Quat::from_rotation_x(player.pitch - 0.18);
    let desired = pivot + look * Vec3::new(shoulder, 0.0, back * controls.zoom);
    let filter = SpatialQueryFilter::from_excluded_entities([entity]);
    let clip = |position: Vec3| {
        let delta = position - pivot;
        let Ok(direction) = Dir3::new(delta) else {
            return pivot;
        };
        let config = ShapeCastConfig::from_max_distance(delta.length());
        let hit = spatial.cast_shape(
            &Collider::sphere(0.22),
            pivot,
            Quat::IDENTITY,
            direction,
            &config,
            &filter,
        );
        pivot + direction * hit.map_or(delta.length(), |hit| (hit.distance - 0.07).max(0.05))
    };
    let safe = clip(desired);
    // Pull in immediately at a wall; ease back out. Clip the smoothed path as well.
    let position = if changed || camera.translation.distance(actor.translation) > 25.0 {
        safe
    } else {
        clip(camera.translation.lerp(safe, blend(12.0, dt)))
    };
    if let Ok(mut visible) = visuals.get_mut(player.visual) {
        *visible = if position.distance(pivot) < 0.75 {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        };
    }
    camera.translation = position;
    camera.look_to(look * Vec3::NEG_Z, Vec3::Y);
    camera.rotation *= jolt;
}

pub(crate) fn pose(
    player: &Player,
    transform: &Transform,
    velocity: &LinearVelocity,
    parked: Option<&Transform>,
) -> ActorPose {
    let point = transform.translation;
    let yaw = |t: &Transform| t.rotation.to_euler(EulerRot::YXZ).0;
    ActorPose {
        avatar: player.form,
        east: point.x,
        north: -point.z,
        up: point.y - half_height(player.form),
        heading: yaw(transform),
        velocity_east: velocity.0.x,
        velocity_north: -velocity.0.z,
        parked_van: parked
            .filter(|_| player.form != Avatar::Van)
            .map(|t| VanPose {
                east: t.translation.x,
                north: -t.translation.z,
                up: t.translation.y,
                heading: yaw(t),
            }),
    }
}

#[cfg(test)]
mod control_tests {
    use super::*;

    #[test]
    fn damping_is_independent_of_render_frame_rate() {
        let approach = |fps: usize| {
            let mut value = 0.0_f32;
            for _ in 0..fps {
                value += (1.0 - value) * blend(5.0, 1.0 / fps as f32);
            }
            value
        };
        assert!((approach(30) - approach(144)).abs() < 1e-5);
    }

    #[test]
    fn van_brakes_before_reversing_and_steering_reverses_with_it() {
        let mut steer = 0.0;
        let mut velocity = Vec3::NEG_Z * 10.0;
        for _ in 0..30 {
            (velocity, _) = van_velocity(
                velocity,
                Vec3::NEG_Z,
                Vec3::X,
                -1.,
                0.,
                false,
                &mut steer,
                1. / 60.,
            );
        }
        assert!(
            velocity.dot(Vec3::NEG_Z) > 3.5,
            "reverse input must first brake"
        );
        for _ in 0..120 {
            (velocity, _) = van_velocity(
                velocity,
                Vec3::NEG_Z,
                Vec3::X,
                -1.,
                0.,
                false,
                &mut steer,
                1. / 60.,
            );
        }
        assert!((velocity.dot(Vec3::NEG_Z) + 4.5).abs() < 0.01);
        let (_, forward_yaw) = van_velocity(
            Vec3::NEG_Z * 4.,
            Vec3::NEG_Z,
            Vec3::X,
            1.,
            1.,
            false,
            &mut 0.0,
            0.1,
        );
        let (_, reverse_yaw) = van_velocity(
            Vec3::Z * 4.,
            Vec3::NEG_Z,
            Vec3::X,
            -1.,
            1.,
            false,
            &mut 0.0,
            0.1,
        );
        assert!(forward_yaw < 0.0 && reverse_yaw > 0.0);
    }

    #[test]
    fn handbrake_stops_and_speed_is_bounded() {
        let mut steer = 0.0;
        let mut velocity = Vec3::ZERO;
        for _ in 0..600 {
            (velocity, _) = van_velocity(
                velocity,
                Vec3::NEG_Z,
                Vec3::X,
                1.,
                0.,
                false,
                &mut steer,
                1. / 60.,
            );
        }
        assert!((velocity.length() - 16.0).abs() < 0.01);
        for _ in 0..60 {
            (velocity, _) = van_velocity(
                velocity,
                Vec3::NEG_Z,
                Vec3::X,
                1.,
                0.,
                true,
                &mut steer,
                1. / 60.,
            );
        }
        assert!(velocity.length() < 0.01);
    }
}
