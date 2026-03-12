//! Manual Haul: pick_manual_haul_stockpile_anchor / upsert 処理

use super::queries::DesignationTargetQuery;
use crate::systems::logistics::ResourceType;
use bevy::prelude::*;
use hw_logistics::manual_haul_selector::{
    ExistingHaulRequestView, StockpileCandidateView, find_existing_request,
    select_stockpile_anchor,
};

pub(super) fn pick_manual_haul_stockpile_anchor(
    source_pos: Vec2,
    resource_type: ResourceType,
    item_owner: Option<Entity>,
    q_targets: &DesignationTargetQuery,
) -> Option<Entity> {
    // DesignationTargetQuery から Stockpile 候補の view model を組み立てる
    let candidates = q_targets.iter().filter_map(|row| {
        let stockpile = row.11?;
        Some(StockpileCandidateView {
            entity: row.0,
            pos: row.1.translation.truncate(),
            owner: row.8.map(|b| b.0),
            resource_type: stockpile.resource_type,
            capacity: stockpile.capacity,
            current_stored: row.12.map(|s| s.len()).unwrap_or(0),
            is_bucket_storage: row.13.is_some(),
        })
    });
    select_stockpile_anchor(source_pos, resource_type, item_owner, candidates)
}

pub(super) fn find_manual_request_for_source(
    source_entity: Entity,
    q_targets: &DesignationTargetQuery,
) -> Option<Entity> {
    // manual_opt(row.14).is_some() かつ transport_request_opt(row.9).is_some() の行のみ対象
    // fixed_source_opt(row.10) が None の行は fixed_source が存在しないため skip する
    let requests = q_targets.iter().filter_map(|row| {
        if row.9.is_none() || row.14.is_none() {
            return None;
        }
        let fixed_source = row.10?.0;
        Some(ExistingHaulRequestView {
            entity: row.0,
            fixed_source,
        })
    });
    find_existing_request(source_entity, requests)
}
