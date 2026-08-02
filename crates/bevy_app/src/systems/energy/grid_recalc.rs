use std::collections::HashSet;

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use hw_core::GameSettings;
use hw_core::relationships::TaskWorkers;
use hw_energy::{
    ConsumesFrom, GeneratesFor, GridConsumers, GridGenerators, PowerAllocationMode, PowerConsumer,
    PowerConsumerAllocationInput, PowerConsumerPolicy, PowerGenerator, PowerGrid,
    PowerGridAllocationSummary, PowerSupplyState, SoulSpaSite, SoulSpaTile, Unpowered,
    YardPowerGrid, allocate_power,
};
use hw_world::WorldMap;
use hw_world::zones::Yard;

/// Dirty wake-up state for the ordered energy transaction. It deliberately
/// contains no Entity IDs, so save/load cannot retain references to a replaced world.
#[derive(Resource, Default)]
pub struct EnergyUpdateDirty {
    pub(crate) topology_reconcile_due: bool,
    pub(crate) power_output_due: bool,
    pub(crate) grid_recalc_due: bool,
}

impl EnergyUpdateDirty {
    /// A loaded world has durable energy policy but no trusted runtime topology
    /// or supply state. Force one ordered rebuild before normal Logic resumes.
    pub(crate) fn request_full_rebuild(&mut self) {
        self.topology_reconcile_due = true;
        self.power_output_due = true;
        self.grid_recalc_due = true;
    }

    pub(crate) fn request_topology_reconcile(&mut self) {
        self.topology_reconcile_due = true;
        self.grid_recalc_due = true;
    }
}

/// Feature-gated work counters for the dirty-driven energy pipeline.
#[cfg(feature = "profiling")]
#[derive(Resource, Debug, Default)]
pub struct EnergyPerfMetrics {
    pub topology_reconcile_runs: u64,
    pub power_output_runs: u64,
    pub grid_recalc_runs: u64,
    pub lamp_steps: u64,
    pub lamp_candidates_scanned: u64,
}

type EnergyOutputSiteDirtyQuery<'w, 's> =
    Query<'w, 's, (), Or<(Added<SoulSpaSite>, Changed<SoulSpaSite>)>>;
type EnergyOutputGeneratorDirtyQuery<'w, 's> =
    Query<'w, 's, (), (With<SoulSpaSite>, Changed<PowerGenerator>)>;
type EnergyTileWorkerDirtyQuery<'w, 's> = Query<
    'w,
    's,
    (),
    (
        With<SoulSpaTile>,
        Or<(
            Added<SoulSpaTile>,
            Changed<SoulSpaTile>,
            Added<TaskWorkers>,
            Changed<TaskWorkers>,
        )>,
    ),
>;
type EnergyGridInputDirtyQuery<'w, 's> = Query<
    'w,
    's,
    (),
    Or<(
        Added<PowerGrid>,
        Changed<PowerGenerator>,
        Changed<PowerConsumer>,
        Changed<PowerConsumerPolicy>,
        Changed<GridGenerators>,
        Changed<GridConsumers>,
        Changed<GeneratesFor>,
        Changed<ConsumesFrom>,
    )>,
>;
type EnergyTopologyEndpointDirtyQuery<'w, 's> = Query<
    'w,
    's,
    (),
    (
        Or<(With<PowerConsumer>, With<PowerGenerator>)>,
        Or<(
            Added<PowerConsumer>,
            Added<PowerGenerator>,
            Changed<Transform>,
        )>,
    ),
>;
type EnergyTopologyYardDirtyQuery<'w, 's> =
    Query<'w, 's, (), Or<(Added<Yard>, Changed<Yard>, Changed<YardPowerGrid>)>>;
type EnergyTopologyGridDirtyQuery<'w, 's> = Query<'w, 's, (), Added<PowerGrid>>;
type EnergyTopologyRelationDirtyQuery<'w, 's> =
    Query<'w, 's, (), Or<(Changed<GeneratesFor>, Changed<ConsumesFrom>)>>;
type MissingPowerSupplyStateQuery<'w, 's> =
    Query<'w, 's, (), (With<PowerConsumer>, Without<PowerSupplyState>)>;
type MissingPowerGridSummaryQuery<'w, 's> =
    Query<'w, 's, (), (With<PowerGrid>, Without<PowerGridAllocationSummary>)>;

