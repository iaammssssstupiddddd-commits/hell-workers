//! Floor construction completion system

use super::components::*;
use crate::assets::GameAssets;
use crate::constants::{TILE_SIZE, Z_MAP};
use crate::systems::jobs::{Building, BuildingType};
use crate::world::map::WorldMap;
use bevy::prelude::*;

/// Handles floor construction completion
pub fn floor_construction_completion_system(
    q_sites: Query<(Entity, &FloorConstructionSite)>,
    q_tiles: Query<(Entity, &FloorTileBlueprint)>,
    game_assets: Res<GameAssets>,
    mut world_map: ResMut<WorldMap>,
    mut commands: Commands,
) {
    for (site_entity, site) in q_sites.iter() {
        // Check if all tiles complete
        let all_complete = q_tiles
            .iter()
            .filter(|(_, t)| t.parent_site == site_entity)
            .all(|(_, t)| t.state == FloorTileState::Complete);

        if !all_complete {
            continue;
        }

        // For each tile: spawn Building entity with Floor type
        let mut tile_count = 0;
        for (tile_entity, tile) in q_tiles.iter().filter(|(_, t)| t.parent_site == site_entity) {
            let world_pos = WorldMap::grid_to_world(tile.grid_pos.0, tile.grid_pos.1);

            commands.spawn((
                Building {
                    kind: BuildingType::Floor,
                    is_provisional: false,
                },
                Sprite {
                    image: game_assets.stone.clone(),
                    custom_size: Some(Vec2::splat(TILE_SIZE)),
                    ..default()
                },
                Transform::from_translation(world_pos.extend(Z_MAP + 0.01)),
                Visibility::default(),
                Name::new("Building (Floor)"),
            ));

            // Update WorldMap walkability (remove obstacle)
            world_map.remove_obstacle(tile.grid_pos.0, tile.grid_pos.1);

            // Despawn tile blueprint
            commands.entity(tile_entity).despawn();
            tile_count += 1;
        }

        // Despawn site
        commands.entity(site_entity).despawn();

        info!(
            "Floor site {:?} completed ({} tiles, total {}/{})",
            site_entity, tile_count, site.tiles_poured, site.tiles_total
        );
    }
}
