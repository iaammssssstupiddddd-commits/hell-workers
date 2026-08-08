use bevy::prelude::*;

use super::WorldMap;

/// Stable, exact-owner view of every WorldMap layer used by building cleanup.
///
/// Raw obstacles are intentionally absent: they are a derived bitmap and do
/// not carry an owner identity.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WorldMapOwnerSnapshot {
    pub building_grids: Vec<(i32, i32)>,
    pub floor_grids: Vec<(i32, i32)>,
    pub door_grids: Vec<(i32, i32)>,
    pub bridge_grids: Vec<(i32, i32)>,
    pub stockpile_grids: Vec<(i32, i32)>,
}

impl WorldMap {
    /// Collects only entries whose current owner exactly matches `owner`.
    ///
    /// Results are sorted by Y then X so callers never depend on HashMap order.
    pub fn snapshot_owner(&self, owner: Entity) -> WorldMapOwnerSnapshot {
        let mut snapshot = WorldMapOwnerSnapshot {
            building_grids: owned_grids(&self.buildings, owner),
            floor_grids: owned_grids(&self.floors, owner),
            door_grids: owned_grids(&self.doors, owner),
            bridge_grids: self
                .bridged_tiles
                .iter()
                .copied()
                .filter(|grid| self.buildings.get(grid) == Some(&owner))
                .collect(),
            stockpile_grids: owned_grids(&self.stockpiles, owner),
        };
        sort_grids(&mut snapshot.bridge_grids);
        snapshot
    }
}

fn owned_grids(
    entries: &std::collections::HashMap<(i32, i32), Entity>,
    owner: Entity,
) -> Vec<(i32, i32)> {
    let mut grids: Vec<_> = entries
        .iter()
        .filter_map(|(&grid, &current)| (current == owner).then_some(grid))
        .collect();
    sort_grids(&mut grids);
    grids
}

fn sort_grids(grids: &mut [(i32, i32)]) {
    grids.sort_unstable_by_key(|&(x, y)| (y, x));
}

#[cfg(test)]
mod tests {
    use super::*;
    use hw_core::world::DoorState;

    #[test]
    fn building_owner_snapshot_is_stable_and_never_mixes_another_owner() {
        let mut map = WorldMap::default();
        let owner = Entity::from_bits(1);
        let other = Entity::from_bits(2);

        map.set_building((8, 4), owner);
        map.set_floor((9, 4), owner);
        map.set_floor((10, 4), other);
        map.register_bridge_tile((2, 3), owner);
        map.register_bridge_tile((1, 3), other);
        map.register_door((5, 2), owner, DoorState::Closed);
        map.register_door((4, 2), other, DoorState::Closed);
        map.set_stockpile((7, 5), owner);
        map.set_stockpile((6, 5), other);

        assert_eq!(
            map.snapshot_owner(owner),
            WorldMapOwnerSnapshot {
                building_grids: vec![(5, 2), (2, 3), (8, 4)],
                floor_grids: vec![(9, 4)],
                door_grids: vec![(5, 2)],
                bridge_grids: vec![(2, 3)],
                stockpile_grids: vec![(7, 5)],
            }
        );
    }

    #[test]
    fn building_bridge_membership_requires_bridge_bit_and_matching_owner() {
        let mut map = WorldMap::default();
        let owner = Entity::from_bits(3);
        let other = Entity::from_bits(4);
        map.register_bridge_tile((9, 9), owner);
        map.set_building((9, 9), other);

        let snapshot = map.snapshot_owner(owner);
        assert!(snapshot.bridge_grids.is_empty());
        assert!(snapshot.building_grids.is_empty());
    }
}
