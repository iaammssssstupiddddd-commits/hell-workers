use bevy::prelude::*;
use hw_core::{WorldEpoch, soul::DamnedSoul};
use hw_energy::{LAMP_FATIGUE_RECOVERY_BONUS, LAMP_STRESS_REDUCTION_RATE};
use hw_infra::lighting::LightGridPos;
use hw_soul_ai::soul_ai::update::slow_simulation::SlowSimulationClock;
use hw_world::WorldMap;

use crate::systems::lighting::{
    IndoorLightConsumerMetrics, IndoorLightRuntime, IndoorLightingLifecycleProbe,
    read_indoor_light_snapshot,
};

pub fn apply_light_recovery_effect_system(
    clock: Res<SlowSimulationClock>,
    runtime: Res<IndoorLightRuntime>,
    world_epoch: Res<WorldEpoch>,
    mut lifecycle_probe: ResMut<IndoorLightingLifecycleProbe>,
    mut metrics: ResMut<IndoorLightConsumerMetrics>,
    mut souls: Query<(&Transform, &mut DamnedSoul)>,
) {
    let steps = clock.steps_this_frame();
    if steps == 0 {
        return;
    }

    metrics.recovery_updates = metrics.recovery_updates.saturating_add(1);
    let Some(snapshot) =
        read_indoor_light_snapshot(&runtime, *world_epoch, *world_epoch, &mut lifecycle_probe)
    else {
        metrics.unavailable_recovery_updates =
            metrics.unavailable_recovery_updates.saturating_add(1);
        return;
    };
    let snapshot_epoch_is_current = runtime.published_epoch() == Some(world_epoch.get());

    let dt = clock.step_secs();
    for _ in 0..steps {
        metrics.recovery_steps = metrics.recovery_steps.saturating_add(1);
        for (transform, mut soul) in &mut souls {
            metrics.soul_samples = metrics.soul_samples.saturating_add(1);
            let grid = WorldMap::world_to_grid(transform.translation.truncate());
            let light_pos = LightGridPos::new(grid.0, grid.1);
            let Some(luminance) = snapshot.sample_luminance(light_pos) else {
                metrics.out_of_bounds_samples = metrics.out_of_bounds_samples.saturating_add(1);
                continue;
            };
            if luminance == 0 {
                metrics.dark_samples = metrics.dark_samples.saturating_add(1);
                continue;
            }

            soul.stress = (soul.stress - LAMP_STRESS_REDUCTION_RATE * dt).max(0.0);
            soul.fatigue = (soul.fatigue - LAMP_FATIGUE_RECOVERY_BONUS * dt).max(0.0);
            metrics.recovery_effects = metrics.recovery_effects.saturating_add(1);
            if !snapshot_epoch_is_current {
                metrics.old_epoch_recovery_effects =
                    metrics.old_epoch_recovery_effects.saturating_add(1);
            }
        }
    }
}
