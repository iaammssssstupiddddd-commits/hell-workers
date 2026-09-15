use super::{TaskArea, TaskMode};
use crate::app_contexts::TaskContext;
use crate::entities::familiar::{ActiveCommand, Familiar, FamiliarCommand};
use crate::input_actions::{InputAction, ResolvedInputFrame};
use bevy::prelude::*;

/// Applies the single Familiar command resolved for the frame-start selection.
pub(crate) fn familiar_command_input_system(
    resolved_frame: Res<ResolvedInputFrame>,
    q_familiars: Query<(), With<Familiar>>,
    mut q_active_commands: Query<(&mut ActiveCommand, Option<&TaskArea>), With<Familiar>>,
    mut task_context: ResMut<TaskContext>,
    time: Option<Res<Time<Virtual>>>,
) {
    let Some(entity) = resolved_frame.selected_familiar() else {
        return;
    };
    if q_familiars.get(entity).is_err() {
        return;
    }

    for action in resolved_frame.actions() {
        if time.as_ref().is_some_and(|time| time.is_paused())
            && !matches!(
                action,
                InputAction::FamiliarChop | InputAction::FamiliarMine
            )
        {
            continue;
        }
        match action {
            InputAction::FamiliarChop => task_context.0 = TaskMode::DesignateChop(None),
            InputAction::FamiliarMine => task_context.0 = TaskMode::DesignateMine(None),
            InputAction::FamiliarHaul => task_context.0 = TaskMode::DesignateHaul(None),
            // Reserved binding: keep this unreachable until the target,
            // assignment, and completion path are implemented together.
            InputAction::FamiliarBuild => {}
            InputAction::FamiliarCancelDesignation => {
                task_context.0 = TaskMode::CancelDesignation(None);
            }
            InputAction::ToggleFamiliarIdlePatrol => {
                task_context.0 = TaskMode::None;
                if let Ok((mut active, area_opt)) = q_active_commands.get_mut(entity) {
                    toggle_idle_patrol(&mut active, area_opt);
                }
            }
            _ => {}
        }
    }
}

/// Shared by the keyboard and the target-bound context-menu command.
pub(crate) fn toggle_idle_patrol(active: &mut ActiveCommand, area: Option<&TaskArea>) {
    active.command = if matches!(active.command, FamiliarCommand::Idle) && area.is_some() {
        FamiliarCommand::Patrol
    } else {
        FamiliarCommand::Idle
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input_actions::InputModifiers;
    use crate::test_support::minimal_app;

    #[test]
    fn familiar_consumer_uses_frame_target_and_resolved_action() {
        let mut app = minimal_app();
        app.init_resource::<TaskContext>()
            .init_resource::<ResolvedInputFrame>()
            .add_systems(Update, familiar_command_input_system);
        let familiar = app
            .world_mut()
            .spawn((Familiar::default(), ActiveCommand::default()))
            .id();
        app.world_mut()
            .resource_mut::<ResolvedInputFrame>()
            .replace(
                InputModifiers::default(),
                vec![InputAction::FamiliarMine],
                Some(familiar),
                true,
            );

        app.update();

        assert_eq!(
            app.world().resource::<TaskContext>().0,
            TaskMode::DesignateMine(None)
        );
    }

    #[test]
    fn familiar_build_binding_does_not_enter_incomplete_selection_mode() {
        let mut app = minimal_app();
        app.init_resource::<TaskContext>()
            .init_resource::<ResolvedInputFrame>()
            .add_systems(Update, familiar_command_input_system);
        let familiar = app
            .world_mut()
            .spawn((Familiar::default(), ActiveCommand::default()))
            .id();
        app.world_mut()
            .resource_mut::<ResolvedInputFrame>()
            .replace(
                InputModifiers::default(),
                vec![InputAction::FamiliarBuild],
                Some(familiar),
                true,
            );

        app.update();

        assert_eq!(app.world().resource::<TaskContext>().0, TaskMode::None);
    }

    #[test]
    fn familiar_command_preserves_idle_patrol_toggle() {
        let mut app = minimal_app();
        app.init_resource::<TaskContext>()
            .init_resource::<ResolvedInputFrame>()
            .add_systems(Update, familiar_command_input_system);
        let familiar = app
            .world_mut()
            .spawn((
                Familiar::default(),
                ActiveCommand::default(),
                TaskArea::from_points(Vec2::ZERO, Vec2::ONE),
            ))
            .id();
        app.world_mut()
            .resource_mut::<ResolvedInputFrame>()
            .replace(
                InputModifiers::default(),
                vec![InputAction::ToggleFamiliarIdlePatrol],
                Some(familiar),
                true,
            );

        app.update();

        assert_eq!(
            app.world()
                .entity(familiar)
                .get::<ActiveCommand>()
                .unwrap()
                .command,
            FamiliarCommand::Patrol
        );
        assert_eq!(app.world().resource::<TaskContext>().0, TaskMode::None);
    }
}
