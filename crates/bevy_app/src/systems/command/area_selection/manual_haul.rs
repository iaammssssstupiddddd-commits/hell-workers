//! Manual Haul: pick_manual_haul_stockpile_anchor / upsert 処理

use super::queries::DesignationTargetQuery;
use crate::systems::logistics::ResourceType;
use bevy::prelude::*;
use hw_logistics::manual_haul_selector::{
    ExistingHaulRequestView, StockpileCandidateView, find_existing_request, select_stockpile_anchor,
};
use std::collections::HashMap;

pub(super) fn pick_manual_haul_stockpile_anchor(
    source_pos: Vec2,
    resource_type: ResourceType,
    item_owner: Option<Entity>,
    cycle_reservations: &HashMap<Entity, HashMap<ResourceType, usize>>,
    q_targets: &DesignationTargetQuery,
) -> Option<Entity> {
    // DesignationTargetQuery から Stockpile 候補の view model を組み立てる
    let candidates = q_targets.iter().filter_map(|row| {
        let stockpile = row.11?;
        let incoming = row.14.2;
        let incoming_reserved = incoming.map_or(0, |incoming| incoming.len());
        let incoming_matching = incoming.map_or(0, |incoming| {
            incoming
                .iter()
                .filter(|item| {
                    q_targets
                        .get(**item)
                        .ok()
                        .and_then(|item_row| item_row.4)
                        .is_some_and(|item| item.0 == resource_type)
                })
                .count()
        });
        let cycle = cycle_reservations.get(&row.0);
        let cycle_reserved = cycle.map_or(0, |by_resource| by_resource.values().sum());
        let cycle_matching = cycle
            .and_then(|by_resource| by_resource.get(&resource_type))
            .copied()
            .unwrap_or(0);
        Some(StockpileCandidateView {
            entity: row.0,
            pos: row.1.translation.truncate(),
            owner: row.8.map(|b| b.0),
            resource_type: stockpile.resource_type,
            capacity: stockpile.capacity,
            current_stored: row.12.map(|s| s.len()).unwrap_or(0),
            is_bucket_storage: row.13.is_some(),
            policy: row.14.1.copied(),
            incoming_reserved,
            incoming_reserved_other_resource: incoming_reserved.saturating_sub(incoming_matching),
            cycle_reserved,
            cycle_reserved_other_resource: cycle_reserved.saturating_sub(cycle_matching),
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
        if row.9.is_none() || row.14.0.is_none() {
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
