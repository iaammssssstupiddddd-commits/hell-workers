//! Wall construction auto-haul system
//!
//! Creates transport requests for wood and mud delivery to wall construction sites.

use bevy::prelude::*;
use hw_core::area::TaskArea;
use hw_core::constants::{
    TILE_SIZE, WALL_COAT_PRIORITY, WALL_FRAME_PRIORITY, WALL_MUD_PER_TILE, WALL_WOOD_PER_TILE,
    WHEELBARROW_CAPACITY,
};
use hw_core::familiar::{ActiveCommand, FamiliarCommand};
use hw_core::relationships::TaskWorkers;
use hw_jobs::construction::{TargetWallConstructionSite, WallConstructionPhase, WallTileBlueprint};
use hw_jobs::{Designation, Priority, TaskSlots, WallConstructionSite, WallTileState, WorkType};
use hw_spatial::ResourceSpatialGrid;
use hw_world::zones::Yard;
use std::collections::HashMap;
use std::time::Instant;

use crate::transport_request::{TransportRequest, TransportRequestKind, TransportRequestMetrics};
use crate::types::{ResourceItem, ResourceType};

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
    q_yards: Query<(Entity, &Yard)>,
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
    let active_familiars: Vec<_> = q_familiars
        .iter()
        .filter(|(_, active_command, _)| !matches!(active_command.command, FamiliarCommand::Idle))
        .map(|(entity, _, area)| (entity, area.bounds()))
        .collect();
    let active_yards: Vec<(Entity, Yard)> = q_yards.iter().map(|(e, y)| (e, y.clone())).collect();
    let all_owners = super::collect_all_area_owners(&active_familiars, &active_yards);

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
        let Some((fam_entity, _)) = super::find_owner(site_pos, &all_owners) else {
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

    // 3. Upsert/cleanup transport request entities
    super::sync_construction_requests(
        &mut commands,
        &q_wall_requests,
        &desired_requests,
        TransportRequestKind::DeliverToWallConstruction,
        "TransportRequest::DeliverToWallConstruction",
        TransportRequestKind::DeliverToWallConstruction,
        |target| target.0,
        TargetWallConstructionSite,
        request_priority,
    );
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
        &ResourceItem,
        Option<&hw_core::relationships::StoredIn>,
    )>,
    resource_grid: Res<ResourceSpatialGrid>,
    mut nearby_buf: Local<Vec<Entity>>,
    mut metrics: ResMut<TransportRequestMetrics>,
) {
    let started_at = Instant::now();
    let pickup_radius = TILE_SIZE * 2.0;
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

        let consumed = {
            let mut q_tiles_write = q_tiles.p1();
            super::sync_construction_delivery(
                &mut commands,
                site_entity,
                site.material_center,
                target_resource,
                required_amount,
                pickup_radius,
                &resource_grid,
                &q_resources,
                &mut *nearby_buf,
                &mut resources_scanned,
                &tiles_by_site,
                &mut q_tiles_write,
                |tile: &WallTileBlueprint| tile.state == waiting_state,
                |tile: &mut WallTileBlueprint| match site.phase {
                    WallConstructionPhase::Framing => &mut tile.wood_delivered,
                    WallConstructionPhase::Coating => &mut tile.mud_delivered,
                },
                |tile: &mut WallTileBlueprint| {
                    tile.state = ready_state;
                },
            )
        };

        if consumed > 0 {
            debug!(
                "WALL_MATERIAL_SYNC: site {:?} consumed {} {:?}",
                site_entity, consumed, target_resource
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
