//! Task area auto-haul system
//!
//! ファミリア単位でStockpileグループを構築し、
//! グループ単位で TransportRequest を発行する。

use bevy::prelude::*;
use std::collections::{HashMap, HashSet};
use std::time::Instant;

use crate::entities::familiar::{ActiveCommand, FamiliarCommand};
use crate::relationships::{StoredItems, TaskWorkers};
use crate::systems::command::TaskArea;
use crate::systems::jobs::{Designation, Priority, TaskSlots, WorkType};
use crate::systems::logistics::transport_request::{
    ManualHaulPinnedSource, ManualTransportRequest, TransportDemand, TransportPolicy,
    TransportPriority, TransportRequest, TransportRequestKind, TransportRequestMetrics,
    TransportRequestState,
};
use crate::systems::logistics::{BelongsTo, BucketStorage, ResourceType, Stockpile};

use crate::systems::spatial::StockpileSpatialGrid;

use super::stockpile_group::{
    build_group_spatial_index, build_stockpile_groups, find_nearest_group_for_item_indexed,
    StockpileGroup, StockpileGroupSpatialIndex,
};

struct GroupEvalContext {
    representative: Entity,
    owner: Option<Entity>,
    rep_pos: Vec2,
    total_capacity: usize,
    total_stored: usize,
    has_untyped_capacity: bool,
    typed_accept: HashSet<ResourceType>,
}

impl GroupEvalContext {
    fn can_accept(&self, resource_type: ResourceType) -> bool {
        self.has_untyped_capacity || self.typed_accept.contains(&resource_type)
    }
}

fn build_group_eval_contexts(
    groups: &[StockpileGroup],
    q_stockpiles_detail: &Query<(
        Entity,
        &Transform,
        &Stockpile,
        Option<&StoredItems>,
        Option<&BelongsTo>,
        Option<&BucketStorage>,
    )>,
) -> Vec<GroupEvalContext> {
    let mut contexts = Vec::with_capacity(groups.len());

    for group in groups {
        let owner = group.cells.first().and_then(|&cell| {
            q_stockpiles_detail
                .get(cell)
                .ok()
                .and_then(|(_, _, _, _, belongs, _)| belongs.map(|b| b.0))
        });

        let rep_pos = q_stockpiles_detail
            .get(group.representative)
            .map(|(_, t, _, _, _, _)| t.translation.truncate())
            .unwrap_or(Vec2::ZERO);

        let mut has_untyped_capacity = false;
        let mut typed_accept = HashSet::new();

        for &cell in &group.cells {
            let Ok((_, _, stockpile, stored_opt, _, bucket_opt)) = q_stockpiles_detail.get(cell)
            else {
                continue;
            };

            if bucket_opt.is_some() {
                continue;
            }

            let current = stored_opt.map(|s| s.len()).unwrap_or(0);
            if current >= stockpile.capacity {
                continue;
            }

            if let Some(resource_type) = stockpile.resource_type {
                typed_accept.insert(resource_type);
            } else {
                has_untyped_capacity = true;
            }
        }

        contexts.push(GroupEvalContext {
            representative: group.representative,
            owner,
            rep_pos,
            total_capacity: group.total_capacity,
            total_stored: group.total_stored,
            has_untyped_capacity,
            typed_accept,
        });
    }

    contexts
}

