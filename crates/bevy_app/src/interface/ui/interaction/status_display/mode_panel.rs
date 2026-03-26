//! モード表示 / エリア編集プレビュー / タスクサマリの中継レイヤー（hw_ui 側実装へ委譲）

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use hw_core::constants::TILE_SIZE;
use hw_core::game_state::PlayMode;

use crate::app_contexts::{BuildContext, CompanionPlacementState, TaskContext, ZoneContext};
use crate::interface::selection::SelectedEntity;
use crate::interface::ui::interaction::mode;
use crate::interface::ui::panels::task_list::{TaskListDirty, TaskListState};
use crate::systems::command::{
    AreaEditClipboard, AreaEditSession, TaskArea, TaskMode, count_positions_in_area,
    overlap_summary_from_areas,
};
use crate::systems::jobs::Designation;
use hw_core::relationships::ManagedBy;
use hw_ui::components::UiNodeRegistry;

#[derive(SystemParam)]
pub struct ModeState<'w> {
    play_mode: Res<'w, State<PlayMode>>,
    build_context: Res<'w, BuildContext>,
    companion_state: Res<'w, CompanionPlacementState>,
    zone_context: Res<'w, ZoneContext>,
    task_context: Res<'w, TaskContext>,
}

#[derive(SystemParam)]
pub struct ModeSelectionData<'w, 's> {
    selected_entity: Res<'w, SelectedEntity>,
    area_edit_session: Res<'w, AreaEditSession>,
    area_edit_clipboard: Res<'w, AreaEditClipboard>,
    q_familiars: Query<'w, 's, &'static hw_core::familiar::Familiar>,
    q_task_areas:
        Query<'w, 's, (Entity, Ref<'static, TaskArea>), With<hw_core::familiar::Familiar>>,
    q_unassigned_tasks: Query<'w, 's, &'static Transform, (With<Designation>, Without<ManagedBy>)>,
}

pub fn update_mode_text_system(
    mode_state: ModeState,
    selection_data: ModeSelectionData,
    q_text: Query<&mut Text>,
    ui_nodes: Res<UiNodeRegistry>,
) {
    let ModeState {
        play_mode,
        build_context,
        companion_state,
        zone_context,
        task_context,
    } = mode_state;
    let ModeSelectionData {
        selected_entity,
        area_edit_session,
        area_edit_clipboard,
        q_familiars,
        q_task_areas,
        q_unassigned_tasks,
    } = selection_data;
    let area_mode_active = matches!(task_context.0, TaskMode::AreaSelection(_));
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
        mode::ModeCtxRefs {
            play_mode: play_mode.get(),
            build_context: &build_context,
            companion_state: &companion_state,
            zone_context: &zone_context,
            task_context: &task_context,
        },
        mode::ModeDisplayInfo {
            selected_familiar_name,
            selected_area_size_tiles,
            area_edit_dragging: area_edit_session.is_dragging(),
            area_edit_operation: area_edit_session.operation_label(),
            area_overlap,
            clipboard_has_area: area_edit_clipboard.has_area(),
            unassigned_tasks_in_area,
        },
    );

    hw_ui::interaction::status_display::update_mode_text_system(
        Some(hw_ui::interaction::status_display::ModeTextPayload { text: mode_text }),
        ui_nodes,
        q_text,
    );
}

pub fn task_summary_ui_system(
    mut dirty: Option<ResMut<TaskListDirty>>,
    state: Option<Res<TaskListState>>,
    theme: Res<hw_ui::theme::UiTheme>,
    ui_nodes: Res<UiNodeRegistry>,
    q_text: Query<(&mut Text, &mut TextColor)>,
) {
    let Some(dirty) = dirty.as_mut() else {
        return;
    };
    let Some(state) = state.as_ref() else {
        return;
    };

    if !theme.is_changed() && !dirty.summary_dirty() {
        return;
    }

    if dirty.summary_dirty() {
        dirty.clear_summary();
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

#[derive(SystemParam)]
pub struct AreaEditContext<'w> {
    task_context: Res<'w, TaskContext>,
    selected_entity: Res<'w, SelectedEntity>,
    area_edit_session: Res<'w, AreaEditSession>,
    area_edit_clipboard: Res<'w, AreaEditClipboard>,
}

#[derive(SystemParam)]
pub struct AreaEditQueries<'w, 's> {
    q_task_areas: Query<'w, 's, (Entity, &'static TaskArea), With<hw_core::familiar::Familiar>>,
    q_unassigned_tasks: Query<'w, 's, &'static Transform, (With<Designation>, Without<ManagedBy>)>,
    q_window: Query<'w, 's, &'static Window, With<PrimaryWindow>>,
}

pub fn update_area_edit_preview_ui_system(
    edit_context: AreaEditContext,
    edit_queries: AreaEditQueries,
    ui_nodes: Res<UiNodeRegistry>,
    q_node: Query<&mut Node>,
    q_text: Query<&mut Text>,
) {
    let AreaEditContext {
        task_context,
        selected_entity,
        area_edit_session,
        area_edit_clipboard,
    } = edit_context;
    let AreaEditQueries {
        q_task_areas,
        q_unassigned_tasks,
        q_window,
    } = edit_queries;
    let mut payload = Some(hw_ui::interaction::status_display::AreaEditPreviewPayload {
        display: false,
        text: String::new(),
        left: 0.0,
        top: 0.0,
    });

    if !matches!(task_context.0, TaskMode::AreaSelection(_)) {
        hw_ui::interaction::status_display::update_area_edit_preview_ui_system(
            payload, ui_nodes, q_node, q_text,
        );
        return;
    }

    let Some(selected) = selected_entity.0 else {
        hw_ui::interaction::status_display::update_area_edit_preview_ui_system(
            payload, ui_nodes, q_node, q_text,
        );
        return;
    };
    let Some(area) = q_task_areas
        .iter()
        .find(|(entity, _)| *entity == selected)
        .map(|(_, area)| area)
    else {
        hw_ui::interaction::status_display::update_area_edit_preview_ui_system(
            payload, ui_nodes, q_node, q_text,
        );
        return;
    };
    let Ok(window) = q_window.single() else {
        hw_ui::interaction::status_display::update_area_edit_preview_ui_system(
            payload, ui_nodes, q_node, q_text,
        );
        return;
    };
    let Some(cursor) = window.cursor_position() else {
        hw_ui::interaction::status_display::update_area_edit_preview_ui_system(
            payload, ui_nodes, q_node, q_text,
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
        q_task_areas
            .iter()
            .map(|(entity, area)| (entity, (*area).clone())),
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
        q_unassigned_tasks
            .iter()
            .map(|transform| transform.translation.truncate()),
    );
    let warn_text = if overlap.is_some_and(|(count, ratio)| count > 0 && ratio >= 0.5) {
        " | WARN:HighOverlap"
    } else {
        ""
    };

    payload = Some(hw_ui::interaction::status_display::AreaEditPreviewPayload {
        display: true,
        text: format!(
            "Area {}x{}t | {} | {} | Tasks:{} | {}{}",
            width_tiles, height_tiles, state, overlap_text, tasks_in_area, clip_text, warn_text,
        ),
        left: (cursor.x + 14.0).min(window.width() - 360.0).max(4.0),
        top: (cursor.y + 18.0).min(window.height() - 34.0).max(4.0),
    });

    hw_ui::interaction::status_display::update_area_edit_preview_ui_system(
        payload, ui_nodes, q_node, q_text,
    );
}
