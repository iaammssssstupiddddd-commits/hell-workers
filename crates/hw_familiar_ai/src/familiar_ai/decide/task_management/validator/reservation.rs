use bevy::prelude::*;
use hw_core::logistics::ResourceSourceKey;

use crate::familiar_ai::decide::task_management::{
    FamiliarTaskAssignmentQueries, ReservationShadow,
};

pub fn can_reserve_source(
    task_entity: Entity,
    queries: &FamiliarTaskAssignmentQueries,
    shadow: &ReservationShadow,
) -> bool {
    let current_reserved = queries
        .reservation
        .resource_cache
        .get_source_reservation(task_entity)
        + shadow.source_reserved(task_entity);

    let max_slots = if let Ok(slots) = queries.task_slots.get(task_entity) {
        slots.max as usize
    } else {
        1
    };

    current_reserved < max_slots
}

pub fn source_not_reserved(
    task_entity: Entity,
    queries: &FamiliarTaskAssignmentQueries,
    shadow: &ReservationShadow,
) -> bool {
    queries
        .reservation
        .resource_cache
        .get_source_reservation(task_entity)
        + shadow.source_reserved(task_entity)
        == 0
}

pub fn source_key_not_reserved(
    source: ResourceSourceKey,
    queries: &FamiliarTaskAssignmentQueries,
    shadow: &ReservationShadow,
) -> bool {
    queries
        .reservation
        .resource_cache
        .get_source_reservation(source)
        + shadow.source_reserved_key(source)
        == 0
}
