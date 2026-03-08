mod core;
mod entity_list;
mod foundation;
mod info_panel;
mod tooltip;
use bevy::prelude::App;
pub use core::ui_core_plugin;
pub use entity_list::ui_entity_list_plugin;
pub use info_panel::ui_info_panel_plugin;
pub use tooltip::ui_tooltip_plugin;

pub fn register_ui_plugins(app: &mut App) {
    app.add_plugins((
        ui_core_plugin(),
        ui_tooltip_plugin(),
        ui_info_panel_plugin(),
        ui_entity_list_plugin(),
    ));
}
pub use foundation::UiFoundationPlugin;
