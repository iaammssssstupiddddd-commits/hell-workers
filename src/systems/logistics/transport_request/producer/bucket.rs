//! Bucket auto-haul system
//!
//! ドロップされたバケツの返却 request を、タンク anchor で管理する。
//! 返却件数は `TransportDemand.desired_slots` で表現し、request はタンクごと最大1件に保つ。

use bevy::prelude::*;

use crate::entities::familiar::{ActiveCommand, Familiar, FamiliarCommand};
use crate::relationships::{StoredItems, TaskWorkers};
use crate::systems::command::TaskArea;
use crate::systems::familiar_ai::perceive::resource_sync::SharedResourceCache;
use crate::systems::jobs::{Designation, Priority, TaskSlots, WorkType};
use crate::systems::logistics::transport_request::{
    TransportDemand, TransportPolicy, TransportPriority, TransportRequest, TransportRequestKind,
    TransportRequestState,
};
use crate::systems::logistics::{
    BelongsTo, BucketStorage, ReservedForTask, ResourceItem, ResourceType, Stockpile,
};

#[derive(Clone, Copy)]
struct DesiredBucketReturn {
    issued_by: Entity,
    tank_pos: Vec2,
    desired_slots: u32,
}

#[derive(Default)]
struct TankDemand {
    dropped_buckets: u32,
    free_slots_total: u32,
}

fn is_bucket_resource(resource_type: ResourceType) -> bool {
    matches!(
        resource_type,
        ResourceType::BucketEmpty | ResourceType::BucketWater
    )
}

fn bucket_storage_accepts_buckets(stockpile: &Stockpile) -> bool {
    matches!(
        stockpile.resource_type,
        None | Some(ResourceType::BucketEmpty) | Some(ResourceType::BucketWater)
    )
}

fn to_u32_saturating(value: usize) -> u32 {
    value.min(u32::MAX as usize) as u32
}

