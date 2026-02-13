//! Task area auto-haul system
//!
//! ファミリア単位でStockpileグループを構築し、
//! グループ単位で TransportRequest を発行する。

use bevy::prelude::*;

use crate::entities::familiar::{ActiveCommand, FamiliarCommand};
use crate::relationships::{StoredItems, TaskWorkers};
use crate::systems::command::TaskArea;
use crate::systems::jobs::{Designation, Priority, TaskSlots, WorkType};
use crate::systems::logistics::transport_request::{
    TransportDemand, TransportPolicy, TransportPriority, TransportRequest, TransportRequestKind,
    TransportRequestState,
};
use crate::systems::logistics::{BelongsTo, BucketStorage, ResourceType, Stockpile};

use crate::systems::spatial::StockpileSpatialGrid;

use super::stockpile_group::{build_stockpile_groups, find_nearest_group_for_item};

/// グループの代表リソースタイプを決定する
///
/// グループ内セルに固定リソースタイプがあればそれを返す。
/// なければ収集範囲内の最寄りフリーアイテムから推定する。
fn resolve_group_resource_type(
    group: &super::stockpile_group::StockpileGroup,
    q_stockpiles_detail: &Query<
        (
            Entity,
            &Transform,
            &Stockpile,
            Option<&StoredItems>,
            Option<&BelongsTo>,
            Option<&BucketStorage>,
        ),
    >,
    q_free_items: &Query<
        (&Transform, &crate::systems::logistics::ResourceItem, &Visibility, Option<&BelongsTo>),
        (
            Without<Designation>,
            Without<TaskWorkers>,
            Without<crate::systems::logistics::ReservedForTask>,
            Without<crate::systems::logistics::InStockpile>,
        ),
    >,
    familiars_with_areas: &[(Entity, TaskArea)],
) -> Option<ResourceType> {
    // 収集範囲内のフリーアイテムから推定（実際に受け入れ可能な型のみ）
    let owner = group.cells.first().and_then(|&cell| {
        q_stockpiles_detail
            .get(cell)
            .ok()
            .and_then(|(_, _, _, _, belongs, _)| belongs.map(|b| b.0))
    });

    let can_accept_in_group = |resource_type: ResourceType| -> bool {
        group.cells.iter().any(|&cell| {
            q_stockpiles_detail.get(cell).ok().is_some_and(
                |(_, _, stockpile, stored_opt, _, bucket_opt)| {
                    if bucket_opt.is_some() {
                        return false;
                    }
                    let current = stored_opt.map(|s| s.len()).unwrap_or(0);
                    let has_capacity = current < stockpile.capacity;
                    let type_ok = stockpile.resource_type.is_none()
                        || stockpile.resource_type == Some(resource_type);
                    has_capacity && type_ok
                },
            )
        })
    };

    q_free_items
        .iter()
        .filter(|(_, item_type, visibility, item_belongs)| {
            *visibility != Visibility::Hidden
                && item_type.0.is_loadable()
                && owner == item_belongs.map(|b| b.0)
                && can_accept_in_group(item_type.0)
        })
        .filter(|(transform, _, _, _)| {
            // 収集範囲内かチェック
            let item_pos = transform.translation.truncate();
            let groups_slice = std::slice::from_ref(group);
            find_nearest_group_for_item(item_pos, groups_slice, familiars_with_areas).is_some()
        })
        .min_by(|(t1, _, _, _), (t2, _, _, _)| {
            // 代表セルからの距離で最寄りを選択
            if let Ok((_, rep_t, _, _, _, _)) = q_stockpiles_detail.get(group.representative) {
                let rep_pos = rep_t.translation.truncate();
                let d1 = t1.translation.truncate().distance_squared(rep_pos);
                let d2 = t2.translation.truncate().distance_squared(rep_pos);
                d1.partial_cmp(&d2).unwrap_or(std::cmp::Ordering::Equal)
            } else {
                std::cmp::Ordering::Equal
            }
        })
        .map(|(_, item_type, _, _)| item_type.0)
}

/// 指揮エリア内での自動運搬タスク生成システム（グループベース）
pub fn task_area_auto_haul_system(
    mut commands: Commands,
    stockpile_grid: Res<StockpileSpatialGrid>,
    q_familiars: Query<(Entity, &ActiveCommand, &TaskArea)>,
    q_stockpiles: Query<
        (
            Entity,
            &Transform,
            &Stockpile,
            Option<&StoredItems>,
            Option<&BucketStorage>,
        ),
    >,
    q_stockpiles_detail: Query<
        (
            Entity,
            &Transform,
            &Stockpile,
            Option<&StoredItems>,
            Option<&BelongsTo>,
            Option<&BucketStorage>,
        ),
    >,
    q_stockpile_requests: Query<(Entity, &TransportRequest, Option<&TaskWorkers>)>,
    q_free_items: Query<
        (&Transform, &crate::systems::logistics::ResourceItem, &Visibility, Option<&BelongsTo>),
        (
            Without<Designation>,
            Without<TaskWorkers>,
            Without<crate::systems::logistics::ReservedForTask>,
            Without<crate::systems::logistics::InStockpile>,
        ),
    >,
) {
    // inflight集計: (anchor, resource_type) -> count
    let mut in_flight = std::collections::HashMap::<(Entity, ResourceType), usize>::new();

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

    // グループごとの desired requests: (representative, resource_type) -> (fam, slots, pos, group_cells)
    let mut desired_requests = std::collections::HashMap::<
        (Entity, ResourceType),
        (Entity, u32, Vec2, Vec<Entity>),
    >::new();

    for group in &groups {
        let Some(resource_type) = resolve_group_resource_type(
            group,
            &q_stockpiles_detail,
            &q_free_items,
            &active_familiars,
        ) else {
            continue;
        };

        if !resource_type.is_loadable() {
            continue;
        }

        // グループ全体の需要計算
        let inflight = *in_flight
            .get(&(group.representative, resource_type))
            .unwrap_or(&0);
        let needed = group.total_capacity.saturating_sub(group.total_stored + inflight);
        if needed == 0 {
            continue;
        }

        // 代表セルのポジション
        let rep_pos = q_stockpiles
            .get(group.representative)
            .map(|(_, t, _, _, _)| t.translation.truncate())
            .unwrap_or(Vec2::ZERO);

        desired_requests.insert(
            (group.representative, resource_type),
            (group.owner_familiar, needed as u32, rep_pos, group.cells.clone()),
        );
    }

    // 既存リクエストの upsert / cleanup
    let mut seen = std::collections::HashSet::new();
    for (req_entity, req, workers_opt) in q_stockpile_requests.iter() {
        if !matches!(req.kind, TransportRequestKind::DepositToStockpile) {
            continue;
        }
        let key = (req.anchor, req.resource_type);
        let workers = workers_opt.map(|w| w.len()).unwrap_or(0);

        if !seen.insert(key) {
            if workers == 0 {
                commands.entity(req_entity).despawn();
            }
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
            commands
                .entity(req_entity)
                .remove::<Designation>()
                .remove::<TaskSlots>()
                .remove::<Priority>();
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
}
