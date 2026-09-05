use bevy::prelude::Entity;

use super::InputOverlay;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum CaptureOpenAction {
    Save,
    Load,
    Help,
    Pause,
    Settings,
    Operation(Entity),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct CaptureOpenSnapshot {
    pub recovery_failed: bool,
    pub help_open: bool,
    pub settings_open: bool,
    pub simulation_paused: bool,
    pub operation_target_is_familiar: bool,
}

pub(super) const fn opening_capture(
    action: CaptureOpenAction,
    state: CaptureOpenSnapshot,
) -> Option<(InputOverlay, Option<Entity>)> {
    match action {
        CaptureOpenAction::Save if !state.recovery_failed => {
            Some((InputOverlay::SaveCatalog, None))
        }
        CaptureOpenAction::Load => Some((
            if state.recovery_failed {
                InputOverlay::RecoveryLoadCatalog
            } else {
                InputOverlay::LoadCatalog
            },
            None,
        )),
        CaptureOpenAction::Help if !state.recovery_failed && !state.help_open => {
            Some((InputOverlay::Help, None))
        }
        CaptureOpenAction::Pause if !state.recovery_failed && !state.simulation_paused => {
            Some((InputOverlay::Pause, None))
        }
        CaptureOpenAction::Settings if !state.recovery_failed && !state.settings_open => {
            Some((InputOverlay::Settings, None))
        }
        CaptureOpenAction::Operation(target)
            if !state.recovery_failed && state.operation_target_is_familiar =>
        {
            Some((InputOverlay::OperationDialog, Some(target)))
        }
        CaptureOpenAction::Save
        | CaptureOpenAction::Help
        | CaptureOpenAction::Pause
        | CaptureOpenAction::Settings
        | CaptureOpenAction::Operation(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entity(index: u32) -> Entity {
        Entity::from_raw_u32(index).expect("test entity index is valid")
    }

    #[test]
    fn recovery_only_admits_load_and_selects_its_recovery_overlay() {
        let state = CaptureOpenSnapshot {
            recovery_failed: true,
            operation_target_is_familiar: true,
            ..Default::default()
        };

        assert_eq!(
            opening_capture(CaptureOpenAction::Load, state),
            Some((InputOverlay::RecoveryLoadCatalog, None))
        );
        for action in [
            CaptureOpenAction::Save,
            CaptureOpenAction::Help,
            CaptureOpenAction::Pause,
            CaptureOpenAction::Settings,
            CaptureOpenAction::Operation(entity(1)),
        ] {
            assert_eq!(opening_capture(action, state), None);
        }
    }

    #[test]
    fn already_open_help_settings_and_pause_do_not_request_another_capture() {
        assert_eq!(
            opening_capture(
                CaptureOpenAction::Help,
                CaptureOpenSnapshot {
                    help_open: true,
                    ..Default::default()
                },
            ),
            None
        );
        assert_eq!(
            opening_capture(
                CaptureOpenAction::Settings,
                CaptureOpenSnapshot {
                    settings_open: true,
                    ..Default::default()
                },
            ),
            None
        );
        assert_eq!(
            opening_capture(
                CaptureOpenAction::Pause,
                CaptureOpenSnapshot {
                    simulation_paused: true,
                    ..Default::default()
                },
            ),
            None
        );
    }

    #[test]
    fn operation_capture_requires_the_exact_target_to_be_a_familiar() {
        let target = entity(7);
        assert_eq!(
            opening_capture(
                CaptureOpenAction::Operation(target),
                CaptureOpenSnapshot::default(),
            ),
            None
        );
        assert_eq!(
            opening_capture(
                CaptureOpenAction::Operation(target),
                CaptureOpenSnapshot {
                    operation_target_is_familiar: true,
                    ..Default::default()
                },
            ),
            Some((InputOverlay::OperationDialog, Some(target)))
        );
    }
}
