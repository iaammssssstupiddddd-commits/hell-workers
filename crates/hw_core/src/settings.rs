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
        }
    }
}
