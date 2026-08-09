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
    /// マウスドラッグパン（PanCamera.mouse_pan_settings.enabled）
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
        }
    }
}

/// Allowed autosave intervals (active real-time minutes).
pub const AUTOSAVE_INTERVAL_MINUTES: [u32; 4] = [5, 10, 20, 30];

impl GameSettings {
    pub fn normalized_autosave_interval_minutes(&self) -> u32 {
        match self.autosave_interval_minutes {
            ..=7 => 5,
            8..=15 => 10,
            16..=25 => 20,
            _ => 30,
        }
    }

    pub fn normalized_autosave_generations(&self) -> u8 {
        self.autosave_generations.clamp(1, 5)
    }

    pub fn autosave_interval_slider_value(&self) -> f32 {
        let normalized = self.normalized_autosave_interval_minutes();
        AUTOSAVE_INTERVAL_MINUTES
            .iter()
            .position(|minutes| *minutes == normalized)
            .unwrap_or(1) as f32
    }

    pub fn set_autosave_interval_from_slider(&mut self, value: f32) {
        let index = value.round().clamp(0.0, 3.0) as usize;
        self.autosave_interval_minutes = AUTOSAVE_INTERVAL_MINUTES[index];
    }

    pub fn autosave_generations_slider_value(&self) -> f32 {
        f32::from(self.normalized_autosave_generations())
    }

    pub fn set_autosave_generations_from_slider(&mut self, value: f32) {
        self.autosave_generations = value.round().clamp(1.0, 5.0) as u8;
    }
}
