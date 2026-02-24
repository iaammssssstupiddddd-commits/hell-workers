use super::components::{Room, RoomBounds};
use super::resources::{RoomDetectionState, RoomTileLookup};
use crate::constants::{MAP_HEIGHT, MAP_WIDTH, ROOM_MAX_TILES};
use crate::systems::jobs::{Building, BuildingType};
use crate::world::map::WorldMap;
use bevy::prelude::*;
use std::collections::{HashMap, HashSet, VecDeque};

const CARDINAL_OFFSETS: [(i32, i32); 4] = [(0, 1), (1, 0), (0, -1), (-1, 0)];

#[derive(Default)]
pub(super) struct RoomDetectionInput {
    pub floor_tiles: HashSet<(i32, i32)>,
    pub solid_wall_tiles: HashSet<(i32, i32)>,
    pub door_tiles: HashSet<(i32, i32)>,
}

#[derive(Debug)]
struct RoomCandidate {
    tiles: Vec<(i32, i32)>,
    wall_tiles: Vec<(i32, i32)>,
    door_tiles: Vec<(i32, i32)>,
    bounds: RoomBounds,
}

pub fn detect_rooms_system(
    mut commands: Commands,
    time: Res<Time>,
    world_map: Res<WorldMap>,
    mut detection_state: ResMut<RoomDetectionState>,
    mut room_tile_lookup: ResMut<RoomTileLookup>,
    q_buildings: Query<(Entity, &Building, &Transform)>,
    q_rooms: Query<Entity, With<Room>>,
) {
    detection_state.cooldown.tick(time.delta());

    if detection_state.dirty_tiles.is_empty() || !detection_state.cooldown.just_finished() {
        return;
    }

    let input = build_detection_input(&q_buildings, &world_map);
    let detected_rooms = detect_rooms(&input);

    for room_entity in q_rooms.iter() {
        commands.entity(room_entity).try_despawn();
    }

    let mut tile_to_room = HashMap::new();
    for (index, candidate) in detected_rooms.into_iter().enumerate() {
        let RoomCandidate {
            tiles,
            wall_tiles,
            door_tiles,
            bounds,
        } = candidate;
        let tile_count = tiles.len();
        let room_tiles_for_lookup = tiles.clone();

        let room_entity = commands
            .spawn((
                Room {
                    tiles,
                    wall_tiles,
                    door_tiles,
                    bounds,
                    tile_count,
                },
                bounds,
                Name::new(format!("Room #{}", index + 1)),
            ))
            .id();

        for tile in room_tiles_for_lookup {
            tile_to_room.insert(tile, room_entity);
        }
    }

    room_tile_lookup.tile_to_room = tile_to_room;
    detection_state.dirty_tiles.clear();
}

pub(super) fn build_detection_input(
    q_buildings: &Query<(Entity, &Building, &Transform)>,
    world_map: &WorldMap,
) -> RoomDetectionInput {
    let mut input = RoomDetectionInput::default();

    for (_entity, building, transform) in q_buildings.iter() {
        let grid = WorldMap::world_to_grid(transform.translation.truncate());

        match building.kind {
            BuildingType::Floor => {
                // FloorConstruction 由来の床は WorldMap.buildings に入らないため、
                // occupancy がないタイルのみ「有効な床」として扱う。
                if !world_map.buildings.contains_key(&grid) {
                    input.floor_tiles.insert(grid);
                }
            }
            BuildingType::Wall if !building.is_provisional => {
                input.solid_wall_tiles.insert(grid);
            }
            BuildingType::Door => {
                input.door_tiles.insert(grid);
            }
            _ => {}
        }
    }

    input
}

pub(super) fn room_is_valid_against_input(room: &Room, input: &RoomDetectionInput) -> bool {
    if room.tiles.is_empty() || room.tiles.len() > ROOM_MAX_TILES {
        return false;
    }

    let room_tile_set: HashSet<(i32, i32)> = room.tiles.iter().copied().collect();
    if room_tile_set.len() != room.tiles.len() {
        return false;
    }

    let mut door_contacts = 0usize;

    for &tile in &room.tiles {
        if !input.floor_tiles.contains(&tile) {
            return false;
        }

        for neighbor in cardinal_neighbors(tile) {
            if !is_in_map_bounds(neighbor) {
                return false;
            }

            if room_tile_set.contains(&neighbor) {
                continue;
            }

            if input.door_tiles.contains(&neighbor) {
                door_contacts += 1;
                continue;
            }

            if input.solid_wall_tiles.contains(&neighbor) {
                continue;
            }

            return false;
        }
    }

    door_contacts > 0
}

fn detect_rooms(input: &RoomDetectionInput) -> Vec<RoomCandidate> {
    let mut unvisited_floors = input.floor_tiles.clone();
    let mut rooms = Vec::new();

    while let Some(seed) = unvisited_floors.iter().next().copied() {
        if let Some(candidate) = flood_fill_room(seed, input, &mut unvisited_floors) {
            rooms.push(candidate);
        }
    }

    rooms
}

fn flood_fill_room(
    seed: (i32, i32),
    input: &RoomDetectionInput,
    unvisited_floors: &mut HashSet<(i32, i32)>,
) -> Option<RoomCandidate> {
    if !unvisited_floors.remove(&seed) {
        return None;
    }

    let mut queue = VecDeque::from([seed]);
    let mut tiles = Vec::new();
    let mut bounds = RoomBounds::from_tile(seed);
    let mut boundary_walls = HashSet::new();
    let mut boundary_doors = HashSet::new();
    let mut is_valid = true;

    while let Some(tile) = queue.pop_front() {
        tiles.push(tile);
        bounds.include(tile);

        if tiles.len() > ROOM_MAX_TILES {
            is_valid = false;
        }

        for neighbor in cardinal_neighbors(tile) {
            if !is_in_map_bounds(neighbor) {
                is_valid = false;
                continue;
            }

            if input.floor_tiles.contains(&neighbor) {
                if unvisited_floors.remove(&neighbor) {
                    queue.push_back(neighbor);
                }
                continue;
            }

            if input.solid_wall_tiles.contains(&neighbor) {
                boundary_walls.insert(neighbor);
                continue;
            }

            if input.door_tiles.contains(&neighbor) {
                boundary_doors.insert(neighbor);
                continue;
            }

            is_valid = false;
        }
    }

    if !is_valid || boundary_doors.is_empty() {
        return None;
    }

    tiles.sort_unstable();

    let mut wall_tiles: Vec<(i32, i32)> = boundary_walls.into_iter().collect();
    wall_tiles.sort_unstable();

    let mut door_tiles: Vec<(i32, i32)> = boundary_doors.into_iter().collect();
    door_tiles.sort_unstable();

    Some(RoomCandidate {
        tiles,
        wall_tiles,
        door_tiles,
        bounds,
    })
}

fn cardinal_neighbors(tile: (i32, i32)) -> [(i32, i32); 4] {
    CARDINAL_OFFSETS.map(|(dx, dy)| (tile.0 + dx, tile.1 + dy))
}

fn is_in_map_bounds(tile: (i32, i32)) -> bool {
    tile.0 >= 0 && tile.0 < MAP_WIDTH && tile.1 >= 0 && tile.1 < MAP_HEIGHT
}
