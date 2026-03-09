//! Tank water request system

use bevy::prelude::*;
use hw_core::constants::BUCKET_CAPACITY;

use hw_core::area::TaskArea;
use hw_core::familiar::{ActiveCommand, FamiliarCommand};
use hw_core::relationships::{IncomingDeliveries, StoredItems, TaskWorkers};
use hw_jobs::{Designation, MovePlanned, Priority, TaskSlots, WorkType};
use hw_world::zones::{AreaBounds, Yard};

use crate::transport_request::{
    TransportDemand, TransportPolicy, TransportPriority, TransportRequest, TransportRequestKind,
    TransportRequestState,
};
use crate::types::ResourceType;
use crate::zone::Stockpile;
use crate::water::tank_can_accept_new_bucket;

pub fn tank_water_request_system(
    mut commands: Commands,
    q_incoming: Query<&IncomingDeliveries>,
    q_familiars: Query<(Entity, &ActiveCommand, &TaskArea)>,
    q_yards: Query<(Entity, &Yard)>,
    q_tanks: Query<(Entity, &Transform, &Stockpile, Option<&StoredItems>)>,
    q_tank_requests: Query<(Entity, &TransportRequest, Option<&TaskWorkers>)>,
    q_move_planned: Query<(), With<MovePlanned>>,
) {
    let active_familiars: Vec<(Entity, AreaBounds)> = q_familiars
        .iter()
        .filter(|(_, active_command, _)| !matches!(active_command.command, FamiliarCommand::Idle))
        .map(|(entity, _, area)| (entity, area.bounds()))
        .collect();
    let active_yards: Vec<(Entity, Yard)> = q_yards.iter().map(|(e, y)| (e, y.clone())).collect();
    let all_owners = super::collect_all_area_owners(&active_familiars, &active_yards);

    let mut desired_requests = std::collections::HashMap::<Entity, (Entity, u32, Vec2)>::new();

    for (tank_entity, tank_transform, tank_stock, stored_opt) in q_tanks.iter() {
        if q_move_planned.get(tank_entity).is_ok() {
            continue;
        }
        if tank_stock.resource_type != Some(ResourceType::Water) {
            continue;
        }

        let tank_pos = tank_transform.translation.truncate();
        let Some((fam_entity, _)) = super::find_owner(tank_pos, &all_owners) else {
            continue;
        };

        let current_water = stored_opt.map(|s| s.len()).unwrap_or(0);
        let incoming_water_tasks = q_incoming
            .get(tank_entity)
            .ok()
            .map(|inc: &IncomingDeliveries| inc.len())
            .unwrap_or(0);
        let total_water = (current_water as u32) + (incoming_water_tasks as u32 * BUCKET_CAPACITY);

        if tank_can_accept_new_bucket(current_water, incoming_water_tasks, tank_stock.capacity) {
            let needed_water = tank_stock.capacity as u32 - total_water;
            let needed_tasks = needed_water / BUCKET_CAPACITY;

            if needed_tasks > 0 {
                desired_requests.insert(tank_entity, (fam_entity, needed_tasks, tank_pos));
            }
        }
    }

    let mut seen_existing = std::collections::HashSet::<Entity>::new();

    for (request_entity, request, workers_opt) in q_tank_requests.iter() {
        if request.kind != TransportRequestKind::GatherWaterToTank {
            continue;
        }
        let tank_entity = request.anchor;
        let workers = workers_opt.map(|w| w.len()).unwrap_or(0);

        if !super::upsert::process_duplicate_key(
            &mut commands,
            request_entity,
            workers,
            &mut seen_existing,
            tank_entity,
        ) {
            continue;
        }

        if let Some((issued_by, slots, tank_pos)) = desired_requests.get(&tank_entity) {
            commands.entity(request_entity).try_insert((
                Transform::from_xyz(tank_pos.x, tank_pos.y, 0.0),
                Visibility::Hidden,
                Designation {
                    work_type: WorkType::GatherWater,
                },
                hw_core::relationships::ManagedBy(*issued_by),
                TaskSlots::new(*slots),
                Priority(3),
                TransportRequest {
                    kind: TransportRequestKind::GatherWaterToTank,
                    anchor: tank_entity,
                    resource_type: ResourceType::Water,
                    issued_by: *issued_by,
                    priority: TransportPriority::Normal,
                    stockpile_group: vec![],
                },
                TransportDemand {
                    desired_slots: *slots,
                    inflight: 0,
                },
                TransportRequestState::Pending,
                TransportPolicy::default(),
            ));
            continue;
        }

        if workers == 0 {
            super::upsert::disable_request(&mut commands, request_entity);
        }
    }

    for (tank_entity, (issued_by, slots, tank_pos)) in desired_requests {
        if seen_existing.contains(&tank_entity) {
            continue;
        }

        commands.spawn((
            Name::new("TransportRequest::GatherWaterToTank"),
            Transform::from_xyz(tank_pos.x, tank_pos.y, 0.0),
            Visibility::Hidden,
            Designation {
                work_type: WorkType::GatherWater,
            },
            hw_core::relationships::ManagedBy(issued_by),
            TaskSlots::new(slots),
            Priority(3),
            TransportRequest {
                kind: TransportRequestKind::GatherWaterToTank,
                anchor: tank_entity,
                resource_type: ResourceType::Water,
                issued_by,
                priority: TransportPriority::Normal,
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
