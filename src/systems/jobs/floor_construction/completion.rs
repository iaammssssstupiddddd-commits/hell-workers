//! Floor construction completion system

use super::components::*;
use crate::constants::Z_MAP;
use crate::systems::jobs::{Building, BuildingType};
use crate::world::map::WorldMap;
use bevy::prelude::*;

/// Handles floor construction completion
pub fn floor_construction_completion_system(
    q_sites: Query<(Entity, &FloorConstructionSite)>,
    q_tiles: Query<&FloorTileBlueprint>,
    mut world_map: ResMut<WorldMap>,
    mut commands: Commands,
) {
    for (site_entity, _site) in q_sites.iter() {
        // Check if all tiles complete
        let all_complete = q_tiles
            .iter()
            .filter(|t| t.parent_site == site_entity)
            .all(|t| t.state == FloorTileState::Complete);

        if !all_complete {
            continue;
        }

        // For each tile: spawn Building entity with Floor type
        for tile in q_tiles.iter().filter(|t| t.parent_site == site_entity) {
            let world_pos = WorldMap::grid_to_world(tile.grid_pos.0, tile.grid_pos.1);

            commands.spawn((
                Building {
                    kind: BuildingType::Floor,
                    is_provisional: false,
                },
                Transform::from_translation(world_pos.extend(Z_MAP + 0.01)),
                Visibility::default(),
            ));

            // Update WorldMap walkability (remove obstacle)
            world_map.remove_obstacle(tile.grid_pos.0, tile.grid_pos.1);
        }

        // Despawn site and tiles
        commands.entity(site_entity).despawn();

        info!("Floor site {:?} completed", site_entity);
    }
}
