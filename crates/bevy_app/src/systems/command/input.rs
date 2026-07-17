use super::{TaskArea, TaskMode};
use crate::app_contexts::TaskContext;
use crate::entities::familiar::{ActiveCommand, Familiar, FamiliarCommand};
use crate::input_actions::{InputAction, ResolvedInputFrame};
use bevy::prelude::*;

/// Applies the single Familiar command resolved for the frame-start selection.
pub fn familiar_command_input_system(
    resolved_frame: Res<ResolvedInputFrame>,
    q_familiars: Query<(), With<Familiar>>,
    mut q_active_commands: Query<(&mut ActiveCommand, Option<&TaskArea>), With<Familiar>>,
    mut task_context: ResMut<TaskContext>,
) {
    let Some(entity) = resolved_frame.selected_familiar() else {
        return;
    };
    if q_familiars.get(entity).is_err() {
        return;
    }

    for action in resolved_frame.actions() {
        match action {
            InputAction::FamiliarChop => task_context.0 = TaskMode::DesignateChop(None),
            InputAction::FamiliarMine => task_context.0 = TaskMode::DesignateMine(None),
            InputAction::FamiliarHaul => task_context.0 = TaskMode::DesignateHaul(None),
            InputAction::FamiliarBuild => task_context.0 = TaskMode::SelectBuildTarget,
            InputAction::FamiliarCancelDesignation => {
                task_context.0 = TaskMode::CancelDesignation(None);
            }
            InputAction::ToggleFamiliarIdlePatrol => {
                task_context.0 = TaskMode::None;
                if let Ok((mut active, area_opt)) = q_active_commands.get_mut(entity) {
                    if matches!(active.command, FamiliarCommand::Idle) && area_opt.is_some() {
                        active.command = FamiliarCommand::Patrol;
                    } else {
                        active.command = FamiliarCommand::Idle;
                    }
                }
            }
            _ => {}
        }
    }
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
    fn familiar_escape_preserves_idle_patrol_toggle() {
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
