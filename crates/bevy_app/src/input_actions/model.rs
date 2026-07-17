use bevy::prelude::*;

/// Project-owned semantic actions resolved from physical keyboard chords.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InputAction {
    SaveGame,
    RequestLoadGame,
    CycleElevation,
    ToggleArchitect,
    ToggleZones,
    TogglePause,
    TimePaused,
    TimeNormal,
    TimeFast,
    TimeSuper,
    FamiliarChop,
    FamiliarMine,
    FamiliarHaul,
    FamiliarBuild,
    FamiliarCancelDesignation,
    ToggleFamiliarIdlePatrol,
    CancelLoadConfirm,
    CloseSettings,
    CloseOperationDialog,
    CancelActiveMode,
    CloseOpenMenu,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InputActionFamily {
    SaveLoad,
    TimeControl,
    MenuToggle,
    FamiliarCommand,
    CancelOrClose,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum InputConflictLane {
    OverlayTransition,
    SelectionOrMode,
    SimulationControl,
    ViewDebug,
}

/// Left/right modifier keys normalized into one frame snapshot.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct InputModifiers {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub super_key: bool,
}

impl InputModifiers {
    pub(crate) fn from_keyboard(keyboard: &ButtonInput<KeyCode>) -> Self {
        Self {
            ctrl: keyboard.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight]),
            alt: keyboard.any_pressed([KeyCode::AltLeft, KeyCode::AltRight]),
            shift: keyboard.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]),
            super_key: keyboard.any_pressed([KeyCode::SuperLeft, KeyCode::SuperRight]),
        }
    }
}

/// Exact physical key plus normalized modifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct InputChord {
    pub key: KeyCode,
    pub modifiers: InputModifiers,
}

impl InputChord {
    pub const fn plain(key: KeyCode) -> Self {
        Self {
            key,
            modifiers: InputModifiers {
                ctrl: false,
                alt: false,
                shift: false,
                super_key: false,
            },
        }
    }
}
