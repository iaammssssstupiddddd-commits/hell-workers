use crate::relationships::ManagedTasks;
use crate::systems::command::TaskArea;
use crate::systems::familiar_ai::helpers::task_management::{
    AssignTaskContext, ReservationShadow, assign_task_to_worker, find_unassigned_task_in_area,
};
use crate::systems::spatial::DesignationSpatialGrid;
use crate::world::map::WorldMap;
use crate::world::pathfinding::PathfindingContext;
use bevy::prelude::*;

use crate::systems::familiar_ai::FamiliarSoulQuery;

pub(super) fn try_assign_for_workers(
    idle_members: &[(Entity, Vec2)],
    fam_entity: Entity,
    fam_pos: Vec2,
    task_area_opt: Option<&TaskArea>,
    fatigue_threshold: f32,
    queries: &mut crate::systems::soul_ai::execute::task_execution::context::TaskAssignmentQueries,
    q_souls: &mut FamiliarSoulQuery,
    designation_grid: &DesignationSpatialGrid,
    managed_tasks: &ManagedTasks,
    world_map: &WorldMap,
    pf_context: &mut PathfindingContext,
    reservation_shadow: &mut ReservationShadow,
) -> Option<Entity> {
    for (worker_entity, pos) in idle_members.iter().copied() {
        let candidates = find_unassigned_task_in_area(
            fam_entity,
            fam_pos,
            pos,
            task_area_opt,
            queries,
            designation_grid,
            managed_tasks,
            &queries.storage.target_blueprints,
            world_map,
            pf_context,
        );

        for task_entity in candidates {
            if assign_task_to_worker(
                AssignTaskContext {
                    fam_entity,
                    task_entity,
                    worker_entity,
                    fatigue_threshold,
                    task_area_opt,
                },
                queries,
                q_souls,
                reservation_shadow,
            ) {
                return Some(task_entity);
            }
        }
    }

    None
}
