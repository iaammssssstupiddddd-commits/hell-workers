//! UIインタラクションモジュール
//!
//! ツールチップ、モードテキスト、タスクサマリー、およびボタン操作を管理します。

mod handlers;
mod intent_context;
mod intent_handler;
mod menu_actions;
mod mode;
mod status_display;
mod systems;
mod tooltip;

pub(crate) use hw_ui::interaction::common::despawn_context_menus;
pub(crate) use intent_handler::handle_ui_intent;

pub use hw_ui::interaction::hover_action::hover_action_button_system;
pub use status_display::{
    task_summary_ui_system, update_area_edit_preview_ui_system, update_dream_loss_popup_ui_system,
    update_dream_pool_display_system, update_fps_display_system, update_mode_text_system,
    update_speed_button_highlight_system,
};
pub(crate) use tooltip::hover_tooltip_system;

pub use systems::{
    arch_category_action_system, door_lock_action_system, move_plant_building_action_system,
    ui_interaction_system, ui_keyboard_shortcuts_system, update_operation_dialog_system,
    update_ui_input_state_system,
};
