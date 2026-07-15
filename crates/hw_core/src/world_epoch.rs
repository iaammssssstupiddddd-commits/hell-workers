//! World replacement epoch shared by systems that retain entity ids locally.
//!
//! A save/load replacement invalidates every simulation `Entity` id. Resources
//! can be reset by the root load coordinator, while system-local state cannot
//! be reached directly. `EpochLocal` gives those systems one neutral contract:
//! clear their local value before its next use when the world epoch changes.

use bevy::prelude::*;

/// Monotonic generation of the currently active simulation world.
#[derive(Resource, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct WorldEpoch(u64);

impl WorldEpoch {
    /// Returns the current world generation.
    pub const fn get(self) -> u64 {
        self.0
    }

    /// Advances after the old persistent world has been removed and before a
    /// replacement DynamicWorld is inserted.
    pub fn advance(&mut self) {
        self.0 = self.0.wrapping_add(1);
    }
}

/// Local state that is reset lazily when [`WorldEpoch`] changes.
///
/// This is intended for system-local values that retain `Entity` ids across
/// frames. Scratch buffers that are cleared before every use do not need it.
#[derive(Debug)]
pub struct EpochLocal<T> {
    epoch: u64,
    value: T,
}

impl<T: Default> Default for EpochLocal<T> {
    fn default() -> Self {
        Self {
            epoch: WorldEpoch::default().get(),
            value: T::default(),
        }
    }
}

impl<T: Default> EpochLocal<T> {
    /// Returns the local value after resetting it when `world_epoch` changed.
    pub fn get_mut(&mut self, world_epoch: WorldEpoch) -> &mut T {
        if self.epoch != world_epoch.get() {
            self.epoch = world_epoch.get();
            self.value = T::default();
        }
        &mut self.value
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn epoch_local_resets_only_after_a_world_replacement() {
        let mut epoch = WorldEpoch::default();
        let mut local = EpochLocal::<Vec<u32>>::default();

        local.get_mut(epoch).push(1);
        assert_eq!(local.get_mut(epoch).as_slice(), [1]);

        epoch.advance();
        assert!(local.get_mut(epoch).is_empty());
    }
}
