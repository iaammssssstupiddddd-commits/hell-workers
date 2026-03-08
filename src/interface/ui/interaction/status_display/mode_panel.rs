//! モード表示 / エリア編集プレビュー / タスクサマリの中継レイヤー（hw_ui 側実装へ委譲）

use hw_core::constants::TILE_SIZE;
use hw_core::game_state::{PlayMode};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::app_contexts::{BuildContext, CompanionPlacementState, TaskContext, ZoneContext};
use crate::interface::selection::SelectedEntity;
use crate::interface::ui::components::UiNodeRegistry;
use crate::interface::ui::interaction::mode;
use crate::interface::ui::panels::task_list::{TaskListDirty, TaskListState};
use crate::relationships::ManagedBy;
use crate::systems::command::{
    count_positions_in_area, overlap_summary_from_areas, AreaEditClipboard, AreaEditSession, TaskArea,
    TaskMode,
};
use crate::systems::jobs::{Designation, Priority};

pub fn update_mode_text_system(
    play_mode: Res<State<PlayMode>>,
    build_context: Res<BuildContext>,
    companion_state: Res<CompanionPlacementState>,
    zone_context: Res<ZoneContext>,
    task_context: Res<TaskContext>,
    selected_entity: Res<SelectedEntity>,
    area_edit_session: Res<AreaEditSession>,
    area_edit_clipboard: Res<AreaEditClipboard>,
    q_familiars: Query<&hw_core::familiar::Familiar>,
    q_task_areas: Query<(Entity, Ref<TaskArea>), With<hw_core::familiar::Familiar>>,
    q_unassigned_tasks: Query<&Transform, (With<Designation>, Without<ManagedBy>)>,
    q_text: Query<&mut Text>,
    ui_nodes: Res<UiNodeRegistry>,
) {
    let area_mode_active = matches!(
        task_context.0,
        TaskMode::AreaSelection(_)
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
        count_positions_in_area(
            area,
            q_unassigned_tasks
                .iter()
                .map(|transform| transform.translation.truncate()),
        )
    });

    let mode_text = mode::build_mode_text(
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

    hw_ui::interaction::status_display::update_mode_text_system(
        Some(hw_ui::interaction::status_display::ModeTextPayload { text: mode_text }),
        ui_nodes,
        q_text,
    );
}

pub fn task_summary_ui_system(
    mut dirty: Option<ResMut<TaskListDirty>>,
    mut state: Option<ResMut<TaskListState>>,
    q_designations: Query<(
        Entity,
        &Transform,
        &Designation,
        Option<&Priority>,
        Option<&crate::relationships::TaskWorkers>,
        Option<&crate::systems::jobs::Blueprint>,
        Option<&crate::systems::logistics::transport_request::TransportRequest>,
        Option<&crate::systems::logistics::ResourceItem>,
        Option<&crate::systems::jobs::Tree>,
        Option<&crate::systems::jobs::Rock>,
        Option<&crate::systems::jobs::SandPile>,
        Option<&crate::systems::jobs::BonePile>,
    )>,
    theme: Res<crate::interface::ui::theme::UiTheme>,
    ui_nodes: Res<UiNodeRegistry>,
    q_text: Query<(&mut Text, &mut TextColor)>,
) {
    let Some(dirty) = dirty.as_mut() else {
        return;
    };
    let Some(state) = state.as_mut() else {
        return;
    };

    if dirty.summary_dirty() {
        let (total, high) = crate::interface::ui::panels::task_list::build_task_summary(&q_designations);
        state.summary_total = total;
        state.summary_high = high;
        dirty.clear_summary();
    }

    if !theme.is_changed() && !state.is_changed() {
        return;
    }

    hw_ui::interaction::status_display::task_summary_ui_system(
        Some(hw_ui::interaction::status_display::TaskSummaryPayload {
            total: state.summary_total as u32,
            high: state.summary_high as u32,
        }),
        theme.colors.task_high_warning,
        theme.colors.panel_accent_time_control,
        ui_nodes,
        q_text,
    );
}

pub fn update_area_edit_preview_ui_system(
    task_context: Res<TaskContext>,
    selected_entity: Res<SelectedEntity>,
    area_edit_session: Res<AreaEditSession>,
    area_edit_clipboard: Res<AreaEditClipboard>,
    ui_nodes: Res<UiNodeRegistry>,
    q_task_areas: Query<(Entity, &TaskArea), With<hw_core::familiar::Familiar>>,
    q_unassigned_tasks: Query<&Transform, (With<Designation>, Without<ManagedBy>)>,
    q_window: Query<&Window, With<PrimaryWindow>>,
    q_node: Query<&mut Node>,
    q_text: Query<&mut Text>,
) {
    let mut payload = Some(
            hw_ui::interaction::status_display::AreaEditPreviewPayload {
            display: false,
            text: String::new(),
            left: 0.0,
            top: 0.0,
        },
    );

    if !matches!(task_context.0, TaskMode::AreaSelection(_)) {
        hw_ui::interaction::status_display::update_area_edit_preview_ui_system(
            payload,
            ui_nodes,
            q_node,
            q_text,
        );
        return;
    }

    let Some(selected) = selected_entity.0 else {
        hw_ui::interaction::status_display::update_area_edit_preview_ui_system(
            payload,
            ui_nodes,
            q_node,
            q_text,
        );
        return;
    };
    let Some(area) = q_task_areas
        .iter()
        .find(|(entity, _)| *entity == selected)
        .map(|(_, area)| area)
    else {
        hw_ui::interaction::status_display::update_area_edit_preview_ui_system(
            payload,
            ui_nodes,
            q_node,
            q_text,
        );
        return;
    };
    let Ok(window) = q_window.single() else {
        hw_ui::interaction::status_display::update_area_edit_preview_ui_system(
            payload,
            ui_nodes,
            q_node,
            q_text,
        );
        return;
    };
    let Some(cursor) = window.cursor_position() else {
        hw_ui::interaction::status_display::update_area_edit_preview_ui_system(
            payload,
            ui_nodes,
            q_node,
            q_text,
        );
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
        q_task_areas.iter().map(|(entity, area)| (entity, (*area).clone())),
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
    let tasks_in_area = count_positions_in_area(
        area,
        q_unassigned_tasks.iter().map(|transform| transform.translation.truncate()),
    );
    let warn_text = if overlap.is_some_and(|(count, ratio)| count > 0 && ratio >= 0.5) {
        " | WARN:HighOverlap"
    } else {
        ""
    };

    payload = Some(
        hw_ui::interaction::status_display::AreaEditPreviewPayload {
            display: true,
            text: format!(
                "Area {}x{}t | {} | {} | Tasks:{} | {}{}",
                width_tiles,
                height_tiles,
                state,
                overlap_text,
                tasks_in_area,
                clip_text,
                warn_text,
            ),
            left: (cursor.x + 14.0).min(window.width() - 360.0).max(4.0),
            top: (cursor.y + 18.0).min(window.height() - 34.0).max(4.0),
        },
    );

    hw_ui::interaction::status_display::update_area_edit_preview_ui_system(
        payload,
        ui_nodes,
        q_node,
        q_text,
    );
}
