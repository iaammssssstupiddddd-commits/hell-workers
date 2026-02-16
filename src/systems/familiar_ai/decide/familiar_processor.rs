//! 使い魔AIの処理ロジック
//!
//! `familiar_ai_system` の処理を複数の関数に分割して管理します。

use crate::entities::damned_soul::{Destination, IdleBehavior, Path};
use crate::entities::familiar::{Familiar, FamiliarOperation};
use crate::events::SquadManagementRequest;
use crate::relationships::{Commanding, ManagedTasks};
use crate::systems::command::TaskArea;
use crate::systems::familiar_ai::FamiliarSoulQuery;
use crate::systems::familiar_ai::decide::task_management::ReservationShadow;
use crate::systems::soul_ai::execute::task_execution::context::TaskAssignmentQueries;
use crate::systems::spatial::{DesignationSpatialGrid, SpatialGrid, TransportRequestSpatialGrid};
use bevy::prelude::*;

use super::recruitment::RecruitmentManager;
use super::squad::SquadManager;
use super::state_handlers;
use super::task_management::TaskManager;
use crate::entities::damned_soul::StressBreakdown;
use crate::systems::familiar_ai::FamiliarAiState;
use crate::world::map::WorldMap;
use crate::world::pathfinding::PathfindingContext;

/// 分隊管理に必要なコンテキスト
pub struct FamiliarSquadContext<'a, 'w, 's> {
    pub fam_entity: Entity,
    pub familiar_op: &'a FamiliarOperation,
    pub commanding: Option<&'a Commanding>,
    pub q_souls: &'a FamiliarSoulQuery<'w, 's>,
    pub request_writer: &'a mut MessageWriter<'w, SquadManagementRequest>,
}

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
    pub spatial_grid: &'a SpatialGrid,
    pub q_souls: &'a mut FamiliarSoulQuery<'w, 's>,
    pub q_breakdown: &'a Query<'w, 's, &'static StressBreakdown>,
    pub q_resting: &'a Query<'w, 's, (), With<crate::relationships::RestingIn>>,
    pub q_cooldown:
        &'a Query<'w, 's, &'static crate::entities::damned_soul::RestAreaCooldown>,
    pub request_writer: &'a mut MessageWriter<'w, SquadManagementRequest>,
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
    pub task_queries: &'a mut TaskAssignmentQueries<'w, 's>,
    pub designation_grid: &'a DesignationSpatialGrid,
    pub transport_request_grid: &'a TransportRequestSpatialGrid,
    pub managed_tasks: &'a ManagedTasks,
    pub world_map: &'a WorldMap,
    pub pf_context: &'a mut PathfindingContext,
    pub delta_secs: f32,
    pub allow_task_delegation: bool,
    pub state_changed: bool,
    pub reservation_shadow: &'a mut ReservationShadow,
}

/// 分隊管理を実行
pub fn process_squad_management(ctx: &mut FamiliarSquadContext<'_, '_, '_>) -> Vec<Entity> {
    let initial_squad = SquadManager::build_squad(ctx.commanding);

    // 分隊を検証（無効なメンバーを除外）
    let (mut squad_entities, invalid_members) =
        SquadManager::validate_squad(initial_squad, ctx.fam_entity, ctx.q_souls);

    // 疲労・崩壊したメンバーをリリース要求
    let released_entities = SquadManager::release_fatigued(
        &squad_entities,
        ctx.fam_entity,
        ctx.familiar_op.fatigue_threshold,
        ctx.q_souls,
        ctx.request_writer,
    );

    // リリースされたメンバーを分隊から除外
    if !released_entities.is_empty() {
        squad_entities.retain(|e| !released_entities.contains(e));
    }

    // 無効なメンバーも分隊から除外
    if !invalid_members.is_empty() {
        squad_entities.retain(|e| !invalid_members.contains(e));
    }

    squad_entities
}

/// リクルート処理を実行
pub fn process_recruitment(ctx: &mut FamiliarRecruitmentContext<'_, '_, '_>) -> bool {
    let fam_pos = ctx.fam_transform.translation.truncate();
    let command_radius = ctx.familiar.command_radius;
    let fatigue_threshold = ctx.familiar_op.fatigue_threshold;

    // スカウト中以外で分隊に空きがあれば新規リクルートを試みる
    if ctx.squad_entities.len() < ctx.max_workers {
        // 近場のリクルート検索 (即時勧誘)
        if let Some(new_recruit) = RecruitmentManager::try_immediate_recruit(
            ctx.fam_entity,
            fam_pos,
            command_radius,
            fatigue_threshold,
            ctx.spatial_grid,
            ctx.q_souls,
            ctx.q_breakdown,
            ctx.q_resting,
            ctx.q_cooldown,
            ctx.request_writer,
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
                ctx.spatial_grid,
                &mut *ctx.q_souls,
                ctx.q_breakdown,
                ctx.q_resting,
                ctx.q_cooldown,
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

/// 状態遷移の最終確定
pub fn finalize_state_transitions(
    ai_state: &mut FamiliarAiState,
    squad_entities: &[Entity],
    fam_entity: Entity,
    max_workers: usize,
) -> bool {
    let mut state_changed = false;

    // 分隊が空になった場合の処理
    if squad_entities.is_empty() {
        if !matches!(
            *ai_state,
            FamiliarAiState::SearchingTask
                | FamiliarAiState::Idle
                | FamiliarAiState::Scouting { .. }
        ) {
            let prev_state = ai_state.clone();
            *ai_state = FamiliarAiState::SearchingTask;
            state_changed = true;
            info!(
                "FAM_AI: {:?} squad is empty. Transitioning to SearchingTask from {:?}",
                fam_entity, prev_state
            );
        }
    } else {
        // メンバーがいる場合
        let is_squad_full = squad_entities.len() >= max_workers;

        if !matches!(*ai_state, FamiliarAiState::Scouting { .. }) {
            // 枠に空きがあるなら、監視を中断して探索へ戻れるようにする
            if !is_squad_full && matches!(*ai_state, FamiliarAiState::Supervising { .. }) {
                *ai_state = FamiliarAiState::SearchingTask;
                state_changed = true;
                info!(
                    "FAM_AI: {:?} squad has open slots ({}/{}). Switching to SearchingTask",
                    fam_entity,
                    squad_entities.len(),
                    max_workers
                );
            } else if is_squad_full && !matches!(*ai_state, FamiliarAiState::Supervising { .. }) {
                // 枠がいっぱいで、かつ監視モード以外なら監視へ
                *ai_state = FamiliarAiState::Supervising {
                    target: None,
                    timer: 0.0,
                };
                state_changed = true;
                info!("FAM_AI: {:?} squad full. -> Supervising", fam_entity);
            }
        }
    }

    state_changed
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
            ctx.world_map,
            ctx.pf_context,
            ctx.reservation_shadow,
        )
        .is_some()
    } else {
        false
    };

    // 移動制御
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
                        q_souls: ctx.q_souls,
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
