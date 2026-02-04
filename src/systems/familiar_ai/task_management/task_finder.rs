//! タスク検索モジュール
//!
//! 未割り当てのタスクを検索するロジックを提供します。

use crate::relationships::ManagedTasks;
use crate::systems::command::TaskArea;
use crate::systems::jobs::{TargetBlueprint, WorkType};
use crate::systems::logistics::ResourceType;
use crate::systems::spatial::DesignationSpatialGrid;
use crate::world::map::WorldMap;
use crate::world::pathfinding::{self, PathfindingContext};
use bevy::prelude::*;

/// 指定ワーカーの位置から到達可能な未割り当てタスクを探す
#[allow(clippy::too_many_arguments)]
pub fn find_unassigned_task_in_area(
    _fam_entity: Entity,
    fam_pos: Vec2,
    worker_pos: Vec2, // 実際に到達するかチェックするワーカーの位置
    task_area_opt: Option<&TaskArea>,
    queries: &crate::systems::soul_ai::task_execution::context::TaskAssignmentQueries,
    designation_grid: &DesignationSpatialGrid,
    managed_tasks: &ManagedTasks,
    q_target_blueprints: &Query<&TargetBlueprint>,
    world_map: &WorldMap,
    pf_context: &mut PathfindingContext,
    // haul_cache removed
) -> Vec<Entity> {
    // パス検索の起点を「ソウルの居場所」に補正する
    let worker_grid = match world_map.get_nearest_walkable_grid(worker_pos) {
        Some(g) => g,
        None => return Vec::new(),
    };

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

    let mut valid_candidates: Vec<(Entity, i32, f32)> = candidates
        .into_iter()
        .filter_map(|entity| {
            let (entity, transform, designation, issued_by, slots, workers, in_stockpile_opt, priority_opt) =
                queries.designations.get(entity).ok()?;

            let is_managed_by_me = managed_tasks.contains(entity);
            let is_unassigned = issued_by.is_none();

            // デバッグ: HaulWaterToMixerタスクの追跡
            if designation.work_type == WorkType::HaulWaterToMixer {
                debug!(
                    "TASK_FINDER: HaulWaterToMixer candidate {:?} - is_managed_by_me: {}, is_unassigned: {}",
                    entity, is_managed_by_me, is_unassigned
                );
            }

            if !is_managed_by_me && !is_unassigned {
                return None;
            }

            let current_workers = workers.map(|w| w.len()).unwrap_or(0);
            let max_slots = slots.map(|s| s.max).unwrap_or(1) as usize;
            if current_workers >= max_slots {
                // デバッグ: HaulWaterToMixerタスクのスロットチェック
                if designation.work_type == WorkType::HaulWaterToMixer {
                    debug!(
                        "TASK_FINDER: HaulWaterToMixer {:?} slots full ({}/{})",
                        entity, current_workers, max_slots
                    );
                }
                return None;
            }

            let pos = transform.translation.truncate();
            let is_mixer_task = queries.target_mixers.get(entity).is_ok();
            
            if let Some(area) = task_area_opt {
                if !area.contains(pos) {
                    if !is_managed_by_me && !is_mixer_task {
                        return None;
                    }
                }
            } else {
                if !is_managed_by_me && !is_mixer_task {
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
                // デバッグ: HaulWaterToMixerタスクの到達可能性
                if designation.work_type == WorkType::HaulWaterToMixer {
                    debug!(
                        "TASK_FINDER: HaulWaterToMixer {:?} not reachable from worker at {:?}",
                        entity, worker_pos
                    );
                }
                return None;
            }

            // 収集系は対象が実在するか追加チェック
            let is_valid = match designation.work_type {
                WorkType::Chop | WorkType::Mine | WorkType::Haul | WorkType::GatherWater | WorkType::CollectSand | WorkType::Refine | WorkType::HaulWaterToMixer => true,
                WorkType::Build => {
                    if let Ok((_, bp, _)) = queries.blueprints.get(entity) {
                        bp.materials_complete()
                    } else {
                        false
                    }
                }
            };

            if is_valid {
                // デバッグ: HaulWaterToMixerタスクが有効候補として残った
                if designation.work_type == WorkType::HaulWaterToMixer {
                    debug!(
                        "TASK_FINDER: HaulWaterToMixer {:?} is valid candidate at pos {:?}",
                        entity, pos
                    );
                }
                let dist_sq = pos.distance_squared(fam_pos);
                let mut priority = priority_opt.map(|p| p.0).unwrap_or(0) as i32;
                if designation.work_type == WorkType::Build {
                    priority += 10;
                } else if designation.work_type == WorkType::Haul {
                    if q_target_blueprints.get(entity).is_ok() {
                        priority += 10;
                    }
                    if queries.target_mixers.get(entity).is_ok() {
                        priority += 2;
                    }
                } else if designation.work_type == WorkType::GatherWater {
                    // 水汲みは基本優先度を高めに（5）
                    priority += 5;

                    // タンクの空き容量チェック
                    let bucket_belongs = queries.belongs.get(entity).ok();
                    let has_tank_space = queries.stockpiles
                        .iter()
                        .any(|(s_entity, _, stock, stored)| {
                            let is_tank = stock.resource_type == Some(ResourceType::Water);
                            let is_my_tank = bucket_belongs.map(|b| b.0) == Some(s_entity);
                            if is_tank && is_my_tank {
                                let current_count = stored.map(|s| s.len()).unwrap_or(0);
                                let reserved = queries.resource_cache.get_destination_reservation(s_entity);
                                (current_count + reserved) < stock.capacity
                            } else {
                                false
                            }
                        });

                    // 一旦ここでは所有権とタンクの存在確認に留める（詳細な容量チェックは assign_task_to_worker で行う）
                    if !has_tank_space {
                        return None;
                    }

                    if in_stockpile_opt.is_none() {
                        priority += 2;
                    }
                }

                Some((entity, priority, dist_sq))
            } else {
                None
            }
        })
        .collect();

    // 優先度が高い順、同じなら距離が近い順にソート
    valid_candidates.sort_by(|(_, p1, d1), (_, p2, d2)| {
        match p2.cmp(p1) {
            std::cmp::Ordering::Equal => {
                d1.partial_cmp(d2).unwrap_or(std::cmp::Ordering::Equal)
            }
            other => other,
        }
    });

    valid_candidates.into_iter().map(|(entity, _, _)| entity).collect()
}
