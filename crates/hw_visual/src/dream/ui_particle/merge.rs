use bevy::prelude::*;
use hw_core::constants::{
    DREAM_UI_MERGE_DURATION, DREAM_UI_MERGE_MAX_COUNT, DREAM_UI_MERGE_MAX_MASS,
    DREAM_UI_MERGE_RADIUS,
};

use crate::dream::DreamGainUiParticle;

pub fn ui_particle_merge_system(
    mut q_particles: Query<(Entity, &mut DreamGainUiParticle, &Node)>,
    #[cfg(feature = "profiling")] mut perf_metrics: Option<
        ResMut<super::super::perf::DreamUiPerfMetrics>,
    >,
    #[cfg(feature = "profiling")] perf_particles: Query<&super::super::perf::DreamUiPerfParticle>,
    #[cfg(feature = "profiling")] perf_control: Option<Res<super::super::perf::DreamUiPerfControl>>,
) {
    #[cfg(feature = "profiling-memory")]
    if perf_metrics.is_some() {
        hw_core::profiling_alloc_scope::begin();
    }
    #[cfg(feature = "profiling")]
    let perf_started = perf_metrics.as_ref().map(|_| std::time::Instant::now());
    let positions: Vec<(Entity, Vec2, f32, bool)> = q_particles
        .iter()
        .map(|(e, p, n)| {
            let pos = Vec2::new(
                match n.left {
                    Val::Px(v) => v,
                    _ => 0.0,
                },
                match n.top {
                    Val::Px(v) => v,
                    _ => 0.0,
                },
            );
            let t = (p.time_alive / 3.5).clamp(0.0, 1.0);
            let merging = p.merging_into.is_some();
            (e, pos, t, merging)
        })
        .collect();
    #[cfg(feature = "profiling")]
    let positions = {
        let mut positions = positions;
        if perf_control
            .as_deref()
            .is_some_and(super::super::perf::DreamUiPerfControl::deterministic_order)
        {
            positions.sort_unstable_by_key(|(entity, _, _, _)| {
                perf_particles
                    .get(*entity)
                    .map_or(u32::MAX, |marker| marker.0)
            });
        }
        positions
    };

    let mut merge_pair: Option<(Entity, Entity)> = None;
    'outer: for i in 0..positions.len() {
        if positions[i].3 {
            continue;
        }
        if positions[i].2 < 0.05 {
            continue;
        }
        for j in (i + 1)..positions.len() {
            #[cfg(feature = "profiling")]
            if let Some(metrics) = perf_metrics.as_deref_mut() {
                metrics.record_merge_comparison();
            }
            if positions[j].3 {
                continue;
            }
            if positions[j].2 < 0.05 {
                continue;
            }
            let dist = positions[i].1.distance(positions[j].1);
            if dist < DREAM_UI_MERGE_RADIUS {
                if positions[i].2 < positions[j].2 {
                    merge_pair = Some((positions[i].0, positions[j].0));
                } else {
                    merge_pair = Some((positions[j].0, positions[i].0));
                }
                break 'outer;
            }
        }
    }

    if let Some((absorbed, absorber)) = merge_pair
        && let Ok([(_, mut absorbed_p, _), (_, mut absorber_p, _)]) =
            q_particles.get_many_mut([absorbed, absorber])
        && absorber_p.merge_count < DREAM_UI_MERGE_MAX_COUNT
        && absorber_p.mass <= DREAM_UI_MERGE_MAX_MASS
    {
        absorbed_p.merging_into = Some(absorber);
        absorbed_p.merge_timer = DREAM_UI_MERGE_DURATION;

        absorber_p.merge_count += 1;
        absorber_p.mass += absorbed_p.mass;
    }
    #[cfg(feature = "profiling")]
    if let (Some(metrics), Some(started)) = (perf_metrics.as_deref_mut(), perf_started) {
        metrics.record_elapsed(started);
    }
    #[cfg(feature = "profiling-memory")]
    if let Some(metrics) = perf_metrics.as_deref_mut() {
        metrics.record_scoped_allocations(hw_core::profiling_alloc_scope::end());
    }
}
