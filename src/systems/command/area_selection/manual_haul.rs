//! Manual Haul: pick_manual_haul_stockpile_anchor / upsert 処理

use super::queries::DesignationTargetQuery;
use crate::systems::logistics::ResourceType;
use bevy::prelude::*;

pub(super) fn pick_manual_haul_stockpile_anchor(
    source_pos: Vec2,
    resource_type: ResourceType,
    item_owner: Option<Entity>,
    q_targets: &DesignationTargetQuery,
) -> Option<Entity> {
    let is_bucket = matches!(
        resource_type,
        ResourceType::BucketEmpty | ResourceType::BucketWater
    );

    let mut best_with_capacity: Option<(Entity, f32)> = None;
    let mut best_any_capacity: Option<(Entity, f32)> = None;

    for (
        stock_entity,
        stock_transform,
        _,
        _,
        _,
        _,
        _,
        _,
        stock_owner_opt,
        _,
        _,
        stockpile_opt,
        stored_opt,
        bucket_opt,
        _,
    ) in q_targets.iter()
    {
        let Some(stockpile) = stockpile_opt else {
            continue;
        };
        let stock_owner = stock_owner_opt.map(|belongs| belongs.0);
        if stock_owner != item_owner {
            continue;
        }

        let is_bucket_storage = bucket_opt.is_some();
        if is_bucket_storage && !is_bucket {
            continue;
        }

        let is_dedicated = stock_owner.is_some();
        let type_match = if is_dedicated && is_bucket {
            true
        } else {
            stockpile.resource_type.is_none() || stockpile.resource_type == Some(resource_type)
        };
        if !type_match {
            continue;
        }

        let dist_sq = stock_transform
            .translation
            .truncate()
            .distance_squared(source_pos);
        match best_any_capacity {
            Some((_, best_dist_sq)) if best_dist_sq <= dist_sq => {}
            _ => best_any_capacity = Some((stock_entity, dist_sq)),
        }

        let current = stored_opt.map(|stored| stored.len()).unwrap_or(0);
        if current >= stockpile.capacity {
            continue;
        }
        match best_with_capacity {
            Some((_, best_dist_sq)) if best_dist_sq <= dist_sq => {}
            _ => best_with_capacity = Some((stock_entity, dist_sq)),
        }
    }

    best_with_capacity
        .or(best_any_capacity)
        .map(|(entity, _)| entity)
}

pub(super) fn find_manual_request_for_source(
    source_entity: Entity,
    q_targets: &DesignationTargetQuery,
) -> Option<Entity> {
    q_targets.iter().find_map(
        |(
            request_entity,
            _,
            _,
            _,
            _,
            _,
            _,
            _,
            _,
            transport_request_opt,
            fixed_source_opt,
            _,
            _,
            _,
            manual_opt,
        )| {
            (manual_opt.is_some()
                && transport_request_opt.is_some()
                && fixed_source_opt.map(|source| source.0) == Some(source_entity))
            .then_some(request_entity)
        },
    )
}
