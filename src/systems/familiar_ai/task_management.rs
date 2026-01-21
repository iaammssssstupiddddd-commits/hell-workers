//! 使い魔のタスク管理モジュール
//!
//! タスクの検索・割り当てロジックを提供します。

use crate::entities::damned_soul::{DamnedSoul, Destination, IdleBehavior, IdleState, Path};
use crate::entities::familiar::UnderCommand;
use crate::events::{OnSoulRecruited, OnTaskAssigned};
use crate::relationships::{Holding, ManagedTasks, TaskWorkers};
use crate::systems::command::TaskArea;
use crate::systems::jobs::{
    Blueprint, Designation, IssuedBy, TargetBlueprint, TaskSlots, WorkType,
};
use crate::systems::logistics::{ResourceItem, ResourceType, Stockpile};
use crate::systems::soul_ai::gathering::ParticipatingIn;
use crate::systems::soul_ai::task_execution::types::{
    AssignedTask, BuildPhase, GatherPhase, HaulPhase, HaulToBpPhase,
};
use crate::systems::spatial::DesignationSpatialGrid;
use bevy::prelude::*;

/// タスク管理ユーティリティ
pub struct TaskManager;

impl TaskManager {
    /// 指定エリア内で未割り当てのタスク（Designation）を探す
    pub fn find_unassigned_task_in_area(
        _fam_entity: Entity,
        fam_pos: Vec2,
        task_area_opt: Option<&TaskArea>,
        q_designations: &Query<(
            Entity,
            &Transform,
            &Designation,
            Option<&IssuedBy>,
            Option<&TaskSlots>,
            Option<&TaskWorkers>,
        )>,
        designation_grid: &DesignationSpatialGrid,
        managed_tasks: &ManagedTasks,
        q_blueprints: &Query<&Blueprint>,
        q_target_blueprints: &Query<&TargetBlueprint>,
    ) -> Option<Entity> {
        // 候補となるエンティティのリスト
        let candidates = if let Some(area) = task_area_opt {
            // エリア指定がある場合、エリア内のタスク + 自分が管理しているタスク を対象にする
            let mut ents = designation_grid.get_in_area(area.min, area.max);

            // 自分が管理しているタスクがエリア外にある可能性も考慮（移動等）
            // ただしManagedTasksは通常少ないため、ここは個別に足しても計算量は抑えられる
            for &managed_entity in managed_tasks.iter() {
                if !ents.contains(&managed_entity) {
                    ents.push(managed_entity);
                }
            }

            // 資材が揃った建築タスク（Blueprint）を直接検索して追加
            // DesignationSpatialGridの更新タイミングの問題を回避するため
            for (bp_entity, bp_transform, bp_designation, bp_issued_by, _, _) in
                q_designations.iter()
            {
                if bp_designation.work_type == WorkType::Build {
                    let bp_pos = bp_transform.translation.truncate();
                    if area.contains(bp_pos) && bp_issued_by.is_none() {
                        // 資材が揃っているかチェック
                        if let Ok(bp) = q_blueprints.get(bp_entity) {
                            if bp.materials_complete() && !ents.contains(&bp_entity) {
                                ents.push(bp_entity);
                            }
                        }
                    }
                }
            }

            ents
        } else {
            // エリア指定がない場合、自分が管理しているタスクのみが対象
            managed_tasks.iter().copied().collect::<Vec<_>>()
        };

        candidates
            .into_iter()
            .filter_map(|entity| {
                let (entity, transform, designation, issued_by, slots, workers) =
                    q_designations.get(entity).ok()?;

                let is_managed_by_me = managed_tasks.contains(entity);
                let is_unassigned = issued_by.is_none();

                // 1. 他の使い魔が管理しているタスクは除外
                if !is_managed_by_me && !is_unassigned {
                    return None;
                }

                // 2. スロットが空いているか
                let current_workers = workers.map(|w| w.len()).unwrap_or(0);
                let max_slots = slots.map(|s| s.max).unwrap_or(1) as usize;
                if current_workers >= max_slots {
                    return None;
                }

                // 3. エリア制限のチェック
                let pos = transform.translation.truncate();
                if let Some(area) = task_area_opt {
                    if !area.contains(pos) {
                        // エリア外のタスクは、既に自分が管理しているものであっても一旦除外（パトロール範囲優先）
                        if !is_managed_by_me {
                            return None;
                        }
                    }
                } else {
                    // エリア指定がない使い魔は、明示的に割り当てられたタスク(Managed)のみ行う
                    if !is_managed_by_me {
                        return None;
                    }
                }

                // 収集系は対象が実在するか追加チェック
                let is_valid = match designation.work_type {
                    WorkType::Chop | WorkType::Mine | WorkType::Haul => true,
                    WorkType::Build => {
                        // 建築の場合、資材が揃っているかチェック
                        if let Ok(bp) = q_blueprints.get(entity) {
                            bp.materials_complete()
                        } else {
                            false
                        }
                    }
                };

                if is_valid {
                    let dist_sq = transform.translation.truncate().distance_squared(fam_pos);
                    // 優先スコアの計算
                    // 10: 建築(Build) または 設計図への運搬(Haul with TargetBlueprint)
                    // 0: その他
                    let mut priority = 0;
                    if designation.work_type == WorkType::Build {
                        priority = 10;
                    } else if designation.work_type == WorkType::Haul {
                        if q_target_blueprints.get(entity).is_ok() {
                            priority = 10;
                        }
                    }

                    Some((entity, priority, dist_sq))
                } else {
                    None
                }
            })
            .min_by(|(_, p1, d1), (_, p2, d2)| {
                // 優先度が高い(大きい)ものを優先
                match p2.cmp(p1) {
                    std::cmp::Ordering::Equal => {
                        d1.partial_cmp(d2).unwrap_or(std::cmp::Ordering::Equal)
                    }
                    other => other,
                }
            })
            .map(|(entity, _, _)| entity)
    }

