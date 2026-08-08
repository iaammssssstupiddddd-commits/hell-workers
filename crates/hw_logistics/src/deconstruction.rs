//! Pure placement planning for resources recovered during deconstruction.

use bevy::prelude::*;
use hw_core::constants::{MAP_HEIGHT, MAP_WIDTH, TILE_SIZE};
use hw_world::WorldMap;

const ITEMS_PER_RECOVERY_CELL: usize = 9;

/// Final, prevalidated world positions for one deconstruction transaction.
#[derive(Debug, Clone, PartialEq)]
pub struct RecoveryPlacementPlan {
    pub item_positions: Vec<Vec2>,
    pub carrier_positions: Vec<Vec2>,
}

/// Finds deterministic post-teardown positions outside the building footprint.
///
/// Existing items receive explicit final coordinates instead of a shared
/// center that a later spawn helper may offset into a blocked or river tile.
/// Carriers use distinct cells.  Up to nine ordinary items share one safe cell
/// using bounded offsets that remain inside that cell.
pub fn build_recovery_placement_plan(
    map: &WorldMap,
    anchor: (i32, i32),
    target_footprint: &[(i32, i32)],
    removed_stockpile_owners: &[Entity],
    item_count: usize,
    carrier_count: usize,
) -> Option<RecoveryPlacementPlan> {
    let item_cell_count = item_count.div_ceil(ITEMS_PER_RECOVERY_CELL);
    let required_cells = carrier_count.checked_add(item_cell_count)?;
    if required_cells == 0 {
        return Some(RecoveryPlacementPlan {
            item_positions: Vec::new(),
            carrier_positions: Vec::new(),
        });
    }

    let mut candidates = Vec::new();
    for y in 0..MAP_HEIGHT {
        for x in 0..MAP_WIDTH {
            let grid = (x, y);
            if target_footprint.contains(&grid)
                || !map.is_walkable(x, y)
                || map.building_entity(grid).is_some()
                || map
                    .stockpile_entity(grid)
                    .is_some_and(|owner| !removed_stockpile_owners.contains(&owner))
            {
                continue;
            }
            candidates.push((x.abs_diff(anchor.0) + y.abs_diff(anchor.1), y, x));
        }
    }
    candidates.sort_unstable();
    if candidates.len() < required_cells {
        return None;
    }

    let carrier_positions = candidates
        .iter()
        .take(carrier_count)
        .map(|&(_, y, x)| WorldMap::grid_to_world(x, y))
        .collect::<Vec<_>>();
    let item_cells = candidates
        .iter()
        .skip(carrier_count)
        .take(item_cell_count)
        .map(|&(_, y, x)| WorldMap::grid_to_world(x, y))
        .collect::<Vec<_>>();
    let mut item_positions = Vec::with_capacity(item_count);
    for index in 0..item_count {
        let center = item_cells[index / ITEMS_PER_RECOVERY_CELL];
        let slot = index % ITEMS_PER_RECOVERY_CELL;
        let column = (slot % 3) as f32 - 1.0;
        let row = (slot / 3) as f32 - 1.0;
        item_positions.push(center + Vec2::new(column, row) * (TILE_SIZE * 0.22));
    }

    Some(RecoveryPlacementPlan {
        item_positions,
        carrier_positions,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use hw_world::TerrainType;

    #[test]
    fn recovery_positions_are_stable_outside_footprint_and_inside_safe_cells() {
        let mut map = WorldMap::default();
        let target = Entity::from_bits(10);
        let companion = Entity::from_bits(11);
        map.set_building_occupancy((5, 5), target);
        map.set_stockpile((5, 4), companion);
        map.set_building_occupancy((4, 5), Entity::from_bits(12));

        let plan = build_recovery_placement_plan(&map, (5, 5), &[(5, 5)], &[companion], 10, 2)
            .expect("nearby safe cells");

        assert_eq!(plan.carrier_positions.len(), 2);
        assert_eq!(plan.item_positions.len(), 10);
        let carrier_grids = plan
            .carrier_positions
            .iter()
            .map(|position| WorldMap::world_to_grid(*position))
            .collect::<Vec<_>>();
        assert_eq!(carrier_grids, vec![(5, 4), (6, 5)]);
        assert!(carrier_grids.iter().all(|grid| *grid != (5, 5)));
        assert!(plan.item_positions.iter().all(|position| {
            let grid = WorldMap::world_to_grid(*position);
            grid != (5, 5) && map.is_walkable(grid.0, grid.1)
        }));
    }

    #[test]
    fn insufficient_post_teardown_space_fails_without_fallback() {
        let mut map = WorldMap::default();
        for index in 0..map.tiles.len() {
            map.tiles[index] = TerrainType::River;
        }
        let safe_idx = map.pos_to_idx(2, 2).unwrap();
        map.tiles[safe_idx] = TerrainType::Grass;

        assert!(build_recovery_placement_plan(&map, (2, 2), &[(2, 2)], &[], 1, 0).is_none());
    }
}
