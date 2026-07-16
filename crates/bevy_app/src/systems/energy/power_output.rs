use bevy::prelude::*;
use hw_core::relationships::TaskWorkers;
use hw_energy::{PowerGenerator, SoulSpaPhase, SoulSpaSite, SoulSpaTile};

#[cfg(feature = "profiling")]
use super::grid_recalc::EnergyPerfMetrics;
use super::grid_recalc::EnergyUpdateDirty;

/// Operational SoulSpaSite の稼働タイル数から `PowerGenerator.current_output` を更新する。
/// FixedUpdate で実行し、Phase 1c の電力グリッド集計が `Changed<PowerGenerator>` を検知する。
pub fn soul_spa_power_output_system(
    mut q_sites: Query<(&SoulSpaSite, &Children, &mut PowerGenerator)>,
    q_tiles: Query<(Option<&TaskWorkers>,), With<SoulSpaTile>>,
    mut dirty: ResMut<EnergyUpdateDirty>,
    #[cfg(feature = "profiling")] mut metrics: ResMut<EnergyPerfMetrics>,
) {
    #[cfg(feature = "profiling")]
    {
        metrics.power_output_runs = metrics.power_output_runs.saturating_add(1);
    }
    let mut output_changed = false;
    for (site, children, mut generator) in q_sites.iter_mut() {
        if site.phase != SoulSpaPhase::Operational {
            if generator.current_output != 0.0 {
                generator.current_output = 0.0;
                output_changed = true;
            }
            continue;
        }

        let active_count = children
            .iter()
            .filter(|&child| {
                q_tiles
                    .get(child)
                    .is_ok_and(|(workers_opt,)| workers_opt.is_some_and(|w| !w.is_empty()))
            })
            .count() as f32;

        let new_output = active_count * generator.output_per_soul;
        if (generator.current_output - new_output).abs() > f32::EPSILON {
            generator.current_output = new_output;
            output_changed = true;
        }
    }
    dirty.power_output_due = false;
    // `Changed<PowerGenerator>` is not observable until the following frame.
    // Propagate an actual output write through this frame's ordered pipeline
    // instead of leaving the grid temporarily stale.
    dirty.grid_recalc_due |= output_changed;
}
