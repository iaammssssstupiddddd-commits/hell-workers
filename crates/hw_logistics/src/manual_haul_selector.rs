//! Manual haul の選定ロジック。
//!
//! `DesignationTargetQuery` のような Bevy Query には依存しない。
//! root adapter が Query から `StockpileCandidateView` / `ExistingHaulRequestView` を
//! 組み立ててからここに渡す。

use bevy::prelude::Entity;
use bevy::math::Vec2;

use crate::types::ResourceType;

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
/// capacity に空きがある中で最も近い Stockpile を優先し、
/// 空きがなければ最も近い Stockpile にフォールバックする。
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

    let mut best_with_capacity: Option<(Entity, f32)> = None;
    let mut best_any_capacity: Option<(Entity, f32)> = None;

    for c in candidates {
        if c.owner != item_owner {
            continue;
        }

        if c.is_bucket_storage && !is_bucket {
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

        let dist_sq = c.pos.distance_squared(source_pos);

        match best_any_capacity {
            Some((_, best_dist_sq)) if best_dist_sq <= dist_sq => {}
            _ => best_any_capacity = Some((c.entity, dist_sq)),
        }

        if c.current_stored >= c.capacity {
            continue;
        }
        match best_with_capacity {
            Some((_, best_dist_sq)) if best_dist_sq <= dist_sq => {}
            _ => best_with_capacity = Some((c.entity, dist_sq)),
        }
    }

    best_with_capacity
        .or(best_any_capacity)
        .map(|(entity, _)| entity)
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
