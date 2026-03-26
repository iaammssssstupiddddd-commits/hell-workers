use bevy::prelude::*;
use hw_core::logistics::ResourceType;

use crate::familiar_ai::decide::task_management::{
    FamiliarTaskAssignmentQueries, ReservationShadow,
};

pub(super) fn stored_items_opt_to_count(
    opt: Option<&hw_core::relationships::StoredItems>,
) -> usize {
    opt.map(|s| s.len()).unwrap_or(0)
}

/// ストックパイルのキャパシティ・タイプ検証。空きがあればその数を返す。
pub(super) fn check_stockpile_capacity(
    cell: Entity,
    resource_type: ResourceType,
    queries: &FamiliarTaskAssignmentQueries,
    shadow: &ReservationShadow,
) -> Option<usize> {
    let (_, _, stock, stored_opt) = queries.storage.stockpiles.get(cell).ok()?;
    let stored = stored_items_opt_to_count(stored_opt);
    let incoming = queries
        .reservation
        .incoming_deliveries_query
        .get(cell)
        .ok()
        .map(|(_, inc)| inc.len())
        .unwrap_or(0);
    let shadow_incoming = shadow.destination_reserved_total(cell);
    let effective_free = stock
        .capacity
        .saturating_sub(stored + incoming + shadow_incoming);
    let type_ok = stock.resource_type.is_none() || stock.resource_type == Some(resource_type);
    if effective_free > 0 && type_ok {
        Some(effective_free)
    } else {
        None
    }
}
