mod dirty;
mod layout;
mod presenter;
mod update;
mod view_model;

pub use dirty::{detect_task_list_changed_components, detect_task_list_removed_components};
pub use hw_ui::panels::task_list::{
    TaskEntry, TaskListDirty, left_panel_tab_system, left_panel_visibility_system,
    rebuild_task_list_ui, task_list_click_system, task_list_visual_feedback_system,
};
pub use update::task_list_update_system;
pub use view_model::{TaskListState, build_task_summary, update_task_list_state_system};
