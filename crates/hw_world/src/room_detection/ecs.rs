use bevy::prelude::*;
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

/// ECS component attached to room entities. Populated by the root detection system.
#[derive(Component, Debug, Clone)]
pub struct Room {
    pub tiles: Vec<(i32, i32)>,
    pub wall_tiles: Vec<(i32, i32)>,
    pub door_tiles: Vec<(i32, i32)>,
    pub bounds: RoomBounds,
    pub tile_count: usize,
}

/// Marker component for visual overlay tiles spawned per room floor tile.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct RoomOverlayTile {
    pub grid_pos: (i32, i32),
}

/// Reverse lookup from floor tile grid position to the owning room entity.
#[derive(Resource, Default, Debug)]
pub struct RoomTileLookup {
    pub tile_to_room: HashMap<(i32, i32), Entity>,
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
