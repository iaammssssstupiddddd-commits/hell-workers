// クリック、タブ、可視状態、ハイライト

use crate::components::{
    EntityListBody, EntityListSearchRow, LeftPanelMode, LeftPanelTabButton, TaskListBody,
    TaskListItem, UiInputState,
};
use crate::list::EntityListMinimizeState;
use crate::list::{RowHighlightState, apply_row_highlight};
use crate::selection::SelectedEntity;
use crate::theme::UiTheme;
use crate::widgets::text_field::TextFieldRole;
use bevy::input_focus::InputFocus;
use bevy::prelude::*;

use super::types::{
    TaskDashboardActionState, TaskDashboardControl, TaskDashboardViewState, TaskListDirty,
};

type TaskListItemQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static Interaction,
        &'static TaskListItem,
        &'static mut Node,
        &'static mut BackgroundColor,
        &'static mut BorderColor,
    ),
    With<Button>,
>;

type TaskChangedQuery<'w, 's> = Query<'w, 's, (), Or<(Changed<Interaction>, Added<TaskListItem>)>>;

pub fn task_list_visual_feedback_system(
    action_state: Res<TaskDashboardActionState>,
    q_changed: TaskChangedQuery,
    mut q_items: TaskListItemQuery<'_, '_>,
    theme: Res<UiTheme>,
) {
    if !action_state.is_changed() && q_changed.is_empty() {
        return;
    }

    for (interaction, item, mut node, mut bg, mut border_color) in q_items.iter_mut() {
        let is_selected = action_state.active_task == Some(item.0);
        apply_row_highlight(
            &mut node,
            &mut bg,
            &mut border_color,
            RowHighlightState {
                interaction: *interaction,
                is_selected,
                is_drop_target: false,
                is_familiar_row: false,
            },
            &theme,
        );
    }
}

pub fn left_panel_tab_system(
    mut mode: ResMut<LeftPanelMode>,
    theme: Res<UiTheme>,
    interactions: Query<(Entity, &LeftPanelTabButton)>,
    tab_buttons: Query<(Entity, &LeftPanelTabButton, &Children)>,
    mut text_colors: Query<&mut TextColor>,
    mut border_colors: Query<&mut BorderColor>,
    ui_input_state: Res<UiInputState>,
) {
    if ui_input_state.world_input_captured {
        return;
    }
    for (entity, tab) in &interactions {
        if ui_input_state.button_activated(entity) && *mode != tab.0 {
            *mode = tab.0;
        }
    }

    if mode.is_changed() {
        for (button_entity, tab, children) in &tab_buttons {
            let is_active = tab.0 == *mode;

            if let Some(child) = children.iter().next()
                && let Ok(mut color) = text_colors.get_mut(child)
            {
                color.0 = if is_active {
                    theme.colors.text_accent_semantic
                } else {
                    theme.colors.text_secondary_semantic
                };
            }

            if let Ok(mut border) = border_colors.get_mut(button_entity) {
                *border = BorderColor::all(if is_active {
                    theme.colors.text_accent_semantic
                } else {
                    Color::NONE
                });
            }
        }
    }
}

