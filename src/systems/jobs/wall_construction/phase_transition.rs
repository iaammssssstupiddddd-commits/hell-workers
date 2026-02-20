//! Wall construction phase transition system

use crate::assets::GameAssets;
use crate::constants::{TILE_SIZE, Z_MAP};
use crate::systems::jobs::{Building, BuildingType, ProvisionalWall};
use crate::world::map::WorldMap;
use super::components::*;
use bevy::prelude::*;

/// Spawns provisional wall entities for framed tiles that do not have spawned walls yet.
pub fn wall_framed_tile_spawn_system(
    mut q_tiles: Query<&mut WallTileBlueprint>,
    game_assets: Res<GameAssets>,
    mut world_map: ResMut<WorldMap>,
    mut commands: Commands,
) {
    for mut tile in q_tiles.iter_mut() {
        if tile.state != WallTileState::FramedProvisional || tile.spawned_wall.is_some() {
            continue;
        }

        let world_pos = WorldMap::grid_to_world(tile.grid_pos.0, tile.grid_pos.1);
        let wall_entity = commands
            .spawn((
                Building {
                    kind: BuildingType::Wall,
                    is_provisional: true,
                },
                ProvisionalWall::default(),
                Sprite {
                    image: game_assets.wall_isolated.clone(),
                    custom_size: Some(Vec2::splat(TILE_SIZE)),
                    ..default()
                },
                Transform::from_translation(world_pos.extend(Z_MAP + 0.01)),
                Visibility::default(),
                Name::new("Building (Wall, Provisional)"),
            ))
            .id();

        tile.spawned_wall = Some(wall_entity);
        world_map.buildings.insert(tile.grid_pos, wall_entity);
        world_map.add_obstacle(tile.grid_pos.0, tile.grid_pos.1);
    }
}

/// Handles transition from Framing to Coating phase
pub fn wall_construction_phase_transition_system(
    mut q_sites: Query<(Entity, &mut WallConstructionSite)>,
    mut q_tiles: Query<(Entity, &mut WallTileBlueprint)>,
    mut commands: Commands,
) {
    for (site_entity, mut site) in q_sites.iter_mut() {
        if site.phase != WallConstructionPhase::Framing {
            continue;
        }

        let mut total_tiles = 0;
        let mut framed_tiles = 0;

        for (_, tile) in q_tiles.iter().filter(|(_, t)| t.parent_site == site_entity) {
            total_tiles += 1;
            if matches!(tile.state, WallTileState::FramedProvisional) && tile.spawned_wall.is_some() {
                framed_tiles += 1;
            }
        }

        // もし残存タイルが0になってしまった場合は何もしない
        if total_tiles == 0 {
            continue;
        }

        if framed_tiles >= total_tiles {
            site.phase = WallConstructionPhase::Coating;

            for (tile_entity, mut tile) in q_tiles
                .iter_mut()
                .filter(|(_, tile)| tile.parent_site == site_entity)
            {
                tile.state = WallTileState::WaitingMud;
                commands.entity(tile_entity).remove::<(
                    crate::systems::jobs::Designation,
                    crate::systems::jobs::TaskSlots,
                    crate::systems::jobs::Priority,
                )>();
            }

            info!(
                "Wall site {:?} -> Coating phase (all {} tiles framed)",
                site_entity, site.tiles_total
            );
        }
    }
}
