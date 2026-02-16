pub mod blueprint;
pub mod bucket;
pub mod consolidation;
pub mod floor_construction;
pub mod mixer;
pub mod provisional_wall;
pub mod stockpile_group;
pub mod tank_water_request;
pub mod task_area;
pub mod upsert;
pub mod wheelbarrow;

use crate::systems::command::TaskArea;
use bevy::math::Vec2;
use bevy::prelude::Entity;

pub(crate) fn find_owner_familiar(
    pos: Vec2,
    familiars: &[(Entity, TaskArea)],
) -> Option<(Entity, &TaskArea)> {
    familiars
        .iter()
        .filter(|(_, area)| area.contains(pos))
        .min_by(|(_, area1), (_, area2)| {
            let d1 = area1.center().distance_squared(pos);
            let d2 = area2.center().distance_squared(pos);
            d1.partial_cmp(&d2).unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(entity, area)| (*entity, area))
}
