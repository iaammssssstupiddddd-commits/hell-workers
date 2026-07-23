use super::*;
use hw_core::events::{
    OnTaskAbandoned, OnTaskCompleted, ResourceReservationOp, ResourceReservationRequest,
    TaskCompletedVisualMessage,
};
use hw_core::soul::{DamnedSoul, Destination, Path};
use hw_core::visual::SoulTaskHandles;
use hw_jobs::{
    Blueprint, BucketTransportData, BucketTransportDestination, BucketTransportPhase,
    BucketTransportSource, BuildData, BuildPhase, BuildingType, Designation, GeneratePowerData,
    GeneratePowerPhase, HaulData, HaulPhase, WorkType,
};
use hw_logistics::zone::Stockpile;
use hw_logistics::{Inventory, ResourceItem, ResourceType, SharedResourceCache};
use hw_world::WorldMap;

#[derive(Resource, Default)]
struct TaskNotificationReceipts {
    completed_domain: Vec<OnTaskCompleted>,
    completed_visual: Vec<TaskCompletedVisualMessage>,
    abandoned: Vec<OnTaskAbandoned>,
    reservation_ops: Vec<ResourceReservationOp>,
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

fn record_task_completed(
    trigger: On<OnTaskCompleted>,
    mut receipts: ResMut<TaskNotificationReceipts>,
) {
    receipts.completed_domain.push(*trigger.event());
}

fn collect_task_notification_messages(
    mut completed: MessageReader<TaskCompletedVisualMessage>,
    mut abandoned: MessageReader<OnTaskAbandoned>,
    mut reservations: MessageReader<ResourceReservationRequest>,
    mut receipts: ResMut<TaskNotificationReceipts>,
) {
    receipts.completed_visual.extend(completed.read().copied());
    receipts.abandoned.extend(abandoned.read().copied());
    receipts
        .reservation_ops
        .extend(reservations.read().map(|request| request.op.clone()));
}

fn task_execution_test_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .insert_resource(WorldMap::default())
        .insert_resource(empty_soul_task_handles())
        .init_resource::<RuntimePathSearchBudget>()
        .init_resource::<SharedResourceCache>()
        .init_resource::<TaskNotificationReceipts>()
        .add_message::<ResourceReservationRequest>()
        .add_message::<TaskCompletedVisualMessage>()
        .add_message::<OnTaskAbandoned>()
        .add_observer(record_task_completed)
        .add_systems(
            Update,
            (
                task_execution_system,
                ApplyDeferred,
                collect_task_notification_messages,
            )
                .chain(),
        );
    #[cfg(feature = "profiling")]
    app.init_resource::<TaskExecutionPerfMetrics>();
    app
}

fn idle_guard_probe_system(mut q_souls: TaskExecutionSoulQuery) {
    for (_, _, _, task, _, _, _, _, _, _) in q_souls.iter_mut() {
        if is_idle_task(&task) {
            continue;
        }
        unreachable!("the probe only spawns AssignedTask::None");
    }
}

#[derive(Resource, Default)]
struct ActiveTaskProbe {
    reached_without_working_on: bool,
}

fn active_task_without_working_on_probe_system(
    mut q_souls: TaskExecutionSoulQuery,
    mut probe: ResMut<ActiveTaskProbe>,
) {
    for (_, _, _, task, _, _, _, _, _, _) in q_souls.iter_mut() {
        if !is_idle_task(&task) {
            probe.reached_without_working_on = true;
        }
    }
}

fn spawn_task_execution_soul(world: &mut World, task: AssignedTask) -> Entity {
    world
        .spawn((
            Transform::default(),
            DamnedSoul::default(),
            task,
            Destination(Vec2::ZERO),
            Path::default(),
            Inventory::default(),
        ))
        .id()
}

fn assert_component_unchanged<T: Component>(world: &mut World, entity: Entity) {
    let mut changed_components = world.query_filtered::<Entity, Changed<T>>();
    assert!(
        !changed_components
            .iter(world)
            .any(|changed| changed == entity),
        "{} was unexpectedly marked Changed",
        std::any::type_name::<T>()
    );
}

mod aborts;
mod completion;
mod guards;
mod stockpile_policy;
