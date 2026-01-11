//! 使い魔AI システムモジュール
//!
//! 使い魔の自律行動（ステートマシン）を管理します。

use crate::constants::*;
use crate::entities::damned_soul::{
    DamnedSoul, Destination, IdleBehavior, IdleState, Path, StressBreakdown,
};
use crate::entities::familiar::{
    ActiveCommand, Familiar, FamiliarCommand, FamiliarOperation, UnderCommand,
};
use crate::events::OnSoulRecruited;
use crate::systems::command::TaskArea;
use crate::systems::jobs::{Designation, IssuedBy, TaskSlots, WorkType};
use crate::systems::logistics::{ClaimedBy, Stockpile};
use crate::systems::spatial::SpatialGrid;
use crate::systems::work::{AssignedTask, GatherPhase, HaulPhase, unassign_task};
use bevy::prelude::*;

// ============================================================
// ステートマシン定義
// ============================================================

/// 使い魔のAI状態
///
/// 使い魔が何をしているかを詳細に管理します。
/// `FamiliarCommand` がプレイヤーからの「指示」であるのに対し、
/// `FamiliarAiState` はその指示を遂行するための「現在の行動」を表します。
#[derive(Component, Debug, Clone, PartialEq)]
pub enum FamiliarAiState {
    /// 待機中 - 巡回や特定の場所で待機
    Idle,

    /// タスク探索中 - 担当エリア内で仕事を探している or 配下の暇人に指示を出そうとしている
    SearchingTask,

    /// スカウト中 - 遠方にいる新しいワーカーを確保しに向かっている
    Scouting { target_soul: Entity },

    /// 監視中 - 作業中のワーカーを見守っている
    Supervising,
}

impl Default for FamiliarAiState {
    fn default() -> Self {
        Self::Idle
    }
}

// ============================================================
// メインAIシステム
// ============================================================

