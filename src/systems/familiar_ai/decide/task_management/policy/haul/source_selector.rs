//! 運搬タスクのソースアイテム探索

use crate::systems::familiar_ai::decide::task_management::{
    ReservationShadow,
    validator::source_not_reserved,
};
use crate::systems::logistics::ResourceType;
use bevy::prelude::*;

pub fn find_nearest_mixer_source_item(
    item_type: ResourceType,
    mixer_pos: Vec2,
    queries: &crate::systems::soul_ai::execute::task_execution::context::TaskAssignmentQueries,
    shadow: &ReservationShadow,
) -> Option<(Entity, Vec2)> {
    queries
        .free_resource_items
        .iter()
        .filter(|(_, _, visibility, res_item)| {
            **visibility != Visibility::Hidden && res_item.0 == item_type
        })
        .filter(|(entity, _, _, _)| source_not_reserved(*entity, queries, shadow))
        .min_by(|(_, t1, _, _), (_, t2, _, _)| {
            let d1 = t1.translation.truncate().distance_squared(mixer_pos);
            let d2 = t2.translation.truncate().distance_squared(mixer_pos);
            d1.partial_cmp(&d2).unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(e, t, _, _)| (e, t.translation.truncate()))
}

pub fn find_nearest_stockpile_source_item(
    resource_type: ResourceType,
    item_owner: Option<Entity>,
    stock_pos: Vec2,
    queries: &crate::systems::soul_ai::execute::task_execution::context::TaskAssignmentQueries,
    shadow: &ReservationShadow,
) -> Option<(Entity, Vec2)> {
    queries
        .free_resource_items
        .iter()
        .filter(|(_, _, visibility, res_item)| {
            **visibility != Visibility::Hidden && res_item.0 == resource_type
        })
        .filter(|(entity, _, _, _)| source_not_reserved(*entity, queries, shadow))
        .filter(|(entity, _, _, _)| {
            queries
                .designation
                .targets
                .get(*entity)
                .ok()
                .is_some_and(|(_, _, _, _, _, _, stored_in_opt)| stored_in_opt.is_none())
        })
        .filter(|(entity, _, _, _)| {
            let belongs = queries.designation.belongs.get(*entity).ok().map(|b| b.0);
            item_owner == belongs
        })
        .min_by(|(_, t1, _, _), (_, t2, _, _)| {
            let d1 = t1.translation.truncate().distance_squared(stock_pos);
            let d2 = t2.translation.truncate().distance_squared(stock_pos);
            d1.partial_cmp(&d2).unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(e, t, _, _)| (e, t.translation.truncate()))
}

pub fn find_nearest_blueprint_source_item(
    resource_type: ResourceType,
    bp_pos: Vec2,
    queries: &crate::systems::soul_ai::execute::task_execution::context::TaskAssignmentQueries,
    shadow: &ReservationShadow,
) -> Option<(Entity, Vec2)> {
    queries
        .free_resource_items
        .iter()
        .filter(|(_, _, visibility, res_item)| {
            **visibility != Visibility::Hidden && res_item.0 == resource_type
        })
        .filter(|(entity, _, _, _)| source_not_reserved(*entity, queries, shadow))
        .min_by(|(_, t1, _, _), (_, t2, _, _)| {
            let d1 = t1.translation.truncate().distance_squared(bp_pos);
            let d2 = t2.translation.truncate().distance_squared(bp_pos);
            d1.partial_cmp(&d2).unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(e, t, _, _)| (e, t.translation.truncate()))
}
