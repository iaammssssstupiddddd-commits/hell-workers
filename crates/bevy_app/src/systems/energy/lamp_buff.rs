#[cfg(feature = "profiling")]
use super::grid_recalc::EnergyPerfMetrics;
use bevy::prelude::*;
use hw_core::soul::DamnedSoul;
use hw_energy::{
    LAMP_FATIGUE_RECOVERY_BONUS, LAMP_STRESS_REDUCTION_RATE, OUTDOOR_LAMP_EFFECT_RADIUS,
    PowerConsumer, Unpowered,
};
use hw_soul_ai::soul_ai::update::slow_simulation::SlowSimulationClock;
use hw_spatial::{SpatialGrid, SpatialGridOps};

type PoweredLampQuery<'w, 's> =
    Query<'w, 's, &'static Transform, (With<PowerConsumer>, Without<Unpowered>)>;

/// 点灯中のランプ半径内にいる Soul の stress と fatigue を軽減する。
/// Unpowered ランプはスキップされるため、停電時はバフが自動停止する。
pub fn lamp_buff_system(
    q_lamps: PoweredLampQuery,
    soul_grid: Res<SpatialGrid>,
    mut candidate_souls: Local<Vec<Entity>>,
    mut q_souls: Query<(&Transform, &mut DamnedSoul)>,
    clock: Res<SlowSimulationClock>,
    #[cfg(feature = "profiling")] mut metrics: ResMut<EnergyPerfMetrics>,
) {
    let r2 = OUTDOOR_LAMP_EFFECT_RADIUS * OUTDOOR_LAMP_EFFECT_RADIUS;

    for _ in 0..clock.steps_this_frame() {
        #[cfg(feature = "profiling")]
        {
            metrics.lamp_steps = metrics.lamp_steps.saturating_add(1);
        }
        let dt = clock.step_secs();
        for lamp_tf in q_lamps.iter() {
            let lamp_pos = lamp_tf.translation.truncate();
            soul_grid.get_nearby_in_radius_into(
                lamp_pos,
                OUTDOOR_LAMP_EFFECT_RADIUS,
                &mut candidate_souls,
            );
            // The index normally has one entry per Soul. Sorting and
            // deduplicating keeps the effect deterministic even while an
            // index implementation changes its cell boundary policy.
            candidate_souls.sort_unstable_by_key(|entity| entity.to_bits());
            candidate_souls.dedup();
            #[cfg(feature = "profiling")]
            {
                metrics.lamp_candidates_scanned = metrics
                    .lamp_candidates_scanned
                    .saturating_add(candidate_souls.len() as u64);
            }

            for &soul_entity in candidate_souls.iter() {
                let Ok((soul_tf, mut soul)) = q_souls.get_mut(soul_entity) else {
                    continue;
                };
                if soul_tf.translation.truncate().distance_squared(lamp_pos) <= r2 {
                    soul.stress = (soul.stress - LAMP_STRESS_REDUCTION_RATE * dt).max(0.0);
                    soul.fatigue = (soul.fatigue - LAMP_FATIGUE_RECOVERY_BONUS * dt).max(0.0);
                }
            }
        }
    }
}
