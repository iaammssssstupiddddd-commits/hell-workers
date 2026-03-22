use crate::systems::command::{TaskArea, TaskMode};
use hw_world::zones::Site;
use bevy::prelude::*;

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

pub(super) fn area_from_center_and_size(center: Vec2, size: Vec2) -> TaskArea {
    hw_core::area::area_from_center_and_size(center, size)
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

pub(super) fn in_selection_area(area: &TaskArea, pos: Vec2) -> bool {
    area.contains_with_margin(pos, AREA_CONTAINS_MARGIN)
}
