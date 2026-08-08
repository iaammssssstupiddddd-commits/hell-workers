use super::*;
use hw_core::relationships::TaskWorkers;
use hw_jobs::{
    Building, DeconstructionOrder, DeconstructionPending, MovePlanned, SandPile,
    TargetDeconstructionRoot, TaskSlots,
};

fn spawn_valid_order(app: &mut App) -> (Entity, Entity, Vec2) {
    let target_pos = WorldMap::grid_to_world(8, 9);
    let target = app
        .world_mut()
        .spawn((
            Building {
                kind: BuildingType::SandPile,
                is_provisional: false,
            },
            SandPile,
            Transform::from_translation(target_pos.extend(0.0)),
        ))
        .id();
    let order = app
        .world_mut()
        .spawn((
            DeconstructionOrder,
            Designation {
                work_type: WorkType::Deconstruct,
            },
            TaskSlots::new(1),
            TargetDeconstructionRoot(target),
            Transform::from_translation(target_pos.extend(0.0)),
        ))
        .id();
    app.world_mut().flush();
    app.world_mut()
        .entity_mut(target)
        .insert(DeconstructionPending { order });
    (order, target, target_pos)
}

#[test]
fn dismantling_emits_one_commit_and_keeps_identity_on_order() {
    let mut app = task_execution_test_app();
    let (order, target, target_pos) = spawn_valid_order(&mut app);
    let soul = spawn_task_execution_soul(
        app.world_mut(),
        AssignedTask::Deconstruct(DeconstructData {
            order,
            target,
            phase: DeconstructPhase::Dismantling { progress: 1.0 },
        }),
    );
    app.world_mut().entity_mut(soul).insert((
        Transform::from_translation(WorldMap::grid_to_world(7, 9).extend(0.0)),
        ActiveTaskIdentity::new(order, order, WorkType::Deconstruct),
        WorkingOn(order),
    ));

    app.update();

    assert!(matches!(
        app.world().get::<AssignedTask>(soul),
        Some(AssignedTask::Deconstruct(DeconstructData {
            phase: DeconstructPhase::AwaitingCommit,
            ..
        }))
    ));
    assert_eq!(
        app.world().get::<WorkingOn>(soul).map(|working| working.0),
        Some(order)
    );
    assert_eq!(
        app.world()
            .get::<ActiveTaskIdentity>(soul)
            .map(|identity| identity.current_target_entity),
        Some(order)
    );
    assert_eq!(
        app.world().get::<TaskWorkers>(order).map(TaskWorkers::len),
        Some(1)
    );
    let commits = &app
        .world()
        .resource::<TaskNotificationReceipts>()
        .deconstruction_commits;
    assert_eq!(commits.len(), 1);
    assert_eq!(commits[0].worker, soul);
    assert_eq!(commits[0].order, order);
    assert_eq!(commits[0].target, target);
    assert_eq!(commits[0].identity.current_target_entity, order);
    assert!(
        app.world()
            .resource::<TaskNotificationReceipts>()
            .completed_domain
            .is_empty()
    );

    app.update();
    assert_eq!(
        app.world()
            .resource::<TaskNotificationReceipts>()
            .deconstruction_commits
            .len(),
        1,
        "AwaitingCommit must not emit the request again"
    );
    assert_eq!(
        app.world()
            .get::<Transform>(target)
            .unwrap()
            .translation
            .truncate(),
        target_pos
    );
}

#[test]
fn moving_target_retryably_aborts_without_emitting_commit() {
    let mut app = task_execution_test_app();
    let (order, target, _) = spawn_valid_order(&mut app);
    let move_task = app.world_mut().spawn_empty().id();
    app.world_mut().entity_mut(target).insert(MovePlanned {
        task_entity: move_task,
    });
    let soul = spawn_task_execution_soul(
        app.world_mut(),
        AssignedTask::Deconstruct(DeconstructData {
            order,
            target,
            phase: DeconstructPhase::GoingToTarget,
        }),
    );
    app.world_mut().entity_mut(soul).insert((
        ActiveTaskIdentity::new(order, order, WorkType::Deconstruct),
        WorkingOn(order),
    ));

    app.update();

    assert!(matches!(
        app.world().get::<AssignedTask>(soul),
        Some(AssignedTask::None)
    ));
    assert!(
        app.world()
            .resource::<TaskNotificationReceipts>()
            .deconstruction_commits
            .is_empty()
    );
    assert!(app.world().get::<DeconstructionPending>(target).is_some());
}
