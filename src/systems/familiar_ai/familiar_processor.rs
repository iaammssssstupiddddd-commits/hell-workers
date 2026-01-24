//! 使い魔AIの処理ロジック
//!
//! `familiar_ai_system` の処理を複数の関数に分割して管理します。

use crate::entities::damned_soul::{DamnedSoul, Destination, IdleBehavior, IdleState, Path};
use crate::entities::familiar::{
    Familiar, FamiliarOperation, FamiliarVoice, UnderCommand,
};
use crate::relationships::{Commanding, ManagedTasks};
use crate::systems::command::TaskArea;
use crate::systems::jobs::DesignationCreatedEvent;
use crate::systems::soul_ai::gathering::ParticipatingIn;
use crate::systems::soul_ai::task_execution::AssignedTask;
use crate::systems::spatial::{DesignationSpatialGrid, SpatialGrid};
use crate::systems::visual::speech::components::{FamiliarBubble, SpeechBubble};
use bevy::prelude::*;

use super::recruitment::RecruitmentManager;
use super::squad::SquadManager;
use super::state_handlers;
use super::task_management::TaskManager;
use super::FamiliarAiState;
use crate::world::map::WorldMap;

/// 分隊管理を実行
pub fn process_squad_management(
    fam_entity: Entity,
    fam_transform: &Transform,
    familiar_op: &FamiliarOperation,
    commanding: Option<&Commanding>,
    voice_opt: Option<&FamiliarVoice>,
    commands: &mut Commands,
    q_souls: &mut Query<
        (
            Entity,
            &Transform,
            &DamnedSoul,
            &mut AssignedTask,
            &mut Destination,
            &mut Path,
            &IdleState,
            Option<&crate::relationships::Holding>,
            Option<&UnderCommand>,
            Option<&ParticipatingIn>,
        ),
        bevy::ecs::query::Without<Familiar>,
    >,
    q_designations: &Query<(
        Entity,
        &Transform,
        &crate::systems::jobs::Designation,
        Option<&crate::systems::jobs::IssuedBy>,
        Option<&crate::systems::jobs::TaskSlots>,
        Option<&crate::relationships::TaskWorkers>,
    )>,
    haul_cache: &mut crate::systems::familiar_ai::haul_cache::HaulReservationCache,
    ev_created: &mut MessageWriter<DesignationCreatedEvent>,
    cooldowns: &mut crate::systems::visual::speech::cooldown::BubbleCooldowns,
    time: &Res<Time>,
    game_assets: &Res<crate::assets::GameAssets>,
    q_bubbles: &Query<(Entity, &SpeechBubble), With<FamiliarBubble>>,
) -> Vec<Entity> {
    let initial_squad = SquadManager::build_squad(commanding);

    // 分隊を検証（無効なメンバーを除外）
    let (mut squad_entities, invalid_members) = SquadManager::validate_squad(
        initial_squad,
        fam_entity,
        q_souls,
    );

    // 疲労・崩壊したメンバーをリリース
    let released_entities = SquadManager::release_fatigued(
        &squad_entities,
        fam_entity,
        familiar_op.fatigue_threshold,
        commands,
        q_souls,
        q_designations,
        haul_cache,
        ev_created,
        cooldowns,
        time,
        game_assets,
        q_bubbles,
        fam_transform,
        voice_opt,
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
pub fn process_recruitment(
    fam_entity: Entity,
    fam_transform: &Transform,
    familiar: &Familiar,
    familiar_op: &FamiliarOperation,
    ai_state: &mut FamiliarAiState,
    fam_dest: &mut Destination,
    fam_path: &mut Path,
    squad_entities: &mut Vec<Entity>,
    max_workers: usize,
    spatial_grid: &SpatialGrid,
    q_souls: &Query<
        (
            Entity,
            &Transform,
            &DamnedSoul,
            &mut AssignedTask,
            &mut Destination,
            &mut Path,
            &IdleState,
            Option<&crate::relationships::Holding>,
            Option<&UnderCommand>,
            Option<&ParticipatingIn>,
        ),
        bevy::ecs::query::Without<Familiar>,
    >,
    q_breakdown: &Query<&crate::entities::damned_soul::StressBreakdown>,
    commands: &mut Commands,
) -> bool {
    let fam_pos = fam_transform.translation.truncate();
    let command_radius = familiar.command_radius;
    let fatigue_threshold = familiar_op.fatigue_threshold;

    // スカウト中以外で分隊に空きがあれば新規リクルートを試みる
    if squad_entities.len() < max_workers {
        // 近場のリクルート検索 (即時勧誘)
        if let Some(new_recruit) = RecruitmentManager::try_immediate_recruit(
            commands,
            fam_entity,
            fam_pos,
            command_radius,
            fatigue_threshold,
            spatial_grid,
            q_souls,
            q_breakdown,
        ) {
            debug!(
                "FAM_AI: {:?} recruiting nearby soul {:?}",
                fam_entity, new_recruit
            );
            squad_entities.push(new_recruit);
            return true;
        }
        // 遠方のリクルート検索 (Scouting開始)
        else {
            if let Some(distant_recruit) = RecruitmentManager::start_scouting(
                fam_pos,
                fatigue_threshold,
                spatial_grid,
                q_souls,
                q_breakdown,
            ) {
                debug!(
                    "FAM_AI: {:?} scouting distant soul {:?}",
                    fam_entity, distant_recruit
                );
                *ai_state = FamiliarAiState::Scouting {
                    target_soul: distant_recruit,
                };

                // 即座に移動開始
                if let Ok((_, target_transform, _, _, _, _, _, _, _, _)) =
                    q_souls.get(distant_recruit)
                {
                    let target_pos = target_transform.translation.truncate();
                    fam_dest.0 = target_pos;
                    fam_path.waypoints = vec![target_pos];
                    fam_path.current_index = 0;
                }
                return true;
            } else {
                // 何も見つからなければログに出す (デバッグ用)
                debug!("FAM_AI: {:?} No recruitable souls found", fam_entity);
            }
        }
    } else {
        debug!(
            "FAM_AI: {:?} Squad full ({}/{})",
            fam_entity,
            squad_entities.len(),
            max_workers
        );
    }
    false
}

/// 状態遷移の最終確定
pub fn finalize_state_transitions(
    ai_state: &mut FamiliarAiState,
    squad_entities: &[Entity],
    fam_entity: Entity,
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
        // メンバーがいるなら、スカウト中でない限り監視モードを維持
        if !matches!(*ai_state, FamiliarAiState::Scouting { .. }) {
            if !matches!(*ai_state, FamiliarAiState::Supervising { .. }) {
                *ai_state = FamiliarAiState::Supervising {
                    target: None,
                    timer: 0.0,
                };
                state_changed = true;
                info!("FAM_AI: {:?} squad not empty. -> Supervising", fam_entity);
            }
        }
    }

    state_changed
}

/// タスク委譲と移動制御を実行
pub fn process_task_delegation_and_movement(
    fam_entity: Entity,
    fam_transform: &Transform,
    familiar_op: &FamiliarOperation,
    ai_state: &mut FamiliarAiState,
    fam_dest: &mut Destination,
    fam_path: &mut Path,
    task_area_opt: Option<&TaskArea>,
    squad_entities: &[Entity],
    commands: &mut Commands,
    q_souls: &mut Query<
        (
            Entity,
            &Transform,
            &DamnedSoul,
            &mut AssignedTask,
            &mut Destination,
            &mut Path,
            &IdleState,
            Option<&crate::relationships::Holding>,
            Option<&UnderCommand>,
            Option<&ParticipatingIn>,
        ),
        bevy::ecs::query::Without<Familiar>,
    >,
    q_designations: &Query<(
        Entity,
        &Transform,
        &crate::systems::jobs::Designation,
        Option<&crate::systems::jobs::IssuedBy>,
        Option<&crate::systems::jobs::TaskSlots>,
        Option<&crate::relationships::TaskWorkers>,
    )>,
    q_stockpiles: &Query<(
        Entity,
        &Transform,
        &crate::systems::logistics::Stockpile,
        Option<&crate::relationships::StoredItems>,
    )>,
    q_resources: &Query<&crate::systems::logistics::ResourceItem>,
    q_target_blueprints: &Query<&crate::systems::jobs::TargetBlueprint>,
    q_blueprints: &Query<&crate::systems::jobs::Blueprint>,
    designation_grid: &DesignationSpatialGrid,
    managed_tasks: &ManagedTasks,
    haul_cache: &mut crate::systems::familiar_ai::haul_cache::HaulReservationCache,
    world_map: &WorldMap,
    time: &Res<Time>,
    state_changed: bool,
) {
    let fam_pos = fam_transform.translation.truncate();
    let fatigue_threshold = familiar_op.fatigue_threshold;

    // タスク委譲
    let assigned_task_opt = TaskManager::delegate_task(
        commands,
        fam_entity,
        fam_pos,
        squad_entities,
        task_area_opt,
        fatigue_threshold,
        q_designations,
        q_souls,
        q_stockpiles,
        q_resources,
        q_target_blueprints,
        q_blueprints,
        designation_grid,
        managed_tasks,
        haul_cache,
        world_map,
    );
    let has_available_task = assigned_task_opt.is_some();

    // 移動制御
    // state_changed があっても、Supervising/SearchingTask なら各ロジックを呼ぶ
    if !state_changed
        || matches!(
            *ai_state,
            FamiliarAiState::Supervising { .. } | FamiliarAiState::SearchingTask
        )
    {
        let active_members: Vec<Entity> = squad_entities
            .iter()
            .filter(|&&e| {
                if let Ok((_, _, _, _, _, _, idle, _, _, _)) = q_souls.get(e) {
                    idle.behavior != IdleBehavior::ExhaustedGathering
                } else {
                    false
                }
            })
            .copied()
            .collect();

        debug!(
            "FAM_AI: Movement control - state: {:?}, active_members: {}, has_available_task: {}, state_changed: {}",
            *ai_state,
            active_members.len(),
            has_available_task,
            state_changed
        );

        match *ai_state {
            FamiliarAiState::Supervising { .. } => {
                state_handlers::supervising::handle_supervising_state(
                    fam_entity,
                    fam_pos,
                    &active_members,
                    task_area_opt,
                    time,
                    ai_state,
                    fam_dest,
                    fam_path,
                    q_souls,
                    has_available_task,
                );
            }
            FamiliarAiState::SearchingTask => {
                debug!("FAM_AI: {:?} executing SearchingTask logic", fam_entity);
                state_handlers::searching::handle_searching_task_state(
                    fam_entity,
                    fam_pos,
                    task_area_opt,
                    fam_dest,
                    fam_path,
                );
            }
            _ => {}
        }
    }
}
