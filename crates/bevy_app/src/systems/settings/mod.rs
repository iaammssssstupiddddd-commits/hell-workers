pub mod apply;
pub(crate) mod feedback;
pub mod persistence;

use bevy::prelude::*;
use bevy::ui_widgets::{ValueChange, checkbox_self_update, slider_self_update};
use hw_core::GameSettings;
use hw_core::game_state::TimeSpeed;
use hw_ui::UiIntent;
use hw_ui::components::{
    SettingsCheckboxMarker, SettingsField, SettingsSliderMarker, SettingsValueText,
};

use crate::interface::ui::interaction::handle_ui_intent;
use crate::plugins::startup::PerfScenarioConfig;
use crate::systems::GameSystemSet;
use apply::apply_default_time_speed;
use persistence::{load_settings_from_disk, save_settings_to_disk};

pub use persistence::{DEFAULT_SETTINGS_STORAGE_ROOT, SettingsStorageRoot};

pub struct SettingsPlugin;

/// Project-owned `Last` work that must finish before a save/load world
/// replacement can run.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct SettingsPersistenceSet;

impl Plugin for SettingsPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<GameSettings>()
            .register_type::<TimeSpeed>()
            .init_resource::<SettingsStorageRoot>()
            .init_resource::<feedback::SettingsSaveState>()
            .add_observer(slider_self_update)
            .add_observer(checkbox_self_update)
            .add_observer(on_settings_slider_value_change)
            .add_observer(on_settings_checkbox_value_change)
            .add_systems(Startup, load_settings_system)
            .add_systems(
                Update,
                (
                    apply::apply_settings_system,
                    update_settings_default_speed_highlight,
                    update_settings_values,
                    feedback::update_settings_save_status,
                )
                    .in_set(GameSystemSet::Interface)
                    .before(handle_ui_intent),
            )
            .add_systems(
                Update,
                update_notification_duration
                    .after(handle_ui_intent)
                    .before(hw_ui::notifications::NotificationSystemSet::Reduce)
                    .in_set(GameSystemSet::Interface),
            )
            .add_systems(
                Update,
                feedback::sync_settings_retry_actions
                    .after(hw_ui::notifications::NotificationSystemSet::Reduce)
                    .before(hw_ui::notifications::NotificationSystemSet::Present)
                    .in_set(GameSystemSet::Interface),
            )
            .configure_sets(Last, SettingsPersistenceSet)
            .add_systems(
                Last,
                save_settings_on_app_exit_system.in_set(SettingsPersistenceSet),
            );
    }
}

fn load_settings_system(
    mut commands: Commands,
    mut time: ResMut<Time<Virtual>>,
    settings_root: Res<SettingsStorageRoot>,
    perf_config: Option<Res<PerfScenarioConfig>>,
) {
    let settings = load_settings_from_disk(&settings_root);
    if let Some(config) = perf_config.filter(|config| config.enabled()) {
        // 計測はローカルの settings.ron にある一時停止・倍速設定の影響を受けない。
        time.set_relative_speed(1.0);
        if config.freezes_fixture_setup() {
            // 全profiling fixtureは、初期checkpointが採れた後にcapture systemが
            // 明示的にunpauseする。Startupから最初のUpdateまでの可変delta更新を
            // initial checksumへ混入させないためのgate。
            time.pause();
        } else {
            time.unpause();
        }
    } else {
        apply_default_time_speed(&mut time, settings.default_time_speed);
    }
    commands.insert_resource(settings);
}

fn save_settings_on_app_exit_system(
    mut exit: MessageReader<AppExit>,
    settings: Res<GameSettings>,
    settings_root: Res<SettingsStorageRoot>,
    perf_config: Option<Res<PerfScenarioConfig>>,
) {
    if perf_config.is_some_and(|config| config.enabled()) {
        return;
    }
    if exit.read().next().is_none() {
        return;
    }

    if let Err(err) = save_settings_to_disk(&settings_root, &settings) {
        warn!("Failed to save settings on exit: {err}");
    }
}

fn on_settings_slider_value_change(
    change: On<ValueChange<f32>>,
    q_sliders: Query<&SettingsSliderMarker>,
    mut intents: MessageWriter<UiIntent>,
) {
    let Ok(marker) = q_sliders.get(change.source) else {
        return;
    };

    let intent = match marker.0 {
        SettingsField::UiScale => UiIntent::SetUiScale(change.value),
        SettingsField::CameraPanSpeed => UiIntent::SetCameraPanSpeed(change.value),
        SettingsField::AutosaveInterval => {
            let index = change.value.round().clamp(0.0, 3.0) as usize;
            UiIntent::SetAutosaveIntervalMinutes(hw_core::AUTOSAVE_INTERVAL_MINUTES[index])
        }
        SettingsField::NotificationDuration => {
            UiIntent::SetNotificationDuration((change.value.round().clamp(0.0, 2.0) as u8 + 1) * 4)
        }
        SettingsField::AutosaveGenerations => {
            UiIntent::SetAutosaveGenerations(change.value.round().clamp(1.0, 5.0) as u8)
        }
        _ => return,
    };
    intents.write(intent);
}

