use bevy::prelude::*;
use hw_logistics::transport_request::WheelbarrowLease;

use super::super::super::builders::{WheelbarrowHaulSpec, issue_haul_with_wheelbarrow};
use crate::familiar_ai::decide::task_management::validator::source_not_reserved;
use crate::familiar_ai::decide::task_management::{
    AssignTaskContext, FamiliarTaskAssignmentQueries, ReservationShadow,
};

pub fn validate_lease(
    lease: &WheelbarrowLease,
    queries: &FamiliarTaskAssignmentQueries,
    shadow: &ReservationShadow,
    min_valid_items: usize,
) -> bool {
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

/// `try_issue_haul_from_lease` の設定パラメータをまとめた構造体。
pub struct HaulFromLeaseSpec {
    pub task_entity: Entity,
    pub task_pos: Vec2,
    pub already_commanded: bool,
    pub min_valid_items: usize,
    pub max_items: usize,
}

pub fn try_issue_haul_from_lease<F>(
    spec: HaulFromLeaseSpec,
    item_filter: F,
    ctx: &AssignTaskContext<'_>,
    queries: &mut FamiliarTaskAssignmentQueries,
    shadow: &mut ReservationShadow,
) -> bool
where
    F: Fn(Entity) -> bool,
{
    let Ok(lease) = queries.wheelbarrow_leases.get(spec.task_entity) else {
        return false;
    };

    if !validate_lease(lease, queries, shadow, spec.min_valid_items) {
        return false;
    }

    let lease_items: Vec<Entity> = lease
        .items
        .iter()
        .copied()
        .filter(|item| item_filter(*item))
        .take(spec.max_items)
        .collect();
    if lease_items.len() < spec.min_valid_items {
        return false;
    }

    issue_haul_with_wheelbarrow(
        WheelbarrowHaulSpec {
            wheelbarrow: lease.wheelbarrow,
            source_pos: lease.source_pos,
            destination: lease.destination,
            items: lease_items,
        },
        spec.task_pos,
        spec.already_commanded,
        ctx,
        queries,
        shadow,
    );
    true
}
