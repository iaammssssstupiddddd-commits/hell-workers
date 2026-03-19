use hw_core::constants::{MAP_HEIGHT, MAP_WIDTH, ROOM_MAX_TILES};
use hw_jobs::BuildingType;
use std::collections::{HashSet, VecDeque};

const CARDINAL_OFFSETS: [(i32, i32); 4] = [(0, 1), (1, 0), (0, -1), (-1, 0)];

// ---------------------------------------------------------------------------
// Public data types
// ---------------------------------------------------------------------------

/// Grid bounds of a room, derived entirely from floor tile positions.
#[derive(bevy::prelude::Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct RoomBounds {
    pub min_x: i32,
    pub min_y: i32,
    pub max_x: i32,
    pub max_y: i32,
}

impl RoomBounds {
    pub fn from_tile(tile: (i32, i32)) -> Self {
        Self {
            min_x: tile.0,
            min_y: tile.1,
            max_x: tile.0,
            max_y: tile.1,
        }
    }

    pub fn include(&mut self, tile: (i32, i32)) {
        self.min_x = self.min_x.min(tile.0);
        self.min_y = self.min_y.min(tile.1);
        self.max_x = self.max_x.max(tile.0);
        self.max_y = self.max_y.max(tile.1);
    }
}

/// Descriptor produced by the root adapter from a single building entity.
///
/// The root collects these from `Query<(Entity, &Building, &Transform)>` and
/// passes the resulting slice to [`build_detection_input`].
pub struct RoomDetectionBuildingTile {
    pub grid: (i32, i32),
    pub kind: BuildingType,
    pub is_provisional: bool,
    /// `true` when another building occupies the same grid cell (i.e.,
    /// `world_map.has_building(grid)` returned `true` for a Floor tile).
    /// Used to exclude floor tiles that are shadowed by a wall or other
    /// structure placed on top.
    pub has_building_on_top: bool,
}

/// Pre-classified tile sets fed into room detection.
#[derive(Default)]
pub struct RoomDetectionInput {
    pub floor_tiles: HashSet<(i32, i32)>,
    pub solid_wall_tiles: HashSet<(i32, i32)>,
    pub door_tiles: HashSet<(i32, i32)>,
}

/// A room candidate produced by a successful flood-fill.
///
/// All fields contain sorted, deduplicated tile lists so that equality
/// comparisons are deterministic.
#[derive(Debug, Clone)]
pub struct DetectedRoom {
    pub tiles: Vec<(i32, i32)>,
    pub wall_tiles: Vec<(i32, i32)>,
    pub door_tiles: Vec<(i32, i32)>,
    pub bounds: RoomBounds,
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Classifies a slice of building tile descriptors into the three tile sets
/// required by room detection.
pub fn build_detection_input(tiles: &[RoomDetectionBuildingTile]) -> RoomDetectionInput {
    let mut input = RoomDetectionInput::default();

    for tile in tiles {
        match tile.kind {
            BuildingType::Floor => {
                // Completed floor tiles are not registered in world_map.buildings.
                // If another building (e.g. a wall) occupies the same cell,
                // exclude it from floor_tiles and let the wall side handle it.
                if !tile.has_building_on_top {
                    input.floor_tiles.insert(tile.grid);
                }
            }
            BuildingType::Wall if !tile.is_provisional => {
                input.solid_wall_tiles.insert(tile.grid);
            }
            BuildingType::Door => {
                input.door_tiles.insert(tile.grid);
            }
            _ => {}
        }
    }

    input
}

/// Runs flood-fill over all floor tiles and returns every valid room.
pub fn detect_rooms(input: &RoomDetectionInput) -> Vec<DetectedRoom> {
    let mut unvisited_floors = input.floor_tiles.clone();
    let mut rooms = Vec::new();

    while let Some(seed) = unvisited_floors.iter().next().copied() {
        if let Some(room) = flood_fill_room(seed, input, &mut unvisited_floors) {
            rooms.push(room);
        }
    }

    rooms
}

/// Returns `true` when every tile in `tiles` is a valid enclosed floor tile
/// with at least one door contact, according to `input`.
///
/// Only `tiles` (the floor tile list) is needed; wall/door membership is
/// derived from `input` itself.
pub fn room_is_valid_against_input(tiles: &[(i32, i32)], input: &RoomDetectionInput) -> bool {
    if tiles.is_empty() || tiles.len() > ROOM_MAX_TILES {
        return false;
    }

    let tile_set: HashSet<(i32, i32)> = tiles.iter().copied().collect();
    if tile_set.len() != tiles.len() {
        return false;
    }

    let mut door_contacts = 0usize;

    for &tile in tiles {
        if !input.floor_tiles.contains(&tile) {
            return false;
        }

        for neighbor in cardinal_neighbors(tile) {
            if !is_in_map_bounds(neighbor) {
                return false;
            }

            if tile_set.contains(&neighbor) {
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

// ---------------------------------------------------------------------------
// Private helpers
// ---------------------------------------------------------------------------

fn flood_fill_room(
    seed: (i32, i32),
    input: &RoomDetectionInput,
    unvisited_floors: &mut HashSet<(i32, i32)>,
) -> Option<DetectedRoom> {
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

    Some(DetectedRoom {
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
