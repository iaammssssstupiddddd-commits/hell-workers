//! WheelbarrowLease の有効性検証

use bevy::prelude::*;

use crate::systems::familiar_ai::decide::task_management::{AssignTaskContext, ReservationShadow};
use crate::systems::logistics::transport_request::WheelbarrowLease;

use super::super::super::builders::issue_haul_with_wheelbarrow;

/// WheelbarrowLease の有効性を検証
///
/// - wheelbarrow がまだ利用可能（parked かつ未使用）か
/// - items のうち最低 `min_valid_items` 個が未予約の地面アイテムか
pub fn validate_lease(
    lease: &WheelbarrowLease,
    queries: &crate::systems::familiar_ai::decide::task_management::FamiliarTaskAssignmentQueries,
    shadow: &ReservationShadow,
    min_valid_items: usize,
) -> bool {
    use crate::systems::familiar_ai::decide::task_management::validator::source_not_reserved;

    if queries.wheelbarrows.get(lease.wheelbarrow).is_err() {
        return false;
    }
    if !source_not_reserved(lease.wheelbarrow, queries, shadow) {
        return false;
    }
    let valid_count = lease
        .items
        .iter()
        .filter(|item| source_not_reserved(**item, queries, shadow))
        .count();
    valid_count >= min_valid_items
}

/// 有効な lease が存在する場合、条件に合うアイテムで猫車運搬を即時発行する。
pub fn try_issue_haul_from_lease<F>(
    task_entity: Entity,
    task_pos: Vec2,
    already_commanded: bool,
    min_valid_items: usize,
    max_items: usize,
    item_filter: F,
    ctx: &AssignTaskContext<'_>,
    queries: &mut crate::systems::familiar_ai::decide::task_management::FamiliarTaskAssignmentQueries,
    shadow: &mut ReservationShadow,
) -> bool
where
    F: Fn(Entity) -> bool,
{
    let Ok(lease) = queries.wheelbarrow_leases.get(task_entity) else {
        return false;
    };

    if !validate_lease(lease, queries, shadow, min_valid_items) {
        return false;
    }

    let lease_items: Vec<Entity> = lease
        .items
        .iter()
        .copied()
        .filter(|item| item_filter(*item))
        .take(max_items)
        .collect();
    if lease_items.len() < min_valid_items {
        return false;
    }

    issue_haul_with_wheelbarrow(
        lease.wheelbarrow,
        lease.source_pos,
        lease.destination.clone(),
        lease_items,
        task_pos,
        already_commanded,
        ctx,
        queries,
        shadow,
    );
    true
}
