//! Task area auto-haul system
//!
//! M4: ストックパイルへの汎用運搬を request エンティティ（アンカー側）で発行する。
//! 割り当て時にアイテムソースを遅延解決する。

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

fn resolve_request_resource_type(
    stock_pos: Vec2,
    stockpile: &Stockpile,
    stock_belongs: Option<&BelongsTo>,
    q_free_items: &Query<
        (&Transform, &crate::systems::logistics::ResourceItem, &Visibility, Option<&BelongsTo>),
        (
            Without<Designation>,
            Without<TaskWorkers>,
            Without<crate::systems::logistics::ReservedForTask>,
            Without<crate::systems::logistics::InStockpile>,
        ),
    >,
) -> Option<ResourceType> {
    if let Some(resource_type) = stockpile.resource_type {
        return Some(resource_type);
    }

    let owner = stock_belongs.map(|b| b.0);
    q_free_items
        .iter()
        .filter(|(_, item_type, visibility, item_belongs)| {
            *visibility != Visibility::Hidden
                && item_type.0.is_loadable()
                && owner == item_belongs.map(|b| b.0)
        })
        .min_by(|(t1, _, _, _), (t2, _, _, _)| {
            let d1 = t1.translation.truncate().distance_squared(stock_pos);
            let d2 = t2.translation.truncate().distance_squared(stock_pos);
            d1.partial_cmp(&d2).unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(_, item_type, _, _)| item_type.0)
}

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
        Option<&BucketStorage>,
    )>,
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

    let mut desired_requests =
        std::collections::HashMap::<(Entity, ResourceType), (Entity, u32, Vec2)>::new();

    let mut stockpiles_to_process = std::collections::HashSet::new();
    for (_, area) in &active_familiars {
        for &stock_entity in stockpile_grid.get_in_area(area.min, area.max).iter() {
            stockpiles_to_process.insert(stock_entity);
        }
    }

    for stock_entity in stockpiles_to_process {
        let Ok((_, stock_transform, stockpile, stored_opt, stock_belongs, bucket_storage)) =
            q_stockpiles.get(stock_entity)
        else {
            continue;
        };

        // バケツ置き場は bucket_auto_haul_system が管理するためスキップ
        if bucket_storage.is_some() {
            continue;
        }

        let stock_pos = stock_transform.translation.truncate();
        let Some(resource_type) = resolve_request_resource_type(
            stock_pos,
            stockpile,
            stock_belongs,
            &q_free_items,
        ) else {
            continue;
        };

        if !resource_type.is_loadable() {
            continue;
        }

        let current = stored_opt.map(|s| s.len()).unwrap_or(0);
        if current >= stockpile.capacity {
            continue;
        }

        let Some((fam_entity, _)) = super::find_owner_familiar(stock_pos, &active_familiars) else {
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