fn pick_representative_resource_type_per_group(
    groups: &[StockpileGroup],
    spatial_index: &StockpileGroupSpatialIndex,
    contexts: &[GroupEvalContext],
    q_free_items: &Query<
        (
            &Transform,
            &crate::systems::logistics::ResourceItem,
            &Visibility,
            Option<&BelongsTo>,
        ),
        (
            Without<Designation>,
            Without<TaskWorkers>,
            Without<crate::systems::logistics::ReservedForTask>,
            Without<ManualHaulPinnedSource>,
            Without<crate::systems::logistics::InStockpile>,
        ),
    >,
) -> (Vec<Option<ResourceType>>, u32, u32) {
    let mut best_types: Vec<Option<(ResourceType, f32)>> = vec![None; groups.len()];
    let mut free_items_scanned = 0u32;
    let mut items_matched = 0u32;

    let mut group_lookup = HashMap::<(Entity, Entity), usize>::new();
    for (idx, group) in groups.iter().enumerate() {
        group_lookup.insert((group.representative, group.owner_familiar), idx);
    }

    for (transform, item_type, visibility, item_belongs) in q_free_items.iter() {
        free_items_scanned += 1;

        if *visibility == Visibility::Hidden || !item_type.0.is_loadable() {
            continue;
        }

        let item_pos = transform.translation.truncate();
        let Some(group) = find_nearest_group_for_item_indexed(item_pos, groups, spatial_index)
        else {
            continue;
        };

        let Some(&group_idx) = group_lookup.get(&(group.representative, group.owner_familiar))
        else {
            continue;
        };

        let context = &contexts[group_idx];
        if item_belongs.map(|b| b.0) != context.owner || !context.can_accept(item_type.0) {
            continue;
        }

        items_matched += 1;
        let dist_sq = item_pos.distance_squared(context.rep_pos);

        match &mut best_types[group_idx] {
            Some((_, best_dist_sq)) if dist_sq >= *best_dist_sq => {}
            slot => {
                *slot = Some((item_type.0, dist_sq));
            }
        }
    }

    let representative_types = best_types
        .into_iter()
        .map(|entry| entry.map(|(resource_type, _)| resource_type))
        .collect();

    (representative_types, free_items_scanned, items_matched)
}

