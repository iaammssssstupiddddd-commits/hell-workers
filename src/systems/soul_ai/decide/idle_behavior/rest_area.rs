//! 休憩所: 容量判定・候補探索・隣接歩行点計算

use std::collections::HashMap;

use bevy::prelude::*;

use crate::constants::TILE_SIZE;
use crate::relationships::{RestAreaOccupants, RestAreaReservations};
use crate::systems::jobs::RestArea;
use crate::world::map::WorldMap;

pub const REST_AREA_ARRIVAL_RADIUS: f32 = TILE_SIZE;

pub fn rest_area_has_capacity(
    rest_area_entity: Entity,
    rest_area: &RestArea,
    occupants: Option<&RestAreaOccupants>,
    reservations: Option<&RestAreaReservations>,
    pending_reservations: &HashMap<Entity, usize>,
) -> bool {
    let occupant_count = occupants.map_or(0, RestAreaOccupants::len);
    let reserved_count = reservations.map_or(0, RestAreaReservations::len);
    let pending_count = pending_reservations
        .get(&rest_area_entity)
        .copied()
        .unwrap_or(0);

    occupant_count + reserved_count + pending_count < rest_area.capacity
}

pub fn find_nearest_available_rest_area(
    pos: Vec2,
    q_rest_areas: &Query<(
        Entity,
        &Transform,
        &RestArea,
        Option<&RestAreaOccupants>,
        Option<&RestAreaReservations>,
    )>,
    pending_reservations: &HashMap<Entity, usize>,
) -> Option<(Entity, Vec2)> {
    q_rest_areas
        .iter()
        .filter(|(rest_area_entity, _, rest_area, occupants, reservations)| {
            rest_area_has_capacity(
                *rest_area_entity,
                rest_area,
                *occupants,
                *reservations,
                pending_reservations,
            )
        })
        .min_by(|a, b| {
            a.1.translation
                .truncate()
                .distance_squared(pos)
                .partial_cmp(&b.1.translation.truncate().distance_squared(pos))
                .unwrap()
        })
        .map(|(entity, transform, _, _, _)| (entity, transform.translation.truncate()))
}

fn rest_area_occupied_grids_from_center(center: Vec2) -> [(i32, i32); 4] {
    let top_right = WorldMap::world_to_grid(center);
    [
        (top_right.0 - 1, top_right.1 - 1),
        (top_right.0, top_right.1 - 1),
        (top_right.0 - 1, top_right.1),
        (top_right.0, top_right.1),
    ]
}

/// 休憩所に最も近い歩行可能な隣接タイルを返す
pub fn nearest_walkable_adjacent_to_rest_area(
    soul_pos: Vec2,
    rest_area_center: Vec2,
    world_map: &WorldMap,
) -> Vec2 {
    let occupied = rest_area_occupied_grids_from_center(rest_area_center);
    let mut best_pos = rest_area_center;
    let mut best_dist = f32::MAX;
    let directions: [(i32, i32); 8] = [
        (0, 1),
        (0, -1),
        (1, 0),
        (-1, 0),
        (1, 1),
        (1, -1),
        (-1, 1),
        (-1, -1),
    ];
    for &(gx, gy) in &occupied {
        for &(dx, dy) in &directions {
            let (nx, ny) = (gx + dx, gy + dy);
            if occupied.contains(&(nx, ny)) || !world_map.is_walkable(nx, ny) {
                continue;
            }
            let pos = WorldMap::grid_to_world(nx, ny);
            let dist = soul_pos.distance_squared(pos);
            if dist < best_dist {
                best_pos = pos;
                best_dist = dist;
            }
        }
    }
    best_pos
}

pub fn has_arrived_at_rest_area(current_pos: Vec2, rest_area_center: Vec2) -> bool {
    if current_pos.distance(rest_area_center) <= REST_AREA_ARRIVAL_RADIUS {
        return true;
    }
    let current_grid = WorldMap::world_to_grid(current_pos);
    let occupied = rest_area_occupied_grids_from_center(rest_area_center);
    occupied.iter().any(|&(gx, gy)| {
        let dx = (current_grid.0 - gx).abs();
        let dy = (current_grid.1 - gy).abs();
        dx <= 1 && dy <= 1 && !(dx == 0 && dy == 0)
    })
}
