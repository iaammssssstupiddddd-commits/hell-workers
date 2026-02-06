use crate::systems::command::TaskArea;
use crate::systems::familiar_ai::task_management::ReservationShadow;
use crate::systems::logistics::ResourceType;
use bevy::prelude::*;

pub fn find_best_stockpile_for_item(
    task_pos: Vec2,
    task_area_opt: Option<&TaskArea>,
    item_type: ResourceType,
    item_owner: Option<Entity>,
    queries: &crate::systems::soul_ai::task_execution::context::TaskAssignmentQueries,
    shadow: &ReservationShadow,
) -> Option<Entity> {
    queries
        .storage.stockpiles
        .iter()
        .filter(|(s_entity, s_transform, stock, stored)| {
            if let Some(area) = task_area_opt {
                if !area.contains(s_transform.translation.truncate()) {
                    return false;
                }
            }

            let stock_owner = queries.designation.belongs.get(*s_entity).ok().map(|b| b.0);
            if item_owner != stock_owner {
                return false;
            }

            let is_dedicated = stock_owner.is_some();
            let is_bucket = matches!(item_type, ResourceType::BucketEmpty | ResourceType::BucketWater);

            let type_match = if is_dedicated && is_bucket {
                true
            } else {
                stock.resource_type.is_none() || stock.resource_type == Some(item_type)
            };

            let current_count = stored.map(|s| s.len()).unwrap_or(0);
            let reserved = queries.reservation.resource_cache.get_destination_reservation(*s_entity)
                + shadow.destination_reserved(*s_entity);
            let has_capacity = (current_count + reserved) < stock.capacity as usize;

            type_match && has_capacity
        })
        .min_by(|(_, t1, _, _), (_, t2, _, _)| {
            let d1 = t1.translation.truncate().distance_squared(task_pos);
            let d2 = t2.translation.truncate().distance_squared(task_pos);
            d1.partial_cmp(&d2).unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(e, _, _, _)| e)
}

pub fn find_best_tank_for_bucket(
    task_entity: Entity,
    task_pos: Vec2,
    task_area_opt: Option<&TaskArea>,
    queries: &crate::systems::soul_ai::task_execution::context::TaskAssignmentQueries,
    shadow: &ReservationShadow,
) -> Option<Entity> {
    queries
        .storage.stockpiles
        .iter()
        .filter(|(s_entity, s_transform, stock, stored)| {
            if let Some(area) = task_area_opt {
                if !area.contains(s_transform.translation.truncate()) {
                    return false;
                }
            }
            let is_tank = stock.resource_type == Some(ResourceType::Water);
            let current_water = stored.map(|s| s.len()).unwrap_or(0);
            let reserved_tank = queries.reservation.resource_cache.get_destination_reservation(*s_entity)
                + shadow.destination_reserved(*s_entity);
            let has_capacity = (current_water + reserved_tank) < stock.capacity;

            let bucket_owner = queries.designation.belongs.get(task_entity).ok().map(|b| b.0);
            let is_my_tank = bucket_owner == Some(*s_entity);

            is_tank && has_capacity && is_my_tank
        })
        .min_by(|(_, t1, _, _), (_, t2, _, _)| {
            let d1 = t1.translation.truncate().distance_squared(task_pos);
            let d2 = t2.translation.truncate().distance_squared(task_pos);
            d1.partial_cmp(&d2).unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(e, _, _, _)| e)
}
