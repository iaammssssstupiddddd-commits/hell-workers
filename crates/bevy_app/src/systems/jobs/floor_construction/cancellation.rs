//! Floor construction cancellation system

use super::components::FloorConstructionCancelRequested;
use crate::entities::damned_soul::{DamnedSoul, Path};
use crate::systems::jobs::{ResourceItemVisualHandles, spawn_refund_items};
use crate::systems::logistics::{Inventory, ResourceType};
use crate::systems::soul_ai::execute::task_execution::context::TaskQueries;
use crate::systems::soul_ai::execute::task_execution::types::AssignedTask;
use crate::world::map::WorldMapWrite;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use hw_core::relationships::WorkingOn;
use hw_logistics::tile_index::TileSiteIndex;
use hw_logistics::transport_request::{TransportRequest, TransportRequestKind};
use hw_soul_ai::unassign_task;
use std::collections::HashSet;

#[derive(Clone, Copy)]
struct SiteTileSnapshot {
    entity: Entity,
    grid_pos: (i32, i32),
    bones_delivered: u32,
    mud_delivered: u32,
}

fn is_floor_task_for_site(task: &AssignedTask, site_entity: Entity) -> bool {
    match task {
        AssignedTask::ReinforceFloorTile(data) => data.site == site_entity,
        AssignedTask::PourFloorTile(data) => data.site == site_entity,
        _ => false,
    }
}

type SoulCancellationQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Transform,
        &'static mut AssignedTask,
        &'static mut Path,
        &'static mut Inventory,
        Option<&'static WorkingOn>,
    ),
    With<DamnedSoul>,
>;

