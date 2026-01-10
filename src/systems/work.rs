//! 作業システムモジュール
//!
//! 魂へのタスク委譲と自動化ロジックを管理します。

pub use crate::systems::spatial::*;
pub use crate::systems::task_execution::*;
pub use crate::systems::task_queue::*;

use crate::constants::*;
use crate::entities::damned_soul::{DamnedSoul, Destination, IdleBehavior, IdleState, Path};
use crate::entities::familiar::{
    ActiveCommand, Familiar, FamiliarCommand, FamiliarOperation, UnderCommand,
};
use crate::systems::command::TaskArea;
use crate::systems::jobs::{
    Designation, DesignationCreatedEvent, IssuedBy, TaskCompletedEvent, TaskSlots, WorkType,
};
use crate::systems::logistics::{ClaimedBy, InStockpile, Inventory, ResourceItem, Stockpile};

use bevy::prelude::*;

// ============================================================
// 自動運搬関連
// ============================================================

/// 実行頻度を制御するためのカウンター (現在は main.rs のタイマーで制御中)
#[derive(Resource, Default)]
pub struct AutoHaulCounter;

// ============================================================
// システム実装
// ============================================================

/// 旧来のタスク委譲システム（現在は使い魔AIに移行したため未使用）
#[allow(dead_code)]
pub fn task_delegation_system(
    mut commands: Commands,
    q_familiars: Query<(Entity, &Transform, &FamiliarOperation), With<ActiveCommand>>,
    mut q_souls: Query<(
        Entity,
        &Transform,
        &DamnedSoul,
        &mut AssignedTask,
        &mut Destination,
        &mut Path,
        &mut Inventory,
        &IdleState,
    )>,
    q_stockpiles: Query<(Entity, &Transform, &Stockpile)>,
    q_under_command: Query<&UnderCommand>,
    mut q_designations: Query<(&Transform, &Designation, &mut TaskSlots)>,
    mut queue: ResMut<TaskQueue>,
    spatial_grid: Res<SpatialGrid>,
    mut ev_created: MessageReader<DesignationCreatedEvent>,
    mut ev_completed: MessageReader<TaskCompletedEvent>,
) {
    // イベントがあるか、キューが空でない場合のみ実行
    if ev_created.is_empty()
        && ev_completed.is_empty()
        && queue.by_familiar.values().all(|v| v.is_empty())
    {
        return;
    }

    // イベントを読み飛ばしてフラグにする（実際にはqueue_management_systemが既に処理している想定）
    ev_created.clear();
    ev_completed.clear();
    for (fam_entity, fam_transform, fam_op) in q_familiars.iter() {
        let fam_pos = fam_transform.translation.truncate();
        let fatigue_threshold = fam_op.fatigue_threshold;

        // 使役枠の空きを確認 (UnderCommandを持つソウルを数える)
        let current_count = q_under_command
            .iter()
            .filter(|uc| uc.0 == fam_entity)
            .count();

        if current_count >= 2 {
            continue;
        }
        let slots_available = 2 - current_count;

        // キューからこの使い魔のタスクを取得
        let Some(tasks) = queue.get_for_familiar(fam_entity) else {
            continue;
        };
        if tasks.is_empty() {
            continue;
        }

        // 優先度と距離でソート（既に優先度でソート済みなので、距離のみ計算）
        let mut sorted_tasks: Vec<_> = tasks.iter().copied().collect();

        sorted_tasks.sort_by(|t1, t2| match t2.priority.cmp(&t1.priority) {
            std::cmp::Ordering::Equal => {
                let p1 = q_designations
                    .get(t1.entity)
                    .map(|(t, _, _)| t.translation.truncate())
                    .unwrap_or(Vec2::ZERO);
                let p2 = q_designations
                    .get(t2.entity)
                    .map(|(t, _, _)| t.translation.truncate())
                    .unwrap_or(Vec2::ZERO);
                let d1 = p1.distance_squared(fam_pos);
                let d2 = p2.distance_squared(fam_pos);
                d1.partial_cmp(&d2).unwrap_or(std::cmp::Ordering::Equal)
            }
            other => other,
        });

        let mut assigned_this_tick = 0;
        let mut to_remove = Vec::new();

        for task_info in sorted_tasks.iter() {
            let des_entity = task_info.entity;
            let work_type = task_info.work_type;

            let Ok((des_transform, _, mut slots)) = q_designations.get_mut(des_entity) else {
                to_remove.push(des_entity);
                continue;
            };

            if !slots.has_slot() {
                to_remove.push(des_entity);
                continue;
            }

            let des_pos = des_transform.translation.truncate();

            if assigned_this_tick >= slots_available {
                break;
            }

            let nearby_souls = spatial_grid.get_nearby(des_pos);

            let mut best_soul = nearby_souls
                .iter()
                .filter_map(|&e| q_souls.get(e).ok())
                .filter(|(_, _, soul, current_task, _, _, _, idle)| {
                    matches!(*current_task, AssignedTask::None)
                        && soul.motivation >= MOTIVATION_THRESHOLD
                        && soul.fatigue < fatigue_threshold
                        && idle.behavior != IdleBehavior::ExhaustedGathering
                })
                .min_by(|(_, t1, _, _, _, _, _, _), (_, t2, _, _, _, _, _, _)| {
                    let d1 = t1.translation.truncate().distance_squared(des_pos);
                    let d2 = t2.translation.truncate().distance_squared(des_pos);
                    d1.partial_cmp(&d2).unwrap_or(std::cmp::Ordering::Equal)
                })
                .map(|(e, _, _, _, _, _, _, _)| e);

            // 【最適化】見つからない場合、段階的に半径を広げて検索（全soulのイテレートを避ける）
            if best_soul.is_none() {
                let search_tiers = [TILE_SIZE * 10.0, TILE_SIZE * 30.0, TILE_SIZE * 60.0];
                for &radius in search_tiers.iter() {
                    let broader_souls = spatial_grid.get_nearby_in_radius(des_pos, radius);
                    best_soul = broader_souls
                        .iter()
                        .filter_map(|&e| q_souls.get(e).ok())
                        .filter(|(_, _, soul, current_task, _, _, _, idle)| {
                            matches!(*current_task, AssignedTask::None)
                                && soul.motivation >= MOTIVATION_THRESHOLD
                                && soul.fatigue < fatigue_threshold
                                && idle.behavior != IdleBehavior::ExhaustedGathering
                        })
                        .min_by(|(_, t1, _, _, _, _, _, _), (_, t2, _, _, _, _, _, _)| {
                            let d1 = t1.translation.truncate().distance_squared(des_pos);
                            let d2 = t2.translation.truncate().distance_squared(des_pos);
                            d1.partial_cmp(&d2).unwrap_or(std::cmp::Ordering::Equal)
                        })
                        .map(|(e, _, _, _, _, _, _, _)| e);

                    if best_soul.is_some() {
                        break;
                    }
                }
            }

            // 【最終フォールバック】空間グリッドで見つからない場合、全ソウルをスキャン（グリッドの同期漏れ対策）
            if best_soul.is_none() {
                best_soul = q_souls
                    .iter()
                    .filter(|(_, _, soul, current_task, _, _, _, idle)| {
                        matches!(*current_task, AssignedTask::None)
                            && soul.motivation >= MOTIVATION_THRESHOLD
                            && soul.fatigue < fatigue_threshold
                            && idle.behavior != IdleBehavior::ExhaustedGathering
                    })
                    .min_by(|(_, t1, _, _, _, _, _, _), (_, t2, _, _, _, _, _, _)| {
                        let d1 = t1.translation.truncate().distance_squared(des_pos);
                        let d2 = t2.translation.truncate().distance_squared(des_pos);
                        d1.partial_cmp(&d2).unwrap_or(std::cmp::Ordering::Equal)
                    })
                    .map(|(e, _, _, _, _, _, _, _)| e);

                if best_soul.is_some() {
                    warn!(
                        "WORK: Soul found via Global Fallback (SpatialGrid might be out of sync)"
                    );
                }
            }

            if let Some(soul_entity) = best_soul {
                match work_type {
                    WorkType::Chop | WorkType::Mine => {
                        if let Ok((mut soul_task, mut dest, mut path)) = q_souls
                            .get_mut(soul_entity)
                            .map(|(_, _, _, t, d, p, _, _)| (t, d, p))
                        {
                            *soul_task = AssignedTask::Gather {
                                target: des_entity,
                                work_type,
                                phase: GatherPhase::GoingToResource,
                            };
                            dest.0 = des_pos;
                            path.waypoints.clear();

                            commands.entity(des_entity).insert(ClaimedBy(soul_entity));
                            slots.current += 1;
                            commands
                                .entity(soul_entity)
                                .insert(UnderCommand(fam_entity));

                            assigned_this_tick += 1;
                            to_remove.push(des_entity);
                            info!(
                                "DELEGATION: Soul {:?} assigned to GATHER target {:?} by Familiar {:?}",
                                soul_entity, des_entity, fam_entity
                            );
                        }
                    }
                    WorkType::Haul => {
                        let best_stockpile = q_stockpiles
                            .iter()
                            .min_by(|(_, t1, _), (_, t2, _)| {
                                let d1 = t1.translation.truncate().distance_squared(des_pos);
                                let d2 = t2.translation.truncate().distance_squared(des_pos);
                                d1.partial_cmp(&d2).unwrap_or(std::cmp::Ordering::Equal)
                            })
                            .map(|(e, _, _)| e);

                        if let Some(stock_entity) = best_stockpile {
                            if let Ok((mut soul_task, mut dest, mut path)) = q_souls
                                .get_mut(soul_entity)
                                .map(|(_, _, _, t, d, p, _, _)| (t, d, p))
                            {
                                *soul_task = AssignedTask::Haul {
                                    item: des_entity,
                                    stockpile: stock_entity,
                                    phase: HaulPhase::GoingToItem,
                                };
                                dest.0 = des_pos;
                                path.waypoints.clear();

                                commands.entity(des_entity).insert(ClaimedBy(soul_entity));
                                slots.current += 1;
                                commands
                                    .entity(soul_entity)
                                    .insert(UnderCommand(fam_entity));

                                assigned_this_tick += 1;
                                to_remove.push(des_entity);
                                info!(
                                    "DELEGATION: Soul {:?} assigned HAUL item {:?} by Familiar {:?}",
                                    soul_entity, des_entity, fam_entity
                                );
                            }
                        }
                    }
                    _ => {}
                }
            }
        }

        for entity in to_remove {
            queue.remove(fam_entity, entity);
        }
    }
}

