// クリック、タブ、可視状態、ハイライト

use crate::camera::MainCamera;
use crate::components::{
    EntityListBody, LeftPanelMode, LeftPanelTabButton, TaskListBody, TaskListItem, UiInputState,
};
use crate::list::{RowHighlightState, apply_row_highlight, focus_camera_on_entity};
use crate::panels::info_panel::InfoPanelPinState;
use crate::theme::UiTheme;
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
    pin_state: Res<InfoPanelPinState>,
    q_changed: TaskChangedQuery,
    mut q_items: TaskListItemQuery<'_, '_>,
    theme: Res<UiTheme>,
) {
    if !pin_state.is_changed() && q_changed.is_empty() {
        return;
    }

    for (interaction, item, mut node, mut bg, mut border_color) in q_items.iter_mut() {
        let is_selected = pin_state.entity == Some(item.0);
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
    interactions: Query<(&Interaction, &LeftPanelTabButton), Changed<Interaction>>,
    tab_buttons: Query<(Entity, &LeftPanelTabButton, &Children)>,
    mut text_colors: Query<&mut TextColor>,
    mut border_colors: Query<&mut BorderColor>,
    ui_input_state: Res<UiInputState>,
) {
    if ui_input_state.world_input_captured {
        return;
    }
    for (interaction, tab) in &interactions {
        if *interaction == Interaction::Pressed && *mode != tab.0 {
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

pub fn left_panel_visibility_system(
    mode: Res<LeftPanelMode>,
    mut entity_list_bodies: Query<&mut Node, (With<EntityListBody>, Without<TaskListBody>)>,
    mut task_list_bodies: Query<&mut Node, (With<TaskListBody>, Without<EntityListBody>)>,
) {
    if !mode.is_changed() {
        return;
    }

    match *mode {
        LeftPanelMode::EntityList => {
            for mut node in &mut entity_list_bodies {
                if node.display != Display::Flex {
                    node.display = Display::Flex;
                }
            }
            for mut node in &mut task_list_bodies {
                if node.display != Display::None {
                    node.display = Display::None;
                }
            }
        }
        LeftPanelMode::TaskList => {
            for mut node in &mut entity_list_bodies {
                if node.display != Display::None {
                    node.display = Display::None;
                }
            }
            for mut node in &mut task_list_bodies {
                if node.display != Display::Flex {
                    node.display = Display::Flex;
                }
            }
        }
    }
}

pub fn task_list_click_system(
    mut pin_state: ResMut<InfoPanelPinState>,
    interactions: Query<(&Interaction, &TaskListItem), Changed<Interaction>>,
    mut camera_query: Query<&mut Transform, With<MainCamera>>,
    target_transforms: Query<&GlobalTransform, Without<MainCamera>>,
    ui_input_state: Res<UiInputState>,
) {
    if ui_input_state.world_input_captured {
        return;
    }
    for (interaction, item) in &interactions {
        if *interaction != Interaction::Pressed {
            continue;
        }

        let target_entity = item.0;
        focus_camera_on_entity(target_entity, &mut camera_query, &target_transforms);
        pin_state.entity = Some(target_entity);
    }
}

pub fn task_dashboard_control_system(
    interactions: Query<(&Interaction, &TaskDashboardControl), Changed<Interaction>>,
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
    for (interaction, control) in &interactions {
        if *interaction == Interaction::Pressed {
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
    pin_state: Res<InfoPanelPinState>,
    mode: Res<LeftPanelMode>,
    ui_input_state: Res<UiInputState>,
    mut action_state: ResMut<TaskDashboardActionState>,
    mut dirty: ResMut<TaskListDirty>,
) {
    let selection_or_panel_changed = pin_state.is_changed() || mode.is_changed();
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
    use crate::panels::task_list::{
        TaskActionButton, TaskActionButtonKind, TaskPriorityAdjustment,
    };
    use hw_core::jobs::WorkType;

    fn task_list_click_test_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<InfoPanelPinState>()
            .init_resource::<UiInputState>()
            .add_systems(Update, task_list_click_system);
        app
    }

    #[test]
    fn row_press_focuses_camera_and_pins_the_target() {
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

        assert_eq!(
            app.world().resource::<InfoPanelPinState>().entity,
            Some(target)
        );
        assert_eq!(
            app.world().get::<Transform>(camera).unwrap().translation,
            Vec3::new(30.0, 40.0, 9.0)
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
        app.add_plugins(MinimalPlugins)
            .init_resource::<InfoPanelPinState>()
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
