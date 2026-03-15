//! Floor construction cancellation system

use super::components::{FloorConstructionCancelRequested, TargetFloorConstructionSite};
use crate::entities::damned_soul::{DamnedSoul, Path};
use crate::systems::jobs::construction_shared::{ResourceItemVisualHandles, spawn_refund_items};
use crate::systems::logistics::{Inventory, ResourceType};
use crate::systems::soul_ai::execute::task_execution::context::TaskQueries;
use crate::systems::soul_ai::execute::task_execution::types::AssignedTask;
use crate::systems::soul_ai::helpers::work::unassign_task;
use crate::world::map::WorldMapWrite;
use bevy::prelude::*;
use hw_core::relationships::WorkingOn;
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

/// Cancels floor construction sites marked with `FloorConstructionCancelRequested`.
///
/// Cancellation is site-wide:
/// - unassign all workers working on the site or its related request/tile entities
/// - despawn floor transport requests linked to the site
/// - despawn all tile blueprints
/// - despawn the site itself
pub fn floor_construction_cancellation_system(
    mut commands: Commands,
    q_sites: Query<Entity, With<FloorConstructionCancelRequested>>,
    q_floor_requests: Query<(Entity, &TargetFloorConstructionSite)>,
    q_entities: Query<Entity>,
    mut q_souls: Query<
        (
            Entity,
            &Transform,
            &mut AssignedTask,
            &mut Path,
            &mut Inventory,
            Option<&WorkingOn>,
        ),
        With<DamnedSoul>,
    >,
    mut reservation_queries: TaskQueries,
    mut world_map: WorldMapWrite,
    resource_item_handles: Res<ResourceItemVisualHandles>,
) {
    for site_entity in q_sites.iter() {
        let (site_material_center, site_tiles_total) = {
            let Ok((_site_transform, site, _)) =
                reservation_queries.storage.floor_sites.get(site_entity)
            else {
                continue;
            };
            (site.material_center, site.tiles_total)
        };

        let mut site_tiles: Vec<SiteTileSnapshot> = Vec::new();
        for entity in q_entities.iter() {
            let Ok(tile) = reservation_queries.storage.floor_tiles.get_mut(entity) else {
                continue;
            };
            if tile.parent_site != site_entity {
                continue;
            }
            site_tiles.push(SiteTileSnapshot {
                entity,
                grid_pos: tile.grid_pos,
                bones_delivered: tile.bones_delivered,
                mud_delivered: tile.mud_delivered,
            });
        }

        let site_requests: Vec<Entity> = q_floor_requests
            .iter()
            .filter(|(_, target_site)| target_site.0 == site_entity)
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
                soul_entity,
                soul_transform.translation.truncate(),
                &mut assigned_task,
                &mut path,
                Some(&mut inventory),
                None,
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

        let cleared_grids: Vec<(i32, i32)> = site_tiles.iter().map(|tile| tile.grid_pos).collect();
        world_map.clear_building_footprint(cleared_grids);

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
