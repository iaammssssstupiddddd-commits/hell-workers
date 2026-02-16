//! Floor construction completion system

use super::components::*;
use crate::assets::GameAssets;
use crate::constants::{FLOOR_CURING_DURATION_SECS, TILE_SIZE, Z_MAP};
use crate::entities::damned_soul::{DamnedSoul, Path};
use crate::systems::jobs::{Building, BuildingType, ObstaclePosition};
use crate::systems::utils::animations::{BounceAnimation, BounceAnimationConfig};
use crate::systems::visual::blueprint::{BOUNCE_DURATION, BuildingBounceEffect};
use crate::world::map::WorldMap;
use bevy::prelude::*;
use std::collections::HashSet;

fn evacuate_souls_from_blocked_tiles(
    q_souls: &mut Query<(Entity, &mut Transform, &mut Path), With<DamnedSoul>>,
    blocked_tiles: &HashSet<(i32, i32)>,
    world_map: &WorldMap,
) -> usize {
    let mut evacuated = 0usize;
    for (soul_entity, mut soul_transform, mut path) in q_souls.iter_mut() {
        let soul_pos = soul_transform.translation.truncate();
        let soul_grid = WorldMap::world_to_grid(soul_pos);
        if !blocked_tiles.contains(&soul_grid) {
            continue;
        }

        if let Some((target_gx, target_gy)) = world_map.get_nearest_walkable_grid(soul_pos) {
            let target_pos = WorldMap::grid_to_world(target_gx, target_gy);
            soul_transform.translation.x = target_pos.x;
            soul_transform.translation.y = target_pos.y;
            path.waypoints.clear();
            path.current_index = 0;
            evacuated += 1;
        } else {
            warn!(
                "FLOOR_CURING: Soul {:?} could not find evacuation tile from {:?}",
                soul_entity, soul_grid
            );
        }
    }

    evacuated
}

/// Handles floor construction completion
pub fn floor_construction_completion_system(
    time: Res<Time>,
    mut q_sites: Query<(Entity, &mut FloorConstructionSite)>,
    q_tiles: Query<(Entity, &FloorTileBlueprint)>,
    mut q_souls: Query<(Entity, &mut Transform, &mut Path), With<DamnedSoul>>,
    game_assets: Res<GameAssets>,
    mut world_map: ResMut<WorldMap>,
    mut commands: Commands,
) {
    for (site_entity, mut site) in q_sites.iter_mut() {
        let site_tiles: Vec<(Entity, (i32, i32), FloorTileState)> = q_tiles
            .iter()
            .filter(|(_, tile)| tile.parent_site == site_entity)
            .map(|(tile_entity, tile)| (tile_entity, tile.grid_pos, tile.state))
            .collect();

        if site_tiles.is_empty() {
            continue;
        }

        // Check if all tiles complete
        let all_complete = site_tiles
            .iter()
            .all(|(_, _, state)| *state == FloorTileState::Complete);

        if !all_complete {
            continue;
        }

        // Enter curing phase once all pouring is done.
        if site.phase != FloorConstructionPhase::Curing {
            site.phase = FloorConstructionPhase::Curing;
            site.curing_remaining_secs = FLOOR_CURING_DURATION_SECS.max(0.0);

            let blocked_tiles: HashSet<(i32, i32)> =
                site_tiles.iter().map(|(_, grid_pos, _)| *grid_pos).collect();

            for (tile_entity, (gx, gy), _) in &site_tiles {
                world_map.add_obstacle(*gx, *gy);
                commands
                    .entity(*tile_entity)
                    .insert(ObstaclePosition(*gx, *gy));
            }

            let evacuated =
                evacuate_souls_from_blocked_tiles(&mut q_souls, &blocked_tiles, &world_map);

            info!(
                "Floor site {:?} entered curing ({:.1}s, evacuated {} souls)",
                site_entity, site.curing_remaining_secs, evacuated
            );
            continue;
        }

        let blocked_tiles: HashSet<(i32, i32)> =
            site_tiles.iter().map(|(_, grid_pos, _)| *grid_pos).collect();
        let re_evacuated =
            evacuate_souls_from_blocked_tiles(&mut q_souls, &blocked_tiles, &world_map);
        if re_evacuated > 0 {
            debug!(
                "FLOOR_CURING: Site {:?} re-evacuated {} souls still inside curing area",
                site_entity, re_evacuated
            );
        }

        site.curing_remaining_secs = (site.curing_remaining_secs - time.delta_secs()).max(0.0);
        if site.curing_remaining_secs > 0.0 {
            continue;
        }

        // For each tile: spawn Building entity with Floor type
        let mut tile_count = 0;
        for (tile_entity, (gx, gy), _) in site_tiles {
            let world_pos = WorldMap::grid_to_world(gx, gy);

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
                BuildingBounceEffect {
                    bounce_animation: BounceAnimation {
                        timer: 0.0,
                        config: BounceAnimationConfig {
                            duration: BOUNCE_DURATION,
                            min_scale: 1.0,
                            max_scale: 1.2,
                        },
                    },
                },
                Transform::from_translation(world_pos.extend(Z_MAP + 0.01)),
                Visibility::default(),
                Name::new("Building (Floor)"),
            ));

            // Curing is complete: tile becomes walkable again.
            world_map.remove_obstacle(gx, gy);

            // Despawn tile blueprint
            commands.entity(tile_entity).despawn();
            tile_count += 1;
        }

        // Despawn site
        commands.entity(site_entity).despawn();

        info!(
            "Floor site {:?} completed after curing ({} tiles, total {}/{})",
            site_entity, tile_count, site.tiles_poured, site.tiles_total
        );
    }
}
