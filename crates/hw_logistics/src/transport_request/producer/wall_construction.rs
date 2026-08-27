//! Wall construction auto-haul system
//!
//! Creates transport requests for wood and mud delivery to wall construction sites.

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use hw_core::constants::{
    TILE_SIZE, WALL_COAT_PRIORITY, WALL_FRAME_PRIORITY, WALL_MUD_PER_TILE, WALL_WOOD_PER_TILE,
    WHEELBARROW_CAPACITY,
};
use hw_core::relationships::TaskWorkers;
use hw_jobs::construction::{TargetWallConstructionSite, WallConstructionPhase, WallTileBlueprint};
use hw_jobs::{Designation, Priority, TaskSlots, WallConstructionSite, WallTileState, WorkType};
use hw_spatial::ResourceSpatialGrid;
use hw_world::{PairedYard, Site};
use std::time::Instant;

use crate::tile_index::TileSiteIndex;
use crate::transport_request::producer::active_unit_cache::{
    CachedActiveFamiliars, CachedActiveYards,
};
use crate::transport_request::producer::tile_wait_cache::WallTileWaitingCache;
use crate::transport_request::producer::{
    ConstructionDeliverySpec, ConstructionMaterialConsumptionShadow, RequestSyncSpec,
};
use crate::transport_request::{TransportRequestKind, WallMaterialSyncMetrics};
use crate::types::{ResourceItem, ResourceType};

type WallTileDesignationQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Transform,
        &'static mut WallTileBlueprint,
        Option<&'static Designation>,
        Option<&'static TaskWorkers>,
        &'static mut Visibility,
    ),
>;

type WallTileMutQuery<'w, 's> = Query<'w, 's, &'static mut WallTileBlueprint>;
type WallResourcesQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Transform,
        &'static Visibility,
        &'static ResourceItem,
        Option<&'static hw_core::relationships::StoredIn>,
    ),
>;

#[derive(SystemParam)]
pub struct WallMaterialDeliveryContext<'w, 's> {
    q_sites: Query<'w, 's, (Entity, &'static WallConstructionSite)>,
    q_tiles: WallTileMutQuery<'w, 's>,
    q_resources: WallResourcesQuery<'w, 's>,
    resource_grid: Res<'w, ResourceSpatialGrid>,
    tile_site_index: Res<'w, TileSiteIndex>,
    consumption_shadow: ResMut<'w, ConstructionMaterialConsumptionShadow>,
    metrics: ResMut<'w, WallMaterialSyncMetrics>,
    nearby_buf: Local<'s, Vec<Entity>>,
}

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
    familiars_cache: Res<CachedActiveFamiliars>,
    yards_cache: Res<CachedActiveYards>,
    q_paired_sites: Query<(&Site, &PairedYard)>,
    q_sites: Query<(
        Entity,
        &Transform,
        &WallConstructionSite,
        Option<&TaskWorkers>,
    )>,
    q_wall_requests: super::ExistingConstructionRequestQuery<TargetWallConstructionSite>,
    waiting_cache: Res<WallTileWaitingCache>,
) {
    let active_familiars = &familiars_cache.data;
    let active_yards = &yards_cache.data;
    let paired_sites: Vec<_> = q_paired_sites
        .iter()
        .map(|(site, paired_yard)| (paired_yard.0, site.bounds()))
        .collect();
    let all_owners =
        super::collect_construction_area_owners(active_familiars, active_yards, &paired_sites);

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

        let (waiting_wood, waiting_mud) = waiting_cache
            .map
            .get(&site_entity)
            .copied()
            .unwrap_or((0, 0));
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
        RequestSyncSpec {
            expected_kind: TransportRequestKind::DeliverToWallConstruction,
            request_name: "TransportRequest::DeliverToWallConstruction",
            request_kind: TransportRequestKind::DeliverToWallConstruction,
        },
        |target| target.0,
        TargetWallConstructionSite,
        request_priority,
    );
}

