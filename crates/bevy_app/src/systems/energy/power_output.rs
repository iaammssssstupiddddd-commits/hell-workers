use bevy::prelude::*;
use hw_core::relationships::TaskWorkers;
use hw_energy::{PowerGenerator, SoulSpaPhase, SoulSpaSite, SoulSpaTile};

/// Operational SoulSpaSite の稼働タイル数から `PowerGenerator.current_output` を更新する。
/// FixedUpdate で実行し、Phase 1c の電力グリッド集計が `Changed<PowerGenerator>` を検知する。
pub fn soul_spa_power_output_system(
    mut q_sites: Query<(&SoulSpaSite, &Children, &mut PowerGenerator)>,
    q_tiles: Query<(Option<&TaskWorkers>,), With<SoulSpaTile>>,
) {
    for (site, children, mut generator) in q_sites.iter_mut() {
        if site.phase != SoulSpaPhase::Operational {
            if generator.current_output != 0.0 {
                generator.current_output = 0.0;
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
        }
    }
}
