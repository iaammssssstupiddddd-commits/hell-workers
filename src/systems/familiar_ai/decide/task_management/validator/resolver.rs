use crate::constants::BUCKET_CAPACITY;
use crate::systems::command::TaskArea;
use crate::systems::logistics::ResourceType;
use crate::systems::logistics::transport_request::TransportRequestKind;
use bevy::prelude::*;

use super::finder::find_nearest_bucket_for_tank;
use super::reservation::source_not_reserved;
use crate::systems::familiar_ai::decide::task_management::ReservationShadow;

/// ConsolidateStockpile request の入力を解決する。
/// 返り値: (receiver_cell, resource_type, donor_cells)
pub fn resolve_consolidation_inputs(
    task_entity: Entity,
    queries: &crate::systems::soul_ai::execute::task_execution::context::TaskAssignmentQueries,
) -> Option<(Entity, ResourceType, Vec<Entity>)> {
    let req = queries.transport_requests.get(task_entity).ok()?;
    if req.kind != TransportRequestKind::ConsolidateStockpile {
        return None;
    }

    let receiver = req.anchor;
    let resource_type = req.resource_type;
    let donor_cells = req.stockpile_group.clone();

    // レシーバーの空き容量を確認
    let (_, _, stock, stored_opt) = queries.storage.stockpiles.get(receiver).ok()?;
    let stored = stored_opt.map(|s| s.len()).unwrap_or(0);
    if stored >= stock.capacity {
        return None;
    }

    // 型互換チェック
    let type_ok = stock.resource_type.is_none() || stock.resource_type == Some(resource_type);
    if !type_ok {
        return None;
    }

    if donor_cells.is_empty() {
        return None;
    }

    Some((receiver, resource_type, donor_cells))
}

pub fn resolve_haul_to_stockpile_inputs(
    task_entity: Entity,
    queries: &crate::systems::soul_ai::execute::task_execution::context::TaskAssignmentQueries,
    _shadow: &crate::systems::familiar_ai::decide::task_management::ReservationShadow,
) -> Option<(Entity, ResourceType, Option<Entity>, Option<Entity>)> {
    let req = queries.transport_requests.get(task_entity).ok()?;
    if !matches!(req.kind, TransportRequestKind::DepositToStockpile) {
        return None;
    }

    let resource_type = req.resource_type;
    let item_owner = queries
        .designation
        .belongs
        .get(req.anchor)
        .ok()
        .map(|b| b.0);
    let fixed_source = queries
        .transport_request_fixed_sources
        .get(task_entity)
        .ok()
        .map(|source| source.0);

    // グループ内の受け入れ可能な空きセルを探す
    let stockpile = if req.stockpile_group.is_empty() {
        let (_, _, stock, stored_opt) = queries.storage.stockpiles.get(req.anchor).ok()?;
        let stored = stored_items_opt_to_count(stored_opt);

        // インフライト（リレーションによる既知の搬入予定）+ 影（当フレーム内での新規予約）
        let incoming = queries
            .reservation
            .incoming_deliveries_query
            .get(req.anchor)
            .ok()
            .map(|inc: &crate::relationships::IncomingDeliveries| inc.len())
            .unwrap_or(0);
        let effective_free = stock.capacity.saturating_sub(stored + incoming);

        let has_capacity = effective_free > 0;
        let type_ok = stock.resource_type.is_none() || stock.resource_type == Some(resource_type);
        if has_capacity && type_ok {
            req.anchor
        } else {
            return None;
        }
    } else {
        req.stockpile_group
            .iter()
            .filter_map(|&cell| {
                let (_, _, stock, stored_opt) = queries.storage.stockpiles.get(cell).ok()?;
                let stored = stored_items_opt_to_count(stored_opt);

                let incoming = queries
                    .reservation
                    .incoming_deliveries_query
                    .get(cell)
                    .ok()
                    .map(|inc: &crate::relationships::IncomingDeliveries| inc.len())
                    .unwrap_or(0);
                let effective_free = stock.capacity.saturating_sub(stored + incoming);

                let type_ok =
                    stock.resource_type.is_none() || stock.resource_type == Some(resource_type);
                if effective_free > 0 && type_ok {
                    Some((cell, effective_free))
                } else {
                    None
                }
            })
            // 貪欲法: 空きが最小（＝もう少しで満杯）のセルを優先して埋める
            .min_by_key(|(_, free)| *free)
            .map(|(cell, _)| cell)?
    };

    Some((stockpile, resource_type, item_owner, fixed_source))
}

fn stored_items_opt_to_count(opt: Option<&crate::relationships::StoredItems>) -> usize {
    opt.map(|s| s.len()).unwrap_or(0)
}