pub(crate) fn on_settings_checkbox_value_change(
    change: On<ValueChange<bool>>,
    q_checkboxes: Query<&SettingsCheckboxMarker>,
    mut intents: MessageWriter<UiIntent>,
) {
    let Ok(marker) = q_checkboxes.get(change.source) else {
        return;
    };

    let intent = match marker.0 {
        SettingsField::CameraMousePan => UiIntent::SetCameraMousePanEnabled(change.value),
        SettingsField::DebugGizmos => UiIntent::SetDebugGizmosEnabled(change.value),
        SettingsField::FpsDisplay => UiIntent::SetFpsDisplayEnabled(change.value),
        SettingsField::PowerPriority => UiIntent::SetPowerPriorityEnabled(change.value),
        SettingsField::AutosaveEnabled => UiIntent::SetAutosaveEnabled(change.value),
        _ => return,
    };
    intents.write(intent);
}

fn update_settings_default_speed_highlight(
    settings: Res<GameSettings>,
    theme: Res<hw_ui::theme::UiTheme>,
    mut q_buttons: Query<(
        &hw_ui::components::SettingsDefaultSpeedButton,
        &mut BackgroundColor,
    )>,
) {
    if !settings.is_changed() {
        return;
    }

    for (button, mut color) in q_buttons.iter_mut() {
        *color = if button.0 == settings.default_time_speed {
            BackgroundColor(theme.colors.speed_button_active)
        } else {
            BackgroundColor(theme.colors.button_default)
        };
    }
}

/// Show the applied resource value, including discrete autosave units.
fn update_settings_values(
    settings: Res<GameSettings>,
    mut values: Query<(&SettingsValueText, &mut Text)>,
) {
    for (marker, mut text) in &mut values {
        let label = match marker.0 {
            SettingsField::NotificationDuration => {
                format!("{} 秒", settings.normalized_notification_duration_seconds())
            }
            SettingsField::UiScale => format!("{:.0}%", settings.ui_scale * 100.0),
            SettingsField::CameraPanSpeed => {
                format!("{:.0} ワールド単位/秒", settings.camera_pan_speed)
            }
            SettingsField::AutosaveInterval => {
                format!("{} 分", settings.normalized_autosave_interval_minutes())
            }
            SettingsField::AutosaveGenerations => {
                format!("{} 世代", settings.normalized_autosave_generations())
            }
            _ => continue,
        };
        if text.0 != label {
            text.0 = label;
        }
    }
}

fn update_notification_duration(
    settings: Res<GameSettings>,
    mut center: ResMut<hw_ui::notifications::NotificationCenter>,
) {
    let duration = std::time::Duration::from_secs(u64::from(
        settings.normalized_notification_duration_seconds(),
    ));
    if center.toast_lifetime() != duration {
        center.set_toast_lifetime(duration);
    }
}

#[cfg(test)]
mod value_tests {
    use super::*;

    #[test]
    fn settings_values_show_applied_units_and_normalized_autosave_values() {
        let mut app = crate::test_support::minimal_app();
        app.insert_resource(GameSettings {
            ui_scale: 1.25,
            camera_pan_speed: 650.0,
            autosave_interval_minutes: 999,
            autosave_generations: 9,
            ..default()
        })
        .add_systems(Update, update_settings_values);
        let entries: Vec<_> = [
            SettingsField::UiScale,
            SettingsField::CameraPanSpeed,
            SettingsField::AutosaveInterval,
            SettingsField::AutosaveGenerations,
        ]
        .into_iter()
        .map(|field| {
            app.world_mut()
                .spawn((SettingsValueText(field), Text::default()))
                .id()
        })
        .collect();
        app.update();
        let settings = app.world().resource::<GameSettings>();
        let expected = [
            "125%".to_string(),
            "650 ワールド単位/秒".to_string(),
            format!("{} 分", settings.normalized_autosave_interval_minutes()),
            "5 世代".to_string(),
        ];
        for (entity, expected) in entries.iter().zip(expected) {
            assert_eq!(app.world().get::<Text>(*entity).unwrap().0, expected);
        }
        app.world_mut().resource_mut::<GameSettings>().ui_scale = 0.85;
        app.update();
        assert_eq!(app.world().get::<Text>(entries[0]).unwrap().0, "85%");
    }
}
