use bevy::prelude::KeyCode;

use super::{InputAction, InputActionFamily, InputChord, InputConflictLane};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct InputBinding {
    pub chord: InputChord,
    pub action: InputAction,
    pub exclusive_family: Option<InputActionFamily>,
    pub family_priority: u8,
    pub conflict_lane: InputConflictLane,
    pub resolution_priority: u8,
}

pub(crate) const DEFAULT_BINDINGS: &[InputBinding] = &[
    InputBinding {
        chord: InputChord::plain(KeyCode::F5),
        action: InputAction::SaveGame,
        exclusive_family: Some(InputActionFamily::SaveLoad),
        family_priority: 2,
        conflict_lane: InputConflictLane::SimulationControl,
        resolution_priority: 2,
    },
    InputBinding {
        chord: InputChord::plain(KeyCode::F9),
        action: InputAction::RequestLoadGame,
        exclusive_family: Some(InputActionFamily::SaveLoad),
        family_priority: 1,
        conflict_lane: InputConflictLane::OverlayTransition,
        resolution_priority: 3,
    },
    InputBinding {
        chord: InputChord::plain(KeyCode::KeyV),
        action: InputAction::CycleElevation,
        exclusive_family: None,
        family_priority: 0,
        conflict_lane: InputConflictLane::ViewDebug,
        resolution_priority: 1,
    },
];

const COMPATIBLE_ACTION_PAIRS: &[(InputAction, InputAction)] =
    &[(InputAction::SaveGame, InputAction::CycleElevation)];

pub(crate) fn actions_are_compatible(left: &InputBinding, right: &InputBinding) -> bool {
    if matches!(
        (left.conflict_lane, right.conflict_lane),
        (
            InputConflictLane::OverlayTransition,
            InputConflictLane::SimulationControl | InputConflictLane::ViewDebug
        ) | (
            InputConflictLane::SimulationControl | InputConflictLane::ViewDebug,
            InputConflictLane::OverlayTransition
        )
    ) {
        return false;
    }

    COMPATIBLE_ACTION_PAIRS.iter().any(|(first, second)| {
        (left.action == *first && right.action == *second)
            || (left.action == *second && right.action == *first)
    })
}
