//! Floor construction auto-haul system
//!
//! Creates transport requests for bones and mud delivery to floor construction sites

use bevy::prelude::*;
use hw_core::area::TaskArea;
use hw_core::constants::{
    FLOOR_BONES_PER_TILE, FLOOR_CONSTRUCTION_PRIORITY, FLOOR_MUD_PER_TILE, TILE_SIZE,
    WHEELBARROW_CAPACITY,
};
use hw_core::familiar::{ActiveCommand, FamiliarCommand};
use hw_core::relationships::TaskWorkers;
use hw_jobs::construction::{
    FloorConstructionPhase, FloorTileBlueprint, TargetFloorConstructionSite,
};
use hw_jobs::{FloorConstructionSite, FloorTileState};
use hw_spatial::{FloorConstructionSpatialGrid, ResourceSpatialGrid};
use hw_world::zones::AreaBounds;
use std::collections::HashMap;
use std::time::Instant;

use crate::transport_request::{TransportRequest, TransportRequestKind, TransportRequestMetrics};
use crate::types::{ResourceItem, ResourceType};

mod designation;
pub use designation::floor_tile_designation_system;

/// Auto-haul system for floor construction materials
pub fn floor_construction_auto_haul_system(
    mut commands: Commands,
    floor_grid: Res<FloorConstructionSpatialGrid>,
    q_familiars: Query<(Entity, &ActiveCommand, &TaskArea)>,
    q_sites: Query<(
        Entity,
        &Transform,
        &FloorConstructionSite,
        Option<&TaskWorkers>,
    )>,
    q_tiles: Query<&FloorTileBlueprint>,
    q_floor_requests: Query<(
        Entity,
        &TargetFloorConstructionSite,
        &TransportRequest,
        Option<&TaskWorkers>,
    )>,
) {
    // Collect active familiars
    let active_familiars: Vec<(Entity, AreaBounds)> = q_familiars
        .iter()
        .filter(|(_, active_command, _)| !matches!(active_command.command, FamiliarCommand::Idle))
        .map(|(entity, _, area)| (entity, area.bounds()))
        .collect();

    // Site ごとの不足材料をタイル1回走査で集計する。
    let mut waiting_by_site = HashMap::<Entity, (u32, u32)>::new();
    for tile in q_tiles.iter() {
        match tile.state {
            FloorTileState::WaitingBones => {
                let needed = FLOOR_BONES_PER_TILE.saturating_sub(tile.bones_delivered);
                if needed > 0 {
                    let entry = waiting_by_site.entry(tile.parent_site).or_insert((0, 0));
                    entry.0 = entry.0.saturating_add(needed);
                }
            }
            FloorTileState::WaitingMud => {
                let needed = FLOOR_MUD_PER_TILE.saturating_sub(tile.mud_delivered);
                if needed > 0 {
                    let entry = waiting_by_site.entry(tile.parent_site).or_insert((0, 0));
                    entry.1 = entry.1.saturating_add(needed);
                }
            }
            _ => {}
        }
    }

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

        let Some((fam_entity, _)) = super::find_owner(site_pos, &active_familiars) else {
            continue;
        };

        let (waiting_bones, waiting_mud) =
            waiting_by_site.get(&site_entity).copied().unwrap_or((0, 0));
        if waiting_bones == 0 && waiting_mud == 0 {
            continue;
        }

        // Create request for bones (Reinforcing phase)
        if waiting_bones > 0 && matches!(site.phase, FloorConstructionPhase::Reinforcing) {
            let resource_type = ResourceType::Bone;
            let desired_slots = waiting_bones.max(1);
            desired_requests.insert(
                (site_entity, resource_type),
                (fam_entity, desired_slots, site.material_center),
            );
        }

        // Create request for mud (Pouring phase)
        if waiting_mud > 0 && matches!(site.phase, FloorConstructionPhase::Pouring) {
            let resource_type = ResourceType::StasisMud;
            // Mud requires wheelbarrow
            let desired_slots = waiting_mud.div_ceil(WHEELBARROW_CAPACITY as u32).max(1);
            desired_requests.insert(
                (site_entity, resource_type),
                (fam_entity, desired_slots, site.material_center),
            );
        }
    }

    // 3. Upsert/cleanup transport request entities
    super::sync_construction_requests(
        &mut commands,
        &q_floor_requests,
        &desired_requests,
        TransportRequestKind::DeliverToFloorConstruction,
        "TransportRequest::DeliverToFloorConstruction",
        TransportRequestKind::DeliverToFloorConstruction,
        |target| target.0,
        TargetFloorConstructionSite,
        |_| FLOOR_CONSTRUCTION_PRIORITY,
    );
}

/// Consumes delivered materials around each site and advances tiles to ready states.
///
/// `DeliverToFloorConstruction` requests drop items near `site.material_center`.
/// This system binds those items to waiting tiles (by incrementing `*_delivered`) and
/// despawns consumed items so each resource is counted exactly once.
pub fn floor_material_delivery_sync_system(
    mut commands: Commands,
    q_sites: Query<(Entity, &FloorConstructionSite)>,
    mut q_tiles: ParamSet<(
        Query<(Entity, &FloorTileBlueprint)>,
        Query<&mut FloorTileBlueprint>,
    )>,
    q_resources: Query<(
        Entity,
        &Transform,
        &Visibility,
        &ResourceItem,
        Option<&hw_core::relationships::StoredIn>,
    )>,
    resource_grid: Res<ResourceSpatialGrid>,
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
            FloorConstructionPhase::Reinforcing => (
                ResourceType::Bone,
                FLOOR_BONES_PER_TILE,
                FloorTileState::WaitingBones,
                FloorTileState::ReinforcingReady,
            ),
            FloorConstructionPhase::Pouring => (
                ResourceType::StasisMud,
                FLOOR_MUD_PER_TILE,
                FloorTileState::WaitingMud,
                FloorTileState::PouringReady,
            ),
            FloorConstructionPhase::Curing => continue,
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
                &mut resources_scanned,
                &tiles_by_site,
                &mut q_tiles_write,
                |tile: &FloorTileBlueprint| tile.state == waiting_state,
                |tile: &mut FloorTileBlueprint| match site.phase {
                    FloorConstructionPhase::Reinforcing => &mut tile.bones_delivered,
                    FloorConstructionPhase::Pouring => &mut tile.mud_delivered,
                    FloorConstructionPhase::Curing => {
                        unreachable!("curing phase should be skipped")
                    }
                },
                |tile: &mut FloorTileBlueprint| {
                    tile.state = ready_state;
                },
            )
        };

        if consumed > 0 {
            debug!(
                "FLOOR_MATERIAL_SYNC: site {:?} consumed {} {:?}",
                site_entity, consumed, target_resource
            );
        }
    }

    metrics.floor_material_sync_sites_processed = sites_processed;
    metrics.floor_material_sync_resources_scanned = resources_scanned;
    metrics.floor_material_sync_tiles_scanned = tiles_scanned;
    metrics.floor_material_sync_elapsed_ms = started_at.elapsed().as_secs_f32() * 1000.0;
}
