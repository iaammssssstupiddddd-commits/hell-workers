use bevy::prelude::Vec2;
use hw_core::constants::{MAP_HEIGHT, MAP_WIDTH, TILE_SIZE};

pub fn world_to_grid(pos: Vec2) -> (i32, i32) {
    let x = (pos.x / TILE_SIZE + (MAP_WIDTH as f32 - 1.0) / 2.0 + 0.5).floor() as i32;
    let y = (pos.y / TILE_SIZE + (MAP_HEIGHT as f32 - 1.0) / 2.0 + 0.5).floor() as i32;
    (x, y)
}

pub fn grid_to_world(x: i32, y: i32) -> Vec2 {
    Vec2::new(
        (x as f32 - (MAP_WIDTH as f32 - 1.0) / 2.0) * TILE_SIZE,
        (y as f32 - (MAP_HEIGHT as f32 - 1.0) / 2.0) * TILE_SIZE,
    )
}

pub fn snap_to_grid_center(pos: Vec2) -> Vec2 {
    let (x, y) = world_to_grid(pos);
    grid_to_world(x, y)
}

pub fn snap_to_grid_edge(pos: Vec2) -> Vec2 {
    let map_offset_x = (MAP_WIDTH as f32 * TILE_SIZE) / 2.0;
    let map_offset_y = (MAP_HEIGHT as f32 * TILE_SIZE) / 2.0;
    let local_x = pos.x + map_offset_x;
    let local_y = pos.y + map_offset_y;
    let snapped_local_x = (local_x / TILE_SIZE).round() * TILE_SIZE;
    let snapped_local_y = (local_y / TILE_SIZE).round() * TILE_SIZE;
    Vec2::new(
        snapped_local_x - map_offset_x,
        snapped_local_y - map_offset_y,
    )
}

pub fn idx_to_pos(idx: usize) -> (i32, i32) {
    let x = idx as i32 % MAP_WIDTH;
    let y = idx as i32 / MAP_WIDTH;
    (x, y)
}
