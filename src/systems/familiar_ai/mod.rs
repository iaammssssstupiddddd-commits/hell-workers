use crate::entities::damned_soul::{
    DamnedSoul, Destination, IdleBehavior, IdleState, Path, StressBreakdown,
};
use crate::entities::familiar::{
    ActiveCommand, Familiar, FamiliarCommand, FamiliarOperation, UnderCommand,
};
use crate::relationships::{Commanding, ManagedTasks, TaskWorkers};
use crate::systems::GameSystemSet;
use crate::systems::command::TaskArea;
use crate::systems::jobs::{IssuedBy, TaskSlots};
use crate::systems::logistics::{ResourceItem, Stockpile};
use crate::systems::spatial::{
    DesignationSpatialGrid, SpatialGrid, update_designation_spatial_grid_system,
};
use crate::systems::work::{AssignedTask, unassign_task};
use bevy::prelude::*;

pub mod following;
pub mod haul_cache;
pub mod helpers;
pub mod scouting;
pub mod searching;
pub mod supervising;

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
            .init_resource::<DesignationSpatialGrid>()
            .add_systems(
                Update,
                (
                    update_designation_spatial_grid_system.in_set(GameSystemSet::Logic),
                    familiar_ai_system.in_set(GameSystemSet::Logic),
                    following::following_familiar_system.in_set(GameSystemSet::Logic),
                ),
            );
    }
}

