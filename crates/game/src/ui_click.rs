//! Keep completed pointer clicks usable when press and release share one frame.
use bevy::{
    picking::{
        PickingSystems,
        events::{PointerClick, pointer_events},
        hover::{HoverMap, PreviousHoverMap, generate_hovermap},
        pointer::{Location, PointerAction, PointerButton, PointerId, PointerInput},
    },
    prelude::*,
    ui::{InteractionDisabled, Pressed},
    ui_widgets::{Activate, ActivateOnPress, Button, Checkbox},
};

pub(super) fn install(app: &mut App) {
    app.add_observer(complete_fast_click).add_systems(
        PreUpdate,
        reconcile_completed_ui_click
            .in_set(PickingSystems::Hover)
            .after(generate_hovermap)
            .before(pointer_events),
    );
}

// Bevy 0.20 presses current hits but releases previous-frame hits. A pointer
// that moves onto a new button and completes its click within one frame has no
// previous hit, so Click is never emitted. Reconcile only that completed UI
// gesture; keep the engine's event routing, occlusion and activation policies.
fn reconcile_completed_ui_click(
    mut inputs: MessageReader<PointerInput>,
    current: Res<HoverMap>,
    mut previous: ResMut<PreviousHoverMap>,
    widgets: Query<(Has<Node>, Has<InteractionDisabled>), Or<(With<Button>, With<Checkbox>)>>,
    parents: Query<&ChildOf>,
) {
    let mut presses = std::collections::HashMap::<PointerId, Location>::new();
    for input in inputs.read() {
        match input.action {
            PointerAction::Press(PointerButton::Primary) => {
                presses.insert(input.pointer_id, input.location.clone());
            }
            PointerAction::Move { .. } => {
                if presses.get(&input.pointer_id).is_some_and(|start| {
                    start.target != input.location.target
                        || start.position.distance_squared(input.location.position) > 16.
                }) {
                    presses.remove(&input.pointer_id);
                }
            }
            PointerAction::Cancel => {
                presses.remove(&input.pointer_id);
            }
            PointerAction::Release(PointerButton::Primary) => {
                let Some(start) = presses.remove(&input.pointer_id) else {
                    continue;
                };
                if start.target != input.location.target
                    || start.position.distance_squared(input.location.position) > 16.
                {
                    continue;
                }
                let Some(hits) = current.get(&input.pointer_id) else {
                    continue;
                };
                for (hit, data) in hits {
                    let mut ancestor = *hit;
                    for _ in 0..32 {
                        if let Ok((ui, disabled)) = widgets.get(ancestor) {
                            if ui && !disabled {
                                previous
                                    .entry(input.pointer_id)
                                    .or_default()
                                    .insert(*hit, data.clone());
                            }
                            break;
                        }
                        let Ok(parent) = parents.get(ancestor) else {
                            break;
                        };
                        ancestor = parent.parent();
                    }
                }
            }
            _ => {}
        }
    }
}

