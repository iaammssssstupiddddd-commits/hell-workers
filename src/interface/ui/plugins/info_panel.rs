use crate::interface::ui::components::LeftPanelMode;
use crate::interface::ui::panels::task_list::{
    TaskListDirty, TaskListState, detect_task_list_changed_components,
    detect_task_list_removed_components, update_task_list_state_system,
};
use crate::interface::ui::panels::task_list::{
    left_panel_tab_system, left_panel_visibility_system, task_list_click_system,
    task_list_update_system, task_list_visual_feedback_system,
};
use crate::interface::ui::{
    InfoPanelNodes, InfoPanelPinState, InfoPanelState, info_panel_system,
    presentation::EntityInspectionViewModel,
    update_entity_inspection_view_model_system,
};
use crate::systems::GameSystemSet;
use bevy::prelude::*;

pub type UiInfoPanelPlugin = hw_ui::plugins::info_panel::UiInfoPanelPlugin;

pub fn ui_info_panel_plugin() -> UiInfoPanelPlugin {
    UiInfoPanelPlugin::new(register_ui_info_panel_plugin_systems)
}

fn register_ui_info_panel_plugin_systems(app: &mut App) {
    app.init_resource::<InfoPanelState>();
    app.init_resource::<InfoPanelPinState>();
    app.init_resource::<InfoPanelNodes>();
    app.init_resource::<LeftPanelMode>();
    app.init_resource::<EntityInspectionViewModel>();
    app.init_resource::<TaskListDirty>();
    app.init_resource::<TaskListState>();
    app.add_systems(
        PreUpdate,
        (
            detect_task_list_changed_components,
            detect_task_list_removed_components,
            update_task_list_state_system,
        )
            .chain(),
    );
    app.add_systems(
        Update,
        (
            (
                update_entity_inspection_view_model_system,
                info_panel_system
                    .run_if(
                        |selected: Res<crate::interface::selection::SelectedEntity>,
                            pin_state: Res<InfoPanelPinState>| {
                            selected.is_changed()
                                || pin_state.is_changed()
                                || selected.0.is_some()
                                || pin_state.entity.is_some()
                        },
                    )
                    .after(update_entity_inspection_view_model_system)
                    .after(crate::interface::ui::menu_visibility_system)
                    .before(crate::interface::ui::update_mode_text_system),
            )
                .chain(),
            left_panel_tab_system,
            left_panel_visibility_system.after(left_panel_tab_system),
            task_list_update_system.after(left_panel_tab_system),
            task_list_click_system,
            task_list_visual_feedback_system.after(task_list_click_system),
        )
            .in_set(GameSystemSet::Interface),
    );
}
