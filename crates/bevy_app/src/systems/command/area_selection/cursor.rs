use super::AreaEditSession;
use crate::app_contexts::TaskContext;
use crate::entities::familiar::Familiar;
use crate::interface::selection::SelectedEntity;
use crate::interface::ui::UiInputState;
use crate::systems::command::{TaskArea, TaskMode};
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::window::{CursorIcon, PrimaryWindow, SystemCursorIcon};
use hw_ui::area_edit::{cursor_icon_for_operation, detect_area_edit_operation};
use hw_ui::camera::{MainCamera, world_cursor_pos};

#[derive(SystemParam)]
pub struct CursorState<'w, 's> {
    task_context: Res<'w, TaskContext>,
    selected: Res<'w, SelectedEntity>,
    ui_input_state: Res<'w, UiInputState>,
    area_edit_session: Res<'w, AreaEditSession>,
    q_task_areas: Query<'w, 's, &'static TaskArea, With<Familiar>>,
    q_window_entity: Query<'w, 's, Entity, With<PrimaryWindow>>,
    q_window: Query<'w, 's, &'static Window, With<PrimaryWindow>>,
    q_camera: Query<'w, 's, (&'static Camera, &'static GlobalTransform), With<MainCamera>>,
}

pub fn task_area_edit_cursor_system(
    state: CursorState,
    mut q_cursor: Query<&mut CursorIcon, With<PrimaryWindow>>,
    mut commands: Commands,
) {
    if state.ui_input_state.pointer_over_ui {
        return;
    }

    let Ok(window_entity) = state.q_window_entity.single() else {
        return;
    };

    let desired = if !matches!(state.task_context.0, TaskMode::AreaSelection(_)) {
        CursorIcon::System(SystemCursorIcon::Default)
    } else if let Some(active_drag) = state.area_edit_session.active_drag.as_ref() {
        cursor_icon_for_operation(active_drag.operation, true)
    } else if let (Some(fam_entity), Some(world_pos)) = (
        state.selected.0,
        world_cursor_pos(&state.q_window, &state.q_camera),
    ) {
        if let Ok(area) = state.q_task_areas.get(fam_entity) {
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
