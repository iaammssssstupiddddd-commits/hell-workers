use bevy::prelude::*;
use bevy::window::PrimaryWindow;

// 型定義は hw_core に移動し、ここでは re-export して既存コードを壊さない
pub use hw_core::camera::MainCamera;

/// Returns the current cursor position in world space, or `None` if unavailable.
pub fn world_cursor_pos(
    q_window: &Query<&Window, With<PrimaryWindow>>,
    q_camera: &Query<(&Camera, &GlobalTransform), With<MainCamera>>,
) -> Option<Vec2> {
    let Ok((camera, camera_transform)) = q_camera.single() else {
        return None;
    };
    let Ok(window) = q_window.single() else {
        return None;
    };
    let cursor_pos: Vec2 = window.cursor_position()?;
    camera
        .viewport_to_world_2d(camera_transform, cursor_pos)
        .ok()
}
