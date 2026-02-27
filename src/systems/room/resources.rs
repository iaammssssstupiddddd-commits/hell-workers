use crate::constants::{ROOM_DETECTION_COOLDOWN_SECS, ROOM_VALIDATION_INTERVAL_SECS};
use bevy::prelude::*;
use std::collections::{HashMap, HashSet};

/// Runtime state for room detection scheduling and dirty tracking.
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
    /// Marks a tile dirty and includes 1-tile neighborhood for boundary updates.
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

/// Reverse lookup from floor tile to room entity.
#[derive(Resource, Default, Debug)]
pub struct RoomTileLookup {
    pub tile_to_room: HashMap<(i32, i32), Entity>,
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
