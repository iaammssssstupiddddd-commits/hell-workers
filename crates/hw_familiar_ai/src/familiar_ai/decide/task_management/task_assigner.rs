//! タスク割り当てモジュール

use bevy::prelude::*;
use hw_core::area::TaskArea;
use hw_core::logistics::ResourceType;
use hw_core::relationships::{CommandedBy, ParticipatingIn};
use hw_core::soul::{DamnedSoul, Destination, IdleState, Path};
use hw_energy::constants::DREAM_GENERATE_ASSIGN_THRESHOLD;
use hw_jobs::AssignedTask;
use hw_jobs::WorkType;
use hw_logistics::tile_index::TileSiteIndex;
use hw_logistics::types::Inventory;
use std::collections::HashMap;

use crate::familiar_ai::decide::task_management::context::ConstructionSitePositions;
use crate::familiar_ai::decide::task_management::{
    FamiliarTaskAssignmentQueries, IncomingDeliverySnapshot,
};
use hw_core::events::ResourceReservationOp;

/// Familiar AI が扱うソウルの標準クエリ型
pub type FamiliarSoulQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Transform,
        &'static DamnedSoul,
        &'static mut AssignedTask,
        &'static mut Destination,
        &'static mut Path,
        &'static IdleState,
        Option<&'static mut Inventory>,
        Option<&'static CommandedBy>,
        Option<&'static ParticipatingIn>,
    ),
    Without<hw_core::familiar::Familiar>,
>;

#[derive(Clone, Copy, Debug)]
pub(crate) struct CachedSourceItem {
    pub entity: Entity,
    pub pos: Vec2,
}

#[derive(Default)]
pub(crate) struct SourceSelectorFrameCache {
    pub by_resource_stockpile: HashMap<(ResourceType, Entity), Vec<Entity>>,
}

/// Thinkフェーズ内の予約増分を追跡する
#[derive(Default)]
pub struct ReservationShadow {
    mixer_destination: HashMap<(Entity, ResourceType), usize>,
    destination_total: HashMap<Entity, usize>,
    destination_by_resource: HashMap<(Entity, ResourceType), usize>,
    source: HashMap<Entity, usize>,
    pub(crate) source_selector_cache: Option<SourceSelectorFrameCache>,
}

impl ReservationShadow {
    pub fn mixer_reserved(&self, target: Entity, resource_type: ResourceType) -> usize {
        self.mixer_destination
            .get(&(target, resource_type))
            .cloned()
            .unwrap_or(0)
    }

    pub fn source_reserved(&self, source: Entity) -> usize {
        self.source.get(&source).cloned().unwrap_or(0)
    }

    pub fn destination_reserved_total(&self, target: Entity) -> usize {
        self.destination_total.get(&target).cloned().unwrap_or(0)
    }

    pub fn destination_reserved_resource(
        &self,
        target: Entity,
        resource_type: ResourceType,
    ) -> usize {
        self.destination_by_resource
            .get(&(target, resource_type))
            .cloned()
            .unwrap_or(0)
    }

    pub fn reserve_destination(
        &mut self,
        target: Entity,
        resource_type: Option<ResourceType>,
        amount: usize,
    ) {
        if amount == 0 {
            return;
        }
        *self.destination_total.entry(target).or_insert(0) += amount;
        if let Some(resource_type) = resource_type {
            *self
                .destination_by_resource
                .entry((target, resource_type))
                .or_insert(0) += amount;
        }
    }

    pub fn apply_reserve_ops(&mut self, ops: &[ResourceReservationOp]) {
        for op in ops {
            match *op {
                ResourceReservationOp::ReserveMixerDestination {
                    target,
                    resource_type,
                } => {
                    *self
                        .mixer_destination
                        .entry((target, resource_type))
                        .or_insert(0) += 1;
                }
                ResourceReservationOp::ReserveSource { source, amount } => {
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
    pub resource_grid: &'a hw_spatial::ResourceSpatialGrid,
    pub tile_site_index: &'a TileSiteIndex,
    pub incoming_snapshot: &'a IncomingDeliverySnapshot,
}

/// ワーカーにタスクを割り当てる
pub fn assign_task_to_worker(
    ctx: AssignTaskContext<'_>,
    queries: &mut FamiliarTaskAssignmentQueries,
    construction_sites: &impl ConstructionSitePositions,
    q_souls: &mut FamiliarSoulQuery,
    shadow: &mut ReservationShadow,
) -> bool {
    let Ok((_, _, soul, _assigned_task, _dest, _path, idle, _, uc_opt, _participating_opt)) =
        q_souls.get_mut(ctx.worker_entity)
    else {
        warn!("ASSIGN: Worker {:?} not found in query", ctx.worker_entity);
        return false;
    };

    if idle.behavior == hw_core::soul::IdleBehavior::ExhaustedGathering {
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

    // GeneratePower: Dream が閾値未満の Soul にはアサインしない（終了→即再アサインのループ防止）
    if work_type == WorkType::GeneratePower && soul.dream < DREAM_GENERATE_ASSIGN_THRESHOLD {
        debug!(
            "ASSIGN: Worker {:?} dream too low for GeneratePower ({:.1} < {:.1})",
            ctx.worker_entity, soul.dream, DREAM_GENERATE_ASSIGN_THRESHOLD
        );
        return false;
    }

    super::policy::assign_by_work_type(
        work_type,
        task_pos,
        uc_opt.is_some(),
        &ctx,
        queries,
        construction_sites,
        shadow,
    )
}
