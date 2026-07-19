//! Root bridge from game-specific change sources to shared task revisions.

use std::collections::{HashMap, HashSet};

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use hw_core::area::TaskArea;
use hw_core::constants::FATIGUE_THRESHOLD;
use hw_core::familiar::{ActiveCommand, Familiar, FamiliarOperation};
use hw_core::relationships::{
    CommandedBy, Commanding, DeliveringTo, IncomingDeliveries, LoadedIn, LoadedItems, ManagedBy,
    ManagedTasks, ParkedAt, PushedBy, StoredIn, StoredItems, TaskWorkers,
};
use hw_core::soul::{DamnedSoul, IdleBehavior, IdleState, StressBreakdown};
use hw_energy::constants::DREAM_GENERATE_ASSIGN_THRESHOLD;
use hw_jobs::construction::{FloorTileBlueprint, WallTileBlueprint};
use hw_jobs::mud_mixer::MudMixerStorage;
use hw_jobs::{AssignedTask, Blueprint, Designation, TaskDiagnosticInputRevisions, TaskSlots};
use hw_logistics::transport_request::{TransportDemand, TransportRequest, WheelbarrowLease};
use hw_logistics::zone::Stockpile;
use hw_logistics::{
    BelongsTo, BucketStorage, Inventory, ResourceItem, SharedResourceCache, Wheelbarrow,
};
use hw_spatial::ResourceSpatialGrid;
use hw_world::WorldMap;

#[derive(Resource, Default)]
pub(crate) struct TaskDiagnosticExternalRevisionState {
    initialized: bool,
    availability_signature: (u64, u64),
    soul_eligibility: HashMap<Entity, SoulEligibilitySnapshot>,
}

#[derive(Debug, Clone, Copy)]
struct SoulEligibilitySnapshot {
    assigned: bool,
    familiar_idle_allowed: bool,
    auto_build_idle_allowed: bool,
    fatigue: f32,
    generate_power_ready: bool,
    has_breakdown: bool,
    commanded_by: Option<Entity>,
}

impl SoulEligibilitySnapshot {
    fn from_components(
        soul: &DamnedSoul,
        assigned_task: &AssignedTask,
        idle: &IdleState,
        has_breakdown: bool,
        commanded_by: Option<&CommandedBy>,
    ) -> Self {
        Self {
            assigned: !matches!(assigned_task, AssignedTask::None),
            familiar_idle_allowed: idle.behavior != IdleBehavior::ExhaustedGathering,
            auto_build_idle_allowed: !matches!(
                idle.behavior,
                IdleBehavior::ExhaustedGathering
                    | IdleBehavior::Resting
                    | IdleBehavior::GoingToRest
                    | IdleBehavior::Escaping
                    | IdleBehavior::Drifting
            ),
            fatigue: soul.fatigue,
            generate_power_ready: soul.dream >= DREAM_GENERATE_ASSIGN_THRESHOLD,
            has_breakdown,
            commanded_by: commanded_by.map(|owner| owner.0),
        }
    }

    fn semantically_differs(self, current: Self, familiar_thresholds: &[f32]) -> bool {
        self.assigned != current.assigned
            || self.familiar_idle_allowed != current.familiar_idle_allowed
            || self.auto_build_idle_allowed != current.auto_build_idle_allowed
            || self.generate_power_ready != current.generate_power_ready
            || self.has_breakdown != current.has_breakdown
            || self.commanded_by != current.commanded_by
            || familiar_thresholds
                .iter()
                .copied()
                .any(|threshold| (self.fatigue <= threshold) != (current.fatigue <= threshold))
            || (self.fatigue < FATIGUE_THRESHOLD) != (current.fatigue < FATIGUE_THRESHOLD)
    }
}

type ChangedTasksQuery<'w, 's> = Query<
    'w,
    's,
    Entity,
    (
        With<Designation>,
        Or<(
            Changed<Designation>,
            Changed<TaskSlots>,
            Changed<TaskWorkers>,
            Changed<Blueprint>,
            Changed<FloorTileBlueprint>,
            Changed<WallTileBlueprint>,
            Changed<ManagedBy>,
            Changed<TransportRequest>,
            Changed<TransportDemand>,
            Changed<Transform>,
        )>,
    ),
>;

type ChangedFamiliarsQuery<'w, 's> = Query<
    'w,
    's,
    (),
    Or<(
        Changed<Familiar>,
        Changed<FamiliarOperation>,
        Changed<ActiveCommand>,
        Changed<TaskArea>,
        Changed<Commanding>,
        Changed<ManagedTasks>,
    )>,
>;

type ChangedSoulsQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static DamnedSoul,
        &'static AssignedTask,
        &'static IdleState,
        Option<&'static StressBreakdown>,
        Option<&'static CommandedBy>,
    ),
    Or<(
        Changed<DamnedSoul>,
        Changed<AssignedTask>,
        Changed<IdleState>,
        Changed<StressBreakdown>,
        Changed<CommandedBy>,
    )>,
>;

type ChangedAvailabilityQuery<'w, 's> = Query<
    'w,
    's,
    (),
    Or<(
        Changed<StoredItems>,
        Changed<IncomingDeliveries>,
        Changed<Inventory>,
        Changed<ResourceItem>,
        Changed<Wheelbarrow>,
        Changed<Stockpile>,
        Changed<MudMixerStorage>,
        Changed<Blueprint>,
        Changed<BucketStorage>,
    )>,
>;

type ChangedAvailabilityRelationsQuery<'w, 's> = Query<
    'w,
    's,
    (),
    Or<(
        Changed<ParkedAt>,
        Changed<PushedBy>,
        Changed<WheelbarrowLease>,
        Changed<TransportDemand>,
        Changed<LoadedItems>,
        Changed<LoadedIn>,
        Changed<StoredIn>,
        Changed<DeliveringTo>,
        Changed<BelongsTo>,
    )>,
>;

#[derive(SystemParam)]
pub struct TaskRevisionDetectors<'w, 's> {
    changed_tasks: ChangedTasksQuery<'w, 's>,
    changed_familiars: ChangedFamiliarsQuery<'w, 's>,
    changed_souls: ChangedSoulsQuery<'w, 's>,
    changed_availability: ChangedAvailabilityQuery<'w, 's>,
    changed_availability_relations: ChangedAvailabilityRelationsQuery<'w, 's>,
    familiar_operations: Query<'w, 's, &'static FamiliarOperation>,
}

#[derive(SystemParam)]
pub struct TaskRevisionRemovals<'w, 's> {
    designations: RemovedComponents<'w, 's, Designation>,
    task_slots: RemovedComponents<'w, 's, TaskSlots>,
    task_workers: RemovedComponents<'w, 's, TaskWorkers>,
    blueprints: RemovedComponents<'w, 's, Blueprint>,
    floor_tiles: RemovedComponents<'w, 's, FloorTileBlueprint>,
    wall_tiles: RemovedComponents<'w, 's, WallTileBlueprint>,
    managed_by: RemovedComponents<'w, 's, ManagedBy>,
    transport_requests: RemovedComponents<'w, 's, TransportRequest>,
    familiars: RemovedComponents<'w, 's, Familiar>,
    familiar_operations: RemovedComponents<'w, 's, FamiliarOperation>,
    active_commands: RemovedComponents<'w, 's, ActiveCommand>,
    task_areas: RemovedComponents<'w, 's, TaskArea>,
    commandings: RemovedComponents<'w, 's, Commanding>,
    managed_tasks: RemovedComponents<'w, 's, ManagedTasks>,
    damned_souls: RemovedComponents<'w, 's, DamnedSoul>,
    assigned_tasks: RemovedComponents<'w, 's, AssignedTask>,
    idle_states: RemovedComponents<'w, 's, IdleState>,
    stress_breakdowns: RemovedComponents<'w, 's, StressBreakdown>,
    commanded_by: RemovedComponents<'w, 's, CommandedBy>,
    stored_items: RemovedComponents<'w, 's, StoredItems>,
    incoming_deliveries: RemovedComponents<'w, 's, IncomingDeliveries>,
    inventories: RemovedComponents<'w, 's, Inventory>,
    resource_items: RemovedComponents<'w, 's, ResourceItem>,
    wheelbarrows: RemovedComponents<'w, 's, Wheelbarrow>,
    parked_at: RemovedComponents<'w, 's, ParkedAt>,
    pushed_by: RemovedComponents<'w, 's, PushedBy>,
    wheelbarrow_leases: RemovedComponents<'w, 's, WheelbarrowLease>,
    transport_demands: RemovedComponents<'w, 's, TransportDemand>,
    stockpiles: RemovedComponents<'w, 's, Stockpile>,
    mixer_storages: RemovedComponents<'w, 's, MudMixerStorage>,
    bucket_storages: RemovedComponents<'w, 's, BucketStorage>,
    loaded_items: RemovedComponents<'w, 's, LoadedItems>,
    loaded_in: RemovedComponents<'w, 's, LoadedIn>,
    stored_in: RemovedComponents<'w, 's, StoredIn>,
    delivering_to: RemovedComponents<'w, 's, DeliveringTo>,
    belongs_to: RemovedComponents<'w, 's, BelongsTo>,
}

