use bevy::prelude::*;
use hw_core::area::TaskArea;
use hw_core::logistics::ResourceType;
use hw_logistics::water::tank_can_accept_new_bucket;

use super::finder::{find_nearest_bucket_for_tank, find_nearest_water_bucket_for_tank};
use crate::familiar_ai::decide::task_management::{
    FamiliarTaskAssignmentQueries, ReservationShadow,
};

pub fn resolve_gather_water_inputs(
    task_entity: Entity,
    task_pos: Vec2,
    _task_area_opt: Option<&TaskArea>,
    queries: &FamiliarTaskAssignmentQueries,
    shadow: &ReservationShadow,
) -> Option<(Entity, Entity)> {
    use hw_logistics::transport_request::TransportRequestKind;
    let req = queries.transport_requests.get(task_entity).ok()?;
    if req.kind != TransportRequestKind::GatherWaterToTank {
        return None;
    }

    let tank_entity = req.anchor;
    let Ok((_, _, tank_stock, stored_opt)) = queries.storage.stockpiles.get(tank_entity) else {
        return None;
    };
    if tank_stock.resource_type != Some(ResourceType::Water) {
        return None;
    }
    let current_water = stored_opt.map(|s| s.len()).unwrap_or(0);
    let incoming_buckets = queries
        .reservation
        .incoming_deliveries_query
        .get(tank_entity)
        .map(|(_, inc)| inc.len())
        .unwrap_or(0);
    if !tank_can_accept_new_bucket(current_water, incoming_buckets, tank_stock.capacity) {
        return None;
    }

    let (bucket_entity, _) = find_nearest_bucket_for_tank(tank_entity, task_pos, queries, shadow)?;
    Some((bucket_entity, tank_entity))
}

pub fn resolve_haul_water_to_mixer_inputs(
    task_entity: Entity,
    task_pos: Vec2,
    task_area_opt: Option<&TaskArea>,
    queries: &FamiliarTaskAssignmentQueries,
    shadow: &ReservationShadow,
) -> Option<(Entity, Entity, Entity)> {
    use hw_logistics::transport_request::TransportRequestKind;
    let req = queries.transport_requests.get(task_entity).ok()?;
    if req.kind != TransportRequestKind::DeliverWaterToMixer {
        return None;
    }
    let mixer_entity = req.anchor;

    find_tank_bucket_for_water_mixer(mixer_entity, task_pos, task_area_opt, queries, shadow)
        .map(|(tank, bucket)| (mixer_entity, tank, bucket))
}

fn find_tank_bucket_for_water_mixer(
    _mixer_entity: Entity,
    mixer_pos: Vec2,
    task_area_opt: Option<&TaskArea>,
    queries: &FamiliarTaskAssignmentQueries,
    shadow: &ReservationShadow,
) -> Option<(Entity, Entity)> {
    let yard_opt = task_area_opt
        .and_then(|area| {
            let center = area.center();
            queries.yards.iter().find(|yard| yard.contains(center))
        })
        .or_else(|| queries.yards.iter().next());

    let mut tank_candidates: Vec<(Entity, f32)> = queries
        .storage
        .stockpiles
        .iter()
        .filter(|(_s_entity, s_transform, stock, _stored)| {
            if stock.resource_type != Some(ResourceType::Water) {
                return false;
            }
            let tank_pos = s_transform.translation.truncate();
            let in_task_area = task_area_opt.is_some_and(|area| area.contains(tank_pos));
            let in_yard = yard_opt.is_some_and(|yard| yard.contains(tank_pos));
            if (task_area_opt.is_some() || yard_opt.is_some()) && !in_task_area && !in_yard {
                return false;
            }
            true
        })
        .map(|(e, t, _, _)| {
            let d = t.translation.truncate().distance_squared(mixer_pos);
            (e, d)
        })
        .collect();
    tank_candidates.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

    for (tank_entity, _) in tank_candidates {
        if let Some((bucket_entity, _)) =
            find_nearest_water_bucket_for_tank(tank_entity, mixer_pos, queries, shadow)
        {
            return Some((tank_entity, bucket_entity));
        }

        let has_stored_water = queries
            .storage
            .stockpiles
            .get(tank_entity)
            .ok()
            .is_some_and(|(_, _, stock, stored)| {
                stock.resource_type == Some(ResourceType::Water)
                    && stored.map(|s| s.len()).unwrap_or(0) > 0
            });
        if !has_stored_water {
            continue;
        }

        if let Some((bucket_entity, _)) =
            find_nearest_bucket_for_tank(tank_entity, mixer_pos, queries, shadow)
        {
            return Some((tank_entity, bucket_entity));
        }
    }
    None
}