type LeftPanelVisibilityQuery<'w, 's> = Query<
    'w,
    's,
    (&'static mut Node, Has<TaskListBody>),
    Or<(
        With<EntityListBody>,
        With<TaskListBody>,
        With<EntityListSearchRow>,
    )>,
>;

pub fn left_panel_visibility_system(
    mode: Res<LeftPanelMode>,
    minimized: Res<EntityListMinimizeState>,
    mut nodes: LeftPanelVisibilityQuery,
    mut focus: ResMut<InputFocus>,
    fields: Query<&TextFieldRole>,
) {
    let entities_visible = !minimized.minimized && *mode == LeftPanelMode::EntityList;
    if !entities_visible
        && focus.get().and_then(|entity| fields.get(entity).ok())
            == Some(&TextFieldRole::EntityListSearch)
    {
        focus.clear();
    }
    if !mode.is_changed() && !minimized.is_changed() {
        return;
    }
    for (mut node, is_tasks) in &mut nodes {
        let visible = !minimized.minimized
            && if is_tasks {
                *mode == LeftPanelMode::TaskList
            } else {
                entities_visible
            };
        let display = if visible {
            Display::Flex
        } else {
            Display::None
        };
        if node.display != display {
            node.display = display;
        }
    }
}

pub fn task_list_click_system(
    mut selected: ResMut<SelectedEntity>,
    mut action_state: ResMut<TaskDashboardActionState>,
    mut dirty: ResMut<TaskListDirty>,
    interactions: Query<(Entity, &TaskListItem)>,
    ui_input_state: Res<UiInputState>,
) {
    if ui_input_state.world_input_captured {
        return;
    }
    for (entity, item) in &interactions {
        if !ui_input_state.button_activated(entity) {
            continue;
        }

        let target_entity = item.0;
        selected.0 = Some(target_entity);
        action_state.active_task = Some(target_entity);
        action_state.confirmation = None;
        dirty.mark_list();
    }
}

pub fn task_dashboard_control_system(
    interactions: Query<(Entity, &TaskDashboardControl)>,
    ui_input_state: Res<UiInputState>,
    mode: Res<LeftPanelMode>,
    mut view_state: ResMut<TaskDashboardViewState>,
    mut action_state: ResMut<TaskDashboardActionState>,
    mut dirty: ResMut<TaskListDirty>,
) {
    if ui_input_state.world_input_captured || *mode != LeftPanelMode::TaskList {
        return;
    }

    let mut changed = false;
    for (entity, control) in &interactions {
        if ui_input_state.button_activated(entity) {
            view_state.apply_control(*control);
            changed = true;
        }
    }
    if changed {
        action_state.confirmation = None;
        dirty.mark_list();
    }
}

pub fn task_dashboard_action_state_sync_system(
    selected: Res<SelectedEntity>,
    mode: Res<LeftPanelMode>,
    ui_input_state: Res<UiInputState>,
    mut action_state: ResMut<TaskDashboardActionState>,
    mut dirty: ResMut<TaskListDirty>,
) {
    let selection_or_panel_changed = selected.is_changed() || mode.is_changed();
    if selected.is_changed() && action_state.active_task != selected.0 {
        action_state.active_task = None;
    }
    let should_clear = selection_or_panel_changed || ui_input_state.world_input_capture_started;
    if should_clear && action_state.confirmation.take().is_some() {
        dirty.mark_list();
    }
    if selection_or_panel_changed {
        dirty.mark_list();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::camera::MainCamera;
    use crate::panels::info_panel::InfoPanelPinState;

    #[test]
    fn tab_and_minimize_have_one_visibility_owner_and_clear_hidden_search_focus() {
        let mut app = App::new();
        crate::accepted_button_fixture(&mut app);
        app.add_plugins(MinimalPlugins)
            .init_resource::<LeftPanelMode>()
            .init_resource::<EntityListMinimizeState>()
            .init_resource::<InputFocus>()
            .add_systems(Update, left_panel_visibility_system);
        let entities = app
            .world_mut()
            .spawn((Node::default(), EntityListBody))
            .id();
        let tasks = app.world_mut().spawn((Node::default(), TaskListBody)).id();
        let search = app
            .world_mut()
            .spawn((Node::default(), EntityListSearchRow))
            .id();
        let field = app.world_mut().spawn(TextFieldRole::EntityListSearch).id();
        for (mode, minimized, entity_display, task_display) in [
            (
                LeftPanelMode::EntityList,
                false,
                Display::Flex,
                Display::None,
            ),
            (LeftPanelMode::TaskList, false, Display::None, Display::Flex),
            (LeftPanelMode::TaskList, true, Display::None, Display::None),
            (
                LeftPanelMode::EntityList,
                true,
                Display::None,
                Display::None,
            ),
            (
                LeftPanelMode::EntityList,
                false,
                Display::Flex,
                Display::None,
            ),
        ] {
            *app.world_mut().resource_mut::<LeftPanelMode>() = mode;
            app.world_mut()
                .resource_mut::<EntityListMinimizeState>()
                .minimized = minimized;
            app.world_mut()
                .resource_mut::<InputFocus>()
                .set(field, bevy::input_focus::FocusCause::Navigated);
            app.update();
            assert_eq!(
                app.world().get::<Node>(entities).unwrap().display,
                entity_display
            );
            assert_eq!(
                app.world().get::<Node>(search).unwrap().display,
                entity_display
            );
            assert_eq!(
                app.world().get::<Node>(tasks).unwrap().display,
                task_display
            );
            assert_eq!(
                app.world().resource::<InputFocus>().get(),
                (entity_display == Display::Flex).then_some(field)
            );
        }
    }
    use crate::panels::task_list::{
        TaskActionButton, TaskActionButtonKind, TaskPriorityAdjustment,
    };
    use hw_core::jobs::WorkType;

    fn task_list_click_test_app() -> App {
        let mut app = App::new();
        crate::accepted_button_fixture(&mut app);
        app.add_plugins(MinimalPlugins)
            .init_resource::<InfoPanelPinState>()
            .init_resource::<SelectedEntity>()
            .init_resource::<TaskDashboardActionState>()
            .init_resource::<TaskListDirty>()
            .init_resource::<UiInputState>()
            .add_systems(Update, task_list_click_system);
        app
    }

    #[test]
    fn row_press_selects_task_without_moving_camera_or_replacing_pin() {
        let mut app = task_list_click_test_app();
        let camera = app
            .world_mut()
            .spawn((MainCamera, Transform::from_xyz(1.0, 2.0, 9.0)))
            .id();
        let target = app
            .world_mut()
            .spawn(GlobalTransform::from(Transform::from_xyz(30.0, 40.0, 0.0)))
            .id();
        app.world_mut()
            .spawn((Interaction::Pressed, TaskListItem(target)));

        app.update();

        assert_eq!(app.world().resource::<SelectedEntity>().0, Some(target));
        assert_eq!(
            app.world()
                .resource::<TaskDashboardActionState>()
                .active_task,
            Some(target)
        );
        assert_eq!(app.world().resource::<InfoPanelPinState>().entity, None);
        assert_eq!(
            app.world().get::<Transform>(camera).unwrap().translation,
            Vec3::new(1.0, 2.0, 9.0)
        );
    }

    #[test]
    fn action_button_press_does_not_trigger_row_focus() {
        let mut app = task_list_click_test_app();
        let camera = app
            .world_mut()
            .spawn((MainCamera, Transform::from_xyz(1.0, 2.0, 9.0)))
            .id();
        let target = app
            .world_mut()
            .spawn(GlobalTransform::from(Transform::from_xyz(30.0, 40.0, 0.0)))
            .id();
        app.world_mut().spawn((
            Interaction::Pressed,
            TaskActionButton {
                target,
                expected_work_type: WorkType::Chop,
                kind: TaskActionButtonKind::AdjustPriority(TaskPriorityAdjustment::Increase),
            },
        ));

        app.update();

        assert_eq!(app.world().resource::<InfoPanelPinState>().entity, None);
        assert_eq!(
            app.world().get::<Transform>(camera).unwrap().translation,
            Vec3::new(1.0, 2.0, 9.0)
        );
    }

    #[test]
    fn captured_row_press_is_drained_without_delayed_focus() {
        let mut app = task_list_click_test_app();
        let camera = app
            .world_mut()
            .spawn((MainCamera, Transform::from_xyz(1.0, 2.0, 9.0)))
            .id();
        let target = app
            .world_mut()
            .spawn(GlobalTransform::from(Transform::from_xyz(30.0, 40.0, 0.0)))
            .id();
        app.world_mut()
            .resource_mut::<UiInputState>()
            .world_input_captured = true;
        app.world_mut()
            .spawn((Interaction::Pressed, TaskListItem(target)));

        app.update();
        app.world_mut()
            .resource_mut::<UiInputState>()
            .world_input_captured = false;
        app.update();

        assert_eq!(app.world().resource::<InfoPanelPinState>().entity, None);
        assert_eq!(
            app.world().get::<Transform>(camera).unwrap().translation,
            Vec3::new(1.0, 2.0, 9.0)
        );
    }

    #[test]
    fn captured_toolbar_press_is_not_applied_after_capture_ends() {
        let mut app = App::new();
        crate::accepted_button_fixture(&mut app);
        app.add_plugins(MinimalPlugins)
            .insert_resource(LeftPanelMode::TaskList)
            .init_resource::<UiInputState>()
            .init_resource::<TaskDashboardViewState>()
            .init_resource::<TaskDashboardActionState>()
            .init_resource::<TaskListDirty>()
            .add_systems(Update, task_dashboard_control_system);
        app.update();

        app.world_mut()
            .resource_mut::<UiInputState>()
            .world_input_captured = true;
        app.world_mut()
            .spawn((Interaction::Pressed, TaskDashboardControl::StatusFilter));
        app.update();
        app.world_mut()
            .resource_mut::<UiInputState>()
            .world_input_captured = false;
        app.update();

        assert_eq!(
            *app.world().resource::<TaskDashboardViewState>(),
            TaskDashboardViewState::default()
        );
    }

    #[test]
    fn capture_start_clears_pending_cancellation_confirmation() {
        let target = Entity::from_raw_u32(12).expect("valid test target");
        let mut app = App::new();
        crate::accepted_button_fixture(&mut app);
        app.add_plugins(MinimalPlugins)
            .init_resource::<InfoPanelPinState>()
            .init_resource::<SelectedEntity>()
            .insert_resource(LeftPanelMode::TaskList)
            .init_resource::<UiInputState>()
            .init_resource::<TaskDashboardActionState>()
            .init_resource::<TaskListDirty>()
            .add_systems(Update, task_dashboard_action_state_sync_system);
        app.update();

        app.world_mut()
            .resource_mut::<TaskDashboardActionState>()
            .confirmation = Some(super::super::types::PendingTaskCancellation {
            target,
            expected_work_type: WorkType::Chop,
            kind: super::super::types::TaskCancelKind::GenericDesignation,
        });
        app.world_mut()
            .resource_mut::<UiInputState>()
            .world_input_capture_started = true;
        app.update();

        assert!(
            app.world()
                .resource::<TaskDashboardActionState>()
                .confirmation
                .is_none()
        );
        assert!(app.world().resource::<TaskListDirty>().list_dirty());
    }
}
