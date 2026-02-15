// Floor construction auto-haul system
//!
//! Creates transport requests for bones and mud delivery to floor construction sites

use bevy::prelude::*;

use crate::constants::WHEELBARROW_CAPACITY;
use crate::entities::familiar::{ActiveCommand, FamiliarCommand};
use crate::relationships::TaskWorkers;
use crate::systems::command::TaskArea;
use crate::systems::jobs::floor_construction::{
    FloorConstructionPhase, FloorConstructionSite, FloorTileBlueprint, FloorTileState,
    TargetFloorConstructionSite,
};
use crate::systems::jobs::{Designation, Priority, TaskSlots, WorkType};
use crate::systems::logistics::transport_request::{
    TransportDemand, TransportPolicy, TransportPriority, TransportRequest, TransportRequestKind,
    TransportRequestState,
};
use crate::systems::logistics::ResourceType;
use crate::systems::spatial::FloorConstructionSpatialGrid;

const FLOOR_BONES_PER_TILE: u32 = 2;
const FLOOR_MUD_PER_TILE: u32 = 1;
const FLOOR_CONSTRUCTION_PRIORITY: u32 = 10;

/// Auto-haul system for floor construction materials
pub fn floor_construction_auto_haul_system(
    mut commands: Commands,
    floor_grid: Res<FloorConstructionSpatialGrid>,
    q_familiars: Query<(Entity, &ActiveCommand, &TaskArea)>,
    q_sites: Query<(Entity, &Transform, &FloorConstructionSite, Option<&TaskWorkers>)>,
    q_tiles: Query<&FloorTileBlueprint>,
    q_floor_requests: Query<(
        Entity,
        &TargetFloorConstructionSite,
        &TransportRequest,
        Option<&TaskWorkers>,
    )>,
) {
    // 1. Count in-flight deliveries per (SiteEntity, ResourceType)
    let mut in_flight = std::collections::HashMap::<(Entity, ResourceType), usize>::new();

    for (_, target_site, req, workers_opt) in q_floor_requests.iter() {
        if matches!(req.kind, TransportRequestKind::DeliverToFloorConstruction) {
            let count = workers_opt.map(|w| w.len()).unwrap_or(0);
            if count > 0 {
                *in_flight
                    .entry((target_site.0, req.resource_type))
                    .or_insert(0) += count;
            }
        }
    }

    // Collect active familiars
    let active_familiars: Vec<(Entity, TaskArea)> = q_familiars
        .iter()
        .filter(|(_, active_command, _)| {
            !matches!(active_command.command, FamiliarCommand::Idle)
        })
        .map(|(entity, _, area)| (entity, area.clone()))
        .collect();

    // 2. Calculate material needs for each site
    let mut desired_requests =
        std::collections::HashMap::<(Entity, ResourceType), (Entity, u32, Vec2)>::new();

    let mut sites_to_process = std::collections::HashSet::new();
    for (_, area) in &active_familiars {
        for &site_entity in floor_grid.get_in_area(area.min, area.max).iter() {
            sites_to_process.insert(site_entity);
        }
    }

    for site_entity in sites_to_process {
        let Ok((_, site_transform, site, workers_opt)) = q_sites.get(site_entity) else {
            continue;
        };
        let site_pos = site_transform.translation.truncate();

        // Skip if workers are actively building
        if workers_opt.map(|w| w.len()).unwrap_or(0) > 0 {
            continue;
        }

        let Some((fam_entity, _task_area)) = super::find_owner_familiar(site_pos, &active_familiars)
        else {
            continue;
        };

        // Count tiles in different states
        let mut waiting_bones = 0u32;
        let mut waiting_mud = 0u32;

        for tile in q_tiles.iter().filter(|t| t.parent_site == site_entity) {
            match tile.state {
                FloorTileState::WaitingBones => {
                    let needed = FLOOR_BONES_PER_TILE.saturating_sub(tile.bones_delivered);
                    waiting_bones += needed;
                }
                FloorTileState::WaitingMud => {
                    let needed = FLOOR_MUD_PER_TILE.saturating_sub(tile.mud_delivered);
                    waiting_mud += needed;
                }
                _ => {}
            }
        }

        // Create request for bones (Reinforcing phase)
        if waiting_bones > 0 && matches!(site.phase, FloorConstructionPhase::Reinforcing) {
            let resource_type = ResourceType::Bone;
            let inflight_count = *in_flight.get(&(site_entity, resource_type)).unwrap_or(&0);

            if inflight_count < waiting_bones as usize {
                let needed = waiting_bones.saturating_sub(inflight_count as u32);
                // Bones don't require wheelbarrow, so slots = needed items
                let desired_slots = needed.max(1);
                desired_requests.insert(
                    (site_entity, resource_type),
                    (fam_entity, desired_slots, site.material_center),
                );
            }
        }

        // Create request for mud (Pouring phase)
        if waiting_mud > 0 && matches!(site.phase, FloorConstructionPhase::Pouring) {
            let resource_type = ResourceType::StasisMud;
            let inflight_count = *in_flight.get(&(site_entity, resource_type)).unwrap_or(&0);

            if inflight_count < waiting_mud as usize {
                let needed = waiting_mud.saturating_sub(inflight_count as u32);
                // Mud requires wheelbarrow
                let desired_slots = needed.div_ceil(WHEELBARROW_CAPACITY as u32).max(1);
                desired_requests.insert(
                    (site_entity, resource_type),
                    (fam_entity, desired_slots, site.material_center),
                );
            }
        }
    }

    // 3. Upsert/cleanup transport request entities
    let mut seen_existing_keys = std::collections::HashSet::<(Entity, ResourceType)>::new();

    for (request_entity, target_site, request, workers_opt) in q_floor_requests.iter() {
        if !matches!(
            request.kind,
            TransportRequestKind::DeliverToFloorConstruction
        ) {
            continue;
        }
        let key = (target_site.0, request.resource_type);
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

        if let Some((issued_by, slots, site_pos)) = desired_requests.get(&key) {
            commands.entity(request_entity).try_insert((
                Transform::from_xyz(site_pos.x, site_pos.y, 0.0),
                Visibility::Hidden,
                Designation {
                    work_type: WorkType::Haul,
                },
                crate::relationships::ManagedBy(*issued_by),
                TaskSlots::new(*slots),
                Priority(FLOOR_CONSTRUCTION_PRIORITY),
                TargetFloorConstructionSite(key.0),
                TransportRequest {
                    kind: TransportRequestKind::DeliverToFloorConstruction,
                    anchor: key.0,
                    resource_type: key.1,
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

    // 4. Spawn new request entities
    for (key, (issued_by, slots, site_pos)) in desired_requests {
        if seen_existing_keys.contains(&key) {
            continue;
        }

        commands.spawn((
            Name::new("TransportRequest::DeliverToFloorConstruction"),
            Transform::from_xyz(site_pos.x, site_pos.y, 0.0),
            Visibility::Hidden,
            Designation {
                work_type: WorkType::Haul,
            },
            crate::relationships::ManagedBy(issued_by),
            TaskSlots::new(slots),
            Priority(FLOOR_CONSTRUCTION_PRIORITY),
            TargetFloorConstructionSite(key.0),
            TransportRequest {
                kind: TransportRequestKind::DeliverToFloorConstruction,
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

/// System to assign Designation to FloorTileBlueprint based on their state
///
/// This system runs in TransportRequestSet::Decide phase (after material delivery logic)
/// to prepare tiles for worker assignment.
pub fn floor_tile_designation_system(
    mut commands: Commands,
    q_tiles: Query<(Entity, &Transform, &FloorTileBlueprint, Option<&Designation>)>,
) {
    for (tile_entity, tile_transform, tile, designation_opt) in q_tiles.iter() {
        let desired_work_type = match tile.state {
            FloorTileState::ReinforcingReady => Some(WorkType::ReinforceFloorTile),
            FloorTileState::PouringReady => Some(WorkType::PourFloorTile),
            _ => None,
        };

        match (desired_work_type, designation_opt) {
            // Need to add designation
            (Some(work_type), None) => {
                commands.entity(tile_entity).try_insert((
                    Transform::from_xyz(
                        tile_transform.translation.x,
                        tile_transform.translation.y,
                        tile_transform.translation.z,
                    ),
                    Visibility::Hidden,
                    Designation { work_type },
                    TaskSlots::new(1), // Only 1 worker per tile
                    Priority(FLOOR_CONSTRUCTION_PRIORITY),
                ));
            }
            // Need to remove designation
            (None, Some(_)) => {
                commands.entity(tile_entity).remove::<Designation>();
                commands.entity(tile_entity).remove::<TaskSlots>();
                commands.entity(tile_entity).remove::<Priority>();
            }
            // Already correct or no change needed
            _ => {}
        }
    }
}
