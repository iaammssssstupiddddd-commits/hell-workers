use crate::systems::command::TaskMode;
pub(super) fn should_exit_after_apply(shift_pressed: bool) -> bool {
    shift_pressed
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolved_shift_snapshot_controls_area_mode_exit() {
        assert!(should_exit_after_apply(true));
        assert!(!should_exit_after_apply(false));
    }
}
