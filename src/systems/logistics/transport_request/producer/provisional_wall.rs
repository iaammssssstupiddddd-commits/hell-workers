//! Provisional wall upgrade transport producer
//!
//! Generates StasisMud transport requests for provisional walls and
//! issues `CoatWall` designations after mud delivery.

use bevy::prelude::*;

use crate::constants::TILE_SIZE;
use crate::entities::familiar::{ActiveCommand, FamiliarCommand};
use crate::relationships::TaskWorkers;
use crate::systems::command::TaskArea;
use crate::systems::jobs::{
    Building, BuildingType, Designation, Priority, ProvisionalWall, TaskSlots, WorkType,
};
use crate::systems::jobs::wall_construction::WallTileBlueprint;
use crate::systems::logistics::ResourceType;
use crate::systems::logistics::transport_request::{
    TransportDemand, TransportPolicy, TransportPriority, TransportRequest, TransportRequestKind,
    TransportRequestState,
};

const PROVISIONAL_WALL_PRIORITY: u32 = 5;

fn to_u32_saturating(value: usize) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}

pub fn provisional_wall_auto_haul_system(
    mut commands: Commands,
    q_familiars: Query<(Entity, &ActiveCommand, &TaskArea)>,
    q_walls: Query<(
        Entity,
        &Transform,
        &Building,
        &ProvisionalWall,
        Option<&TaskWorkers>,
    )>,
    q_requests: Query<(Entity, &TransportRequest, Option<&TaskWorkers>)>,
    q_wall_tiles: Query<&WallTileBlueprint>,
) {
    let site_managed_walls: std::collections::HashSet<Entity> =
        q_wall_tiles.iter().filter_map(|tile| tile.spawned_wall).collect();

    let mut in_flight = std::collections::HashMap::<Entity, usize>::new();
    for (_, req, workers_opt) in q_requests.iter() {
        if req.kind != TransportRequestKind::DeliverToProvisionalWall {
            continue;
        }
        let workers = workers_opt.map(|w| w.len()).unwrap_or(0);
        if workers > 0 {
            *in_flight.entry(req.anchor).or_insert(0) += workers;
        }
    }

    let active_familiars: Vec<(Entity, TaskArea)> = q_familiars
        .iter()
        .filter(|(_, active_command, _)| !matches!(active_command.command, FamiliarCommand::Idle))
        .map(|(entity, _, area)| (entity, area.clone()))
        .collect();

    let mut desired_requests = std::collections::HashMap::<Entity, (Entity, Vec2, u32)>::new();
    for (wall_entity, wall_transform, building, provisional, workers_opt) in q_walls.iter() {
        if site_managed_walls.contains(&wall_entity) {
            continue;
        }
        if building.kind != BuildingType::Wall
            || !building.is_provisional
            || provisional.mud_delivered
        {
            continue;
        }

        if workers_opt.map(|w| w.len()).unwrap_or(0) > 0 {
            continue;
        }

        let wall_pos = wall_transform.translation.truncate();
        let Some((fam_entity, _)) = super::find_owner_familiar(wall_pos, &active_familiars) else {
            continue;
        };

        let inflight = *in_flight.get(&wall_entity).unwrap_or(&0);
        if inflight >= 1 {
            continue;
        }

        desired_requests.insert(wall_entity, (fam_entity, wall_pos, 1));
    }

    let mut seen_existing = std::collections::HashSet::<Entity>::new();
    for (request_entity, request, workers_opt) in q_requests.iter() {
        if request.kind != TransportRequestKind::DeliverToProvisionalWall {
            continue;
        }

        let key = request.anchor;
        let workers = workers_opt.map(|w| w.len()).unwrap_or(0);
        if !super::upsert::process_duplicate_key(
            &mut commands,
            request_entity,
            workers,
            &mut seen_existing,
            key,
        ) {
            continue;
        }

        let inflight = to_u32_saturating(workers);
        if let Some((issued_by, wall_pos, slots)) = desired_requests.get(&key) {
            commands.entity(request_entity).insert((
                Transform::from_xyz(wall_pos.x, wall_pos.y, 0.0),
                Visibility::Hidden,
                Designation {
                    work_type: WorkType::Haul,
                },
                crate::relationships::ManagedBy(*issued_by),
                TaskSlots::new(*slots),
                Priority(PROVISIONAL_WALL_PRIORITY),
                TransportRequest {
                    kind: TransportRequestKind::DeliverToProvisionalWall,
                    anchor: key,
                    resource_type: ResourceType::StasisMud,
                    issued_by: *issued_by,
                    priority: TransportPriority::Low,
                    stockpile_group: vec![],
                },
                TransportDemand {
                    desired_slots: *slots,
                    inflight,
                },
                TransportRequestState::Pending,
                TransportPolicy::default(),
            ));
            continue;
        }

        super::upsert::disable_request(&mut commands, request_entity);
        commands.entity(request_entity).insert(TransportDemand {
            desired_slots: 0,
            inflight,
        });
    }

    for (wall_entity, (issued_by, wall_pos, slots)) in desired_requests {
        if seen_existing.contains(&wall_entity) {
            continue;
        }

        commands.spawn((
            Name::new("TransportRequest::DeliverToProvisionalWall"),
            Transform::from_xyz(wall_pos.x, wall_pos.y, 0.0),
            Visibility::Hidden,
            Designation {
                work_type: WorkType::Haul,
            },
            crate::relationships::ManagedBy(issued_by),
            TaskSlots::new(slots),
            Priority(PROVISIONAL_WALL_PRIORITY),
            TransportRequest {
                kind: TransportRequestKind::DeliverToProvisionalWall,
                anchor: wall_entity,
                resource_type: ResourceType::StasisMud,
                issued_by,
                priority: TransportPriority::Low,
                stockpile_group: vec![],
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

pub fn provisional_wall_material_delivery_sync_system(
    mut commands: Commands,
    mut q_walls: Query<(Entity, &Transform, &Building, &mut ProvisionalWall)>,
    q_resources: Query<(
        Entity,
        &Transform,
        &Visibility,
        &crate::systems::logistics::ResourceItem,
        Option<&crate::relationships::StoredIn>,
    )>,
    q_wall_tiles: Query<&WallTileBlueprint>,
) {
    let site_managed_walls: std::collections::HashSet<Entity> =
        q_wall_tiles.iter().filter_map(|tile| tile.spawned_wall).collect();

    let pickup_radius_sq = (TILE_SIZE * 1.5) * (TILE_SIZE * 1.5);

    for (wall_entity, wall_transform, building, mut provisional) in q_walls.iter_mut() {
        if site_managed_walls.contains(&wall_entity) {
            continue;
        }
        if building.kind != BuildingType::Wall
            || !building.is_provisional
            || provisional.mud_delivered
        {
            continue;
        }

        let wall_pos = wall_transform.translation.truncate();
        let nearest_mud = q_resources
            .iter()
            .filter(|(_, transform, visibility, item, stored_in_opt)| {
                *visibility != &Visibility::Hidden
                    && stored_in_opt.is_none()
                    && item.0 == ResourceType::StasisMud
                    && transform.translation.truncate().distance_squared(wall_pos)
                        <= pickup_radius_sq
            })
            .min_by(|(_, t1, _, _, _), (_, t2, _, _, _)| {
                t1.translation
                    .truncate()
                    .distance_squared(wall_pos)
                    .partial_cmp(&t2.translation.truncate().distance_squared(wall_pos))
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(entity, _, _, _, _)| entity);

        if let Some(mud_entity) = nearest_mud {
            commands.entity(mud_entity).try_despawn();
            provisional.mud_delivered = true;
            info!(
                "PROVISIONAL_WALL: Wall {:?} received StasisMud and is ready to coat",
                wall_entity
            );
        }
    }
}

pub fn provisional_wall_designation_system(
    mut commands: Commands,
    q_walls: Query<(
        Entity,
        &Building,
        &ProvisionalWall,
        Option<&Designation>,
        Option<&TaskWorkers>,
    )>,
    q_wall_tiles: Query<&WallTileBlueprint>,
) {
    let site_managed_walls: std::collections::HashSet<Entity> =
        q_wall_tiles.iter().filter_map(|tile| tile.spawned_wall).collect();

    for (wall_entity, building, provisional, designation_opt, workers_opt) in q_walls.iter() {
        if site_managed_walls.contains(&wall_entity) {
            continue;
        }
        if building.kind != BuildingType::Wall {
            continue;
        }

        let should_designate = building.is_provisional && provisional.mud_delivered;
        let has_workers = workers_opt.map(|workers| workers.len()).unwrap_or(0) > 0;
        let is_coat_designation = designation_opt
            .map(|designation| designation.work_type == WorkType::CoatWall)
            .unwrap_or(false);

        if should_designate {
            if !is_coat_designation {
                commands.entity(wall_entity).insert((
                    Designation {
                        work_type: WorkType::CoatWall,
                    },
                    TaskSlots::new(1),
                    Priority(PROVISIONAL_WALL_PRIORITY),
                ));
            }
            continue;
        }

        if is_coat_designation && !has_workers {
            commands
                .entity(wall_entity)
                .remove::<Designation>()
                .remove::<TaskSlots>()
                .remove::<Priority>();
        }
    }
}
