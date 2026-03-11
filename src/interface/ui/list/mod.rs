//! エンティティリストの動的更新システム

pub(crate) mod change_detection;
pub(crate) mod dirty;
mod drag_drop;
mod interaction;
mod selection_focus;
mod sync;
mod view_model;

pub use drag_drop::{DragState, entity_list_drag_drop_system};
pub use hw_ui::list::{
    EntityListNodeIndex, EntityListSnapshot, EntityListViewModel, FamiliarRowViewModel,
    FamiliarSectionNodes, SoulGender, SoulRowViewModel, StressBucket, TaskVisual,
};
pub use hw_ui::list::{EntityListMinimizeState, entity_list_minimize_toggle_system};
pub use hw_ui::list::{
    EntityListResizeState, entity_list_resize_cursor_system, entity_list_resize_system,
};
pub use selection_focus::focus_camera_on_entity;
pub use interaction::{
    apply_row_highlight, entity_list_interaction_system, entity_list_scroll_hint_visibility_system,
    entity_list_scroll_system, entity_list_section_toggle_system, entity_list_tab_focus_system,
    entity_list_visual_feedback_system, update_unassigned_arrow_icon_system,
};
pub use sync::{sync_entity_list_from_view_model_system, sync_entity_list_value_rows_system};
pub use view_model::build_entity_list_view_model_system;
