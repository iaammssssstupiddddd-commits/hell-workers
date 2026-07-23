//! Manual haul の選定ロジック。
//!
//! `DesignationTargetQuery` のような Bevy Query には依存しない。
//! root adapter が Query から `StockpileCandidateView` / `ExistingHaulRequestView` を
//! 組み立ててからここに渡す。

use bevy::math::Vec2;
use bevy::prelude::Entity;

use crate::stockpile_policy::{
    StockpilePolicyInput, StockpileTransferPhase, evaluate_stockpile_policy,
    stockpile_owner_accepts_item,
};
use crate::types::ResourceType;
use crate::zone::StockpilePolicy;

// ---------------------------------------------------------------------------
// View models
// ---------------------------------------------------------------------------

/// Stockpile 候補の軽量 view model。
pub struct StockpileCandidateView {
    pub entity: Entity,
    pub pos: Vec2,
    pub owner: Option<Entity>,
    pub resource_type: Option<ResourceType>,
    pub capacity: usize,
    pub current_stored: usize,
    pub is_bucket_storage: bool,
    pub policy: Option<StockpilePolicy>,
    pub incoming_reserved: usize,
    pub incoming_reserved_other_resource: usize,
    pub cycle_reserved: usize,
    pub cycle_reserved_other_resource: usize,
}

/// 既存の manual haul request の軽量 view model。
pub struct ExistingHaulRequestView {
    pub entity: Entity,
    pub fixed_source: Entity,
}

// ---------------------------------------------------------------------------
// Selector functions
// ---------------------------------------------------------------------------

/// 運搬先 Stockpile の anchor entity を選定する。
///
/// Policy-managed cell は `NewInbound` evaluator を通過する候補だけを選ぶ。
/// Policy を持たない特殊 storage は既存の bucket 専用規則を維持する。
pub fn select_stockpile_anchor(
    source_pos: Vec2,
    resource_type: ResourceType,
    item_owner: Option<Entity>,
    candidates: impl Iterator<Item = StockpileCandidateView>,
) -> Option<Entity> {
    let is_bucket = matches!(
        resource_type,
        ResourceType::BucketEmpty | ResourceType::BucketWater
    );

    let mut best_eligible: Option<(Entity, f32, Vec2)> = None;
    let mut best_legacy_any: Option<(Entity, f32, Vec2)> = None;

    for c in candidates {
        let owner_compatible = if c.is_bucket_storage {
            c.owner == item_owner
        } else {
            stockpile_owner_accepts_item(item_owner, c.owner)
        };
        if !owner_compatible {
            continue;
        }

        if c.is_bucket_storage && !is_bucket {
            continue;
        }

        let dist_sq = c.pos.distance_squared(source_pos);

        if let Some(policy) = c.policy {
            let evaluation = evaluate_stockpile_policy(StockpilePolicyInput {
                phase: StockpileTransferPhase::NewInbound,
                policy,
                capacity: c.capacity,
                stored_amount: c.current_stored,
                stored_resource: c.resource_type,
                transfer_resource: resource_type,
                requested_amount: 1,
                incoming_reserved: c.incoming_reserved,
                incoming_reserved_other_resource: c.incoming_reserved_other_resource,
                cycle_reserved: c.cycle_reserved,
                cycle_reserved_other_resource: c.cycle_reserved_other_resource,
            });
            if evaluation.allowed_amount == 0 {
                continue;
            }
            update_nearest(&mut best_eligible, c.entity, dist_sq, c.pos);
            continue;
        }

        let is_dedicated = c.owner.is_some();
        let type_match = if is_dedicated && is_bucket {
            true
        } else {
            c.resource_type.is_none() || c.resource_type == Some(resource_type)
        };
        if !type_match {
            continue;
        }

        update_nearest(&mut best_legacy_any, c.entity, dist_sq, c.pos);
        let reserved = c.incoming_reserved.saturating_add(c.cycle_reserved);
        if c.current_stored.saturating_add(reserved) < c.capacity {
            update_nearest(&mut best_eligible, c.entity, dist_sq, c.pos);
        }
    }

    best_eligible
        .or(best_legacy_any)
        .map(|(entity, _, _)| entity)
}