/// 使い魔が Idle コマンドの場合、または使い魔が存在しない場合に部下をリリースする
pub fn cleanup_commanded_souls_system(
    mut commands: Commands,
    q_souls: Query<(Entity, &UnderCommand)>,
    q_familiars: Query<&ActiveCommand, With<Familiar>>,
) {
    for (soul_entity, under_command) in q_souls.iter() {
        let should_release = match q_familiars.get(under_command.0) {
            Ok(active_cmd) => matches!(active_cmd.command, FamiliarCommand::Idle),
            Err(_) => true, // 使い魔が存在しない場合はリリース
        };

        if should_release {
            info!(
                "RELEASE: Soul {:?} released from Familiar {:?}",
                soul_entity, under_command.0
            );
            commands.entity(soul_entity).remove::<UnderCommand>();
        }
    }
}

pub fn task_area_auto_haul_system(
    mut commands: Commands,
    _counter: ResMut<AutoHaulCounter>,
    resource_grid: Res<ResourceSpatialGrid>,
    q_familiars: Query<(Entity, &ActiveCommand, &TaskArea)>,
    q_stockpiles: Query<(&Transform, &Stockpile)>,
    q_resources: Query<
        (Entity, &Transform, &Visibility),
        (
            With<ResourceItem>,
            Without<InStockpile>,
            Without<Designation>,
            Without<ClaimedBy>,
        ),
    >,
    mut ev_created: MessageWriter<DesignationCreatedEvent>,
) {
    for (fam_entity, active_command, task_area) in q_familiars.iter() {
        if matches!(active_command.command, FamiliarCommand::Idle) {
            // TaskAreaがあれば続行
        }

        for (stock_transform, stockpile) in q_stockpiles.iter() {
            let stock_pos = stock_transform.translation.truncate();
            if !task_area.contains(stock_pos) {
                continue;
            }

            if stockpile.current_count >= stockpile.capacity {
                continue;
            }

            let search_radius = TILE_SIZE * 15.0;
            let nearby_resources = resource_grid.get_nearby_in_radius(stock_pos, search_radius);

            // 【最適化】全リソースを iterate するのではなく、空間グリッドで見つかった近傍のみをチェック
            let nearest_resource = nearby_resources
                .iter()
                .filter_map(|&entity| {
                    // ここで q_resources を使って詳細チェック（Without等のフィルタは適用済みクエリを使う）
                    let Ok((_, transform, visibility)) = q_resources.get(entity) else {
                        return None;
                    };
                    // Visibility::Hidden（誰かが持っている）は除外
                    if *visibility == Visibility::Hidden {
                        return None;
                    }

                    let dist_sq = transform.translation.truncate().distance_squared(stock_pos);
                    if dist_sq < search_radius * search_radius {
                        Some((entity, dist_sq))
                    } else {
                        None
                    }
                })
                .min_by(|(_, d1), (_, d2)| d1.partial_cmp(d2).unwrap_or(std::cmp::Ordering::Equal))
                .map(|(entity, _)| entity);

            if let Some(item_entity) = nearest_resource {
                commands.entity(item_entity).insert((
                    Designation {
                        work_type: WorkType::Haul,
                    },
                    IssuedBy(fam_entity),
                    TaskSlots::new(1),
                ));
                ev_created.write(DesignationCreatedEvent {
                    entity: item_entity,
                    work_type: WorkType::Haul,
                    issued_by: Some(fam_entity),
                    priority: 0,
                });
            }
        }
    }
}
