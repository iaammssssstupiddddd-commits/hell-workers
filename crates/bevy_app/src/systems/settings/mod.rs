pub mod apply;
pub mod persistence;

use bevy::prelude::*;
use bevy::ui_widgets::{ValueChange, checkbox_self_update, slider_self_update};
use hw_core::GameSettings;
use hw_core::game_state::TimeSpeed;
use hw_ui::UiIntent;
use hw_ui::components::{SettingsCheckboxMarker, SettingsField, SettingsSliderMarker};

use crate::plugins::startup::PerfScenarioConfig;
use apply::apply_default_time_speed;
use persistence::{load_settings_from_disk, save_settings_to_disk};

pub struct SettingsPlugin;

impl Plugin for SettingsPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<GameSettings>()
            .register_type::<TimeSpeed>()
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
                ),
            )
            .add_systems(Last, save_settings_on_app_exit_system);
    }
}

fn load_settings_system(
    mut commands: Commands,
    mut time: ResMut<Time<Virtual>>,
    perf_config: Option<Res<PerfScenarioConfig>>,
) {
    let settings = load_settings_from_disk();
    if let Some(config) = perf_config.filter(|config| config.enabled()) {
        // 計測はローカルの settings.ron にある一時停止・倍速設定の影響を受けない。
        time.set_relative_speed(1.0);
        if config.uses_fixed_timesteps() {
            // fixed-step audit は fixture checkpoint が採れた後に capture system が
            // 明示的に unpause する。Startup から最初の Update までのゲーム更新を
            // 混入させないための gate である。
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
    perf_config: Option<Res<PerfScenarioConfig>>,
) {
    if perf_config.is_some_and(|config| config.enabled()) {
        return;
    }
    if exit.read().next().is_none() {
        return;
    }

    if let Err(err) = save_settings_to_disk(&settings) {
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
        _ => return,
    };
    intents.write(intent);
}

fn on_settings_checkbox_value_change(
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
