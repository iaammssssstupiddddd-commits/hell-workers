//! Cleanup owned by the room domain when the persisted world is replaced.

use bevy::prelude::*;

use crate::{
    Room, RoomBoundaryLookup, RoomDetectionState, RoomOverlayTile, RoomTileLookup,
    RoomValidationState,
};

/// Removes runtime-only room entities and drops every entity reference held by
/// room scheduling resources. The operation is intentionally idempotent so a
/// rollback or recovery-only retry can run it again on a partial world.
pub fn reset_for_world_replace(world: &mut World) {
    let runtime_entities: Vec<Entity> = {
        let mut query = world.query_filtered::<Entity, Or<(With<Room>, With<RoomOverlayTile>)>>();
        query.iter(world).collect()
    };
    for entity in runtime_entities {
        if let Ok(entity_mut) = world.get_entity_mut(entity) {
            entity_mut.despawn();
        }
    }
    world.insert_resource(RoomDetectionState::default());
    world.insert_resource(RoomTileLookup::default());
    world.insert_resource(RoomBoundaryLookup::default());
    world.insert_resource(RoomValidationState::default());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::RoomBounds;

    #[test]
    fn reset_removes_room_roots_overlays_and_entity_lookup_idempotently() {
        let mut world = World::new();
        let room = world
            .spawn(Room {
                tiles: vec![(1, 1)],
                wall_tiles: Vec::new(),
                door_tiles: Vec::new(),
                bounds: RoomBounds {
                    min_x: 1,
                    max_x: 1,
                    min_y: 1,
                    max_y: 1,
                },
                tile_count: 1,
            })
            .id();
        let overlay = world.spawn(RoomOverlayTile { grid_pos: (1, 1) }).id();
        let durable = world.spawn_empty().id();
        let mut lookup = RoomTileLookup::default();
        lookup.tile_to_room.insert((1, 1), room);
        world.insert_resource(lookup);
        let mut boundary_lookup = RoomBoundaryLookup::default();
        boundary_lookup.boundary_to_rooms.insert((0, 1), vec![room]);
        world.insert_resource(boundary_lookup);

        reset_for_world_replace(&mut world);
        reset_for_world_replace(&mut world);

        assert!(world.get_entity(room).is_err());
        assert!(world.get_entity(overlay).is_err());
        assert!(world.get_entity(durable).is_ok());
        assert!(world.resource::<RoomTileLookup>().tile_to_room.is_empty());
        assert!(
            world
                .resource::<RoomBoundaryLookup>()
                .boundary_to_rooms
                .is_empty()
        );
    }
}
