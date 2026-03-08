use bevy::prelude::*;

use crate::systems::world::zones::AreaBounds;

pub mod area_selection;
pub mod assign_task;
pub mod indicators;
pub mod input;
pub mod visualization;
pub mod zone_placement;

/// タスクモード - どのタスクを指定中か
#[derive(Resource, Default, Debug, Clone, Copy, PartialEq)]
pub enum TaskMode {
    #[default]
    None, // 通常モード
    DesignateChop(Option<Vec2>),     // 伐採指示モード (ドラッグ開始位置)
    DesignateMine(Option<Vec2>),     // 採掘指示モード (ドラッグ開始位置)
    DesignateHaul(Option<Vec2>),     // 運搬指示モード (ドラッグ開始位置)
    CancelDesignation(Option<Vec2>), // 指示キャンセルモード (ドラッグ開始位置)
    SelectBuildTarget,               // 建築対象選択中
    AreaSelection(Option<Vec2>),     // エリア選択モード (始点)
    AssignTask(Option<Vec2>),        // 未アサインタスクを使い魔に割り当てるモード
    ZonePlacement(crate::systems::logistics::ZoneType, Option<Vec2>), // ゾーン（ストックパイル等）配置モード
    ZoneRemoval(crate::systems::logistics::ZoneType, Option<Vec2>),   // ゾーン解除モード
    FloorPlace(Option<Vec2>),    // 床エリア配置モード (ドラッグ開始位置)
    WallPlace(Option<Vec2>),     // 壁ライン配置モード (ドラッグ開始位置)
    DreamPlanting(Option<Vec2>), // Dream植林モード (ドラッグ開始位置)
}

/// タスクエリア - 使い魔が担当するエリア
#[derive(Component, Clone, Debug, PartialEq)]
pub struct TaskArea {
    pub bounds: AreaBounds,
}

impl TaskArea {
    pub fn from_points(a: Vec2, b: Vec2) -> Self {
        Self { bounds: AreaBounds::from_points(a, b) }
    }

    pub fn center(&self) -> Vec2 {
        self.bounds.center()
    }

    pub fn size(&self) -> Vec2 {
        self.bounds.size()
    }

    pub fn contains(&self, pos: Vec2) -> bool {
        self.bounds.contains(pos)
    }

    pub fn contains_with_margin(&self, pos: Vec2, margin: f32) -> bool {
        self.bounds.contains_with_margin(pos, margin)
    }

    pub fn contains_border(&self, pos: Vec2, thickness: f32) -> bool {
        let in_outer = self.bounds.contains_with_margin(pos, thickness);
        let inner = AreaBounds::new(
            self.bounds.min + Vec2::splat(thickness),
            self.bounds.max - Vec2::splat(thickness),
        );
        let in_inner = inner.contains(pos);
        in_outer && !in_inner
    }

    pub fn bounds(&self) -> AreaBounds {
        self.bounds.clone()
    }

    pub fn min(&self) -> Vec2 {
        self.bounds.min
    }

    pub fn max(&self) -> Vec2 {
        self.bounds.max
    }
}

impl From<&TaskArea> for AreaBounds {
    fn from(area: &TaskArea) -> Self {
        area.bounds.clone()
    }
}

impl From<AreaBounds> for TaskArea {
    fn from(bounds: AreaBounds) -> Self {
        TaskArea { bounds }
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AreaEditHandleKind {
    TopLeft,
    Top,
    TopRight,
    Right,
    BottomRight,
    Bottom,
    BottomLeft,
    Left,
    Center,
}

// 公開API
pub use area_selection::{
    AreaEditClipboard, AreaEditHistory, AreaEditPresets, AreaEditSession,
    area_selection_indicator_system, blueprint_cancel_cleanup_system, count_positions_in_area,
    dream_tree_planting_preview_system, overlap_summary_from_areas, task_area_edit_cursor_system,
    task_area_edit_history_shortcuts_system, task_area_selection_system,
};
pub use assign_task::assign_task_system;
pub use indicators::{
    area_edit_handles_visual_system, sync_designation_indicator_system, task_area_indicator_system,
    update_designation_indicator_system,
};
pub use input::familiar_command_input_system;
pub use visualization::{designation_visual_system, familiar_command_visual_system};
pub use zone_placement::{zone_placement_system, zone_removal_system, ZoneRemovalPreviewState};
