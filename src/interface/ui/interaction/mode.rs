use bevy::prelude::*;

use crate::game_state::{BuildContext, PlayMode, TaskContext, ZoneContext};
use crate::interface::ui::components::MenuState;
use crate::systems::command::TaskMode;
use crate::systems::logistics::ZoneType;
use crate::systems::jobs::BuildingType;

pub(super) fn toggle_menu_and_reset_mode(
    menu_state: &mut MenuState,
    target: MenuState,
    next_play_mode: &mut NextState<PlayMode>,
    build_context: &mut BuildContext,
    zone_context: &mut ZoneContext,
    task_context: &mut TaskContext,
    clear_task_context: bool,
) {
    let current = *menu_state;
    *menu_state = if std::mem::discriminant(&current) == std::mem::discriminant(&target) {
        MenuState::Hidden
    } else {
        target
    };

    build_context.0 = None;
    zone_context.0 = None;
    if clear_task_context {
        task_context.0 = TaskMode::None;
    }
    next_play_mode.set(PlayMode::Normal);
}

pub(super) fn set_build_mode(
    kind: BuildingType,
    next_play_mode: &mut NextState<PlayMode>,
    build_context: &mut BuildContext,
    zone_context: &mut ZoneContext,
    task_context: &mut TaskContext,
) {
    zone_context.0 = None;
    task_context.0 = TaskMode::None;
    build_context.0 = Some(kind);
    next_play_mode.set(PlayMode::BuildingPlace);
    info!("UI: Build mode set to {:?}, PlayMode -> BuildingPlace", kind);
}

pub(super) fn set_zone_mode(
    kind: ZoneType,
    next_play_mode: &mut NextState<PlayMode>,
    build_context: &mut BuildContext,
    zone_context: &mut ZoneContext,
    task_context: &mut TaskContext,
) {
    build_context.0 = None;
    task_context.0 = TaskMode::None;
    zone_context.0 = Some(kind);
    next_play_mode.set(PlayMode::ZonePlace);
    info!("UI: Zone mode set to {:?}, PlayMode -> ZonePlace", kind);
}

pub(super) fn set_task_mode(
    mode: TaskMode,
    next_play_mode: &mut NextState<PlayMode>,
    build_context: &mut BuildContext,
    zone_context: &mut ZoneContext,
    task_context: &mut TaskContext,
) {
    build_context.0 = None;
    zone_context.0 = None;
    task_context.0 = mode;
    next_play_mode.set(PlayMode::TaskDesignation);
    info!("UI: TaskMode set to {:?}, PlayMode -> TaskDesignation", mode);
}

pub(super) fn set_area_task_mode(
    next_play_mode: &mut NextState<PlayMode>,
    build_context: &mut BuildContext,
    zone_context: &mut ZoneContext,
    task_context: &mut TaskContext,
) {
    let mode = TaskMode::AreaSelection(None);
    build_context.0 = None;
    zone_context.0 = None;
    task_context.0 = mode;
    next_play_mode.set(PlayMode::TaskDesignation);
    info!("UI: Area Selection Mode entered, PlayMode -> TaskDesignation");
}

pub(super) fn build_mode_text(
    play_mode: &PlayMode,
    build_context: &BuildContext,
    zone_context: &ZoneContext,
    task_context: &TaskContext,
) -> String {
    match play_mode {
        PlayMode::Normal => "Mode: Normal".to_string(),
        PlayMode::BuildingPlace => {
            if let Some(kind) = build_context.0 {
                format!("Mode: Build ({:?})", kind)
            } else {
                "Mode: Build".to_string()
            }
        }
        PlayMode::ZonePlace => {
            if let Some(kind) = zone_context.0 {
                format!("Mode: Zone ({:?})", kind)
            } else {
                "Mode: Zone".to_string()
            }
        }
        PlayMode::TaskDesignation => match task_context.0 {
            TaskMode::DesignateChop(None) => "Mode: Chop (Drag to select)".to_string(),
            TaskMode::DesignateChop(Some(_)) => "Mode: Chop (Dragging...)".to_string(),
            TaskMode::DesignateMine(None) => "Mode: Mine (Drag to select)".to_string(),
            TaskMode::DesignateMine(Some(_)) => "Mode: Mine (Dragging...)".to_string(),
            TaskMode::DesignateHaul(None) => "Mode: Haul (Drag to select)".to_string(),
            TaskMode::DesignateHaul(Some(_)) => "Mode: Haul (Dragging...)".to_string(),
            TaskMode::CancelDesignation(None) => "Mode: Cancel (Drag to select)".to_string(),
            TaskMode::CancelDesignation(Some(_)) => "Mode: Cancel (Dragging...)".to_string(),
            TaskMode::AreaSelection(_) => "Mode: Area Selection".to_string(),
            TaskMode::AssignTask(_) => "Mode: Assign Task".to_string(),
            _ => "Mode: Task".to_string(),
        },
    }
}
