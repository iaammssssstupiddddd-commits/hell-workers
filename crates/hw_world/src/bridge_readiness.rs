//! TAK-14 readiness specification, deliberately test-only.
//!
//! These candidates are NOT accepted by the current all-River UI validator.
//! A later atomic preview/commit change must adopt the same terrain contract.
//! Neither a candidate nor map-layer tests prove construction, load, actor
//! presentation, native acceptance, or authorization to install Bridge art.

use crate::{TerrainType, WorldMap, generate_world_layout};
use bevy::prelude::Entity;
use hw_core::constants::{MAP_HEIGHT, MAP_WIDTH};
use hw_jobs::{BuildingAnchorBasis, BuildingType, building_shape};

const REPRO_SEED: u64 = 20260920;

#[derive(Debug, PartialEq, Eq)]
enum Rejection {
    OutOfBounds((i32, i32)),
    MissingRiver(i32),
    RiverGap((i32, i32)),
    SpanTooLong { min_y: i32, max_y: i32 },
    Building((i32, i32)),
    Stockpile((i32, i32)),
    RawObstacle((i32, i32)),
    BankNotDry((i32, i32)),
    NotWalkable((i32, i32)),
}

#[derive(Debug, PartialEq, Eq)]
struct Crossing {
    /// Lower-left footprint cell; shape order and center offsets stay unchanged.
    anchor: (i32, i32),
    river_intervals: [(i32, i32); 2],
    footprint: Vec<(i32, i32)>,
    /// South pair followed by north pair, immediately outside the footprint.
    banks: [(i32, i32); 4],
}

/// Read the live logical terrain, never fixed river constants, centerline,
/// rendered shoreline, or the worldgen snapshot after runtime has changed it.
/// Every column must have exactly one nonempty continuous River interval.
/// The union must fit the existing five-row deck. Spare rows are split with
/// floor(spare / 2) south and the remainder north; never search past blockers.
/// All deck and bank cells must be unoccupied and raw-obstacle-free. Dry deck
/// cells and both external banks must already be walkable. This proves local
/// cardinal bank-to-bank connectivity after completion, not worker access from
/// any particular Site/Yard or Soul spawn.
fn diagnose(map: &WorldMap, x: i32) -> Result<Crossing, Rejection> {
    if !(0..MAP_WIDTH - 1).contains(&x) {
        return Err(Rejection::OutOfBounds((x, 0)));
    }
    let shape = building_shape(BuildingType::Bridge);
    assert_eq!(shape.anchor_basis, BuildingAnchorBasis::RiverYMin);
    assert_eq!(shape.size_tiles, (2.0, 5.0));
    assert_eq!(shape.center_offset_tiles, (0.5, 2.0));
    let mut intervals = [(0, 0); 2];
    for (column, interval) in intervals.iter_mut().enumerate() {
        let gx = x + column as i32;
        let ys: Vec<_> = (0..MAP_HEIGHT)
            .filter(|&y| map.is_river_tile(gx, y))
            .collect();
        let Some(&min_y) = ys.first() else {
            return Err(Rejection::MissingRiver(gx));
        };
        let max_y = *ys.last().unwrap();
        for y in min_y..=max_y {
            if !map.is_river_tile(gx, y) {
                return Err(Rejection::RiverGap((gx, y)));
            }
        }
        *interval = (min_y, max_y);
    }
    let min_y = intervals[0].0.min(intervals[1].0);
    let max_y = intervals[0].1.max(intervals[1].1);
    let span = max_y - min_y + 1;
    if span > 5 {
        return Err(Rejection::SpanTooLong { min_y, max_y });
    }
    let anchor = (x, min_y - (5 - span) / 2);
    let footprint: Vec<_> = shape
        .ordered_relative_tiles
        .iter()
        .map(|&(dx, dy)| (x + dx, anchor.1 + dy))
        .collect();
    let banks = [
        (x, anchor.1 - 1),
        (x + 1, anchor.1 - 1),
        (x, anchor.1 + 5),
        (x + 1, anchor.1 + 5),
    ];
    for &grid in footprint.iter().chain(&banks) {
        if map.pos_to_idx(grid.0, grid.1).is_none() {
            return Err(Rejection::OutOfBounds(grid));
        }
        if map.has_building(grid) {
            return Err(Rejection::Building(grid));
        }
        if map.has_stockpile(grid) {
            return Err(Rejection::Stockpile(grid));
        }
        if map.has_raw_obstacle(grid.0, grid.1) {
            return Err(Rejection::RawObstacle(grid));
        }
        if banks.contains(&grid) && map.is_river_tile(grid.0, grid.1) {
            return Err(Rejection::BankNotDry(grid));
        }
        if !map.is_river_tile(grid.0, grid.1) && !map.is_walkable(grid.0, grid.1) {
            return Err(Rejection::NotWalkable(grid));
        }
    }
    Ok(Crossing {
        anchor,
        river_intervals: intervals,
        footprint,
        banks,
    })
}