/// 使い魔AIの更新システム
///
/// ステートマシンに基づいて使い魔の行動を制御します。
pub fn familiar_ai_system(
    mut commands: Commands,
    _time: Res<Time>,
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
            &mut crate::systems::logistics::Inventory,
            Option<&UnderCommand>,
        ),
        Without<Familiar>,
    >,
    mut q_designations: Query<(
        Entity,
        &Transform,
        &Designation,
        Option<&IssuedBy>,
        Option<&mut TaskSlots>,
    )>,
    q_stockpiles: Query<(Entity, &Transform, &Stockpile)>,
    q_souls_lite: Query<(Entity, &UnderCommand), With<DamnedSoul>>,
    q_breakdown: Query<&StressBreakdown>,
) {
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
    ) in q_familiars.iter_mut()
    {
        // 使い魔が Idle の場合は AI 処理をスキップ（部下を持つのをやめる指示のため）
        if matches!(active_command.command, FamiliarCommand::Idle) {
            continue;
        }

        let fam_pos = fam_transform.translation.truncate();
        let command_radius = familiar.command_radius;

        // 管理下のワーカー（分隊メンバー）を特定
        let mut squad_members_entities: Vec<Entity> = q_souls_lite
            .iter()
            .filter(|(_, uc)| uc.0 == fam_entity)
            .map(|(e, _)| e)
            .collect();

        // 使い魔の設定した疲労閾値を取得
        let fatigue_threshold = familiar_op.fatigue_threshold;

        // 部下の疲労をチェックし、閾値を超えたら使役を解除
        let mut released_entities: Vec<Entity> = Vec::new();
        for &member_entity in &squad_members_entities {
            if let Ok((entity, transform, soul, mut task, _, mut path, idle, mut inventory, _)) =
                q_souls.get_mut(member_entity)
            {
                // 疲労が閾値を超えている、または ExhaustedGathering 状態なら解除
                if soul.fatigue > fatigue_threshold
                    || idle.behavior == IdleBehavior::ExhaustedGathering
                {
                    info!(
                        "FAMILIAR_AI: {:?} releasing soul {:?} due to fatigue/exhaustion (fatigue: {:.1}%, behavior: {:?})",
                        fam_entity,
                        member_entity,
                        soul.fatigue * 100.0,
                        idle.behavior
                    );

                    // タスクを適切に解除（アイテムがあればドロップ）
                    unassign_task(
                        &mut commands,
                        entity,
                        transform.translation.truncate(),
                        &mut task,
                        &mut path,
                        &mut inventory,
                        &mut q_designations,
                    );

                    commands.entity(member_entity).remove::<UnderCommand>();
                    released_entities.push(member_entity);
                }
            }
        }

        // 解除したメンバーをローカルリストから除外（コマンドバッファ適用を待たずに即座に反映）
        if !released_entities.is_empty() {
            squad_members_entities.retain(|e| !released_entities.contains(e));
        }

        // --------------------------------------------------------
        // 優先度1: 使役数の確保 (Scouting / Recruitment)
        // --------------------------------------------------------

        // コマンドが Idle ならリクルート（スカウト・勧誘）を行わない
        if matches!(active_command.command, FamiliarCommand::Idle) {
            if *ai_state != FamiliarAiState::Idle {
                info!(
                    "FAMILIAR_AI: {:?} is now Idle (squad: {})",
                    fam_entity,
                    squad_members_entities.len()
                );
                *ai_state = FamiliarAiState::Idle;
            }

            // スカウト中だった場合は中断
            fam_dest.0 = fam_pos;
            fam_path.waypoints.clear();
            continue;
        }

        let mut force_dest = None;
        let max_workers = familiar_op.max_controlled_soul;

        if squad_members_entities.len() < max_workers {
            if let FamiliarAiState::Scouting { target_soul } = *ai_state {
                if let Ok((_soul_entity, target_transform, soul, task, _, _, idle, _, uc)) =
                    q_souls.get(target_soul)
                {
                    // 疲労が閾値未満のsoulを探す（疲労が低いsoulも使役可能）
                    let fatigue_ok = soul.fatigue < fatigue_threshold;
                    // ブレイクダウン中は使役不可
                    let stress_ok = q_breakdown.get(target_soul).is_err();

                    if uc.is_none()
                        && matches!(*task, AssignedTask::None)
                        && fatigue_ok
                        && stress_ok
                        && idle.behavior != IdleBehavior::ExhaustedGathering
                    {
                        let target_pos = target_transform.translation.truncate();
                        let dist = fam_pos.distance(target_pos);
                        if dist < TILE_SIZE * 5.0 {
                            info!(
                                "FAMILIAR_AI: {:?} arrived and RECRUITED soul {:?}",
                                fam_entity, target_soul
                            );
                            commands
                                .entity(target_soul)
                                .insert(UnderCommand(fam_entity));

                            // Bevy 0.17 の Observer をトリガー
                            commands.trigger(OnSoulRecruited {
                                entity: target_soul,
                                familiar_entity: fam_entity,
                            });
                            squad_members_entities.push(target_soul);

                            // リクルート後、分隊が満員になった場合は監視モードに移行
                            if squad_members_entities.len() >= max_workers {
                                *ai_state = FamiliarAiState::Supervising;
                            } else {
                                *ai_state = FamiliarAiState::SearchingTask;
                            }
                        } else {
                            force_dest = Some(target_pos);
                        }
                    } else {
                        info!(
                            "FAMILIAR_AI: {:?} lost scouting target {:?}",
                            fam_entity, target_soul
                        );
                        *ai_state = FamiliarAiState::SearchingTask;
                    }
                } else {
                    *ai_state = FamiliarAiState::SearchingTask;
                }
            }

            if !matches!(*ai_state, FamiliarAiState::Scouting { .. })
                && squad_members_entities.len() < max_workers
            {
                // 影響範囲内のワーカーのみリクルート可能
                if let Some(new_recruit) = find_best_recruit(
                    fam_pos,
                    fatigue_threshold,
                    0.0, // 互換性のため（実際には使用しない）
                    &*spatial_grid,
                    &q_souls,
                    &q_breakdown,
                    Some(command_radius), // 影響範囲内のみ
                ) {
                    info!(
                        "FAMILIAR_AI: {:?} RECRUITED soul {:?} within command radius (squad: {}/{})",
                        fam_entity,
                        new_recruit,
                        squad_members_entities.len() + 1,
                        max_workers
                    );
                    commands
                        .entity(new_recruit)
                        .insert(UnderCommand(fam_entity));
                    squad_members_entities.push(new_recruit);

                    // リクルート後、分隊が満員になった場合は監視モードに移行
                    if squad_members_entities.len() >= max_workers {
                        *ai_state = FamiliarAiState::Supervising;
                    } else {
                        *ai_state = FamiliarAiState::SearchingTask;
                    }
                } else if squad_members_entities.len() < max_workers {
                    // 影響範囲外のワーカーを探してスカウト（移動）する
                    if let Some(distant_recruit) = find_best_recruit(
                        fam_pos,
                        fatigue_threshold,
                        0.0,
                        &*spatial_grid,
                        &q_souls,
                        &q_breakdown,
                        None, // 全域検索
                    ) {
                        info!(
                            "FAMILIAR_AI: {:?} spotted soul {:?} outside range, moving to recruit ({}/{})",
                            fam_entity,
                            distant_recruit,
                            squad_members_entities.len(),
                            max_workers
                        );
                        *ai_state = FamiliarAiState::Scouting {
                            target_soul: distant_recruit,
                        };
                        if let Ok((_, t, _, _, _, _, _, _, _)) = q_souls.get(distant_recruit) {
                            force_dest = Some(t.translation.truncate());
                        }
                    }
                }
            }
        } else if matches!(*ai_state, FamiliarAiState::Scouting { .. }) {
            // スカウト中だが分隊が満員になった場合
            if squad_members_entities.len() >= max_workers {
                *ai_state = FamiliarAiState::Supervising;
            } else {
                *ai_state = FamiliarAiState::SearchingTask;
            }
        }

        // 分隊が満員になった場合は監視モードに移行 (二重チェックになるが安全のため維持)
        if squad_members_entities.len() >= max_workers {
            if *ai_state != FamiliarAiState::Supervising {
                info!(
                    "FAMILIAR_AI: {:?} transitioned to Supervising (squad full: {}/{})",
                    fam_entity,
                    squad_members_entities.len(),
                    max_workers
                );
                *ai_state = FamiliarAiState::Supervising;
            }
        }

        // --------------------------------------------------------
        // 優先度2: タスクの移譲 (Delegation)
        // --------------------------------------------------------
        // 部下の中で暇な人を探す（ExhaustedGathering状態は除外、疲労閾値未満のみ）
        let mut idle_members = Vec::new();
        for &member_entity in &squad_members_entities {
            if let Ok((_, _, soul, task, _, _, idle, _, _)) = q_souls.get(member_entity) {
                if matches!(*task, AssignedTask::None)
                    && idle.behavior != IdleBehavior::ExhaustedGathering
                    && soul.fatigue < fatigue_threshold
                {
                    idle_members.push(member_entity);
                }
            }
        }

        if !idle_members.is_empty() {
            // 暇な部下がいるなら、タスクを探して割り当てる
            if let Some(task_entity) = find_unassigned_task_in_area(
                fam_entity,
                fam_pos,
                task_area_opt,
                &q_designations,
                familiar,
                &q_souls_lite,
                Some(familiar_op),
            ) {
                // 最も近い暇な部下を選ぶ
                let best_idle_member = idle_members
                    .into_iter()
                    .min_by(|&e1, &e2| {
                        let p1 = q_souls
                            .get(e1)
                            .map(|(_, t, _, _, _, _, _, _, _)| t.translation.truncate())
                            .unwrap_or(Vec2::ZERO);
                        let p2 = q_souls
                            .get(e2)
                            .map(|(_, t, _, _, _, _, _, _, _)| t.translation.truncate())
                            .unwrap_or(Vec2::ZERO);
                        p1.distance_squared(fam_pos)
                            .partial_cmp(&p2.distance_squared(fam_pos))
                            .unwrap_or(std::cmp::Ordering::Equal)
                    })
                    .unwrap();

                info!(
                    "FAMILIAR_AI: {:?} assigning task {:?} to member {:?}",
                    fam_entity, task_entity, best_idle_member
                );
                assign_task_to_worker(
                    &mut commands,
                    fam_entity,
                    task_entity,
                    best_idle_member,
                    fatigue_threshold,
                    &mut q_designations,
                    &mut q_souls,
                    &q_stockpiles,
                );
                commands.entity(task_entity).insert(IssuedBy(fam_entity));
            }
        }

        // --------------------------------------------------------
        // 移動制御 (Movement)
        // --------------------------------------------------------
        if let Some(dest) = force_dest {
            // スカウトなどの強制移動を優先
            fam_dest.0 = dest;
            fam_path.waypoints = vec![dest];
            fam_path.current_index = 0;
        } else if squad_members_entities.len() >= max_workers {
            // 全員揃っていて、かつスカウト中ではない
            // ただし、有効なメンバー（ExhaustedGathering でない）がいるか確認
            let active_members: Vec<Entity> = squad_members_entities
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

            if active_members.is_empty() {
                // 全員が疲労休息中 → 拠点で待機
                if *ai_state != FamiliarAiState::SearchingTask {
                    info!(
                        "FAMILIAR_AI: {:?} all members exhausted, waiting at base",
                        fam_entity
                    );
                    *ai_state = FamiliarAiState::SearchingTask;
                }
                if let Some(area) = task_area_opt {
                    let center = (area.min + area.max) * 0.5;
                    if fam_pos.distance_squared(center) > (TILE_SIZE * 3.0).powi(2) {
                        fam_dest.0 = center;
                        fam_path.waypoints = vec![center];
                        fam_path.current_index = 0;
                    }
                }
            } else {
                // 有効なメンバーがいる → 監視モード
                if *ai_state != FamiliarAiState::Supervising {
                    info!("FAMILIAR_AI: {:?} transitioned to Supervising", fam_entity);
                    *ai_state = FamiliarAiState::Supervising;
                }

                // 監視移動（作業中のワーカーを優先して追尾）
                // まずタスク中のワーカーを探す、いなければアクティブメンバーの一人目
                let target_worker = active_members
                    .iter()
                    .find(|&&e| {
                        if let Ok((_, _, _, task, _, _, _, _, _)) = q_souls.get(e) {
                            !matches!(*task, AssignedTask::None)
                        } else {
                            false
                        }
                    })
                    .or(active_members.first());

                if let Some(&target) = target_worker {
                    if let Ok((_, worker_transform, _, _, _, _, _, _, _)) = q_souls.get(target) {
                        let worker_pos = worker_transform.translation.truncate();
                        // 距離閾値を緩和し、リアルタイムで追跡
                        if fam_pos.distance(worker_pos) > TILE_SIZE * 5.0 {
                            fam_dest.0 = worker_pos;
                            fam_path.waypoints = vec![worker_pos];
                            fam_path.current_index = 0;
                        }
                    }
                }
            }
        } else {
            // 分隊が満員でない → リクルートを探すか拠点で待機
            // SearchingTask 以外の状態（Scoutingが失敗した場合など）でも拠点に戻れるようにする
            if let Some(area) = task_area_opt {
                let center = (area.min + area.max) * 0.5;
                if fam_pos.distance_squared(center) > (TILE_SIZE * 5.0).powi(2) {
                    fam_dest.0 = center;
                    fam_path.waypoints = vec![center];
                    fam_path.current_index = 0;
                }
            }
        }
    }
}

