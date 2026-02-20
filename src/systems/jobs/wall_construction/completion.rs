//! Wall construction completion system

use super::components::*;
use crate::systems::jobs::{Building, BuildingType, ProvisionalWall};
use crate::world::map::WorldMap;
use bevy::prelude::*;

/// Handles wall construction completion (no curing phase)
pub fn wall_construction_completion_system(
    mut q_sites: Query<(Entity, &WallConstructionSite)>,
    q_tiles: Query<(Entity, &WallTileBlueprint)>,
    q_requests: Query<(Entity, &TargetWallConstructionSite)>,
    mut q_buildings: Query<&mut Building>,
    mut world_map: ResMut<WorldMap>,
    mut commands: Commands,
) {
    for (site_entity, site) in q_sites.iter_mut() {
        let site_tiles: Vec<(Entity, (i32, i32), WallTileState, Option<Entity>)> = q_tiles
            .iter()
            .filter(|(_, tile)| tile.parent_site == site_entity)
            .map(|(tile_entity, tile)| (tile_entity, tile.grid_pos, tile.state, tile.spawned_wall))
            .collect();

        if site_tiles.is_empty() {
            continue;
        }

        let all_complete = site_tiles
            .iter()
            .all(|(_, _, state, _)| *state == WallTileState::Complete);
        if !all_complete || site.phase != WallConstructionPhase::Coating {
            continue;
        }

        for (request_entity, target_site) in q_requests.iter() {
            if target_site.0 == site_entity {
                commands.entity(request_entity).try_despawn();
            }
        }

        for (tile_entity, (gx, gy), _, spawned_wall) in site_tiles {
            if let Some(wall_entity) = spawned_wall {
                if let Ok(mut building) = q_buildings.get_mut(wall_entity)
                    && building.kind == BuildingType::Wall
                {
                    building.is_provisional = false;
                }
                commands.entity(wall_entity).remove::<ProvisionalWall>();
            } else {
                if world_map
                    .buildings
                    .get(&(gx, gy))
                    .copied()
                    .is_some_and(|entity| entity == site_entity)
                {
                    world_map.buildings.remove(&(gx, gy));
                }
                world_map.remove_obstacle(gx, gy);
            }

            commands.entity(tile_entity).try_despawn();
        }

        commands.entity(site_entity).try_despawn();

        info!(
            "Wall site {:?} completed ({} tiles, coated {}/{})",
            site_entity, site.tiles_total, site.tiles_coated, site.tiles_total
        );
    }
}
