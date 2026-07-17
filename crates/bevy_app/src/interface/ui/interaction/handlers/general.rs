use bevy::ecs::query::QueryFilter;
use bevy::input_focus::InputFocus;
use bevy::prelude::*;
use hw_core::game_state::TimeSpeed;
use hw_ui::UiIntent;

use super::super::intent_context::{IntentSelectionCtx, IntentUiQueries};
use super::begin_overlay_open;

pub(crate) fn handle_selection(intent: UiIntent, ctx: &mut IntentSelectionCtx<'_>) {
    match intent {
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
    can_open_operation: bool,
    ui_queries: &mut IntentUiQueries<'_, '_>,
) {
    match intent {
        UiIntent::OpenOperationDialog => {
            open_operation_dialog_with_focus(
                can_open_operation,
                &mut ui_queries.q_dialog,
                &mut ui_queries.input_focus,
            );
        }
        UiIntent::CloseDialog => {
            hw_ui::interaction::dialog::close_operation_dialog(&mut ui_queries.q_dialog);
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

pub(crate) fn handle_time(
    intent: UiIntent,
    time: &mut Time<Virtual>,
    input_focus: &mut InputFocus,
) {
    match intent {
        UiIntent::TogglePause => {
            if time.is_paused() {
                time.unpause();
            } else {
                begin_overlay_open(input_focus);
                time.pause();
            }
        }
        UiIntent::SetTimeSpeed(speed) => match speed {
            TimeSpeed::Paused => {
                if !time.is_paused() {
                    begin_overlay_open(input_focus);
                }
                time.pause();
            }
            TimeSpeed::Normal => {
                time.unpause();
                time.set_relative_speed(1.0);
            }
            TimeSpeed::Fast => {
                time.unpause();
                time.set_relative_speed(2.0);
            }
            TimeSpeed::Super => {
                time.unpause();
                time.set_relative_speed(4.0);
            }
        },
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
    fn opening_pause_clears_focus_but_resuming_does_not_reclear_it() {
        let mut time = Time::<Virtual>::default();
        let mut focus = InputFocus::from_entity(Entity::PLACEHOLDER);

        handle_time(UiIntent::TogglePause, &mut time, &mut focus);
        assert!(time.is_paused());
        assert!(focus.get().is_none());

        focus = InputFocus::from_entity(Entity::PLACEHOLDER);
        handle_time(UiIntent::TogglePause, &mut time, &mut focus);
        assert!(!time.is_paused());
        assert_eq!(focus.get(), Some(Entity::PLACEHOLDER));
    }
}
