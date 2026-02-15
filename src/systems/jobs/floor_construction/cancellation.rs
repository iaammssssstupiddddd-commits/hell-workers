//! Floor construction cancellation system

use super::components::{
    FloorConstructionCancelRequested, FloorConstructionSite, FloorTileBlueprint,
    TargetFloorConstructionSite,
};
use crate::entities::damned_soul::{DamnedSoul, Path};
use crate::relationships::WorkingOn;
use crate::systems::logistics::Inventory;
use crate::systems::soul_ai::execute::task_execution::context::TaskAssignmentQueries;
use crate::systems::soul_ai::execute::task_execution::types::AssignedTask;
use crate::systems::soul_ai::helpers::work::unassign_task;
use crate::world::map::WorldMap;
use bevy::prelude::*;
use std::collections::HashSet;

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
    q_sites: Query<
        (Entity, &FloorConstructionSite),
        With<FloorConstructionCancelRequested>,
    >,
    q_tiles: Query<(Entity, &FloorTileBlueprint)>,
    q_floor_requests: Query<(Entity, &TargetFloorConstructionSite)>,
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
    mut reservation_queries: TaskAssignmentQueries,
    world_map: Res<WorldMap>,
) {
    for (site_entity, site) in q_sites.iter() {
        let site_tiles: Vec<Entity> = q_tiles
            .iter()
            .filter(|(_, tile)| tile.parent_site == site_entity)
            .map(|(tile_entity, _)| tile_entity)
            .collect();

        let site_requests: Vec<Entity> = q_floor_requests
            .iter()
            .filter(|(_, target_site)| target_site.0 == site_entity)
            .map(|(request_entity, _)| request_entity)
            .collect();

        let mut related_targets: HashSet<Entity> =
            HashSet::with_capacity(site_tiles.len() + site_requests.len() + 1);
        related_targets.extend(site_tiles.iter().copied());
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

        for request_entity in site_requests {
            commands.entity(request_entity).try_despawn();
        }

        for tile_entity in site_tiles {
            commands.entity(tile_entity).try_despawn();
        }

        commands.entity(site_entity).try_despawn();

        info!(
            "FLOOR_CANCEL: Site {:?} cancelled (tiles: {}, workers released: {})",
            site_entity,
            site.tiles_total,
            released_workers
        );
    }
}
