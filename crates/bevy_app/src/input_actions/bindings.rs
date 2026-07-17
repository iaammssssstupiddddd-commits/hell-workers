use bevy::prelude::KeyCode;

use super::{
    InputAction, InputActionFamily, InputChord, InputConflictLane, InputContextSnapshot,
    InputOverlay,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum InputBindingContext {
    Global,
    WorldNormal,
    Familiar,
    LoadConfirm,
    Settings,
    Pause,
    OperationDialog,
    ActiveMode,
    OpenMenu,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct InputBinding {
    pub chord: InputChord,
    pub action: InputAction,
    pub context: InputBindingContext,
    pub context_priority: u8,
    pub exclusive_family: Option<InputActionFamily>,
    pub family_priority: u8,
    pub conflict_lane: InputConflictLane,
    pub resolution_priority: u8,
    pub suppresses_pointer_selection: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct InputBindingResolution {
    context_priority: u8,
    exclusive_family: Option<InputActionFamily>,
    family_priority: u8,
    resolution_priority: u8,
}

const fn resolution(
    context_priority: u8,
    exclusive_family: Option<InputActionFamily>,
    family_priority: u8,
    resolution_priority: u8,
) -> InputBindingResolution {
    InputBindingResolution {
        context_priority,
        exclusive_family,
        family_priority,
        resolution_priority,
    }
}

const fn binding(
    key: KeyCode,
    action: InputAction,
    context: InputBindingContext,
    resolution: InputBindingResolution,
    conflict_lane: InputConflictLane,
    suppresses_pointer_selection: bool,
) -> InputBinding {
    InputBinding {
        chord: InputChord::plain(key),
        action,
        context,
        context_priority: resolution.context_priority,
        exclusive_family: resolution.exclusive_family,
        family_priority: resolution.family_priority,
        conflict_lane,
        resolution_priority: resolution.resolution_priority,
        suppresses_pointer_selection,
    }
}

pub(crate) const DEFAULT_BINDINGS: &[InputBinding] = &[
    binding(
        KeyCode::F5,
        InputAction::SaveGame,
        InputBindingContext::Global,
        resolution(40, Some(InputActionFamily::SaveLoad), 2, 50),
        InputConflictLane::SimulationControl,
        false,
    ),
    binding(
        KeyCode::F9,
        InputAction::RequestLoadGame,
        InputBindingContext::Global,
        resolution(40, Some(InputActionFamily::SaveLoad), 1, 40),
        InputConflictLane::OverlayTransition,
        true,
    ),
    binding(
        KeyCode::KeyV,
        InputAction::CycleElevation,
        InputBindingContext::Global,
        resolution(40, None, 0, 20),
        InputConflictLane::ViewDebug,
        false,
    ),
    binding(
        KeyCode::KeyB,
        InputAction::ToggleArchitect,
        InputBindingContext::WorldNormal,
        resolution(40, Some(InputActionFamily::MenuToggle), 1, 70),
        InputConflictLane::SelectionOrMode,
        true,
    ),
    binding(
        KeyCode::KeyZ,
        InputAction::ToggleZones,
        InputBindingContext::WorldNormal,
        resolution(40, Some(InputActionFamily::MenuToggle), 2, 70),
        InputConflictLane::SelectionOrMode,
        true,
    ),
    binding(
        KeyCode::Space,
        InputAction::TogglePause,
        InputBindingContext::Global,
        resolution(40, Some(InputActionFamily::TimeControl), 1, 100),
        InputConflictLane::OverlayTransition,
        true,
    ),
    binding(
        KeyCode::Digit1,
        InputAction::TimePaused,
        InputBindingContext::Global,
        resolution(40, Some(InputActionFamily::TimeControl), 2, 60),
        InputConflictLane::SimulationControl,
        true,
    ),
    binding(
        KeyCode::Digit2,
        InputAction::TimeNormal,
        InputBindingContext::Global,
        resolution(40, Some(InputActionFamily::TimeControl), 3, 60),
        InputConflictLane::SimulationControl,
        true,
    ),
    binding(
        KeyCode::Digit3,
        InputAction::TimeFast,
        InputBindingContext::Global,
        resolution(40, Some(InputActionFamily::TimeControl), 4, 60),
        InputConflictLane::SimulationControl,
        true,
    ),
    binding(
        KeyCode::Digit4,
        InputAction::TimeSuper,
        InputBindingContext::Global,
        resolution(40, Some(InputActionFamily::TimeControl), 5, 60),
        InputConflictLane::SimulationControl,
        true,
    ),
    binding(
        KeyCode::KeyC,
        InputAction::FamiliarChop,
        InputBindingContext::Familiar,
        resolution(50, Some(InputActionFamily::FamiliarCommand), 6, 80),
        InputConflictLane::SelectionOrMode,
        true,
    ),
    binding(
        KeyCode::Digit1,
        InputAction::FamiliarChop,
        InputBindingContext::Familiar,
        resolution(50, Some(InputActionFamily::FamiliarCommand), 6, 80),
        InputConflictLane::SelectionOrMode,
        true,
    ),
    binding(
        KeyCode::KeyM,
        InputAction::FamiliarMine,
        InputBindingContext::Familiar,
        resolution(50, Some(InputActionFamily::FamiliarCommand), 5, 80),
        InputConflictLane::SelectionOrMode,
        true,
    ),
    binding(
        KeyCode::Digit2,
        InputAction::FamiliarMine,
        InputBindingContext::Familiar,
        resolution(50, Some(InputActionFamily::FamiliarCommand), 5, 80),
        InputConflictLane::SelectionOrMode,
        true,
    ),
    binding(
        KeyCode::KeyH,
        InputAction::FamiliarHaul,
        InputBindingContext::Familiar,
        resolution(50, Some(InputActionFamily::FamiliarCommand), 4, 80),
        InputConflictLane::SelectionOrMode,
        true,
    ),
    binding(
        KeyCode::Digit3,
        InputAction::FamiliarHaul,
        InputBindingContext::Familiar,
        resolution(50, Some(InputActionFamily::FamiliarCommand), 4, 80),
        InputConflictLane::SelectionOrMode,
        true,
    ),
    binding(
        KeyCode::KeyB,
        InputAction::FamiliarBuild,
        InputBindingContext::Familiar,
        resolution(50, Some(InputActionFamily::FamiliarCommand), 3, 80),
        InputConflictLane::SelectionOrMode,
        true,
    ),
    binding(
        KeyCode::Digit4,
        InputAction::FamiliarBuild,
        InputBindingContext::Familiar,
        resolution(50, Some(InputActionFamily::FamiliarCommand), 3, 80),
        InputConflictLane::SelectionOrMode,
        true,
    ),
    binding(
        KeyCode::Digit0,
        InputAction::FamiliarCancelDesignation,
        InputBindingContext::Familiar,
        resolution(50, Some(InputActionFamily::FamiliarCommand), 2, 80),
        InputConflictLane::SelectionOrMode,
        true,
    ),
    binding(
        KeyCode::Delete,
        InputAction::FamiliarCancelDesignation,
        InputBindingContext::Familiar,
        resolution(50, Some(InputActionFamily::FamiliarCommand), 2, 80),
        InputConflictLane::SelectionOrMode,
        true,
    ),
    binding(
        KeyCode::Escape,
        InputAction::ToggleFamiliarIdlePatrol,
        InputBindingContext::Familiar,
        resolution(50, Some(InputActionFamily::FamiliarCommand), 1, 80),
        InputConflictLane::SelectionOrMode,
        true,
    ),
    binding(
        KeyCode::Escape,
        InputAction::CancelLoadConfirm,
        InputBindingContext::LoadConfirm,
        resolution(100, Some(InputActionFamily::CancelOrClose), 6, 110),
        InputConflictLane::OverlayTransition,
        true,
    ),
    binding(
        KeyCode::Escape,
        InputAction::CloseSettings,
        InputBindingContext::Settings,
        resolution(100, Some(InputActionFamily::CancelOrClose), 5, 110),
        InputConflictLane::OverlayTransition,
        true,
    ),
    binding(
        KeyCode::Escape,
        InputAction::TogglePause,
        InputBindingContext::Pause,
        resolution(100, Some(InputActionFamily::TimeControl), 1, 100),
        InputConflictLane::OverlayTransition,
        true,
    ),
    binding(
        KeyCode::Escape,
        InputAction::CloseOperationDialog,
        InputBindingContext::OperationDialog,
        resolution(100, Some(InputActionFamily::CancelOrClose), 4, 110),
        InputConflictLane::OverlayTransition,
        true,
    ),
    binding(
        KeyCode::Escape,
        InputAction::CancelActiveMode,
        InputBindingContext::ActiveMode,
        resolution(80, Some(InputActionFamily::CancelOrClose), 3, 95),
        InputConflictLane::SelectionOrMode,
        true,
    ),
    binding(
        KeyCode::Escape,
        InputAction::CloseOpenMenu,
        InputBindingContext::OpenMenu,
        resolution(70, Some(InputActionFamily::CancelOrClose), 2, 90),
        InputConflictLane::SelectionOrMode,
        true,
    ),
];

pub(crate) fn binding_matches_context(
    binding: &InputBinding,
    context: &InputContextSnapshot,
) -> bool {
    if let Some(overlay) = context.top_overlay {
        return match overlay {
            InputOverlay::LoadConfirm => binding.context == InputBindingContext::LoadConfirm,
            InputOverlay::Settings => binding.context == InputBindingContext::Settings,
            InputOverlay::Pause => {
                binding.context == InputBindingContext::Pause
                    || (binding.context == InputBindingContext::Global
                        && action_allowed_while_paused(binding.action))
            }
            InputOverlay::OperationDialog => {
                binding.context == InputBindingContext::OperationDialog
            }
        };
    }

    match binding.context {
        InputBindingContext::Global => true,
        InputBindingContext::WorldNormal => {
            !context.active_mode() && context.pending_play_mode.is_none()
        }
        InputBindingContext::Familiar => context.familiar_shortcuts_enabled(),
        InputBindingContext::ActiveMode => context.active_mode(),
        InputBindingContext::OpenMenu => context.open_menu(),
        InputBindingContext::LoadConfirm
        | InputBindingContext::Settings
        | InputBindingContext::Pause
        | InputBindingContext::OperationDialog => false,
    }
}

fn action_allowed_while_paused(action: InputAction) -> bool {
    matches!(
        action,
        InputAction::SaveGame
            | InputAction::RequestLoadGame
            | InputAction::TogglePause
            | InputAction::TimePaused
            | InputAction::TimeNormal
            | InputAction::TimeFast
            | InputAction::TimeSuper
    )
}

pub(crate) fn actions_are_compatible(
    left: &InputBinding,
    right: &InputBinding,
    context: &InputContextSnapshot,
) -> bool {
    use InputConflictLane::{OverlayTransition, SelectionOrMode, SimulationControl, ViewDebug};

    if left.conflict_lane == OverlayTransition || right.conflict_lane == OverlayTransition {
        return false;
    }

    if matches!(
        (left.conflict_lane, right.conflict_lane),
        (ViewDebug, SelectionOrMode | SimulationControl)
            | (SelectionOrMode | SimulationControl, ViewDebug)
    ) {
        return true;
    }

    if matches!(
        (left.conflict_lane, right.conflict_lane),
        (SelectionOrMode, SimulationControl) | (SimulationControl, SelectionOrMode)
    ) {
        return !context.has_in_progress_gesture;
    }

    is_save_time_pair(left.action, right.action)
}

fn is_save_time_pair(left: InputAction, right: InputAction) -> bool {
    let is_time = |action| {
        matches!(
            action,
            InputAction::TimePaused
                | InputAction::TimeNormal
                | InputAction::TimeFast
                | InputAction::TimeSuper
        )
    };
    (left == InputAction::SaveGame && is_time(right))
        || (right == InputAction::SaveGame && is_time(left))
}
