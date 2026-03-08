#[path = "view_model.rs"]
mod view_model;
#[path = "dirty.rs"]
mod dirty;
#[path = "../../panels_legacy/task_list/interaction.rs"]
mod interaction;
#[path = "../../panels_legacy/task_list/presenter.rs"]
mod presenter;
#[path = "../../panels_legacy/task_list/render.rs"]
mod render;
#[path = "../../panels_legacy/task_list/update.rs"]
mod update;
#[path = "../../panels_legacy/task_list/layout.rs"]
mod layout;

pub use dirty::{
    TaskListDirty, detect_task_list_changed_components, detect_task_list_removed_components,
};
pub use update::{
    left_panel_tab_system, left_panel_visibility_system, task_list_click_system,
    task_list_update_system, task_list_visual_feedback_system,
};
pub use view_model::{TaskListState, build_task_summary, update_task_list_state_system};
