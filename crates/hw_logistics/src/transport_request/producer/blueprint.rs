//! Blueprint auto-haul system

use bevy::prelude::*;

use hw_core::constants::WHEELBARROW_CAPACITY;
use hw_core::relationships::{ManagedBy, TaskWorkers};
use hw_jobs::{Blueprint, Designation, Priority, TargetBlueprint, TaskSlots, WorkType};
use hw_spatial::BlueprintSpatialGrid;
use hw_world::{PairedYard, Site};

use crate::transport_request::producer::active_unit_cache::{
    CachedActiveFamiliars, CachedActiveYards,
};
use crate::transport_request::{
    TransportDemand, TransportPolicy, TransportPriority, TransportRequest, TransportRequestKind,
    TransportRequestState,
};
use crate::types::ResourceType;

pub fn blueprint_auto_haul_system(
    mut commands: Commands,
    blueprint_grid: Res<BlueprintSpatialGrid>,
    familiars_cache: Res<CachedActiveFamiliars>,
    yards_cache: Res<CachedActiveYards>,
    q_paired_sites: Query<(&Site, &PairedYard)>,
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

    let active_familiars = &familiars_cache.data;
    let active_yards = &yards_cache.data;

    let mut desired_requests =
        std::collections::HashMap::<(Entity, ResourceType), (Entity, u32, Vec2)>::new();

    let paired_sites: Vec<_> = q_paired_sites
        .iter()
        .map(|(site, paired_yard)| (paired_yard.0, site.bounds()))
        .collect();
    let all_owners =
        super::collect_construction_area_owners(active_familiars, active_yards, &paired_sites);

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
            super::find_owner_for_position(bp_pos, &all_owners, active_yards)
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

#[cfg(test)]
mod tests {
    use super::*;
    use hw_jobs::BuildingType;
    use hw_spatial::SpatialGridOps;

    #[test]
    fn paired_yard_issues_blueprint_request_without_active_familiar() {
        let mut app = App::new();
        app.init_resource::<BlueprintSpatialGrid>()
            .init_resource::<CachedActiveFamiliars>()
            .init_resource::<CachedActiveYards>()
            .add_systems(Update, blueprint_auto_haul_system);

        let yard_bounds = hw_world::Yard {
            min: Vec2::new(320.0, 0.0),
            max: Vec2::new(640.0, 320.0),
        };
        let yard = app.world_mut().spawn(yard_bounds.clone()).id();
        app.world_mut()
            .resource_mut::<CachedActiveYards>()
            .data
            .push((yard, yard_bounds));
        app.world_mut().spawn((
            Site {
                min: Vec2::ZERO,
                max: Vec2::new(288.0, 320.0),
            },
            PairedYard(yard),
        ));

        let blueprint_pos = Vec2::new(128.0, 128.0);
        let mut blueprint = Blueprint::new(BuildingType::Tank, vec![(4, 4)]);
        blueprint.required_materials.clear();
        blueprint.required_materials.insert(ResourceType::Wood, 1);
        blueprint.flexible_material_requirement = None;
        let blueprint_entity = app
            .world_mut()
            .spawn((
                Transform::from_translation(blueprint_pos.extend(0.0)),
                blueprint,
            ))
            .id();
        app.world_mut()
            .resource_mut::<BlueprintSpatialGrid>()
            .insert(blueprint_entity, blueprint_pos);

        app.update();

        let world = app.world_mut();
        let mut requests = world.query::<(&TransportRequest, &TargetBlueprint, &ManagedBy)>();
        let matching: Vec<_> = requests
            .iter(world)
            .filter(|(request, target, _)| {
                target.0 == blueprint_entity && request.resource_type == ResourceType::Wood
            })
            .collect();
        assert_eq!(matching.len(), 1);
        assert_eq!(matching[0].0.issued_by, yard);
        assert_eq!(matching[0].2.0, yard);
    }
}