pub fn bucket_auto_haul_system(
    mut commands: Commands,
    haul_cache: Res<SharedResourceCache>,
    q_familiars: Query<(Entity, &ActiveCommand, &TaskArea), With<Familiar>>,
    q_tanks: Query<(&Transform, &Stockpile), Without<BucketStorage>>,
    q_dropped_buckets: Query<
        (
            &Visibility,
            &ResourceItem,
            &BelongsTo,
            Option<&ReservedForTask>,
            Option<&TaskWorkers>,
        ),
        (
            Without<crate::relationships::StoredIn>,
            Without<Designation>,
        ),
    >,
    q_bucket_storages: Query<
        (Entity, &Stockpile, &BelongsTo, Option<&StoredItems>),
        With<BucketStorage>,
    >,
    q_bucket_requests: Query<(Entity, &TransportRequest, Option<&TaskWorkers>)>,
) {
    let active_familiars: Vec<(Entity, TaskArea)> = q_familiars
        .iter()
        .filter(|(_, active_command, _)| !matches!(active_command.command, FamiliarCommand::Idle))
        .map(|(entity, _, area)| (entity, area.clone()))
        .collect();

    let mut tank_demands = std::collections::HashMap::<Entity, TankDemand>::new();

    for (storage_entity, stockpile, storage_belongs, stored_opt) in q_bucket_storages.iter() {
        if !bucket_storage_accepts_buckets(stockpile) {
            continue;
        }

        let current = stored_opt.map(|stored| stored.len()).unwrap_or(0);
        let reserved = haul_cache.get_destination_reservation(storage_entity);
        let anticipated = current + reserved;
        let free_slots = stockpile.capacity.saturating_sub(anticipated);

        let tank = storage_belongs.0;
        let demand = tank_demands.entry(tank).or_default();
        demand.free_slots_total = demand
            .free_slots_total
            .saturating_add(to_u32_saturating(free_slots));
    }

    for (visibility, resource_item, belongs_to, reserved_opt, workers_opt) in
        q_dropped_buckets.iter()
    {
        if *visibility == Visibility::Hidden {
            continue;
        }
        if !is_bucket_resource(resource_item.0) {
            continue;
        }
        if reserved_opt.is_some() {
            continue;
        }
        if workers_opt.is_some_and(|workers| !workers.is_empty()) {
            continue;
        }

        let demand = tank_demands.entry(belongs_to.0).or_default();
        demand.dropped_buckets = demand.dropped_buckets.saturating_add(1);
    }

    let mut desired_requests = std::collections::HashMap::<Entity, DesiredBucketReturn>::new();
    for (tank_entity, demand) in tank_demands.iter() {
        let Ok((tank_transform, tank_stockpile)) = q_tanks.get(*tank_entity) else {
            continue;
        };
        if tank_stockpile.resource_type != Some(ResourceType::Water) {
            continue;
        }

        let tank_pos = tank_transform.translation.truncate();
        let Some((issued_by, _)) = super::find_owner_familiar(tank_pos, &active_familiars) else {
            continue;
        };

        let desired_slots = demand.dropped_buckets.min(demand.free_slots_total);
        if desired_slots == 0 {
            continue;
        }

        desired_requests.insert(
            *tank_entity,
            DesiredBucketReturn {
                issued_by,
                tank_pos,
                desired_slots,
            },
        );
    }

    let mut seen_existing = std::collections::HashSet::<Entity>::new();
    for (request_entity, request, workers_opt) in q_bucket_requests.iter() {
        if request.kind != TransportRequestKind::ReturnBucket {
            continue;
        }

        let tank_entity = request.anchor;
        let workers = workers_opt.map(|workers| workers.len()).unwrap_or(0);
        let inflight = to_u32_saturating(workers);
        let valid_tank = q_tanks
            .get(tank_entity)
            .is_ok_and(|(_, stockpile)| stockpile.resource_type == Some(ResourceType::Water));

        if !valid_tank {
            if workers == 0 {
                commands.entity(request_entity).despawn();
            } else {
                super::upsert::disable_request(&mut commands, request_entity);
                commands.entity(request_entity).insert(TransportDemand {
                    desired_slots: 0,
                    inflight,
                });
            }
            continue;
        }

        if !super::upsert::process_duplicate_key(
            &mut commands,
            request_entity,
            workers,
            &mut seen_existing,
            tank_entity,
        ) {
            if workers > 0 {
                super::upsert::disable_request(&mut commands, request_entity);
                commands.entity(request_entity).insert(TransportDemand {
                    desired_slots: 0,
                    inflight,
                });
            }
            continue;
        }

        if let Some(desired) = desired_requests.get(&tank_entity) {
            commands.entity(request_entity).insert((
                Transform::from_xyz(desired.tank_pos.x, desired.tank_pos.y, 0.0),
                Visibility::Hidden,
                Designation {
                    work_type: WorkType::Haul,
                },
                crate::relationships::ManagedBy(desired.issued_by),
                TaskSlots::new(desired.desired_slots),
                Priority(5),
                TransportRequest {
                    kind: TransportRequestKind::ReturnBucket,
                    anchor: tank_entity,
                    resource_type: ResourceType::BucketEmpty,
                    issued_by: desired.issued_by,
                    priority: TransportPriority::Normal,
                    stockpile_group: vec![],
                },
                TransportDemand {
                    desired_slots: desired.desired_slots,
                    inflight,
                },
                TransportRequestState::Pending,
                TransportPolicy::default(),
            ));
        } else if workers == 0 {
            super::upsert::disable_request(&mut commands, request_entity);
            commands
                .entity(request_entity)
                .insert(TransportDemand {
                    desired_slots: 0,
                    inflight: 0,
                });
        } else {
            super::upsert::disable_request(&mut commands, request_entity);
            commands
                .entity(request_entity)
                .insert(TransportDemand {
                    desired_slots: 0,
                    inflight,
                });
        }
    }

    for (tank_entity, desired) in desired_requests {
        if seen_existing.contains(&tank_entity) {
            continue;
        }

        commands.spawn((
            Name::new("TransportRequest::ReturnBucket"),
            Transform::from_xyz(desired.tank_pos.x, desired.tank_pos.y, 0.0),
            Visibility::Hidden,
            Designation {
                work_type: WorkType::Haul,
            },
            crate::relationships::ManagedBy(desired.issued_by),
            TaskSlots::new(desired.desired_slots),
            Priority(5),
            TransportRequest {
                kind: TransportRequestKind::ReturnBucket,
                anchor: tank_entity,
                resource_type: ResourceType::BucketEmpty,
                issued_by: desired.issued_by,
                priority: TransportPriority::Normal,
                stockpile_group: vec![],
            },
            TransportDemand {
                desired_slots: desired.desired_slots,
                inflight: 0_u32,
            },
            TransportRequestState::Pending,
            TransportPolicy::default(),
        ));
    }
}