/// Resolves (bucket, tank) for GatherWater request.
pub fn resolve_gather_water_inputs(
    task_entity: Entity,
    task_pos: Vec2,
    _task_area_opt: Option<&TaskArea>,
    queries: &crate::systems::soul_ai::execute::task_execution::context::TaskAssignmentQueries,
    shadow: &ReservationShadow,
) -> Option<(Entity, Entity)> {
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
    let reserved_water = queries
        .reservation
        .incoming_deliveries_query
        .get(tank_entity)
        .map(|inc| inc.len())
        .unwrap_or(0);
    if (current_water + reserved_water) >= tank_stock.capacity {
        return None;
    }

    let (bucket_entity, _) = find_nearest_bucket_for_tank(tank_entity, task_pos, queries, shadow)?;
    Some((bucket_entity, tank_entity))
}

/// ReturnBucket request の tank anchor を解決する。
pub fn resolve_return_bucket_tank(
    task_entity: Entity,
    queries: &crate::systems::soul_ai::execute::task_execution::context::TaskAssignmentQueries,
) -> Option<Entity> {
    let req = queries.transport_requests.get(task_entity).ok()?;
    if req.kind != TransportRequestKind::ReturnBucket {
        return None;
    }
    let tank = req.anchor;
    let (_, _, stockpile, _) = queries.storage.stockpiles.get(tank).ok()?;
    if stockpile.resource_type != Some(ResourceType::Water) {
        return None;
    }
    Some(tank)
}

/// ReturnWheelbarrow request の対象を解決する。
/// 返り値: (wheelbarrow, parking_anchor, wheelbarrow_pos)
pub fn resolve_return_wheelbarrow(
    task_entity: Entity,
    queries: &crate::systems::soul_ai::execute::task_execution::context::TaskAssignmentQueries,
) -> Option<(Entity, Entity, Vec2)> {
    let req = queries.transport_requests.get(task_entity).ok()?;
    if req.kind != TransportRequestKind::ReturnWheelbarrow {
        return None;
    }

    let wheelbarrow = req.anchor;
    let parking_anchor = queries.designation.belongs.get(wheelbarrow).ok()?.0;
    let (_, wheelbarrow_transform) = queries.wheelbarrows.get(wheelbarrow).ok()?;

    Some((
        wheelbarrow,
        parking_anchor,
        wheelbarrow_transform.translation.truncate(),
    ))
}

pub fn resolve_haul_to_blueprint_inputs(
    task_entity: Entity,
    queries: &crate::systems::soul_ai::execute::task_execution::context::TaskAssignmentQueries,
) -> Option<(Entity, ResourceType)> {
    let req = queries.transport_requests.get(task_entity).ok()?;
    if !matches!(req.kind, TransportRequestKind::DeliverToBlueprint) {
        return None;
    }
    let blueprint = req.anchor;

    Some((blueprint, req.resource_type))
}

pub fn resolve_haul_to_floor_construction_inputs(
    task_entity: Entity,
    queries: &crate::systems::soul_ai::execute::task_execution::context::TaskAssignmentQueries,
) -> Option<(Entity, ResourceType)> {
    let req = queries.transport_requests.get(task_entity).ok()?;
    if !matches!(req.kind, TransportRequestKind::DeliverToFloorConstruction) {
        return None;
    }

    Some((req.anchor, req.resource_type))
}

pub fn resolve_haul_to_mixer_inputs(
    task_entity: Entity,
    queries: &crate::systems::soul_ai::execute::task_execution::context::TaskAssignmentQueries,
) -> Option<(Entity, ResourceType)> {
    let req = queries.transport_requests.get(task_entity).ok()?;
    if !matches!(req.kind, TransportRequestKind::DeliverToMixerSolid) {
        return None;
    }
    let mixer_entity = req.anchor;

    Some((mixer_entity, req.resource_type))
}

/// Resolves (mixer, tank, bucket) for HaulWaterToMixer request.
pub fn resolve_haul_water_to_mixer_inputs(
    task_entity: Entity,
    task_pos: Vec2,
    task_area_opt: Option<&TaskArea>,
    queries: &crate::systems::soul_ai::execute::task_execution::context::TaskAssignmentQueries,
    shadow: &ReservationShadow,
) -> Option<(Entity, Entity, Entity)> {
    let req = queries.transport_requests.get(task_entity).ok()?;
    if req.kind != TransportRequestKind::DeliverWaterToMixer {
        return None;
    }
    let mixer_entity = req.anchor;

    find_tank_bucket_for_water_mixer(mixer_entity, task_pos, task_area_opt, queries, shadow)
        .map(|(tank, bucket)| (mixer_entity, tank, bucket))
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
