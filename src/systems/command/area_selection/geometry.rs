use super::state::{Drag, Operation};
use crate::interface::camera::MainCamera;
use crate::systems::command::{AreaEditHandleKind, TaskArea, TaskMode};
use crate::systems::world::zones::Site;
use bevy::prelude::*;
use bevy::window::{CursorIcon, PrimaryWindow, SystemCursorIcon};
use hw_core::constants::TILE_SIZE;
pub use hw_core::area::{
    area_from_center_and_size, count_positions_in_area, get_drag_start, overlap_summary_from_areas,
    wall_line_area,
};

const AREA_CONTAINS_MARGIN: f32 = 0.1;

pub(super) fn hotkey_slot_index(keyboard: &ButtonInput<KeyCode>) -> Option<usize> {
    if keyboard.just_pressed(KeyCode::Digit1) {
        Some(0)
    } else if keyboard.just_pressed(KeyCode::Digit2) {
        Some(1)
    } else if keyboard.just_pressed(KeyCode::Digit3) {
        Some(2)
    } else {
        None
    }
}

pub fn get_indicator_color(mode: TaskMode, is_valid: bool) -> LinearRgba {
    match mode {
        TaskMode::AreaSelection(_) => LinearRgba::from(Color::srgba(1.0, 1.0, 1.0, 0.4)),
        TaskMode::CancelDesignation(_) => LinearRgba::from(Color::srgba(1.0, 0.2, 0.2, 0.5)),
        TaskMode::ZonePlacement(_, _) => {
            if is_valid {
                LinearRgba::from(Color::srgba(1.0, 1.0, 1.0, 0.4))
            } else {
                LinearRgba::from(Color::srgba(1.0, 0.2, 0.2, 0.5))
            }
        }
        TaskMode::ZoneRemoval(_, _) => LinearRgba::from(Color::srgba(1.0, 0.2, 0.2, 0.5)), // 削除は赤
        TaskMode::FloorPlace(_) => LinearRgba::from(Color::srgba(1.0, 1.0, 1.0, 0.4)),
        TaskMode::WallPlace(_) => LinearRgba::from(Color::srgba(1.0, 1.0, 1.0, 0.4)),
        TaskMode::DreamPlanting(_) => LinearRgba::from(Color::srgba(0.5, 0.5, 1.0, 0.5)), // Dream は青紫
        _ => LinearRgba::from(Color::srgba(0.2, 1.0, 0.2, 0.5)),
    }
}

pub(super) fn clamp_area_to_site(area: &TaskArea, q_sites: &Query<&Site>) -> TaskArea {
    let Some(site) = q_sites.iter().find(|site| site.contains(area.center())) else {
        return area.clone();
    };

    let min = Vec2::new(area.min().x.max(site.min.x), area.min().y.max(site.min.y));
    let max = Vec2::new(area.max().x.min(site.max.x), area.max().y.min(site.max.y));
    if min.x > max.x || min.y > max.y {
        return area.clone();
    }

    TaskArea::from_points(min, max)
}

pub(super) fn world_cursor_pos(
    q_window: &Query<&Window, With<PrimaryWindow>>,
    q_camera: &Query<(&Camera, &GlobalTransform), With<MainCamera>>,
) -> Option<Vec2> {
    let Ok((camera, camera_transform)) = q_camera.single() else {
        return None;
    };
    let Ok(window) = q_window.single() else {
        return None;
    };
    let cursor_pos = window.cursor_position()?;
    camera
        .viewport_to_world_2d(camera_transform, cursor_pos)
        .ok()
}

