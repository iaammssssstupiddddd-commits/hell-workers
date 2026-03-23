use bevy::prelude::*;

use crate::systems::logistics::ZoneType;

pub mod area_selection;
pub mod assign_task;
pub mod indicators;
pub mod input;
pub mod visualization;
pub mod zone_placement;

pub use hw_core::area::TaskArea;
pub use hw_core::game_state::{TaskMode, TaskModeZoneType};

pub fn to_task_mode_zone_type(zone_type: ZoneType) -> TaskModeZoneType {
    match zone_type {
        ZoneType::Stockpile => TaskModeZoneType::Stockpile,
        ZoneType::Yard => TaskModeZoneType::Yard,
    }
}

/// タスクエリア表示用
#[derive(Component)]
pub struct TaskAreaIndicator(pub Entity); // 親の使い魔Entity

#[derive(Component)]
pub struct DesignationIndicator(pub Entity);

#[derive(Component)]
pub struct AreaSelectionIndicator;

#[derive(Component)]
pub struct DreamTreePreviewIndicator;

#[derive(Component, Clone, Copy, Debug)]
pub struct AreaEditHandleVisual {
    pub owner: Entity,
    pub kind: AreaEditHandleKind,
}

pub use hw_ui::area_edit::AreaEditHandleKind;

// ---------------------------------------------------------------------------
// crate 所有 helper の re-export（ECS 非依存 pure function）
// ---------------------------------------------------------------------------

/// ドメイン helper: `hw_core::area` 由来の pure helper
pub use hw_core::area::{
    area_from_center_and_size, count_positions_in_area, get_drag_start, overlap_summary_from_areas,
    wall_line_area,
};

// ---------------------------------------------------------------------------
// shell system / ECS apply / visual（root 残留）の re-export
// ---------------------------------------------------------------------------

/// エリア選択: Resource 型・System
pub use area_selection::{
    AreaEditClipboard, AreaEditHistory, AreaEditPresets, AreaEditSession,
    area_selection_indicator_system, blueprint_cancel_cleanup_system,
    dream_tree_planting_preview_system, task_area_edit_cursor_system,
    task_area_edit_history_shortcuts_system, task_area_selection_system,
};
/// タスク割り当て: ECS apply
pub use assign_task::assign_task_system;
/// インジケーター: visual sync
pub use indicators::{
    area_edit_handles_visual_system, sync_designation_indicator_system, task_area_indicator_system,
    update_designation_indicator_system,
};
/// 入力: Familiar コマンド入力 orchestration
pub use input::familiar_command_input_system;
/// 視覚フィードバック: designation / command visual
pub use visualization::{designation_visual_system, familiar_command_visual_system};
/// ゾーン操作: ECS apply（バリデーション helper は `hw_world::zone_ops` 所有）
pub use zone_placement::{ZoneRemovalPreviewState, zone_placement_system, zone_removal_system};
