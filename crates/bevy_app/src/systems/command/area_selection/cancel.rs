//! cancel_single_designation と補助処理

use crate::systems::jobs::{
    BlueprintCancelRequested, Designation, PlayerIssuedDesignation, Priority, TaskSlots,
};
use crate::systems::logistics::transport_request::ManualHaulPinnedSource;
use bevy::prelude::*;
use hw_core::events::SoulTaskUnassignRequest;
use hw_core::relationships::{ManagedBy, TaskWorkers};

/// Designation/Blueprint/TransportRequest を 1 件キャンセル
pub fn cancel_single_designation(
    commands: &mut Commands,
    target_entity: Entity,
    task_workers: Option<&TaskWorkers>,
    is_blueprint: bool,
    is_transport_request: bool,
    fixed_source: Option<Entity>,
) {
    if let Some(workers) = task_workers {
        for &soul in workers.iter() {
            // Cleanup and notification ordering belong to Soul AI. The request
            // reaches Perceive before Execute, so a user cancellation cannot
            // race a terminal task handler in the same Update.
            commands.write_message(SoulTaskUnassignRequest {
                soul_entity: soul,
                emit_abandoned: true,
            });
        }
    }

    if let Some(source_entity) = fixed_source {
        commands
            .entity(source_entity)
            .try_remove::<ManualHaulPinnedSource>();
    }

    if is_blueprint {
        commands
            .entity(target_entity)
            .try_insert(BlueprintCancelRequested);
    } else if is_transport_request {
        commands.entity(target_entity).try_despawn();
    } else {
        commands.entity(target_entity).try_remove::<(
            Designation,
            TaskSlots,
            ManagedBy,
            Priority,
            PlayerIssuedDesignation,
        )>();
    }
}

#[cfg(test)]
mod tests {
    use super::cancel_single_designation;
    use bevy::ecs::schedule::ApplyDeferred;
    use bevy::prelude::*;
    use hw_core::events::{
        OnTaskAbandoned, OnTaskCompleted, ResourceReservationRequest, TaskCompletedVisualMessage,
    };
    use hw_core::relationships::{TaskWorkers, WorkingOn};
    use hw_core::soul::{DamnedSoul, Destination, Path};
    use hw_core::system_sets::SoulAiSystemSet;
    use hw_core::visual::SoulTaskHandles;
    use hw_jobs::{
        ActiveTaskIdentity, AssignedTask, Blueprint, BuildData, BuildPhase, BuildingType, WorkType,
    };
    use hw_logistics::{Inventory, SharedResourceCache};
    #[cfg(feature = "profiling")]
    use hw_soul_ai::soul_ai::execute::task_execution::TaskExecutionPerfMetrics;
    use hw_soul_ai::soul_ai::execute::task_execution_system::task_execution_system;
    use hw_soul_ai::soul_ai::execute::task_unassign_apply::handle_soul_task_unassign_system;
    use hw_world::{RuntimePathSearchBudget, WorldMap};

    #[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
    struct UserCancelSet;

    #[derive(Resource)]
    struct CancellationTarget(Entity);

    #[derive(Resource, Default)]
    struct Receipts {
        completed_domain: usize,
        completed_visual: Vec<TaskCompletedVisualMessage>,
        abandoned: Vec<OnTaskAbandoned>,
    }

    fn empty_soul_task_handles() -> SoulTaskHandles {
        SoulTaskHandles {
            wood: default(),
            tree_animes: Vec::new(),
            rock: default(),
            icon_bone_small: default(),
            icon_sand_small: default(),
            icon_stasis_mud_small: default(),
            bucket_water: default(),
            bucket_empty: default(),
        }
    }

    fn cancel_blueprint(
        mut commands: Commands,
        target: Res<CancellationTarget>,
        workers: Query<&TaskWorkers>,
    ) {
        cancel_single_designation(
            &mut commands,
            target.0,
            workers.get(target.0).ok(),
            true,
            false,
            None,
        );
    }

