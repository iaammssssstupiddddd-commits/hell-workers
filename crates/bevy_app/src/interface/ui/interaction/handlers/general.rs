use bevy::ecs::query::QueryFilter;
use bevy::input_focus::InputFocus;
use bevy::prelude::*;
use hw_core::game_state::TimeSpeed;
use hw_ui::UiIntent;
use hw_ui::components::OperationDialogState;

use super::super::intent_context::{IntentFamiliarQueries, IntentSelectionCtx, IntentUiQueries};
use super::begin_overlay_open;
use crate::input_actions::PendingWorldInputCapture;

pub(crate) fn handle_selection(intent: UiIntent, ctx: &mut IntentSelectionCtx<'_, '_>) {
    match intent {
        UiIntent::FocusEntity(entity) => {
            if !ctx.resolved_frame.pointer_selection_suppressed() {
                hw_ui::list::focus_camera_on_entity(entity, &mut ctx.camera, &ctx.transforms);
            }
        }
        UiIntent::InspectEntity(entity) => {
            if ctx.resolved_frame.pointer_selection_suppressed() {
                return;
            }
            ctx.selected_entity.0 = Some(entity);
            ctx.info_panel_pin.entity = Some(entity);
        }
        UiIntent::ClearInspectPin => {
            ctx.info_panel_pin.entity = None;
        }
        _ => {}
    }
}

pub(crate) fn handle_dialog(
    intent: UiIntent,
    pending: &PendingWorldInputCapture,
    familiar_queries: &IntentFamiliarQueries<'_, '_>,
    dialog_state: &mut OperationDialogState,
    ui_queries: &mut IntentUiQueries<'_, '_>,
) {
    match intent {
        UiIntent::OpenOperationDialog { opener, target } => {
            let accepted = pending.accepts_operation(opener, target)
                && familiar_queries.q_familiar_settings.contains(target);
            if accepted {
                dialog_state.target = Some(target);
            }
            open_operation_dialog_with_focus(
                accepted,
                &mut ui_queries.q_dialog,
                &mut ui_queries.input_focus,
            );
        }
        UiIntent::CloseDialog => {
            dialog_state.target = None;
            hw_ui::interaction::dialog::close_operation_dialog(&mut ui_queries.q_dialog);
            for mut scroll in &mut ui_queries.q_operation_scroll {
                scroll.0 = Vec2::ZERO;
            }
        }
        _ => {}
    }
}

fn open_operation_dialog_with_focus<F: QueryFilter>(
    can_open: bool,
    q_dialog: &mut Query<&mut Node, F>,
    input_focus: &mut InputFocus,
) {
    if can_open && q_dialog.single().is_ok() {
        begin_overlay_open(input_focus);
        hw_ui::interaction::dialog::open_operation_dialog(q_dialog);
    }
}

pub(crate) fn toggle_system_menu(
    menu: &mut hw_ui::interaction::pause_menu::SystemMenuState,
    time: &mut Time<Virtual>,
) {
    if menu.open {
        menu.open = false;
        if let Some(speed) = menu.resume_speed.take() {
            time.set_relative_speed(speed);
            time.unpause();
        }
    } else {
        menu.open = true;
        menu.resume_speed = (!time.is_paused()).then_some(time.relative_speed());
        time.pause();
    }
}

