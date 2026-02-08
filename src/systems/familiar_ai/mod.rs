use crate::constants::FAMILIAR_TASK_DELEGATION_INTERVAL;
use crate::entities::damned_soul::StressBreakdown;
use crate::entities::familiar::FamiliarCommand;
use crate::systems::GameSystemSet;
use crate::systems::soul_ai::scheduling::FamiliarAiSystemSet;
use crate::systems::spatial::{DesignationSpatialGrid, SpatialGrid};
use crate::systems::visual::speech::components::{FamiliarBubble, SpeechBubble};
use crate::world::pathfinding::PathfindingContext;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;

pub mod decide;
pub mod execute;
pub mod helpers;
pub mod perceive;
pub mod update;
use helpers::familiar_processor::{
    FamiliarDelegationContext, FamiliarRecruitmentContext, FamiliarSquadContext,
    finalize_state_transitions, process_recruitment, process_squad_management,
    process_task_delegation_and_movement,
};
pub use helpers::query_types::{FamiliarSoulQuery, FamiliarStateQuery, FamiliarTaskQuery};
use perceive::state_detection::determine_transition_reason;

/// 使い魔のAI状態
#[derive(Component, Debug, Clone, PartialEq, Reflect)]
#[reflect(Component)]
pub enum FamiliarAiState {
    /// 待機中
    Idle,
    /// タスク探索中
    SearchingTask,
    /// スカウト中
    Scouting { target_soul: Entity },
    /// 監視中
    Supervising {
        /// 現在固定しているターゲット
        target: Option<Entity>,
        /// 切り替え禁止タイマー
        timer: f32,
    },
}

impl Default for FamiliarAiState {
    fn default() -> Self {
        Self::Idle
    }
}

pub struct FamiliarAiPlugin;

impl Plugin for FamiliarAiPlugin {
    fn build(&self, app: &mut App) {
        app.configure_sets(
            Update,
            (
                FamiliarAiSystemSet::Perceive,
                FamiliarAiSystemSet::Update,
                FamiliarAiSystemSet::Decide,
                FamiliarAiSystemSet::Execute,
            )
                .chain()
                .in_set(GameSystemSet::Logic),
        )
        .register_type::<FamiliarAiState>()
        .register_type::<decide::encouragement::EncouragementCooldown>()
        .init_resource::<perceive::resource_sync::SharedResourceCache>()
        .init_resource::<perceive::resource_sync::ReservationSyncTimer>()
        .init_resource::<DesignationSpatialGrid>()
        .init_resource::<FamiliarTaskDelegationTimer>()
        .add_systems(
            Update,
            (
                // === Perceive Phase ===
                // 環境情報の読み取り、変化の検出
                (
                    perceive::state_detection::detect_state_changes_system,
                    perceive::state_detection::detect_command_changes_system,
                    perceive::resource_sync::sync_reservations_system,
                    perceive::max_soul::handle_max_soul_changed_system,
                )
                    .in_set(crate::systems::soul_ai::scheduling::FamiliarAiSystemSet::Perceive),
                // Perceive → Update 間の同期
                ApplyDeferred
                    .after(crate::systems::soul_ai::scheduling::FamiliarAiSystemSet::Perceive)
                    .before(crate::systems::soul_ai::scheduling::FamiliarAiSystemSet::Update),
                // Update → Decide 間の同期
                ApplyDeferred
                    .after(crate::systems::soul_ai::scheduling::FamiliarAiSystemSet::Update)
                    .before(crate::systems::soul_ai::scheduling::FamiliarAiSystemSet::Decide),
                // === Decide Phase ===
                // 次の行動の選択、要求の生成
                ((
                    familiar_ai_state_system,
                    ApplyDeferred,
                    familiar_task_delegation_system,
                    decide::following::following_familiar_system,
                    decide::encouragement::encouragement_decision_system,
                )
                    .chain(),)
                    .in_set(crate::systems::soul_ai::scheduling::FamiliarAiSystemSet::Decide),
                // === Execute Phase ===
                // 決定された行動の実行
                (
                    execute::state_apply::familiar_state_apply_system,
                    perceive::state_detection::handle_state_changed_system,
                    process_squad_management_apply_system,
                    execute::encouragement_apply::encouragement_apply_system,
                    execute::encouragement_apply::cleanup_encouragement_cooldowns_system,
                )
                    .in_set(crate::systems::soul_ai::scheduling::FamiliarAiSystemSet::Execute),
            ),
        );
    }
}

/// Redirect for name consistency if needed, or just use the one in processor
pub use helpers::familiar_processor::apply_squad_management_requests_system as process_squad_management_apply_system;

