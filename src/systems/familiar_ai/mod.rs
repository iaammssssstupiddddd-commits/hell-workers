use crate::entities::damned_soul::{DamnedSoul, Destination, IdleState, Path, StressBreakdown};
use crate::entities::familiar::{
    ActiveCommand, Familiar, FamiliarCommand, FamiliarOperation, FamiliarVoice, UnderCommand,
};
use crate::relationships::TaskWorkers;
use crate::relationships::{Commanding, ManagedTasks};
use crate::systems::GameSystemSet;
use crate::systems::command::TaskArea;
use crate::systems::jobs::{Blueprint, IssuedBy, TargetBlueprint, TaskSlots};
use crate::systems::logistics::{ResourceItem, Stockpile};
use crate::systems::soul_ai::gathering::ParticipatingIn;
use crate::systems::soul_ai::task_execution::AssignedTask;
use crate::systems::spatial::{
    DesignationSpatialGrid, SpatialGrid, update_designation_spatial_grid_system,
};
use crate::systems::visual::speech::components::{FamiliarBubble, SpeechBubble};
use crate::world::pathfinding::PathfindingContext;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;

pub mod encouragement; // 新規追加
pub mod familiar_processor;
pub mod following;
pub mod haul_cache;
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
        app.register_type::<FamiliarAiState>()
            .init_resource::<haul_cache::HaulReservationCache>()
            .init_resource::<encouragement::EncouragementCooldowns>()
            .init_resource::<DesignationSpatialGrid>()
            .init_resource::<state_transition::PreviousFamiliarAiStates>()
            .add_systems(
                Update,
                (
                    // 状態遷移の検知（Changed フィルタを使用）
                    state_transition::detect_state_changes_system
                        .in_set(GameSystemSet::Logic)
                        .before(familiar_ai_system),
                    state_transition::detect_command_changes_system
                        .in_set(GameSystemSet::Logic)
                        .before(familiar_ai_system),
                    // メインのAIシステム
                    update_designation_spatial_grid_system.in_set(GameSystemSet::Logic),
                    familiar_ai_system.in_set(GameSystemSet::Logic),
                    max_soul_handler::handle_max_soul_changed_system.in_set(GameSystemSet::Logic),
                    following::following_familiar_system.in_set(GameSystemSet::Logic),
                    encouragement::encouragement_system.in_set(GameSystemSet::Logic),
                    // 状態遷移イベントの処理
                    state_transition::handle_state_changed_system.in_set(GameSystemSet::Logic),
                    // クリーンアップ
                    state_transition::cleanup_previous_states_system.in_set(GameSystemSet::Logic),
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
            Option<&'static ManagedTasks>,
            Option<&'static FamiliarVoice>,
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
            Option<&'static crate::relationships::Holding>,
            Option<&'static crate::entities::familiar::UnderCommand>,
            Option<&'static ParticipatingIn>,
        ),
        Without<Familiar>,
    >,
    pub q_designations: Query<
        'w,
        's,
        (
            Entity,
            &'static Transform,
            &'static crate::systems::jobs::Designation,
            Option<&'static IssuedBy>,
            Option<&'static TaskSlots>,
            Option<&'static TaskWorkers>,
        ),
    >,
    pub q_stockpiles: Query<
        'w,
        's,
        (
            Entity,
            &'static Transform,
            &'static Stockpile,
            Option<&'static crate::relationships::StoredItems>,
        ),
    >,
    pub _q_souls_lite: Query<'w, 's, (Entity, &'static UnderCommand), With<DamnedSoul>>,
    pub q_breakdown: Query<'w, 's, &'static StressBreakdown>,
    pub q_resources: Query<'w, 's, &'static ResourceItem>,
    pub q_target_blueprints: Query<'w, 's, &'static TargetBlueprint>,
    pub q_blueprints: Query<'w, 's, &'static Blueprint>,
    pub haul_cache: ResMut<'w, haul_cache::HaulReservationCache>,
    pub designation_grid: Res<'w, DesignationSpatialGrid>,
    pub game_assets: Res<'w, crate::assets::GameAssets>,
    pub q_bubbles: Query<'w, 's, (Entity, &'static SpeechBubble), With<FamiliarBubble>>,
    pub cooldowns: ResMut<'w, crate::systems::visual::speech::cooldown::BubbleCooldowns>,
    pub ev_created: MessageWriter<'w, crate::systems::jobs::DesignationCreatedEvent>,
    pub ev_state_changed: MessageWriter<'w, crate::events::FamiliarAiStateChangedEvent>,
    pub world_map: Res<'w, crate::world::map::WorldMap>,
    pub pf_context: ResMut<'w, PathfindingContext>,
}

/// 使い魔AIの更新システム
pub fn familiar_ai_system(params: FamiliarAiParams) {
    let FamiliarAiParams {
        mut commands,
        time,
        spatial_grid,
        mut q_familiars,
        mut q_souls,
        q_designations,
        q_stockpiles,
        _q_souls_lite,
        q_breakdown,
        q_resources,
        q_target_blueprints,
        q_blueprints,
        mut haul_cache,
        designation_grid,
        game_assets,
        q_bubbles,
        mut cooldowns,
        mut ev_created,
        mut ev_state_changed,
        world_map,
        mut pf_context,
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
        managed_tasks_opt,
        voice_opt,
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
            managed_tasks_opt,
            voice_opt,
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
            Option<&ManagedTasks>,
            Option<&FamiliarVoice>,
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
            managed_tasks_opt,
            voice_opt,
        );
        let default_tasks = crate::relationships::ManagedTasks::default();
        let managed_tasks = managed_tasks_opt.unwrap_or(&default_tasks);

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
                &mut cooldowns,
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
            &q_designations,
            &mut *haul_cache,
            &mut ev_created,
            &mut cooldowns,
            &time,
            &game_assets,
            &q_bubbles,
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
                    &q_souls,
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
                    &q_souls,
                    &q_breakdown,
                    &mut commands,
                ) {
                    state_changed = true;
                }
            }
        }

        // 状態遷移の最終確定
        if finalize_state_transitions(&mut ai_state, &squad_entities, fam_entity) {
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

        // タスク委譲と移動制御
        process_task_delegation_and_movement(
            fam_entity,
            fam_transform,
            familiar_op,
            &mut ai_state,
            &mut fam_dest,
            &mut fam_path,
            task_area_opt,
            &squad_entities,
            &mut commands,
            &mut q_souls,
            &q_designations,
            &q_stockpiles,
            &q_resources,
            &q_target_blueprints,
            &q_blueprints,
            &designation_grid,
            managed_tasks,
            &mut *haul_cache,
            &world_map,
            &mut *pf_context,
            &time,
            state_changed,
        );
    }
}
