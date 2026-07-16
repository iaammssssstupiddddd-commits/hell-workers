mod assignment_loop;
mod members;

use bevy::prelude::*;
use hw_core::area::TaskArea;
use hw_core::relationships::ManagedTasks;
use hw_logistics::tile_index::TileSiteIndex;
use hw_spatial::{DesignationSpatialGrid, ResourceSpatialGrid, TransportRequestSpatialGrid};
use hw_world::{WalkabilityConnectivityCache, WorldMap};

use crate::familiar_ai::decide::task_management::context::ConstructionSitePositions;
use crate::familiar_ai::decide::task_management::{
    FamiliarSoulQuery, FamiliarTaskAssignmentQueries, IncomingDeliverySnapshot, ReservationShadow,
};

pub use assignment_loop::take_reachable_with_cache_calls;
use assignment_loop::try_assign_for_workers;
use members::collect_idle_members;

/// タスク委譲に必要なイミュータブルな環境データをまとめた構造体。
pub struct DelegationEnvCtx<'a> {
    pub fam_entity: Entity,
    pub fam_pos: Vec2,
    pub squad: &'a [Entity],
    pub task_area_opt: Option<&'a TaskArea>,
    pub fatigue_threshold: f32,
    pub designation_grid: &'a DesignationSpatialGrid,
    pub transport_request_grid: &'a TransportRequestSpatialGrid,
    pub managed_tasks: &'a ManagedTasks,
    pub resource_grid: &'a ResourceSpatialGrid,
    pub world_map: &'a WorldMap,
    pub tile_site_index: &'a TileSiteIndex,
    pub incoming_snapshot: &'a IncomingDeliverySnapshot,
}

pub struct TaskManager;

impl TaskManager {
    pub fn delegate_task(
        env: DelegationEnvCtx<'_>,
        queries: &mut FamiliarTaskAssignmentQueries,
        construction_sites: &impl ConstructionSitePositions,
        q_souls: &mut FamiliarSoulQuery,
        connectivity_cache: &mut WalkabilityConnectivityCache,
        reservation_shadow: &mut ReservationShadow,
    ) -> Option<Entity> {
        let idle_members = collect_idle_members(env.squad, env.fatigue_threshold, q_souls);

        try_assign_for_workers(
            &idle_members,
            &env,
            queries,
            construction_sites,
            q_souls,
            connectivity_cache,
            reservation_shadow,
        )
    }
}
