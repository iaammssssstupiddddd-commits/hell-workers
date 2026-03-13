//! UIパネル・メニューモジュール
//!
//! メニューの表示制御は `hw_ui` 側へ移譲し、情報パネル・タスクリスト・ツールチップは残留。

mod context_menu;
pub mod task_list;
pub use hw_ui::panels::menu_visibility_system;
pub use hw_ui::panels::info_panel::{InfoPanelPinState, InfoPanelState, info_panel_system, spawn_info_panel_ui};

pub use context_menu::context_menu_system;

pub mod tooltip_builder {
    pub use hw_ui::panels::tooltip_builder::rebuild_tooltip_content;
}