    fn record_completed(_: On<OnTaskCompleted>, mut receipts: ResMut<Receipts>) {
        receipts.completed_domain += 1;
    }

    fn collect_messages(
        mut completed: MessageReader<TaskCompletedVisualMessage>,
        mut abandoned: MessageReader<OnTaskAbandoned>,
        mut receipts: ResMut<Receipts>,
    ) {
        receipts.completed_visual.extend(completed.read().copied());
        receipts.abandoned.extend(abandoned.read().copied());
    }

    #[test]
    fn blueprint_cancel_unassigns_before_same_update_task_execution() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .insert_resource(WorldMap::default())
            .insert_resource(empty_soul_task_handles())
            .insert_resource(hw_logistics::ResourceItemVisualHandles {
                icon_bone_small: default(),
                icon_wood_small: default(),
                icon_rock_small: default(),
                icon_sand_small: default(),
                icon_stasis_mud_small: default(),
            })
            .init_resource::<RuntimePathSearchBudget>()
            .init_resource::<SharedResourceCache>()
            .init_resource::<Receipts>();
        #[cfg(feature = "profiling")]
        app.init_resource::<TaskExecutionPerfMetrics>();
        app.add_message::<hw_core::events::SoulTaskUnassignRequest>()
            .add_message::<ResourceReservationRequest>()
            .add_message::<TaskCompletedVisualMessage>()
            .add_message::<OnTaskAbandoned>()
            .add_observer(record_completed)
            .configure_sets(
                Update,
                (
                    UserCancelSet,
                    SoulAiSystemSet::Perceive,
                    SoulAiSystemSet::Execute,
                )
                    .chain(),
            )
            .add_systems(
                Update,
                (
                    cancel_blueprint,
                    ApplyDeferred,
                    crate::systems::jobs::blueprint_cancellation_system,
                    ApplyDeferred,
                )
                    .chain()
                    .in_set(UserCancelSet),
            )
            .add_systems(
                Update,
                handle_soul_task_unassign_system.in_set(SoulAiSystemSet::Perceive),
            )
            .add_systems(
                Update,
                ApplyDeferred
                    .after(SoulAiSystemSet::Perceive)
                    .before(SoulAiSystemSet::Execute),
            )
            .add_systems(
                Update,
                task_execution_system.in_set(SoulAiSystemSet::Execute),
            )
            .add_systems(
                Update,
                (ApplyDeferred, collect_messages)
                    .chain()
                    .after(SoulAiSystemSet::Execute),
            );

        let blueprint = app
            .world_mut()
            .spawn((
                Transform::default(),
                Blueprint::new(BuildingType::Floor, vec![(1, 1)]),
            ))
            .id();
        let assignment = app.world_mut().spawn_empty().id();
        let soul = app
            .world_mut()
            .spawn((
                Transform::default(),
                DamnedSoul::default(),
                AssignedTask::Build(BuildData {
                    blueprint,
                    phase: BuildPhase::Done,
                }),
                Destination(Vec2::ZERO),
                Path::default(),
                Inventory::default(),
                ActiveTaskIdentity::new(assignment, blueprint, WorkType::Build),
                WorkingOn(blueprint),
            ))
            .id();
        assert_eq!(
            app.world()
                .get::<TaskWorkers>(blueprint)
                .expect("WorkingOn must create its TaskWorkers target")
                .len(),
            1
        );
        app.insert_resource(CancellationTarget(blueprint));

        app.update();

        let receipts = app.world().resource::<Receipts>();
        assert_eq!(receipts.completed_domain, 0);
        assert!(receipts.completed_visual.is_empty());
        assert_eq!(receipts.abandoned, vec![OnTaskAbandoned { entity: soul }]);
        assert!(app.world().get_entity(blueprint).is_err());
        assert!(matches!(
            app.world().get::<AssignedTask>(soul),
            Some(AssignedTask::None)
        ));
        assert!(app.world().get::<ActiveTaskIdentity>(soul).is_none());
        assert!(app.world().get::<WorkingOn>(soul).is_none());
    }
}
