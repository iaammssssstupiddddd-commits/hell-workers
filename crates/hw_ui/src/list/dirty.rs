//! Dirty flags for entity list view-model synchronization.

use bevy::prelude::*;
use std::time::Duration;

/// Value-only row updates are intentionally coalesced. Structure changes are
/// still immediate so search, drag/drop, and membership edits never wait for
/// this cadence.
pub const ENTITY_LIST_VALUE_SYNC_INTERVAL: Duration = Duration::from_millis(100);

#[derive(Resource)]
pub struct EntityListDirty {
    structure_dirty: bool,
    value_dirty: bool,
    value_gate_open: bool,
    value_timer: Timer,
}

impl Default for EntityListDirty {
    fn default() -> Self {
        Self {
            structure_dirty: false,
            value_dirty: false,
            // First value change can render immediately. Subsequent changes
            // are coalesced by the real-time timer below.
            value_gate_open: true,
            value_timer: Timer::new(ENTITY_LIST_VALUE_SYNC_INTERVAL, TimerMode::Repeating),
        }
    }
}

impl EntityListDirty {
    pub fn mark_structure(&mut self) {
        self.structure_dirty = true;
    }

    pub fn mark_values(&mut self) {
        self.value_dirty = true;
    }

    pub fn clear_all(&mut self) {
        self.structure_dirty = false;
        self.value_dirty = false;
        self.value_gate_open = false;
    }

    pub fn clear_values(&mut self) {
        self.value_dirty = false;
        self.value_gate_open = false;
    }

    /// Advances the 10 Hz value-only latch from `Time<Real>`. Real time keeps
    /// the list responsive while gameplay's virtual clock is paused.
    pub fn advance_value_gate(&mut self, delta: Duration) {
        if self.value_timer.tick(delta).just_finished() {
            self.value_gate_open = true;
        }
    }

    pub fn needs_structure_sync(&self) -> bool {
        self.structure_dirty
    }

    pub fn needs_value_sync_only(&self) -> bool {
        self.value_dirty && !self.structure_dirty && self.value_gate_open
    }
}