fn generated_map() -> WorldMap {
    let layout = generate_world_layout(REPRO_SEED);
    let mut map = WorldMap::default();
    map.tiles.clone_from(&layout.terrain_tiles);
    // Mirror production's initial natural blockers; do not clear obstacles to
    // make a crossing pass. No terrain cells are rewritten in this fixture.
    for &(x, y) in layout
        .initial_tree_positions
        .iter()
        .chain(&layout.initial_rock_positions)
    {
        map.add_obstacle(x, y);
    }
    map
}

#[test]
fn bridge_readiness_generated_crossings_cover_widths_and_adjacent_owners() {
    let mut map = generated_map();
    let terrain = map.tiles.clone();
    let candidates: Vec<_> = (0..MAP_WIDTH - 1)
        .filter_map(|x| diagnose(&map, x).ok())
        .collect();
    assert!(!candidates.is_empty());
    for width in 2..=4 {
        let candidate = candidates
            .iter()
            .find(|c| {
                c.river_intervals
                    .iter()
                    .any(|&(lo, hi)| hi - lo + 1 == width)
            })
            .expect("generated fixture needs each production river width");
        eprintln!("BRIDGE_READINESS seed={REPRO_SEED} width={width} candidate={candidate:?}");
    }
    let (left, right) = candidates
        .iter()
        .find_map(|left| {
            candidates
                .iter()
                .find(|right| right.anchor.0 == left.anchor.0 + 2)
                .map(|right| (left, right))
        })
        .expect("generated fixture needs adjacent nonoverlapping Bridge candidates");
    eprintln!("BRIDGE_READINESS seed={REPRO_SEED} adjacent={left:?} / {right:?}");
    let blueprint = Entity::from_bits(1);
    let completed = Entity::from_bits(2);
    let neighbor = Entity::from_bits(3);
    let initial_walkability: Vec<_> = left
        .footprint
        .iter()
        .map(|&(x, y)| map.is_walkable(x, y))
        .collect();

    map.reserve_building_footprint(
        BuildingType::Bridge,
        blueprint,
        left.footprint.iter().copied(),
    );
    for (&grid, &walkable) in left.footprint.iter().zip(&initial_walkability) {
        assert_eq!(map.building_entity(grid), Some(blueprint));
        assert!(!map.bridged_tiles.contains(&grid));
        assert_eq!(map.is_walkable(grid.0, grid.1), walkable);
    }
    // The map calls used by cancellation restore the original mixed dry/River
    // topology without ever granting passage on an unfinished river cell.
    for &grid in &left.footprint {
        assert!(map.clear_building_occupancy_if_owned(grid, blueprint));
    }
    assert_eq!(diagnose(&map, left.anchor.0).unwrap(), *left);
    map.reserve_building_footprint(
        BuildingType::Bridge,
        blueprint,
        left.footprint.iter().copied(),
    );
    map.complete_owned_building_footprint(
        blueprint,
        completed,
        BuildingType::Bridge,
        &left.footprint,
        &Default::default(),
    )
    .unwrap();
    assert_eq!(diagnose(&map, right.anchor.0).unwrap(), *right);
    map.register_completed_building_footprint(
        BuildingType::Bridge,
        neighbor,
        right.footprint.iter().copied(),
    );
    // Current movement policy has a cardinal route through each lane, including
    // south end, center, north end and both banks. This is not a Soul run.
    for crossing in [left, right] {
        for x in crossing.anchor.0..=crossing.anchor.0 + 1 {
            for y in crossing.anchor.1 - 1..=crossing.anchor.1 + 5 {
                assert!(map.is_walkable(x, y), "blocked crossing at {x},{y}");
            }
        }
        let path = crate::pathfinding::find_path(
            &map,
            &mut crate::PathfindingContext::default(),
            crossing.banks[0],
            crossing.banks[2],
            crate::PathGoalPolicy::RespectGoalWalkability,
        )
        .expect("current A* policy must connect the diagnosed banks after completion");
        assert!(path.iter().all(|&(x, y)| map.is_walkable(x, y)));
    }
    for &grid in &left.footprint {
        assert!(!map.clear_bridge_if_owned(grid, blueprint));
        assert!(map.clear_bridge_if_owned(grid, completed));
    }
    for (&grid, &walkable) in left.footprint.iter().zip(&initial_walkability) {
        assert!(!map.bridged_tiles.contains(&grid));
        assert_eq!(map.is_walkable(grid.0, grid.1), walkable);
    }
    for &grid in &right.footprint {
        assert_eq!(map.building_entity(grid), Some(neighbor));
        assert!(map.bridged_tiles.contains(&grid));
    }
    assert_eq!(map.tiles, terrain);
}

