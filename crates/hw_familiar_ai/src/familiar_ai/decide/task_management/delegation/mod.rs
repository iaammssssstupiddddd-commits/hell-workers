mod assignment_loop;
mod members;

use bevy::prelude::*;
use hw_core::area::TaskArea;
use hw_core::relationships::ManagedTasks;
use hw_logistics::tile_index::TileSiteIndex;
use hw_spatial::{DesignationSpatialGrid, ResourceSpatialGrid, TransportRequestSpatialGrid};
use hw_world::WorldMap;
use hw_world::pathfinding::PathfindingContext;
use std::collections::HashMap;

use crate::familiar_ai::decide::task_management::{
    FamiliarSoulQuery, FamiliarTaskAssignmentQueries, IncomingDeliverySnapshot, ReservationShadow,
};
use crate::familiar_ai::decide::task_management::context::ConstructionSitePositions;

pub use assignment_loop::{ReachabilityCacheKey, take_reachable_with_cache_calls};
use assignment_loop::try_assign_for_workers;
use members::collect_idle_members;

pub struct TaskManager;

impl TaskManager {
    #[allow(clippy::too_many_arguments)]
    pub fn delegate_task(
        fam_entity: Entity,
        fam_pos: Vec2,
        squad: &[Entity],
        task_area_opt: Option<&TaskArea>,
        fatigue_threshold: f32,
        queries: &mut FamiliarTaskAssignmentQueries,
        construction_sites: &impl ConstructionSitePositions,
        q_souls: &mut FamiliarSoulQuery,
        designation_grid: &DesignationSpatialGrid,
        transport_request_grid: &TransportRequestSpatialGrid,
        managed_tasks: &ManagedTasks,
        resource_grid: &ResourceSpatialGrid,
        world_map: &WorldMap,
        pf_context: &mut PathfindingContext,
        reservation_shadow: &mut ReservationShadow,
        tile_site_index: &TileSiteIndex,
        incoming_snapshot: &IncomingDeliverySnapshot,
        reachability_cache: &mut HashMap<ReachabilityCacheKey, bool>,
    ) -> Option<Entity> {
        let idle_members = collect_idle_members(squad, fatigue_threshold, q_souls);

        try_assign_for_workers(
            &idle_members,
            fam_entity,
            fam_pos,
            task_area_opt,
            fatigue_threshold,
            queries,
            construction_sites,
            q_souls,
            designation_grid,
            transport_request_grid,
            managed_tasks,
            resource_grid,
            world_map,
            pf_context,
            reservation_shadow,
            tile_site_index,
            incoming_snapshot,
            reachability_cache,
        )
    }
}
