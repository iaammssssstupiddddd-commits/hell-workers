//! FamiliarDelegationContext と process_task_delegation_and_movement の定義。
//!
//! WorldMap / PathfindingContext / ConstructionSiteAccess などは全て leaf crate 由来であり、
//! hw_familiar_ai から直接参照できる。

use bevy::prelude::*;
use hw_core::familiar::{FamiliarAiState, FamiliarOperation, Familiar};
use hw_core::relationships::ManagedTasks;
use hw_core::soul::{Destination, IdleBehavior, Path};
use hw_jobs::AssignedTask;
use hw_jobs::ConstructionSiteAccess;
use hw_logistics::tile_index::TileSiteIndex;
use hw_spatial::{DesignationSpatialGrid, ResourceSpatialGrid, TransportRequestSpatialGrid};
use hw_world::map::WorldMap;
use hw_world::pathfinding::PathfindingContext;
use std::collections::HashMap;

use super::query_types::FamiliarSoulQuery;
use super::resources::ReachabilityCacheKey;
use super::task_management::{FamiliarTaskAssignmentQueries, IncomingDeliverySnapshot, ReservationShadow};
use super::task_management::TaskManager;
use super::{state_handlers, supervising};

pub use super::recruitment::{FamiliarRecruitmentContext, RecruitmentOutcome, process_recruitment};
pub use super::helpers::{
    FamiliarSquadContext, SquadManagementOutcome, finalize_state_transitions,
    process_squad_management,
};

/// タスク委譲と移動制御に必要なコンテキスト
pub struct FamiliarDelegationContext<'a, 'w, 's> {
    pub fam_entity: Entity,
    pub fam_transform: &'a Transform,
    pub familiar_op: &'a FamiliarOperation,
    pub ai_state: &'a mut FamiliarAiState,
    pub fam_dest: &'a mut Destination,
    pub fam_path: &'a mut Path,
    pub task_area_opt: Option<&'a hw_core::area::TaskArea>,
    pub squad_entities: &'a [Entity],
    pub q_souls: &'a mut FamiliarSoulQuery<'w, 's>,
    pub task_queries: &'a mut FamiliarTaskAssignmentQueries<'w, 's>,
    pub construction_sites: &'a ConstructionSiteAccess<'w, 's>,
    pub designation_grid: &'a DesignationSpatialGrid,
    pub transport_request_grid: &'a TransportRequestSpatialGrid,
    pub resource_grid: &'a ResourceSpatialGrid,
    pub managed_tasks: &'a ManagedTasks,
    pub world_map: &'a WorldMap,
    pub pf_context: &'a mut PathfindingContext,
    pub delta_secs: f32,
    pub allow_task_delegation: bool,
    pub state_changed: bool,
    pub reservation_shadow: &'a mut ReservationShadow,
    pub tile_site_index: &'a TileSiteIndex,
    pub incoming_snapshot: &'a IncomingDeliverySnapshot,
    pub reachability_frame_cache: &'a mut HashMap<ReachabilityCacheKey, bool>,
}

/// タスク委譲と移動制御を実行
pub fn process_task_delegation_and_movement(ctx: &mut FamiliarDelegationContext<'_, '_, '_>) {
    let fam_pos = ctx.fam_transform.translation.truncate();
    let fatigue_threshold = ctx.familiar_op.fatigue_threshold;

    // タスク委譲
    let has_available_task = if ctx.allow_task_delegation {
        TaskManager::delegate_task(
            ctx.fam_entity,
            fam_pos,
            ctx.squad_entities,
            ctx.task_area_opt,
            fatigue_threshold,
            ctx.task_queries,
            ctx.construction_sites,
            ctx.q_souls,
            ctx.designation_grid,
            ctx.transport_request_grid,
            ctx.managed_tasks,
            ctx.resource_grid,
            ctx.world_map,
            ctx.pf_context,
            ctx.reservation_shadow,
            ctx.tile_site_index,
            ctx.incoming_snapshot,
            ctx.reachability_frame_cache,
        )
        .is_some()
    } else {
        false
    };

    // state_changed があっても、Supervising/SearchingTask なら各ロジックを呼ぶ
    if !ctx.state_changed
        || matches!(
            *ctx.ai_state,
            FamiliarAiState::Supervising { .. } | FamiliarAiState::SearchingTask
        )
    {
        let active_members: Vec<Entity> = ctx
            .squad_entities
            .iter()
            .filter(|&&e| {
                if let Ok((_, _, _, _, _, _, idle, _, _, _)) = ctx.q_souls.get(e) {
                    idle.behavior != IdleBehavior::ExhaustedGathering
                } else {
                    false
                }
            })
            .copied()
            .collect();

        debug!(
            "FAM_AI: Movement control - state: {:?}, active_members: {}, has_available_task: {}, state_changed: {}",
            *ctx.ai_state,
            active_members.len(),
            has_available_task,
            ctx.state_changed
        );

        match *ctx.ai_state {
            FamiliarAiState::Supervising { .. } => {
                let mut q_supervising_lens = ctx.q_souls.transmute_lens_filtered::<
                    (Entity, &Transform, &AssignedTask),
                    Without<Familiar>,
                >();
                let q_supervising = q_supervising_lens.query();
                let mut supervising_ctx = supervising::FamiliarSupervisingContext {
                    fam_entity: ctx.fam_entity,
                    fam_pos,
                    active_members: &active_members,
                    task_area_opt: ctx.task_area_opt,
                    delta_secs: ctx.delta_secs,
                    ai_state: ctx.ai_state,
                    fam_dest: ctx.fam_dest,
                    fam_path: ctx.fam_path,
                    q_souls: &q_supervising,
                };
                state_handlers::supervising::handle_supervising_state(&mut supervising_ctx);
            }
            FamiliarAiState::SearchingTask => {
                debug!("FAM_AI: {:?} executing SearchingTask logic", ctx.fam_entity);
                state_handlers::searching::handle_searching_task_state(
                    ctx.fam_entity,
                    fam_pos,
                    ctx.task_area_opt,
                    ctx.fam_dest,
                    ctx.fam_path,
                );
            }
            _ => {}
        }
    }
}