#[derive(Resource)]
pub struct FamiliarTaskDelegationTimer {
    pub timer: Timer,
    pub first_run_done: bool,
}

impl Default for FamiliarTaskDelegationTimer {
    fn default() -> Self {
        Self {
            timer: Timer::from_seconds(FAMILIAR_TASK_DELEGATION_INTERVAL, TimerMode::Repeating),
            first_run_done: false,
        }
    }
}

#[derive(SystemParam)]
pub struct FamiliarAiParams<'w, 's> {
    pub commands: Commands<'w, 's>,
    pub time: Res<'w, Time>,
    pub spatial_grid: Res<'w, SpatialGrid>,
    pub q_familiars: FamiliarStateQuery<'w, 's>,
    pub q_souls: FamiliarSoulQuery<'w, 's>,
    pub q_breakdown: Query<'w, 's, &'static StressBreakdown>,
    // resource_cache removed (included in task_queries)
    pub game_assets: Res<'w, crate::assets::GameAssets>,
    pub q_bubbles: Query<'w, 's, (Entity, &'static SpeechBubble), With<FamiliarBubble>>,
    pub decide_output: decide::FamiliarDecideOutput<'w>,
}

#[derive(SystemParam)]
pub struct FamiliarAiTaskParams<'w, 's> {
    pub time: Res<'w, Time>,
    pub delegation_timer: ResMut<'w, FamiliarTaskDelegationTimer>,
    pub q_familiars: FamiliarTaskQuery<'w, 's>,
    pub q_souls: FamiliarSoulQuery<'w, 's>,
    pub task_queries:
        crate::systems::soul_ai::execute::task_execution::context::TaskAssignmentQueries<'w, 's>,
    pub designation_grid: Res<'w, DesignationSpatialGrid>,
    pub world_map: Res<'w, crate::world::map::WorldMap>,
    pub pf_context: Local<'s, PathfindingContext>,
}

/// 使い魔AIの状態更新システム
pub fn familiar_ai_state_system(params: FamiliarAiParams) {
    let FamiliarAiParams {
        mut commands,
        time,
        spatial_grid,
        mut q_familiars,
        mut q_souls,
        q_breakdown,
        // fatigue_threshold removed
        // max_workers removed
        mut decide_output,
        q_bubbles,
        game_assets,
        ..
    } = params;
    // 1. 搬送中のアイテム・ストックパイル予約状況を事前計算
    // フェーズ2: 全ソウルをイテレートする代わりにキャッシュ（HaulReservationCache）を使用
    // let mut in_flight_haulers = std::collections::HashMap::new();
    // for (_, _, _, task, _, _, _, _, _) in q_souls.iter() {
    //     if let AssignedTask::Haul { stockpile, .. } = *task {
    //         *in_flight_haulers.entry(stockpile).or_insert(0) += 1;
    //     }
    // }

    for (
        fam_entity,
        fam_transform,
        familiar,
        familiar_op,
        active_command,
        mut ai_state,
        mut fam_dest,
        mut fam_path,
        task_area_opt,
        commanding,
        voice_opt,
        mut history_opt,
    ) in q_familiars.iter_mut()
    {
        // 個別の使い魔の処理開始ログ
        debug!(
            "FAM_AI: {:?} Processing. Command: {:?}, State: {:?}, Area: {}",
            fam_entity,
            active_command.command,
            *ai_state,
            task_area_opt.is_some()
        );

        // 1. 基本コマンドチェック（Idle状態の処理）
        if matches!(active_command.command, FamiliarCommand::Idle) {
            let transition_result = helpers::state_handlers::idle::handle_idle_state(
                // state_handlers は helpers 配下に移動済み
                fam_entity,
                fam_transform,
                active_command,
                &mut ai_state,
                &mut fam_dest,
                &mut fam_path,
                &mut commands,
                &time,
                &game_assets,
                &q_bubbles,
                history_opt.as_deref_mut(),
                voice_opt,
            );
            if transition_result.apply_to(&mut ai_state) {
                debug!("FAM_AI: {:?} state changed to Idle", fam_entity);
            }
            continue;
        }

        let old_state = ai_state.clone();
        let mut state_changed = false;
        let fam_pos = fam_transform.translation.truncate();
        let fatigue_threshold = familiar_op.fatigue_threshold;
        let max_workers = familiar_op.max_controlled_soul;

        // 分隊管理を実行
        let mut squad_ctx = FamiliarSquadContext {
            fam_entity,
            familiar_op,
            commanding,
            q_souls: &q_souls,
            request_writer: &mut decide_output.squad_requests,
        };
        let mut squad_entities = process_squad_management(&mut squad_ctx);

        // 状態に応じたロジック実行
        match *ai_state {
            FamiliarAiState::Scouting { target_soul } => {
                // Scoutingロジックを実行 (分隊の空き状況に関わらず常にチェック)
                let mut scouting_ctx = helpers::scouting::FamiliarScoutingContext {
                    fam_entity,
                    fam_pos,
                    target_soul,
                    fatigue_threshold,
                    max_workers,
                    squad: &mut squad_entities,
                    ai_state: &mut ai_state,
                    fam_dest: &mut fam_dest,
                    fam_path: &mut fam_path,
                    q_souls: &mut q_souls,
                    q_breakdown: &q_breakdown,
                    request_writer: &mut decide_output.squad_requests,
                };
                let transition_result =
                    helpers::state_handlers::scouting::handle_scouting_state(&mut scouting_ctx);
                state_changed = transition_result.apply_to(&mut ai_state);
            }
            _ => {
                // スカウト中以外でリクルートを試みる
                let mut recruitment_ctx = FamiliarRecruitmentContext {
                    fam_entity,
                    fam_transform,
                    familiar,
                    familiar_op,
                    ai_state: &mut ai_state,
                    fam_dest: &mut fam_dest,
                    fam_path: &mut fam_path,
                    squad_entities: &mut squad_entities,
                    max_workers,
                    spatial_grid: &spatial_grid,
                    q_souls: &mut q_souls,
                    q_breakdown: &q_breakdown,
                    request_writer: &mut decide_output.squad_requests,
                };

                if process_recruitment(&mut recruitment_ctx) {
                    state_changed = true;
                }
            }
        }

        // 状態遷移の最終確定
        if finalize_state_transitions(&mut ai_state, &squad_entities, fam_entity, max_workers) {
            state_changed = true;
        }

        if state_changed {
            decide_output
                .state_requests
                .write(crate::events::FamiliarStateRequest {
                    familiar_entity: fam_entity,
                    new_state: ai_state.clone(),
                });
            // 状態遷移イベントを発火（Changed フィルタで検知できない場合の補完）
            decide_output
                .state_changed_events
                .write(crate::events::FamiliarAiStateChangedEvent {
                    familiar_entity: fam_entity,
                    from: old_state.clone(),
                    to: ai_state.clone(),
                    reason: determine_transition_reason(&old_state, &*ai_state),
                });
        }
    }
}

/// 使い魔AIのタスク委譲・移動システム
pub fn familiar_task_delegation_system(params: FamiliarAiTaskParams) {
    let FamiliarAiTaskParams {
        time,
        mut delegation_timer,
        mut q_familiars,
        mut q_souls,
        mut task_queries,
        designation_grid,
        world_map,
        mut pf_context,
        ..
    } = params;

    let timer_finished = delegation_timer.timer.tick(time.delta()).just_finished();
    let allow_task_delegation = !delegation_timer.first_run_done || timer_finished;
    delegation_timer.first_run_done = true;

    let mut reservation_shadow =
        crate::systems::familiar_ai::helpers::task_management::ReservationShadow::default();

    for (
        fam_entity,
        fam_transform,
        familiar_op,
        active_command,
        mut ai_state,
        mut fam_dest,
        mut fam_path,
        task_area_opt,
        commanding,
        managed_tasks_opt,
    ) in q_familiars.iter_mut()
    {
        if matches!(active_command.command, FamiliarCommand::Idle) {
            continue;
        }

        let state_changed = ai_state.is_changed();
        let default_tasks = crate::relationships::ManagedTasks::default();
        let managed_tasks = managed_tasks_opt.unwrap_or(&default_tasks);

        let initial_squad =
            crate::systems::familiar_ai::helpers::squad::SquadManager::build_squad(commanding);
        let (squad_entities, _invalid_members) =
            crate::systems::familiar_ai::helpers::squad::SquadManager::validate_squad(
                initial_squad,
                fam_entity,
                &mut q_souls,
            );

        let mut delegation_ctx = FamiliarDelegationContext {
            fam_entity,
            fam_transform,
            familiar_op,
            ai_state: &mut *ai_state,
            fam_dest: &mut *fam_dest,
            fam_path: &mut *fam_path,
            task_area_opt,
            squad_entities: &squad_entities,
            q_souls: &mut q_souls,
            task_queries: &mut task_queries,
            designation_grid: &designation_grid,
            managed_tasks,
            world_map: &world_map,
            pf_context: &mut *pf_context,
            delta_secs: time.delta_secs(),
            allow_task_delegation,
            state_changed,
            reservation_shadow: &mut reservation_shadow,
        };
        process_task_delegation_and_movement(&mut delegation_ctx);
    }
}
