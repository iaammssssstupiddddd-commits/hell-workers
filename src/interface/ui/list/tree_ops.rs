//! ツリー操作（clear_children 等）

use bevy::prelude::*;

pub(crate) fn clear_children(
    commands: &mut Commands,
    q_children: &Query<&Children>,
    parent: Entity,
) {
    if let Ok(children) = q_children.get(parent) {
        for child in children.iter() {
            despawn_with_children(commands, q_children, child);
        }
    }
}

fn despawn_with_children(commands: &mut Commands, q_children: &Query<&Children>, entity: Entity) {
    if let Ok(children) = q_children.get(entity) {
        for child in children.iter() {
            despawn_with_children(commands, q_children, child);
        }
    }
    commands.entity(entity).despawn();
}
