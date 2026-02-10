//! Task area auto-haul system
//!
//! M4: ストックパイルへの汎用運搬を request エンティティ（アンカー側）で発行する。
//! 割り当て時にアイテムソースを遅延解決する。
//!
//! 制限: resource_type が確定しているストックパイルのみ request 化。

use bevy::prelude::*;

use crate::entities::familiar::{ActiveCommand, FamiliarCommand};
use crate::relationships::{StoredItems, TaskWorkers};
use crate::systems::command::TaskArea;
use crate::systems::jobs::{Designation, Priority, TaskSlots, WorkType};
use crate::systems::logistics::transport_request::{
    TransportDemand, TransportPolicy, TransportPriority, TransportRequest, TransportRequestKind,
    TransportRequestState,
};
use crate::systems::logistics::{BelongsTo, ResourceItem, ResourceType, Stockpile};
use crate::systems::soul_ai::execute::task_execution::AssignedTask;
use crate::systems::soul_ai::helpers::query_types::AutoHaulAssignedTaskQuery;
use crate::systems::spatial::StockpileSpatialGrid;

/// 指揮エリア内での自動運搬タスク生成システム
pub fn task_area_auto_haul_system(
    mut commands: Commands,
    stockpile_grid: Res<StockpileSpatialGrid>,
    q_familiars: Query<(Entity, &ActiveCommand, &TaskArea)>,
    q_stockpiles: Query<(
        Entity,
        &Transform,
        &Stockpile,
        Option<&StoredItems>,
        Option<&BelongsTo>,
    )>,
    q_souls: AutoHaulAssignedTaskQuery,
    q_all_resources: Query<&ResourceItem>,
    q_stockpile_requests: Query<(Entity, &TransportRequest, Option<&TaskWorkers>)>,
) {
    let mut in_flight = std::collections::HashMap::<(Entity, ResourceType), usize>::new();

    for task in q_souls.iter() {
        if let AssignedTask::Haul(data) = task {
            let stockpile = data.stockpile;
            if let Ok(res_item) = q_all_resources.get(data.item) {
                *in_flight.entry((stockpile, res_item.0)).or_insert(0) += 1;
            }
        }
    }

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

    let mut desired_requests =
        std::collections::HashMap::<(Entity, ResourceType), (Entity, u32, Vec2)>::new();

    let mut stockpiles_to_process = std::collections::HashSet::new();
    for (_, area) in &active_familiars {
        for &stock_entity in stockpile_grid.get_in_area(area.min, area.max).iter() {
            stockpiles_to_process.insert(stock_entity);
        }
    }

    for stock_entity in stockpiles_to_process {
        let Ok((_, stock_transform, stockpile, stored_opt, _stock_belongs)) =
            q_stockpiles.get(stock_entity)
        else {
            continue;
        };

        let Some(resource_type) = stockpile.resource_type else {
            continue;
        };

        if matches!(
            resource_type,
            ResourceType::BucketEmpty | ResourceType::BucketWater
        ) {
            continue;
        }

        let stock_pos = stock_transform.translation.truncate();
        let current = stored_opt.map(|s| s.len()).unwrap_or(0);
        if current >= stockpile.capacity {
            continue;
        }

        let Some((fam_entity, _)) = find_owner_familiar(stock_pos, &active_familiars) else {
            continue;
        };

        let inflight = *in_flight.get(&(stock_entity, resource_type)).unwrap_or(&0);
        let needed = (stockpile.capacity - current).saturating_sub(inflight);
        if needed == 0 {
            continue;
        }

        desired_requests.insert(
            (stock_entity, resource_type),
            (fam_entity, needed as u32, stock_pos),
        );
    }

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

        if let Some((issued_by, slots, pos)) = desired_requests.get(&key) {
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

    for (key, (issued_by, slots, pos)) in desired_requests {
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

fn find_owner_familiar(
    pos: Vec2,
    familiars: &[(Entity, TaskArea)],
) -> Option<(Entity, &TaskArea)> {
    familiars
        .iter()
        .filter(|(_, area)| area.contains(pos))
        .min_by(|(_, a1), (_, a2)| {
            let d1 = a1.center().distance_squared(pos);
            let d2 = a2.center().distance_squared(pos);
            d1.partial_cmp(&d2).unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(e, a)| (*e, a))
}
