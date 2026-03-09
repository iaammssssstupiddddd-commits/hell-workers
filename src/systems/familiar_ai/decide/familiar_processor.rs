//! 使い魔AIの処理ロジック
//!
//! `familiar_ai_system` の処理を複数の関数に分割して管理します。

use crate::entities::damned_soul::{Destination, IdleBehavior, Path};
use crate::entities::familiar::{Familiar, FamiliarOperation};
use crate::events::SquadManagementRequest;
use crate::relationships::ManagedTasks;
use crate::systems::command::TaskArea;
use crate::systems::familiar_ai::FamiliarAiState;
use crate::systems::familiar_ai::FamiliarSoulQuery;
use crate::systems::familiar_ai::decide::task_delegation::ReachabilityCacheKey;
use crate::systems::familiar_ai::decide::task_management::IncomingDeliverySnapshot;
use crate::systems::familiar_ai::decide::task_management::{
    FamiliarTaskAssignmentQueries, ReservationShadow,
};
use crate::systems::logistics::TileSiteIndex;
use crate::systems::soul_ai::execute::task_execution::AssignedTask;
use crate::systems::spatial::{
    DesignationSpatialGrid, ResourceSpatialGrid, SpatialGrid, TransportRequestSpatialGrid,
};
use bevy::prelude::*;
use std::collections::HashMap;
use std::collections::HashSet;

use super::recruitment::RecruitmentManager;
use super::state_handlers;
use super::task_management::TaskManager;
use crate::entities::damned_soul::StressBreakdown;
use crate::world::map::WorldMap;
use crate::world::pathfinding::PathfindingContext;

pub use hw_ai::familiar_ai::decide::helpers::{
    FamiliarSquadContext, SquadManagementOutcome, finalize_state_transitions,
    process_squad_management,
};

/// リクルート判定に必要なコンテキスト
pub struct FamiliarRecruitmentContext<'a, 'w, 's> {
    pub fam_entity: Entity,
    pub fam_transform: &'a Transform,
    pub familiar: &'a Familiar,
    pub familiar_op: &'a FamiliarOperation,
    pub ai_state: &'a mut FamiliarAiState,
    pub fam_dest: &'a mut Destination,
    pub fam_path: &'a mut Path,
    pub squad_entities: &'a mut Vec<Entity>,
    pub max_workers: usize,
    pub task_area_opt: Option<&'a TaskArea>,
    pub spatial_grid: &'a SpatialGrid,
    pub q_souls: &'a mut FamiliarSoulQuery<'w, 's>,
    pub q_breakdown: &'a Query<'w, 's, &'static StressBreakdown>,
    pub q_resting: &'a Query<'w, 's, (), With<crate::relationships::RestingIn>>,
    pub q_cooldown: &'a Query<'w, 's, &'static crate::entities::damned_soul::RestAreaCooldown>,
    pub request_writer: &'a mut MessageWriter<'w, SquadManagementRequest>,
    /// 同フレーム内でのリクルート予約セット（重複防止）
    pub recruitment_reservations: &'a mut HashSet<Entity>,
}

/// タスク委譲と移動制御に必要なコンテキスト
pub struct FamiliarDelegationContext<'a, 'w, 's> {
    pub fam_entity: Entity,
    pub fam_transform: &'a Transform,
    pub familiar_op: &'a FamiliarOperation,
    pub ai_state: &'a mut FamiliarAiState,
    pub fam_dest: &'a mut Destination,
    pub fam_path: &'a mut Path,
    pub task_area_opt: Option<&'a TaskArea>,
    pub squad_entities: &'a [Entity],
    pub q_souls: &'a mut FamiliarSoulQuery<'w, 's>,
    pub task_queries: &'a mut FamiliarTaskAssignmentQueries<'w, 's>,
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

/// リクルート処理を実行
pub fn process_recruitment(ctx: &mut FamiliarRecruitmentContext<'_, '_, '_>) -> bool {
    let fam_pos = ctx.fam_transform.translation.truncate();
    let command_radius = ctx.familiar.command_radius;
    let fatigue_threshold = ctx.familiar_op.fatigue_threshold;
    let task_area_center = ctx.task_area_opt.map(TaskArea::center);

    // スカウト中以外で分隊に空きがあれば新規リクルートを試みる
    if ctx.squad_entities.len() < ctx.max_workers {
        // 近場のリクルート検索 (即時勧誘)
        if let Some(new_recruit) = RecruitmentManager::try_immediate_recruit(
            ctx.fam_entity,
            fam_pos,
            command_radius,
            fatigue_threshold,
            task_area_center,
            ctx.spatial_grid,
            ctx.q_souls,
            ctx.q_breakdown,
            ctx.q_resting,
            ctx.q_cooldown,
            ctx.request_writer,
            ctx.recruitment_reservations,
        ) {
            debug!(
                "FAM_AI: {:?} recruiting nearby soul {:?}",
                ctx.fam_entity, new_recruit
            );
            ctx.squad_entities.push(new_recruit);
            return true;
        }
        // 遠方のリクルート検索 (Scouting開始)
        else {
            if let Some(distant_recruit) = RecruitmentManager::start_scouting(
                fam_pos,
                fatigue_threshold,
                task_area_center,
                ctx.spatial_grid,
                &mut *ctx.q_souls,
                ctx.q_breakdown,
                ctx.q_resting,
                ctx.q_cooldown,
                ctx.recruitment_reservations,
            ) {
                debug!(
                    "FAM_AI: {:?} scouting distant soul {:?}",
                    ctx.fam_entity, distant_recruit
                );
                *ctx.ai_state = FamiliarAiState::Scouting {
                    target_soul: distant_recruit,
                };

                // 即座に移動開始
                if let Ok((_, target_transform, _, _, _, _, _, _, _, _)) =
                    ctx.q_souls.get(distant_recruit)
                {
                    let target_pos = target_transform.translation.truncate();
                    ctx.fam_dest.0 = target_pos;
                    ctx.fam_path.waypoints = vec![target_pos];
                    ctx.fam_path.current_index = 0;
                }
                return true;
            } else {
                // 何も見つからなければログに出す (デバッグ用)
                debug!("FAM_AI: {:?} No recruitable souls found", ctx.fam_entity);
            }
        }
    } else {
        debug!(
            "FAM_AI: {:?} Squad full ({}/{})",
            ctx.fam_entity,
            ctx.squad_entities.len(),
            ctx.max_workers
        );
    }
    false
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
                    Without<crate::entities::familiar::Familiar>,
                >();
                let q_supervising = q_supervising_lens.query();
                let mut supervising_ctx =
                    crate::systems::familiar_ai::decide::supervising::FamiliarSupervisingContext {
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
