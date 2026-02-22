//! タスク割り当てモジュール
//!
//! ワーカーへのタスク割り当てロジックを提供します。

use crate::entities::damned_soul::IdleBehavior;
use crate::systems::command::TaskArea;
use crate::systems::familiar_ai::FamiliarSoulQuery;
use crate::systems::logistics::ResourceType;

use bevy::prelude::*;
use std::collections::HashMap;

#[derive(Clone, Copy, Debug)]
pub(crate) struct CachedSourceItem {
    pub entity: Entity,
    pub pos: Vec2,
}

#[derive(Default)]
pub(crate) struct SourceSelectorFrameCache {
    pub by_resource: HashMap<ResourceType, Vec<CachedSourceItem>>,
    pub by_resource_owner_ground: HashMap<(ResourceType, Option<Entity>), Vec<CachedSourceItem>>,
    pub by_resource_stockpile: HashMap<(ResourceType, Entity), Vec<Entity>>,
}

/// Thinkフェーズ内の予約増分を追跡する
#[derive(Default)]
pub struct ReservationShadow {
    mixer_destination: HashMap<(Entity, crate::systems::logistics::ResourceType), usize>,
    source: HashMap<Entity, usize>,
    pub(crate) source_selector_cache: Option<SourceSelectorFrameCache>,
}

impl ReservationShadow {
    pub fn mixer_reserved(
        &self,
        target: Entity,
        resource_type: crate::systems::logistics::ResourceType,
    ) -> usize {
        self.mixer_destination
            .get(&(target, resource_type))
            .cloned()
            .unwrap_or(0)
    }

    pub fn source_reserved(&self, source: Entity) -> usize {
        self.source.get(&source).cloned().unwrap_or(0)
    }

    pub fn apply_reserve_ops(&mut self, ops: &[crate::events::ResourceReservationOp]) {
        for op in ops {
            match *op {
                crate::events::ResourceReservationOp::ReserveMixerDestination {
                    target,
                    resource_type,
                } => {
                    *self
                        .mixer_destination
                        .entry((target, resource_type))
                        .or_insert(0) += 1;
                }
                crate::events::ResourceReservationOp::ReserveSource { source, amount } => {
                    *self.source.entry(source).or_insert(0) += amount;
                }
                _ => {}
            }
        }
    }
}

pub struct AssignTaskContext<'a> {
    pub fam_entity: Entity,
    pub task_entity: Entity,
    pub worker_entity: Entity,
    pub fatigue_threshold: f32,
    pub task_area_opt: Option<&'a TaskArea>,
}

/// ワーカーにタスクを割り当てる
pub fn assign_task_to_worker(
    ctx: AssignTaskContext<'_>,
    queries: &mut crate::systems::soul_ai::execute::task_execution::context::TaskAssignmentQueries,
    q_souls: &mut FamiliarSoulQuery,
    shadow: &mut ReservationShadow,
) -> bool {
    let Ok((_, _, soul, _assigned_task, _dest, _path, idle, _, uc_opt, _participating_opt)) =
        q_souls.get_mut(ctx.worker_entity)
    else {
        warn!("ASSIGN: Worker {:?} not found in query", ctx.worker_entity);
        return false;
    };

    if idle.behavior == IdleBehavior::ExhaustedGathering {
        debug!(
            "ASSIGN: Worker {:?} is exhausted gathering",
            ctx.worker_entity
        );
        return false;
    }

    if soul.fatigue > ctx.fatigue_threshold {
        debug!(
            "ASSIGN: Worker {:?} is too fatigued ({:.2} > {:.2})",
            ctx.worker_entity, soul.fatigue, ctx.fatigue_threshold
        );
        return false;
    }

    // タスクが存在するか最終確認
    let (task_pos, work_type) = if let Ok((_, transform, designation, _, _, _, _, _)) =
        queries.designation.designations.get(ctx.task_entity)
    {
        (transform.translation.truncate(), designation.work_type)
    } else {
        debug!("ASSIGN: Task designation {:?} disappeared", ctx.task_entity);
        return false;
    };

    super::policy::assign_by_work_type(work_type, task_pos, uc_opt.is_some(), &ctx, queries, shadow)
}
