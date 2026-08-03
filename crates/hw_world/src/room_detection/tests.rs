use hw_core::constants::ROOM_MAX_TILES;
use hw_jobs::{BuildingCategory, BuildingType, RoomDetectionRole};

use super::*;

fn building_tile(grid: (i32, i32), role: RoomDetectionRole) -> RoomDetectionBuildingTile {
    RoomDetectionBuildingTile { grid, role }
}

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
            tiles.push(building_tile((x, y), RoomDetectionRole::Floor));
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
        tiles.push(building_tile(pos, RoomDetectionRole::SolidBoundary));
    }

    // One door — replaces the wall at DOOR_POS and is adjacent to floor (1,3)
    tiles.push(building_tile(DOOR_POS, RoomDetectionRole::DoorBoundary));

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
    tiles.retain(|tile| !(tile.role == RoomDetectionRole::SolidBoundary && tile.grid.0 == 4));
    let input = build_detection_input(&tiles);
    let rooms = detect_rooms(&input);
    assert_eq!(rooms.len(), 0);
}

#[test]
fn test_no_door_is_not_a_room() {
    let mut tiles = closed_room_tiles();
    tiles.retain(|tile| tile.role != RoomDetectionRole::DoorBoundary);
    let input = build_detection_input(&tiles);
    let rooms = detect_rooms(&input);
    assert_eq!(rooms.len(), 0);
}

#[test]
fn test_provisional_wall_not_solid() {
    let mut tiles = closed_room_tiles();
    // Make all walls provisional
    for t in &mut tiles {
        if t.role == RoomDetectionRole::SolidBoundary {
            t.role = RoomDetectionRole::FloorInvalidator;
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
            tiles.push(building_tile((x, y), RoomDetectionRole::Floor));
        }
    }
    // Surround with walls
    for x in 0..=(side + 1) {
        for &y in &[0i32, side + 1] {
            tiles.push(building_tile((x, y), RoomDetectionRole::SolidBoundary));
        }
    }
    for y in 0..=(side + 1) {
        for &x in &[0i32, side + 1] {
            tiles.push(building_tile((x, y), RoomDetectionRole::SolidBoundary));
        }
    }
    tiles.push(building_tile(
        (1, side + 1),
        RoomDetectionRole::DoorBoundary,
    ));

    let input = build_detection_input(&tiles);
    let rooms = detect_rooms(&input);
    assert_eq!(rooms.len(), 0, "over-sized area must not become a room");
}

#[test]
fn test_map_boundary_contact_is_not_a_room() {
    // Floor tile at (0,0) touches map boundary (x < 0 is out of bounds)
    let mut tiles = Vec::new();
    tiles.push(building_tile((0, 0), RoomDetectionRole::Floor));
    // walls on 3 sides
    for &g in &[(0i32, -1i32), (0, 1), (1, 0), (-1, 0)] {
        if g.0 >= 0 && g.1 >= 0 {
            tiles.push(building_tile(g, RoomDetectionRole::SolidBoundary));
        }
    }
    tiles.push(building_tile((0, 1), RoomDetectionRole::DoorBoundary));
    let input = build_detection_input(&tiles);
    let rooms = detect_rooms(&input);
    assert_eq!(
        rooms.len(),
        0,
        "room touching map boundary must not be valid"
    );
}

#[test]
fn test_floor_blocked_by_floor_invalidator_is_excluded() {
    let tiles = vec![
        building_tile((5, 5), RoomDetectionRole::Floor),
        building_tile((5, 5), RoomDetectionRole::FloorInvalidator),
    ];
    let input = build_detection_input(&tiles);
    assert!(!input.floor_tiles.contains(&(5, 5)));
}

#[test]
fn test_interior_fixture_keeps_completed_floor_in_room() {
    let mut tiles = closed_room_tiles();
    tiles.push(building_tile((2, 2), RoomDetectionRole::InteriorFixture));

    let input = build_detection_input(&tiles);
    let rooms = detect_rooms(&input);

    assert_eq!(rooms.len(), 1);
    assert!(rooms[0].tiles.contains(&(2, 2)));
}

#[test]
fn test_existing_plant_and_temporary_buildings_are_interior_fixtures() {
    let fixture_kinds: Vec<BuildingType> = BuildingType::ALL
        .into_iter()
        .filter(|kind| {
            matches!(
                kind.category(),
                BuildingCategory::Plant | BuildingCategory::Temporary
            )
        })
        .collect();

    assert!(!fixture_kinds.is_empty());
    for kind in fixture_kinds {
        let tiles = vec![
            building_tile((5, 5), RoomDetectionRole::Floor),
            building_tile((5, 5), kind.room_detection_role(false)),
        ];
        assert!(
            build_detection_input(&tiles).floor_tiles.contains(&(5, 5)),
            "{kind:?} must preserve its completed floor"
        );
    }
}

#[test]
fn test_generic_solid_boundary_closes_room_like_wall() {
    let tiles = closed_room_tiles();
    let input = build_detection_input(&tiles);

    assert!(input.solid_wall_tiles.contains(&(0, 0)));
    assert_eq!(detect_rooms(&input).len(), 1);
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
