//! タスク実行モジュール
//!
//! 魂に割り当てられたタスクの実行ロジックを提供します。

use crate::assets::GameAssets;
use crate::constants::*;
use crate::entities::damned_soul::{DamnedSoul, Destination, Path, StressBreakdown};
use crate::events::OnTaskCompleted;
use crate::relationships::{Holding, WorkingOn};
use crate::systems::familiar_ai::haul_cache::HaulReservationCache;
use crate::systems::jobs::{
    Blueprint, Designation, IssuedBy, Rock, TaskCompletedEvent, TaskSlots, Tree, WorkType,
};
use crate::systems::logistics::{ResourceItem, Stockpile};
use bevy::prelude::*;

// ============================================================
// タスク型定義
// ============================================================

/// 人間に割り当てられたタスク
#[derive(Component, Reflect, Clone, Debug, Default)]
#[reflect(Component)]
pub enum AssignedTask {
    #[default]
    None,
    /// リソースを収集する（簡略版）
    Gather {
        target: Entity,
        work_type: WorkType,
        phase: GatherPhase,
    },
    /// リソースを運搬する（ストックパイルへ）
    Haul {
        item: Entity,
        stockpile: Entity,
        phase: HaulPhase,
    },
    /// 資材を設計図へ運搬する
    HaulToBlueprint {
        item: Entity,
        blueprint: Entity,
        phase: HaulToBpPhase,
    },
    /// 建築作業を行う
    Build {
        blueprint: Entity,
        phase: BuildPhase,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Reflect, Default)]
pub enum HaulPhase {
    #[default]
    GoingToItem,
    GoingToStockpile,
    Dropping,
}

#[derive(Clone, Copy, Debug, PartialEq, Reflect, Default)]
pub enum GatherPhase {
    #[default]
    GoingToResource,
    Collecting {
        progress: f32,
    },
    Done,
}

#[derive(Clone, Copy, Debug, PartialEq, Reflect, Default)]
pub enum BuildPhase {
    #[default]
    GoingToBlueprint,
    Building {
        progress: f32,
    },
    Done,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Reflect, Default)]
pub enum HaulToBpPhase {
    #[default]
    GoingToItem,
    GoingToBlueprint,
    Delivering,
}

