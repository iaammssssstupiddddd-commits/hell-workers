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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::systems::jobs::{Building, BuildingType};
    use hw_jobs::BuildingCompletedEvent;
    use hw_world::WorldMap;

    #[test]
    fn passable_completion_owner_survives_removed_blueprint_cleanup() {
        let mut app = App::new();
        app.init_resource::<WorldMap>()
            .add_observer(hw_soul_ai::soul_ai::building_completed::on_building_completed)
            .add_systems(Update, blueprint_cancel_cleanup_system);

        let grid = (14, 15);
        let blueprint = app
            .world_mut()
            .spawn(Blueprint::new(BuildingType::OutdoorLamp, vec![grid]))
            .id();
        let building = app
            .world_mut()
            .spawn(Building {
                kind: BuildingType::OutdoorLamp,
                is_provisional: false,
            })
            .id();
        app.world_mut()
            .resource_mut::<WorldMap>()
            .reserve_building_footprint(BuildingType::OutdoorLamp, blueprint, [grid]);

        app.world_mut().trigger(BuildingCompletedEvent {
            building_entity: building,
            kind: BuildingType::OutdoorLamp,
            occupied_grids: vec![grid],
        });
        assert_eq!(
            app.world().resource::<WorldMap>().building_entity(grid),
            Some(building)
        );
        assert!(
            !app.world()
                .resource::<WorldMap>()
                .has_raw_obstacle(grid.0, grid.1)
        );

        app.world_mut().despawn(blueprint);
        app.update();

        assert_eq!(
            app.world().resource::<WorldMap>().building_entity(grid),
            Some(building)
        );
    }
}
