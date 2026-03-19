use crate::systems::command::TaskMode;
use bevy::prelude::*;

pub(super) fn should_exit_after_apply(keyboard: &ButtonInput<KeyCode>) -> bool {
    keyboard.pressed(KeyCode::ShiftLeft) || keyboard.pressed(KeyCode::ShiftRight)
}

pub(super) fn reset_designation_mode(mode: TaskMode) -> TaskMode {
    match mode {
        TaskMode::DesignateChop(_) => TaskMode::DesignateChop(None),
        TaskMode::DesignateMine(_) => TaskMode::DesignateMine(None),
        TaskMode::DesignateHaul(_) => TaskMode::DesignateHaul(None),
        TaskMode::CancelDesignation(_) => TaskMode::CancelDesignation(None),
        _ => TaskMode::None,
    }
}
