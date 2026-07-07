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
) {
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
            velocity: bucket.velocity,
        };
        apply_ui_material_bucket(&mut mat_node, &mut bucket, desired, &handles);
    }
}