/// Final semantic revision sync. It runs after auto-gather Commands are
/// applied and immediately before the normal delegation cycle.
pub(crate) fn sync_task_diagnostic_revisions_system(
    detectors: TaskRevisionDetectors,
    mut removed: TaskRevisionRemovals,
    resource_grid: Res<ResourceSpatialGrid>,
    resource_cache: Res<SharedResourceCache>,
    world_map: Res<WorldMap>,
    mut external: ResMut<TaskDiagnosticExternalRevisionState>,
    mut revisions: ResMut<TaskDiagnosticInputRevisions>,
) {
    for entity in &detectors.changed_tasks {
        revisions.bump_task(entity);
    }

    let removed_designations: HashSet<_> = removed.designations.read().collect();
    let removed_task_slots: Vec<_> = removed.task_slots.read().collect();
    let removed_task_workers: Vec<_> = removed.task_workers.read().collect();
    let removed_blueprints: Vec<_> = removed.blueprints.read().collect();
    let removed_floor_tiles: Vec<_> = removed.floor_tiles.read().collect();
    let removed_wall_tiles: Vec<_> = removed.wall_tiles.read().collect();
    let removed_managed_by: Vec<_> = removed.managed_by.read().collect();
    let removed_transport_requests: Vec<_> = removed.transport_requests.read().collect();
    let removed_transport_demands: Vec<_> = removed.transport_demands.read().collect();
    for &entity in removed_task_slots
        .iter()
        .chain(&removed_task_workers)
        .chain(&removed_blueprints)
        .chain(&removed_floor_tiles)
        .chain(&removed_wall_tiles)
        .chain(&removed_managed_by)
        .chain(&removed_transport_requests)
        .chain(&removed_transport_demands)
    {
        if !removed_designations.contains(&entity) {
            revisions.bump_task(entity);
        }
    }
    for entity in removed_designations {
        revisions.remove_task(entity);
    }

    let familiar_thresholds: Vec<_> = detectors
        .familiar_operations
        .iter()
        .map(|operation| operation.fatigue_threshold)
        .collect();
    let mut roster_changed = !detectors.changed_familiars.is_empty();
    for (entity, soul, assigned_task, idle, breakdown, commanded_by) in &detectors.changed_souls {
        let current = SoulEligibilitySnapshot::from_components(
            soul,
            assigned_task,
            idle,
            breakdown.is_some(),
            commanded_by,
        );
        let previous = external.soul_eligibility.insert(entity, current);
        roster_changed |= previous
            .is_none_or(|previous| previous.semantically_differs(current, &familiar_thresholds));
    }
    roster_changed |= removed.familiars.read().count() > 0;
    roster_changed |= removed.familiar_operations.read().count() > 0;
    roster_changed |= removed.active_commands.read().count() > 0;
    roster_changed |= removed.task_areas.read().count() > 0;
    roster_changed |= removed.commandings.read().count() > 0;
    roster_changed |= removed.managed_tasks.read().count() > 0;
    let mut removed_soul_eligibility = HashSet::new();
    removed_soul_eligibility.extend(removed.damned_souls.read());
    removed_soul_eligibility.extend(removed.assigned_tasks.read());
    removed_soul_eligibility.extend(removed.idle_states.read());
    removed_soul_eligibility.extend(removed.stress_breakdowns.read());
    removed_soul_eligibility.extend(removed.commanded_by.read());
    if !removed_soul_eligibility.is_empty() {
        roster_changed = true;
        for entity in removed_soul_eligibility {
            external.soul_eligibility.remove(&entity);
        }
    }
    if roster_changed {
        revisions.bump_roster();
    }

    let availability_signature = (
        resource_grid.generation(),
        resource_cache.semantic_generation(),
    );
    let mut availability_changed = !detectors.changed_availability.is_empty()
        || !detectors.changed_availability_relations.is_empty();
    availability_changed |= removed.stored_items.read().count() > 0;
    availability_changed |= removed.incoming_deliveries.read().count() > 0;
    availability_changed |= removed.inventories.read().count() > 0;
    availability_changed |= removed.resource_items.read().count() > 0;
    availability_changed |= removed.wheelbarrows.read().count() > 0;
    availability_changed |= removed.parked_at.read().count() > 0;
    availability_changed |= removed.pushed_by.read().count() > 0;
    availability_changed |= removed.wheelbarrow_leases.read().count() > 0;
    availability_changed |= !removed_transport_demands.is_empty();
    availability_changed |= !removed_blueprints.is_empty();
    availability_changed |= removed.stockpiles.read().count() > 0;
    availability_changed |= removed.mixer_storages.read().count() > 0;
    availability_changed |= removed.bucket_storages.read().count() > 0;
    availability_changed |= removed.loaded_items.read().count() > 0;
    availability_changed |= removed.loaded_in.read().count() > 0;
    availability_changed |= removed.stored_in.read().count() > 0;
    availability_changed |= removed.delivering_to.read().count() > 0;
    availability_changed |= removed.belongs_to.read().count() > 0;
    availability_changed |=
        external.initialized && availability_signature != external.availability_signature;
    if availability_changed {
        revisions.bump_availability();
    }
    external.availability_signature = availability_signature;
    external.initialized = true;
    if revisions.topology != world_map.obstacle_version {
        revisions.set_topology(world_map.obstacle_version);
    }
}

