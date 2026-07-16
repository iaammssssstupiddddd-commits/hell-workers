use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use hw_core::relationships::TaskWorkers;
use hw_energy::{
    ConsumesFrom, GeneratesFor, GridConsumers, GridGenerators, PowerConsumer, PowerGenerator,
    PowerGrid, SoulSpaSite, SoulSpaTile, Unpowered,
};

/// Dirty wake-up state for the energy pipeline. It deliberately contains no
/// Entity IDs, so save/load cannot retain references to a replaced world.
#[derive(Resource, Default)]
pub struct EnergyUpdateDirty {
    pub(crate) power_output_due: bool,
    pub(crate) grid_recalc_due: bool,
}

impl EnergyUpdateDirty {
    /// A loaded world has durable energy components but no runtime-derived
    /// totals or `Unpowered` markers yet. Force one ordered rebuild before
    /// normal Logic resumes.
    pub(crate) fn request_full_rebuild(&mut self) {
        self.power_output_due = true;
        self.grid_recalc_due = true;
    }
}

/// Feature-gated work counters for the dirty-driven energy pipeline.
#[cfg(feature = "profiling")]
#[derive(Resource, Debug, Default)]
pub struct EnergyPerfMetrics {
    pub power_output_runs: u64,
    pub grid_recalc_runs: u64,
    pub lamp_steps: u64,
    pub lamp_candidates_scanned: u64,
}

type EnergyOutputSiteDirtyQuery<'w, 's> =
    Query<'w, 's, (), Or<(Added<SoulSpaSite>, Changed<SoulSpaSite>, Changed<Children>)>>;
type EnergyOutputGeneratorDirtyQuery<'w, 's> =
    Query<'w, 's, (), (With<SoulSpaSite>, Changed<PowerGenerator>)>;
type EnergyTileWorkerDirtyQuery<'w, 's> = Query<
    'w,
    's,
    (),
    (
        With<SoulSpaTile>,
        Or<(Added<TaskWorkers>, Changed<TaskWorkers>)>,
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
        Changed<GridGenerators>,
        Changed<GridConsumers>,
        Changed<GeneratesFor>,
        Changed<ConsumesFrom>,
    )>,
>;

#[derive(SystemParam)]
pub(crate) struct EnergyDirtySignals<'w, 's> {
    q_output_sites: EnergyOutputSiteDirtyQuery<'w, 's>,
    q_output_generators: EnergyOutputGeneratorDirtyQuery<'w, 's>,
    q_tile_workers: EnergyTileWorkerDirtyQuery<'w, 's>,
    q_grid_inputs: EnergyGridInputDirtyQuery<'w, 's>,
    removed_workers: RemovedComponents<'w, 's, TaskWorkers>,
    removed_generators: RemovedComponents<'w, 's, GeneratesFor>,
    removed_consumers: RemovedComponents<'w, 's, ConsumesFrom>,
    removed_power_generators: RemovedComponents<'w, 's, PowerGenerator>,
    removed_power_consumers: RemovedComponents<'w, 's, PowerConsumer>,
    removed_power_grids: RemovedComponents<'w, 's, PowerGrid>,
}

pub(crate) fn detect_energy_update_dirty_system(
    mut dirty: ResMut<EnergyUpdateDirty>,
    mut signals: EnergyDirtySignals,
) {
    let output_changed = !signals.q_output_sites.is_empty()
        || !signals.q_tile_workers.is_empty()
        || !signals.q_output_generators.is_empty()
        || signals.removed_workers.read().count() != 0;
    dirty.power_output_due |= output_changed;
    dirty.grid_recalc_due |= output_changed
        || !signals.q_grid_inputs.is_empty()
        || signals.removed_generators.read().count() != 0
        || signals.removed_consumers.read().count() != 0
        || signals.removed_power_generators.read().count() != 0
        || signals.removed_power_consumers.read().count() != 0
        || signals.removed_power_grids.read().count() != 0;
}

pub fn energy_power_output_should_run(dirty: Res<EnergyUpdateDirty>) -> bool {
    dirty.power_output_due
}

