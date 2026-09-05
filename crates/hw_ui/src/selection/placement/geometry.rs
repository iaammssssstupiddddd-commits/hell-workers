use bevy::prelude::*;
use hw_core::constants::TILE_SIZE;

use super::PlacementGeometry;

/// Builds the UI-only two-tile BucketStorage visual from a root-projected
/// anchor. Occupied grids remain a root/world concern.
pub fn bucket_storage_geometry(anchor_world: Vec2) -> PlacementGeometry {
    PlacementGeometry {
        occupied_grids: Vec::new(),
        draw_pos: anchor_world + Vec2::new(TILE_SIZE * 0.5, 0.0),
        size: Vec2::new(TILE_SIZE * 2.0, TILE_SIZE),
    }
}

pub fn grid_is_nearby(base: (i32, i32), target: (i32, i32), tiles: i32) -> bool {
    (target.0 - base.0).abs() <= tiles && (target.1 - base.1).abs() <= tiles
}