    /// ワーカーにタスクを割り当てる
    #[allow(clippy::too_many_arguments)]
    pub fn assign_task_to_worker(
        commands: &mut Commands,
        fam_entity: Entity,
        task_entity: Entity,
        worker_entity: Entity,
        fatigue_threshold: f32,
        q_designations: &Query<(
            Entity,
            &Transform,
            &Designation,
            Option<&IssuedBy>,
            Option<&TaskSlots>,
            Option<&TaskWorkers>,
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
                Option<&Holding>,
                Option<&UnderCommand>,
                Option<&ParticipatingIn>,
            ),
            Without<crate::entities::familiar::Familiar>,
        >,
        q_stockpiles: &Query<(
            Entity,
            &Transform,
            &Stockpile,
            Option<&crate::relationships::StoredItems>,
        )>,
        q_resources: &Query<&ResourceItem>,
        q_target_blueprints: &Query<&TargetBlueprint>,
        q_blueprints: &Query<&Blueprint>,
        task_area_opt: Option<&TaskArea>,
        haul_cache: &mut crate::systems::familiar_ai::haul_cache::HaulReservationCache,
    ) {
        let Ok((_, _, soul, mut assigned_task, mut dest, mut path, idle, _, uc_opt, participating_opt)) =
            q_souls.get_mut(worker_entity)
        else {
            warn!("ASSIGN: Worker {:?} not found in query", worker_entity);
            return;
        };

        // もし集会に参加中なら抜ける
        if let Some(p) = participating_opt {
            commands.entity(worker_entity).remove::<ParticipatingIn>();
            commands.trigger(crate::events::OnGatheringLeft {
                entity: worker_entity,
                spot_entity: p.0,
            });
        }

        if idle.behavior == IdleBehavior::ExhaustedGathering {
            return;
        }

        if soul.fatigue >= fatigue_threshold {
            return;
        }

        // タスクが存在するか最終確認
        let (task_pos, work_type) =
            if let Ok((_, transform, designation, _, _, _)) = q_designations.get(task_entity) {
                (transform.translation.truncate(), designation.work_type)
            } else {
                return;
            };

        match work_type {
            WorkType::Chop | WorkType::Mine => {
                if uc_opt.is_none() {
                    commands.trigger(OnSoulRecruited {
                        entity: worker_entity,
                        familiar_entity: fam_entity,
                    });
                }

                commands.entity(worker_entity).insert((
                    UnderCommand(fam_entity),
                    crate::relationships::WorkingOn(task_entity),
                ));
                commands
                    .entity(task_entity)
                    .insert(crate::systems::jobs::IssuedBy(fam_entity));

                *assigned_task = AssignedTask::Gather {
                    target: task_entity,
                    work_type,
                    phase: GatherPhase::GoingToResource,
                };
                dest.0 = task_pos;
                path.waypoints = vec![task_pos];
                path.current_index = 0;
                commands.trigger(OnTaskAssigned {
                    entity: worker_entity,
                    task_entity,
                    work_type,
                });
            }
            WorkType::Haul => {
                if let Ok(target_bp) = q_target_blueprints.get(task_entity) {
                    if uc_opt.is_none() {
                        commands.trigger(OnSoulRecruited {
                            entity: worker_entity,
                            familiar_entity: fam_entity,
                        });
                    }

                    commands.entity(worker_entity).insert((
                        UnderCommand(fam_entity),
                        crate::relationships::WorkingOn(task_entity),
                    ));
                    commands
                        .entity(task_entity)
                        .insert(crate::systems::jobs::IssuedBy(fam_entity));

                    *assigned_task = AssignedTask::HaulToBlueprint {
                        item: task_entity,
                        blueprint: target_bp.0,
                        phase: HaulToBpPhase::GoingToItem,
                    };
                    dest.0 = task_pos;
                    path.waypoints = vec![task_pos];
                    path.current_index = 0;
                    commands.trigger(OnTaskAssigned {
                        entity: worker_entity,
                        task_entity,
                        work_type: WorkType::Haul,
                    });
                    return;
                }

                let item_type = q_resources
                    .get(task_entity)
                    .map(|ri| ri.0)
                    .unwrap_or(ResourceType::Wood);

                let best_stockpile = q_stockpiles
                    .iter()
                    .filter(|(s_entity, s_transform, stock, stored)| {
                        if let Some(area) = task_area_opt {
                            if !area.contains(s_transform.translation.truncate()) {
                                return false;
                            }
                        }

                        let type_match =
                            stock.resource_type.is_none() || stock.resource_type == Some(item_type);

                        let current_count = stored.map(|s| s.len()).unwrap_or(0);
                        let reserved = haul_cache.get(*s_entity);
                        let has_capacity = (current_count + reserved) < stock.capacity as usize;

                        type_match && has_capacity
                    })
                    .min_by(|(_, t1, _, _), (_, t2, _, _)| {
                        let d1 = t1.translation.truncate().distance_squared(task_pos);
                        let d2 = t2.translation.truncate().distance_squared(task_pos);
                        d1.partial_cmp(&d2).unwrap_or(std::cmp::Ordering::Equal)
                    })
                    .map(|(e, _, _, _)| e);

                if let Some(stock_entity) = best_stockpile {
                    if uc_opt.is_none() {
                        commands.trigger(OnSoulRecruited {
                            entity: worker_entity,
                            familiar_entity: fam_entity,
                        });
                    }

                    commands.entity(worker_entity).insert((
                        UnderCommand(fam_entity),
                        crate::relationships::WorkingOn(task_entity),
                    ));
                    commands
                        .entity(task_entity)
                        .insert(crate::systems::jobs::IssuedBy(fam_entity));

                    *assigned_task = AssignedTask::Haul {
                        item: task_entity,
                        stockpile: stock_entity,
                        phase: HaulPhase::GoingToItem,
                    };
                    haul_cache.reserve(stock_entity);

                    dest.0 = task_pos;
                    path.waypoints = vec![task_pos];
                    path.current_index = 0;
                    commands.trigger(OnTaskAssigned {
                        entity: worker_entity,
                        task_entity,
                        work_type: WorkType::Haul,
                    });
                }
            }
            WorkType::Build => {
                if let Ok(bp) = q_blueprints.get(task_entity) {
                    if !bp.materials_complete() {
                        return;
                    }
                }

                if uc_opt.is_none() {
                    commands.trigger(OnSoulRecruited {
                        entity: worker_entity,
                        familiar_entity: fam_entity,
                    });
                }

                commands.entity(worker_entity).insert((
                    UnderCommand(fam_entity),
                    crate::relationships::WorkingOn(task_entity),
                ));
                commands
                    .entity(task_entity)
                    .insert(crate::systems::jobs::IssuedBy(fam_entity));

                *assigned_task = AssignedTask::Build {
                    blueprint: task_entity,
                    phase: BuildPhase::GoingToBlueprint,
                };
                dest.0 = task_pos;
                path.waypoints = vec![task_pos];
                path.current_index = 0;
                commands.trigger(OnTaskAssigned {
                    entity: worker_entity,
                    task_entity,
                    work_type: WorkType::Build,
                });
            }
        }
    }