#[derive(SystemParam)]
pub(crate) struct EnergyDirtySignals<'w, 's> {
    q_output_sites: EnergyOutputSiteDirtyQuery<'w, 's>,
    q_output_generators: EnergyOutputGeneratorDirtyQuery<'w, 's>,
    q_tile_workers: EnergyTileWorkerDirtyQuery<'w, 's>,
    q_soul_spa_tiles: Query<'w, 's, (), With<SoulSpaTile>>,
    q_grid_inputs: EnergyGridInputDirtyQuery<'w, 's>,
    q_topology_endpoints: EnergyTopologyEndpointDirtyQuery<'w, 's>,
    q_topology_yards: EnergyTopologyYardDirtyQuery<'w, 's>,
    q_topology_grids: EnergyTopologyGridDirtyQuery<'w, 's>,
    q_topology_relations: EnergyTopologyRelationDirtyQuery<'w, 's>,
    q_missing_supply_state: MissingPowerSupplyStateQuery<'w, 's>,
    q_missing_grid_summary: MissingPowerGridSummaryQuery<'w, 's>,
    removed_workers: RemovedComponents<'w, 's, TaskWorkers>,
    removed_soul_spa_tiles: RemovedComponents<'w, 's, SoulSpaTile>,
    removed_generators: RemovedComponents<'w, 's, GeneratesFor>,
    removed_consumers: RemovedComponents<'w, 's, ConsumesFrom>,
    removed_power_generators: RemovedComponents<'w, 's, PowerGenerator>,
    removed_power_consumers: RemovedComponents<'w, 's, PowerConsumer>,
    removed_consumer_policies: RemovedComponents<'w, 's, PowerConsumerPolicy>,
    removed_power_grids: RemovedComponents<'w, 's, PowerGrid>,
    removed_grid_owners: RemovedComponents<'w, 's, YardPowerGrid>,
    removed_yards: RemovedComponents<'w, 's, Yard>,
}

pub(crate) fn detect_energy_update_dirty_system(
    mut dirty: ResMut<EnergyUpdateDirty>,
    mut signals: EnergyDirtySignals,
) {
    let spa_workers_removed = signals
        .removed_workers
        .read()
        .any(|entity| signals.q_soul_spa_tiles.get(entity).is_ok());
    let spa_tile_removed = signals.removed_soul_spa_tiles.read().count() != 0;
    let generator_relation_removed = signals.removed_generators.read().count() != 0;
    let consumer_relation_removed = signals.removed_consumers.read().count() != 0;
    let generator_removed = signals.removed_power_generators.read().count() != 0;
    let consumer_removed = signals.removed_power_consumers.read().count() != 0;
    let policy_removed = signals.removed_consumer_policies.read().count() != 0;
    let grid_removed = signals.removed_power_grids.read().count() != 0;
    let grid_owner_removed = signals.removed_grid_owners.read().count() != 0;
    let yard_removed = signals.removed_yards.read().count() != 0;

    let output_changed = !signals.q_output_sites.is_empty()
        || !signals.q_output_generators.is_empty()
        || !signals.q_tile_workers.is_empty()
        || spa_workers_removed
        || spa_tile_removed;
    let topology_changed = !signals.q_topology_endpoints.is_empty()
        || !signals.q_topology_yards.is_empty()
        || !signals.q_topology_grids.is_empty()
        || !signals.q_topology_relations.is_empty()
        || generator_relation_removed
        || consumer_relation_removed
        || generator_removed
        || consumer_removed
        || grid_removed
        || grid_owner_removed
        || yard_removed;

    dirty.power_output_due |= output_changed;
    dirty.topology_reconcile_due |= topology_changed;
    dirty.grid_recalc_due |= output_changed
        || topology_changed
        || policy_removed
        || !signals.q_grid_inputs.is_empty()
        || !signals.q_missing_supply_state.is_empty()
        || !signals.q_missing_grid_summary.is_empty();
}

/// Settings の compatibility toggle を runtime mode へ写す。
/// 他の設定変更では energy dirty を立てない。
pub fn sync_power_allocation_mode_from_settings_system(
    settings: Res<GameSettings>,
    mut mode: ResMut<PowerAllocationMode>,
    mut dirty: ResMut<EnergyUpdateDirty>,
) {
    let desired = if settings.power_priority_enabled {
        PowerAllocationMode::PriorityPrefix
    } else {
        PowerAllocationMode::LegacyAllOrNone
    };
    if *mode != desired {
        *mode = desired;
        dirty.grid_recalc_due = true;
    }
}

