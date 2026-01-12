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
#[derive(Component, Clone, Debug)]
pub struct TaskArea {
    pub min: Vec2,
    pub max: Vec2,
}

impl TaskArea {
    pub fn center(&self) -> Vec2 {
        (self.min + self.max) / 2.0
    }
    pub fn size(&self) -> Vec2 {
        (self.max - self.min).abs()
    }
    pub fn contains(&self, pos: Vec2) -> bool {
        pos.x >= self.min.x && pos.x <= self.max.x && pos.y >= self.min.y && pos.y <= self.max.y
    }
}

/// タスクエリア表示用
#[derive(Component)]
pub struct TaskAreaIndicator(pub Entity); // 親の使い魔Entity

#[derive(Component)]
pub struct DesignationIndicator(pub Entity);

#[derive(Component)]
pub struct AreaSelectionIndicator;

// 公開API
pub use area_selection::{area_selection_indicator_system, task_area_selection_system};
pub use assign_task::assign_task_system;
pub use indicators::{task_area_indicator_system, update_designation_indicator_system};
pub use input::familiar_command_input_system;
pub use visualization::{designation_visual_system, familiar_command_visual_system};
