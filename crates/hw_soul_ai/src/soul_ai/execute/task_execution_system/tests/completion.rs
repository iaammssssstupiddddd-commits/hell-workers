use super::*;

#[test]
fn normal_completion_publishes_matching_assignment_and_current_identity() {
    let mut app = task_execution_test_app();
    let assignment = app.world_mut().spawn_empty().id();
    let target = app
        .world_mut()
        .spawn((
            Transform::default(),
            Blueprint::new(BuildingType::Floor, vec![(1, 1)]),
        ))
        .id();
    let soul = spawn_task_execution_soul(
        app.world_mut(),
        AssignedTask::Build(BuildData {
            blueprint: target,
            phase: BuildPhase::Done,
        }),
    );
    app.world_mut().entity_mut(soul).insert((
        ActiveTaskIdentity::new(assignment, target, WorkType::Build),
        WorkingOn(target),
    ));

    app.update();

    let receipts = app.world().resource::<TaskNotificationReceipts>();
    let expected = OnTaskCompleted {
        entity: soul,
        assignment_entity: assignment,
        current_target_entity: target,
        current_work_type: WorkType::Build,
    };
    assert_eq!(receipts.completed_domain.as_slice(), &[expected]);
    assert_eq!(
        receipts.completed_visual.as_slice(),
        &[TaskCompletedVisualMessage {
            entity: soul,
            assignment_entity: assignment,
            current_target_entity: target,
            current_work_type: WorkType::Build,
        }]
    );
    assert!(receipts.abandoned.is_empty());
}

#[test]
fn building_progress_completion_finishes_without_a_follow_up_done_frame() {
    let mut app = task_execution_test_app();
    let assignment = app.world_mut().spawn_empty().id();
    let target = app
        .world_mut()
        .spawn((
            Transform::default(),
            Blueprint::new(BuildingType::Floor, vec![(0, 0)]),
            Designation {
                work_type: WorkType::Build,
            },
        ))
        .id();
    let soul = spawn_task_execution_soul(
        app.world_mut(),
        AssignedTask::Build(BuildData {
            blueprint: target,
            phase: BuildPhase::Building { progress: 1.0 },
        }),
    );
    app.world_mut().entity_mut(soul).insert((
        ActiveTaskIdentity::new(assignment, target, WorkType::Build),
        WorkingOn(target),
    ));
    app.world_mut()
        .entity_mut(soul)
        .get_mut::<Transform>()
        .expect("task execution soul has Transform")
        .translation = WorldMap::grid_to_world(1, 0).extend(0.0);
    app.update();

    let receipts = app.world().resource::<TaskNotificationReceipts>();
    assert_eq!(
        receipts.completed_domain.as_slice(),
        &[OnTaskCompleted {
            entity: soul,
            assignment_entity: assignment,
            current_target_entity: target,
            current_work_type: WorkType::Build,
        }]
    );
    assert!(matches!(
        app.world().get::<AssignedTask>(soul),
        Some(AssignedTask::None)
    ));
    assert!(app.world().get::<ActiveTaskIdentity>(soul).is_none());
}

#[test]
fn chain_completion_preserves_root_assignment_and_publishes_final_identity() {
    let mut app = task_execution_test_app();
    let assignment = app.world_mut().spawn_empty().id();
    let initial_target = app.world_mut().spawn_empty().id();
    let final_target = app
        .world_mut()
        .spawn((
            Transform::default(),
            Blueprint::new(BuildingType::Floor, vec![(1, 1)]),
        ))
        .id();
    let mut identity = ActiveTaskIdentity::new(assignment, initial_target, WorkType::Chop);
    identity.transition_to(final_target, WorkType::Build);
    let soul = spawn_task_execution_soul(
        app.world_mut(),
        AssignedTask::Build(BuildData {
            blueprint: final_target,
            phase: BuildPhase::Done,
        }),
    );
    app.world_mut()
        .entity_mut(soul)
        .insert((identity, WorkingOn(final_target)));

    app.update();

    let receipts = app.world().resource::<TaskNotificationReceipts>();
    assert_eq!(
        receipts.completed_domain.as_slice(),
        &[OnTaskCompleted {
            entity: soul,
            assignment_entity: assignment,
            current_target_entity: final_target,
            current_work_type: WorkType::Build,
        }]
    );
    assert_eq!(
        receipts.completed_visual.as_slice(),
        &[TaskCompletedVisualMessage {
            entity: soul,
            assignment_entity: assignment,
            current_target_entity: final_target,
            current_work_type: WorkType::Build,
        }]
    );
    assert!(receipts.abandoned.is_empty());
}