fn update_nearest(best: &mut Option<(Entity, f32, Vec2)>, entity: Entity, dist_sq: f32, pos: Vec2) {
    let replace = best
        .as_ref()
        .is_none_or(|(best_entity, best_dist, best_pos)| {
            dist_sq
                .total_cmp(best_dist)
                .then_with(|| pos.x.total_cmp(&best_pos.x))
                .then_with(|| pos.y.total_cmp(&best_pos.y))
                .then_with(|| entity.index_u32().cmp(&best_entity.index_u32()))
                .then_with(|| {
                    entity
                        .generation()
                        .to_bits()
                        .cmp(&best_entity.generation().to_bits())
                })
                .is_lt()
        });
    if replace {
        *best = Some((entity, dist_sq, pos));
    }
}

/// `source_entity` を固定ソースとして持つ既存の manual haul request を探す。
pub fn find_existing_request(
    source_entity: Entity,
    mut requests: impl Iterator<Item = ExistingHaulRequestView>,
) -> Option<Entity> {
    requests
        .find(|r| r.fixed_source == source_entity)
        .map(|r| r.entity)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport_request::TransportPriority;
    use crate::zone::StockpileAcceptance;

    fn entity(index: u32) -> Entity {
        Entity::from_raw_u32(index).expect("valid entity")
    }

    fn managed_candidate(index: u32, pos: Vec2) -> StockpileCandidateView {
        StockpileCandidateView {
            entity: entity(index),
            pos,
            owner: Some(entity(100)),
            resource_type: None,
            capacity: 2,
            current_stored: 0,
            is_bucket_storage: false,
            policy: Some(StockpilePolicy {
                acceptance: StockpileAcceptance::Any,
                inbound_priority: TransportPriority::Normal,
                target_amount: 2,
                allow_export: true,
            }),
            incoming_reserved: 0,
            incoming_reserved_other_resource: 0,
            cycle_reserved: 0,
            cycle_reserved_other_resource: 0,
        }
    }

    #[test]
    fn managed_cell_never_falls_back_when_target_is_reached() {
        let mut cell = managed_candidate(1, Vec2::ZERO);
        cell.current_stored = 1;
        cell.policy.as_mut().unwrap().target_amount = 1;

        assert_eq!(
            select_stockpile_anchor(
                Vec2::ZERO,
                ResourceType::Wood,
                Some(entity(100)),
                [cell].into_iter(),
            ),
            None
        );
    }

    #[test]
    fn other_resource_reservation_blocks_empty_any_cell() {
        let mut cell = managed_candidate(1, Vec2::ZERO);
        cell.incoming_reserved = 1;
        cell.incoming_reserved_other_resource = 1;

        assert_eq!(
            select_stockpile_anchor(
                Vec2::ZERO,
                ResourceType::Rock,
                Some(entity(100)),
                [cell].into_iter(),
            ),
            None
        );
    }

    #[test]
    fn unowned_item_can_target_owned_managed_cell_but_other_owner_cannot() {
        let accepted = managed_candidate(1, Vec2::ZERO);
        assert_eq!(
            select_stockpile_anchor(Vec2::ZERO, ResourceType::Wood, None, [accepted].into_iter(),),
            Some(entity(1))
        );

        let rejected = managed_candidate(1, Vec2::ZERO);
        assert_eq!(
            select_stockpile_anchor(
                Vec2::ZERO,
                ResourceType::Wood,
                Some(entity(101)),
                [rejected].into_iter(),
            ),
            None
        );
    }

    #[test]
    fn deterministic_tie_uses_position_then_entity() {
        let left = managed_candidate(2, Vec2::new(-1.0, 0.0));
        let right = managed_candidate(1, Vec2::new(1.0, 0.0));

        assert_eq!(
            select_stockpile_anchor(
                Vec2::ZERO,
                ResourceType::Wood,
                Some(entity(100)),
                [right, left].into_iter(),
            ),
            Some(entity(2))
        );
    }
}
