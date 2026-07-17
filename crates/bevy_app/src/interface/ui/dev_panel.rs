//! 開発用デバッグパネル
//!
//! ロジック確認のための 3D 表示切り替えボタン・即時ビルドトグルを提供する。

use crate::assets::GameAssets;
use crate::systems::visual::terrain_lod::{LodLevel, TerrainLodMetrics, TerrainLodState};
use bevy::prelude::*;
use hw_core::quality::{QualitySettings, RttQualityPreset};
use hw_ui::components::{UiInputBlocker, UiMountSlot, UiNodeRegistry, UiSlot};
use hw_ui::theme::UiTheme;
use hw_ui::widgets::{TextFieldConfig, TextFieldRole, spawn_text_field};

mod actions;
mod components;
mod presentation;
mod spawn;

pub use actions::{
    toggle_instant_build_button_system, toggle_render3d_button_system,
    toggle_rtt_extra_light_button_system, toggle_rtt_light_button_system,
    toggle_rtt_scene_objects_button_system, toggle_rtt_terrain_button_system,
    toggle_soul_mask_button_system,
};
pub use components::{
    InstantBuildButton, LodIndicatorText, RenderPerfStatusText, ToggleRender3dButton,
    ToggleRttExtraLightButton, ToggleRttLightButton, ToggleRttSceneObjectsButton,
    ToggleRttTerrainButton, ToggleSoulMaskButton,
};
pub use presentation::{
    update_instant_build_button_visual_system, update_lod_indicator_system,
    update_render_perf_status_system, update_render3d_button_visual_system,
    update_rtt_extra_light_button_visual_system, update_rtt_light_button_visual_system,
    update_rtt_scene_objects_button_visual_system, update_rtt_terrain_button_visual_system,
    update_soul_mask_button_visual_system,
};
pub use spawn::spawn_dev_panel_system;
