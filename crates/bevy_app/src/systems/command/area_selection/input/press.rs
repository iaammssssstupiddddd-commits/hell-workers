use crate::app_contexts::TaskContext;
use crate::entities::familiar::Familiar;
use crate::systems::command::{TaskArea, TaskMode};
use crate::world::map::WorldMap;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use hw_ui::area_edit::{AreaEditDrag, AreaEditSession, detect_area_edit_operation};
use hw_ui::camera::{MainCamera, world_cursor_pos};

pub(super) fn try_start_direct_edit_drag(
    task_context: TaskMode,
    selected: Option<Entity>,
    q_familiar_areas: &Query<&TaskArea, With<Familiar>>,
    world_pos: Vec2,
    snapped_pos: Vec2,
    area_edit_session: &mut AreaEditSession,
) -> bool {
    if !matches!(task_context, TaskMode::AreaSelection(None)) {
        return false;
    }

    let Some(fam_entity) = selected else {
        return false;
    };
    let Ok(existing_area) = q_familiar_areas.get(fam_entity) else {
        return false;
    };
    let Some(operation) = detect_area_edit_operation(existing_area, world_pos) else {
        return false;
    };

    area_edit_session.active_drag = Some(AreaEditDrag {
        familiar_entity: fam_entity,
        operation,
        original_area: existing_area.clone(),
        drag_start: snapped_pos,
    });

    true
}

pub(super) fn handle_left_just_pressed_input(
    task_context: &mut TaskContext,
    selected_entity: Option<Entity>,
    q_familiar_areas: &Query<&TaskArea, With<Familiar>>,
    q_window: &Query<&Window, With<PrimaryWindow>>,
    q_camera: &Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    area_edit_session: &mut AreaEditSession,
) -> bool {
    let Some(world_pos) = world_cursor_pos(q_window, q_camera) else {
        return false;
    };
    let snapped_pos = WorldMap::snap_to_grid_edge(world_pos);
    let snapped_center = WorldMap::snap_to_grid_center(world_pos);

    if try_start_direct_edit_drag(
        task_context.0,
        selected_entity,
        q_familiar_areas,
        world_pos,
        snapped_pos,
        area_edit_session,
    ) {
        return true;
    }

    match task_context.0 {
        TaskMode::AreaSelection(None) => {
            task_context.0 = TaskMode::AreaSelection(Some(snapped_pos))
        }
        TaskMode::DesignateChop(None) => {
            task_context.0 = TaskMode::DesignateChop(Some(snapped_pos))
        }
        TaskMode::DesignateMine(None) => {
            task_context.0 = TaskMode::DesignateMine(Some(snapped_pos))
        }
        TaskMode::DesignateHaul(None) => {
            task_context.0 = TaskMode::DesignateHaul(Some(snapped_pos))
        }
        TaskMode::CancelDesignation(None) => {
            task_context.0 = TaskMode::CancelDesignation(Some(snapped_pos))
        }
        TaskMode::AssignTask(None) => task_context.0 = TaskMode::AssignTask(Some(snapped_pos)),
        TaskMode::DreamPlanting(None) => {
            area_edit_session.dream_planting_preview_seed = Some(rand::random::<u64>());
            task_context.0 = TaskMode::DreamPlanting(Some(snapped_center))
        }
        _ => {}
    }

    false
}
