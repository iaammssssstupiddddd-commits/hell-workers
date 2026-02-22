use crate::systems::familiar_ai::decide::task_management::ReservationShadow;
use crate::systems::logistics::ResourceType;
use bevy::prelude::*;

use super::reservation::source_not_reserved;

/// M7: ReturnBucket request 用に、指定タンクに紐づくドロップバケツで最も近いものを検索
/// ストックパイル内のバケツは除外する（既に返却済み）
pub fn find_nearest_bucket_for_return(
    tank_entity: Entity,
    task_pos: Vec2,
    queries: &crate::systems::familiar_ai::decide::task_management::FamiliarTaskAssignmentQueries,
    shadow: &ReservationShadow,
) -> Option<(Entity, Vec2)> {
    queries
        .free_resource_items
        .iter()
        .filter(|(_, _, vis, res)| {
            *vis != Visibility::Hidden
                && matches!(res.0, ResourceType::BucketEmpty | ResourceType::BucketWater)
        })
        .filter(|(e, _, _, _)| {
            queries.designation.belongs.get(*e).ok().map(|b| b.0) == Some(tank_entity)
        })
        // ストックパイル内のバケツを除外（返却対象は地面にあるもののみ）
        .filter(|(e, _, _, _)| {
            queries
                .designation
                .targets
                .get(*e)
                .ok()
                .is_some_and(|(_, _, _, _, _, _, stored_in_opt)| stored_in_opt.is_none())
        })
        .filter(|(e, _, _, _)| source_not_reserved(*e, queries, shadow))
        .min_by(|(_, t1, _, _), (_, t2, _, _)| {
            let d1 = t1.translation.truncate().distance_squared(task_pos);
            let d2 = t2.translation.truncate().distance_squared(task_pos);
            d1.partial_cmp(&d2).unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(e, t, _, _)| (e, t.translation.truncate()))
}

fn find_best_bucket_storage_for_return(
    tank_entity: Entity,
    source_pos: Vec2,
    queries: &crate::systems::familiar_ai::decide::task_management::FamiliarTaskAssignmentQueries,
    _shadow: &ReservationShadow,
) -> Option<Entity> {
    queries
        .storage
        .stockpiles
        .iter()
        .filter(|(stockpile_entity, _, stockpile, stored_opt)| {
            if queries
                .storage
                .bucket_storages
                .get(*stockpile_entity)
                .is_err()
            {
                return false;
            }

            let owner = queries
                .designation
                .belongs
                .get(*stockpile_entity)
                .ok()
                .map(|belongs| belongs.0);
            if owner != Some(tank_entity) {
                return false;
            }

            let type_ok = matches!(
                stockpile.resource_type,
                None | Some(ResourceType::BucketEmpty) | Some(ResourceType::BucketWater)
            );
            if !type_ok {
                return false;
            }

            let current = stored_opt.map(|stored| stored.len()).unwrap_or(0);
            let incoming = queries
                .reservation
                .incoming_deliveries_query
                .get(*stockpile_entity)
                .ok()
                .map(|inc: &crate::relationships::IncomingDeliveries| inc.len())
                .unwrap_or(0);
            let reserved = incoming;
            (current + reserved) < stockpile.capacity
        })
        .min_by(|(_, t1, _, _), (_, t2, _, _)| {
            let d1 = t1.translation.truncate().distance_squared(source_pos);
            let d2 = t2.translation.truncate().distance_squared(source_pos);
            d1.partial_cmp(&d2).unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(entity, _, _, _)| entity)
}

pub fn find_bucket_return_assignment(
    tank_entity: Entity,
    task_pos: Vec2,
    queries: &crate::systems::familiar_ai::decide::task_management::FamiliarTaskAssignmentQueries,
    shadow: &ReservationShadow,
) -> Option<(Entity, Vec2, Entity)> {
    let (source_item, source_pos) =
        find_nearest_bucket_for_return(tank_entity, task_pos, queries, shadow)?;
    let destination =
        find_best_bucket_storage_for_return(tank_entity, source_pos, queries, shadow)?;
    Some((source_item, source_pos, destination))
}

pub fn find_nearest_bucket_for_tank(
    tank_entity: Entity,
    task_pos: Vec2,
    queries: &crate::systems::familiar_ai::decide::task_management::FamiliarTaskAssignmentQueries,
    shadow: &ReservationShadow,
) -> Option<(Entity, Vec2)> {
    queries
        .free_resource_items
        .iter()
        .filter(|(_, _, vis, res)| {
            *vis != Visibility::Hidden
                && matches!(res.0, ResourceType::BucketEmpty | ResourceType::BucketWater)
        })
        .filter(|(e, _, _, _)| {
            queries.designation.belongs.get(*e).ok().map(|b| b.0) == Some(tank_entity)
        })
        .filter(|(e, _, _, _)| source_not_reserved(*e, queries, shadow))
        .min_by(|(_, t1, _, _), (_, t2, _, _)| {
            let d1 = t1.translation.truncate().distance_squared(task_pos);
            let d2 = t2.translation.truncate().distance_squared(task_pos);
            d1.partial_cmp(&d2).unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(e, t, _, _)| (e, t.translation.truncate()))
}
