use std::collections::HashMap;

use bevy::prelude::Entity;

use crate::room_detection::{RoomBoundaryLookup, RoomTileLookup};

#[derive(Default)]
pub(super) struct RoomLookupBuilder {
    tile_to_room: HashMap<(i32, i32), Entity>,
    boundary_to_rooms: HashMap<(i32, i32), Vec<Entity>>,
}

impl RoomLookupBuilder {
    pub(super) fn add(
        &mut self,
        room: Entity,
        floor_tiles: &[(i32, i32)],
        wall_tiles: &[(i32, i32)],
        door_tiles: &[(i32, i32)],
    ) {
        for &tile in floor_tiles {
            self.tile_to_room.insert(tile, room);
        }
        for &grid in wall_tiles.iter().chain(door_tiles) {
            let rooms = self.boundary_to_rooms.entry(grid).or_default();
            rooms.push(room);
            rooms.sort_unstable_by_key(|entity| entity.to_bits());
            rooms.dedup();
        }
    }

    pub(super) fn publish(
        self,
        room_tile_lookup: &mut RoomTileLookup,
        room_boundary_lookup: &mut RoomBoundaryLookup,
    ) {
        room_tile_lookup.replace(self.tile_to_room);
        room_boundary_lookup.boundary_to_rooms = self.boundary_to_rooms;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entity(index: u32) -> Entity {
        Entity::from_raw_u32(index).expect("test entity index is valid")
    }

    #[test]
    fn shared_boundaries_are_sorted_and_deduplicated() {
        let mut builder = RoomLookupBuilder::default();
        builder.add(entity(9), &[(1, 1)], &[(2, 1)], &[(3, 1)]);
        builder.add(entity(2), &[(1, 2)], &[(2, 1)], &[(2, 1)]);

        let mut tiles = RoomTileLookup::default();
        let mut boundaries = RoomBoundaryLookup::default();
        builder.publish(&mut tiles, &mut boundaries);

        assert_eq!(tiles.tile_to_room.get(&(1, 1)), Some(&entity(9)));
        assert_eq!(tiles.tile_to_room.get(&(1, 2)), Some(&entity(2)));
        let mut expected_rooms = vec![entity(2), entity(9)];
        expected_rooms.sort_unstable_by_key(|entity| entity.to_bits());
        assert_eq!(
            boundaries.boundary_to_rooms.get(&(2, 1)),
            Some(&expected_rooms)
        );
    }

    #[test]
    fn publishing_an_empty_builder_clears_both_lookups() {
        let mut tiles = RoomTileLookup::default();
        tiles.tile_to_room.insert((1, 1), entity(1));
        let mut boundaries = RoomBoundaryLookup::default();
        boundaries.boundary_to_rooms.insert((2, 1), vec![entity(1)]);

        RoomLookupBuilder::default().publish(&mut tiles, &mut boundaries);

        assert!(tiles.tile_to_room.is_empty());
        assert!(boundaries.boundary_to_rooms.is_empty());
    }
}
