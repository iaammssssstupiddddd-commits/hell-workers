//! タスクリストパネル
//!
//! Designationを持つエンティティを一覧表示し、クリックでカメラ移動＋InfoPanel表示を行う。

mod interaction;
mod presenter;
mod render;
mod update;
mod view_model;

pub use update::{
    left_panel_tab_system, left_panel_visibility_system, task_list_click_system,
    task_list_update_system, task_list_visual_feedback_system,
};
pub use view_model::TaskListState;
