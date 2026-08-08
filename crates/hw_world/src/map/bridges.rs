use super::WorldMap;
use bevy::prelude::*;

impl WorldMap {
    pub fn add_bridged_tile(&mut self, grid: (i32, i32)) {
        let was_walkable = self.is_walkable(grid.0, grid.1);
        if self.bridged_tiles.insert(grid) && was_walkable != self.is_walkable(grid.0, grid.1) {
            self.bump_obstacle_version();
        }
    }

    pub fn register_bridge_tile(&mut self, grid: (i32, i32), entity: Entity) {
        self.add_bridged_tile(grid);
        self.set_building(grid, entity);
    }

    /// Removes one completed bridge tile only when both its logical building
    /// owner and bridge bit still describe the same owner-owned tile.
    pub fn clear_bridge_if_owned(&mut self, grid: (i32, i32), entity: Entity) -> bool {
        if self.buildings.get(&grid) != Some(&entity) || !self.bridged_tiles.contains(&grid) {
            return false;
        }

        let was_walkable = self.is_walkable(grid.0, grid.1);
        self.buildings.remove(&grid);
        self.bridged_tiles.remove(&grid);
        if was_walkable != self.is_walkable(grid.0, grid.1) {
            self.bump_obstacle_version();
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TerrainType;

    #[test]
    fn owner_safe_clear_preserves_replaced_bridge_layers() {
        let mut map = WorldMap::default();
        let grid = (8, 9);
        let index = map.pos_to_idx(grid.0, grid.1).unwrap();
        map.tiles[index] = TerrainType::River;
        let owner = Entity::from_bits(1);
        let replacement = Entity::from_bits(2);
        map.register_bridge_tile(grid, owner);
        map.set_building(grid, replacement);

        assert!(!map.clear_bridge_if_owned(grid, owner));
        assert_eq!(map.building_entity(grid), Some(replacement));
        assert!(map.bridged_tiles.contains(&grid));

        map.set_building(grid, owner);
        let version = map.obstacle_version;
        assert!(map.clear_bridge_if_owned(grid, owner));
        assert_eq!(map.building_entity(grid), None);
        assert!(!map.bridged_tiles.contains(&grid));
        assert_eq!(map.obstacle_version, version + 1);
        assert!(!map.is_walkable(grid.0, grid.1));
    }

    #[test]
    fn clearing_a_grass_bridge_does_not_change_topology_generation() {
        let mut map = WorldMap::default();
        let grid = (10, 11);
        let owner = Entity::from_bits(3);
        map.register_bridge_tile(grid, owner);
        let version = map.obstacle_version;

        assert!(map.clear_bridge_if_owned(grid, owner));

        assert!(map.is_walkable(grid.0, grid.1));
        assert_eq!(map.obstacle_version, version);
        assert_eq!(map.building_entity(grid), None);
        assert!(!map.bridged_tiles.contains(&grid));
    }
}
