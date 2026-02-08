use super::mode;
use crate::constants::TILE_SIZE;
use crate::entities::familiar::Familiar;
use crate::game_state::{
    BuildContext, CompanionPlacementState, PlayMode, TaskContext, ZoneContext,
};
use crate::interface::selection::SelectedEntity;
use crate::interface::ui::components::*;
use crate::relationships::ManagedBy;
use crate::systems::command::{AreaEditClipboard, AreaEditSession, TaskArea};
use crate::systems::jobs::Designation;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use std::time::Duration;

fn overlap_summary_from_areas(
    selected_entity: Entity,
    selected_area: &TaskArea,
    areas: impl Iterator<Item = (Entity, TaskArea)>,
) -> Option<(usize, f32)> {
    let selected_size = selected_area.size();
    let selected_area_value = selected_size.x.abs() * selected_size.y.abs();
    if selected_area_value <= f32::EPSILON {
        return None;
    }

    let mut overlap_count = 0usize;
    let mut max_ratio = 0.0f32;

    for (entity, area) in areas {
        if entity == selected_entity {
            continue;
        }

        let overlap_w =
            (selected_area.max.x.min(area.max.x) - selected_area.min.x.max(area.min.x)).max(0.0);
        let overlap_h =
            (selected_area.max.y.min(area.max.y) - selected_area.min.y.max(area.min.y)).max(0.0);
        let overlap_area = overlap_w * overlap_h;
        if overlap_area <= f32::EPSILON {
            continue;
        }

        overlap_count += 1;
        let ratio = (overlap_area / selected_area_value).clamp(0.0, 1.0);
        if ratio > max_ratio {
            max_ratio = ratio;
        }
    }

    Some((overlap_count, max_ratio))
}

fn count_unassigned_tasks_in_area(
    selected_area: &TaskArea,
    task_positions: impl Iterator<Item = Vec2>,
) -> usize {
    let mut count = 0usize;
    for pos in task_positions {
        if pos.x >= selected_area.min.x - 0.1
            && pos.x <= selected_area.max.x + 0.1
            && pos.y >= selected_area.min.y - 0.1
            && pos.y <= selected_area.max.y + 0.1
        {
            count += 1;
        }
    }
    count
}

pub fn update_mode_text_system(
    play_mode: Res<State<PlayMode>>,
    build_context: Res<BuildContext>,
    companion_state: Res<CompanionPlacementState>,
    zone_context: Res<ZoneContext>,
    task_context: Res<TaskContext>,
    selected_entity: Res<SelectedEntity>,
    area_edit_session: Res<AreaEditSession>,
    area_edit_clipboard: Res<AreaEditClipboard>,
    ui_nodes: Res<UiNodeRegistry>,
    q_familiars: Query<&Familiar>,
    q_task_areas: Query<(Entity, Ref<TaskArea>), With<Familiar>>,
    q_unassigned_tasks: Query<&Transform, (With<Designation>, Without<ManagedBy>)>,
    mut q_text: Query<&mut Text>,
) {
    let area_mode_active = matches!(
        task_context.0,
        crate::systems::command::TaskMode::AreaSelection(_)
    );
    let selected_area_changed = selected_entity.0.is_some_and(|selected| {
        q_task_areas
            .iter()
            .find(|(entity, _)| *entity == selected)
            .is_some_and(|(_, area)| area.is_changed())
    });

    if !play_mode.is_changed()
        && !build_context.is_changed()
        && !companion_state.is_changed()
        && !zone_context.is_changed()
        && !task_context.is_changed()
        && !selected_entity.is_changed()
        && !area_edit_session.is_changed()
        && !area_edit_clipboard.is_changed()
        && !selected_area_changed
        && !area_mode_active
    {
        return;
    }
    let Some(entity) = ui_nodes.get_slot(UiSlot::ModeText) else {
        return;
    };
    if let Ok(mut text) = q_text.get_mut(entity) {
        let selected_familiar_name = selected_entity
            .0
            .and_then(|entity| q_familiars.get(entity).ok())
            .map(|familiar| familiar.name.as_str());
        let selected_area = selected_entity.0.and_then(|selected| {
            q_task_areas
                .iter()
                .find(|(entity, _)| *entity == selected)
                .map(|(_, area)| (*area).clone())
        });
        let selected_area_size_tiles = selected_area.as_ref().map(|area| {
            let size = area.size();
            UVec2::new(
                (size.x.abs() / TILE_SIZE).round().max(1.0) as u32,
                (size.y.abs() / TILE_SIZE).round().max(1.0) as u32,
            )
        });
        let area_overlap = selected_entity.0.and_then(|selected| {
            selected_area.as_ref().and_then(|selected_area| {
                overlap_summary_from_areas(
                    selected,
                    selected_area,
                    q_task_areas
                        .iter()
                        .map(|(entity, area)| (entity, (*area).clone())),
                )
            })
        });
        let unassigned_tasks_in_area = selected_area.as_ref().map(|area| {
            count_unassigned_tasks_in_area(
                area,
                q_unassigned_tasks
                    .iter()
                    .map(|transform| transform.translation.truncate()),
            )
        });

        text.0 = mode::build_mode_text(
            play_mode.get(),
            &build_context,
            &companion_state,
            &zone_context,
            &task_context,
            selected_familiar_name,
            selected_area_size_tiles,
            area_edit_session.is_dragging(),
            area_edit_session.operation_label(),
            area_overlap,
            area_edit_clipboard.has_area(),
            unassigned_tasks_in_area,
        );
    }
}

