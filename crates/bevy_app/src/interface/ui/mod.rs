//! UIモジュール
//!
//! UIセットアップ、パネル、インタラクションを統合管理します。

pub mod interaction;
pub mod list;
pub mod panels;
pub mod plugins;
pub mod presentation;
pub mod setup;
pub mod vignette;

// hw_ui::components から外部が使うシンボル
pub use hw_ui::components::{InfoPanelNodes, MenuState, PlacementFailureTooltip, UiInputState};

// interaction から外部が使うシンボル
pub use interaction::{
    arch_category_action_system, door_lock_action_system, hover_action_button_system,
    move_plant_building_action_system, task_summary_ui_system, ui_interaction_system,
    ui_keyboard_shortcuts_system, update_area_edit_preview_ui_system,
    update_dream_loss_popup_ui_system, update_dream_pool_display_system,
    update_fps_display_system, update_mode_text_system, update_operation_dialog_system,
    update_speed_button_highlight_system, update_ui_input_state_system,
};

// list から外部が使うシンボル
pub use list::{
    DragState, EntityListMinimizeState, EntityListNodeIndex, EntityListResizeState,
    EntityListViewModel, build_entity_list_view_model_system, entity_list_drag_drop_system,
    entity_list_interaction_system, entity_list_minimize_toggle_system,
    entity_list_resize_cursor_system, entity_list_resize_system,
    entity_list_scroll_hint_visibility_system, entity_list_scroll_system,
    entity_list_section_toggle_system, entity_list_tab_focus_system,
    entity_list_visual_feedback_system, sync_entity_list_from_view_model_system,
    sync_entity_list_value_rows_system, update_unassigned_arrow_icon_system,
};

// panels から外部が使うシンボル
pub use panels::{
    InfoPanelPinState, InfoPanelState, context_menu_system, info_panel_system,
    menu_visibility_system,
};

// presentation から外部が使うシンボル
pub use presentation::update_entity_inspection_view_model_system;

// setup から外部が使うシンボル
pub use setup::setup_ui;
