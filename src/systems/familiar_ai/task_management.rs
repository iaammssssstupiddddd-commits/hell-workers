//! 使い魔のタスク管理モジュール
//!
//! タスクの検索・割り当てロジックを提供します。

use crate::entities::damned_soul::{DamnedSoul, Destination, IdleBehavior, IdleState, Path};
use crate::entities::familiar::UnderCommand;
use crate::events::{OnSoulRecruited, OnTaskAssigned};
use crate::relationships::ManagedTasks;
use crate::systems::command::TaskArea;
use crate::systems::jobs::{TargetBlueprint, WorkType};
use crate::systems::logistics::ResourceType;
use crate::systems::soul_ai::gathering::ParticipatingIn;
use crate::systems::soul_ai::task_execution::types::{
    AssignedTask, BuildPhase, GatherPhase, GatherWaterPhase, HaulPhase, HaulToBpPhase,
};
use crate::systems::spatial::DesignationSpatialGrid;
use crate::world::map::WorldMap;
use crate::world::pathfinding::{self, PathfindingContext};
use bevy::prelude::*;

/// タスク管理ユーティリティ
pub struct TaskManager;

impl TaskManager {
    /// ワーカーにタスク割り当てのための共通セットアップを行う
    fn prepare_worker_for_task(
        commands: &mut Commands,
        worker_entity: Entity,
        fam_entity: Entity,
        task_entity: Entity,
        already_commanded: bool,
    ) {
        if !already_commanded {
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
    }

    /// 指定ワーカーの位置から到達可能な未割り当てタスクを探す
    #[allow(clippy::too_many_arguments)]
    pub fn find_unassigned_task_in_area(
        _fam_entity: Entity,
        fam_pos: Vec2,
        worker_pos: Vec2, // 実際に到達するかチェックするワーカーの位置
        task_area_opt: Option<&TaskArea>,
        queries: &crate::systems::soul_ai::task_execution::context::TaskQueries,
        designation_grid: &DesignationSpatialGrid,
        managed_tasks: &ManagedTasks,
        q_target_blueprints: &Query<&TargetBlueprint>,
        world_map: &WorldMap,
        pf_context: &mut PathfindingContext,
    ) -> Option<Entity> {
        // パス検索の起点を「ソウルの居場所」に補正する
        let worker_grid = world_map.get_nearest_walkable_grid(worker_pos)?;

        // 候補となるエンティティのリスト
        let candidates = if let Some(area) = task_area_opt {
            // エリア指定がある場合、エリア内のタスク + 自分が管理しているタスク を対象にする
            let mut ents = designation_grid.get_in_area(area.min, area.max);

            for &managed_entity in managed_tasks.iter() {
                if !ents.contains(&managed_entity) {
                    ents.push(managed_entity);
                }
            }

            // 資材が揃った建築タスク（Blueprint）を直接検索して追加
            for (bp_entity, bp_transform, bp_designation, bp_issued_by, _, _, _, _) in
                queries.designations.iter()
            {
                if bp_designation.work_type == WorkType::Build {
                    let bp_pos = bp_transform.translation.truncate();
                    if area.contains(bp_pos) && bp_issued_by.is_none() {
                        if let Ok((_, bp, _)) = queries.blueprints.get(bp_entity) {
                            if bp.materials_complete() && !ents.contains(&bp_entity) {
                                ents.push(bp_entity);
                            }
                        }
                    }
                }
            }

            ents
        } else {
            managed_tasks.iter().copied().collect::<Vec<_>>()
        };

        candidates
            .into_iter()
            .filter_map(|entity| {
                let (entity, transform, designation, issued_by, slots, workers, in_stockpile_opt, priority_opt) =
                    queries.designations.get(entity).ok()?;

                let is_managed_by_me = managed_tasks.contains(entity);
                let is_unassigned = issued_by.is_none();

                if !is_managed_by_me && !is_unassigned {
                    return None;
                }

                let current_workers = workers.map(|w| w.len()).unwrap_or(0);
                let max_slots = slots.map(|s| s.max).unwrap_or(1) as usize;
                if current_workers >= max_slots {
                    return None;
                }

                let pos = transform.translation.truncate();
                if let Some(area) = task_area_opt {
                    if !area.contains(pos) {
                        if !is_managed_by_me {
                            return None;
                        }
                    }
                } else {
                    if !is_managed_by_me {
                        return None;
                    }
                }

                // 4. 到達可能性チェック（逆引き検索: タスクからソウルまで歩けるかチェック）
                let target_grid = WorldMap::world_to_grid(pos);


                let is_reachable = if world_map.is_walkable(target_grid.0, target_grid.1) {
                    // 通常アイテム（通行可能位置）: 
                    // 1. まずその場所まで直接いけるか試す
                    // 2. 失敗した場合、隣接地点までいけるか試す（アイテムが狭い場所にある場合など）
                    if pathfinding::find_path(world_map, pf_context, target_grid, worker_grid).is_some() {
                        true
                    } else {
                        pathfinding::find_path_to_adjacent(world_map, pf_context, worker_grid, target_grid).is_some()
                    }
                } else {
                    // 障害物（岩・木など）: 隣接マスまで行けるか
                    pathfinding::find_path_to_adjacent(world_map, pf_context, worker_grid, target_grid).is_some()
                };

                if !is_reachable {
                    return None;
                }

                // 収集系は対象が実在するか追加チェック
                let is_valid = match designation.work_type {
                    WorkType::Chop | WorkType::Mine | WorkType::Haul | WorkType::GatherWater => true,
                    WorkType::Build => {
                        if let Ok((_, bp, _)) = queries.blueprints.get(entity) {
                            bp.materials_complete()
                        } else {
                            false
                        }
                    }
                };

                if is_valid {
                    let dist_sq = pos.distance_squared(fam_pos);
                    let mut priority = priority_opt.map(|p| p.0).unwrap_or(0) as i32;
                    if designation.work_type == WorkType::Build {
                        priority += 10;
                    } else if designation.work_type == WorkType::Haul {
                        if q_target_blueprints.get(entity).is_ok() {
                            priority += 10;
                        }
                    } else if designation.work_type == WorkType::GatherWater {
                        // 水汲みは基本優先度を高めに（5）
                        priority += 5;
                        // 備蓄場所にあっても優先度を維持するが、地面にあればさらに少し上乗せ?
                        // 一旦一律 +5 にして、地面ならさらに +2 など
                        if in_stockpile_opt.is_none() {
                            priority += 2;
                        }
                    }

                    Some((entity, priority, dist_sq))
                } else {
                    None
                }
            })
            .min_by(|(_, p1, d1): &(Entity, i32, f32), (_, p2, d2): &(Entity, i32, f32)| {
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
        queries: &crate::systems::soul_ai::task_execution::context::TaskQueries,
        q_souls: &mut Query<
            (
                Entity,
                &Transform,
                &DamnedSoul,
                &mut AssignedTask,
                &mut Destination,
                &mut Path,
                &IdleState,

                Option<&mut crate::systems::logistics::Inventory>,
                Option<&UnderCommand>,
                Option<&ParticipatingIn>,
            ),
            Without<crate::entities::familiar::Familiar>,
        >,
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
            if let Ok((_, transform, designation, _, _, _, _, _)) = queries.designations.get(task_entity) {
                (transform.translation.truncate(), designation.work_type)
            } else {
                return;
            };

        match work_type {
            WorkType::Chop | WorkType::Mine => {
                Self::prepare_worker_for_task(
                    commands, worker_entity, fam_entity, task_entity, uc_opt.is_some(),
                );

                *assigned_task = AssignedTask::Gather(crate::systems::soul_ai::task_execution::types::GatherData {
                    target: task_entity,
                    work_type,
                    phase: GatherPhase::GoingToResource,
                });
                dest.0 = task_pos;
                path.waypoints.clear();
                path.current_index = 0;
                commands.trigger(OnTaskAssigned {
                    entity: worker_entity,
                    task_entity,
                    work_type,
                });
            }
            WorkType::Haul => {
                if let Ok(target_bp) = queries.target_blueprints.get(task_entity) {
                    Self::prepare_worker_for_task(
                        commands, worker_entity, fam_entity, task_entity, uc_opt.is_some(),
                    );

                    *assigned_task = AssignedTask::HaulToBlueprint(crate::systems::soul_ai::task_execution::types::HaulToBlueprintData {
                        item: task_entity,
                        blueprint: target_bp.0,
                        phase: HaulToBpPhase::GoingToItem,
                    });
                    dest.0 = task_pos;
                    path.waypoints.clear();
                    path.current_index = 0;
                    commands.trigger(OnTaskAssigned {
                        entity: worker_entity,
                        task_entity,
                        work_type: WorkType::Haul,
                    });
                    return;
                }

                let item_info = queries.items.get(task_entity).ok().map(|(it, _)| it.0);
                let item_belongs = queries.belongs.get(task_entity).ok();

                if item_info.is_none() {
                    return;
                }
                let item_type = item_info.unwrap();

                let best_stockpile = queries.stockpiles
                    .iter()
                    .filter(|(s_entity, s_transform, stock, stored)| {
                        if let Some(area) = task_area_opt {
                            if !area.contains(s_transform.translation.truncate()) {
                                return false;
                            }
                        }

                        // 所有権チェック
                        let stock_belongs = queries.belongs.get(*s_entity).ok();
                        if item_belongs != stock_belongs {
                            return false;
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
                    Self::prepare_worker_for_task(
                        commands, worker_entity, fam_entity, task_entity, uc_opt.is_some(),
                    );

                    *assigned_task = AssignedTask::Haul(crate::systems::soul_ai::task_execution::types::HaulData {
                        item: task_entity,
                        stockpile: stock_entity,
                        phase: HaulPhase::GoingToItem,
                    });
                    haul_cache.reserve(stock_entity);

                    dest.0 = task_pos;
                    path.waypoints.clear();
                    path.current_index = 0;
                    commands.trigger(OnTaskAssigned {
                        entity: worker_entity,
                        task_entity,
                        work_type: WorkType::Haul,
                    });
                }
            }
            WorkType::Build => {
                if let Ok((_, bp, _)) = queries.blueprints.get(task_entity) {
                    if !bp.materials_complete() {
                        return;
                    }
                }

                Self::prepare_worker_for_task(
                    commands, worker_entity, fam_entity, task_entity, uc_opt.is_some(),
                );

                *assigned_task = AssignedTask::Build(crate::systems::soul_ai::task_execution::types::BuildData {
                    blueprint: task_entity,
                    phase: BuildPhase::GoingToBlueprint,
                });
                dest.0 = task_pos;
                path.waypoints.clear();
                path.current_index = 0;
                commands.trigger(OnTaskAssigned {
                    entity: worker_entity,
                    task_entity,
                    work_type: WorkType::Build,
                });
            }
            WorkType::GatherWater => {
                let best_tank = queries.stockpiles
                    .iter()
                    .filter(|(s_entity, s_transform, stock, stored)| {
                        if let Some(area) = task_area_opt {
                            if !area.contains(s_transform.translation.truncate()) {
                                return false;
                            }
                        }
                        let is_tank = stock.resource_type == Some(ResourceType::Water);
                        let current_count = stored.map(|s| s.len()).unwrap_or(0);
                        let reserved = haul_cache.get(*s_entity);
                        let has_capacity = (current_count + reserved) < stock.capacity;

                        // 所有権チェック（バケツとタンク）
                        let bucket_belongs = queries.belongs.get(task_entity).ok();
                        let _tank_belongs = Some(&crate::systems::logistics::BelongsTo(*s_entity)); // タンク自身への帰属
                        let is_my_tank = bucket_belongs.map(|b| b.0) == Some(*s_entity);

                        is_tank && has_capacity && is_my_tank
                    })
                    .min_by(|(_, t1, _, _), (_, t2, _, _)| {
                        let d1 = t1.translation.truncate().distance_squared(task_pos);
                        let d2 = t2.translation.truncate().distance_squared(task_pos);
                        d1.partial_cmp(&d2).unwrap_or(std::cmp::Ordering::Equal)
                    })
                    .map(|(e, _, _, _)| e);

                if let Some(tank_entity) = best_tank {
                    Self::prepare_worker_for_task(
                        commands,
                        worker_entity,
                        fam_entity,
                        task_entity,
                        uc_opt.is_some(),
                    );

                    *assigned_task = AssignedTask::GatherWater(crate::systems::soul_ai::task_execution::types::GatherWaterData {
                        bucket: task_entity,
                        tank: tank_entity,
                        phase: GatherWaterPhase::GoingToBucket,
                    });
                    haul_cache.reserve(tank_entity);

                    dest.0 = task_pos;
                    path.waypoints.clear();
                    path.current_index = 0;
                    commands.trigger(OnTaskAssigned {
                        entity: worker_entity,
                        task_entity,
                        work_type: WorkType::GatherWater,
                    });
                }
            }
        }
    }

    /// 分隊内のアイドルメンバーを検索
    /// タスクを委譲する（タスク検索 + 割り当て）
    pub fn delegate_task(
        commands: &mut Commands,
        fam_entity: Entity,
        fam_pos: Vec2,
        squad: &[Entity],
        task_area_opt: Option<&TaskArea>,
        fatigue_threshold: f32,
        queries: &crate::systems::soul_ai::task_execution::context::TaskQueries,
        q_souls: &mut Query<
            (
                Entity,
                &Transform,
                &DamnedSoul,
                &mut AssignedTask,
                &mut Destination,
                &mut Path,
                &IdleState,

                Option<&mut crate::systems::logistics::Inventory>,
                Option<&UnderCommand>,
                Option<&ParticipatingIn>,
            ),
            Without<crate::entities::familiar::Familiar>,
        >,
        designation_grid: &DesignationSpatialGrid,
        managed_tasks: &ManagedTasks,
        haul_cache: &mut crate::systems::familiar_ai::haul_cache::HaulReservationCache,
        world_map: &WorldMap,
        pf_context: &mut PathfindingContext,
    ) -> Option<Entity> {
        // 1. 公平性/効率のため、アイドルメンバーを全員リストアップ
        let mut idle_members = Vec::new();
        for &member_entity in squad {
            if let Ok(soul_data) = q_souls.get(member_entity) {
                let (_, transform, soul, task, _, _, idle, _, _, _) = soul_data;
                if matches!(*task, AssignedTask::None)
                    && idle.behavior != IdleBehavior::ExhaustedGathering
                    && soul.fatigue < fatigue_threshold
                {
                    idle_members.push((member_entity, transform.translation.truncate()));
                }
            }
        }

        // 2. 各メンバーに対して最適なタスクを一つずつ探して試みる
        for (worker_entity, pos) in idle_members {
            if let Some(task_entity) = Self::find_unassigned_task_in_area(
                fam_entity,
                fam_pos,
                pos, // 個別ソウルの位置を使用
                task_area_opt,
                queries,
                designation_grid,
                managed_tasks,
                &queries.target_blueprints,
                world_map,
                pf_context,
            ) {
                // アサイン成功！1サイクル1人へのアサインとする（安定性のため）
                Self::assign_task_to_worker(
                    commands,
                    fam_entity,
                    task_entity,
                    worker_entity,
                    fatigue_threshold,
                    queries,
                    q_souls,
                    task_area_opt,
                    haul_cache,
                );
                return Some(task_entity);
            }
        }

        None
    }
}
