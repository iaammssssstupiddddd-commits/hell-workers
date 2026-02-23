use super::apply::{
    apply_area_and_record_history, apply_designation_in_area, assign_unassigned_tasks_in_area,
};
use super::cancel::cancel_single_designation;
use super::geometry::{apply_area_edit_drag, detect_area_edit_operation, world_cursor_pos};
use super::queries::DesignationTargetQuery;
use super::state::{AreaEditHistory, AreaEditSession, Drag};
use crate::constants::TILE_SIZE;
use crate::entities::damned_soul::Destination;
use crate::entities::familiar::{ActiveCommand, Familiar, FamiliarCommand};
use crate::game_state::{PlayMode, TaskContext};
use crate::interface::camera::MainCamera;
use crate::interface::selection::SelectedEntity;
use crate::interface::ui::UiInputState;
use crate::systems::command::{AreaSelectionIndicator, TaskArea, TaskMode};
use crate::systems::jobs::Designation;
use crate::systems::jobs::floor_construction::{
    FloorConstructionCancelRequested, FloorTileBlueprint,
};
use crate::systems::jobs::wall_construction::{WallConstructionCancelRequested, WallTileBlueprint};
use crate::world::map::WorldMap;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use std::collections::HashSet;

mod release;

use release::handle_left_just_released_input;

fn should_exit_after_apply(keyboard: &ButtonInput<KeyCode>) -> bool {
    keyboard.pressed(KeyCode::ShiftLeft) || keyboard.pressed(KeyCode::ShiftRight)
}

fn reset_designation_mode(mode: TaskMode) -> TaskMode {
    match mode {
        TaskMode::DesignateChop(_) => TaskMode::DesignateChop(None),
        TaskMode::DesignateMine(_) => TaskMode::DesignateMine(None),
        TaskMode::DesignateHaul(_) => TaskMode::DesignateHaul(None),
        TaskMode::CancelDesignation(_) => TaskMode::CancelDesignation(None),
        _ => TaskMode::None,
    }
}

fn try_start_direct_edit_drag(
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

    area_edit_session.active_drag = Some(Drag {
        familiar_entity: fam_entity,
        operation,
        original_area: existing_area.clone(),
        drag_start: snapped_pos,
    });

    true
}

fn despawn_selection_indicators(
    q_selection_indicator: &Query<Entity, With<AreaSelectionIndicator>>,
    commands: &mut Commands,
) {
    for indicator_entity in q_selection_indicator.iter() {
        commands.entity(indicator_entity).try_despawn();
    }
}

fn handle_active_drag_input(
    buttons: &ButtonInput<MouseButton>,
    q_window: &Query<&Window, With<PrimaryWindow>>,
    q_camera: &Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    keyboard: &ButtonInput<KeyCode>,
    task_context: &mut TaskContext,
    next_play_mode: &mut NextState<PlayMode>,
    q_familiars: &mut Query<(&mut ActiveCommand, &mut Destination), With<Familiar>>,
    q_unassigned: &Query<
        (Entity, &Transform, &Designation),
        Without<crate::relationships::ManagedBy>,
    >,
    commands: &mut Commands,
    area_edit_session: &mut AreaEditSession,
    area_edit_history: &mut AreaEditHistory,
) -> bool {
    let Some(active_drag) = area_edit_session.active_drag.clone() else {
        return false;
    };

    if buttons.pressed(MouseButton::Left)
        && let Some(world_pos) = world_cursor_pos(q_window, q_camera)
    {
        let snapped_pos = WorldMap::snap_to_grid_edge(world_pos);
        let updated_area = apply_area_edit_drag(&active_drag, snapped_pos);

        commands
            .entity(active_drag.familiar_entity)
            .insert(updated_area.clone());
        if let Ok((mut active_command, mut familiar_dest)) =
            q_familiars.get_mut(active_drag.familiar_entity)
        {
            familiar_dest.0 = updated_area.center();
            active_command.command = FamiliarCommand::Patrol;
        }
    }

    if buttons.just_released(MouseButton::Left) {
        let applied_area = world_cursor_pos(q_window, q_camera)
            .map(WorldMap::snap_to_grid_edge)
            .map(|snapped| apply_area_edit_drag(&active_drag, snapped))
            .unwrap_or_else(|| active_drag.original_area.clone());

        if applied_area != active_drag.original_area {
            apply_area_and_record_history(
                active_drag.familiar_entity,
                &applied_area,
                Some(active_drag.original_area.clone()),
                commands,
                q_familiars,
                area_edit_history,
            );

            assign_unassigned_tasks_in_area(
                commands,
                active_drag.familiar_entity,
                &applied_area,
                q_unassigned,
            );
        }

        area_edit_session.active_drag = None;
        if should_exit_after_apply(keyboard) {
            task_context.0 = TaskMode::None;
            next_play_mode.set(PlayMode::Normal);
        } else {
            task_context.0 = TaskMode::AreaSelection(None);
        }
        return true;
    }

    if buttons.pressed(MouseButton::Left) {
        return true;
    }

    area_edit_session.active_drag = None;
    false
}

