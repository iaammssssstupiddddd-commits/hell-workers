//! Blueprint キャンセル時の WorldMap / PendingBelongsToBlueprint クリーンアップ

use crate::systems::jobs::Blueprint;
use crate::world::map::WorldMapWrite;
use bevy::prelude::*;

/// Blueprint が despawn された時に WorldMap と PendingBelongsToBlueprint を掃除する
pub fn blueprint_cancel_cleanup_system(
    mut commands: Commands,
    mut world_map: WorldMapWrite,
    mut removed: RemovedComponents<Blueprint>,
    q_pending: Query<(
        Entity,
        &crate::systems::logistics::PendingBelongsToBlueprint,
    )>,
) {
    for removed_entity in removed.read() {
        let grids_to_remove: Vec<(i32, i32)> = world_map
            .building_entries()
            .filter(|&(_, entity)| *entity == removed_entity)
            .map(|(&grid, _)| grid)
            .collect();
        for (gx, gy) in grids_to_remove {
            world_map.clear_building_occupancy((gx, gy));
        }

        for (companion_entity, pending) in q_pending.iter() {
            if pending.0 == removed_entity {
                commands.entity(companion_entity).try_despawn();
            }
        }
    }
}
