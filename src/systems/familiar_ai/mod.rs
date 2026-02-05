use crate::entities::damned_soul::{DamnedSoul, Destination, IdleState, Path, StressBreakdown};
use crate::entities::familiar::{
    ActiveCommand, Familiar, FamiliarCommand, FamiliarOperation, FamiliarVoice,
};
use crate::relationships::{Commanding, ManagedTasks};
// use crate::systems::GameSystemSet; // Removed unused import
use crate::systems::command::TaskArea;
use crate::systems::soul_ai::gathering::ParticipatingIn;
use crate::systems::soul_ai::task_execution::AssignedTask;
use crate::systems::spatial::{
    DesignationSpatialGrid, SpatialGrid,
};
use crate::systems::visual::speech::components::{FamiliarBubble, SpeechBubble};
use crate::world::pathfinding::PathfindingContext;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;

pub mod encouragement; // 新規追加
pub mod familiar_processor;
pub mod following;
pub mod resource_cache;
pub mod helpers;
pub mod max_soul_handler;
pub mod recruitment;
pub mod scouting;
pub mod squad;
pub mod state_handlers;
pub mod state_transition;
pub mod supervising;
pub mod task_management;

use familiar_processor::{
    finalize_state_transitions, process_recruitment, process_squad_management,
    process_task_delegation_and_movement,
};
use state_transition::determine_transition_reason;

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
        app
            .register_type::<FamiliarAiState>()
            .register_type::<encouragement::EncouragementCooldown>()
            .init_resource::<resource_cache::SharedResourceCache>()
            .init_resource::<DesignationSpatialGrid>()
            .add_systems(
                Update,
                (
                    // --- Sense Phase (読み取り専用) ---
                    (
                        state_transition::detect_state_changes_system,
                        state_transition::detect_command_changes_system,
                        resource_cache::sync_reservations_system,
                        encouragement::cleanup_encouragement_cooldowns_system,
                    )
                        .in_set(crate::systems::soul_ai::scheduling::SoulAiSystemSet::Sense),
                    // --- React Phase (反応的状態変更) ---
                    (
                        max_soul_handler::handle_max_soul_changed_system,
                    )
                        .in_set(crate::systems::soul_ai::scheduling::SoulAiSystemSet::React),
                    // --- Think Phase (意思決定) ---
                    (
                        (
                            familiar_ai_state_system,
                            ApplyDeferred,
                            familiar_task_delegation_system,
                            following::following_familiar_system,
                            encouragement::encouragement_system,
                        )
                            .chain(),
                    )
                        .in_set(crate::systems::soul_ai::scheduling::SoulAiSystemSet::Think),
                    // --- Act Phase (実行) ---
                    (
                        state_transition::handle_state_changed_system,
                    )
                        .in_set(crate::systems::soul_ai::scheduling::SoulAiSystemSet::Act),
                ),
            );
    }
}

