use bevy::prelude::*;

use crate::game_state::TimeSpeed;

/// 永続化対象のゲーム設定（型定義のみ。ロード/保存は bevy_app）
#[derive(Resource, Reflect, Debug, Clone, PartialEq)]
#[reflect(Resource)]
pub struct GameSettings {
    /// UI 全体スケール（UiScale.0）
    pub ui_scale: f32,
    /// カメラ WASD パン速度（PanCamera.pan_speed）
    pub camera_pan_speed: f32,
    /// Project-owned world pointer gestureによるマウスドラッグパン。
    pub camera_mouse_pan_enabled: bool,
    /// 起動時のゲーム速度
    pub default_time_speed: TimeSpeed,
    /// gizmo デバッグ表示（DebugVisible.0）。F12 と双方向同期
    pub debug_gizmos_enabled: bool,
    /// DevPanel 内 FPS テキスト表示
    pub fps_display_enabled: bool,
    /// 電力不足時に consumer 優先度で配電する。false は旧 all-or-none 動作。
    pub power_priority_enabled: bool,
    /// Autosave enabled. Default off is an independent product decision.
    pub autosave_enabled: bool,
    /// Active-play interval in minutes. Allowed values: 5 / 10 / 20 / 30.
    pub autosave_interval_minutes: u32,
    /// Active autosave generation count `1..=5`.
    pub autosave_generations: u8,
    /// New toast duration in real seconds: 4 / 8 / 12.
    pub notification_duration_seconds: u8,
}

impl Default for GameSettings {
    fn default() -> Self {
        Self {
            ui_scale: 1.0,
            camera_pan_speed: 500.0,
            camera_mouse_pan_enabled: true,
            default_time_speed: TimeSpeed::Normal,
            debug_gizmos_enabled: false,
            fps_display_enabled: true,
            power_priority_enabled: true,
            autosave_enabled: false,
            autosave_interval_minutes: 10,
            autosave_generations: 3,
            notification_duration_seconds: 4,
        }
    }
}

/// Allowed autosave intervals (active real-time minutes).
pub const AUTOSAVE_INTERVAL_MINUTES: [u32; 4] = [5, 10, 20, 30];
pub const UI_SCALE_RANGE: (f32, f32) = (0.85, 1.25);
pub const CAMERA_PAN_SPEED_RANGE: (f32, f32) = (200.0, 1000.0);
pub const AUTOSAVE_INTERVAL_SLIDER_RANGE: (f32, f32) = (0.0, 3.0);
pub const AUTOSAVE_GENERATIONS_MIN: u8 = 1;
pub const AUTOSAVE_GENERATIONS_MAX: u8 = 5;
pub const AUTOSAVE_GENERATIONS_SLIDER_RANGE: (f32, f32) = (
    AUTOSAVE_GENERATIONS_MIN as f32,
    AUTOSAVE_GENERATIONS_MAX as f32,
);
pub const NOTIFICATION_DURATION_SLIDER_RANGE: (f32, f32) = (0.0, 2.0);

pub fn autosave_interval_from_slider(value: f32) -> u32 {
    let index = value.round().clamp(
        AUTOSAVE_INTERVAL_SLIDER_RANGE.0,
        AUTOSAVE_INTERVAL_SLIDER_RANGE.1,
    ) as usize;
    AUTOSAVE_INTERVAL_MINUTES[index]
}

pub fn autosave_generations_from_slider(value: f32) -> u8 {
    value.round().clamp(
        AUTOSAVE_GENERATIONS_SLIDER_RANGE.0,
        AUTOSAVE_GENERATIONS_SLIDER_RANGE.1,
    ) as u8
}

pub fn notification_duration_from_slider(value: f32) -> u8 {
    (value.round().clamp(
        NOTIFICATION_DURATION_SLIDER_RANGE.0,
        NOTIFICATION_DURATION_SLIDER_RANGE.1,
    ) as u8
        + 1)
        * 4
}

pub const fn normalize_notification_duration(value: u8) -> u8 {
    match value {
        8 => 8,
        12 => 12,
        _ => 4,
    }
}

pub fn normalize_autosave_generations(value: u8) -> u8 {
    value.clamp(AUTOSAVE_GENERATIONS_MIN, AUTOSAVE_GENERATIONS_MAX)
}

impl GameSettings {
    pub fn normalized_notification_duration_seconds(&self) -> u8 {
        normalize_notification_duration(self.notification_duration_seconds)
    }

    pub fn notification_duration_slider_value(&self) -> f32 {
        f32::from(self.normalized_notification_duration_seconds() / 4 - 1)
    }

    pub fn normalized_autosave_interval_minutes(&self) -> u32 {
        match self.autosave_interval_minutes {
            ..=7 => 5,
            8..=15 => 10,
            16..=25 => 20,
            _ => 30,
        }
    }

    pub fn normalized_autosave_generations(&self) -> u8 {
        normalize_autosave_generations(self.autosave_generations)
    }

    pub fn autosave_interval_slider_value(&self) -> f32 {
        let normalized = self.normalized_autosave_interval_minutes();
        AUTOSAVE_INTERVAL_MINUTES
            .iter()
            .position(|minutes| *minutes == normalized)
            .unwrap_or(1) as f32
    }

    pub fn set_autosave_interval_from_slider(&mut self, value: f32) {
        self.autosave_interval_minutes = autosave_interval_from_slider(value);
    }

    pub fn autosave_generations_slider_value(&self) -> f32 {
        f32::from(self.normalized_autosave_generations())
    }

    pub fn set_autosave_generations_from_slider(&mut self, value: f32) {
        self.autosave_generations = autosave_generations_from_slider(value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discrete_slider_inputs_keep_existing_rounding_and_nonfinite_values() {
        for (input, interval, generations, duration) in [
            (f32::NEG_INFINITY, 5, 1, 4),
            (0.49, 5, 1, 4),
            (0.5, 10, 1, 8),
            (1.5, 20, 2, 12),
            (2.0, 20, 2, 12),
            (2.5, 30, 3, 12),
            (3.0, 30, 3, 12),
            (4.0, 30, 4, 12),
            (5.0, 30, 5, 12),
            (f32::INFINITY, 30, 5, 12),
            (f32::NAN, 5, 0, 4),
        ] {
            assert_eq!(autosave_interval_from_slider(input), interval);
            assert_eq!(autosave_generations_from_slider(input), generations);
            assert_eq!(notification_duration_from_slider(input), duration);
        }
    }
}
