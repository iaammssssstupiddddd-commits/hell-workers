pub mod apply;
pub mod persistence;

use bevy::prelude::*;
use bevy::ui_widgets::{ValueChange, checkbox_self_update, slider_self_update};
use hw_core::GameSettings;
use hw_core::game_state::TimeSpeed;
use hw_ui::UiIntent;
use hw_ui::components::{SettingsCheckboxMarker, SettingsField, SettingsSliderMarker};

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
                )
                    .in_set(GameSystemSet::Interface)
                    .before(handle_ui_intent),
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
            // fixed-step audit と初期 checksum を横断比較する fixture は、
            // fixture checkpoint が採れた後に capture system が明示的に unpause する。
            // Startup から最初の Update までのゲーム更新を混入させないための gate。
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
