//! 作業管理モジュール
//!
//! 魂へのタスク解除や自動運搬ロジックを管理します。

use crate::constants::*;
use crate::entities::damned_soul::Path;
use crate::entities::familiar::{ActiveCommand, Familiar, FamiliarCommand, UnderCommand};
use crate::relationships::{Holding, TaskWorkers, WorkingOn};
use crate::systems::command::TaskArea;
use crate::systems::familiar_ai::haul_cache::HaulReservationCache;
use crate::systems::jobs::{
    Blueprint, Designation, DesignationCreatedEvent, IssuedBy, TaskSlots, WorkType,
};
use crate::systems::logistics::{ResourceItem, Stockpile};
use crate::systems::soul_ai::task_execution::AssignedTask;
use crate::systems::spatial::{ResourceSpatialGrid, SpatialGridOps};
use crate::world::map::WorldMap;
use bevy::prelude::*;

// ============================================================
// 自動運搬関連
// ============================================================

/// 実行頻度を制御するためのカウンター
#[derive(Resource, Default)]
pub struct AutoHaulCounter;

// ============================================================
// システム実装
// ============================================================

/// 使い魔が Idle コマンドの場合、または使い魔が存在しない場合に部下をリリースする
pub fn cleanup_commanded_souls_system(
    mut commands: Commands,
    mut q_souls: Query<(
        Entity,
        &Transform,
        &UnderCommand,
        &mut AssignedTask,
        &mut Path,
        Option<&Holding>,
    )>,
    q_designations: Query<(
        Entity,
        &Transform,
        &Designation,
        Option<&IssuedBy>,
        Option<&TaskSlots>,
        Option<&TaskWorkers>,
    )>,
    q_familiars: Query<&ActiveCommand, With<Familiar>>,
    mut haul_cache: ResMut<HaulReservationCache>,
    mut ev_created: MessageWriter<DesignationCreatedEvent>,
) {
    for (soul_entity, transform, under_command, mut task, mut path, holding_opt) in
        q_souls.iter_mut()
    {
        let should_release = match q_familiars.get(under_command.0) {
            Ok(active_cmd) => matches!(active_cmd.command, FamiliarCommand::Idle),
            Err(_) => true,
        };

        if should_release {
            info!(
                "RELEASE: Soul {:?} released from Familiar {:?}",
                soul_entity, under_command.0
            );

            unassign_task(
                &mut commands,
                soul_entity,
                transform.translation.truncate(),
                &mut task,
                &mut path,
                holding_opt,
                &q_designations,
                &mut *haul_cache,
                Some(&mut ev_created),
                false, // emit_abandoned_event: 解放時は個別のタスク中断セリフを出さない
            );

            commands.trigger(crate::events::OnReleasedFromService {
                entity: soul_entity,
            });

            commands.entity(soul_entity).remove::<UnderCommand>();
        }
    }
}

/// 魂からタスクの割り当てを解除し、スロットを解放する。
///
/// ソウル側のみを処理し、タスク側（Designation, IssuedBy）には触らない。
/// 使い魔がスロットの空きを検知して別のソウルに再アサインする。
pub fn unassign_task(
    commands: &mut Commands,
    soul_entity: Entity,
    drop_pos: Vec2,
    task: &mut AssignedTask,
    path: &mut Path,
    holding: Option<&Holding>,
    _q_designations: &Query<(
        Entity,
        &Transform,
        &Designation,
        Option<&IssuedBy>,
        Option<&TaskSlots>,
        Option<&TaskWorkers>,
    )>,
    haul_cache: &mut HaulReservationCache,
    _ev_created: Option<&mut MessageWriter<DesignationCreatedEvent>>,
    emit_abandoned_event: bool,
) {
    // タスク中断イベントを発火
    if !matches!(*task, AssignedTask::None) && emit_abandoned_event {
        commands.trigger(crate::events::OnTaskAbandoned {
            entity: soul_entity,
        });
    }

    // 運搬タスクの備蓄場所予約を解除
    if let AssignedTask::Haul { stockpile, .. } = *task {
        haul_cache.release(stockpile);
    }

    // アイテムのドロップ処理（運搬タスクの場合）
    // クリーンな状態でドロップ → オートホールシステムに任せる
    if let Some(Holding(item_entity)) = holding {
        let item_entity = *item_entity;
        let grid = WorldMap::world_to_grid(drop_pos);
        let snapped_pos = WorldMap::grid_to_world(grid.0, grid.1);

        // クリーンな状態でドロップ（Designation なし）
        commands.entity(item_entity).insert((
            Visibility::Visible,
            Transform::from_xyz(snapped_pos.x, snapped_pos.y, Z_ITEM_PICKUP),
        ));
        // 既存のタスク関連コンポーネントを削除
        commands.entity(item_entity).remove::<Designation>();
        commands.entity(item_entity).remove::<IssuedBy>();
        commands.entity(item_entity).remove::<TaskSlots>();
        commands
            .entity(item_entity)
            .remove::<crate::systems::jobs::TargetBlueprint>();

        commands.entity(soul_entity).remove::<Holding>();

        info!(
            "UNASSIGN: Soul dropped item {:?} (clean state for auto-haul)",
            item_entity
        );
    }

    // ソウルからタスクを解除（タスク側には触らない）
    // 使い魔がスロットの空きを検知して別のソウルに再アサインする
    commands.entity(soul_entity).remove::<WorkingOn>();

    *task = AssignedTask::None;
    path.waypoints.clear();

    info!("UNASSIGN: Soul {:?} unassigned from task", soul_entity);
}

