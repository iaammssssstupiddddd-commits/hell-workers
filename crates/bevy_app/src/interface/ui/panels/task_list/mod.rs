mod actions;
mod dirty;
mod layout;
mod presenter;
mod update;
mod view_model;

pub use actions::{
    TaskActionKind, TaskActionOutcome, TaskActionResult, adapt_task_action_outcomes,
    apply_task_action_intents_system, task_dashboard_action_button_system,
};
pub use dirty::{detect_task_list_changed_components, detect_task_list_removed_components};
pub use hw_ui::panels::task_list::{
    TaskEntry, TaskListDirty, left_panel_tab_system, left_panel_visibility_system,
    rebuild_task_list_ui, task_dashboard_action_state_sync_system, task_dashboard_control_system,
    task_list_click_system, task_list_visual_feedback_system,
};
pub use update::task_list_update_system;
pub use view_model::{TaskListState, build_task_summary, update_task_list_state_system};
