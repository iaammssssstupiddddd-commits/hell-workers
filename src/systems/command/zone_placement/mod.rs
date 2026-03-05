pub mod connectivity;
pub mod placement;
pub mod removal;
pub mod removal_preview;

use crate::interface::camera::MainCamera;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

fn world_cursor_pos(
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

pub(crate) use placement::{is_stockpile_area_within_yards, is_yard_expansion_area_valid};
pub use placement::zone_placement_system;
pub use removal::zone_removal_system;
pub use removal_preview::ZoneRemovalPreviewState;