pub(crate) fn reset_task_diagnostics_for_world_replace(world: &mut World) {
    world.insert_resource(TaskDiagnosticInputRevisions::default());
    world.insert_resource(hw_familiar_ai::FamiliarTaskCandidateDiagnostics::default());
    world.insert_resource(hw_soul_ai::BlueprintAutoBuildDiagnostics::default());
    world.insert_resource(TaskDiagnosticExternalRevisionState::default());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn world_replace_reset_drops_entity_revisions_and_snapshots() {
        let task = Entity::from_raw_u32(9).expect("valid test entity");
        let mut world = World::new();
        let mut revisions = TaskDiagnosticInputRevisions::default();
        revisions.bump_task(task);
        world.insert_resource(revisions);
        world.insert_resource(hw_familiar_ai::FamiliarTaskCandidateDiagnostics::default());
        world.insert_resource(TaskDiagnosticExternalRevisionState {
            initialized: true,
            availability_signature: (2, 3),
            ..Default::default()
        });

        reset_task_diagnostics_for_world_replace(&mut world);

        assert_eq!(
            world
                .resource::<TaskDiagnosticInputRevisions>()
                .task_revision(task),
            0
        );
        assert!(
            world
                .resource::<hw_familiar_ai::FamiliarTaskCandidateDiagnostics>()
                .header()
                .is_none()
        );
        assert!(
            !world
                .resource::<TaskDiagnosticExternalRevisionState>()
                .initialized
        );
    }

    #[test]
    fn designation_removal_does_not_recreate_a_task_revision() {
        let mut app = App::new();
        app.init_resource::<TaskDiagnosticInputRevisions>()
            .init_resource::<TaskDiagnosticExternalRevisionState>()
            .init_resource::<ResourceSpatialGrid>()
            .init_resource::<SharedResourceCache>()
            .init_resource::<WorldMap>()
            .add_systems(Update, sync_task_diagnostic_revisions_system);

        let task = app
            .world_mut()
            .spawn((
                Designation {
                    work_type: hw_jobs::WorkType::Chop,
                },
                TaskSlots::new(1),
            ))
            .id();
        app.update();
        assert_ne!(
            app.world()
                .resource::<TaskDiagnosticInputRevisions>()
                .task_revision(task),
            0
        );

        app.world_mut()
            .entity_mut(task)
            .remove::<(Designation, TaskSlots)>();
        app.update();

        assert_eq!(
            app.world()
                .resource::<TaskDiagnosticInputRevisions>()
                .task_revision(task),
            0
        );
    }

    #[test]
    fn roster_revision_tracks_eligibility_boundaries_not_idle_timers() {
        let mut app = App::new();
        app.init_resource::<TaskDiagnosticInputRevisions>()
            .init_resource::<TaskDiagnosticExternalRevisionState>()
            .init_resource::<ResourceSpatialGrid>()
            .init_resource::<SharedResourceCache>()
            .init_resource::<WorldMap>()
            .add_systems(Update, sync_task_diagnostic_revisions_system);

        let soul = app
            .world_mut()
            .spawn((
                DamnedSoul::default(),
                AssignedTask::None,
                IdleState::default(),
            ))
            .id();
        app.update();
        let initial_revision = app
            .world()
            .resource::<TaskDiagnosticInputRevisions>()
            .roster;

        app.world_mut()
            .entity_mut(soul)
            .get_mut::<IdleState>()
            .expect("idle state exists")
            .idle_timer += 1.0;
        app.update();
        assert_eq!(
            app.world()
                .resource::<TaskDiagnosticInputRevisions>()
                .roster,
            initial_revision
        );

        app.world_mut()
            .entity_mut(soul)
            .get_mut::<DamnedSoul>()
            .expect("soul exists")
            .fatigue = 0.4;
        app.update();
        assert_eq!(
            app.world()
                .resource::<TaskDiagnosticInputRevisions>()
                .roster,
            initial_revision
        );

        app.world_mut()
            .entity_mut(soul)
            .get_mut::<DamnedSoul>()
            .expect("soul exists")
            .fatigue = 0.9;
        app.update();
        assert_ne!(
            app.world()
                .resource::<TaskDiagnosticInputRevisions>()
                .roster,
            initial_revision
        );
    }
}
