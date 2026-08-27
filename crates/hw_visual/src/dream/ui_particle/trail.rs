use bevy::prelude::*;
use bevy::ui_render::prelude::MaterialNode;
use hw_core::constants::DREAM_UI_TRAIL_ALPHA;

use crate::dream::{
    DreamBubbleUiHandles, DreamBubbleUiMaterial, DreamTrailGhost, DreamUiMaterialBucket,
    alpha_to_bucket, apply_ui_material_bucket,
};

pub fn dream_trail_ghost_update_system(
    mut commands: Commands,
    time: Res<Time>,
    handles: Res<DreamBubbleUiHandles>,
    mut q_ghosts: Query<(
        Entity,
        &mut DreamTrailGhost,
        &mut DreamUiMaterialBucket,
        &mut MaterialNode<DreamBubbleUiMaterial>,
    )>,
    #[cfg(feature = "profiling")] mut perf_metrics: Option<
        ResMut<super::super::perf::DreamUiPerfMetrics>,
    >,
) {
    #[cfg(feature = "profiling-memory")]
    if perf_metrics.is_some() {
        hw_core::profiling_alloc_scope::begin();
    }
    #[cfg(feature = "profiling")]
    let perf_started = perf_metrics.as_ref().map(|_| std::time::Instant::now());
    let dt = time.delta_secs();
    for (entity, mut ghost, mut bucket, mut mat_node) in q_ghosts.iter_mut() {
        ghost.lifetime -= dt;
        if ghost.lifetime <= 0.0 {
            commands.entity(entity).try_despawn();
            continue;
        }
        let alpha = (ghost.lifetime / ghost.max_lifetime) * DREAM_UI_TRAIL_ALPHA;
        let desired = DreamUiMaterialBucket {
            alpha: alpha_to_bucket(alpha),
            mass: bucket.mass,
            color: bucket.color,
        };
        apply_ui_material_bucket(&mut mat_node, &mut bucket, desired, &handles);
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