pub(crate) fn handle_time(
    intent: UiIntent,
    time: &mut Time<Virtual>,
    _input_focus: &mut InputFocus,
    recovery_failed: bool,
) {
    if recovery_failed {
        time.pause();
        return;
    }
    match intent {
        UiIntent::TogglePause => {
            if time.is_paused() {
                time.unpause();
            } else {
                time.pause();
            }
        }
        UiIntent::SetTimeSpeed(TimeSpeed::Paused) => time.pause(),
        UiIntent::SetTimeSpeed(speed) => {
            time.set_relative_speed(match speed {
                TimeSpeed::Normal => 1.0,
                TimeSpeed::Fast => 2.0,
                TimeSpeed::Super => 4.0,
                TimeSpeed::Paused => unreachable!(),
            });
            time.unpause();
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input_actions::{InputAction, InputModifiers, ResolvedInputFrame};
    use crate::interface::ui::{EntityListNodeIndex, InfoPanelPinState};
    use crate::test_support::minimal_app;
    use hw_ui::components::OperationDialog;

    #[test]
    fn system_menu_restores_only_the_pause_it_owns_and_preserves_speed() {
        use hw_ui::interaction::pause_menu::SystemMenuState;
        for speed in [1.0, 2.0, 4.0] {
            for already_paused in [false, true] {
                let mut time = Time::<Virtual>::default();
                time.set_relative_speed(speed);
                if already_paused {
                    time.pause();
                }
                let mut menu = SystemMenuState::default();
                toggle_system_menu(&mut menu, &mut time);
                assert!(menu.open && time.is_paused());
                assert_eq!(menu.resume_speed, (!already_paused).then_some(speed));
                toggle_system_menu(&mut menu, &mut time);
                assert!(!menu.open && menu.resume_speed.is_none());
                assert_eq!(time.is_paused(), already_paused);
                assert_eq!(time.relative_speed(), speed);
            }
        }
    }

    #[test]
    fn world_replace_discards_system_menu_resume_ownership() {
        use hw_ui::interaction::pause_menu::SystemMenuState;
        let mut world = World::new();
        world.insert_resource(SystemMenuState {
            open: true,
            resume_speed: Some(4.0),
        });
        hw_ui::reset_for_world_replace(&mut world);
        let menu = world.resource::<SystemMenuState>();
        assert!(!menu.open && menu.resume_speed.is_none());
    }

    #[derive(Resource)]
    struct FocusTarget(Entity);

    fn focus_target(target: Res<FocusTarget>, mut selection: IntentSelectionCtx) {
        handle_selection(UiIntent::FocusEntity(target.0), &mut selection);
    }

    #[test]
    fn explicit_focus_uses_live_position_and_preserves_selection_and_pin() {
        let mut app = minimal_app();
        app.init_resource::<crate::interface::selection::SelectedEntity>()
            .init_resource::<InfoPanelPinState>()
            .init_resource::<ResolvedInputFrame>()
            .add_systems(Update, focus_target);
        let target = app
            .world_mut()
            .spawn(GlobalTransform::from_translation(Vec3::new(
                30.0, 40.0, 0.0,
            )))
            .id();
        let camera = app
            .world_mut()
            .spawn((
                hw_ui::camera::MainCamera,
                Transform::from_xyz(1.0, 2.0, 9.0),
            ))
            .id();
        let selected = app.world_mut().spawn_empty().id();
        let pinned = app.world_mut().spawn_empty().id();
        app.world_mut().insert_resource(FocusTarget(target));
        app.world_mut()
            .resource_mut::<crate::interface::selection::SelectedEntity>()
            .0 = Some(selected);
        app.world_mut().resource_mut::<InfoPanelPinState>().entity = Some(pinned);
        app.update();
        assert_eq!(
            app.world().get::<Transform>(camera).unwrap().translation,
            Vec3::new(30.0, 40.0, 9.0)
        );
        assert_eq!(
            app.world()
                .resource::<crate::interface::selection::SelectedEntity>()
                .0,
            Some(selected)
        );
        assert_eq!(
            app.world().resource::<InfoPanelPinState>().entity,
            Some(pinned)
        );
        app.world_mut()
            .entity_mut(target)
            .insert(GlobalTransform::from_translation(Vec3::new(
                60.0, 80.0, 0.0,
            )));
        app.world_mut()
            .resource_mut::<ResolvedInputFrame>()
            .replace(InputModifiers::default(), Vec::new(), None, true);
        app.update();
        assert_eq!(
            app.world().get::<Transform>(camera).unwrap().translation,
            Vec3::new(30.0, 40.0, 9.0)
        );
        app.world_mut()
            .resource_mut::<ResolvedInputFrame>()
            .replace(InputModifiers::default(), Vec::new(), None, false);
        app.update();
        assert_eq!(
            app.world().get::<Transform>(camera).unwrap().translation,
            Vec3::new(60.0, 80.0, 9.0)
        );
        app.world_mut().despawn(target);
        app.update();
        assert_eq!(
            app.world().get::<Transform>(camera).unwrap().translation,
            Vec3::new(60.0, 80.0, 9.0)
        );
    }

    fn inspect_placeholder(mut selection: IntentSelectionCtx) {
        handle_selection(UiIntent::InspectEntity(Entity::PLACEHOLDER), &mut selection);
    }

    fn inspect_app(pointer_selection_suppressed: bool) -> App {
        let mut app = minimal_app();
        app.init_resource::<crate::interface::selection::SelectedEntity>()
            .init_resource::<InfoPanelPinState>()
            .init_resource::<EntityListNodeIndex>()
            .init_resource::<ResolvedInputFrame>()
            .add_systems(Update, inspect_placeholder);
        if pointer_selection_suppressed {
            app.world_mut()
                .resource_mut::<ResolvedInputFrame>()
                .replace(
                    InputModifiers::default(),
                    vec![InputAction::FamiliarChop],
                    None,
                    true,
                );
        }
        app
    }

    #[test]
    fn inspect_intent_obeys_resolved_selection_suppression() {
        let mut suppressed = inspect_app(true);
        suppressed.update();
        assert!(
            suppressed
                .world()
                .resource::<crate::interface::selection::SelectedEntity>()
                .0
                .is_none()
        );

        let mut accepted = inspect_app(false);
        accepted.update();
        assert_eq!(
            accepted
                .world()
                .resource::<crate::interface::selection::SelectedEntity>()
                .0,
            Some(Entity::PLACEHOLDER)
        );
    }

    fn open_operation(
        mut q_dialog: Query<&mut Node, With<OperationDialog>>,
        mut input_focus: ResMut<InputFocus>,
    ) {
        open_operation_dialog_with_focus(true, &mut q_dialog, &mut input_focus);
    }

    fn reject_operation(
        mut q_dialog: Query<&mut Node, With<OperationDialog>>,
        mut input_focus: ResMut<InputFocus>,
    ) {
        open_operation_dialog_with_focus(false, &mut q_dialog, &mut input_focus);
    }

    #[test]
    fn accepted_operation_dialog_open_clears_input_focus() {
        let mut app = minimal_app();
        app.insert_resource(InputFocus::from_entity(Entity::PLACEHOLDER));
        let dialog = app
            .world_mut()
            .spawn((
                Node {
                    display: Display::None,
                    ..default()
                },
                OperationDialog,
            ))
            .id();
        app.add_systems(Update, open_operation);

        app.update();

        assert!(app.world().resource::<InputFocus>().get().is_none());
        assert_eq!(
            app.world().entity(dialog).get::<Node>().unwrap().display,
            Display::Flex
        );
    }

    #[test]
    fn rejected_operation_dialog_open_preserves_input_focus() {
        let mut app = minimal_app();
        app.insert_resource(InputFocus::from_entity(Entity::PLACEHOLDER));
        let dialog = app
            .world_mut()
            .spawn((
                Node {
                    display: Display::None,
                    ..default()
                },
                OperationDialog,
            ))
            .id();
        app.add_systems(Update, reject_operation);

        app.update();

        assert_eq!(
            app.world().resource::<InputFocus>().get(),
            Some(Entity::PLACEHOLDER)
        );
        assert_eq!(
            app.world().entity(dialog).get::<Node>().unwrap().display,
            Display::None
        );
    }

    #[test]
    fn time_pause_preserves_focus_and_does_not_open_a_menu() {
        let mut time = Time::<Virtual>::default();
        let mut focus = InputFocus::from_entity(Entity::PLACEHOLDER);

        handle_time(UiIntent::TogglePause, &mut time, &mut focus, false);
        assert!(time.is_paused());
        assert_eq!(focus.get(), Some(Entity::PLACEHOLDER));

        focus = InputFocus::from_entity(Entity::PLACEHOLDER);
        handle_time(UiIntent::TogglePause, &mut time, &mut focus, false);
        assert!(!time.is_paused());
        assert_eq!(focus.get(), Some(Entity::PLACEHOLDER));
    }
}
