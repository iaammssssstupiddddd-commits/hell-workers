use crate::systems::logistics::ResourceType;
use bevy::prelude::*;

pub fn resolve_haul_to_mixer_inputs(
    task_entity: Entity,
    queries: &crate::systems::soul_ai::task_execution::context::TaskAssignmentQueries,
) -> Option<(Entity, ResourceType)> {
    let mixer_entity = queries.target_mixers.get(task_entity).ok().map(|tm| tm.0)?;
    let item_type = queries.items.get(task_entity).ok().map(|(it, _)| it.0)?;
    Some((mixer_entity, item_type))
}

pub fn resolve_haul_water_to_mixer_inputs(
    task_entity: Entity,
    queries: &crate::systems::soul_ai::task_execution::context::TaskAssignmentQueries,
) -> Option<(Entity, Entity)> {
    let mixer_entity = queries.target_mixers.get(task_entity).ok().map(|tm| tm.0)?;
    let tank_entity = queries.belongs.get(task_entity).ok().map(|b| b.0)?;
    Some((mixer_entity, tank_entity))
}
