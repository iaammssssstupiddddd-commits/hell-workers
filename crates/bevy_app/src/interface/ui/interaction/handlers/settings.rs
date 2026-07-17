use bevy::input_focus::InputFocus;
use bevy::prelude::*;
use hw_core::GameSettings;
use hw_ui::UiIntent;
use hw_ui::components::MenuState;

use super::begin_overlay_open;
use crate::DebugVisible;
use crate::systems::settings::apply::sync_debug_gizmos;
use crate::systems::settings::persistence::save_settings_to_disk;

pub fn handle(
    intent: UiIntent,
    settings: &mut GameSettings,
    menu_state: &mut MenuState,
    debug_visible: &mut DebugVisible,
    config_store: &mut GizmoConfigStore,
    input_focus: &mut InputFocus,
) -> bool {
    match intent {
        UiIntent::ToggleSettings => {
            if *menu_state == MenuState::Settings {
                *menu_state = MenuState::Hidden;
                true
            } else {
                begin_overlay_open(input_focus);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opening_settings_clears_input_focus() {
        let mut settings = GameSettings::default();
        let mut menu_state = MenuState::Hidden;
        let mut debug_visible = DebugVisible::default();
        let mut config_store = GizmoConfigStore::default();
        let mut input_focus = InputFocus::from_entity(Entity::PLACEHOLDER);

        handle(
            UiIntent::ToggleSettings,
            &mut settings,
            &mut menu_state,
            &mut debug_visible,
            &mut config_store,
            &mut input_focus,
        );

        assert_eq!(menu_state, MenuState::Settings);
        assert!(input_focus.get().is_none());
    }
}