pub fn energy_grid_recalc_should_run(dirty: Res<EnergyUpdateDirty>) -> bool {
    dirty.grid_recalc_due
}

/// PowerGrid の generation/consumption を集計し、停電状態を更新する。
/// soul_spa_power_output_system の後に実行することで PowerGenerator の変化を即時反映する。
pub fn grid_recalc_system(
    mut q_grids: Query<(
        &mut PowerGrid,
        Option<&GridGenerators>,
        Option<&GridConsumers>,
    )>,
    q_generators: Query<&PowerGenerator>,
    q_consumers: Query<&PowerConsumer>,
    mut commands: Commands,
    mut dirty: ResMut<EnergyUpdateDirty>,
    #[cfg(feature = "profiling")] mut metrics: ResMut<EnergyPerfMetrics>,
) {
    #[cfg(feature = "profiling")]
    {
        metrics.grid_recalc_runs = metrics.grid_recalc_runs.saturating_add(1);
    }
    for (mut grid, generators_opt, consumers_opt) in q_grids.iter_mut() {
        let new_gen: f32 = generators_opt
            .map(|generators| {
                generators
                    .iter()
                    .filter_map(|e| q_generators.get(*e).ok())
                    .map(|g| g.current_output)
                    .sum()
            })
            .unwrap_or(0.0);
        let new_cons: f32 = consumers_opt
            .map(|consumers| {
                consumers
                    .iter()
                    .filter_map(|e| q_consumers.get(*e).ok())
                    .map(|c| c.demand)
                    .sum()
            })
            .unwrap_or(0.0);
        // consumers == 0 は停電なし（PowerGrid::default() の仕様に合わせる）
        let new_powered = new_cons == 0.0 || new_gen >= new_cons;

        let gen_changed = (grid.generation - new_gen).abs() > f32::EPSILON;
        let cons_changed = (grid.consumption - new_cons).abs() > f32::EPSILON;
        let powered_changed = grid.powered != new_powered;

        if gen_changed {
            grid.generation = new_gen;
        }
        if cons_changed {
            grid.consumption = new_cons;
        }

        if powered_changed {
            grid.powered = new_powered;
            info!(
                "[Energy] Grid {} (gen={:.2}W, cons={:.2}W)",
                if new_powered { "POWERED" } else { "BLACKOUT" },
                new_gen,
                new_cons
            );
        }

        // powered_changed に加え、グリッドが通電中にコンシューマーが追加された場合も
        // Unpowered マーカーを同期する（新規追加コンシューマーは #[require(Unpowered)] で
        // デフォルト Unpowered になるが、powered_changed は発生しないため）。
        let sync_consumers = powered_changed || (new_powered && cons_changed);
        if sync_consumers && let Some(consumers) = consumers_opt {
            for &consumer in consumers.iter() {
                if new_powered {
                    commands.entity(consumer).remove::<Unpowered>();
                } else {
                    commands.entity(consumer).try_insert(Unpowered);
                }
            }
        }
    }
    dirty.grid_recalc_due = false;
}

#[cfg(all(test, feature = "profiling"))]
mod tests {
    use super::{
        EnergyPerfMetrics, EnergyUpdateDirty, energy_grid_recalc_should_run,
        energy_power_output_should_run, grid_recalc_system,
    };
    use crate::systems::energy::power_output::soul_spa_power_output_system;
    use bevy::prelude::*;

    #[test]
    fn ordered_energy_pipeline_returns_to_zero_work_when_steady() {
        let mut app = App::new();
        app.init_resource::<EnergyUpdateDirty>()
            .init_resource::<EnergyPerfMetrics>()
            .add_systems(
                Update,
                (
                    soul_spa_power_output_system.run_if(energy_power_output_should_run),
                    grid_recalc_system.run_if(energy_grid_recalc_should_run),
                )
                    .chain(),
            );
        app.world_mut()
            .resource_mut::<EnergyUpdateDirty>()
            .request_full_rebuild();

        app.update();
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
