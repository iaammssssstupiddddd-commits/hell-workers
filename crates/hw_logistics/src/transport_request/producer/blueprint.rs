//! Blueprint auto-haul system

use bevy::prelude::*;

use hw_core::area::TaskArea;
use hw_core::constants::WHEELBARROW_CAPACITY;
use hw_core::familiar::{ActiveCommand, FamiliarCommand};
use hw_core::relationships::{ManagedBy, TaskWorkers};
use hw_jobs::{Blueprint, Designation, Priority, TargetBlueprint, TaskSlots, WorkType};
use hw_spatial::BlueprintSpatialGrid;
use hw_world::zones::{AreaBounds, Yard};

use crate::transport_request::{
    TransportDemand, TransportPolicy, TransportPriority, TransportRequest, TransportRequestKind,
    TransportRequestState,
};
use crate::types::ResourceType;

pub fn blueprint_auto_haul_system(
    mut commands: Commands,
    blueprint_grid: Res<BlueprintSpatialGrid>,
    q_familiars: Query<(Entity, &ActiveCommand, &TaskArea)>,
    q_yards: Query<(Entity, &Yard)>,
    q_blueprints: Query<(Entity, &Transform, &Blueprint, Option<&TaskWorkers>)>,
    q_bp_requests: Query<(
        Entity,
        &TargetBlueprint,
        &TransportRequest,
        Option<&TaskWorkers>,
    )>,
) {
    let mut in_flight = std::collections::HashMap::<(Entity, ResourceType), usize>::new();

    for (_, target_bp, req, workers_opt) in q_bp_requests.iter() {
        if matches!(req.kind, TransportRequestKind::DeliverToBlueprint) {
            let count = workers_opt.map(|w| w.len()).unwrap_or(0);
            if count > 0 {
                *in_flight
                    .entry((target_bp.0, req.resource_type))
                    .or_insert(0) += count;
            }
        }
    }

    let active_familiars: Vec<(Entity, AreaBounds)> = q_familiars
        .iter()
        .filter(|(_, active_command, _)| !matches!(active_command.command, FamiliarCommand::Idle))
        .map(|(entity, _, area)| (entity, area.bounds()))
        .collect();
    let active_yards: Vec<(Entity, Yard)> = q_yards
        .iter()
        .map(|(entity, yard)| (entity, yard.clone()))
        .collect();

    let mut desired_requests =
        std::collections::HashMap::<(Entity, ResourceType), (Entity, u32, Vec2)>::new();

    let all_owners = super::collect_all_area_owners(&active_familiars, &active_yards);

    let mut blueprints_to_process = std::collections::HashSet::new();
    for (_, area) in &all_owners {
        for &bp_entity in blueprint_grid.get_in_area(area.min, area.max).iter() {
            blueprints_to_process.insert(bp_entity);
        }
    }

    for bp_entity in blueprints_to_process {
        let Ok((_, bp_transform, blueprint, workers_opt)) = q_blueprints.get(bp_entity) else {
            continue;
        };
        let bp_pos = bp_transform.translation.truncate();

        if workers_opt.map(|w| w.len()).unwrap_or(0) > 0 {
            continue;
        }
        if blueprint.materials_complete() {
            continue;
        }

        let Some((fam_entity, _)) =
            super::find_owner_for_position(bp_pos, &all_owners, &active_yards)
        else {
            continue;
        };

        for (resource_type, &required) in &blueprint.required_materials {
            let delivered = *blueprint
                .delivered_materials
                .get(resource_type)
                .unwrap_or(&0);
            let inflight_count = *in_flight.get(&(bp_entity, *resource_type)).unwrap_or(&0);

            if delivered + inflight_count as u32 >= required {
                continue;
            }

            let needed = required.saturating_sub(delivered);
            let desired_slots = if resource_type.requires_wheelbarrow() {
                needed.div_ceil(WHEELBARROW_CAPACITY as u32).max(1)
            } else {
                needed.max(1)
            };
            desired_requests.insert(
                (bp_entity, *resource_type),
                (fam_entity, desired_slots, bp_pos),
            );
        }

        if let Some(flexible) = &blueprint.flexible_material_requirement
            && !flexible.is_complete()
        {
            let total_inflight: u32 = flexible
                .accepted_types
                .iter()
                .map(|resource_type| {
                    *in_flight.get(&(bp_entity, *resource_type)).unwrap_or(&0) as u32
                })
                .sum();
            let total_needed = flexible.remaining();
            if total_needed > total_inflight {
                for &resource_type in &flexible.accepted_types {
                    let desired_slots = if resource_type.requires_wheelbarrow() {
                        total_needed.div_ceil(WHEELBARROW_CAPACITY as u32).max(1)
                    } else {
                        total_needed.max(1)
                    };
                    desired_requests.insert(
                        (bp_entity, resource_type),
                        (fam_entity, desired_slots, bp_pos),
                    );
                }
            }
        }
    }

    let mut seen_existing_keys = std::collections::HashSet::<(Entity, ResourceType)>::new();

    for (request_entity, target_bp, request, workers_opt) in q_bp_requests.iter() {
        if !matches!(request.kind, TransportRequestKind::DeliverToBlueprint) {
            continue;
        }
        let key = (target_bp.0, request.resource_type);
        let workers = workers_opt.map(|w| w.len()).unwrap_or(0);

        if !super::upsert::process_duplicate_key(
            &mut commands,
            request_entity,
            workers,
            &mut seen_existing_keys,
            key,
        ) {
            continue;
        }

        if let Some((issued_by, slots, bp_pos)) = desired_requests.get(&key) {
            let inflight = super::to_u32_saturating(workers);
            commands.entity(request_entity).try_insert((
                Transform::from_xyz(bp_pos.x, bp_pos.y, 0.0),
                Visibility::Hidden,
                Designation {
                    work_type: WorkType::Haul,
                },
                ManagedBy(*issued_by),
                TaskSlots::new(*slots),
                Priority(0),
                TargetBlueprint(key.0),
                TransportRequest {
                    kind: TransportRequestKind::DeliverToBlueprint,
                    anchor: key.0,
                    resource_type: key.1,
                    issued_by: *issued_by,
                    priority: TransportPriority::Normal,
                    stockpile_group: vec![],
                },
                TransportDemand {
                    desired_slots: *slots,
                    inflight,
                },
                super::upsert::request_state_for_workers(workers),
                TransportPolicy::default(),
            ));
            continue;
        }

        if workers == 0 {
            super::upsert::disable_request(&mut commands, request_entity);
        }
    }

    for (key, (issued_by, slots, bp_pos)) in desired_requests {
        if seen_existing_keys.contains(&key) {
            continue;
        }

        commands.spawn((
            Name::new("TransportRequest::DeliverToBlueprint"),
            Transform::from_xyz(bp_pos.x, bp_pos.y, 0.0),
            Visibility::Hidden,
            Designation {
                work_type: WorkType::Haul,
            },
            ManagedBy(issued_by),
            TaskSlots::new(slots),
            Priority(0),
            TargetBlueprint(key.0),
            TransportRequest {
                kind: TransportRequestKind::DeliverToBlueprint,
                anchor: key.0,
                resource_type: key.1,
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
