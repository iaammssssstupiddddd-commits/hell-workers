//! 手押し車候補の検証ヘルパー

use bevy::prelude::*;

use crate::systems::familiar_ai::decide::task_management::ReservationShadow;

use super::reservation::source_not_reserved;

pub fn find_nearest_wheelbarrow(
    task_pos: Vec2,
    queries: &crate::systems::familiar_ai::decide::task_management::FamiliarTaskAssignmentQueries,
    shadow: &ReservationShadow,
) -> Option<Entity> {
    queries
        .wheelbarrows
        .iter()
        .filter(|(wb_entity, _)| source_not_reserved(*wb_entity, queries, shadow))
        .min_by(|(_, t1), (_, t2)| {
            let d1 = t1.translation.truncate().distance_squared(task_pos);
            let d2 = t2.translation.truncate().distance_squared(task_pos);
            d1.partial_cmp(&d2).unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(e, _)| e)
}
