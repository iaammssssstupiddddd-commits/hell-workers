use bevy::camera_controller::pan_camera::PanCamera;
use bevy::prelude::*;
use hw_core::GameSettings;
use hw_core::game_state::TimeSpeed;
use hw_core::ui_nodes::{UiNodeRegistry, UiSlot};
use hw_ui::camera::MainCamera;

use crate::DebugVisible;

pub fn apply_default_time_speed(time: &mut Time<Virtual>, speed: TimeSpeed) {
    match speed {
        TimeSpeed::Paused => time.pause(),
        TimeSpeed::Normal => {
            time.unpause();
            time.set_relative_speed(1.0);
        }
        TimeSpeed::Fast => {
            time.unpause();
            time.set_relative_speed(2.0);
        }
        TimeSpeed::Super => {
            time.unpause();
            time.set_relative_speed(4.0);
        }
    }
}

pub fn sync_debug_gizmos(
    enabled: bool,
    visible: &mut DebugVisible,
    config_store: &mut GizmoConfigStore,
) {
    visible.0 = enabled;
    for (_, config, _) in config_store.iter_mut() {
        config.enabled = enabled;
    }
}

pub fn apply_fps_display_visibility(
    ui_nodes: &UiNodeRegistry,
    q_visibility: &mut Query<&mut Visibility>,
    enabled: bool,
) {
    let Some(entity) = ui_nodes.get_slot(UiSlot::FpsText) else {
        return;
    };
    if let Ok(mut visibility) = q_visibility.get_mut(entity) {
        *visibility = if enabled {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
}

pub fn apply_settings_system(
    settings: Res<GameSettings>,
    mut ui_scale: ResMut<UiScale>,
    mut q_pan_camera: Query<&mut PanCamera, With<MainCamera>>,
    mut debug_visible: ResMut<DebugVisible>,
    mut config_store: ResMut<GizmoConfigStore>,
    ui_nodes: Res<UiNodeRegistry>,
    mut q_visibility: Query<&mut Visibility>,
) {
    if !settings.is_changed() {
        return;
    }

    ui_scale.0 = settings.ui_scale;

    if let Ok(mut pan_camera) = q_pan_camera.single_mut() {
        pan_camera.pan_speed = settings.camera_pan_speed;
        pan_camera.mouse_pan_settings.enabled = settings.camera_mouse_pan_enabled;
    }

    sync_debug_gizmos(
        settings.debug_gizmos_enabled,
        &mut debug_visible,
        &mut config_store,
    );

    apply_fps_display_visibility(&ui_nodes, &mut q_visibility, settings.fps_display_enabled);
}
