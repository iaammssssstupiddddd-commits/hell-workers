use super::WorldMap;
use bevy::prelude::*;

impl WorldMap {
    /// Returns the completed Floor stacked beneath ordinary building occupancy.
    pub fn floor_entity(&self, grid: (i32, i32)) -> Option<Entity> {
        self.floors.get(&grid).copied()
    }

    pub fn set_floor(&mut self, grid: (i32, i32), entity: Entity) {
        self.floors.insert(grid, entity);
    }

    /// Clears a completed Floor only when its stackable layer still belongs to
    /// the expected owner. Floors do not alter walkability, so this does not
    /// touch the ordinary building or obstacle layers.
    pub fn clear_floor_if_owned(&mut self, grid: (i32, i32), entity: Entity) -> bool {
        if self.floors.get(&grid) != Some(&entity) {
            return false;
        }
        self.floors.remove(&grid);
        true
    }

    /// Replaces every completed-Floor owner during trusted load normalization.
    ///
    /// Callers validate that each grid is canonical before this infallible
    /// update, so no ordinary building layer is touched here.
    pub fn replace_floor_owners(&mut self, floors: Vec<((i32, i32), Entity)>) {
        self.floors.clear();
        self.floors.extend(floors);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn owner_safe_clear_preserves_a_replaced_floor_layer() {
        let mut map = WorldMap::default();
        let grid = (7, 9);
        let owner = Entity::from_bits(1);
        let replacement = Entity::from_bits(2);
        map.set_floor(grid, replacement);

        assert!(!map.clear_floor_if_owned(grid, owner));
        assert_eq!(map.floor_entity(grid), Some(replacement));
        assert!(map.clear_floor_if_owned(grid, replacement));
        assert_eq!(map.floor_entity(grid), None);
    }

    #[test]
    fn replacement_drops_stale_floor_owners_without_touching_buildings() {
        let mut map = WorldMap::default();
        let stale = Entity::from_bits(1);
        let first = Entity::from_bits(2);
        let second = Entity::from_bits(3);
        let building = Entity::from_bits(4);
        map.set_floor((1, 1), stale);
        map.set_building((2, 2), building);

        map.replace_floor_owners(vec![((3, 3), first), ((4, 4), second)]);

        assert_eq!(map.floor_entity((1, 1)), None);
        assert_eq!(map.floor_entity((3, 3)), Some(first));
        assert_eq!(map.floor_entity((4, 4)), Some(second));
        assert_eq!(map.building_entity((2, 2)), Some(building));
    }
}
