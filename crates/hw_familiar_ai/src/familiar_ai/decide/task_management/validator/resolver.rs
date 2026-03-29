use bevy::prelude::*;
use hw_core::logistics::ResourceType;
use hw_logistics::transport_request::TransportRequestKind;

use super::capacity_helpers::check_stockpile_capacity;
use crate::familiar_ai::decide::task_management::{
    FamiliarTaskAssignmentQueries, ReservationShadow,
};

pub use super::water_resolver::{resolve_gather_water_inputs, resolve_haul_water_to_mixer_inputs};

pub fn resolve_consolidation_inputs(
    task_entity: Entity,
    queries: &FamiliarTaskAssignmentQueries,
) -> Option<(Entity, ResourceType, Vec<Entity>)> {
    let req = queries.transport_requests.get(task_entity).ok()?;
    if req.kind != TransportRequestKind::ConsolidateStockpile {
        return None;
    }

    let receiver = req.anchor;
    let resource_type = req.resource_type;
    let donor_cells = req.stockpile_group.clone();

    let (_, _, stock, stored_opt) = queries.storage.stockpiles.get(receiver).ok()?;
    let stored = stored_opt.map(|s| s.len()).unwrap_or(0);
    if stored >= stock.capacity {
        return None;
    }

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
    queries: &FamiliarTaskAssignmentQueries,
    shadow: &ReservationShadow,
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

    let stockpile = if req.stockpile_group.is_empty() {
        check_stockpile_capacity(req.anchor, resource_type, queries, shadow).map(|_| req.anchor)?
    } else {
        req.stockpile_group
            .iter()
            .filter_map(|&cell| {
                let free = check_stockpile_capacity(cell, resource_type, queries, shadow)?;
                Some((cell, free))
            })
            .min_by_key(|(_, free)| *free)
            .map(|(cell, _)| cell)?
    };

    Some((stockpile, resource_type, item_owner, fixed_source))
}

pub fn resolve_return_bucket_tank(
    task_entity: Entity,
    queries: &FamiliarTaskAssignmentQueries,
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

pub fn resolve_return_wheelbarrow(
    task_entity: Entity,
    queries: &FamiliarTaskAssignmentQueries,
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
    queries: &FamiliarTaskAssignmentQueries,
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
    queries: &FamiliarTaskAssignmentQueries,
) -> Option<(Entity, ResourceType)> {
    let req = queries.transport_requests.get(task_entity).ok()?;
    if !matches!(req.kind, TransportRequestKind::DeliverToFloorConstruction) {
        return None;
    }
    Some((req.anchor, req.resource_type))
}

pub fn resolve_haul_to_wall_construction_inputs(
    task_entity: Entity,
    queries: &FamiliarTaskAssignmentQueries,
) -> Option<(Entity, ResourceType)> {
    let req = queries.transport_requests.get(task_entity).ok()?;
    if !matches!(req.kind, TransportRequestKind::DeliverToWallConstruction) {
        return None;
    }
    Some((req.anchor, req.resource_type))
}

pub fn resolve_haul_to_provisional_wall_inputs(
    task_entity: Entity,
    queries: &FamiliarTaskAssignmentQueries,
) -> Option<(Entity, ResourceType)> {
    let req = queries.transport_requests.get(task_entity).ok()?;
    if !matches!(req.kind, TransportRequestKind::DeliverToProvisionalWall) {
        return None;
    }
    Some((req.anchor, req.resource_type))
}

pub fn resolve_haul_to_mixer_inputs(
    task_entity: Entity,
    queries: &FamiliarTaskAssignmentQueries,
) -> Option<(Entity, ResourceType)> {
    let req = queries.transport_requests.get(task_entity).ok()?;
    if !matches!(req.kind, TransportRequestKind::DeliverToMixerSolid) {
        return None;
    }
    let mixer_entity = req.anchor;
    Some((mixer_entity, req.resource_type))
}

pub fn resolve_haul_to_soul_spa_inputs(
    task_entity: Entity,
    queries: &FamiliarTaskAssignmentQueries,
) -> Option<(Entity, ResourceType)> {
    let req = queries.transport_requests.get(task_entity).ok()?;
    if !matches!(req.kind, TransportRequestKind::DeliverToSoulSpa) {
        return None;
    }
    Some((req.anchor, req.resource_type))
}