/// Consumes delivered materials around each wall site and advances tiles to ready states.
pub fn wall_material_delivery_sync_system(
    mut commands: Commands,
    context: WallMaterialDeliveryContext,
) {
    let WallMaterialDeliveryContext {
        q_sites,
        mut q_tiles,
        q_resources,
        resource_grid,
        tile_site_index,
        mut consumption_shadow,
        mut metrics,
        mut nearby_buf,
    } = context;
    let started_at = Instant::now();
    let pickup_radius = TILE_SIZE * 2.0;
    let mut sites_processed = 0u32;
    let mut resources_scanned = 0u32;
    let mut tiles_scanned = 0u32;

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
        let Some(site_tiles) = tile_site_index.wall_tiles_by_site.get(&site_entity) else {
            continue;
        };
        tiles_scanned = tiles_scanned.saturating_add(site_tiles.len() as u32);

        let consumed = super::sync_construction_delivery(
            &mut commands,
            ConstructionDeliverySpec {
                site_pos: site.material_center,
                target_resource,
                required_amount,
                pickup_radius,
                resource_grid: &resource_grid,
                scratch: &mut nearby_buf,
                resources_scanned: &mut resources_scanned,
                site_tiles,
                consumed_resources: &mut consumption_shadow.consumed,
            },
            &q_resources,
            &mut q_tiles,
            |tile: &WallTileBlueprint| tile.state == waiting_state,
            |tile: &mut WallTileBlueprint| match site.phase {
                WallConstructionPhase::Framing => &mut tile.wood_delivered,
                WallConstructionPhase::Coating => &mut tile.mud_delivered,
            },
            |tile: &mut WallTileBlueprint| {
                tile.state = ready_state;
            },
        );

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
pub fn wall_tile_designation_system(mut commands: Commands, mut q_tiles: WallTileDesignationQuery) {
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

#[cfg(test)]
mod material_sync_tests {
    use super::*;
    use hw_core::area::TaskArea;
    use hw_spatial::SpatialGridOps;

    #[test]
    fn material_sync_reads_only_the_indexed_site_tiles() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<ResourceSpatialGrid>()
            .init_resource::<TileSiteIndex>()
            .init_resource::<WallMaterialSyncMetrics>()
            .init_resource::<ConstructionMaterialConsumptionShadow>()
            .add_systems(Update, wall_material_delivery_sync_system);
        let center = Vec2::new(64.0, 64.0);
        let site = app
            .world_mut()
            .spawn(WallConstructionSite::new(
                TaskArea::from_points(center, center),
                center,
                1,
            ))
            .id();
        let tile = app
            .world_mut()
            .spawn(WallTileBlueprint::new(site, (2, 2)))
            .id();
        let unrelated_site = app.world_mut().spawn_empty().id();
        let unrelated_tile = app
            .world_mut()
            .spawn(WallTileBlueprint::new(unrelated_site, (20, 20)))
            .id();
        app.world_mut()
            .resource_mut::<TileSiteIndex>()
            .rebuild_from_tiles([], [(tile, site), (unrelated_tile, unrelated_site)]);

        let resource = app
            .world_mut()
            .spawn((
                ResourceItem(ResourceType::Wood),
                Transform::from_translation(center.extend(0.0)),
                Visibility::Visible,
            ))
            .id();
        app.world_mut()
            .resource_mut::<ResourceSpatialGrid>()
            .insert(resource, center);

        app.update();

        let tile_state = app.world().entity(tile).get::<WallTileBlueprint>().unwrap();
        assert_eq!(tile_state.wood_delivered, 1);
        assert_eq!(tile_state.state, WallTileState::FramingReady);
        assert_eq!(
            app.world()
                .entity(unrelated_tile)
                .get::<WallTileBlueprint>()
                .unwrap()
                .wood_delivered,
            0
        );
        assert_eq!(
            app.world()
                .resource::<WallMaterialSyncMetrics>()
                .wall_material_sync_tiles_scanned,
            1
        );
        assert!(app.world().get_entity(resource).is_err());
    }
}
