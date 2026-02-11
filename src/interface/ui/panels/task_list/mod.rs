//! タスクリストパネル
//!
//! Designationを持つエンティティを一覧表示し、クリックでカメラ移動＋InfoPanel表示を行う。

mod layout;
pub mod update;

pub use layout::spawn_task_list_panel_ui;
pub use update::{
    right_panel_tab_system, right_panel_visibility_system, task_list_click_system,
    task_list_update_system,
};
