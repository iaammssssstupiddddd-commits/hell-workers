//! タスク実行モジュール
//!
//! 魂に割り当てられたタスクの実行ロジックを提供します。

use crate::assets::GameAssets;
use crate::constants::*;
use crate::entities::damned_soul::{DamnedSoul, Destination, Path};
use crate::events::OnTaskCompleted;
use crate::systems::jobs::{
    Designation, IssuedBy, Rock, TaskCompletedEvent, TaskSlots, Tree, WorkType,
};
use crate::systems::logistics::{ClaimedBy, InStockpile, Inventory, ResourceItem, Stockpile};
use bevy::prelude::*;

// ============================================================
// タスク型定義
// ============================================================

/// 人間に割り当てられたタスク
#[derive(Component, Clone, Debug)]
pub enum AssignedTask {
    None,
    /// リソースを収集する（簡略版）
    Gather {
        target: Entity,
        work_type: WorkType,
        phase: GatherPhase,
    },
    /// リソースを運搬する
    Haul {
        item: Entity,
        stockpile: Entity,
        phase: HaulPhase,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HaulPhase {
    GoingToItem,
    GoingToStockpile,
    Dropping,
}

impl Default for AssignedTask {
    fn default() -> Self {
        Self::None
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GatherPhase {
    GoingToResource,
    Collecting { progress: f32 },
    Done,
}

impl AssignedTask {
    pub fn work_type(&self) -> Option<WorkType> {
        match self {
            AssignedTask::Gather { work_type, .. } => Some(*work_type),
            AssignedTask::Haul { .. } => Some(WorkType::Haul),
            AssignedTask::None => None,
        }
    }
}

// ============================================================
// タスク実行システム
// ============================================================

pub fn task_execution_system(
    mut commands: Commands,
    mut q_souls: Query<(
        Entity,
        &Transform,
        &mut DamnedSoul,
        &mut AssignedTask,
        &mut Destination,
        &mut Path,
        &mut Inventory,
    )>,
    q_targets: Query<(
        &Transform,
        Option<&Tree>,
        Option<&Rock>,
        Option<&ResourceItem>,
        Option<&Designation>,
        Option<&InStockpile>,
    )>,
    mut q_stockpiles: Query<(&Transform, &mut Stockpile)>,
    game_assets: Res<GameAssets>,
    mut ev_completed: MessageWriter<TaskCompletedEvent>,
    time: Res<Time>,
) {
    for (soul_entity, soul_transform, mut soul, mut task, mut dest, mut path, mut inventory) in
        q_souls.iter_mut()
    {
        let was_busy = !matches!(*task, AssignedTask::None);
        let old_work_type = task.work_type();

        let old_task_entity = match *task {
            AssignedTask::Gather { target, .. } => Some(target),
            AssignedTask::Haul { item, .. } => Some(item),
            AssignedTask::None => None,
        };

        match *task {
            AssignedTask::Gather {
                target,
                work_type,
                phase,
            } => {
                handle_gather_task(
                    soul_entity,
                    soul_transform,
                    &mut soul,
                    &mut task,
                    &mut dest,
                    &mut path,
                    target,
                    &work_type,
                    phase,
                    &q_targets,
                    &mut commands,
                    &game_assets,
                    &time,
                );
            }
            AssignedTask::Haul {
                item,
                stockpile,
                phase,
            } => {
                handle_haul_task(
                    soul_entity,
                    soul_transform,
                    &mut soul,
                    &mut task,
                    &mut dest,
                    &mut path,
                    &mut inventory,
                    item,
                    stockpile,
                    phase,
                    &q_targets,
                    &mut q_stockpiles,
                    &mut commands,
                );
            }
            AssignedTask::None => {}
        }

        // 完了イベントの発行
        if was_busy && matches!(*task, AssignedTask::None) {
            if let Some(work_type) = old_work_type {
                // 既存のMessage送信
                ev_completed.write(TaskCompletedEvent {
                    _soul_entity: soul_entity,
                    _task_type: work_type,
                });

                // Bevy 0.17 の Observer をトリガー
                commands.trigger(OnTaskCompleted {
                    entity: soul_entity,
                    task_entity: old_task_entity.unwrap_or(Entity::PLACEHOLDER),
                    work_type,
                });

                info!(
                    "EVENT: TaskCompletedEvent sent & OnTaskCompleted triggered for Soul {:?}",
                    soul_entity
                );
            }
        }
    }
}

// ============================================================
// ヘルパー関数
// ============================================================

fn handle_gather_task(
    soul_entity: Entity,
    soul_transform: &Transform,
    soul: &mut DamnedSoul,
    task: &mut AssignedTask,
    dest: &mut Destination,
    path: &mut Path,
    target: Entity,
    work_type: &WorkType,
    phase: GatherPhase,
    q_targets: &Query<(
        &Transform,
        Option<&Tree>,
        Option<&Rock>,
        Option<&ResourceItem>,
        Option<&Designation>,
        Option<&InStockpile>,
    )>,
    commands: &mut Commands,
    game_assets: &Res<GameAssets>,
    time: &Res<Time>,
) {
    let soul_pos = soul_transform.translation.truncate();
    match phase {
        GatherPhase::GoingToResource => {
            if let Ok((res_transform, _, _, _, des_opt, _)) = q_targets.get(target) {
                // 指定が解除されていたら中止
                if des_opt.is_none() {
                    *task = AssignedTask::None;
                    path.waypoints.clear();
                    return;
                }
                let res_pos = res_transform.translation.truncate();
                if dest.0.distance_squared(res_pos) > 2.0 {
                    dest.0 = res_pos;
                    path.waypoints.clear();
                }

                let dist = soul_pos.distance(res_pos);
                if dist < TILE_SIZE * 1.2 {
                    *task = AssignedTask::Gather {
                        target,
                        work_type: *work_type,
                        phase: GatherPhase::Collecting { progress: 0.0 },
                    };
                    path.waypoints.clear();
                }
            } else {
                *task = AssignedTask::None;
                path.waypoints.clear();
            }
        }
        GatherPhase::Collecting { mut progress } => {
            if let Ok((res_transform, tree, rock, _, des_opt, _)) = q_targets.get(target) {
                // 指定が解除されていたら中止
                if des_opt.is_none() {
                    *task = AssignedTask::None;
                    path.waypoints.clear();
                    return;
                }
                let pos = res_transform.translation;

                // 進行度を更新 (仮に 2秒で完了とする)
                progress += time.delta_secs() * 0.5;

                if progress >= 1.0 {
                    if tree.is_some() {
                        commands.spawn((
                            ResourceItem(crate::systems::logistics::ResourceType::Wood),
                            Sprite {
                                image: game_assets.wood.clone(),
                                custom_size: Some(Vec2::splat(TILE_SIZE * 0.5)),
                                color: Color::srgb(0.5, 0.35, 0.05),
                                ..default()
                            },
                            Transform::from_translation(pos),
                        ));
                        info!("TASK_EXEC: Soul {:?} chopped a tree", soul_entity);
                    } else if rock.is_some() {
                        commands.spawn((
                            ResourceItem(crate::systems::logistics::ResourceType::Stone),
                            Sprite {
                                image: game_assets.stone.clone(),
                                custom_size: Some(Vec2::splat(TILE_SIZE * 0.5)),
                                ..default()
                            },
                            Transform::from_translation(pos),
                        ));
                        info!("TASK_EXEC: Soul {:?} mined a rock", soul_entity);
                    }
                    commands.entity(target).despawn();

                    *task = AssignedTask::Gather {
                        target,
                        work_type: *work_type,
                        phase: GatherPhase::Done,
                    };
                    soul.fatigue = (soul.fatigue + 0.1).min(1.0);
                } else {
                    // 進捗を保存
                    *task = AssignedTask::Gather {
                        target,
                        work_type: *work_type,
                        phase: GatherPhase::Collecting { progress },
                    };
                }
            } else {
                *task = AssignedTask::None;
            }
        }
        GatherPhase::Done => {
            *task = AssignedTask::None;
            path.waypoints.clear();
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn handle_haul_task(
    soul_entity: Entity,
    soul_transform: &Transform,
    soul: &mut DamnedSoul,
    task: &mut AssignedTask,
    dest: &mut Destination,
    path: &mut Path,
    inventory: &mut Inventory,
    item: Entity,
    stockpile: Entity,
    phase: HaulPhase,
    q_targets: &Query<(
        &Transform,
        Option<&Tree>,
        Option<&Rock>,
        Option<&ResourceItem>,
        Option<&Designation>,
        Option<&InStockpile>,
    )>,
    q_stockpiles: &mut Query<(&Transform, &mut Stockpile)>,
    commands: &mut Commands,
) {
    let soul_pos = soul_transform.translation.truncate();
    match phase {
        HaulPhase::GoingToItem => {
            if let Ok((res_transform, _, _, _, des_opt, in_stockpile_opt)) = q_targets.get(item) {
                // 指示がキャンセルされていないか確認
                if des_opt.is_none() {
                    *task = AssignedTask::None;
                    path.waypoints.clear();
                    return;
                }

                // アイテムと備蓄場所の情報を取得
                let res_pos = res_transform.translation.truncate();
                if dest.0.distance_squared(res_pos) > 2.0 {
                    dest.0 = res_pos;
                    path.waypoints.clear();
                }

                if soul_pos.distance(res_pos) < TILE_SIZE * 1.2 {
                    inventory.0 = Some(item);
                    commands.entity(item).insert(Visibility::Hidden);

                    // 【修正】もしアイテムが備蓄場所にあったなら、カウントを減らし、InStockpileコンポーネントを削除
                    if let Some(InStockpile(stock_entity)) = in_stockpile_opt {
                        if let Ok((_, mut stock_comp)) = q_stockpiles.get_mut(*stock_entity) {
                            stock_comp.current_count = stock_comp.current_count.saturating_sub(1);
                            info!(
                                "HAUL: Item picked up from Stockpile {:?}. New count: {}",
                                stock_entity, stock_comp.current_count
                            );
                        }
                    }
                    commands.entity(item).remove::<InStockpile>();

                    commands.entity(item).remove::<Designation>();
                    commands.entity(item).remove::<IssuedBy>();
                    commands.entity(item).remove::<TaskSlots>();

                    *task = AssignedTask::Haul {
                        item,
                        stockpile,
                        phase: HaulPhase::GoingToStockpile,
                    };
                    path.waypoints.clear();
                    info!("HAUL: Soul {:?} picked up item", soul_entity);
                }
            } else {
                *task = AssignedTask::None;
                path.waypoints.clear();
            }
        }
        HaulPhase::GoingToStockpile => {
            if let Ok((stock_transform, _)) = q_stockpiles.get(stockpile) {
                let stock_pos = stock_transform.translation.truncate();
                if dest.0.distance_squared(stock_pos) > 2.0 {
                    dest.0 = stock_pos;
                    path.waypoints.clear();
                }

                if soul_pos.distance(stock_pos) < TILE_SIZE * 1.2 {
                    *task = AssignedTask::Haul {
                        item,
                        stockpile,
                        phase: HaulPhase::Dropping,
                    };
                    path.waypoints.clear();
                }
            } else {
                warn!(
                    "HAUL: Soul {:?} stockpile {:?} not found",
                    soul_entity, stockpile
                );
                if let Some(item_entity) = inventory.0.take() {
                    commands.entity(item_entity).insert(Visibility::Visible);
                    commands.entity(item_entity).remove::<ClaimedBy>();
                }
                *task = AssignedTask::None;
                path.waypoints.clear();
            }
        }
        HaulPhase::Dropping => {
            if let Ok((stock_transform, mut stockpile_comp)) = q_stockpiles.get_mut(stockpile) {
                let stock_pos = stock_transform.translation.truncate();
                if let Some(item_entity) = inventory.0.take() {
                    commands.entity(item_entity).insert((
                        Visibility::Visible,
                        Transform::from_xyz(stock_pos.x, stock_pos.y, 0.6),
                        InStockpile(stockpile),
                    ));
                    commands.entity(item_entity).remove::<ClaimedBy>();
                    stockpile_comp.current_count += 1; // カウントアップ
                    info!(
                        "TASK_EXEC: Soul {:?} dropped item at stockpile. Count: {}",
                        soul_entity, stockpile_comp.current_count
                    );
                }
            }
            *task = AssignedTask::None;
            path.waypoints.clear();
            soul.fatigue = (soul.fatigue + 0.05).min(1.0);
        }
    }
}
