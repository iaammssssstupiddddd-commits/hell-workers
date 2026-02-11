use bevy::prelude::*;

use crate::game_state::{
    BuildContext, CompanionPlacementState, PlayMode, TaskContext, ZoneContext,
};
use crate::interface::ui::components::MenuState;
use crate::systems::command::TaskMode;
use crate::systems::jobs::BuildingType;
use crate::systems::logistics::ZoneType;

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
    info!(
        "UI: Build mode set to {:?}, PlayMode -> BuildingPlace",
        kind
    );
}

pub(super) fn set_zone_mode(
    kind: ZoneType,
    next_play_mode: &mut NextState<PlayMode>,
    build_context: &mut BuildContext,
    zone_context: &mut ZoneContext,
    task_context: &mut TaskContext,
) {
    build_context.0 = None;
    zone_context.0 = Some(kind);
    task_context.0 = TaskMode::ZonePlacement(kind, None);
    next_play_mode.set(PlayMode::TaskDesignation);
    info!("UI: Zone mode set to {:?}, PlayMode -> TaskDesignation", kind);
}

pub(super) fn set_zone_removal_mode(
    kind: ZoneType,
    next_play_mode: &mut NextState<PlayMode>,
    build_context: &mut BuildContext,
    zone_context: &mut ZoneContext,
    task_context: &mut TaskContext,
) {
    build_context.0 = None;
    zone_context.0 = Some(kind); // 削除モードでも一応セットしておく
    task_context.0 = TaskMode::ZoneRemoval(kind, None);
    next_play_mode.set(PlayMode::TaskDesignation);
    info!(
        "UI: Zone Removal mode set to {:?}, PlayMode -> TaskDesignation",
        kind
    );
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
    info!(
        "UI: TaskMode set to {:?}, PlayMode -> TaskDesignation",
        mode
    );
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
    info!("UI: Area Edit mode entered (continuous), PlayMode -> TaskDesignation");
}

pub(super) fn build_mode_text(
    play_mode: &PlayMode,
    build_context: &BuildContext,
    companion_state: &CompanionPlacementState,
    zone_context: &ZoneContext,
    task_context: &TaskContext,
    selected_familiar_name: Option<&str>,
    selected_area_size_tiles: Option<UVec2>,
    area_edit_dragging: bool,
    area_edit_operation: Option<&str>,
    area_overlap: Option<(usize, f32)>,
    clipboard_has_area: bool,
    unassigned_tasks_in_area: Option<usize>,
) -> String {
    match play_mode {
        PlayMode::Normal => "Mode: Normal".to_string(),
        PlayMode::BuildingPlace => {
            if let Some(companion) = companion_state.0.as_ref() {
                format!(
                    "Mode: Companion ({:?} -> {:?})",
                    companion.parent_kind, companion.kind
                )
            } else if let Some(kind) = build_context.0 {
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
            TaskMode::AreaSelection(None) => {
                let target_name = selected_familiar_name.unwrap_or("No Familiar");
                let size = selected_area_size_tiles
                    .map(|v| format!("{}x{}t", v.x, v.y))
                    .unwrap_or_else(|| "?x?t".to_string());
                let state = if area_edit_dragging {
                    if let Some(op) = area_edit_operation {
                        format!("Dragging {}", op)
                    } else {
                        "Dragging".to_string()
                    }
                } else {
                    "Ready".to_string()
                };
                let overlap_text = if let Some((count, max_ratio)) = area_overlap {
                    if count > 0 {
                        format!("Overlap:{}({:.0}%)", count, max_ratio * 100.0)
                    } else {
                        "Overlap:0".to_string()
                    }
                } else {
                    "Overlap:-".to_string()
                };
                let clip = if clipboard_has_area {
                    "Clip:Ready"
                } else {
                    "Clip:Empty"
                };
                let tasks = unassigned_tasks_in_area
                    .map(|count| format!("Tasks:{}", count))
                    .unwrap_or_else(|| "Tasks:-".to_string());
                let overlap_warn =
                    area_overlap.is_some_and(|(count, max_ratio)| count > 0 && max_ratio >= 0.5);
                let warn = if overlap_warn {
                    " WARN:HighOverlap"
                } else {
                    ""
                };
                format!(
                    "Mode: Area Edit [{}] {} {} {} {} {}{} (Drag:Apply, Esc:Exit, Shift+Release:Exit, Ctrl+C/V, Ctrl+Z/Y, Ctrl+1..3 Save, Alt+1..3 Apply)",
                    target_name, size, state, overlap_text, tasks, clip, warn
                )
            }
            TaskMode::AreaSelection(Some(_)) => {
                let target_name = selected_familiar_name.unwrap_or("No Familiar");
                format!("Mode: Area Edit [{}] (New Area Dragging...)", target_name)
            }
            TaskMode::AssignTask(_) => "Mode: Assign Task".to_string(),
            TaskMode::ZonePlacement(kind, start_pos) => {
                if start_pos.is_some() {
                    format!("Mode: Zone {:?} (Dragging...)", kind)
                } else {
                    format!("Mode: Zone {:?} (Drag to place)", kind)
                }
            }
            TaskMode::ZoneRemoval(kind, start_pos) => {
                if start_pos.is_some() {
                    format!("Mode: Remove Zone {:?} (Dragging...)", kind)
                } else {
                    format!("Mode: Remove Zone {:?} (Drag to remove)", kind)
                }
            }
            _ => "Mode: Task".to_string(),
        },
    }
}
