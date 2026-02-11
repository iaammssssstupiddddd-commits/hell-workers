use crate::interface::ui::components::RightPanelMode;
use crate::interface::ui::panels::task_list::{
    right_panel_tab_system, right_panel_visibility_system, task_list_click_system,
    task_list_update_system,
};
use crate::interface::ui::panels::task_list::update::TaskListState;
use crate::interface::ui::{
    InfoPanelNodes, InfoPanelPinState, InfoPanelState, info_panel_system, menu_visibility_system,
    update_mode_text_system,
};
use crate::systems::GameSystemSet;
use bevy::prelude::*;

pub struct UiInfoPanelPlugin;

impl Plugin for UiInfoPanelPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<InfoPanelState>();
        app.init_resource::<InfoPanelPinState>();
        app.init_resource::<InfoPanelNodes>();
        app.init_resource::<RightPanelMode>();
        app.init_resource::<TaskListState>();
        app.add_systems(
            Update,
            (
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
                    .after(menu_visibility_system)
                    .before(update_mode_text_system),
                right_panel_tab_system,
                right_panel_visibility_system
                    .after(info_panel_system)
                    .after(right_panel_tab_system),
                task_list_update_system.after(right_panel_tab_system),
                task_list_click_system,
            )
                .in_set(GameSystemSet::Interface),
        );
    }
}
