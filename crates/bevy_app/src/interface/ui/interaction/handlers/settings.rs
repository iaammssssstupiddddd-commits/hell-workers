use bevy::input_focus::InputFocus;
use bevy::prelude::*;
use hw_core::GameSettings;
use hw_ui::UiIntent;
use hw_ui::components::MenuState;

use super::begin_overlay_open;
use crate::DebugVisible;
use crate::systems::settings::apply::sync_debug_gizmos;
use crate::systems::settings::persistence::SettingsStorageRoot;

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
            settings.ui_scale = value.clamp(
                hw_core::settings::UI_SCALE_RANGE.0,
                hw_core::settings::UI_SCALE_RANGE.1,
            );
            false
        }
        UiIntent::SetCameraPanSpeed(value) => {
            settings.camera_pan_speed = value.clamp(
                hw_core::settings::CAMERA_PAN_SPEED_RANGE.0,
                hw_core::settings::CAMERA_PAN_SPEED_RANGE.1,
            );
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
        UiIntent::SetPowerPriorityEnabled(enabled) => {
            settings.power_priority_enabled = enabled;
            false
        }
        UiIntent::SetAutosaveEnabled(enabled) => {
            settings.autosave_enabled = enabled;
            false
        }
        UiIntent::SetAutosaveIntervalMinutes(minutes) => {
            settings.autosave_interval_minutes = minutes;
            false
        }
        UiIntent::SetNotificationDuration(seconds) => {
            settings.notification_duration_seconds =
                hw_core::settings::normalize_notification_duration(seconds);
            false
        }
        UiIntent::SetAutosaveGenerations(generations) => {
            settings.autosave_generations =
                hw_core::settings::normalize_autosave_generations(generations);
            false
        }
        _ => false,
    }
}

pub(crate) fn save_if_requested(
    should_save: bool,
    settings_root: &SettingsStorageRoot,
    settings: &GameSettings,
    feedback: &mut crate::systems::settings::feedback::SettingsSaveFeedback,
) {
    if !should_save {
        return;
    }

    feedback.save_current(settings_root, settings);
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

    #[test]
    fn direct_setting_intents_preserve_entry_specific_normalization() {
        let mut settings = GameSettings::default();
        let mut menu = MenuState::Hidden;
        let mut debug = DebugVisible::default();
        let mut gizmos = GizmoConfigStore::default();
        let mut focus = InputFocus::default();
        let mut apply = |intent| {
            handle(
                intent,
                &mut settings,
                &mut menu,
                &mut debug,
                &mut gizmos,
                &mut focus,
            );
        };
        apply(UiIntent::SetAutosaveIntervalMinutes(17));
        apply(UiIntent::SetAutosaveGenerations(0));
        apply(UiIntent::SetNotificationDuration(7));
        apply(UiIntent::SetUiScale(f32::NAN));
        apply(UiIntent::SetCameraPanSpeed(f32::INFINITY));
        assert_eq!(settings.autosave_interval_minutes, 17);
        assert_eq!(settings.normalized_autosave_interval_minutes(), 20);
        assert_eq!(settings.autosave_generations, 1);
        assert_eq!(settings.notification_duration_seconds, 4);
        assert!(settings.ui_scale.is_nan());
        assert_eq!(settings.camera_pan_speed, 1000.0);
    }
}
