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
    presentation::EntityInspectionViewModel, update_entity_inspection_view_model_system,
};
use crate::systems::GameSystemSet;
use bevy::prelude::*;
use bevy::time::Real;
use hw_ui::components::{LeftPanelMode, SoulRenameState};
use hw_ui::interaction::{soul_rename_button_system, soul_rename_cleanup_system};

const INSPECTION_REFRESH_INTERVAL_SECS: f32 = 0.1;

/// The selected/pinned inspector is dynamic, but rebuilding its strings every
/// render frame is unnecessary. Selection and pin changes wake immediately;
/// steady inspection refreshes run from real time so pausing simulation does
/// not freeze the panel.
#[derive(Resource)]
struct InspectionRefreshCadence {
    timer: Timer,
    due: bool,
}

impl Default for InspectionRefreshCadence {
    fn default() -> Self {
        Self {
            timer: Timer::from_seconds(INSPECTION_REFRESH_INTERVAL_SECS, TimerMode::Repeating),
            due: true,
        }
    }
}

fn advance_inspection_refresh_cadence_system(
    time: Res<Time<Real>>,
    mut cadence: ResMut<InspectionRefreshCadence>,
) {
    cadence.due = cadence.timer.tick(time.delta()).just_finished();
}

fn inspection_refresh_should_run(
    selected: Res<crate::interface::selection::SelectedEntity>,
    pin_state: Res<InfoPanelPinState>,
    rename_state: Res<SoulRenameState>,
    cadence: Res<InspectionRefreshCadence>,
) -> bool {
    selected.is_changed()
        || pin_state.is_changed()
        || rename_state.is_changed()
        || (cadence.due && (selected.0.is_some() || pin_state.entity.is_some()))
}

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
    app.init_resource::<InspectionRefreshCadence>();
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
        advance_inspection_refresh_cadence_system.in_set(GameSystemSet::Interface),
    );
    app.add_systems(
        Update,
        (
            left_panel_tab_system,
            left_panel_visibility_system.after(left_panel_tab_system),
            task_list_update_system.after(left_panel_tab_system),
            task_list_click_system,
            task_list_visual_feedback_system.after(task_list_click_system),
            soul_rename_button_system::<crate::assets::GameAssets>,
            soul_rename_cleanup_system,
        )
            .in_set(GameSystemSet::Interface),
    );
    app.add_systems(
        Update,
        (
            update_entity_inspection_view_model_system,
            info_panel_system::<crate::assets::GameAssets>
                .after(update_entity_inspection_view_model_system)
                .after(crate::interface::ui::menu_visibility_system)
                .before(crate::interface::ui::update_mode_text_system),
        )
            .chain()
            .run_if(inspection_refresh_should_run)
            .after(advance_inspection_refresh_cadence_system)
            .in_set(GameSystemSet::Interface),
    );
}
