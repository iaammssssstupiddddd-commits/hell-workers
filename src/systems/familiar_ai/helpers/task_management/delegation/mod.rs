mod assignment_loop;
mod members;

use crate::relationships::ManagedTasks;
use crate::systems::command::TaskArea;
use crate::systems::familiar_ai::helpers::task_management::ReservationShadow;
use crate::systems::spatial::DesignationSpatialGrid;
use crate::world::map::WorldMap;
use crate::world::pathfinding::PathfindingContext;
use bevy::prelude::*;

use crate::systems::familiar_ai::FamiliarSoulQuery;

use assignment_loop::try_assign_for_workers;
use members::collect_idle_members;

/// タスク管理ユーティリティ
pub struct TaskManager;

impl TaskManager {
    /// タスクを委譲する（タスク検索 + 割り当て）
    pub fn delegate_task(
        fam_entity: Entity,
        fam_pos: Vec2,
        squad: &[Entity],
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
        let idle_members = collect_idle_members(squad, fatigue_threshold, q_souls);

        try_assign_for_workers(
            &idle_members,
            fam_entity,
            fam_pos,
            task_area_opt,
            fatigue_threshold,
            queries,
            q_souls,
            designation_grid,
            managed_tasks,
            world_map,
            pf_context,
            reservation_shadow,
        )
    }
}