fn handle_left_just_pressed_input(
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

pub fn task_area_selection_system(
    buttons: Res<ButtonInput<MouseButton>>,
    q_window: Query<&Window, With<PrimaryWindow>>,
    q_camera: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    ui_input_state: Res<UiInputState>,
    selected: Res<SelectedEntity>,
    mut task_context: ResMut<TaskContext>,
    mut next_play_mode: ResMut<NextState<PlayMode>>,
    mut q_familiars: Query<(&mut ActiveCommand, &mut Destination), With<Familiar>>,
    q_familiar_areas: Query<&TaskArea, With<Familiar>>,
    mut q_target_sets: ParamSet<(
        DesignationTargetQuery<'_, '_>,
        Query<(Entity, &Transform, &FloorTileBlueprint)>,
        Query<(Entity, &Transform, &WallTileBlueprint)>,
    )>,
    mut commands: Commands,
    keyboard: Res<ButtonInput<KeyCode>>,
    q_unassigned: Query<
        (Entity, &Transform, &Designation),
        Without<crate::relationships::ManagedBy>,
    >,
    q_selection_indicator: Query<Entity, With<AreaSelectionIndicator>>,
    mut area_edit_session: ResMut<AreaEditSession>,
    mut area_edit_history: ResMut<AreaEditHistory>,
) {
    if !matches!(task_context.0, TaskMode::DreamPlanting(_)) {
        area_edit_session.dream_planting_preview_seed = None;
    }

    if ui_input_state.pointer_over_ui {
        return;
    }

    if task_context.0 == TaskMode::None {
        area_edit_session.active_drag = None;
        return;
    }

    if handle_active_drag_input(
        &buttons,
        &q_window,
        &q_camera,
        &keyboard,
        &mut task_context,
        &mut next_play_mode,
        &mut q_familiars,
        &q_unassigned,
        &mut commands,
        &mut area_edit_session,
        &mut area_edit_history,
    ) {
        return;
    }

    if buttons.just_pressed(MouseButton::Left)
        && handle_left_just_pressed_input(
            &mut task_context,
            selected.0,
            &q_familiar_areas,
            &q_window,
            &q_camera,
            &mut area_edit_session,
        )
    {
        return;
    }

    if !buttons.just_released(MouseButton::Left) {
        return;
    }

    let Some(world_pos) = world_cursor_pos(&q_window, &q_camera) else {
        return;
    };

    handle_left_just_released_input(
        &mut task_context,
        selected.0,
        world_pos,
        &q_familiar_areas,
        &mut q_familiars,
        &mut q_target_sets,
        &q_unassigned,
        &q_selection_indicator,
        &keyboard,
        &mut next_play_mode,
        &mut commands,
        &mut area_edit_session,
        &mut area_edit_history,
    );
}
