use crate::constants::DREAM_UI_TRAIL_ALPHA;
use bevy::prelude::*;
use bevy::ui_render::prelude::MaterialNode;

use crate::systems::visual::dream::{DreamBubbleUiMaterial, DreamTrailGhost};

pub fn dream_trail_ghost_update_system(
    mut commands: Commands,
    time: Res<Time>,
    mut materials: ResMut<Assets<DreamBubbleUiMaterial>>,
    mut q_ghosts: Query<(
        Entity,
        &mut DreamTrailGhost,
        &MaterialNode<DreamBubbleUiMaterial>,
    )>,
) {
    let dt = time.delta_secs();
    for (entity, mut ghost, mat_node) in q_ghosts.iter_mut() {
        ghost.lifetime -= dt;
        if ghost.lifetime <= 0.0 {
            commands.entity(entity).try_despawn();
            continue;
        }
        let alpha = (ghost.lifetime / ghost.max_lifetime) * DREAM_UI_TRAIL_ALPHA;
        if let Some(mat) = materials.get_mut(&mat_node.0) {
            mat.alpha = alpha;
        }
    }
}