pub fn task_summary_ui_system(
    q_designations: Query<&crate::systems::jobs::Priority, With<crate::systems::jobs::Designation>>,
    ui_nodes: Res<UiNodeRegistry>,
    mut q_text: Query<&mut Text>,
) {
    let Some(entity) = ui_nodes.get_slot(UiSlot::TaskSummaryText) else {
        return;
    };
    if let Ok(mut text) = q_text.get_mut(entity) {
        let total = q_designations.iter().count();
        let high = q_designations.iter().filter(|p| p.0 > 0).count();
        text.0 = format!("Tasks: {} ({} High)", total, high);
    }
}

pub fn update_area_edit_preview_ui_system(
    task_context: Res<TaskContext>,
    selected_entity: Res<SelectedEntity>,
    area_edit_session: Res<AreaEditSession>,
    area_edit_clipboard: Res<AreaEditClipboard>,
    ui_nodes: Res<UiNodeRegistry>,
    q_task_areas: Query<(Entity, &TaskArea), With<Familiar>>,
    q_unassigned_tasks: Query<&Transform, (With<Designation>, Without<ManagedBy>)>,
    q_window: Query<&Window, With<PrimaryWindow>>,
    mut q_node: Query<&mut Node>,
    mut q_text: Query<&mut Text>,
) {
    let Some(preview_entity) = ui_nodes.get_slot(UiSlot::AreaEditPreview) else {
        return;
    };
    let Ok(mut node) = q_node.get_mut(preview_entity) else {
        return;
    };
    let Ok(mut text) = q_text.get_mut(preview_entity) else {
        return;
    };

    if !matches!(
        task_context.0,
        crate::systems::command::TaskMode::AreaSelection(_)
    ) {
        node.display = Display::None;
        return;
    }

    let Some(selected) = selected_entity.0 else {
        node.display = Display::None;
        return;
    };
    let Some(area) = q_task_areas
        .iter()
        .find(|(entity, _)| *entity == selected)
        .map(|(_, area)| area)
    else {
        node.display = Display::None;
        return;
    };
    let Ok(window) = q_window.single() else {
        node.display = Display::None;
        return;
    };
    let Some(cursor) = window.cursor_position() else {
        node.display = Display::None;
        return;
    };

    let size = area.size();
    let width_tiles = (size.x.abs() / TILE_SIZE).round().max(1.0) as i32;
    let height_tiles = (size.y.abs() / TILE_SIZE).round().max(1.0) as i32;

    let state = if area_edit_session.is_dragging() {
        if let Some(op) = area_edit_session.operation_label() {
            format!("Dragging {}", op)
        } else {
            "Dragging".to_string()
        }
    } else {
        "Ready".to_string()
    };

    let overlap = overlap_summary_from_areas(
        selected,
        area,
        q_task_areas
            .iter()
            .map(|(entity, area)| (entity, area.clone())),
    );
    let overlap_text = if let Some((count, ratio)) = overlap {
        if count > 0 {
            format!("Overlap:{} ({:.0}%)", count, ratio * 100.0)
        } else {
            "Overlap:0".to_string()
        }
    } else {
        "Overlap:-".to_string()
    };
    let clip_text = if area_edit_clipboard.has_area() {
        "Clip:Ready"
    } else {
        "Clip:Empty"
    };
    let tasks_in_area = count_unassigned_tasks_in_area(
        area,
        q_unassigned_tasks
            .iter()
            .map(|transform| transform.translation.truncate()),
    );
    let warn_text = if overlap.is_some_and(|(count, ratio)| count > 0 && ratio >= 0.5) {
        " | WARN:HighOverlap"
    } else {
        ""
    };

    text.0 = format!(
        "Area {}x{}t | {} | {} | Tasks:{} | {}{}",
        width_tiles, height_tiles, state, overlap_text, tasks_in_area, clip_text, warn_text
    );
    node.display = Display::Flex;
    node.left = Val::Px((cursor.x + 14.0).min(window.width() - 360.0).max(4.0));
    node.top = Val::Px((cursor.y + 18.0).min(window.height() - 34.0).max(4.0));
}

#[derive(Default)]
pub struct FpsCounter {
    pub frame_count: u32,
    pub elapsed_time: Duration,
}

pub fn update_fps_display_system(
    time: Res<Time>,
    mut fps_counter: Local<FpsCounter>,
    ui_nodes: Res<UiNodeRegistry>,
    mut q_text: Query<&mut Text>,
) {
    fps_counter.elapsed_time += time.delta();
    fps_counter.frame_count += 1;

    if fps_counter.elapsed_time >= Duration::from_secs(1) {
        let Some(entity) = ui_nodes.get_slot(UiSlot::FpsText) else {
            return;
        };
        if let Ok(mut text) = q_text.get_mut(entity) {
            let fps = fps_counter.frame_count as f32 / fps_counter.elapsed_time.as_secs_f32();
            text.0 = format!("FPS: {:.0}", fps);
            fps_counter.frame_count = 0;
            fps_counter.elapsed_time = Duration::ZERO;
        }
    }
}
