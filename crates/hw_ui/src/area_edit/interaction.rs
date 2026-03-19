use super::{AreaEditDrag, AreaEditHandleKind, AreaEditOperation};
use bevy::prelude::*;
use bevy::window::{CursorIcon, SystemCursorIcon};
use hw_core::area::TaskArea;
use hw_core::constants::TILE_SIZE;

pub fn detect_area_edit_operation(area: &TaskArea, world_pos: Vec2) -> Option<AreaEditOperation> {
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
            return Some(AreaEditOperation::Resize(kind));
        }
    }

    if (world_pos.y - max.y).abs() <= threshold && world_pos.x >= min.x && world_pos.x <= max.x {
        return Some(AreaEditOperation::Resize(AreaEditHandleKind::Top));
    }
    if (world_pos.x - max.x).abs() <= threshold && world_pos.y >= min.y && world_pos.y <= max.y {
        return Some(AreaEditOperation::Resize(AreaEditHandleKind::Right));
    }
    if (world_pos.y - min.y).abs() <= threshold && world_pos.x >= min.x && world_pos.x <= max.x {
        return Some(AreaEditOperation::Resize(AreaEditHandleKind::Bottom));
    }
    if (world_pos.x - min.x).abs() <= threshold && world_pos.y >= min.y && world_pos.y <= max.y {
        return Some(AreaEditOperation::Resize(AreaEditHandleKind::Left));
    }

    if Vec2::new(mid_x, mid_y).distance(world_pos) <= threshold || area.contains(world_pos) {
        return Some(AreaEditOperation::Move);
    }

    None
}

pub fn apply_area_edit_drag(active_drag: &AreaEditDrag, current_snapped: Vec2) -> TaskArea {
    let min_size = TILE_SIZE.max(1.0);
    let mut min = active_drag.original_area.min();
    let mut max = active_drag.original_area.max();

    match active_drag.operation {
        AreaEditOperation::Move => {
            let delta = current_snapped - active_drag.drag_start;
            min += delta;
            max += delta;
        }
        AreaEditOperation::Resize(handle) => match handle {
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

pub fn cursor_icon_for_operation(operation: AreaEditOperation, dragging: bool) -> CursorIcon {
    match operation {
        AreaEditOperation::Move => {
            if dragging {
                CursorIcon::System(SystemCursorIcon::Grabbing)
            } else {
                CursorIcon::System(SystemCursorIcon::Grab)
            }
        }
        AreaEditOperation::Resize(handle) => {
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
