//! Blueprint キャンセル時の WorldMap / PendingBelongsToBlueprint クリーンアップ

use crate::systems::jobs::Blueprint;
use crate::world::map::WorldMapWrite;
use bevy::prelude::*;
use hw_core::relationships::StoredIn;

/// Blueprint が despawn された時に WorldMap と PendingBelongsToBlueprint を掃除する
pub fn blueprint_cancel_cleanup_system(
    mut commands: Commands,
    mut world_map: WorldMapWrite,
    mut removed: RemovedComponents<Blueprint>,
    q_pending: Query<(
        Entity,
        &crate::systems::logistics::PendingBelongsToBlueprint,
    )>,
    q_stored_items: Query<(Entity, &StoredIn)>,
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
                let grids: Vec<_> = world_map
                    .stockpile_entries()
                    .filter_map(|(&grid, &owner)| (owner == companion_entity).then_some(grid))
                    .collect();
                for grid in grids {
                    world_map.clear_stockpile_tile_if_owned(grid, companion_entity);
                }
                for (item_entity, stored_in) in &q_stored_items {
                    if stored_in.0 == companion_entity {
                        commands
                            .entity(item_entity)
                            .remove::<StoredIn>()
                            .try_insert(Visibility::Visible);
                    }
                }
                commands.entity(companion_entity).try_despawn();
            }
        }
    }
}
