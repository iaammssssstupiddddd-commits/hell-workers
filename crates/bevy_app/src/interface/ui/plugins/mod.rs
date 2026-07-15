mod core;
mod entity_list;
mod foundation;
mod info_panel;
mod tooltip;
use bevy::prelude::*;
pub use core::ui_core_plugin;
pub use entity_list::ui_entity_list_plugin;
pub use foundation::UiFoundationPlugin;
use hw_ui::HwUiPlugin;
pub use info_panel::ui_info_panel_plugin;
pub use tooltip::ui_tooltip_plugin;

pub fn register_ui_plugins(app: &mut App) {
    app.add_plugins((
        HwUiPlugin,
        UiFoundationPlugin,
        ui_core_plugin(),
        ui_tooltip_plugin(),
        ui_info_panel_plugin(),
        ui_entity_list_plugin(),
    ));
    crate::systems::save::register_load_reset_hook(app, "hw-ui", hw_ui::reset_for_world_replace);
    crate::systems::save::register_load_reset_hook(
        app,
        "root-ui-task-list",
        reset_root_ui_task_list,
    );
}

fn reset_root_ui_task_list(world: &mut World) {
    use crate::interface::ui::panels::task_list::{TaskListDirty, TaskListState};

    if world.contains_resource::<TaskListState>() {
        world.insert_resource(TaskListState::default());
    }
    if let Some(mut dirty) = world.get_resource_mut::<TaskListDirty>() {
        dirty.mark_all();
    }
}
