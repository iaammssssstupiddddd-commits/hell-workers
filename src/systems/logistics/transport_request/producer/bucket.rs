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

    // (stockpile_entity) -> (fam_entity, stock_pos, dropped_bucket_count)
    let mut desired_returns =
        std::collections::HashMap::<Entity, (Entity, Vec2, u32)>::new();

    let total_buckets = q_buckets.iter().count();
    let total_bucket_stockpiles = q_stockpiles.iter().count();
    if total_buckets > 0 {
        debug!(
            "BUCKET_RETURN: {} dropped buckets found, {} bucket stockpiles, {} active familiars",
            total_buckets, total_bucket_stockpiles, active_familiars.len()
        );
    }

    for (fam_entity, task_area) in active_familiars.iter() {
        for (
            bucket_entity,
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
                debug!("BUCKET_RETURN: bucket {:?} skipped: has workers", bucket_entity);
                continue;
            }
            if reserved_opt.is_some() {
                debug!("BUCKET_RETURN: bucket {:?} skipped: reserved", bucket_entity);
                continue;
            }
            if *visibility == Visibility::Hidden {
                debug!("BUCKET_RETURN: bucket {:?} skipped: hidden", bucket_entity);
                continue;
            }

            let bucket_pos = bucket_transform.translation.truncate();
            if !task_area.contains(bucket_pos) {
                debug!(
                    "BUCKET_RETURN: bucket {:?} at {:?} skipped: outside task area",
                    bucket_entity, bucket_pos
                );
                continue;
            }

            let tank_entity = bucket_belongs.0;
            let search_radius = TILE_SIZE * 20.0;
            let nearby_stockpiles = stockpile_grid.get_nearby_in_radius(bucket_pos, search_radius);

            let target_stockpile = nearby_stockpiles
                .iter()
                .filter_map(|&e| q_stockpiles.get(e).ok())
                .filter(|(_, _, stock, _, stock_belongs, stored_opt)| {
                    let owner_match = stock_belongs.0 == tank_entity;
                    let type_ok = matches!(
                        stock.resource_type,
                        None | Some(ResourceType::BucketEmpty) | Some(ResourceType::BucketWater)
                    );
                    let capacity_ok =
                        stored_opt.map(|s| s.len()).unwrap_or(0) < stock.capacity;
                    if !owner_match || !type_ok || !capacity_ok {
                        debug!(
                            "BUCKET_RETURN: stockpile filtered out: owner={}, type={}, capacity={}",
                            owner_match, type_ok, capacity_ok
                        );
                    }
                    owner_match && type_ok && capacity_ok
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
                    .and_modify(|(_, _, count)| *count += 1)
                    .or_insert_with(|| (*fam_entity, stock_pos, 1));
            } else {
                debug!(
                    "BUCKET_RETURN: bucket {:?} (tank {:?}) has no matching stockpile in {} nearby",
                    bucket_entity, tank_entity, nearby_stockpiles.len()
                );
            }
        }
    }

    if !desired_returns.is_empty() {
        info!(
            "BUCKET_RETURN: creating/updating {} return requests",
            desired_returns.len()
        );
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

        if let Some((issued_by, pos, bucket_count)) = desired_returns.get(&stockpile) {
            commands.entity(req_entity).insert((
                Transform::from_xyz(pos.x, pos.y, 0.0),
                Visibility::Hidden,
                Designation {
                    work_type: WorkType::Haul,
                },
                crate::relationships::ManagedBy(*issued_by),
                TaskSlots::new(*bucket_count),
                Priority(5),
                TransportRequest {
                    kind: TransportRequestKind::ReturnBucket,
                    anchor: stockpile,
                    resource_type: ResourceType::BucketEmpty,
                    issued_by: *issued_by,
                    priority: TransportPriority::Normal,
                },
                TransportDemand {
                    desired_slots: *bucket_count,
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

    for (stockpile, (issued_by, pos, bucket_count)) in desired_returns {
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
            TaskSlots::new(bucket_count),
            Priority(5),
            TransportRequest {
                kind: TransportRequestKind::ReturnBucket,
                anchor: stockpile,
                resource_type: ResourceType::BucketEmpty,
                issued_by,
                priority: TransportPriority::Normal,
            },
            TransportDemand {
                desired_slots: bucket_count,
                inflight: 0,
            },
            TransportRequestState::Pending,
            TransportPolicy::default(),
        ));
    }
}
