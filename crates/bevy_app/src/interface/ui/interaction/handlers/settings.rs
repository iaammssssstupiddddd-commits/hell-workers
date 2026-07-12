use bevy::prelude::*;
use hw_core::GameSettings;
use hw_ui::UiIntent;
use hw_ui::components::MenuState;

use crate::DebugVisible;
use crate::systems::settings::apply::sync_debug_gizmos;
use crate::systems::settings::persistence::save_settings_to_disk;

pub fn handle(
    intent: UiIntent,
    settings: &mut GameSettings,
    menu_state: &mut MenuState,
    debug_visible: &mut DebugVisible,
    config_store: &mut GizmoConfigStore,
) -> bool {
    match intent {
        UiIntent::ToggleSettings => {
            if *menu_state == MenuState::Settings {
                *menu_state = MenuState::Hidden;
                true
            } else {
                *menu_state = MenuState::Settings;
                false
            }
        }
        UiIntent::CloseSettings => {
            *menu_state = MenuState::Hidden;
            true
        }
        UiIntent::SetUiScale(value) => {
            settings.ui_scale = value.clamp(0.85, 1.25);
            false
        }
        UiIntent::SetCameraPanSpeed(value) => {
            settings.camera_pan_speed = value.clamp(200.0, 1000.0);
            false
        }
        UiIntent::SetCameraMousePanEnabled(enabled) => {
            settings.camera_mouse_pan_enabled = enabled;
            false
        }
        UiIntent::SetDefaultTimeSpeed(speed) => {
            settings.default_time_speed = speed;
            false
        }
        UiIntent::SetDebugGizmosEnabled(enabled) => {
            settings.debug_gizmos_enabled = enabled;
            sync_debug_gizmos(enabled, debug_visible, config_store);
            false
        }
        UiIntent::SetFpsDisplayEnabled(enabled) => {
            settings.fps_display_enabled = enabled;
            false
        }
        _ => false,
    }
}

pub fn save_if_requested(should_save: bool, settings: &GameSettings) {
    if !should_save {
        return;
    }

    if let Err(err) = save_settings_to_disk(settings) {
        warn!("Failed to save settings: {err}");
    }
}