pub fn energy_topology_should_run(dirty: Res<EnergyUpdateDirty>) -> bool {
    dirty.topology_reconcile_due
}

pub fn energy_power_output_should_run(dirty: Res<EnergyUpdateDirty>) -> bool {
    dirty.power_output_due
}

pub fn energy_grid_recalc_should_run(dirty: Res<EnergyUpdateDirty>) -> bool {
    dirty.grid_recalc_due
}

type PowerConsumerQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static PowerConsumer,
        Option<&'static PowerConsumerPolicy>,
        Option<&'static Transform>,
        Option<&'static PowerSupplyState>,
        Option<&'static ConsumesFrom>,
        Has<Unpowered>,
    ),
>;

type PowerGridQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static mut PowerGrid,
        Option<&'static GridGenerators>,
        Option<&'static GridConsumers>,
        Option<&'static mut PowerGridAllocationSummary>,
    ),
>;

/// 各 PowerGrid の generator/consumer を単一 transaction で集計・配電する。
pub fn grid_recalc_system(
    mut q_grids: PowerGridQuery,
    q_generators: Query<&PowerGenerator>,
    q_consumers: PowerConsumerQuery,
    mode: Res<PowerAllocationMode>,
    mut commands: Commands,
    mut dirty: ResMut<EnergyUpdateDirty>,
    #[cfg(feature = "profiling")] mut metrics: ResMut<EnergyPerfMetrics>,
) {
    #[cfg(feature = "profiling")]
    {
        metrics.grid_recalc_runs = metrics.grid_recalc_runs.saturating_add(1);
    }

    let valid_grids: HashSet<Entity> = q_grids.iter().map(|(entity, ..)| entity).collect();
    let mut allocated_consumers = HashSet::new();

    for (grid_entity, mut grid, generators, consumers, summary) in &mut q_grids {
        let generation = generators
            .into_iter()
            .flat_map(GridGenerators::iter)
            .filter_map(|entity| q_generators.get(*entity).ok())
            .map(|generator| generator.current_output)
            .filter(|output| output.is_finite() && *output > 0.0)
            .sum();

        let inputs: Vec<_> = consumers
            .into_iter()
            .flat_map(GridConsumers::iter)
            .filter_map(|entity| q_consumers.get(*entity).ok())
            .map(
                |(entity, consumer, policy, transform, previous_state, _, _)| {
                    PowerConsumerAllocationInput {
                        entity,
                        grid_pos: transform
                            .map(|transform| WorldMap::world_to_grid(transform.translation.xy()))
                            .unwrap_or((0, 0)),
                        demand: consumer.demand,
                        priority: policy.copied().unwrap_or_default().priority,
                        previous_state: previous_state.copied(),
                    }
                },
            )
            .collect();
        let result = allocate_power(*mode, generation, &inputs);

        let mut supplied_count = 0;
        let mut shed_order = Vec::new();
        let mut invalid_count = 0;
        for allocation in &result.consumers {
            allocated_consumers.insert(allocation.entity);
            match allocation.state {
                PowerSupplyState::Supplied => supplied_count += 1,
                PowerSupplyState::Shed { .. } => shed_order.push(allocation.entity),
                PowerSupplyState::InvalidDemand => invalid_count += 1,
                PowerSupplyState::Disconnected => {}
            }
            if let Ok((_, _, _, _, current_state, _, unpowered)) =
                q_consumers.get(allocation.entity)
            {
                sync_consumer_runtime_state(
                    &mut commands,
                    allocation.entity,
                    current_state.copied(),
                    unpowered,
                    allocation.state,
                );
            }
        }

        update_grid_field(&mut grid.generation, generation);
        update_grid_field(&mut grid.consumption, result.total_demand);
        if grid.powered != result.all_supplied {
            grid.powered = result.all_supplied;
        }

        let next_summary = PowerGridAllocationSummary {
            mode: *mode,
            generation,
            total_demand: result.total_demand,
            served_demand: result.served_demand,
            consumer_count: result.consumers.len(),
            supplied_count,
            shed_count: shed_order.len(),
            invalid_count,
            shed_order,
        };
        if let Some(mut summary) = summary {
            if *summary != next_summary {
                *summary = next_summary;
            }
        } else {
            commands.entity(grid_entity).insert(next_summary);
        }
    }

    for (entity, _, _, _, current_state, relation, unpowered) in &q_consumers {
        if allocated_consumers.contains(&entity) {
            continue;
        }
        let relation_is_valid = relation.is_some_and(|relation| valid_grids.contains(&relation.0));
        debug_assert!(
            !relation_is_valid,
            "valid ConsumesFrom must be represented by GridConsumers"
        );
        sync_consumer_runtime_state(
            &mut commands,
            entity,
            current_state.copied(),
            unpowered,
            PowerSupplyState::Disconnected,
        );
    }

    dirty.grid_recalc_due = false;
}

