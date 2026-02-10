use bevy::prelude::*;

pub mod area_selection;
pub mod assign_task;
pub mod indicators;
pub mod input;
pub mod visualization;

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
}

/// タスクエリア - 使い魔が担当するエリア
#[derive(Component, Clone, Debug, PartialEq)]
pub struct TaskArea {
    pub min: Vec2,
    pub max: Vec2,
}

impl TaskArea {
    pub fn from_points(a: Vec2, b: Vec2) -> Self {
        Self {
            min: Vec2::new(a.x.min(b.x), a.y.min(b.y)),
            max: Vec2::new(a.x.max(b.x), a.y.max(b.y)),
        }
    }
    pub fn center(&self) -> Vec2 {
        (self.min + self.max) / 2.0
    }
    pub fn size(&self) -> Vec2 {
        (self.max - self.min).abs()
    }
    pub fn contains(&self, pos: Vec2) -> bool {
        self.contains_with_margin(pos, 0.0)
    }
    pub fn contains_with_margin(&self, pos: Vec2, margin: f32) -> bool {
        let m = margin.abs();
        pos.x >= self.min.x - m
            && pos.x <= self.max.x + m
            && pos.y >= self.min.y - m
            && pos.y <= self.max.y + m
    }
    pub fn contains_border(&self, pos: Vec2, thickness: f32) -> bool {
        let in_outer = pos.x >= self.min.x - thickness
            && pos.x <= self.max.x + thickness
            && pos.y >= self.min.y - thickness
            && pos.y <= self.max.y + thickness;
        let in_inner = pos.x >= self.min.x + thickness
            && pos.x <= self.max.x - thickness
            && pos.y >= self.min.y + thickness
            && pos.y <= self.max.y - thickness;
        in_outer && !in_inner
    }
}

/// タスクエリア表示用
#[derive(Component)]
pub struct TaskAreaIndicator(pub Entity); // 親の使い魔Entity

#[derive(Component)]
pub struct DesignationIndicator(pub Entity);

#[derive(Component)]
pub struct AreaSelectionIndicator;

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
    area_selection_indicator_system, count_positions_in_area, overlap_summary_from_areas,
    task_area_edit_cursor_system, task_area_edit_history_shortcuts_system,
    task_area_selection_system,
};
pub use assign_task::assign_task_system;
pub use indicators::{
    area_edit_handles_visual_system, sync_designation_indicator_system, task_area_indicator_system,
    update_designation_indicator_system,
};
pub use input::familiar_command_input_system;
pub use visualization::{designation_visual_system, familiar_command_visual_system};