impl AssignedTask {
    pub fn work_type(&self) -> Option<WorkType> {
        match self {
            AssignedTask::Gather { work_type, .. } => Some(*work_type),
            AssignedTask::Haul { .. } => Some(WorkType::Haul),
            AssignedTask::HaulToBlueprint { .. } => Some(WorkType::Haul), // 同じ Haul カテゴリ
            AssignedTask::Build { .. } => Some(WorkType::Build),
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
        Option<&Holding>,
        Option<&StressBreakdown>,
    )>,
    q_targets: Query<(
        &Transform,
        Option<&Tree>,
        Option<&Rock>,
        Option<&ResourceItem>,
        Option<&Designation>,
        Option<&crate::relationships::StoredIn>,
    )>,
    q_designations: Query<(
        Entity,
        &Transform,
        &Designation,
        Option<&IssuedBy>,
        Option<&TaskSlots>,
        Option<&crate::relationships::TaskWorkers>,
    )>,
    mut q_stockpiles: Query<(
        &Transform,
        &mut Stockpile,
        Option<&crate::relationships::StoredItems>,
    )>,
    game_assets: Res<GameAssets>,
    mut ev_completed: MessageWriter<TaskCompletedEvent>,
    time: Res<Time>,
    mut haul_cache: ResMut<HaulReservationCache>,
    mut q_blueprints: Query<(&Transform, &mut Blueprint, Option<&Designation>)>,
) {
    let mut dropped_this_frame = std::collections::HashMap::<Entity, usize>::new();

    for (
        soul_entity,
        soul_transform,
        mut soul,
        mut task,
        mut dest,
        mut path,
        holding_opt,
        breakdown_opt,
    ) in q_souls.iter_mut()
    {
        let was_busy = !matches!(*task, AssignedTask::None);
        let old_work_type = task.work_type();

        let old_task_entity = match *task {
            AssignedTask::Gather { target, .. } => Some(target),
            AssignedTask::Haul { item, .. } => Some(item),
            AssignedTask::HaulToBlueprint { item, .. } => Some(item),
            AssignedTask::Build { blueprint, .. } => Some(blueprint),
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
                    holding_opt,
                    item,
                    stockpile,
                    phase,
                    &q_targets,
                    &mut q_stockpiles,
                    &mut commands,
                    &mut dropped_this_frame,
                    &mut *haul_cache,
                );
            }
            AssignedTask::Build { blueprint, phase } => {
                handle_build_task(
                    soul_entity,
                    soul_transform,
                    &mut soul,
                    &mut task,
                    &mut dest,
                    &mut path,
                    blueprint,
                    phase,
                    &mut q_blueprints,
                    &mut commands,
                    &time,
                );
            }
            AssignedTask::HaulToBlueprint {
                item,
                blueprint,
                phase,
            } => {
                handle_haul_to_blueprint_task(
                    soul_entity,
                    soul_transform,
                    &mut soul,
                    &mut task,
                    &mut dest,
                    &mut path,
                    holding_opt,
                    breakdown_opt,
                    item,
                    blueprint,
                    phase,
                    &q_targets,
                    &q_designations,
                    &mut q_blueprints,
                    &mut q_stockpiles,
                    &mut haul_cache,
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
        Option<&crate::relationships::StoredIn>,
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
            if let Ok(target_data) = q_targets.get(target) {
                let (res_transform, tree, rock, _res_item, des_opt, _stored_in) = target_data;
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
    holding: Option<&Holding>,
    item: Entity,
    stockpile: Entity,
    phase: HaulPhase,
    q_targets: &Query<(
        &Transform,
        Option<&Tree>,
        Option<&Rock>,
        Option<&ResourceItem>,
        Option<&Designation>,
        Option<&crate::relationships::StoredIn>,
    )>,
    q_stockpiles: &mut Query<(
        &Transform,
        &mut Stockpile,
        Option<&crate::relationships::StoredItems>,
    )>,
    commands: &mut Commands,
    dropped_this_frame: &mut std::collections::HashMap<Entity, usize>,
    haul_cache: &mut HaulReservationCache,
) {
    let soul_pos = soul_transform.translation.truncate();
    match phase {
        HaulPhase::GoingToItem => {
            if let Ok((res_transform, _, _, _res_item_opt, des_opt, stored_in_opt)) =
                q_targets.get(item)
            {
                // 指示がキャンセルされていないか確認
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

                if soul_pos.distance(res_pos) < TILE_SIZE * 1.2 {
                    commands.entity(soul_entity).insert(Holding(item));
                    commands.entity(item).insert(Visibility::Hidden);

                    // もしアイテムが備蓄場所にあったなら、その備蓄場所の型管理を更新する
                    if let Some(crate::relationships::StoredIn(stock_entity)) = stored_in_opt {
                        if let Ok((_, mut stock_comp, Some(stored_items))) =
                            q_stockpiles.get_mut(*stock_entity)
                        {
                            // 自分を引いて 0 個になるなら None に戻す
                            if stored_items.len() <= 1 {
                                stock_comp.resource_type = None;
                                info!(
                                    "HAUL: Stockpile {:?} became empty. Resetting resource type.",
                                    stock_entity
                                );
                            }
                        }
                    }

                    // 元のコンポーネントを削除
                    commands
                        .entity(item)
                        .remove::<crate::relationships::StoredIn>();
                    commands.entity(item).remove::<Designation>();
                    commands.entity(item).remove::<IssuedBy>();
                    commands.entity(item).remove::<TaskSlots>();

                    *task = AssignedTask::Haul {
                        item,
                        stockpile,
                        phase: HaulPhase::GoingToStockpile,
                    };
                    path.waypoints.clear();
                    info!("HAUL: Soul {:?} picked up item {:?}", soul_entity, item);
                }
            } else {
                *task = AssignedTask::None;
                path.waypoints.clear();
                haul_cache.release(stockpile);
            }
        }
        HaulPhase::GoingToStockpile => {
            if let Ok((stock_transform, _, _)) = q_stockpiles.get(stockpile) {
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
                if let Some(Holding(held_item_entity)) = holding {
                    commands
                        .entity(*held_item_entity)
                        .insert(Visibility::Visible);
                }
                commands.entity(soul_entity).remove::<Holding>();
                commands.entity(soul_entity).remove::<WorkingOn>();
                *task = AssignedTask::None;
                path.waypoints.clear();
                haul_cache.release(stockpile);
            }
        }
        HaulPhase::Dropping => {
            if let Ok((stock_transform, mut stockpile_comp, stored_items_opt)) =
                q_stockpiles.get_mut(stockpile)
            {
                let current_count = stored_items_opt.map(|si| si.len()).unwrap_or(0);

                // アイテムの型を取得
                let item_type = q_targets
                    .get(item)
                    .ok()
                    .and_then(|(_, _, _, ri, _, _)| ri.map(|r| r.0));
                let this_frame = dropped_this_frame.get(&stockpile).cloned().unwrap_or(0);

                let can_drop = if let Some(it) = item_type {
                    let type_match = stockpile_comp.resource_type.is_none()
                        || stockpile_comp.resource_type == Some(it);
                    // 現在の数 + このフレームですでに置かれた数
                    let capacity_ok = (current_count + this_frame) < stockpile_comp.capacity;
                    type_match && capacity_ok
                } else {
                    false
                };

                if can_drop {
                    // 資源タイプがNoneなら設定
                    if stockpile_comp.resource_type.is_none() {
                        stockpile_comp.resource_type = item_type;
                    }

                    commands.entity(item).insert((
                        Visibility::Visible,
                        Transform::from_xyz(
                            stock_transform.translation.x,
                            stock_transform.translation.y,
                            0.6,
                        ),
                        crate::relationships::StoredIn(stockpile),
                    ));

                    // カウンタを増やす
                    *dropped_this_frame.entry(stockpile).or_insert(0) += 1;

                    info!(
                        "TASK_EXEC: Soul {:?} dropped item at stockpile. New count: {}",
                        soul_entity,
                        current_count + this_frame + 1
                    );
                } else {
                    // 到着時に条件を満たさなくなった場合（型違いor満杯）
                    // 本来は代替地を探すべきだが、今回はシンプルにその場にドロップする
                    warn!("HAUL: Stockpile condition changed. Dropping item on the ground.");
                    commands.entity(item).insert((
                        Visibility::Visible,
                        Transform::from_xyz(soul_pos.x, soul_pos.y, 0.6),
                    ));
                }
            } else {
                // 備蓄場所消失
                if holding.is_some() {
                    commands.entity(item).insert((
                        Visibility::Visible,
                        Transform::from_xyz(soul_pos.x, soul_pos.y, 0.6),
                    ));
                }
            }

            commands.entity(soul_entity).remove::<Holding>();
            commands.entity(soul_entity).remove::<WorkingOn>();
            *task = AssignedTask::None;
            path.waypoints.clear();
            soul.fatigue = (soul.fatigue + 0.05).min(1.0);

            // 搬送予約を解放
            haul_cache.release(stockpile);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn handle_build_task(
    soul_entity: Entity,
    soul_transform: &Transform,
    soul: &mut DamnedSoul,
    task: &mut AssignedTask,
    dest: &mut Destination,
    path: &mut Path,
    blueprint_entity: Entity,
    phase: BuildPhase,
    q_blueprints: &mut Query<(&Transform, &mut Blueprint, Option<&Designation>)>,
    commands: &mut Commands,
    time: &Res<Time>,
) {
    let soul_pos = soul_transform.translation.truncate();

    match phase {
        BuildPhase::GoingToBlueprint => {
            if let Ok((bp_transform, bp, des_opt)) = q_blueprints.get(blueprint_entity) {
                // 指定が解除されていたら中止
                if des_opt.is_none() {
                    *task = AssignedTask::None;
                    path.waypoints.clear();
                    commands.entity(soul_entity).remove::<WorkingOn>();
                    return;
                }

                // 資材が揃っていない場合は中止（資材運搬は別タスク）
                if !bp.materials_complete() {
                    info!(
                        "BUILD: Soul {:?} waiting for materials at blueprint {:?}",
                        soul_entity, blueprint_entity
                    );
                    *task = AssignedTask::None;
                    path.waypoints.clear();
                    commands.entity(soul_entity).remove::<WorkingOn>();
                    return;
                }

                let bp_pos = bp_transform.translation.truncate();
                if dest.0.distance_squared(bp_pos) > 2.0 {
                    dest.0 = bp_pos;
                    path.waypoints.clear();
                }

                let dist = soul_pos.distance(bp_pos);
                if dist < TILE_SIZE * 1.2 {
                    *task = AssignedTask::Build {
                        blueprint: blueprint_entity,
                        phase: BuildPhase::Building { progress: 0.0 },
                    };
                    path.waypoints.clear();
                    info!(
                        "BUILD: Soul {:?} started building at {:?}",
                        soul_entity, bp_pos
                    );
                }
            } else {
                // 設計図が消失
                *task = AssignedTask::None;
                path.waypoints.clear();
                commands.entity(soul_entity).remove::<WorkingOn>();
            }
        }
        BuildPhase::Building { mut progress } => {
            if let Ok((_, mut bp, des_opt)) = q_blueprints.get_mut(blueprint_entity) {
                // 指定が解除されていたら中止
                if des_opt.is_none() {
                    *task = AssignedTask::None;
                    path.waypoints.clear();
                    commands.entity(soul_entity).remove::<WorkingOn>();
                    return;
                }

                // 進捗を更新（3秒で完了）
                progress += time.delta_secs() * 0.33;
                bp.progress = progress;

                if progress >= 1.0 {
                    *task = AssignedTask::Build {
                        blueprint: blueprint_entity,
                        phase: BuildPhase::Done,
                    };
                    soul.fatigue = (soul.fatigue + 0.15).min(1.0);
                    info!(
                        "BUILD: Soul {:?} completed building {:?}",
                        soul_entity, blueprint_entity
                    );
                } else {
                    *task = AssignedTask::Build {
                        blueprint: blueprint_entity,
                        phase: BuildPhase::Building { progress },
                    };
                }
            } else {
                *task = AssignedTask::None;
                path.waypoints.clear();
                commands.entity(soul_entity).remove::<WorkingOn>();
            }
        }
        BuildPhase::Done => {
            commands.entity(soul_entity).remove::<WorkingOn>();
            *task = AssignedTask::None;
            path.waypoints.clear();
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn handle_haul_to_blueprint_task(
    soul_entity: Entity,
    soul_transform: &Transform,
    soul: &mut DamnedSoul,
    task: &mut AssignedTask,
    dest: &mut Destination,
    path: &mut Path,
    holding: Option<&Holding>,
    breakdown_opt: Option<&StressBreakdown>,
    item_entity: Entity,
    blueprint_entity: Entity,
    phase: HaulToBpPhase,
    q_targets: &Query<(
        &Transform,
        Option<&Tree>,
        Option<&Rock>,
        Option<&ResourceItem>,
        Option<&Designation>,
        Option<&crate::relationships::StoredIn>,
    )>,
    q_designations: &Query<(
        Entity,
        &Transform,
        &Designation,
        Option<&IssuedBy>,
        Option<&TaskSlots>,
        Option<&crate::relationships::TaskWorkers>,
    )>,
    q_blueprints: &mut Query<(&Transform, &mut Blueprint, Option<&Designation>)>,
    q_stockpiles: &mut Query<(
        &Transform,
        &mut Stockpile,
        Option<&crate::relationships::StoredItems>,
    )>,
    haul_cache: &mut HaulReservationCache,
    commands: &mut Commands,
) {
    // 疲労またはストレス崩壊のチェック
    if soul.fatigue > 0.95 || breakdown_opt.is_some() {
        info!(
            "HAUL_TO_BP: Cancelled for {:?} - Exhausted or Stress breakdown",
            soul_entity
        );
        crate::systems::soul_ai::work::unassign_task(
            commands,
            soul_entity,
            soul_transform.translation.truncate(),
            task,
            path,
            holding,
            q_designations,
            haul_cache,
        );
        return;
    }

    let soul_pos = soul_transform.translation.truncate();

    match phase {
        HaulToBpPhase::GoingToItem => {
            if let Ok((item_transform, _, _, _, des_opt, _)) = q_targets.get(item_entity) {
                // 指示がキャンセルされていないか確認
                if des_opt.is_none() {
                    info!(
                        "HAUL_TO_BP: Cancelled for {:?} - Designation missing",
                        soul_entity
                    );
                    *task = AssignedTask::None;
                    path.waypoints.clear();
                    commands.entity(soul_entity).remove::<WorkingOn>();
                    return;
                }

                let item_pos = item_transform.translation.truncate();
                if dest.0.distance_squared(item_pos) > 2.0 {
                    dest.0 = item_pos;
                    path.waypoints.clear();
                }

                if soul_pos.distance(item_pos) < TILE_SIZE * 1.2 {
                    commands.entity(soul_entity).insert(Holding(item_entity));
                    commands.entity(item_entity).insert(Visibility::Hidden);

                    // もしアイテムが備蓄場所にあったなら、その備蓄場所の型管理を更新する
                    if let Ok((_, _, _, _, _, stored_in_opt)) = q_targets.get(item_entity) {
                        if let Some(crate::relationships::StoredIn(stock_entity)) = stored_in_opt {
                            if let Ok((_, mut stock_comp, Some(stored_items))) =
                                q_stockpiles.get_mut(*stock_entity)
                            {
                                // 自分を引いて 0 個になるなら None に戻す
                                if stored_items.len() <= 1 {
                                    stock_comp.resource_type = None;
                                    info!(
                                        "HAUL_TO_BP: Stockpile {:?} became empty. Resetting resource type.",
                                        stock_entity
                                    );
                                }
                            }
                        }
                    }

                    // 元のコンポーネントを削除
                    commands
                        .entity(item_entity)
                        .remove::<crate::relationships::StoredIn>();
                    commands.entity(item_entity).remove::<Designation>();
                    commands.entity(item_entity).remove::<IssuedBy>();
                    commands.entity(item_entity).remove::<TaskSlots>();

                    *task = AssignedTask::HaulToBlueprint {
                        item: item_entity,
                        blueprint: blueprint_entity,
                        phase: HaulToBpPhase::GoingToBlueprint,
                    };
                    path.waypoints.clear();
                    info!(
                        "HAUL_TO_BP: Soul {:?} picked up item {:?}",
                        soul_entity, item_entity
                    );
                }
            } else {
                info!(
                    "HAUL_TO_BP: Cancelled for {:?} - Item {:?} gone",
                    soul_entity, item_entity
                );
                *task = AssignedTask::None;
                path.waypoints.clear();
                commands.entity(soul_entity).remove::<WorkingOn>();
            }
        }
        HaulToBpPhase::GoingToBlueprint => {
            if let Ok((bp_transform, _, _)) = q_blueprints.get(blueprint_entity) {
                let bp_pos = bp_transform.translation.truncate();
                if dest.0.distance_squared(bp_pos) > 2.0 {
                    dest.0 = bp_pos;
                    path.waypoints.clear();
                }

                if soul_pos.distance(bp_pos) < TILE_SIZE * 1.2 {
                    info!(
                        "HAUL_TO_BP: Soul {:?} arrived at blueprint {:?}",
                        soul_entity, blueprint_entity
                    );
                    *task = AssignedTask::HaulToBlueprint {
                        item: item_entity,
                        blueprint: blueprint_entity,
                        phase: HaulToBpPhase::Delivering,
                    };
                    path.waypoints.clear();
                }
            } else {
                info!(
                    "HAUL_TO_BP: Cancelled for {:?} - Blueprint {:?} gone",
                    soul_entity, blueprint_entity
                );
                // Blueprint が消失 - アイテムをドロップ
                if holding.is_some() {
                    commands.entity(item_entity).insert((
                        Visibility::Visible,
                        Transform::from_xyz(soul_pos.x, soul_pos.y, 0.6),
                    ));
                }
                commands.entity(soul_entity).remove::<Holding>();
                commands.entity(soul_entity).remove::<WorkingOn>();
                *task = AssignedTask::None;
                path.waypoints.clear();
            }
        }
        HaulToBpPhase::Delivering => {
            if let Ok((_, mut bp, _)) = q_blueprints.get_mut(blueprint_entity) {
                // アイテムの資材タイプを取得
                if let Ok((_, _, _, Some(res_item), _, _)) = q_targets.get(item_entity) {
                    let resource_type = res_item.0;

                    // Blueprint に資材を搬入
                    bp.deliver_material(resource_type, 1);
                    info!(
                        "HAUL_TO_BP: Soul {:?} delivered {:?} to blueprint {:?}. Progress: {:?}/{:?}",
                        soul_entity,
                        resource_type,
                        blueprint_entity,
                        bp.delivered_materials,
                        bp.required_materials
                    );

                    // アイテムを消費
                    commands.entity(item_entity).despawn();
                }
            }

            commands.entity(soul_entity).remove::<Holding>();
            commands.entity(soul_entity).remove::<WorkingOn>();
            *task = AssignedTask::None;
            path.waypoints.clear();
            soul.fatigue = (soul.fatigue + 0.05).min(1.0);
        }
    }
}