pub(super) fn detect_area_edit_operation(area: &TaskArea, world_pos: Vec2) -> Option<Operation> {
    let threshold = TILE_SIZE * 0.55;
    let min = area.min();
    let max = area.max();
    let mid_x = (min.x + max.x) * 0.5;
    let mid_y = (min.y + max.y) * 0.5;

    let corners = [
        (AreaEditHandleKind::TopLeft, Vec2::new(min.x, max.y)),
        (AreaEditHandleKind::TopRight, Vec2::new(max.x, max.y)),
        (AreaEditHandleKind::BottomRight, Vec2::new(max.x, min.y)),
        (AreaEditHandleKind::BottomLeft, Vec2::new(min.x, min.y)),
    ];
    for (kind, point) in corners {
        if point.distance(world_pos) <= threshold {
            return Some(Operation::Resize(kind));
        }
    }

    if (world_pos.y - max.y).abs() <= threshold && world_pos.x >= min.x && world_pos.x <= max.x {
        return Some(Operation::Resize(AreaEditHandleKind::Top));
    }
    if (world_pos.x - max.x).abs() <= threshold && world_pos.y >= min.y && world_pos.y <= max.y {
        return Some(Operation::Resize(AreaEditHandleKind::Right));
    }
    if (world_pos.y - min.y).abs() <= threshold && world_pos.x >= min.x && world_pos.x <= max.x {
        return Some(Operation::Resize(AreaEditHandleKind::Bottom));
    }
    if (world_pos.x - min.x).abs() <= threshold && world_pos.y >= min.y && world_pos.y <= max.y {
        return Some(Operation::Resize(AreaEditHandleKind::Left));
    }

    if Vec2::new(mid_x, mid_y).distance(world_pos) <= threshold || area.contains(world_pos) {
        return Some(Operation::Move);
    }

    None
}

pub(super) fn apply_area_edit_drag(active_drag: &Drag, current_snapped: Vec2) -> TaskArea {
    let min_size = TILE_SIZE.max(1.0);
    let mut min = active_drag.original_area.min();
    let mut max = active_drag.original_area.max();

    match active_drag.operation {
        Operation::Move => {
            let delta = current_snapped - active_drag.drag_start;
            min += delta;
            max += delta;
        }
        Operation::Resize(handle) => match handle {
            AreaEditHandleKind::TopLeft => {
                min.x = current_snapped.x.min(max.x - min_size);
                max.y = current_snapped.y.max(min.y + min_size);
            }
            AreaEditHandleKind::Top => {
                max.y = current_snapped.y.max(min.y + min_size);
            }
            AreaEditHandleKind::TopRight => {
                max.x = current_snapped.x.max(min.x + min_size);
                max.y = current_snapped.y.max(min.y + min_size);
            }
            AreaEditHandleKind::Right => {
                max.x = current_snapped.x.max(min.x + min_size);
            }
            AreaEditHandleKind::BottomRight => {
                max.x = current_snapped.x.max(min.x + min_size);
                min.y = current_snapped.y.min(max.y - min_size);
            }
            AreaEditHandleKind::Bottom => {
                min.y = current_snapped.y.min(max.y - min_size);
            }
            AreaEditHandleKind::BottomLeft => {
                min.x = current_snapped.x.min(max.x - min_size);
                min.y = current_snapped.y.min(max.y - min_size);
            }
            AreaEditHandleKind::Left => {
                min.x = current_snapped.x.min(max.x - min_size);
            }
            AreaEditHandleKind::Center => {
                let delta = current_snapped - active_drag.drag_start;
                min += delta;
                max += delta;
            }
        },
    }

    TaskArea::from_points(min, max)
}

pub(super) fn cursor_icon_for_operation(operation: Operation, dragging: bool) -> CursorIcon {
    match operation {
        Operation::Move => {
            if dragging {
                CursorIcon::System(SystemCursorIcon::Grabbing)
            } else {
                CursorIcon::System(SystemCursorIcon::Grab)
            }
        }
        Operation::Resize(handle) => {
            let icon = match handle {
                AreaEditHandleKind::Top | AreaEditHandleKind::Bottom => SystemCursorIcon::NsResize,
                AreaEditHandleKind::Left | AreaEditHandleKind::Right => SystemCursorIcon::EwResize,
                AreaEditHandleKind::TopLeft | AreaEditHandleKind::BottomRight => {
                    SystemCursorIcon::NwseResize
                }
                AreaEditHandleKind::TopRight | AreaEditHandleKind::BottomLeft => {
                    SystemCursorIcon::NeswResize
                }
                AreaEditHandleKind::Center => SystemCursorIcon::Grab,
            };
            CursorIcon::System(icon)
        }
    }
}

pub(super) fn in_selection_area(area: &TaskArea, pos: Vec2) -> bool {
    area.contains_with_margin(pos, AREA_CONTAINS_MARGIN)
}
