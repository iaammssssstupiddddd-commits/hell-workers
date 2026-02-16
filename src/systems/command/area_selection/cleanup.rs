//! Blueprint キャンセル時の WorldMap / PendingBelongsToBlueprint クリーンアップ

use crate::systems::jobs::Blueprint;
use bevy::prelude::*;

/// Blueprint が despawn された時に WorldMap と PendingBelongsToBlueprint を掃除する
pub fn blueprint_cancel_cleanup_system(
    mut commands: Commands,
    mut world_map: ResMut<crate::world::map::WorldMap>,
    mut removed: RemovedComponents<Blueprint>,
    q_pending: Query<(
        Entity,
        &crate::systems::logistics::PendingBelongsToBlueprint,
    )>,
) {
    for removed_entity in removed.read() {
        let grids_to_remove: Vec<(i32, i32)> = world_map
            .buildings
            .iter()
            .filter(|&(_, entity)| *entity == removed_entity)
            .map(|(&grid, _)| grid)
            .collect();
        for (gx, gy) in grids_to_remove {
            world_map.buildings.remove(&(gx, gy));
            world_map.remove_obstacle(gx, gy);
        }

        for (companion_entity, pending) in q_pending.iter() {
            if pending.0 == removed_entity {
                commands.entity(companion_entity).try_despawn();
            }
        }
    }
}
