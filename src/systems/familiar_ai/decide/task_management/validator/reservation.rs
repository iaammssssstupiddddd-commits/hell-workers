use crate::systems::familiar_ai::decide::task_management::ReservationShadow;
use bevy::prelude::*;

pub fn can_reserve_source(
    task_entity: Entity,
    queries: &crate::systems::soul_ai::execute::task_execution::context::TaskAssignmentQueries,
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
    queries: &crate::systems::soul_ai::execute::task_execution::context::TaskAssignmentQueries,
    shadow: &ReservationShadow,
) -> bool {
    queries
        .reservation
        .resource_cache
        .get_source_reservation(task_entity)
        + shadow.source_reserved(task_entity)
        == 0
}
