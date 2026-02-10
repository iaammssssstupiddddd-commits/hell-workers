use crate::constants::BUCKET_CAPACITY;
use crate::systems::command::TaskArea;
use crate::systems::logistics::transport_request::TransportRequestKind;
use crate::systems::logistics::ResourceType;
use bevy::prelude::*;

use crate::systems::familiar_ai::decide::task_management::ReservationShadow;
use super::finder::{find_best_tank_for_bucket, find_nearest_bucket_for_tank};
use super::reservation::source_not_reserved;

pub fn resolve_haul_to_stockpile_inputs(
    task_entity: Entity,
    queries: &crate::systems::soul_ai::execute::task_execution::context::TaskAssignmentQueries,
) -> Option<(Entity, ResourceType, Option<Entity>)> {
    let req = queries.transport_requests.get(task_entity).ok()?;
    if !matches!(req.kind, TransportRequestKind::DepositToStockpile) {
        return None;
    }

    let stockpile = req.anchor;
    let resource_type = req.resource_type;
    let item_owner = queries.designation.belongs.get(stockpile).ok().map(|b| b.0);
    Some((stockpile, resource_type, item_owner))
}

/// Resolves (bucket, tank) for GatherWater.
/// - Bucket-based: task_entity = bucket, tank is selected from bucket owner.
/// - Request-based: task_entity = request, tank is request.anchor, bucket is resolved lazily.
pub fn resolve_gather_water_inputs(
    task_entity: Entity,
    task_pos: Vec2,
    task_area_opt: Option<&TaskArea>,
    queries: &crate::systems::soul_ai::execute::task_execution::context::TaskAssignmentQueries,
    shadow: &ReservationShadow,
) -> Option<(Entity, Entity)> {
    if let Ok(req) = queries.transport_requests.get(task_entity) {
        if req.kind == TransportRequestKind::GatherWaterToTank {
            let tank_entity = req.anchor;
            let Ok((_, _, tank_stock, stored_opt)) = queries.storage.stockpiles.get(tank_entity) else {
                return None;
            };
            if tank_stock.resource_type != Some(ResourceType::Water) {
                return None;
            }
            let current_water = stored_opt.map(|s| s.len()).unwrap_or(0);
            let reserved_water = queries
                .reservation
                .resource_cache
                .get_destination_reservation(tank_entity)
                + shadow.destination_reserved(tank_entity);
            if (current_water + reserved_water) >= tank_stock.capacity {
                return None;
            }

            let (bucket_entity, _) =
                find_nearest_bucket_for_tank(tank_entity, task_pos, queries, shadow)?;
            return Some((bucket_entity, tank_entity));
        }
    }

    let tank_entity =
        find_best_tank_for_bucket(task_entity, task_pos, task_area_opt, queries, shadow)?;
    Some((task_entity, tank_entity))
}

/// M7: ReturnBucket request の (stockpile, tank) を解決
pub fn resolve_haul_return_bucket_inputs(
    task_entity: Entity,
    queries: &crate::systems::soul_ai::execute::task_execution::context::TaskAssignmentQueries,
) -> Option<(Entity, Entity)> {
    let req = queries.transport_requests.get(task_entity).ok()?;
    if req.kind != TransportRequestKind::ReturnBucket {
        return None;
    }
    let stockpile = req.anchor;
    let tank = queries.designation.belongs.get(stockpile).ok().map(|b| b.0)?;
    Some((stockpile, tank))
}

pub fn resolve_haul_to_blueprint_inputs(
    task_entity: Entity,
    queries: &crate::systems::soul_ai::execute::task_execution::context::TaskAssignmentQueries,
) -> Option<(Entity, ResourceType)> {
    let blueprint = queries
        .storage
        .target_blueprints
        .get(task_entity)
        .ok()
        .map(|tb| tb.0)?;

    // request タスクは resource_type を TransportRequest から取得
    if let Ok(req) = queries.transport_requests.get(task_entity) {
        if matches!(req.kind, TransportRequestKind::DeliverToBlueprint) {
            return Some((blueprint, req.resource_type));
        }
    }

    // 従来タスクはアイテム実体から取得
    let item_type = queries.items.get(task_entity).ok().map(|(it, _)| it.0)?;
    Some((blueprint, item_type))
}

