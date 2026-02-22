//! Wall construction auto-haul system
//!
//! Creates transport requests for wood and mud delivery to wall construction sites.

use bevy::prelude::*;

use crate::constants::{
    TILE_SIZE, WALL_COAT_PRIORITY, WALL_FRAME_PRIORITY, WALL_MUD_PER_TILE, WALL_WOOD_PER_TILE,
    WHEELBARROW_CAPACITY,
};
use crate::entities::familiar::{ActiveCommand, FamiliarCommand};
use crate::relationships::TaskWorkers;
use crate::systems::command::TaskArea;
use crate::systems::jobs::wall_construction::{
    TargetWallConstructionSite, WallConstructionPhase, WallConstructionSite, WallTileBlueprint,
    WallTileState,
};
use crate::systems::jobs::{Designation, Priority, TaskSlots, WorkType};
use crate::systems::logistics::ResourceType;
use crate::systems::logistics::transport_request::{
    TransportRequest, TransportRequestKind, TransportRequestMetrics,
};
use crate::systems::spatial::ResourceSpatialGrid;
use std::collections::HashMap;
use std::time::Instant;

fn request_priority(resource_type: ResourceType) -> u32 {
    match resource_type {
        ResourceType::Wood => WALL_FRAME_PRIORITY,
        ResourceType::StasisMud => WALL_COAT_PRIORITY,
        _ => WALL_FRAME_PRIORITY,
    }
}

