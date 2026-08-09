//! Neutral save-slot identifiers shared by root save ownership and UI intents.
//!
//! Path resolution, revision inspection, and filesystem I/O stay outside this
//! crate. Only the closed slot ID set and player-safe labels are exported.

use bevy::prelude::Reflect;

/// Closed set of save slots. Raw numeric / string constructors are intentionally
/// unavailable so out-of-range IDs cannot enter requests.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect)]
pub enum SaveSlotId {
    Manual1,
    Manual2,
    Manual3,
    Autosave1,
    Autosave2,
    Autosave3,
    Autosave4,
    Autosave5,
    LegacyDefault,
}

/// Role of a slot for capability derivation. Kept separate from content health.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect)]
pub enum SaveSlotRole {
    Manual,
    Autosave,
    LegacyDefault,
}

impl SaveSlotId {
    /// Fixed iteration order used by catalog scans. Manual → Autosave → Legacy.
    pub const ALL: [Self; 9] = [
        Self::Manual1,
        Self::Manual2,
        Self::Manual3,
        Self::Autosave1,
        Self::Autosave2,
        Self::Autosave3,
        Self::Autosave4,
        Self::Autosave5,
        Self::LegacyDefault,
    ];

    pub const MANUAL: [Self; 3] = [Self::Manual1, Self::Manual2, Self::Manual3];

    pub const AUTOSAVE: [Self; 5] = [
        Self::Autosave1,
        Self::Autosave2,
        Self::Autosave3,
        Self::Autosave4,
        Self::Autosave5,
    ];

    pub const fn role(self) -> SaveSlotRole {
        match self {
            Self::Manual1 | Self::Manual2 | Self::Manual3 => SaveSlotRole::Manual,
            Self::Autosave1
            | Self::Autosave2
            | Self::Autosave3
            | Self::Autosave4
            | Self::Autosave5 => SaveSlotRole::Autosave,
            Self::LegacyDefault => SaveSlotRole::LegacyDefault,
        }
    }

    /// Canonical filename under the storage root. Never an absolute path.
    pub const fn canonical_file_name(self) -> &'static str {
        match self {
            Self::Manual1 => "manual-1.scn.ron",
            Self::Manual2 => "manual-2.scn.ron",
            Self::Manual3 => "manual-3.scn.ron",
            Self::Autosave1 => "autosave-1.scn.ron",
            Self::Autosave2 => "autosave-2.scn.ron",
            Self::Autosave3 => "autosave-3.scn.ron",
            Self::Autosave4 => "autosave-4.scn.ron",
            Self::Autosave5 => "autosave-5.scn.ron",
            Self::LegacyDefault => "world.scn.ron",
        }
    }

    /// Player-facing label used by notifications and catalog rows.
    pub const fn player_label(self) -> &'static str {
        match self {
            Self::Manual1 => "Manual slot 1",
            Self::Manual2 => "Manual slot 2",
            Self::Manual3 => "Manual slot 3",
            Self::Autosave1 => "Autosave 1",
            Self::Autosave2 => "Autosave 2",
            Self::Autosave3 => "Autosave 3",
            Self::Autosave4 => "Autosave 4",
            Self::Autosave5 => "Autosave 5",
            Self::LegacyDefault => "Legacy default save",
        }
    }

    /// Active autosave generation index `1..=5` when this ID is an autosave slot.
    pub const fn autosave_generation(self) -> Option<u8> {
        match self {
            Self::Autosave1 => Some(1),
            Self::Autosave2 => Some(2),
            Self::Autosave3 => Some(3),
            Self::Autosave4 => Some(4),
            Self::Autosave5 => Some(5),
            _ => None,
        }
    }

    /// Returns the autosave slot for generation `1..=5`.
    pub const fn autosave(generation: u8) -> Option<Self> {
        match generation {
            1 => Some(Self::Autosave1),
            2 => Some(Self::Autosave2),
            3 => Some(Self::Autosave3),
            4 => Some(Self::Autosave4),
            5 => Some(Self::Autosave5),
            _ => None,
        }
    }

    pub const fn is_manual(self) -> bool {
        matches!(self.role(), SaveSlotRole::Manual)
    }

    pub const fn is_autosave(self) -> bool {
        matches!(self.role(), SaveSlotRole::Autosave)
    }

    pub const fn is_legacy_default(self) -> bool {
        matches!(self.role(), SaveSlotRole::LegacyDefault)
    }
}

#[cfg(test)]
mod save_slot_tests {
    use super::*;

    #[test]
    fn closed_slot_set_exposes_only_fixed_ids_and_filenames() {
        assert_eq!(SaveSlotId::ALL.len(), 9);
        assert_eq!(
            SaveSlotId::Manual1.canonical_file_name(),
            "manual-1.scn.ron"
        );
        assert_eq!(
            SaveSlotId::Autosave5.canonical_file_name(),
            "autosave-5.scn.ron"
        );
        assert_eq!(
            SaveSlotId::LegacyDefault.canonical_file_name(),
            "world.scn.ron"
        );
        assert_eq!(SaveSlotId::autosave(0), None);
        assert_eq!(SaveSlotId::autosave(6), None);
        assert_eq!(SaveSlotId::autosave(3), Some(SaveSlotId::Autosave3));
    }

    #[test]
    fn role_and_capability_axes_stay_orthogonal_to_content() {
        assert!(SaveSlotId::Manual2.is_manual());
        assert!(SaveSlotId::Autosave1.is_autosave());
        assert!(SaveSlotId::LegacyDefault.is_legacy_default());
        assert_eq!(SaveSlotId::Manual1.role(), SaveSlotRole::Manual);
        assert_eq!(SaveSlotId::Autosave4.autosave_generation(), Some(4));
        assert_eq!(SaveSlotId::Manual1.autosave_generation(), None);
    }

    #[test]
    fn player_labels_never_expose_paths() {
        for slot in SaveSlotId::ALL {
            let label = slot.player_label();
            assert!(!label.contains('/'));
            assert!(!label.contains('\\'));
            assert!(!label.is_empty());
        }
    }
}