// ============================================================
// ヘルパー関数
// ============================================================

/// 最も近い「フリーな」ワーカーをスカウト対象として探す
fn find_best_recruit(
    fam_pos: Vec2,
    fatigue_threshold: f32,
    _min_fatigue: f32, // 互換性のために残す（実際には使用しない）
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
            &mut crate::systems::logistics::Inventory,
            Option<&UnderCommand>,
        ),
        Without<Familiar>,
    >,
    q_breakdown: &Query<&StressBreakdown>,
    radius_opt: Option<f32>,
) -> Option<Entity> {
    let mut candidates = Vec::new();

    if let Some(radius) = radius_opt {
        // 近くを検索
        let nearby = spatial_grid.get_nearby_in_radius(fam_pos, radius);
        for &e in &nearby {
            if let Ok((entity, transform, soul, task, _, _, idle, _, uc)) = q_souls.get(e) {
                // Gathering状態（回復中）なら疲労チェックをスキップ
                let is_gathering = idle.behavior == IdleBehavior::Gathering;
                let fatigue_ok = is_gathering || soul.fatigue < fatigue_threshold;
                // ブレイクダウン中は使役不可
                let stress_ok = q_breakdown.get(entity).is_err();

                if uc.is_none()
                    && matches!(*task, AssignedTask::None)
                    && fatigue_ok
                    && stress_ok
                    && idle.behavior != IdleBehavior::ExhaustedGathering
                {
                    candidates.push((entity, transform.translation.truncate()));
                }
            }
        }
    } else {
        // 全域検索
        for (entity, transform, soul, task, _, _, idle, _, uc) in q_souls.iter() {
            // Gathering状態（回復中）なら疲労チェックをスキップ
            let is_gathering = idle.behavior == IdleBehavior::Gathering;
            let fatigue_ok = if is_gathering {
                true // 回復中なので疲労に関係なくリクルート可能
            } else {
                // 疲労が閾値未満
                soul.fatigue < fatigue_threshold
            };
            // ブレイクダウン中は使役不可
            let stress_ok = q_breakdown.get(entity).is_err();

            let is_free = uc.is_none();
            let has_no_task = matches!(*task, AssignedTask::None);
            let not_exhausted = idle.behavior != IdleBehavior::ExhaustedGathering;

            if is_free && has_no_task && fatigue_ok && stress_ok && not_exhausted {
                candidates.push((entity, transform.translation.truncate()));
            } else {
                // デバッグ: なぜ除外されたか
                info!(
                    "FIND_RECRUIT: Soul {:?} rejected - free:{}, no_task:{}, fatigue_ok:{} (fatigue:{:.1}%, behavior:{:?}), stress_ok:{}, not_exhausted:{}",
                    entity,
                    is_free,
                    has_no_task,
                    fatigue_ok,
                    soul.fatigue * 100.0,
                    idle.behavior,
                    stress_ok,
                    not_exhausted
                );
            }
        }
    }

    // デバッグ: 候補数をログ（ただし候補がいない場合のみ）
    if candidates.is_empty() {
        info!(
            "FIND_RECRUIT: No candidates found (threshold: {:.1}%)",
            fatigue_threshold * 100.0
        );
    }

    // 最も近い候補を返す
    candidates
        .into_iter()
        .min_by(|(_, p1), (_, p2)| {
            p1.distance_squared(fam_pos)
                .partial_cmp(&p2.distance_squared(fam_pos))
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(e, _)| e)
}

/// 担当エリア内の未アサインタスクを探す
fn find_unassigned_task_in_area(
    fam_entity: Entity,
    fam_pos: Vec2,
    task_area_opt: Option<&TaskArea>,
    q_designations: &Query<(
        Entity,
        &Transform,
        &Designation,
        Option<&IssuedBy>,
        Option<&mut TaskSlots>,
    )>,
    _familiar: &Familiar,
    _q_souls: &Query<(Entity, &UnderCommand), With<DamnedSoul>>,
    _familiar_op: Option<&FamiliarOperation>,
) -> Option<Entity> {
    let mut best_task = None;
    let mut best_dist = f32::MAX;

    for (entity, transform, _designation, issued_by_opt, slots_opt) in q_designations.iter() {
        let pos = transform.translation.truncate();

        // 自分が担当しているタスクか、誰も担当していないタスクのみ
        let is_mine = issued_by_opt.map(|ib| ib.0 == fam_entity).unwrap_or(false);
        let is_unassigned = issued_by_opt.is_none();

        if !is_mine && !is_unassigned {
            continue;
        }

        // エリア内か確認 (2.0タイル分のマージンを持たせる)
        let mut in_area = true;
        if let Some(area) = task_area_opt {
            let margin = TILE_SIZE * 2.0;
            if pos.x < area.min.x - margin
                || pos.x > area.max.x + margin
                || pos.y < area.min.y - margin
                || pos.y > area.max.y + margin
            {
                in_area = false;
            }
        }

        if !is_mine && !in_area {
            continue;
        }

        // スロットに空きがあるか
        let has_slot = slots_opt.as_ref().map(|s| s.has_slot()).unwrap_or(true);
        if !has_slot {
            continue;
        }

        let dist = fam_pos.distance_squared(pos);
        if dist < best_dist {
            best_dist = dist;
            best_task = Some(entity);
        }
    }

    best_task
}

/// ワーカーにタスクを割り当てる
fn assign_task_to_worker(
    commands: &mut Commands,
    fam_entity: Entity,
    task_entity: Entity,
    worker_entity: Entity,
    fatigue_threshold: f32,
    q_designations: &mut Query<(
        Entity,
        &Transform,
        &Designation,
        Option<&IssuedBy>,
        Option<&mut TaskSlots>,
    )>,
    q_souls: &mut Query<
        (
            Entity,
            &Transform,
            &DamnedSoul,
            &mut AssignedTask,
            &mut Destination,
            &mut Path,
            &IdleState,
            &mut crate::systems::logistics::Inventory,
            Option<&UnderCommand>,
        ),
        Without<Familiar>,
    >,
    q_stockpiles: &Query<(Entity, &Transform, &Stockpile)>,
) {
    let Ok((_, _, soul, mut assigned_task, mut dest, mut path, idle, _, _)) =
        q_souls.get_mut(worker_entity)
    else {
        return;
    };

    // ExhaustedGathering状態の魂は使役しない
    if idle.behavior == IdleBehavior::ExhaustedGathering {
        return;
    }

    // 疲労が閾値を超えている場合は使役しない
    if soul.fatigue >= fatigue_threshold {
        info!(
            "FAMILIAR_AI: {:?} cannot assign task to soul {:?} (fatigue: {:.1}% >= threshold: {:.1}%)",
            fam_entity,
            worker_entity,
            soul.fatigue * 100.0,
            fatigue_threshold * 100.0
        );
        return;
    }

    let task_pos = if let Ok((_, transform, _, _, _)) = q_designations.get(task_entity) {
        transform.translation.truncate()
    } else {
        return;
    };

    let designation = if let Ok((_, _, designation, _, _)) = q_designations.get(task_entity) {
        designation
    } else {
        return;
    };

    let work_type = designation.work_type;

    // タスクを割り当て
    match work_type {
        WorkType::Chop | WorkType::Mine => {
            *assigned_task = AssignedTask::Gather {
                target: task_entity,
                work_type,
                phase: GatherPhase::GoingToResource,
            };
        }
        WorkType::Haul => {
            // 最も近い備蓄場所を探す
            let best_stockpile = q_stockpiles
                .iter()
                .min_by(|(_, t1, _), (_, t2, _)| {
                    let d1 = t1.translation.truncate().distance_squared(task_pos);
                    let d2 = t2.translation.truncate().distance_squared(task_pos);
                    d1.partial_cmp(&d2).unwrap_or(std::cmp::Ordering::Equal)
                })
                .map(|(e, _, _)| e);

            if let Some(stock_entity) = best_stockpile {
                *assigned_task = AssignedTask::Haul {
                    item: task_entity,
                    stockpile: stock_entity,
                    phase: HaulPhase::GoingToItem,
                };
            } else {
                // 備蓄場所がない場合は割り当て不可
                return;
            }
        }
        _ => return,
    }

    // 確定後にスロットを増やす
    if let Ok((_, _, _, _, mut slots_opt)) = q_designations.get_mut(task_entity) {
        if let Some(ref mut slots) = slots_opt {
            slots.current += 1;
        }
    }

    dest.0 = task_pos;
    path.waypoints.clear();

    // 使い魔の管理下に入れる
    commands
        .entity(worker_entity)
        .insert(crate::entities::familiar::UnderCommand(fam_entity));

    // タスクのスロットを埋める
    commands
        .entity(task_entity)
        .insert(ClaimedBy(worker_entity));
}

// ============================================================
// 追従システム
// ============================================================

/// 部下が使い魔を追尾するシステム
///
/// `UnderCommand` を持つソウルがタスクなし（Idle）の場合、
/// 使い魔の近くに集まるように移動します。
pub fn following_familiar_system(
    mut q_souls: Query<
        (
            Entity,
            &Transform,
            &AssignedTask,
            &UnderCommand,
            &IdleState,
            &mut Destination,
            &mut Path,
        ),
        (With<DamnedSoul>, Without<Familiar>),
    >,
    q_familiars: Query<&Transform, With<Familiar>>,
) {
    for (_soul_entity, soul_transform, task, under_command, idle, mut dest, mut path) in
        q_souls.iter_mut()
    {
        // ExhaustedGathering状態の魂は追従しない（使役状態は既に解除されているはず）
        if idle.behavior == IdleBehavior::ExhaustedGathering {
            continue;
        }
        // タスクがある場合はスキップ
        if !matches!(task, AssignedTask::None) {
            continue;
        }

        // 使い魔の位置を取得
        let Ok(fam_transform) = q_familiars.get(under_command.0) else {
            continue;
        };

        let fam_pos = fam_transform.translation.truncate();
        let soul_pos = soul_transform.translation.truncate();
        let distance = soul_pos.distance(fam_pos);

        // 使い魔から離れすぎている場合のみ移動（閾値を緩めた）
        const FOLLOW_DISTANCE: f32 = TILE_SIZE * 2.0;
        const START_FOLLOW_DISTANCE: f32 = TILE_SIZE * 4.0; // 5.0 -> 4.0 に緩和

        if distance > START_FOLLOW_DISTANCE {
            let direction = (fam_pos - soul_pos).normalize_or_zero();
            let target = fam_pos - direction * FOLLOW_DISTANCE;

            // 現在の目的地が使い魔から離れすぎている場合のみ更新
            if dest.0.distance(fam_pos) > TILE_SIZE * 3.0 || path.waypoints.is_empty() {
                dest.0 = target;
                path.waypoints.clear();
            }
        }
    }
}