pub fn resolve_haul_to_mixer_inputs(
    task_entity: Entity,
    queries: &crate::systems::soul_ai::execute::task_execution::context::TaskAssignmentQueries,
) -> Option<(Entity, ResourceType)> {
    let mixer_entity = queries
        .storage
        .target_mixers
        .get(task_entity)
        .ok()
        .map(|tm| tm.0)?;

    // requestタスクは resource_type を専用コンポーネントから取得する
    if let Ok(req) = queries.transport_requests.get(task_entity) {
        if matches!(req.kind, TransportRequestKind::DeliverToMixerSolid) {
            return Some((mixer_entity, req.resource_type));
        }
    }

    // 従来タスクはアイテム実体から取得する
    let item_type = queries.items.get(task_entity).ok().map(|(it, _)| it.0)?;
    Some((mixer_entity, item_type))
}

/// Resolves (mixer, tank, bucket) for HaulWaterToMixer.
/// - Bucket-based: task_entity = bucket, tank from BelongsTo.
/// - Request-based: task_entity = request, finds (tank, bucket) via find_tank_bucket_for_water_mixer.
pub fn resolve_haul_water_to_mixer_inputs(
    task_entity: Entity,
    task_pos: Vec2,
    task_area_opt: Option<&TaskArea>,
    queries: &crate::systems::soul_ai::execute::task_execution::context::TaskAssignmentQueries,
    shadow: &ReservationShadow,
) -> Option<(Entity, Entity, Entity)> {
    let mixer_entity = queries
        .storage
        .target_mixers
        .get(task_entity)
        .ok()
        .map(|tm| tm.0)?;

    let is_request = queries
        .transport_requests
        .get(task_entity)
        .is_ok_and(|r| r.kind == TransportRequestKind::DeliverWaterToMixer);

    if is_request {
        find_tank_bucket_for_water_mixer(mixer_entity, task_pos, task_area_opt, queries, shadow)
            .map(|(tank, bucket)| (mixer_entity, tank, bucket))
    } else {
        // Bucket-based: task_entity is the bucket
        let tank_entity = queries
            .designation
            .belongs
            .get(task_entity)
            .ok()
            .map(|b| b.0)?;
        Some((mixer_entity, tank_entity, task_entity))
    }
}

/// Finds (tank, bucket) for a DeliverWaterToMixer request at mixer_entity.
fn find_tank_bucket_for_water_mixer(
    _mixer_entity: Entity,
    mixer_pos: Vec2,
    task_area_opt: Option<&TaskArea>,
    queries: &crate::systems::soul_ai::execute::task_execution::context::TaskAssignmentQueries,
    shadow: &ReservationShadow,
) -> Option<(Entity, Entity)> {
    let mut tank_candidates: Vec<(Entity, f32)> = queries
        .storage
        .stockpiles
        .iter()
        .filter(|(_s_entity, s_transform, stock, stored)| {
            if stock.resource_type != Some(ResourceType::Water) {
                return false;
            }
            if let Some(area) = task_area_opt {
                if !area.contains(s_transform.translation.truncate()) {
                    return false;
                }
            }
            let water_count = stored.map(|s| s.len()).unwrap_or(0);
            water_count >= BUCKET_CAPACITY as usize
        })
        .map(|(e, t, _, _)| {
            let d = t.translation.truncate().distance_squared(mixer_pos);
            (e, d)
        })
        .collect();
    tank_candidates.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

    for (tank_entity, _) in tank_candidates {
        let bucket_opt = queries
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
                let d1 = t1.translation.truncate().distance_squared(mixer_pos);
                let d2 = t2.translation.truncate().distance_squared(mixer_pos);
                d1.partial_cmp(&d2).unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(e, _, _, _)| e);

        if let Some(bucket_entity) = bucket_opt {
            return Some((tank_entity, bucket_entity));
        }
    }
    None
}
