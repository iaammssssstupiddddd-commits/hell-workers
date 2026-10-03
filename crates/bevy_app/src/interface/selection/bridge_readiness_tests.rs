//! Actual generated terrain through the shared production placement resolver.
use super::{building_geometry, live_building_geometry};
use crate::world::map::WorldMapRef;
use hw_core::constants::MAP_WIDTH;
use hw_jobs::BuildingType;
use hw_ui::selection::{
    BuildingPlacementContext, resolve_bridge_crossing, validate_building_placement,
};
use hw_world::{WorldMap, generate_world_layout};

#[test]
fn bridge_generated_terrain_uses_same_live_geometry_for_preview_and_commit() {
    let layout = generate_world_layout(20260920);
    let mut map = WorldMap::default();
    map.tiles.clone_from(&layout.terrain_tiles);
    for &(x, y) in layout
        .initial_tree_positions
        .iter()
        .chain(&layout.initial_rock_positions)
    {
        map.add_obstacle(x, y);
    }
    let terrain = map.tiles.clone();
    let mut count = 0;
    for x in 0..MAP_WIDTH - 1 {
        let read = WorldMapRef(&map);
        let Ok(crossing) = resolve_bridge_crossing(&read, (x, 0)) else {
            continue;
        };
        count += 1;
        let ctx = BuildingPlacementContext {
            world: &read,
            in_site: true,
            in_yard: true,
            is_wall_or_door_at: &|_| false,
            is_replaceable_wall_at: &|_| false,
        };
        for y in [0, 50, 99] {
            let preview = live_building_geometry(&map, BuildingType::Bridge, (x, y));
            let commit = building_geometry(BuildingType::Bridge, (x, y), crossing.anchor.1);
            assert_eq!(preview.occupied_grids, crossing.occupied_grids);
            assert_eq!(preview.draw_pos, commit.draw_pos);
            assert_eq!(preview.size, commit.size);
            assert!(
                validate_building_placement(&ctx, BuildingType::Bridge, (x, y), &preview).can_place
            );
        }
    }
    assert!(count > 0);
    assert_eq!(map.tiles, terrain);
}

#[test]
fn bridge_generated_crossing_reservation_cancel_completion_and_owned_removal() {
    use bevy::prelude::Entity;
    let layout = generate_world_layout(20260920);
    let mut map = WorldMap::default();
    map.tiles.clone_from(&layout.terrain_tiles);
    for &(x, y) in layout
        .initial_tree_positions
        .iter()
        .chain(&layout.initial_rock_positions)
    {
        map.add_obstacle(x, y);
    }
    let candidates: Vec<_> = (0..MAP_WIDTH - 1)
        .filter_map(|x| resolve_bridge_crossing(&WorldMapRef(&map), (x, 0)).ok())
        .collect();
    let (left, right) = candidates
        .iter()
        .find_map(|a| {
            candidates
                .iter()
                .find(|b| b.anchor.0 == a.anchor.0 + 2)
                .map(|b| (a, b))
        })
        .unwrap();
    let terrain = map.tiles.clone();
    let initial: Vec<_> = left
        .occupied_grids
        .iter()
        .map(|&(x, y)| map.is_walkable(x, y))
        .collect();
    let blueprint = Entity::from_bits(1);
    let complete = Entity::from_bits(2);
    let neighbor = Entity::from_bits(3);
    map.reserve_building_footprint(
        BuildingType::Bridge,
        blueprint,
        left.occupied_grids.iter().copied(),
    );
    for (&grid, &walkable) in left.occupied_grids.iter().zip(&initial) {
        assert!(!map.bridged_tiles.contains(&grid));
        assert_eq!(map.is_walkable(grid.0, grid.1), walkable);
        assert!(map.clear_building_occupancy_if_owned(grid, blueprint));
    }
    assert_eq!(
        resolve_bridge_crossing(&WorldMapRef(&map), left.anchor).unwrap(),
        *left
    );
    map.reserve_building_footprint(
        BuildingType::Bridge,
        blueprint,
        left.occupied_grids.iter().copied(),
    );
    map.complete_owned_building_footprint(
        blueprint,
        complete,
        BuildingType::Bridge,
        &left.occupied_grids,
        &Default::default(),
    )
    .unwrap();
    assert_eq!(
        resolve_bridge_crossing(&WorldMapRef(&map), right.anchor).unwrap(),
        *right
    );
    map.register_completed_building_footprint(
        BuildingType::Bridge,
        neighbor,
        right.occupied_grids.iter().copied(),
    );
    for crossing in [left, right] {
        for &grid in crossing.occupied_grids.iter().chain(&crossing.banks) {
            assert!(map.is_walkable(grid.0, grid.1));
        }
    }
    for (&grid, &walkable) in left.occupied_grids.iter().zip(&initial) {
        assert!(!map.clear_bridge_if_owned(grid, blueprint));
        assert!(map.clear_bridge_if_owned(grid, complete));
        assert_eq!(map.is_walkable(grid.0, grid.1), walkable);
    }
    for &grid in &right.occupied_grids {
        assert_eq!(map.building_entity(grid), Some(neighbor));
        assert!(map.bridged_tiles.contains(&grid));
    }
    assert_eq!(map.tiles, terrain);
    assert!(!BuildingType::Bridge.is_player_movable());
}
