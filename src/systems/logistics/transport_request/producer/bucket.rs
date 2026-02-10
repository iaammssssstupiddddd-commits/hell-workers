//! Bucket auto-haul system
//!
//! M7: ドロップされたバケツの返却を request エンティティ化。
//! ストックパイル（バケツ置き場）位置に request を生成し、割り当て時にバケツを遅延解決する。

use bevy::prelude::*;

use crate::constants::TILE_SIZE;
use crate::entities::familiar::{ActiveCommand, Familiar, FamiliarCommand};
use crate::relationships::TaskWorkers;
use crate::systems::command::TaskArea;
use crate::systems::jobs::{Designation, Priority, TaskSlots, WorkType};
use crate::systems::logistics::transport_request::{
    TransportDemand, TransportPolicy, TransportPriority, TransportRequest, TransportRequestKind,
    TransportRequestState,
};
use crate::systems::logistics::{
    BucketStorage, ReservedForTask, ResourceItem, ResourceType, Stockpile,
};
use crate::systems::spatial::{SpatialGridOps, StockpileSpatialGrid};

/// バケツ専用オートホールシステム（M7: request エンティティ化）
pub fn bucket_auto_haul_system(
    mut commands: Commands,
    stockpile_grid: Res<StockpileSpatialGrid>,
    q_familiars: Query<(Entity, &ActiveCommand, &TaskArea), With<Familiar>>,
    q_buckets: Query<
        (
            Entity,
            &Transform,
            &Visibility,
            &ResourceItem,
            &crate::systems::logistics::BelongsTo,
            Option<&ReservedForTask>,
            Option<&TaskWorkers>,
        ),
        (
            Without<crate::relationships::StoredIn>,
            Without<Designation>,
        ),
    >,
    q_stockpiles: Query<(
        Entity,
        &Transform,
        &Stockpile,
        &BucketStorage,
        &crate::systems::logistics::BelongsTo,
        Option<&crate::relationships::StoredItems>,
    )>,
    q_bucket_requests: Query<(Entity, &TransportRequest, Option<&TaskWorkers>)>,
) {
    let active_familiars: Vec<(Entity, TaskArea)> = q_familiars
        .iter()
        .filter(|(_, ac, _)| !matches!(ac.command, FamiliarCommand::Idle))
        .map(|(e, _, area)| (e, area.clone()))
        .collect();

    // (stockpile_entity) -> (fam_entity, stock_pos)
    let mut desired_returns = std::collections::HashMap::<Entity, (Entity, Vec2)>::new();

    for (fam_entity, task_area) in active_familiars.iter() {
        for (
            _bucket_entity,
            bucket_transform,
            visibility,
            res_item,
            bucket_belongs,
            reserved_opt,
            workers_opt,
        ) in q_buckets.iter()
        {
            if !matches!(
                res_item.0,
                ResourceType::BucketEmpty | ResourceType::BucketWater
            ) {
                continue;
            }
            if workers_opt.is_some_and(|w| !w.is_empty()) {
                continue;
            }
            if reserved_opt.is_some() {
                continue;
            }
            if *visibility == Visibility::Hidden {
                continue;
            }

            let bucket_pos = bucket_transform.translation.truncate();
            if !task_area.contains(bucket_pos) {
                continue;
            }

            let tank_entity = bucket_belongs.0;
            let search_radius = TILE_SIZE * 20.0;
            let nearby_stockpiles = stockpile_grid.get_nearby_in_radius(bucket_pos, search_radius);

            let target_stockpile = nearby_stockpiles
                .iter()
                .filter_map(|&e| q_stockpiles.get(e).ok())
                .filter(|(_, _, stock, _, stock_belongs, stored_opt)| {
                    stock_belongs.0 == tank_entity
                        && matches!(
                            stock.resource_type,
                            None | Some(ResourceType::BucketEmpty) | Some(ResourceType::BucketWater)
                        )
                        && stored_opt.map(|s| s.len()).unwrap_or(0) < stock.capacity
                })
                .min_by(|(_, t1, _, _, _, _), (_, t2, _, _, _, _)| {
                    let d1 = t1.translation.truncate().distance_squared(bucket_pos);
                    let d2 = t2.translation.truncate().distance_squared(bucket_pos);
                    d1.partial_cmp(&d2).unwrap_or(std::cmp::Ordering::Equal)
                })
                .map(|(e, t, _, _, _, _)| (e, t.translation.truncate()));

            if let Some((stockpile_entity, stock_pos)) = target_stockpile {
                desired_returns
                    .entry(stockpile_entity)
                    .or_insert_with(|| (*fam_entity, stock_pos));
            }
        }
    }

    let mut seen = std::collections::HashSet::new();
    for (req_entity, req, workers_opt) in q_bucket_requests.iter() {
        if req.kind != TransportRequestKind::ReturnBucket {
            continue;
        }
        let stockpile = req.anchor;
        let workers = workers_opt.map(|w| w.len()).unwrap_or(0);

        if !seen.insert(stockpile) {
            if workers == 0 {
                commands.entity(req_entity).despawn();
            }
            continue;
        }

        if let Some((issued_by, pos)) = desired_returns.get(&stockpile) {
            commands.entity(req_entity).insert((
                Transform::from_xyz(pos.x, pos.y, 0.0),
                Visibility::Hidden,
                Designation {
                    work_type: WorkType::Haul,
                },
                crate::relationships::ManagedBy(*issued_by),
                TaskSlots::new(1),
                Priority(5),
                TransportRequest {
                    kind: TransportRequestKind::ReturnBucket,
                    anchor: stockpile,
                    resource_type: ResourceType::BucketEmpty,
                    issued_by: *issued_by,
                    priority: TransportPriority::Normal,
                },
                TransportDemand {
                    desired_slots: 1,
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

    for (stockpile, (issued_by, pos)) in desired_returns {
        if seen.contains(&stockpile) {
            continue;
        }

        commands.spawn((
            Name::new("TransportRequest::ReturnBucket"),
            Transform::from_xyz(pos.x, pos.y, 0.0),
            Visibility::Hidden,
            Designation {
                work_type: WorkType::Haul,
            },
            crate::relationships::ManagedBy(issued_by),
            TaskSlots::new(1),
            Priority(5),
            TransportRequest {
                kind: TransportRequestKind::ReturnBucket,
                anchor: stockpile,
                resource_type: ResourceType::BucketEmpty,
                issued_by,
                priority: TransportPriority::Normal,
            },
            TransportDemand {
                desired_slots: 1,
                inflight: 0,
            },
            TransportRequestState::Pending,
            TransportPolicy::default(),
        ));
    }
}