/// 使い魔AIの更新システム
pub fn familiar_ai_system(
    mut commands: Commands,
    time: Res<Time>,
    spatial_grid: Res<SpatialGrid>,
    mut q_familiars: Query<(
        Entity,
        &Transform,
        &Familiar,
        &FamiliarOperation,
        &ActiveCommand,
        &mut FamiliarAiState,
        &mut Destination,
        &mut Path,
        Option<&TaskArea>,
        Option<&Commanding>,
        &ManagedTasks,
    )>,
    mut q_souls: Query<
        (
            Entity,
            &Transform,
            &DamnedSoul,
            &mut AssignedTask,
            &mut Destination,
            &mut Path,
            &IdleState,
            Option<&crate::relationships::Holding>,
            Option<&crate::entities::familiar::UnderCommand>,
        ),
        Without<Familiar>,
    >,
    q_designations: Query<(
        Entity,
        &Transform,
        &crate::systems::jobs::Designation,
        Option<&IssuedBy>,
        Option<&TaskSlots>,
        Option<&TaskWorkers>,
    )>,
    q_stockpiles: Query<(
        Entity,
        &Transform,
        &Stockpile,
        Option<&crate::relationships::StoredItems>,
    )>,
    _q_souls_lite: Query<(Entity, &UnderCommand), With<DamnedSoul>>,
    q_breakdown: Query<&StressBreakdown>,
    q_resources: Query<&ResourceItem>,
    mut haul_cache: ResMut<haul_cache::HaulReservationCache>,
    designation_grid: Res<DesignationSpatialGrid>,
) {
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
        managed_tasks,
    ) in q_familiars.iter_mut()
    {
        let old_state = ai_state.clone();
        // 1. 基本コマンドチェック
        if matches!(active_command.command, FamiliarCommand::Idle) {
            if *ai_state != FamiliarAiState::Idle {
                *ai_state = FamiliarAiState::Idle;
            }
            fam_dest.0 = fam_transform.translation.truncate();
            fam_path.waypoints.clear();
            continue;
        }

        let fam_pos = fam_transform.translation.truncate();
        let command_radius = familiar.command_radius;
        let fatigue_threshold = familiar_op.fatigue_threshold;

        // Relationshipから現在の部下リストを取得 (Commandingがない場合は空)
        let mut squad_entities: Vec<Entity> = commanding
            .map(|c| c.iter().copied().collect())
            .unwrap_or_default();

        // 2. 疲労解放
        let mut released_entities: Vec<Entity> = Vec::new();
        for &member_entity in &squad_entities {
            if let Ok((entity, transform, soul, mut task, _, mut path, idle, holding_opt, uc)) =
                q_souls.get_mut(member_entity)
            {
                // 整合性チェック: 相手が自分を主人だと思っていないならリストから外す ( Relationship更新遅延対策 )
                if let Some(uc_comp) = uc {
                    if uc_comp.0 != fam_entity {
                        info!(
                            "FAM_AI: {:?} squad member {:?} belongs to another master {:?}",
                            fam_entity, member_entity, uc_comp.0
                        );
                        released_entities.push(member_entity);
                        continue;
                    }
                } else {
                    // ここで即座にリリースせず、1フレーム待つか警告に留める
                    // Relationship の反映がコンポーネントより先に来る可能性があるため
                    debug!(
                        "FAM_AI: {:?} squad member {:?} has no UnderCommand comp yet (waiting sync)",
                        fam_entity, member_entity
                    );
                    //released_entities.push(member_entity); // 一時的にコメントアウト
                    //continue;
                }

                // 疲労・崩壊チェック
                let is_resting = idle.behavior == IdleBehavior::Gathering;
                if (!is_resting && soul.fatigue > fatigue_threshold)
                    || idle.behavior == IdleBehavior::ExhaustedGathering
                {
                    info!(
                        "FAM_AI: {:?} releasing soul {:?} (Fatigue/Exhausted)",
                        fam_entity, member_entity
                    );
                    unassign_task(
                        &mut commands,
                        entity,
                        transform.translation.truncate(),
                        &mut task,
                        &mut path,
                        holding_opt,
                        &q_designations,
                        &mut *haul_cache,
                    );
                    commands.entity(member_entity).remove::<UnderCommand>();
                    released_entities.push(member_entity);
                }
            } else {
                // エンティティが消失している
                released_entities.push(member_entity);
            }
        }
        if !released_entities.is_empty() {
            let was_scouting = matches!(*ai_state, FamiliarAiState::Scouting { .. });
            squad_entities.retain(|e| !released_entities.contains(e));

            // 分隊が空になった瞬間を検知
            if squad_entities.is_empty() {
                info!(
                    "FAM_AI: {:?} squad became empty (was_scouting: {}, state: {:?})",
                    fam_entity, was_scouting, *ai_state
                );
            }
        }

        // 3. 状態に応じたロジック実行
        let max_workers = familiar_op.max_controlled_soul;
        let mut state_changed = false;

        // --- ステートに応じた主要ロジック ---
        match *ai_state {
            FamiliarAiState::Scouting { target_soul } => {
                // Scoutingロジックを実行 (分隊の空き状況に関わらず常にチェック)
                state_changed = scouting::scouting_logic(
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
            }
            _ => {
                // スカウト中以外で分隊に空きがあれば新規リクルートを試みる
                if squad_entities.len() < max_workers {
                    // 近場のリクルート検索 (即時勧誘)
                    if let Some(new_recruit) = helpers::find_best_recruit(
                        fam_pos,
                        fatigue_threshold,
                        0.0,
                        &*spatial_grid,
                        &q_souls,
                        &q_breakdown,
                        Some(command_radius),
                    ) {
                        info!(
                            "FAM_AI: {:?} recruiting nearby soul {:?}",
                            fam_entity, new_recruit
                        );
                        commands
                            .entity(new_recruit)
                            .insert(UnderCommand(fam_entity));
                        squad_entities.push(new_recruit);
                        state_changed = true;
                    }
                    // 遠方のリクルート検索 (Scouting開始)
                    else if let Some(distant_recruit) = helpers::find_best_recruit(
                        fam_pos,
                        fatigue_threshold,
                        0.0,
                        &*spatial_grid,
                        &q_souls,
                        &q_breakdown,
                        None,
                    ) {
                        info!(
                            "FAM_AI: {:?} scouting distant soul {:?}",
                            fam_entity, distant_recruit
                        );
                        *ai_state = FamiliarAiState::Scouting {
                            target_soul: distant_recruit,
                        };
                        state_changed = true;

                        // 即座に移動開始
                        if let Ok((_, target_transform, _, _, _, _, _, _, _)) =
                            q_souls.get(distant_recruit)
                        {
                            let target_pos = target_transform.translation.truncate();
                            fam_dest.0 = target_pos;
                            fam_path.waypoints = vec![target_pos];
                            fam_path.current_index = 0;
                            info!(
                                "FAM_AI: {:?} path set to distant recruit {:?}",
                                fam_entity, distant_recruit
                            );
                        }
                    }
                }
            }
        }

        // --- ステートの最終確定 ---
        if squad_entities.is_empty() {
            if !matches!(
                *ai_state,
                FamiliarAiState::SearchingTask
                    | FamiliarAiState::Idle
                    | FamiliarAiState::Scouting { .. }
            ) {
                *ai_state = FamiliarAiState::SearchingTask;
                state_changed = true;
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
                }
            }
        }

        if state_changed {
            info!(
                "FAM_AI: {:?} state changed: {:?} -> {:?}",
                fam_entity, old_state, *ai_state
            );
        }

        // 4. タスク委譲
        let mut idle_member_opt = None;
        for &member_entity in &squad_entities {
            if let Ok((_, _, soul, task, _, _, idle, _, _)) = q_souls.get(member_entity) {
                if matches!(*task, AssignedTask::None)
                    && idle.behavior != IdleBehavior::ExhaustedGathering
                    && soul.fatigue < fatigue_threshold
                {
                    idle_member_opt = Some(member_entity);
                    break; // 一人ずつ割り当てる
                }
            }
        }

        if let Some(best_idle_member) = idle_member_opt {
            if let Some(task_entity) = helpers::find_unassigned_task_in_area(
                fam_entity,
                fam_pos,
                task_area_opt,
                &q_designations,
                &designation_grid,
                managed_tasks,
            ) {
                helpers::assign_task_to_worker(
                    &mut commands,
                    fam_entity,
                    task_entity,
                    best_idle_member,
                    fatigue_threshold,
                    &q_designations,
                    &mut q_souls,
                    &q_stockpiles,
                    &q_resources,
                    task_area_opt,
                    &mut *haul_cache,
                );
                commands.entity(task_entity).insert(IssuedBy(fam_entity));
            }
        }

        // 5. 移動制御
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
                    if let Ok((_, _, _, _, _, _, idle, _, _)) = q_souls.get(e) {
                        idle.behavior != IdleBehavior::ExhaustedGathering
                    } else {
                        false
                    }
                })
                .copied()
                .collect();

            match *ai_state {
                FamiliarAiState::Supervising { .. } => {
                    supervising::supervising_logic(
                        fam_entity,
                        fam_pos,
                        &active_members,
                        task_area_opt,
                        &time,
                        &mut ai_state,
                        &mut fam_dest,
                        &mut fam_path,
                        &q_souls,
                    );
                }
                FamiliarAiState::SearchingTask => {
                    searching::searching_logic(
                        fam_pos,
                        task_area_opt,
                        &mut fam_dest,
                        &mut fam_path,
                    );
                }
                _ => {}
            }
        }
    }
}
