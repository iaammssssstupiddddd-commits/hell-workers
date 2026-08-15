use bevy::prelude::*;
use hw_core::GridPos;
use hw_core::constants::{ROOM_DETECTION_COOLDOWN_SECS, ROOM_VALIDATION_INTERVAL_SECS};
use std::collections::{HashMap, HashSet};

use super::core::RoomBounds;

// ---------------------------------------------------------------------------
// ECS Components & Resources
// ---------------------------------------------------------------------------
//
// These types are owned by hw_world because their semantics belong to the
// world domain. Root systems (bevy_app) drive the detection pipeline and
// update these components/resources; they re-export these types for
// convenience.

/// ECS component attached to room entities. Populated by `hw_world` room systems.
#[derive(Component, Debug, Clone)]
pub struct Room {
    pub tiles: Vec<(i32, i32)>,
    pub wall_tiles: Vec<(i32, i32)>,
    pub door_tiles: Vec<(i32, i32)>,
    pub bounds: RoomBounds,
    pub tile_count: usize,
}

/// Marker component for border-line sprites spawned along a room's inner wall edge.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct RoomOverlayTile {
    pub grid_pos: GridPos,
}

/// Reverse lookup from floor tile grid position to the owning room entity.
#[derive(Resource, Default, Debug)]
pub struct RoomTileLookup {
    pub tile_to_room: HashMap<(i32, i32), Entity>,
    mask_signature: RoomMaskSignature,
}

/// Entity-independent identity for the currently published indoor mask.
///
/// Room entities are recreated during detection, so only canonical tile
/// membership may advance the lighting-facing revision.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct RoomMaskSignature {
    revision: u64,
    canonical_tiles: Vec<GridPos>,
}

impl RoomMaskSignature {
    pub const fn revision(&self) -> u64 {
        self.revision
    }

    pub fn canonical_tiles(&self) -> &[GridPos] {
        &self.canonical_tiles
    }
}

impl RoomTileLookup {
    pub const fn mask_signature(&self) -> &RoomMaskSignature {
        &self.mask_signature
    }

    /// Publishes a new reverse lookup while advancing the semantic mask
    /// revision only when canonical tile membership changes.
    pub fn replace(&mut self, tile_to_room: HashMap<GridPos, Entity>) -> bool {
        let mut canonical_tiles = tile_to_room.keys().copied().collect::<Vec<_>>();
        canonical_tiles.sort_unstable_by_key(|&(x, y)| (y, x));
        canonical_tiles.dedup();
        let changed = canonical_tiles != self.mask_signature.canonical_tiles;
        if changed {
            self.mask_signature.revision = self
                .mask_signature
                .revision
                .checked_add(1)
                .expect("Room mask revision overflow");
            self.mask_signature.canonical_tiles = canonical_tiles;
        }
        self.tile_to_room = tile_to_room;
        changed
    }
}

/// Reverse lookup from a wall or door grid position to every adjacent room.
///
/// A shared boundary may belong to two rooms, so values are sorted,
/// deduplicated entity lists rather than a single entity.
#[derive(Resource, Default, Debug)]
pub struct RoomBoundaryLookup {
    pub boundary_to_rooms: HashMap<(i32, i32), Vec<Entity>>,
}

impl RoomBoundaryLookup {
    pub fn rooms_at(&self, grid: (i32, i32)) -> &[Entity] {
        self.boundary_to_rooms
            .get(&grid)
            .map(Vec::as_slice)
            .unwrap_or_default()
    }
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

#[cfg(test)]
mod mask_signature_tests {
    use super::*;

    #[test]
    fn entity_replacement_does_not_advance_room_mask_revision() {
        let mut lookup = RoomTileLookup::default();
        let mut first = HashMap::new();
        first.insert((4, 7), Entity::from_bits(1));
        assert!(lookup.replace(first));
        let revision = lookup.mask_signature().revision();

        let mut recreated = HashMap::new();
        recreated.insert((4, 7), Entity::from_bits(2));
        assert!(!lookup.replace(recreated));
        assert_eq!(lookup.mask_signature().revision(), revision);

        let mut changed = HashMap::new();
        changed.insert((4, 7), Entity::from_bits(3));
        changed.insert((5, 7), Entity::from_bits(4));
        assert!(lookup.replace(changed));
        assert_eq!(lookup.mask_signature().revision(), revision + 1);
        assert_eq!(lookup.mask_signature().canonical_tiles(), &[(4, 7), (5, 7)]);
    }
}