/// 指揮エリア内での自動運搬タスク生成システム（グループベース）
pub fn task_area_auto_haul_system(
    mut commands: Commands,
    stockpile_grid: Res<StockpileSpatialGrid>,
    q_familiars: Query<(Entity, &ActiveCommand, &TaskArea)>,
    q_stockpiles: Query<(
        Entity,
        &Transform,
        &Stockpile,
        Option<&StoredItems>,
        Option<&BucketStorage>,
    )>,
    q_stockpiles_detail: Query<(
        Entity,
        &Transform,
        &Stockpile,
        Option<&StoredItems>,
        Option<&BelongsTo>,
        Option<&BucketStorage>,
    )>,
    q_stockpile_requests:
        Query<(Entity, &TransportRequest, Option<&TaskWorkers>), Without<ManualTransportRequest>>,
    q_free_items: Query<
        (
            &Transform,
            &crate::systems::logistics::ResourceItem,
            &Visibility,
            Option<&BelongsTo>,
        ),
        (
            Without<Designation>,
            Without<TaskWorkers>,
            Without<crate::systems::logistics::ReservedForTask>,
            Without<ManualHaulPinnedSource>,
            Without<crate::systems::logistics::InStockpile>,
        ),
    >,
    mut metrics: ResMut<TransportRequestMetrics>,
) {
    let started_at = Instant::now();

    // inflight集計: (anchor, resource_type) -> count
    let mut in_flight = HashMap::<(Entity, ResourceType), usize>::new();

    for (_, req, workers_opt) in q_stockpile_requests.iter() {
        if matches!(req.kind, TransportRequestKind::DepositToStockpile) {
            let count = workers_opt.map(|w| w.len()).unwrap_or(0);
            if count > 0 {
                *in_flight
                    .entry((req.anchor, req.resource_type))
                    .or_insert(0) += count;
            }
        }
    }

    let active_familiars: Vec<(Entity, TaskArea)> = q_familiars
        .iter()
        .filter(|(_, ac, _)| !matches!(ac.command, FamiliarCommand::Idle))
        .map(|(e, _, a)| (e, a.clone()))
        .collect();

    // グループ構築
    let groups = build_stockpile_groups(&stockpile_grid, &active_familiars, &q_stockpiles);
    let group_spatial_index = build_group_spatial_index(&groups, &active_familiars);
    let group_contexts = build_group_eval_contexts(&groups, &q_stockpiles_detail);
    let (group_resource_types, free_items_scanned, items_matched) =
        pick_representative_resource_type_per_group(
            &groups,
            &group_spatial_index,
            &group_contexts,
            &q_free_items,
        );

    // グループごとの desired requests: (representative, resource_type) -> (fam, slots, pos, group_cells)
    let mut desired_requests =
        HashMap::<(Entity, ResourceType), (Entity, u32, Vec2, Vec<Entity>)>::new();

    for (idx, group) in groups.iter().enumerate() {
        let Some(resource_type) = group_resource_types[idx] else {
            continue;
        };

        let context = &group_contexts[idx];

        // グループ全体の需要計算
        let inflight = *in_flight
            .get(&(context.representative, resource_type))
            .unwrap_or(&0);
        let needed = context
            .total_capacity
            .saturating_sub(context.total_stored + inflight);
        if needed == 0 {
            continue;
        }

        desired_requests.insert(
            (context.representative, resource_type),
            (
                group.owner_familiar,
                needed as u32,
                context.rep_pos,
                group.cells.clone(),
            ),
        );
    }

    // 既存リクエストの upsert / cleanup（共通ヘルパー使用）
    let mut seen = std::collections::HashSet::new();
    for (req_entity, req, workers_opt) in q_stockpile_requests.iter() {
        if !matches!(req.kind, TransportRequestKind::DepositToStockpile) {
            continue;
        }
        let key = (req.anchor, req.resource_type);
        let workers = workers_opt.map(|w| w.len()).unwrap_or(0);

        if !super::upsert::process_duplicate_key(
            &mut commands,
            req_entity,
            workers,
            &mut seen,
            key,
        ) {
            continue;
        }

        if let Some((issued_by, slots, pos, group_cells)) = desired_requests.get(&key) {
            commands.entity(req_entity).insert((
                Transform::from_xyz(pos.x, pos.y, 0.0),
                Visibility::Hidden,
                Designation {
                    work_type: WorkType::Haul,
                },
                crate::relationships::ManagedBy(*issued_by),
                TaskSlots::new(*slots),
                Priority(0),
                TransportRequest {
                    kind: TransportRequestKind::DepositToStockpile,
                    anchor: key.0,
                    resource_type: key.1,
                    issued_by: *issued_by,
                    priority: TransportPriority::Normal,
                    stockpile_group: group_cells.clone(),
                },
                TransportDemand {
                    desired_slots: *slots,
                    inflight: 0,
                },
                TransportRequestState::Pending,
                TransportPolicy::default(),
            ));
        } else if workers == 0 {
            super::upsert::disable_request(&mut commands, req_entity);
        }
    }

    for (key, (issued_by, slots, pos, group_cells)) in desired_requests {
        if seen.contains(&key) {
            continue;
        }

        commands.spawn((
            Name::new("TransportRequest::DepositToStockpile"),
            Transform::from_xyz(pos.x, pos.y, 0.0),
            Visibility::Hidden,
            Designation {
                work_type: WorkType::Haul,
            },
            crate::relationships::ManagedBy(issued_by),
            TaskSlots::new(slots),
            Priority(0),
            TransportRequest {
                kind: TransportRequestKind::DepositToStockpile,
                anchor: key.0,
                resource_type: key.1,
                issued_by,
                priority: TransportPriority::Normal,
                stockpile_group: group_cells,
            },
            TransportDemand {
                desired_slots: slots,
                inflight: 0,
            },
            TransportRequestState::Pending,
            TransportPolicy::default(),
        ));
    }

    metrics.task_area_groups = groups.len() as u32;
    metrics.task_area_free_items_scanned = free_items_scanned;
    metrics.task_area_items_matched = items_matched;
    metrics.task_area_elapsed_ms = started_at.elapsed().as_secs_f32() * 1000.0;
}
