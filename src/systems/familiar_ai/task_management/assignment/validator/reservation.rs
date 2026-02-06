use crate::systems::familiar_ai::task_management::ReservationShadow;
use crate::systems::logistics::ResourceType;
use bevy::prelude::*;

pub fn can_reserve_source(
    task_entity: Entity,
    queries: &crate::systems::soul_ai::task_execution::context::TaskAssignmentQueries,
    shadow: &ReservationShadow,
) -> bool {
    let current_reserved =
        queries.reservation.resource_cache.get_source_reservation(task_entity) + shadow.source_reserved(task_entity);

    let max_slots = if let Ok(slots) = queries.task_slots.get(task_entity) {
        slots.max as usize
    } else {
        1
    };

    current_reserved < max_slots
}

pub fn source_not_reserved(
    task_entity: Entity,
    queries: &crate::systems::soul_ai::task_execution::context::TaskAssignmentQueries,
    shadow: &ReservationShadow,
) -> bool {
    queries.reservation.resource_cache.get_source_reservation(task_entity) + shadow.source_reserved(task_entity) == 0
}

pub fn can_accept_mixer_item(
    mixer_entity: Entity,
    item_type: ResourceType,
    queries: &crate::systems::soul_ai::task_execution::context::TaskAssignmentQueries,
    shadow: &ReservationShadow,
) -> bool {
    if let Ok((_, storage, _)) = queries.storage.mixers.get(mixer_entity) {
        let reserved = queries
            .reservation.resource_cache
            .get_mixer_destination_reservation(mixer_entity, item_type)
            + shadow.mixer_reserved(mixer_entity, item_type);
        storage.can_accept(item_type, (1 + reserved) as u32)
    } else {
        false
    }
}
