//! Wall construction cancellation system

use super::components::WallConstructionCancelRequested;
use crate::systems::jobs::ResourceItemVisualHandles;
use crate::systems::jobs::construction_cancellation::{
    ConstructionCancellationPolicy, ConstructionCancellationSoulQuery, collect_site_requests,
    release_matching_workers, spawn_construction_refunds,
};
use crate::systems::soul_ai::execute::task_execution::context::TaskQueries;
use crate::world::map::WorldMapWrite;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use hw_logistics::tile_index::TileSiteIndex;
use hw_logistics::transport_request::TransportRequest;
use std::collections::HashSet;

#[derive(Clone, Copy)]
struct SiteTileSnapshot {
    entity: Entity,
    grid_pos: (i32, i32),
    wood_delivered: u32,
    mud_delivered: u32,
    spawned_wall: Option<Entity>,
}

#[derive(SystemParam)]
pub struct WallCancellationQueries<'w, 's> {
    q_sites: Query<'w, 's, Entity, With<WallConstructionCancelRequested>>,
    q_wall_requests: Query<'w, 's, (Entity, &'static TransportRequest)>,
    tile_site_index: Res<'w, TileSiteIndex>,
}

/// Cancels wall construction sites marked with `WallConstructionCancelRequested`.
pub fn wall_construction_cancellation_system(
    mut commands: Commands,
    wl_queries: WallCancellationQueries,
    mut q_souls: ConstructionCancellationSoulQuery,
    mut reservation_queries: TaskQueries,
    mut world_map: WorldMapWrite,
    resource_item_handles: Res<ResourceItemVisualHandles>,
) {
    for site_entity in wl_queries.q_sites.iter() {
        let (site_material_center, site_tiles_total) = {
            let Ok((_site_transform, site, _)) =
                reservation_queries.storage.wall_sites.get(site_entity)
            else {
                continue;
            };
            (site.material_center, site.tiles_total)
        };

        let indexed_tiles = wl_queries
            .tile_site_index
            .wall_tiles_by_site
            .get(&site_entity)
            .cloned()
            .unwrap_or_default();
        let mut site_tiles: Vec<SiteTileSnapshot> = Vec::with_capacity(indexed_tiles.len());
        for tile_entity in indexed_tiles {
            if let Ok((_, tile, _)) = reservation_queries.storage.wall_tiles.get_mut(tile_entity)
                && tile.parent_site == site_entity
            {
                site_tiles.push(SiteTileSnapshot {
                    entity: tile_entity,
                    grid_pos: tile.grid_pos,
                    wood_delivered: tile.wood_delivered,
                    mud_delivered: tile.mud_delivered,
                    spawned_wall: tile.spawned_wall,
                });
            }
        }
        let mut seen_tiles: HashSet<Entity> = site_tiles.iter().map(|tile| tile.entity).collect();
        for (tile_entity, tile, _) in reservation_queries.storage.wall_tiles.iter_mut() {
            if tile.parent_site == site_entity && seen_tiles.insert(tile_entity) {
                site_tiles.push(SiteTileSnapshot {
                    entity: tile_entity,
                    grid_pos: tile.grid_pos,
                    wood_delivered: tile.wood_delivered,
                    mud_delivered: tile.mud_delivered,
                    spawned_wall: tile.spawned_wall,
                });
            }
        }

        let site_requests = collect_site_requests(
            &wl_queries.q_wall_requests,
            site_entity,
            ConstructionCancellationPolicy::WALL,
        );

        let mut related_targets: HashSet<Entity> =
            HashSet::with_capacity(site_tiles.len() + site_requests.len() + 1);
        related_targets.extend(site_tiles.iter().map(|tile| tile.entity));
        related_targets.extend(site_tiles.iter().filter_map(|tile| tile.spawned_wall));
        related_targets.extend(site_requests.iter().copied());
        related_targets.insert(site_entity);

        let released_workers = release_matching_workers(
            &mut commands,
            &mut q_souls,
            &mut reservation_queries,
            &world_map,
            |assigned_task, working_on_opt| {
                assigned_task
                    .primary_payload_entity()
                    .is_some_and(|target| related_targets.contains(&target))
                    || working_on_opt
                        .is_some_and(|working_on| related_targets.contains(&working_on.0))
            },
        );

        let refunded_wood: u32 = site_tiles.iter().map(|tile| tile.wood_delivered).sum();
        let refunded_mud: u32 = site_tiles.iter().map(|tile| tile.mud_delivered).sum();
        spawn_construction_refunds(
            &mut commands,
            &resource_item_handles,
            site_material_center,
            ConstructionCancellationPolicy::WALL,
            refunded_wood,
            refunded_mud,
        );

        for request_entity in site_requests {
            commands.entity(request_entity).try_despawn();
        }

        for tile in &site_tiles {
            if world_map
                .building_entity(tile.grid_pos)
                .is_some_and(|owner| owner == site_entity || Some(owner) == tile.spawned_wall)
            {
                world_map.clear_building_occupancy(tile.grid_pos);
            }
        }

        for tile in site_tiles {
            if let Some(wall_entity) = tile.spawned_wall {
                commands.entity(wall_entity).try_despawn();
            }
            commands.entity(tile.entity).try_despawn();
        }

        commands.entity(site_entity).try_despawn();

        info!(
            "WALL_CANCEL: Site {:?} cancelled (tiles: {}, workers: {}, refund wood: {}, refund mud: {})",
            site_entity, site_tiles_total, released_workers, refunded_wood, refunded_mud
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entities::damned_soul::{DamnedSoul, Path};
    use crate::systems::logistics::{Inventory, ResourceType};
    use crate::systems::soul_ai::execute::task_execution::types::AssignedTask;
    use bevy::ecs::schedule::ApplyDeferred;
    use hw_core::area::TaskArea;
    use hw_core::events::{OnTaskAbandoned, ResourceReservationRequest};
    use hw_core::relationships::WorkingOn;
    use hw_core::visual_mirror::building::{BuildingTypeVisual, BuildingVisualState};
    use hw_core::visual_mirror::construction::WallTileVisualMirror;
    use hw_jobs::construction::{WallConstructionSite, WallTileBlueprint};
    use hw_jobs::{FrameWallPhase, FrameWallTileData};
    use hw_logistics::SharedResourceCache;
    use hw_logistics::transport_request::{
        TransportPriority, TransportRequest, TransportRequestKind,
    };
    use hw_visual::wall_connection::{
        WallConnectionDirty, WallConnectionMask, WallTopologyIndex, WallTopologyState,
        wall_connections_system,
    };

    fn empty_handles() -> ResourceItemVisualHandles {
        ResourceItemVisualHandles {
            icon_bone_small: default(),
            icon_wood_small: default(),
            icon_rock_small: default(),
            icon_sand_small: default(),
            icon_stasis_mud_small: default(),
        }
    }

    #[test]
    fn construction_cancellation_wall_falls_back_from_empty_index_and_anchor_request() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .insert_resource(crate::world::map::WorldMap::default())
            .init_resource::<TileSiteIndex>()
            .init_resource::<SharedResourceCache>()
            .insert_resource(empty_handles())
            .add_message::<ResourceReservationRequest>()
            .add_message::<OnTaskAbandoned>()
            .add_systems(
                Update,
                (wall_construction_cancellation_system, ApplyDeferred).chain(),
            );

        let site = app
            .world_mut()
            .spawn((
                Transform::default(),
                WallConstructionSite::new(
                    TaskArea::from_points(Vec2::ZERO, Vec2::splat(32.0)),
                    Vec2::ZERO,
                    1,
                ),
                WallConstructionCancelRequested,
            ))
            .id();
        let mut tile = WallTileBlueprint::new(site, (7, 8));
        tile.wood_delivered = 1;
        tile.mud_delivered = 1;
        let spawned_wall = app.world_mut().spawn_empty().id();
        tile.spawned_wall = Some(spawned_wall);
        let tile_entity = app.world_mut().spawn(tile).id();
        let soul = app
            .world_mut()
            .spawn((
                Transform::default(),
                DamnedSoul::default(),
                AssignedTask::FrameWallTile(FrameWallTileData {
                    tile: tile_entity,
                    site,
                    phase: FrameWallPhase::GoingToTile,
                }),
                Path::default(),
                Inventory::default(),
                WorkingOn(tile_entity),
            ))
            .id();
        let request = app
            .world_mut()
            .spawn(TransportRequest {
                kind: TransportRequestKind::DeliverToWallConstruction,
                anchor: site,
                resource_type: ResourceType::Wood,
                issued_by: site,
                priority: TransportPriority::Normal,
                stockpile_group: vec![],
            })
            .id();
        app.world_mut()
            .resource_mut::<crate::world::map::WorldMap>()
            .set_building_occupancy((7, 8), site);

        app.update();

        assert!(app.world().get_entity(site).is_err());
        assert!(app.world().get_entity(tile_entity).is_err());
        assert!(app.world().get_entity(request).is_err());
        assert!(app.world().get_entity(spawned_wall).is_err());
        assert!(matches!(
            app.world().get::<AssignedTask>(soul),
            Some(AssignedTask::None)
        ));
        assert!(app.world().get::<WorkingOn>(soul).is_none());
        assert_eq!(
            app.world()
                .resource::<crate::world::map::WorldMap>()
                .building_entity((7, 8)),
            None
        );
        let mut refunds = app.world_mut().query::<&hw_logistics::ResourceItem>();
        assert_eq!(refunds.iter(app.world()).count(), 2);
    }

    #[test]
    fn construction_cancellation_wall_handles_a_zero_tile_site() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .insert_resource(crate::world::map::WorldMap::default())
            .init_resource::<TileSiteIndex>()
            .init_resource::<SharedResourceCache>()
            .insert_resource(empty_handles())
            .add_message::<ResourceReservationRequest>()
            .add_message::<OnTaskAbandoned>()
            .add_systems(
                Update,
                (wall_construction_cancellation_system, ApplyDeferred).chain(),
            );
        let site = app
            .world_mut()
            .spawn((
                Transform::default(),
                WallConstructionSite::new(
                    TaskArea::from_points(Vec2::ZERO, Vec2::ZERO),
                    Vec2::ZERO,
                    0,
                ),
                WallConstructionCancelRequested,
            ))
            .id();

        app.update();

        assert!(app.world().get_entity(site).is_err());
        let mut refunds = app.world_mut().query::<&hw_logistics::ResourceItem>();
        assert_eq!(refunds.iter(app.world()).count(), 0);
    }

    #[test]
    fn cancellation_removes_pre_and_post_framing_connectors_in_the_same_frame() {
        for with_spawned_wall in [false, true] {
            let mut app = App::new();
            app.add_plugins(MinimalPlugins)
                .insert_resource(crate::world::map::WorldMap::default())
                .init_resource::<TileSiteIndex>()
                .init_resource::<SharedResourceCache>()
                .insert_resource(empty_handles())
                .insert_resource(crate::test_support::empty_wall_visual_handles())
                .init_resource::<WallConnectionDirty>()
                .init_resource::<WallTopologyIndex>()
                .add_message::<ResourceReservationRequest>()
                .add_message::<OnTaskAbandoned>()
                .add_systems(
                    Update,
                    (wall_construction_cancellation_system, ApplyDeferred).chain(),
                )
                .add_systems(PostUpdate, (wall_connections_system, ApplyDeferred).chain());

            let target_grid = (7, 7);
            let construction_grid = (7, 8);
            let target = app
                .world_mut()
                .spawn((
                    Transform::from_translation(
                        crate::world::map::WorldMap::grid_to_world(target_grid.0, target_grid.1)
                            .extend(0.0),
                    ),
                    BuildingVisualState {
                        kind: BuildingTypeVisual::Wall,
                        is_provisional: false,
                    },
                ))
                .id();
            let site = app
                .world_mut()
                .spawn((
                    Transform::default(),
                    WallConstructionSite::new(
                        TaskArea::from_points(Vec2::ZERO, Vec2::splat(32.0)),
                        Vec2::ZERO,
                        1,
                    ),
                ))
                .id();
            let spawned_wall = with_spawned_wall.then(|| {
                app.world_mut()
                    .spawn((
                        Transform::from_translation(
                            crate::world::map::WorldMap::grid_to_world(
                                construction_grid.0,
                                construction_grid.1,
                            )
                            .extend(0.0),
                        ),
                        BuildingVisualState {
                            kind: BuildingTypeVisual::Wall,
                            is_provisional: true,
                        },
                    ))
                    .id()
            });
            let mut tile = WallTileBlueprint::new(site, construction_grid);
            tile.spawned_wall = spawned_wall;
            let tile_entity = app
                .world_mut()
                .spawn((
                    tile,
                    WallTileVisualMirror::default(),
                    Transform::from_translation(
                        crate::world::map::WorldMap::grid_to_world(
                            construction_grid.0,
                            construction_grid.1,
                        )
                        .extend(0.0),
                    ),
                ))
                .id();
            app.world_mut()
                .resource_mut::<TileSiteIndex>()
                .wall_tiles_by_site
                .insert(site, vec![tile_entity]);
            app.world_mut()
                .resource_mut::<crate::world::map::WorldMap>()
                .set_building_occupancy(construction_grid, spawned_wall.unwrap_or(site));

            app.update();
            assert_eq!(
                app.world().get::<WallTopologyState>(target).unwrap().mask,
                WallConnectionMask::from_neighbors(true, false, false, false),
                "construction connector missing before cancellation (spawned={with_spawned_wall})"
            );

            app.world_mut()
                .entity_mut(site)
                .insert(WallConstructionCancelRequested);
            app.update();

            assert!(app.world().get_entity(site).is_err());
            assert!(app.world().get_entity(tile_entity).is_err());
            if let Some(wall) = spawned_wall {
                assert!(app.world().get_entity(wall).is_err());
            }
            assert_eq!(
                app.world().get::<WallTopologyState>(target).unwrap().mask,
                WallConnectionMask::from_neighbors(false, false, false, false),
                "cancelled connector survived PostUpdate (spawned={with_spawned_wall})"
            );
        }
    }
}
