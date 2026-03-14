//! Room detection core: pure input construction, flood-fill, and validation.
//!
//! This module contains no ECS system logic.
//! ECS system logic (query adapter layer) is in [`crate::room_systems`].

use bevy::prelude::*;
use hw_core::constants::{MAP_HEIGHT, MAP_WIDTH, ROOM_MAX_TILES};
use hw_jobs::BuildingType;
use std::collections::{HashSet, VecDeque};

const CARDINAL_OFFSETS: [(i32, i32); 4] = [(0, 1), (1, 0), (0, -1), (-1, 0)];

// ---------------------------------------------------------------------------
// Public data types
// ---------------------------------------------------------------------------

/// Grid bounds of a room, derived entirely from floor tile positions.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
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

// ---------------------------------------------------------------------------
// ECS Components & Resources
// ---------------------------------------------------------------------------
//
// These types are owned by hw_world because their semantics belong to the
// world domain. Root systems (bevy_app) drive the detection pipeline and
// update these components/resources; they re-export these types for
// convenience.

use hw_core::constants::{ROOM_DETECTION_COOLDOWN_SECS, ROOM_VALIDATION_INTERVAL_SECS};
use std::collections::HashMap;

/// ECS component attached to room entities. Populated by the root detection system.
#[derive(Component, Debug, Clone)]
pub struct Room {
    pub tiles: Vec<(i32, i32)>,
    pub wall_tiles: Vec<(i32, i32)>,
    pub door_tiles: Vec<(i32, i32)>,
    pub bounds: RoomBounds,
    pub tile_count: usize,
}

/// Marker component for visual overlay tiles spawned per room floor tile.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct RoomOverlayTile {
    pub grid_pos: (i32, i32),
}

/// Reverse lookup from floor tile grid position to the owning room entity.
#[derive(Resource, Default, Debug)]
pub struct RoomTileLookup {
    pub tile_to_room: HashMap<(i32, i32), Entity>,
}

/// Runtime state for room detection scheduling and dirty-tile tracking.
#[derive(Resource)]
pub struct RoomDetectionState {
    pub dirty_tiles: HashSet<(i32, i32)>,
    pub cooldown: Timer,
}

impl Default for RoomDetectionState {
    fn default() -> Self {
        Self {
            dirty_tiles: HashSet::new(),
            cooldown: Timer::from_seconds(ROOM_DETECTION_COOLDOWN_SECS, TimerMode::Repeating),
        }
    }
}

impl RoomDetectionState {
    /// Marks a tile dirty and includes the 1-tile neighborhood for boundary updates.
    pub fn mark_dirty(&mut self, tile: (i32, i32)) {
        for dx in -1..=1 {
            for dy in -1..=1 {
                self.dirty_tiles.insert((tile.0 + dx, tile.1 + dy));
            }
        }
    }

    pub fn mark_dirty_many<I>(&mut self, tiles: I)
    where
        I: IntoIterator<Item = (i32, i32)>,
    {
        for tile in tiles {
            self.mark_dirty(tile);
        }
    }
}

/// Timer state for periodic room validation.
#[derive(Resource)]
pub struct RoomValidationState {
    pub timer: Timer,
}

impl Default for RoomValidationState {
    fn default() -> Self {
        Self {
            timer: Timer::from_seconds(ROOM_VALIDATION_INTERVAL_SECS, TimerMode::Repeating),
        }
    }
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: build tiles for a small rectangular room enclosed by walls with
    /// one door.
    ///
    /// Layout (3×3 floor at x=1..=3, y=1..=3, surrounding wall, door at (1,4)):
    /// ```text
    ///  W D W W W   y = 4  — (1,4) is door, adjacent to floor (1,3)
    ///  W F F F W   y = 3
    ///  W F F F W   y = 2
    ///  W F F F W   y = 1
    ///  W W W W W   y = 0
    ///  0 1 2 3 4   ← x
    /// ```
    ///
    /// All coordinates are >= 0 so they pass `is_in_map_bounds`.
    /// In the game each building entity occupies exactly one grid cell, so a
    /// door and a wall never share the same cell.
    fn closed_room_tiles() -> Vec<RoomDetectionBuildingTile> {
        const DOOR_POS: (i32, i32) = (1, 4);
        let mut tiles = Vec::new();

        // 3×3 floor at x=1..=3, y=1..=3
        for x in 1..=3 {
            for y in 1..=3 {
                tiles.push(RoomDetectionBuildingTile {
                    grid: (x, y),
                    kind: BuildingType::Floor,
                    is_provisional: false,
                    has_building_on_top: false,
                });
            }
        }

        // Surrounding walls — skip the door position
        let wall_positions: Vec<(i32, i32)> = {
            let mut v = Vec::new();
            // bottom row (y = 0) and top row (y = 4)
            for x in 0..=4 {
                v.push((x, 0));
                v.push((x, 4));
            }
            // left column (x = 0) and right column (x = 4)
            for y in 0..=4i32 {
                v.push((0, y));
                v.push((4, y));
            }
            v.sort_unstable();
            v.dedup();
            v
        };

        for pos in wall_positions {
            if pos == DOOR_POS {
                continue; // door replaces wall here
            }
            tiles.push(RoomDetectionBuildingTile {
                grid: pos,
                kind: BuildingType::Wall,
                is_provisional: false,
                has_building_on_top: false,
            });
        }

        // One door — replaces the wall at DOOR_POS and is adjacent to floor (1,3)
        tiles.push(RoomDetectionBuildingTile {
            grid: DOOR_POS,
            kind: BuildingType::Door,
            is_provisional: false,
            has_building_on_top: false,
        });

        tiles
    }

