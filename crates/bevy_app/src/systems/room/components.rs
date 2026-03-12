use bevy::prelude::*;
pub use hw_world::room_detection::RoomBounds;
#[derive(Component, Debug, Clone)]
pub struct Room {
    pub tiles: Vec<(i32, i32)>,
    pub wall_tiles: Vec<(i32, i32)>,
    pub door_tiles: Vec<(i32, i32)>,
    pub bounds: RoomBounds,
    pub tile_count: usize,
}

/// Marker for visual overlay tiles spawned per room floor tile.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct RoomOverlayTile {
    pub grid_pos: (i32, i32),
}
