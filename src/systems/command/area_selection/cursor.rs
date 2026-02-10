use super::geometry::{cursor_icon_for_operation, detect_area_edit_operation, world_cursor_pos};
use super::state::AreaEditSession;
use crate::entities::familiar::Familiar;
use crate::game_state::TaskContext;
use crate::interface::selection::SelectedEntity;
use crate::interface::ui::UiInputState;
use crate::systems::command::{TaskArea, TaskMode};
use bevy::prelude::*;
use bevy::window::{CursorIcon, PrimaryWindow, SystemCursorIcon};

pub fn task_area_edit_cursor_system(
    task_context: Res<TaskContext>,
    selected: Res<SelectedEntity>,
    ui_input_state: Res<UiInputState>,
    area_edit_session: Res<AreaEditSession>,
    q_task_areas: Query<&TaskArea, With<Familiar>>,
    q_window_entity: Query<Entity, With<PrimaryWindow>>,
    q_window: Query<&Window, With<PrimaryWindow>>,
    q_camera: Query<(&Camera, &GlobalTransform), With<crate::interface::camera::MainCamera>>,
    mut q_cursor: Query<&mut CursorIcon, With<PrimaryWindow>>,
    mut commands: Commands,
) {
    if ui_input_state.pointer_over_ui {
        return;
    }

    let Ok(window_entity) = q_window_entity.single() else {
        return;
    };

    let desired = if !matches!(task_context.0, TaskMode::AreaSelection(_)) {
        CursorIcon::System(SystemCursorIcon::Default)
    } else if let Some(active_drag) = area_edit_session.active_drag.as_ref() {
        cursor_icon_for_operation(active_drag.operation, true)
    } else if let (Some(fam_entity), Some(world_pos)) =
        (selected.0, world_cursor_pos(&q_window, &q_camera))
    {
        if let Ok(area) = q_task_areas.get(fam_entity) {
            if let Some(operation) = detect_area_edit_operation(area, world_pos) {
                cursor_icon_for_operation(operation, false)
            } else {
                CursorIcon::System(SystemCursorIcon::Default)
            }
        } else {
            CursorIcon::System(SystemCursorIcon::Default)
        }
    } else {
        CursorIcon::System(SystemCursorIcon::Default)
    };

    if let Ok(mut icon) = q_cursor.get_mut(window_entity) {
        if *icon != desired {
            *icon = desired;
        }
    } else {
        commands.entity(window_entity).insert(desired);
    }
}