    #[test]
    fn test_closed_room_with_door() {
        let tiles = closed_room_tiles();
        let input = build_detection_input(&tiles);
        let rooms = detect_rooms(&input);
        assert_eq!(rooms.len(), 1);
        assert_eq!(rooms[0].tiles.len(), 9);
        assert!(!rooms[0].door_tiles.is_empty());
    }

    #[test]
    fn test_open_region_is_not_a_room() {
        // Same as closed room but missing the right wall column
        let mut tiles = closed_room_tiles();
        tiles.retain(|t| !(t.kind == BuildingType::Wall && t.grid.0 == 4));
        let input = build_detection_input(&tiles);
        let rooms = detect_rooms(&input);
        assert_eq!(rooms.len(), 0);
    }

    #[test]
    fn test_no_door_is_not_a_room() {
        let mut tiles = closed_room_tiles();
        tiles.retain(|t| t.kind != BuildingType::Door);
        let input = build_detection_input(&tiles);
        let rooms = detect_rooms(&input);
        assert_eq!(rooms.len(), 0);
    }

    #[test]
    fn test_provisional_wall_not_solid() {
        let mut tiles = closed_room_tiles();
        // Make all walls provisional
        for t in &mut tiles {
            if t.kind == BuildingType::Wall {
                t.is_provisional = true;
            }
        }
        let input = build_detection_input(&tiles);
        assert!(input.solid_wall_tiles.is_empty());
        // Without solid walls the flood fill leaves via open edges → no room
        let rooms = detect_rooms(&input);
        assert_eq!(rooms.len(), 0);
    }

    #[test]
    fn test_room_max_tiles_exceeded() {
        // Build a huge open floor area larger than ROOM_MAX_TILES
        let mut tiles = Vec::new();
        let side = (ROOM_MAX_TILES as i32).isqrt() + 2;

        for x in 1..=side {
            for y in 1..=side {
                tiles.push(RoomDetectionBuildingTile {
                    grid: (x, y),
                    kind: BuildingType::Floor,
                    is_provisional: false,
                    has_building_on_top: false,
                });
            }
        }
        // Surround with walls
        for x in 0..=(side + 1) {
            for &y in &[0i32, side + 1] {
                tiles.push(RoomDetectionBuildingTile {
                    grid: (x, y),
                    kind: BuildingType::Wall,
                    is_provisional: false,
                    has_building_on_top: false,
                });
            }
        }
        for y in 0..=(side + 1) {
            for &x in &[0i32, side + 1] {
                tiles.push(RoomDetectionBuildingTile {
                    grid: (x, y),
                    kind: BuildingType::Wall,
                    is_provisional: false,
                    has_building_on_top: false,
                });
            }
        }
        tiles.push(RoomDetectionBuildingTile {
            grid: (1, side + 1),
            kind: BuildingType::Door,
            is_provisional: false,
            has_building_on_top: false,
        });

        let input = build_detection_input(&tiles);
        let rooms = detect_rooms(&input);
        assert_eq!(rooms.len(), 0, "over-sized area must not become a room");
    }

    #[test]
    fn test_map_boundary_contact_is_not_a_room() {
        // Floor tile at (0,0) touches map boundary (x < 0 is out of bounds)
        let mut tiles = Vec::new();
        tiles.push(RoomDetectionBuildingTile {
            grid: (0, 0),
            kind: BuildingType::Floor,
            is_provisional: false,
            has_building_on_top: false,
        });
        // walls on 3 sides
        for &g in &[(0i32, -1i32), (0, 1), (1, 0), (-1, 0)] {
            if g.0 >= 0 && g.1 >= 0 {
                tiles.push(RoomDetectionBuildingTile {
                    grid: g,
                    kind: BuildingType::Wall,
                    is_provisional: false,
                    has_building_on_top: false,
                });
            }
        }
        tiles.push(RoomDetectionBuildingTile {
            grid: (0, 1),
            kind: BuildingType::Door,
            is_provisional: false,
            has_building_on_top: false,
        });
        let input = build_detection_input(&tiles);
        let rooms = detect_rooms(&input);
        assert_eq!(rooms.len(), 0, "room touching map boundary must not be valid");
    }

    #[test]
    fn test_floor_blocked_by_building_excluded() {
        // A floor tile with has_building_on_top=true must not appear in floor_tiles
        let tiles = vec![RoomDetectionBuildingTile {
            grid: (5, 5),
            kind: BuildingType::Floor,
            is_provisional: false,
            has_building_on_top: true,
        }];
        let input = build_detection_input(&tiles);
        assert!(!input.floor_tiles.contains(&(5, 5)));
    }

    #[test]
    fn test_valid_room_passes_validator() {
        let tiles = closed_room_tiles();
        let input = build_detection_input(&tiles);
        let rooms = detect_rooms(&input);
        assert_eq!(rooms.len(), 1);
        assert!(room_is_valid_against_input(&rooms[0].tiles, &input));
    }

    #[test]
    fn test_invalid_room_fails_validator() {
        let input = RoomDetectionInput::default();
        // tiles not in floor_tiles → invalid
        let fake_tiles = vec![(1, 1), (2, 1)];
        assert!(!room_is_valid_against_input(&fake_tiles, &input));
    }
}