fn complete_fast_click(
    mut click: On<PointerClick>,
    buttons: Query<(Has<Pressed>, Has<InteractionDisabled>, Has<ActivateOnPress>), With<Button>>,
    mut commands: Commands,
) {
    if let Ok((pressed, disabled, activate_on_press)) = buttons.get(click.entity) {
        click.propagate(false);
        // The built-in widget handles clicks while Pressed still exists. A fast
        // release can remove it before Click dispatch; activate only that case.
        if !pressed && !disabled && !activate_on_press {
            commands.trigger(Activate {
                entity: click.entity,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::{
        camera::NormalizedRenderTarget,
        picking::{InteractionPlugin, PickingPlugin, backend::PointerHits},
        picking::{
            backend::HitData,
            events::{Pointer, PointerPress},
            pointer::{Location, PointerButton, PointerId},
        },
        ui_widgets::ButtonPlugin,
        window::WindowRef,
    };
    use std::time::Duration;

    #[derive(Resource, Default)]
    struct Activations(usize);
    fn widget_app() -> (App, Entity) {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            PickingPlugin,
            InteractionPlugin,
            ButtonPlugin,
        ))
        .init_resource::<Activations>();
        install(&mut app);
        app.add_observer(|_: On<Activate>, mut activations: ResMut<Activations>| {
            activations.0 += 1
        });
        let window = app.world_mut().spawn(Window::default()).id();
        app.world_mut().spawn(PointerId::Mouse);
        (app, window)
    }
    fn pointer(window: Entity) -> Pointer {
        Pointer::new(
            PointerId::Mouse,
            Location {
                target: NormalizedRenderTarget::Window(
                    WindowRef::Entity(window).normalize(Some(window)).unwrap(),
                ),
                position: Vec2::ZERO,
            },
        )
    }
    fn click(app: &mut App, button: Entity, window: Entity, milliseconds: u64) {
        app.world_mut().trigger(PointerClick {
            entity: button,
            pointer: pointer(window),
            button: PointerButton::Primary,
            hit: HitData::new(window, 0., None, None),
            duration: Duration::from_millis(milliseconds),
            count: 1,
        });
        app.update();
    }

    #[test]
    fn completed_click_without_pressed_activates_once_and_normal_click_does_not_duplicate() {
        // Given the real ButtonPlugin and a completed click whose press state
        // disappeared in the same frame, When Click arrives, Then Activate once.
        let (mut app, window) = widget_app();
        let fast = app.world_mut().spawn(Button).id();
        click(&mut app, fast, window, 0);
        assert_eq!(app.world().resource::<Activations>().0, 1);
        let held = app.world_mut().spawn((Button, Pressed)).id();
        click(&mut app, held, window, 80);
        assert_eq!(app.world().resource::<Activations>().0, 2);
    }

    #[test]
    fn disabled_and_activate_on_press_buttons_keep_their_existing_policy() {
        let (mut app, window) = widget_app();
        let disabled = app.world_mut().spawn((Button, InteractionDisabled)).id();
        click(&mut app, disabled, window, 0);
        assert_eq!(app.world().resource::<Activations>().0, 0);
        let immediate = app.world_mut().spawn((Button, ActivateOnPress)).id();
        app.world_mut().trigger(PointerPress {
            entity: immediate,
            pointer: pointer(window),
            button: PointerButton::Primary,
            hit: HitData::new(window, 0., None, None),
            count: 1,
        });
        app.update();
        assert_eq!(app.world().resource::<Activations>().0, 1);
        click(&mut app, immediate, window, 0);
        assert_eq!(app.world().resource::<Activations>().0, 1);
    }
    fn pointer_input(app: &mut App, window: Entity, action: PointerAction, position: Vec2) {
        app.world_mut().write_message(PointerInput::new(
            PointerId::Mouse,
            Location {
                target: NormalizedRenderTarget::Window(
                    WindowRef::Entity(window).normalize(Some(window)).unwrap(),
                ),
                position,
            },
            action,
        ));
    }
    fn ui_hit(app: &mut App, window: Entity, target: Entity) {
        app.world_mut().write_message(PointerHits::new(
            PointerId::Mouse,
            vec![(target, HitData::new(window, 0., None, None))],
            100.,
        ));
    }
    #[test]
    fn same_frame_pointer_press_release_on_new_button_emits_one_activation_through_picking() {
        // Given no prior hover on the button, When the backend reports a new
        // child hit and PointerInput press/release arrive together, Then the
        // real picking and ButtonPlugin pipeline activates the ancestor once.
        let (mut app, window) = widget_app();
        app.update();
        let button = app.world_mut().spawn((Button, Node::default())).id();
        let child = app
            .world_mut()
            .spawn((Node::default(), ChildOf(button)))
            .id();
        ui_hit(&mut app, window, child);
        pointer_input(
            &mut app,
            window,
            PointerAction::Press(PointerButton::Primary),
            Vec2::new(100., 30.),
        );
        pointer_input(
            &mut app,
            window,
            PointerAction::Release(PointerButton::Primary),
            Vec2::new(100., 30.),
        );
        app.update();
        assert_eq!(app.world().resource::<Activations>().0, 1);
        // A held gesture takes another frame and stays on the standard path.
        ui_hit(&mut app, window, child);
        pointer_input(
            &mut app,
            window,
            PointerAction::Press(PointerButton::Primary),
            Vec2::new(100., 30.),
        );
        app.update();
        ui_hit(&mut app, window, child);
        pointer_input(
            &mut app,
            window,
            PointerAction::Release(PointerButton::Primary),
            Vec2::new(100., 30.),
        );
        app.update();
        assert_eq!(app.world().resource::<Activations>().0, 2);
    }
    #[test]
    fn compressed_pointer_cancel_drag_and_disabled_button_do_not_activate() {
        for cancelled in [true, false] {
            let (mut app, window) = widget_app();
            app.update();
            let button = app.world_mut().spawn((Button, Node::default())).id();
            ui_hit(&mut app, window, button);
            pointer_input(
                &mut app,
                window,
                PointerAction::Press(PointerButton::Primary),
                Vec2::new(100., 30.),
            );
            if cancelled {
                pointer_input(
                    &mut app,
                    window,
                    PointerAction::Cancel,
                    Vec2::new(100., 30.),
                );
            } else {
                pointer_input(
                    &mut app,
                    window,
                    PointerAction::Move {
                        delta: Vec2::new(30., 0.),
                    },
                    Vec2::new(130., 30.),
                );
                pointer_input(
                    &mut app,
                    window,
                    PointerAction::Release(PointerButton::Primary),
                    Vec2::new(130., 30.),
                );
            }
            app.update();
            assert_eq!(app.world().resource::<Activations>().0, 0);
        }
        let (mut app, window) = widget_app();
        app.update();
        let disabled = app
            .world_mut()
            .spawn((Button, Node::default(), InteractionDisabled))
            .id();
        ui_hit(&mut app, window, disabled);
        pointer_input(
            &mut app,
            window,
            PointerAction::Press(PointerButton::Primary),
            Vec2::ZERO,
        );
        pointer_input(
            &mut app,
            window,
            PointerAction::Release(PointerButton::Primary),
            Vec2::ZERO,
        );
        app.update();
        assert_eq!(app.world().resource::<Activations>().0, 0);
    }
}