#[derive(SystemParam)]
pub struct FamiliarAiParams<'w, 's> {
    pub commands: Commands<'w, 's>,
    pub time: Res<'w, Time>,
    pub spatial_grid: Res<'w, SpatialGrid>,
    pub q_familiars: Query<
        'w,
        's,
        (
            Entity,
            &'static Transform,
            &'static Familiar,
            &'static FamiliarOperation,
            &'static ActiveCommand,
            &'static mut FamiliarAiState,
            &'static mut Destination,
            &'static mut Path,
            Option<&'static TaskArea>,
            Option<&'static Commanding>,
            Option<&'static FamiliarVoice>,
            Option<&'static mut crate::systems::visual::speech::cooldown::SpeechHistory>,
        ),
    >,
    pub q_souls: Query<
        'w,
        's,
        (
            Entity,
            &'static Transform,
            &'static DamnedSoul,
            &'static mut AssignedTask,
            &'static mut Destination,
            &'static mut Path,
            &'static IdleState,
            Option<&'static mut crate::systems::logistics::Inventory>,
            Option<&'static crate::entities::familiar::UnderCommand>,
            Option<&'static ParticipatingIn>,
        ),
        Without<Familiar>,
    >,
    pub q_breakdown: Query<'w, 's, &'static StressBreakdown>,
    pub task_queries: crate::systems::soul_ai::task_execution::context::TaskAssignmentQueries<'w, 's>,
    // resource_cache removed (included in task_queries)
    pub game_assets: Res<'w, crate::assets::GameAssets>,
    pub q_bubbles: Query<'w, 's, (Entity, &'static SpeechBubble), With<FamiliarBubble>>,
    // cooldowns removed (now a component)
    pub ev_state_changed: MessageWriter<'w, crate::events::FamiliarAiStateChangedEvent>,
    pub world_map: Res<'w, crate::world::map::WorldMap>,
}

#[derive(SystemParam)]
pub struct FamiliarAiTaskParams<'w, 's> {
    pub commands: Commands<'w, 's>,
    pub time: Res<'w, Time>,
    pub q_familiars: Query<
        'w,
        's,
        (
            Entity,
            &'static Transform,
            &'static Familiar,
            &'static FamiliarOperation,
            &'static ActiveCommand,
            &'static mut FamiliarAiState,
            &'static mut Destination,
            &'static mut Path,
            Option<&'static TaskArea>,
            Option<&'static Commanding>,
            Option<&'static ManagedTasks>,
        ),
        With<Familiar>,
    >,
    pub q_souls: Query<
        'w,
        's,
        (
            Entity,
            &'static Transform,
            &'static DamnedSoul,
            &'static mut AssignedTask,
            &'static mut Destination,
            &'static mut Path,
            &'static IdleState,
            Option<&'static mut crate::systems::logistics::Inventory>,
            Option<&'static crate::entities::familiar::UnderCommand>,
            Option<&'static ParticipatingIn>,
        ),
        Without<Familiar>,
    >,
    pub task_queries: crate::systems::soul_ai::task_execution::context::TaskAssignmentQueries<'w, 's>,
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
        mut task_queries,
        // resource_cache removed
        game_assets,
        q_bubbles,
        // cooldowns removed
        mut ev_state_changed,
        world_map,
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
        ai_state,
        fam_dest,
        fam_path,
        task_area_opt,
        commanding,
        voice_opt,
        history_opt,
    ) in q_familiars.iter_mut()
    {
        #[allow(clippy::type_complexity)]
        let (
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
        ): (
            Entity,
            &Transform,
            &Familiar,
            &FamiliarOperation,
            &ActiveCommand,
            Mut<FamiliarAiState>,
            Mut<Destination>,
            Mut<Path>,
            Option<&TaskArea>,
            Option<&Commanding>,
            Option<&FamiliarVoice>,
            Option<Mut<crate::systems::visual::speech::cooldown::SpeechHistory>>,
        ) = (
            fam_entity,
            fam_transform,
            familiar,
            familiar_op,
            active_command,
            ai_state,
            fam_dest,
            fam_path,
            task_area_opt,
            commanding,
            voice_opt,
            history_opt,
        );

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
            let transition_result = state_handlers::idle::handle_idle_state(
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
        let mut squad_entities = process_squad_management(
            fam_entity,
            fam_transform,
            familiar_op,
            commanding,
            voice_opt,
            &mut commands,
            &mut q_souls,
            &mut task_queries,
            history_opt.as_deref_mut(),
            &time,
            &game_assets,
            &q_bubbles,
            &world_map,
            // resource_cache arg removed
        );

        // 状態に応じたロジック実行
        match *ai_state {
            FamiliarAiState::Scouting { target_soul } => {
                // Scoutingロジックを実行 (分隊の空き状況に関わらず常にチェック)
                let transition_result = state_handlers::scouting::handle_scouting_state(
                    fam_entity,
                    fam_pos,
                    target_soul,
                    fatigue_threshold,
                    max_workers,
                    &mut squad_entities,
                    &mut ai_state,
                    &mut fam_dest,
                    &mut fam_path,
                    &mut q_souls,
                    &q_breakdown,
                    &mut commands,
                );
                state_changed = transition_result.apply_to(&mut ai_state);
            }
            _ => {
                // スカウト中以外でリクルートを試みる
                if process_recruitment(
                    fam_entity,
                    fam_transform,
                    familiar,
                    familiar_op,
                    &mut ai_state,
                    &mut fam_dest,
                    &mut fam_path,
                    &mut squad_entities,
                    max_workers,
                    &*spatial_grid,
                    &mut q_souls,
                    &q_breakdown,
                    &mut commands,
                ) {
                    state_changed = true;
                }
            }
        }

        // 状態遷移の最終確定
        if finalize_state_transitions(&mut ai_state, &squad_entities, fam_entity, max_workers) {
            state_changed = true;
        }

        if state_changed {
            // 状態遷移イベントを発火（Changed フィルタで検知できない場合の補完）
            ev_state_changed.write(crate::events::FamiliarAiStateChangedEvent {
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
        mut commands,
        time,
        mut q_familiars,
        mut q_souls,
        mut task_queries,
        designation_grid,
        world_map,
        mut pf_context,
        ..
    } = params;

    for (
        fam_entity,
        fam_transform,
        _familiar,
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

        let initial_squad = crate::systems::familiar_ai::squad::SquadManager::build_squad(commanding);
        let (squad_entities, _invalid_members) =
            crate::systems::familiar_ai::squad::SquadManager::validate_squad(
                initial_squad,
                fam_entity,
                &mut q_souls,
            );

        process_task_delegation_and_movement(
            fam_entity,
            fam_transform,
            familiar_op,
            &mut *ai_state,
            &mut *fam_dest,
            &mut *fam_path,
            task_area_opt,
            &squad_entities,
            &mut commands,
            &mut q_souls,
            &mut task_queries,
            &designation_grid,
            managed_tasks,
            &world_map,
            &mut *pf_context,
            &time,
            state_changed,
        );
    }
}