#[derive(SystemParam)]
pub struct FloorCancellationQueries<'w, 's> {
    q_sites: Query<'w, 's, Entity, With<FloorConstructionCancelRequested>>,
    q_floor_requests: Query<'w, 's, (Entity, &'static TransportRequest)>,
    tile_site_index: Res<'w, TileSiteIndex>,
}

/// Cancels floor construction sites marked with `FloorConstructionCancelRequested`.
///
/// Cancellation is site-wide:
/// - unassign all workers working on the site or its related request/tile entities
/// - despawn floor transport requests linked to the site
/// - despawn all tile blueprints
/// - despawn the site itself
pub fn floor_construction_cancellation_system(
    mut commands: Commands,
    fl_queries: FloorCancellationQueries,
    mut q_souls: SoulCancellationQuery,
    mut reservation_queries: TaskQueries,
    mut world_map: WorldMapWrite,
    resource_item_handles: Res<ResourceItemVisualHandles>,
) {
    for site_entity in fl_queries.q_sites.iter() {
        let (site_material_center, site_tiles_total) = {
            let Ok((_site_transform, site, _)) =
                reservation_queries.storage.floor_sites.get(site_entity)
            else {
                continue;
            };
            (site.material_center, site.tiles_total)
        };

        let indexed_tiles = fl_queries
            .tile_site_index
            .floor_tiles_by_site
            .get(&site_entity)
            .cloned()
            .unwrap_or_default();
        let mut site_tiles = Vec::with_capacity(indexed_tiles.len());
        for entity in indexed_tiles {
            if let Ok((_, tile, _)) = reservation_queries.storage.floor_tiles.get_mut(entity)
                && tile.parent_site == site_entity
            {
                site_tiles.push(SiteTileSnapshot {
                    entity,
                    grid_pos: tile.grid_pos,
                    bones_delivered: tile.bones_delivered,
                    mud_delivered: tile.mud_delivered,
                });
            }
        }
        let mut seen_tiles: HashSet<Entity> = site_tiles.iter().map(|tile| tile.entity).collect();
        for (entity, tile, _) in reservation_queries.storage.floor_tiles.iter_mut() {
            if tile.parent_site == site_entity && seen_tiles.insert(entity) {
                site_tiles.push(SiteTileSnapshot {
                    entity,
                    grid_pos: tile.grid_pos,
                    bones_delivered: tile.bones_delivered,
                    mud_delivered: tile.mud_delivered,
                });
            }
        }

        let site_requests: Vec<Entity> = fl_queries
            .q_floor_requests
            .iter()
            .filter(|(_, request)| {
                request.kind == TransportRequestKind::DeliverToFloorConstruction
                    && request.anchor == site_entity
            })
            .map(|(request_entity, _)| request_entity)
            .collect();

        let mut related_targets: HashSet<Entity> =
            HashSet::with_capacity(site_tiles.len() + site_requests.len() + 1);
        related_targets.extend(site_tiles.iter().map(|tile| tile.entity));
        related_targets.extend(site_requests.iter().copied());
        related_targets.insert(site_entity);

        let mut released_workers = 0usize;
        for (
            soul_entity,
            soul_transform,
            mut assigned_task,
            mut path,
            mut inventory,
            working_on_opt,
        ) in q_souls.iter_mut()
        {
            let matches_site_task = is_floor_task_for_site(&assigned_task, site_entity);
            let matches_working_on = working_on_opt
                .map(|working_on| related_targets.contains(&working_on.0))
                .unwrap_or(false);

            if !(matches_site_task || matches_working_on) {
                continue;
            }

            unassign_task(
                &mut commands,
                hw_soul_ai::SoulDropCtx {
                    soul_entity,
                    drop_pos: soul_transform.translation.truncate(),
                    inventory: Some(&mut inventory),
                    dropped_item_res: None,
                },
                &mut assigned_task,
                &mut path,
                &mut reservation_queries,
                &world_map,
                true,
            );
            released_workers += 1;
        }

        let refunded_bones: u32 = site_tiles.iter().map(|tile| tile.bones_delivered).sum();
        let refunded_mud: u32 = site_tiles.iter().map(|tile| tile.mud_delivered).sum();
        spawn_refund_items(
            &mut commands,
            &resource_item_handles,
            site_material_center,
            ResourceType::Bone,
            refunded_bones,
        );
        spawn_refund_items(
            &mut commands,
            &resource_item_handles,
            site_material_center,
            ResourceType::StasisMud,
            refunded_mud,
        );

        for request_entity in site_requests {
            commands.entity(request_entity).try_despawn();
        }

        for grid in site_tiles.iter().map(|tile| tile.grid_pos) {
            world_map.clear_building_occupancy_if_owned(grid, site_entity);
        }

        for tile in site_tiles {
            commands.entity(tile.entity).try_despawn();
        }

        commands.entity(site_entity).try_despawn();

        info!(
            "FLOOR_CANCEL: Site {:?} cancelled (tiles: {}, workers: {}, refund bone: {}, refund mud: {})",
            site_entity, site_tiles_total, released_workers, refunded_bones, refunded_mud
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::schedule::ApplyDeferred;
    use hw_core::area::TaskArea;
    use hw_core::events::{OnTaskAbandoned, ResourceReservationRequest};
    use hw_jobs::construction::{FloorConstructionSite, FloorTileBlueprint};
    use hw_jobs::{ReinforceFloorPhase, ReinforceFloorTileData};
    use hw_logistics::SharedResourceCache;
    use hw_logistics::transport_request::{TransportPriority, TransportRequest};

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
    fn construction_cancellation_floor_falls_back_from_empty_index_and_anchor_request() {
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
                (floor_construction_cancellation_system, ApplyDeferred).chain(),
            );

        let site = app
            .world_mut()
            .spawn((
                Transform::default(),
                FloorConstructionSite::new(
                    TaskArea::from_points(Vec2::ZERO, Vec2::splat(32.0)),
                    Vec2::ZERO,
                    1,
                ),
                FloorConstructionCancelRequested,
            ))
            .id();
        let mut tile = FloorTileBlueprint::new(site, (4, 5));
        tile.bones_delivered = 2;
        tile.mud_delivered = 1;
        let tile_entity = app.world_mut().spawn(tile).id();
        let soul = app
            .world_mut()
            .spawn((
                Transform::default(),
                DamnedSoul::default(),
                AssignedTask::ReinforceFloorTile(ReinforceFloorTileData {
                    tile: tile_entity,
                    site,
                    phase: ReinforceFloorPhase::GoingToTile,
                }),
                Path::default(),
                Inventory::default(),
                WorkingOn(tile_entity),
            ))
            .id();
        let request = app
            .world_mut()
            .spawn(TransportRequest {
                kind: TransportRequestKind::DeliverToFloorConstruction,
                anchor: site,
                resource_type: ResourceType::Bone,
                issued_by: site,
                priority: TransportPriority::Normal,
                stockpile_group: vec![],
            })
            .id();
        app.world_mut()
            .resource_mut::<crate::world::map::WorldMap>()
            .set_building_occupancy((4, 5), site);

        app.update();

        assert!(app.world().get_entity(site).is_err());
        assert!(app.world().get_entity(tile_entity).is_err());
        assert!(app.world().get_entity(request).is_err());
        assert!(matches!(
            app.world().get::<AssignedTask>(soul),
            Some(AssignedTask::None)
        ));
        assert!(app.world().get::<WorkingOn>(soul).is_none());
        assert_eq!(
            app.world()
                .resource::<crate::world::map::WorldMap>()
                .building_entity((4, 5)),
            None
        );
        let mut refunds = app.world_mut().query::<&hw_logistics::ResourceItem>();
        assert_eq!(refunds.iter(app.world()).count(), 3);
    }

    #[test]
    fn construction_cancellation_floor_handles_a_zero_tile_site() {
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
                (floor_construction_cancellation_system, ApplyDeferred).chain(),
            );
        let site = app
            .world_mut()
            .spawn((
                Transform::default(),
                FloorConstructionSite::new(
                    TaskArea::from_points(Vec2::ZERO, Vec2::ZERO),
                    Vec2::ZERO,
                    0,
                ),
                FloorConstructionCancelRequested,
            ))
            .id();

        app.update();

        assert!(app.world().get_entity(site).is_err());
        let mut refunds = app.world_mut().query::<&hw_logistics::ResourceItem>();
        assert_eq!(refunds.iter(app.world()).count(), 0);
    }
}
