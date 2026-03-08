use bevy::prelude::Vec2;

use crate::coords::{idx_to_pos, world_to_grid};
use crate::pathfinding::PathWorld;
use crate::terrain::TerrainType;

pub fn find_nearest_walkable_grid(
    world: &impl PathWorld,
    pos: Vec2,
    max_radius: i32,
) -> Option<(i32, i32)> {
    let grid = world_to_grid(pos);
    if world.is_walkable(grid.0, grid.1) {
        return Some(grid);
    }

    for radius in 1..=max_radius {
        for dx in -radius..=radius {
            for dy in -radius..=radius {
                let test = (grid.0 + dx, grid.1 + dy);
                if world.is_walkable(test.0, test.1) {
                    return Some(test);
                }
            }
        }
    }

    None
}

pub fn find_nearest_river_grid(pos: Vec2, tiles: &[TerrainType]) -> Option<(i32, i32)> {
    let from = world_to_grid(pos);
    let mut nearest = None;
    let mut nearest_dist_sq = i64::MAX;

    for (idx, terrain) in tiles.iter().enumerate() {
        if *terrain != TerrainType::River {
            continue;
        }

        let (x, y) = idx_to_pos(idx);
        let dx = (x - from.0) as i64;
        let dy = (y - from.1) as i64;
        let dist_sq = dx * dx + dy * dy;
        if dist_sq < nearest_dist_sq {
            nearest_dist_sq = dist_sq;
            nearest = Some((x, y));
        }
    }

    nearest
}