fn update_grid_field(current: &mut f32, next: f32) {
    if (*current - next).abs() > f32::EPSILON {
        *current = next;
    }
}

fn sync_consumer_runtime_state(
    commands: &mut Commands,
    entity: Entity,
    current: Option<PowerSupplyState>,
    unpowered: bool,
    next: PowerSupplyState,
) {
    if current != Some(next) {
        commands.entity(entity).insert(next);
    }
    let should_be_unpowered = next != PowerSupplyState::Supplied;
    if should_be_unpowered && !unpowered {
        commands.entity(entity).insert(Unpowered);
    } else if !should_be_unpowered && unpowered {
        commands.entity(entity).remove::<Unpowered>();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hw_energy::{PowerPriority, PowerShedReason};

    fn test_app() -> App {
        let mut app = App::new();
        app.init_resource::<EnergyUpdateDirty>()
            .init_resource::<PowerAllocationMode>()
            .add_systems(Update, (grid_recalc_system, ApplyDeferred).chain());
        app
    }

    #[test]
    fn grid_transaction_supplies_priority_prefix_and_updates_runtime_markers() {
        let mut app = test_app();
        let grid = app.world_mut().spawn(PowerGrid::default()).id();
        app.world_mut().spawn((
            PowerGenerator {
                current_output: 0.75,
                output_per_soul: 1.0,
            },
            GeneratesFor(grid),
        ));
        let high = app
            .world_mut()
            .spawn((
                PowerConsumer { demand: 0.75 },
                PowerConsumerPolicy {
                    priority: PowerPriority::High,
                },
                Transform::from_xyz(5.0, 5.0, 0.0),
                ConsumesFrom(grid),
            ))
            .id();
        let low = app
            .world_mut()
            .spawn((
                PowerConsumer { demand: 0.75 },
                PowerConsumerPolicy {
                    priority: PowerPriority::Low,
                },
                Transform::from_xyz(0.0, 0.0, 0.0),
                ConsumesFrom(grid),
            ))
            .id();

        app.world_mut()
            .resource_mut::<EnergyUpdateDirty>()
            .grid_recalc_due = true;
        app.update();

        assert_eq!(
            app.world().get::<PowerSupplyState>(high),
            Some(&PowerSupplyState::Supplied)
        );
        assert!(app.world().get::<Unpowered>(high).is_none());
        assert_eq!(
            app.world().get::<PowerSupplyState>(low),
            Some(&PowerSupplyState::Shed {
                reason: PowerShedReason::InsufficientGeneration,
            })
        );
        assert!(app.world().get::<Unpowered>(low).is_some());
        let summary = app.world().get::<PowerGridAllocationSummary>(grid).unwrap();
        assert_eq!(summary.supplied_count, 1);
        assert_eq!(summary.shed_order, vec![low]);
        assert_eq!(summary.served_demand, 0.75);
        assert!(!app.world().get::<PowerGrid>(grid).unwrap().powered);
    }

    #[test]
    fn disconnected_and_invalid_consumers_fail_closed() {
        let mut app = test_app();
        let grid = app.world_mut().spawn(PowerGrid::default()).id();
        let invalid = app
            .world_mut()
            .spawn((PowerConsumer { demand: f32::NAN }, ConsumesFrom(grid)))
            .id();
        let disconnected = app.world_mut().spawn(PowerConsumer { demand: 0.1 }).id();

        app.update();

        assert_eq!(
            app.world().get::<PowerSupplyState>(invalid),
            Some(&PowerSupplyState::InvalidDemand)
        );
        assert_eq!(
            app.world().get::<PowerSupplyState>(disconnected),
            Some(&PowerSupplyState::Disconnected)
        );
        assert!(app.world().get::<Unpowered>(invalid).is_some());
        assert!(app.world().get::<Unpowered>(disconnected).is_some());
    }

    #[test]
    fn settings_sync_dirties_only_when_allocation_mode_changes() {
        let mut app = App::new();
        app.init_resource::<GameSettings>()
            .init_resource::<PowerAllocationMode>()
            .init_resource::<EnergyUpdateDirty>()
            .add_systems(Update, sync_power_allocation_mode_from_settings_system);

        app.update();
        assert!(!app.world().resource::<EnergyUpdateDirty>().grid_recalc_due);
        app.world_mut().resource_mut::<GameSettings>().ui_scale = 1.1;
        app.update();
        assert!(!app.world().resource::<EnergyUpdateDirty>().grid_recalc_due);

        app.world_mut()
            .resource_mut::<GameSettings>()
            .power_priority_enabled = false;
        app.update();
        assert_eq!(
            *app.world().resource::<PowerAllocationMode>(),
            PowerAllocationMode::LegacyAllOrNone
        );
        assert!(app.world().resource::<EnergyUpdateDirty>().grid_recalc_due);
    }

    #[test]
    fn presentation_visibility_changes_do_not_wake_energy_work() {
        let mut app = App::new();
        app.init_resource::<EnergyUpdateDirty>()
            .add_systems(Update, detect_energy_update_dirty_system);
        let panel = app.world_mut().spawn(Visibility::Hidden).id();

        app.update();
        let dirty = app.world().resource::<EnergyUpdateDirty>();
        assert!(!dirty.topology_reconcile_due);
        assert!(!dirty.power_output_due);
        assert!(!dirty.grid_recalc_due);

        app.world_mut()
            .entity_mut(panel)
            .insert(Visibility::Visible);
        app.update();
        let dirty = app.world().resource::<EnergyUpdateDirty>();
        assert!(!dirty.topology_reconcile_due);
        assert!(!dirty.power_output_due);
        assert!(!dirty.grid_recalc_due);
    }

    #[test]
    fn only_soul_spa_task_worker_removals_wake_energy_output() {
        use hw_core::relationships::WorkingOn;

        let mut app = App::new();
        app.init_resource::<EnergyUpdateDirty>()
            .add_systems(Update, detect_energy_update_dirty_system);
        let regular_task = app.world_mut().spawn_empty().id();
        let regular_worker = app.world_mut().spawn(WorkingOn(regular_task)).id();
        app.update();
        *app.world_mut().resource_mut::<EnergyUpdateDirty>() = EnergyUpdateDirty::default();

        app.world_mut().despawn(regular_worker);
        app.update();
        let dirty = app.world().resource::<EnergyUpdateDirty>();
        assert!(!dirty.power_output_due);
        assert!(!dirty.grid_recalc_due);

        let tile = app
            .world_mut()
            .spawn(SoulSpaTile {
                parent_site: Entity::PLACEHOLDER,
                grid_pos: (0, 0),
            })
            .id();
        let spa_worker = app.world_mut().spawn(WorkingOn(tile)).id();
        app.update();
        *app.world_mut().resource_mut::<EnergyUpdateDirty>() = EnergyUpdateDirty::default();

        app.world_mut().despawn(spa_worker);
        app.update();
        let dirty = app.world().resource::<EnergyUpdateDirty>();
        assert!(dirty.power_output_due);
        assert!(dirty.grid_recalc_due);

        *app.world_mut().resource_mut::<EnergyUpdateDirty>() = EnergyUpdateDirty::default();
        app.world_mut().despawn(tile);
        app.update();
        let dirty = app.world().resource::<EnergyUpdateDirty>();
        assert!(dirty.power_output_due);
        assert!(dirty.grid_recalc_due);
    }

    #[test]
    fn removed_policy_and_missing_runtime_state_wake_grid_recalculation() {
        let mut app = App::new();
        app.init_resource::<EnergyUpdateDirty>()
            .add_systems(Update, detect_energy_update_dirty_system);
        let owner = app.world_mut().spawn_empty().id();
        let grid = app
            .world_mut()
            .spawn((
                PowerGrid::default(),
                PowerGridAllocationSummary::default(),
                YardPowerGrid(owner),
            ))
            .id();
        let consumer = app
            .world_mut()
            .spawn((PowerConsumer { demand: 1.0 }, PowerSupplyState::Supplied))
            .id();

        app.update();
        *app.world_mut().resource_mut::<EnergyUpdateDirty>() = EnergyUpdateDirty::default();
        app.world_mut()
            .entity_mut(consumer)
            .remove::<PowerConsumerPolicy>();
        app.update();
        assert!(app.world().resource::<EnergyUpdateDirty>().grid_recalc_due);

        *app.world_mut().resource_mut::<EnergyUpdateDirty>() = EnergyUpdateDirty::default();
        app.world_mut()
            .entity_mut(consumer)
            .remove::<PowerSupplyState>();
        app.world_mut()
            .entity_mut(grid)
            .remove::<PowerGridAllocationSummary>();
        app.update();
        assert!(app.world().resource::<EnergyUpdateDirty>().grid_recalc_due);

        *app.world_mut().resource_mut::<EnergyUpdateDirty>() = EnergyUpdateDirty::default();
        app.world_mut().entity_mut(grid).remove::<YardPowerGrid>();
        app.update();
        assert!(
            app.world()
                .resource::<EnergyUpdateDirty>()
                .topology_reconcile_due
        );
    }

    #[test]
    fn a_direct_soul_spa_generator_change_wakes_output_and_allocation() {
        let mut app = App::new();
        app.init_resource::<EnergyUpdateDirty>()
            .add_systems(Update, detect_energy_update_dirty_system);
        let site = app.world_mut().spawn(SoulSpaSite::default()).id();

        app.update();
        *app.world_mut().resource_mut::<EnergyUpdateDirty>() = EnergyUpdateDirty::default();
        app.world_mut()
            .get_mut::<PowerGenerator>(site)
            .unwrap()
            .output_per_soul = 2.0;
        app.update();

        let dirty = app.world().resource::<EnergyUpdateDirty>();
        assert!(dirty.power_output_due);
        assert!(dirty.grid_recalc_due);
    }

    #[cfg(feature = "profiling")]
    #[test]
    fn ordered_energy_pipeline_returns_to_zero_work_when_steady() {
        use crate::systems::energy::power_output::soul_spa_power_output_system;
        use hw_core::relationships::WorkingOn;
        use hw_energy::{GeneratesFor, SoulSpaPhase, SoulSpaSite, SoulSpaTile};

        let mut app = App::new();
        app.init_resource::<EnergyUpdateDirty>()
            .init_resource::<EnergyPerfMetrics>()
            .init_resource::<PowerAllocationMode>()
            .add_systems(
                Update,
                (
                    detect_energy_update_dirty_system,
                    soul_spa_power_output_system.run_if(energy_power_output_should_run),
                    grid_recalc_system.run_if(energy_grid_recalc_should_run),
                    ApplyDeferred,
                )
                    .chain(),
            );
        let grid = app.world_mut().spawn(PowerGrid::default()).id();
        let site = app
            .world_mut()
            .spawn((
                SoulSpaSite {
                    phase: SoulSpaPhase::Operational,
                    ..default()
                },
                GeneratesFor(grid),
            ))
            .id();
        let tile = app
            .world_mut()
            .spawn(SoulSpaTile {
                parent_site: site,
                grid_pos: (0, 0),
            })
            .id();
        app.world_mut().spawn(WorkingOn(tile));
        app.world_mut()
            .resource_mut::<EnergyUpdateDirty>()
            .request_full_rebuild();

        app.update();
        assert_eq!(
            app.world()
                .get::<PowerGenerator>(site)
                .expect("Soul Spa generator")
                .current_output,
            1.0
        );
        assert_eq!(
            app.world()
                .resource::<EnergyPerfMetrics>()
                .power_output_runs,
            1
        );
        assert_eq!(
            app.world().resource::<EnergyPerfMetrics>().grid_recalc_runs,
            1
        );

        app.update();
        let metrics = app.world().resource::<EnergyPerfMetrics>();
        assert_eq!(metrics.power_output_runs, 1);
        assert_eq!(metrics.grid_recalc_runs, 1);
    }
}