/// Synthetic predicate cases only, never a measurement or production fixture.
fn synthetic_columns(left: (i32, i32), right: (i32, i32)) -> WorldMap {
    let mut map = WorldMap::default();
    for (x, (lo, hi)) in [(10, left), (11, right)] {
        for y in lo..=hi {
            let index = map.pos_to_idx(x, y).unwrap();
            map.tiles[index] = TerrainType::River;
        }
    }
    map
}

#[test]
fn bridge_readiness_span_gaps_missing_banks_and_bounds_fail_closed() {
    // Width 4 with one-row meander occupies the full five-row span.
    let map = synthetic_columns((20, 23), (21, 24));
    let crossing = diagnose(&map, 10).unwrap();
    assert_eq!(crossing.anchor, (10, 20));
    assert_eq!(crossing.banks, [(10, 19), (11, 19), (10, 25), (11, 25)]);
    assert_eq!(
        diagnose(&synthetic_columns((20, 23), (22, 25)), 10),
        Err(Rejection::SpanTooLong {
            min_y: 20,
            max_y: 25
        })
    );
    let mut gap = synthetic_columns((20, 23), (21, 24));
    let index = gap.pos_to_idx(10, 21).unwrap();
    gap.tiles[index] = TerrainType::Grass;
    assert_eq!(diagnose(&gap, 10), Err(Rejection::RiverGap((10, 21))));
    assert_eq!(diagnose(&map, 9), Err(Rejection::MissingRiver(9)));
    for x in [-1, MAP_WIDTH - 1, i32::MAX] {
        assert_eq!(diagnose(&map, x), Err(Rejection::OutOfBounds((x, 0))));
    }
    assert!(matches!(
        diagnose(&synthetic_columns((0, 3), (0, 3)), 10),
        Err(Rejection::OutOfBounds(_))
    ));
    assert!(matches!(
        diagnose(&synthetic_columns((96, 99), (96, 99)), 10),
        Err(Rejection::OutOfBounds(_))
    ));
}

#[test]
fn bridge_readiness_rejects_blockers_on_every_deck_and_bank_cell() {
    let crossing = diagnose(&synthetic_columns((20, 21), (20, 21)), 10).unwrap();
    assert_eq!(crossing.anchor, (10, 19));
    for &grid in crossing.footprint.iter().chain(&crossing.banks) {
        let mut map = synthetic_columns((20, 21), (20, 21));
        map.set_building(grid, Entity::from_bits(1));
        assert_eq!(diagnose(&map, 10), Err(Rejection::Building(grid)));
        map.clear_building(grid);
        map.set_stockpile(grid, Entity::from_bits(2));
        assert_eq!(diagnose(&map, 10), Err(Rejection::Stockpile(grid)));
        map.clear_stockpile(grid);
        map.add_grid_obstacle(grid);
        assert_eq!(diagnose(&map, 10), Err(Rejection::RawObstacle(grid)));
    }
}