/// Auto-haul system for wall construction materials
pub fn wall_construction_auto_haul_system(
    mut commands: Commands,
    q_familiars: Query<(Entity, &ActiveCommand, &TaskArea)>,
    q_sites: Query<(
        Entity,
        &Transform,
        &WallConstructionSite,
        Option<&TaskWorkers>,
    )>,
    q_tiles: Query<&WallTileBlueprint>,
    q_wall_requests: Query<(
        Entity,
        &TargetWallConstructionSite,
        &TransportRequest,
        Option<&TaskWorkers>,
    )>,
) {
    let mut in_flight = std::collections::HashMap::<(Entity, ResourceType), usize>::new();
    for (_, target_site, req, workers_opt) in q_wall_requests.iter() {
        if !matches!(req.kind, TransportRequestKind::DeliverToWallConstruction) {
            continue;
        }
        let count = workers_opt.map(|w| w.len()).unwrap_or(0);
        if count > 0 {
            *in_flight
                .entry((target_site.0, req.resource_type))
                .or_insert(0) += count;
        }
    }

    let active_familiars: Vec<(Entity, TaskArea)> = q_familiars
        .iter()
        .filter(|(_, active_command, _)| !matches!(active_command.command, FamiliarCommand::Idle))
        .map(|(entity, _, area)| (entity, area.clone()))
        .collect();

    let mut waiting_by_site = HashMap::<Entity, (u32, u32)>::new();
    for tile in q_tiles.iter() {
        match tile.state {
            WallTileState::WaitingWood => {
                let needed = WALL_WOOD_PER_TILE.saturating_sub(tile.wood_delivered);
                if needed > 0 {
                    let entry = waiting_by_site.entry(tile.parent_site).or_insert((0, 0));
                    entry.0 = entry.0.saturating_add(needed);
                }
            }
            WallTileState::WaitingMud => {
                let needed = WALL_MUD_PER_TILE.saturating_sub(tile.mud_delivered);
                if needed > 0 {
                    let entry = waiting_by_site.entry(tile.parent_site).or_insert((0, 0));
                    entry.1 = entry.1.saturating_add(needed);
                }
            }
            _ => {}
        }
    }

    let mut desired_requests =
        std::collections::HashMap::<(Entity, ResourceType), (Entity, u32, Vec2)>::new();

    for (site_entity, site_transform, site, workers_opt) in q_sites.iter() {
        if workers_opt.map(|w| w.len()).unwrap_or(0) > 0 {
            continue;
        }

        let site_pos = site_transform.translation.truncate();
        let Some((fam_entity, _)) = super::find_owner_familiar(site_pos, &active_familiars) else {
            continue;
        };

        let (waiting_wood, waiting_mud) =
            waiting_by_site.get(&site_entity).copied().unwrap_or((0, 0));
        if waiting_wood == 0 && waiting_mud == 0 {
            continue;
        }

        if waiting_wood > 0 && matches!(site.phase, WallConstructionPhase::Framing) {
            let resource_type = ResourceType::Wood;
            desired_requests.insert(
                (site_entity, resource_type),
                (fam_entity, waiting_wood.max(1), site.material_center),
            );
        }

        if waiting_mud > 0 && matches!(site.phase, WallConstructionPhase::Coating) {
            let resource_type = ResourceType::StasisMud;
            let total_slots = waiting_mud.div_ceil(WHEELBARROW_CAPACITY as u32).max(1);
            desired_requests.insert(
                (site_entity, resource_type),
                (fam_entity, total_slots, site.material_center),
            );
        }
    }

    let mut seen_existing_keys = std::collections::HashSet::<(Entity, ResourceType)>::new();
    for (request_entity, target_site, request, workers_opt) in q_wall_requests.iter() {
        if !matches!(
            request.kind,
            TransportRequestKind::DeliverToWallConstruction
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

        let inflight = super::to_u32_saturating(workers);
        if let Some((issued_by, slots, site_pos)) = desired_requests.get(&key) {
            super::upsert::upsert_transport_request(
                &mut commands,
                request_entity,
                key,
                *site_pos,
                *issued_by,
                *slots,
                inflight,
                request_priority(key.1),
                TargetWallConstructionSite(key.0),
                TransportRequestKind::DeliverToWallConstruction,
            );
            continue;
        }

        super::upsert::disable_request_with_demand(&mut commands, request_entity, inflight);
    }

    for (key, (issued_by, slots, site_pos)) in desired_requests {
        if seen_existing_keys.contains(&key) {
            continue;
        }

        super::upsert::spawn_transport_request(
            &mut commands,
            "TransportRequest::DeliverToWallConstruction",
            key,
            site_pos,
            issued_by,
            slots,
            request_priority(key.1),
            TargetWallConstructionSite(key.0),
            TransportRequestKind::DeliverToWallConstruction,
        );
    }
}

/// Consumes delivered materials around each wall site and advances tiles to ready states.
pub fn wall_material_delivery_sync_system(
    mut commands: Commands,
    q_sites: Query<(Entity, &WallConstructionSite)>,
    mut q_tiles: ParamSet<(
        Query<(Entity, &WallTileBlueprint)>,
        Query<&mut WallTileBlueprint>,
    )>,
    q_resources: Query<(
        Entity,
        &Transform,
        &Visibility,
        &crate::systems::logistics::ResourceItem,
        Option<&crate::relationships::StoredIn>,
    )>,
    resource_grid: Res<ResourceSpatialGrid>,
    mut metrics: ResMut<TransportRequestMetrics>,
) {
    let started_at = Instant::now();
    let pickup_radius = TILE_SIZE * 2.0;
    let pickup_radius_sq = pickup_radius * pickup_radius;
    let mut sites_processed = 0u32;
    let mut resources_scanned = 0u32;
    let mut tiles_scanned = 0u32;

    let tiles_by_site = {
        let q_tiles_read = q_tiles.p0();
        super::group_tiles_by_site(&q_tiles_read, |tile| tile.parent_site, &mut tiles_scanned)
    };

    for (site_entity, site) in q_sites.iter() {
        sites_processed += 1;
        let (target_resource, required_amount, waiting_state, ready_state) = match site.phase {
            WallConstructionPhase::Framing => (
                ResourceType::Wood,
                WALL_WOOD_PER_TILE,
                WallTileState::WaitingWood,
                WallTileState::FramingReady,
            ),
            WallConstructionPhase::Coating => (
                ResourceType::StasisMud,
                WALL_MUD_PER_TILE,
                WallTileState::WaitingMud,
                WallTileState::CoatingReady,
            ),
        };

        let mut nearby_resources = super::collect_nearby_resource_entities(
            site.material_center,
            pickup_radius,
            pickup_radius_sq,
            target_resource,
            &resource_grid,
            &q_resources,
            &mut resources_scanned,
        );

        if nearby_resources.is_empty() {
            continue;
        }

        let Some(site_tiles) = tiles_by_site.get(&site_entity) else {
            continue;
        };

        {
            let mut q_tiles_write = q_tiles.p1();
            super::consume_waiting_tile_resources(
                &mut commands,
                site_tiles,
                &mut q_tiles_write,
                &mut nearby_resources,
                required_amount,
                |tile: &WallTileBlueprint| tile.state == waiting_state,
                |tile: &mut WallTileBlueprint| match site.phase {
                    WallConstructionPhase::Framing => &mut tile.wood_delivered,
                    WallConstructionPhase::Coating => &mut tile.mud_delivered,
                },
                |tile: &mut WallTileBlueprint| {
                    tile.state = ready_state;
                },
            );
        }
    }

    metrics.wall_material_sync_sites_processed = sites_processed;
    metrics.wall_material_sync_resources_scanned = resources_scanned;
    metrics.wall_material_sync_tiles_scanned = tiles_scanned;
    metrics.wall_material_sync_elapsed_ms = started_at.elapsed().as_secs_f32() * 1000.0;
}

/// Assign/remove tile designations based on wall tile state.
pub fn wall_tile_designation_system(
    mut commands: Commands,
    mut q_tiles: Query<(
        Entity,
        &Transform,
        &mut WallTileBlueprint,
        Option<&Designation>,
        Option<&TaskWorkers>,
        &mut Visibility,
    )>,
) {
    for (tile_entity, tile_transform, mut tile, designation_opt, workers_opt, mut visibility) in
        q_tiles.iter_mut()
    {
        if *visibility == Visibility::Hidden {
            *visibility = Visibility::Visible;
        }

        if workers_opt.map(|w| w.len()).unwrap_or(0) == 0 {
            match tile.state {
                WallTileState::Framing { .. } => {
                    tile.state = WallTileState::FramingReady;
                }
                WallTileState::Coating { .. } => {
                    tile.state = WallTileState::CoatingReady;
                }
                _ => {}
            }
        }

        let desired = match tile.state {
            WallTileState::FramingReady => Some((WorkType::FrameWallTile, WALL_FRAME_PRIORITY)),
            WallTileState::CoatingReady => Some((WorkType::CoatWall, WALL_COAT_PRIORITY)),
            _ => None,
        };

        match (desired, designation_opt) {
            (Some((work_type, priority)), None) => {
                commands.entity(tile_entity).try_insert((
                    Transform::from_xyz(
                        tile_transform.translation.x,
                        tile_transform.translation.y,
                        tile_transform.translation.z,
                    ),
                    Visibility::Visible,
                    Designation { work_type },
                    TaskSlots::new(1),
                    Priority(priority),
                ));
            }
            (None, Some(_)) => {
                commands.entity(tile_entity).remove::<Designation>();
                commands.entity(tile_entity).remove::<TaskSlots>();
                commands.entity(tile_entity).remove::<Priority>();
            }
            _ => {}
        }
    }
}
