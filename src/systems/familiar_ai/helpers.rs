use crate::constants::TILE_SIZE;
use crate::entities::damned_soul::{
    DamnedSoul, Destination, IdleBehavior, IdleState, Path, StressBreakdown,
};
use crate::entities::familiar::Familiar;
use crate::relationships::{
    CommandedBy as UnderCommand, ManagedBy as IssuedBy, ManagedTasks, TaskWorkers, WorkingOn,
};
use crate::systems::command::TaskArea;
use crate::systems::jobs::{Designation, TaskSlots, WorkType};
use crate::systems::logistics::Stockpile;
use crate::systems::spatial::SpatialGrid;
use crate::systems::work::{AssignedTask, GatherPhase, HaulPhase};
use bevy::prelude::*;

/// 最も近い「フリーな」ワーカーをスカウト対象として探す
pub fn find_best_recruit(
    fam_pos: Vec2,
    fatigue_threshold: f32,
    _min_fatigue: f32,
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
        ),
        Without<Familiar>,
    >,
    q_breakdown: &Query<&StressBreakdown>,
    radius_opt: Option<f32>,
) -> Option<Entity> {
    let mut candidates = Vec::new();

    if let Some(radius) = radius_opt {
        let nearby = spatial_grid.get_nearby_in_radius(fam_pos, radius);
        for &e in &nearby {
            if let Ok((entity, transform, soul, task, _, _, idle, _, uc)) = q_souls.get(e) {
                let is_gathering = idle.behavior == IdleBehavior::Gathering;
                let fatigue_ok = is_gathering || soul.fatigue < fatigue_threshold;
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
        for (entity, transform, soul, task, _, _, idle, _, uc) in q_souls.iter() {
            let is_gathering = idle.behavior == IdleBehavior::Gathering;
            let fatigue_ok = is_gathering || soul.fatigue < fatigue_threshold;
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
pub fn find_unassigned_task_in_area(
    _fam_entity: Entity,
    fam_pos: Vec2,
    _task_area_opt: Option<&TaskArea>,
    q_designations: &Query<(
        Entity,
        &Transform,
        &Designation,
        Option<&IssuedBy>,
        Option<&TaskSlots>,
        Option<&TaskWorkers>,
    )>,
    managed_tasks: &ManagedTasks,
) -> Option<Entity> {
    let mut best_task = None;
    let mut best_dist = f32::MAX;

    // 1. 自分が管理しているタスクから探す (ManagedTasks ターゲットを利用)
    for &entity in managed_tasks.iter() {
        if let Ok((entity, transform, _, _, slots_opt, workers_opt)) = q_designations.get(entity) {
            let pos = transform.translation.truncate();

            let has_slot = if let Some(slots) = slots_opt {
                let current = workers_opt.map(|w| w.len()).unwrap_or(0);
                (current as u32) < slots.max
            } else {
                true
            };

            if !has_slot {
                continue;
            }

            let dist = fam_pos.distance_squared(pos);
            if dist < best_dist {
                best_dist = dist;
                best_task = Some(entity);
            }
        }
    }

    if best_task.is_some() {
        return best_task;
    }

    // 2. 自分が管理していない未アサインタスクをエリア内で探す
    for (entity, transform, _designation, issued_by_opt, slots_opt, workers_opt) in
        q_designations.iter()
    {
        if issued_by_opt.is_some() {
            continue; // すでに誰かが管理している
        }

        let has_slot = if let Some(slots) = slots_opt {
            let current = workers_opt.map(|w| w.len()).unwrap_or(0);
            (current as u32) < slots.max
        } else {
            true
        };
        if !has_slot {
            continue;
        }

        let pos = transform.translation.truncate();

        // エリア内チェック (未アサインタスクのみチェック)
        if let Some(area) = _task_area_opt {
            let margin = TILE_SIZE * 2.0;
            if pos.x < area.min.x - margin
                || pos.x > area.max.x + margin
                || pos.y < area.min.y - margin
                || pos.y > area.max.y + margin
            {
                continue;
            }
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
            Option<&crate::relationships::Holding>,
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

    if idle.behavior == IdleBehavior::ExhaustedGathering {
        return;
    }

    if soul.fatigue >= fatigue_threshold {
        return;
    }

    let (task_pos, work_type) =
        if let Ok((_, transform, designation, _, _, _)) = q_designations.get(task_entity) {
            (transform.translation.truncate(), designation.work_type)
        } else {
            return;
        };

    match work_type {
        WorkType::Chop | WorkType::Mine => {
            *assigned_task = AssignedTask::Gather {
                target: task_entity,
                work_type,
                phase: GatherPhase::GoingToResource,
            };
        }
        WorkType::Haul => {
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
                return;
            }
        }
        _ => return,
    }

    // スロットのインクリメントは不要（Relationshipにより自動管理）
    // if let Ok((..., mut slots_opt)) = q_designations.get_mut(task_entity) { ... }

    dest.0 = task_pos;
    path.waypoints.clear();

    commands
        .entity(worker_entity)
        .insert(UnderCommand(fam_entity));

    // ワーカー側に WorkingOn を付与（タスク側の TaskWorkers は自動更新される）
    commands
        .entity(worker_entity)
        .insert(WorkingOn(task_entity));
}