/// 指揮エリア内での自動運搬タスク生成システム
pub fn task_area_auto_haul_system(
    mut commands: Commands,
    _counter: ResMut<AutoHaulCounter>,
    resource_grid: Res<ResourceSpatialGrid>,
    q_familiars: Query<(Entity, &ActiveCommand, &TaskArea)>,
    q_stockpiles: Query<(
        &Transform,
        &Stockpile,
        Option<&crate::relationships::StoredItems>,
    )>,
    q_resources: Query<
        (Entity, &Transform, &Visibility, &ResourceItem),
        (
            Without<crate::relationships::StoredIn>,
            Without<Designation>,
            Without<TaskWorkers>,
            Without<crate::systems::jobs::TargetBlueprint>,
        ),
    >,
    mut ev_created: MessageWriter<DesignationCreatedEvent>,
) {
    let mut already_assigned = std::collections::HashSet::new();

    for (fam_entity, _active_command, task_area) in q_familiars.iter() {
        for (stock_transform, stockpile, stored_items_opt) in q_stockpiles.iter() {
            let stock_pos = stock_transform.translation.truncate();
            if !task_area.contains(stock_pos) {
                continue;
            }

            let current_count = stored_items_opt.map(|si| si.len()).unwrap_or(0);
            if current_count >= stockpile.capacity {
                continue;
            }

            let search_radius = TILE_SIZE * 15.0;
            let nearby_resources = resource_grid.get_nearby_in_radius(stock_pos, search_radius);

            let nearest_resource = nearby_resources
                .iter()
                .filter(|&&entity| !already_assigned.contains(&entity))
                .filter_map(|&entity| {
                    let Ok((_, transform, visibility, res_item)) = q_resources.get(entity) else {
                        return None;
                    };
                    if *visibility == Visibility::Hidden {
                        return None;
                    }

                    if let Some(target_type) = stockpile.resource_type {
                        if res_item.0 != target_type {
                            return None;
                        }
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
                already_assigned.insert(item_entity);
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

/// 資材が揃った建築タスクの自動割り当てシステム
pub fn blueprint_auto_build_system(
    mut commands: Commands,
    q_familiars: Query<(Entity, &ActiveCommand, &TaskArea)>,
    q_blueprints: Query<(Entity, &Transform, &Blueprint, Option<&TaskWorkers>)>,
    q_designations: Query<(
        Entity,
        &Transform,
        &Designation,
        Option<&IssuedBy>,
        Option<&TaskSlots>,
        Option<&TaskWorkers>,
    )>,
    mut q_souls: Query<
        (
            Entity,
            &Transform,
            &crate::entities::damned_soul::DamnedSoul,
            &mut AssignedTask,
            &mut crate::entities::damned_soul::Destination,
            &mut Path,
            &crate::entities::damned_soul::IdleState,
            Option<&crate::relationships::Holding>,
            Option<&UnderCommand>,
        ),
        Without<Familiar>,
    >,
    q_breakdown: Query<&crate::entities::damned_soul::StressBreakdown>,
) {
    for (fam_entity, _active_command, task_area) in q_familiars.iter() {
        // エリア内の Blueprint を探す
        for (bp_entity, bp_transform, blueprint, workers_opt) in q_blueprints.iter() {
            let bp_pos = bp_transform.translation.truncate();
            if !task_area.contains(bp_pos) {
                continue;
            }

            // 既に作業員が割り当てられている場合はスキップ（建築中）
            if workers_opt.map(|w| w.len()).unwrap_or(0) > 0 {
                continue;
            }

            // 資材が揃っていて、まだIssuedByが付与されていない場合のみ処理
            if !blueprint.materials_complete() {
                continue;
            }

            // Designationが存在し、IssuedByが付与されていないか確認
            if let Ok((_, _, designation, issued_by_opt, _, _)) = q_designations.get(bp_entity) {
                if designation.work_type != WorkType::Build {
                    continue;
                }

                // 既に割り当てられている場合はスキップ
                if issued_by_opt.is_some() {
                    continue;
                }

                // 使い魔の部下から待機中の魂を探す
                let fatigue_threshold = 0.8; // デフォルトの疲労閾値

                // 近くの待機中の魂を探す
                let mut best_worker = None;
                let mut min_dist_sq = f32::MAX;

                for (soul_entity, soul_transform, soul, task, _, _, idle, _, uc_opt) in
                    q_souls.iter()
                {
                    // この使い魔の部下か確認
                    if let Some(uc) = uc_opt {
                        if uc.0 != fam_entity {
                            continue;
                        }
                    } else {
                        continue;
                    }

                    // 待機中で、疲労が閾値以下で、ストレス崩壊していないか確認
                    if !matches!(*task, AssignedTask::None) {
                        continue;
                    }
                    if idle.behavior
                        == crate::entities::damned_soul::IdleBehavior::ExhaustedGathering
                    {
                        continue;
                    }
                    if soul.fatigue >= fatigue_threshold {
                        continue;
                    }
                    if q_breakdown.get(soul_entity).is_ok() {
                        continue;
                    }

                    // 最も近い魂を選択
                    let dist_sq = soul_transform
                        .translation
                        .truncate()
                        .distance_squared(bp_pos);
                    if dist_sq < min_dist_sq {
                        min_dist_sq = dist_sq;
                        best_worker = Some(soul_entity);
                    }
                }

                // 見つかった魂に建築タスクを割り当て
                if let Some(worker_entity) = best_worker {
                    if let Ok((_, _, soul, mut assigned_task, mut dest, mut path, idle, _, _)) =
                        q_souls.get_mut(worker_entity)
                    {
                        if idle.behavior
                            == crate::entities::damned_soul::IdleBehavior::ExhaustedGathering
                        {
                            continue;
                        }
                        if soul.fatigue >= fatigue_threshold {
                            continue;
                        }

                        // 建築タスクを割り当て
                        use crate::systems::soul_ai::task_execution::types::BuildPhase;
                        *assigned_task = AssignedTask::Build {
                            blueprint: bp_entity,
                            phase: BuildPhase::GoingToBlueprint,
                        };
                        dest.0 = bp_pos;
                        path.waypoints = vec![bp_pos];
                        path.current_index = 0;

                        commands
                            .entity(worker_entity)
                            .insert((UnderCommand(fam_entity), WorkingOn(bp_entity)));
                        commands.entity(bp_entity).insert(IssuedBy(fam_entity));

                        info!(
                            "AUTO_BUILD: Assigned build task {:?} to worker {:?}",
                            bp_entity, worker_entity
                        );
                    }
                }
            }
        }
    }
}

/// 設計図への自動資材運搬タスク生成システム
pub fn blueprint_auto_haul_system(
    mut commands: Commands,
    resource_grid: Res<ResourceSpatialGrid>,
    q_familiars: Query<(Entity, &ActiveCommand, &TaskArea)>,
    q_blueprints: Query<(Entity, &Transform, &Blueprint, Option<&TaskWorkers>)>,
    q_resources: Query<
        (
            Entity,
            &Transform,
            &Visibility,
            &ResourceItem,
            Option<&crate::relationships::StoredIn>,
        ),
        (Without<Designation>, Without<TaskWorkers>),
    >,
    q_stockpiles: Query<&Transform, With<crate::systems::logistics::Stockpile>>,
    q_souls: Query<&AssignedTask>,
    q_all_resources: Query<&ResourceItem>,
    q_reserved_items: Query<
        (&ResourceItem, &crate::systems::jobs::TargetBlueprint),
        With<Designation>,
    >,
    mut ev_created: MessageWriter<DesignationCreatedEvent>,
) {
    // 1. 集計フェーズ: 各設計図への「運搬中」および「予約済み」の数を集計
    // (BlueprintEntity, ResourceType) -> Count
    let mut in_flight =
        std::collections::HashMap::<(Entity, crate::systems::logistics::ResourceType), usize>::new(
        );

    // 運搬中 (ソウルが持っている、または向かっている)
    for task in q_souls.iter() {
        if let AssignedTask::HaulToBlueprint {
            item, blueprint, ..
        } = task
        {
            if let Ok(res_item) = q_all_resources.get(*item) {
                *in_flight.entry((*blueprint, res_item.0)).or_insert(0) += 1;
            }
        }
    }

    // 予約済み (Designation はあるが、まだソウルに割り当てられていないアイテム)
    for (res_item, target_bp) in q_reserved_items.iter() {
        *in_flight.entry((target_bp.0, res_item.0)).or_insert(0) += 1;
    }

    let mut already_assigned_this_frame = std::collections::HashSet::new();

    for (fam_entity, _active_command, task_area) in q_familiars.iter() {
        // エリア内の Blueprint を探す
        for (bp_entity, bp_transform, blueprint, workers_opt) in q_blueprints.iter() {
            let bp_pos = bp_transform.translation.truncate();
            if !task_area.contains(bp_pos) {
                continue;
            }

            // 既に作業員が割り当てられている場合はスキップ（建築中）
            if workers_opt.map(|w| w.len()).unwrap_or(0) > 0 {
                continue;
            }

            // 資材が揃っている場合はスキップ（建築タスクへ遷移）
            if blueprint.materials_complete() {
                continue;
            }

            // 必要な資材タイプを探す
            for (resource_type, &required) in &blueprint.required_materials {
                let delivered = *blueprint
                    .delivered_materials
                    .get(resource_type)
                    .unwrap_or(&0);
                let inflight_count = *in_flight.get(&(bp_entity, *resource_type)).unwrap_or(&0);

                // 配達済み + 運搬中 + 予約済み >= 必要数 ならこれ以上探さない
                if delivered + inflight_count as u32 >= required {
                    continue;
                }

                // 近くの対応する資材を探す
                let search_radius = TILE_SIZE * 20.0;
                let nearby_resources = resource_grid.get_nearby_in_radius(bp_pos, search_radius);

                // まだ Designation が付いていないものから探す
                let matching_resource = nearby_resources
                    .iter()
                    .filter(|&&entity| !already_assigned_this_frame.contains(&entity))
                    .filter_map(|&entity| {
                        let Ok((_, transform, visibility, res_item, stored_in_opt)) =
                            q_resources.get(entity)
                        else {
                            return None;
                        };
                        if *visibility == Visibility::Hidden {
                            return None;
                        }
                        if res_item.0 != *resource_type {
                            return None;
                        }

                        // ストックパイル内にある場合、そのストックパイルが主のタスクエリア内にあるかチェック
                        if let Some(crate::relationships::StoredIn(stock_entity)) = stored_in_opt {
                            if let Ok(stock_transform) = q_stockpiles.get(*stock_entity) {
                                let stock_pos = stock_transform.translation.truncate();
                                if !task_area.contains(stock_pos) {
                                    return None;
                                }
                            } else {
                                // ストックパイルが見つからない（消失している）場合は地上扱いとするか除外するか
                                // 基本的には StoredIn があるなら存在するはず
                                return None;
                            }
                        }

                        let dist_sq = transform.translation.truncate().distance_squared(bp_pos);
                        if dist_sq < search_radius * search_radius {
                            Some((entity, dist_sq))
                        } else {
                            None
                        }
                    })
                    .min_by(|(_, d1), (_, d2)| {
                        d1.partial_cmp(d2).unwrap_or(std::cmp::Ordering::Equal)
                    })
                    .map(|(entity, _)| entity);

                if let Some(item_entity) = matching_resource {
                    already_assigned_this_frame.insert(item_entity);
                    // 次の集計に備えてカウントアップ（同フレーム内の別使い魔が重複させないため）
                    *in_flight.entry((bp_entity, *resource_type)).or_insert(0) += 1;

                    // Designation を付与
                    commands.entity(item_entity).insert((
                        Designation {
                            work_type: WorkType::Haul,
                        },
                        IssuedBy(fam_entity),
                        TaskSlots::new(1),
                        crate::systems::jobs::TargetBlueprint(bp_entity),
                    ));
                    ev_created.write(DesignationCreatedEvent {
                        entity: item_entity,
                        work_type: WorkType::Haul,
                        issued_by: Some(fam_entity),
                        priority: 1,
                    });

                    info!(
                        "AUTO_HAUL_BP: Assigned {:?} for bp {:?} (Total expected: {})",
                        resource_type,
                        bp_entity,
                        delivered + inflight_count as u32 + 1
                    );

                    // 1つ割り当てたら、この Blueprint のこのリソースについては一旦終了して次のリソースまたは次の設計図へ
                    // (複数同時に探すと1フレームで一気に割り当てすぎてしまう可能性があるが、集計ロジックがあるので基本安全)
                }
            }
        }
    }
}
