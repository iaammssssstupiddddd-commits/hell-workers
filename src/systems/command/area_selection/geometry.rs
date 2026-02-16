use super::state::{Drag, Operation};
use crate::constants::TILE_SIZE;
use crate::interface::camera::MainCamera;
use crate::systems::command::{AreaEditHandleKind, TaskArea, TaskMode};
use bevy::prelude::*;
use bevy::window::{CursorIcon, PrimaryWindow, SystemCursorIcon};

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

pub fn get_drag_start(mode: TaskMode) -> Option<Vec2> {
    match mode {
        TaskMode::AreaSelection(s) => s,
        TaskMode::DesignateChop(s) => s,
        TaskMode::DesignateMine(s) => s,
        TaskMode::DesignateHaul(s) => s,
        TaskMode::CancelDesignation(s) => s,
        TaskMode::ZonePlacement(_, s) => s,
        TaskMode::ZoneRemoval(_, s) => s,
        TaskMode::FloorPlace(s) => s,
        TaskMode::WallPlace(s) => s,
        _ => None,
    }
}

pub fn get_indicator_color(mode: TaskMode) -> LinearRgba {
    match mode {
        TaskMode::AreaSelection(_) => LinearRgba::from(Color::srgba(1.0, 1.0, 1.0, 0.4)),
        TaskMode::CancelDesignation(_) => LinearRgba::from(Color::srgba(1.0, 0.2, 0.2, 0.5)),
        TaskMode::ZonePlacement(_, _) => LinearRgba::from(Color::srgba(1.0, 1.0, 1.0, 0.4)), // TaskAreaと同様に白/透明
        TaskMode::ZoneRemoval(_, _) => LinearRgba::from(Color::srgba(1.0, 0.2, 0.2, 0.5)), // 削除は赤
        TaskMode::FloorPlace(_) => LinearRgba::from(Color::srgba(1.0, 1.0, 1.0, 0.4)),
        TaskMode::WallPlace(_) => LinearRgba::from(Color::srgba(1.0, 1.0, 1.0, 0.4)),
        _ => LinearRgba::from(Color::srgba(0.2, 1.0, 0.2, 0.5)),
    }
}

pub fn wall_line_area(start_pos: Vec2, end_pos: Vec2) -> TaskArea {
    let delta = end_pos - start_pos;
    if delta.length_squared() <= f32::EPSILON {
        return TaskArea::from_points(start_pos, start_pos + Vec2::splat(TILE_SIZE));
    }

    if delta.x.abs() >= delta.y.abs() {
        let y_dir = if delta.y < 0.0 { -1.0 } else { 1.0 };
        TaskArea::from_points(
            start_pos,
            Vec2::new(end_pos.x, start_pos.y + TILE_SIZE * y_dir),
        )
    } else {
        let x_dir = if delta.x < 0.0 { -1.0 } else { 1.0 };
        TaskArea::from_points(
            start_pos,
            Vec2::new(start_pos.x + TILE_SIZE * x_dir, end_pos.y),
        )
    }
}

pub(super) fn area_from_center_and_size(center: Vec2, size: Vec2) -> TaskArea {
    let half = size.abs() * 0.5;
    TaskArea {
        min: center - half,
        max: center + half,
    }
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
    let min = area.min;
    let max = area.max;
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
    let mut min = active_drag.original_area.min;
    let mut max = active_drag.original_area.max;

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

    TaskArea { min, max }
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

pub fn count_positions_in_area(area: &TaskArea, positions: impl Iterator<Item = Vec2>) -> usize {
    let mut count = 0usize;
    for pos in positions {
        if in_selection_area(area, pos) {
            count += 1;
        }
    }
    count
}

pub fn overlap_summary_from_areas(
    selected_entity: Entity,
    selected_area: &TaskArea,
    areas: impl Iterator<Item = (Entity, TaskArea)>,
) -> Option<(usize, f32)> {
    let selected_size = selected_area.size();
    let selected_area_value = selected_size.x.abs() * selected_size.y.abs();
    if selected_area_value <= f32::EPSILON {
        return None;
    }

    let mut overlap_count = 0usize;
    let mut max_ratio = 0.0f32;

    for (entity, area) in areas {
        if entity == selected_entity {
            continue;
        }

        let overlap_w =
            (selected_area.max.x.min(area.max.x) - selected_area.min.x.max(area.min.x)).max(0.0);
        let overlap_h =
            (selected_area.max.y.min(area.max.y) - selected_area.min.y.max(area.min.y)).max(0.0);
        let overlap_area = overlap_w * overlap_h;
        if overlap_area <= f32::EPSILON {
            continue;
        }

        overlap_count += 1;
        let ratio = (overlap_area / selected_area_value).clamp(0.0, 1.0);
        if ratio > max_ratio {
            max_ratio = ratio;
        }
    }

    Some((overlap_count, max_ratio))
}
