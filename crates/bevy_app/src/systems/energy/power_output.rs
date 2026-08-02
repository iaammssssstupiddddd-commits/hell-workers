use bevy::prelude::*;
use hw_core::relationships::TaskWorkers;
use hw_energy::{PowerGenerator, SoulSpaPhase, SoulSpaSite, SoulSpaTile};
use std::collections::HashMap;

#[cfg(feature = "profiling")]
use super::grid_recalc::EnergyPerfMetrics;
use super::grid_recalc::EnergyUpdateDirty;

/// Operational SoulSpaSite の稼働タイル数から `PowerGenerator.current_output` を更新する。
/// `Update` のdirty-gated energy pipelineで実行し、同じframeのgrid再計算へ変更を伝播する。
pub fn soul_spa_power_output_system(
    mut q_sites: Query<(Entity, &SoulSpaSite, &mut PowerGenerator)>,
    q_tiles: Query<(&SoulSpaTile, Option<&TaskWorkers>)>,
    mut dirty: ResMut<EnergyUpdateDirty>,
    #[cfg(feature = "profiling")] mut metrics: ResMut<EnergyPerfMetrics>,
) {
    #[cfg(feature = "profiling")]
    {
        metrics.power_output_runs = metrics.power_output_runs.saturating_add(1);
    }
    let occupied_by_site = q_tiles
        .iter()
        .filter(|(_, workers)| workers.is_some_and(|workers| !workers.is_empty()))
        .fold(HashMap::<Entity, u32>::new(), |mut counts, (tile, _)| {
            *counts.entry(tile.parent_site).or_default() += 1;
            counts
        });

    let mut output_changed = false;
    for (site_entity, site, mut generator) in q_sites.iter_mut() {
        if site.phase != SoulSpaPhase::Operational {
            if generator.current_output != 0.0 {
                generator.bypass_change_detection().current_output = 0.0;
                output_changed = true;
            }
            continue;
        }

        let active_count = occupied_by_site.get(&site_entity).copied().unwrap_or(0) as f32;

        let raw_output = active_count * generator.output_per_soul;
        let new_output = if raw_output.is_finite() && raw_output > 0.0 {
            raw_output
        } else {
            0.0
        };
        if !generator.current_output.is_finite()
            || (generator.current_output - new_output).abs() > f32::EPSILON
        {
            // This derived write is propagated to allocation below in the same
            // ordered transaction. Suppress only this write's Changed flag so
            // the next frame does not repeat identical work; direct external
            // PowerGenerator writes remain normal dirty inputs.
            generator.bypass_change_detection().current_output = new_output;
            output_changed = true;
        }
    }
    dirty.power_output_due = false;
    // Propagate an actual derived output write through this frame's ordered
    // pipeline instead of leaving the grid temporarily stale.
    dirty.grid_recalc_due |= output_changed;
}

#[cfg(test)]
mod tests {
    use super::*;
    use hw_core::relationships::WorkingOn;

    #[test]
    fn output_uses_durable_parent_site_without_display_hierarchy() {
        let mut app = App::new();
        app.init_resource::<EnergyUpdateDirty>()
            .add_systems(Update, soul_spa_power_output_system);

        let site = app
            .world_mut()
            .spawn((
                SoulSpaSite {
                    phase: SoulSpaPhase::Operational,
                    ..default()
                },
                PowerGenerator {
                    current_output: 99.0,
                    ..default()
                },
            ))
            .id();
        let tile = app
            .world_mut()
            .spawn(SoulSpaTile {
                parent_site: site,
                grid_pos: (3, 4),
            })
            .id();
        let worker = app.world_mut().spawn(WorkingOn(tile)).id();

        app.update();

        assert_eq!(
            app.world()
                .get::<PowerGenerator>(site)
                .unwrap()
                .current_output,
            1.0
        );
        assert!(app.world().get::<Children>(site).is_none());

        app.world_mut().despawn(worker);
        app.update();

        assert_eq!(
            app.world()
                .get::<PowerGenerator>(site)
                .unwrap()
                .current_output,
            0.0,
            "a worker-free loaded site must not retain a saved stale output"
        );
    }

    #[test]
    fn state_sanity_flush_removes_stale_worker_before_output() {
        use hw_core::relationships::TaskWorkers;
        use hw_core::system_sets::{SoulAiSystemSet, StateSanityFlushSet};
        use hw_jobs::AssignedTask;

        let mut app = App::new();
        app.init_resource::<EnergyUpdateDirty>()
            .add_systems(
                Update,
                hw_soul_ai::soul_ai::update::state_sanity::clear_stale_working_on_system
                    .in_set(SoulAiSystemSet::Update),
            )
            .add_systems(
                Update,
                ApplyDeferred
                    .after(SoulAiSystemSet::Update)
                    .in_set(StateSanityFlushSet),
            )
            .add_systems(
                Update,
                soul_spa_power_output_system.after(StateSanityFlushSet),
            );
        let site = app
            .world_mut()
            .spawn((
                SoulSpaSite {
                    phase: SoulSpaPhase::Operational,
                    ..default()
                },
                PowerGenerator {
                    current_output: 99.0,
                    output_per_soul: 1.0,
                },
            ))
            .id();
        let tile = app
            .world_mut()
            .spawn(SoulSpaTile {
                parent_site: site,
                grid_pos: (0, 0),
            })
            .id();
        let soul = app
            .world_mut()
            .spawn((AssignedTask::None, WorkingOn(tile)))
            .id();

        app.update();

        assert!(app.world().get::<WorkingOn>(soul).is_none());
        assert!(
            app.world()
                .get::<TaskWorkers>(tile)
                .is_none_or(TaskWorkers::is_empty)
        );
        assert_eq!(
            app.world()
                .get::<PowerGenerator>(site)
                .unwrap()
                .current_output,
            0.0
        );
    }

    #[test]
    fn invalid_output_rates_fail_closed_instead_of_retaining_saved_output() {
        let mut app = App::new();
        app.init_resource::<EnergyUpdateDirty>()
            .add_systems(Update, soul_spa_power_output_system);

        for (x, output_per_soul) in [(0, f32::NAN), (1, -1.0)] {
            let site = app
                .world_mut()
                .spawn((
                    SoulSpaSite {
                        phase: SoulSpaPhase::Operational,
                        ..default()
                    },
                    PowerGenerator {
                        current_output: 99.0,
                        output_per_soul,
                    },
                ))
                .id();
            let tile = app
                .world_mut()
                .spawn(SoulSpaTile {
                    parent_site: site,
                    grid_pos: (x, 0),
                })
                .id();
            app.world_mut().spawn(WorkingOn(tile));
        }

        app.update();

        let outputs: Vec<f32> = app
            .world_mut()
            .query::<&PowerGenerator>()
            .iter(app.world())
            .map(|generator| generator.current_output)
            .collect();
        assert_eq!(outputs, vec![0.0, 0.0]);
    }
}