    /// 分隊内のアイドルメンバーを検索
    pub fn find_idle_member(
        squad: &[Entity],
        fatigue_threshold: f32,
        q_souls: &mut Query<
            (
                Entity,
                &Transform,
                &DamnedSoul,
                &mut AssignedTask,
                &mut Destination,
                &mut Path,
                &IdleState,
                Option<&Holding>,
                Option<&UnderCommand>,
                Option<&ParticipatingIn>,
            ),
            Without<crate::entities::familiar::Familiar>,
        >,
    ) -> Option<Entity> {
        for &member_entity in squad {
            if let Ok((_, _, soul, task, _, _, idle, _, _, _)) = q_souls.get(member_entity) {
                if matches!(*task, AssignedTask::None)
                    && idle.behavior != IdleBehavior::ExhaustedGathering
                    && soul.fatigue < fatigue_threshold
                {
                    return Some(member_entity);
                }
            }
        }
        None
    }

    /// タスクを委譲する（タスク検索 + 割り当て）
    pub fn delegate_task(
        commands: &mut Commands,
        fam_entity: Entity,
        fam_pos: Vec2,
        squad: &[Entity],
        task_area_opt: Option<&TaskArea>,
        fatigue_threshold: f32,
        q_designations: &Query<(
            Entity,
            &Transform,
            &Designation,
            Option<&IssuedBy>,
            Option<&TaskSlots>,
            Option<&TaskWorkers>,
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
                Option<&Holding>,
                Option<&UnderCommand>,
                Option<&ParticipatingIn>,
            ),
            Without<crate::entities::familiar::Familiar>,
        >,
        q_stockpiles: &Query<(
            Entity,
            &Transform,
            &Stockpile,
            Option<&crate::relationships::StoredItems>,
        )>,
        q_resources: &Query<&ResourceItem>,
        q_target_blueprints: &Query<&TargetBlueprint>,
        q_blueprints: &Query<&Blueprint>,
        designation_grid: &DesignationSpatialGrid,
        managed_tasks: &ManagedTasks,
        haul_cache: &mut crate::systems::familiar_ai::haul_cache::HaulReservationCache,
    ) -> Option<Entity> {
        // タスクを検索
        let task_entity = Self::find_unassigned_task_in_area(
            fam_entity,
            fam_pos,
            task_area_opt,
            q_designations,
            designation_grid,
            managed_tasks,
            q_blueprints,
            q_target_blueprints,
        )?;

        // アイドルメンバーを検索
        let idle_member = Self::find_idle_member(squad, fatigue_threshold, q_souls)?;

        // タスクを割り当て（assign_task_to_worker 内で IssuedBy も設定される）
        Self::assign_task_to_worker(
            commands,
            fam_entity,
            task_entity,
            idle_member,
            fatigue_threshold,
            q_designations,
            q_souls,
            q_stockpiles,
            q_resources,
            q_target_blueprints,
            q_blueprints,
            task_area_opt,
            haul_cache,
        );

        Some(task_entity)
    }
}
