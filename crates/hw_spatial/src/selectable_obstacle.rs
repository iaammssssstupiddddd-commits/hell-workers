use bevy::prelude::*;
use hw_jobs::{ObstaclePosition, Rock, Tree};
use hw_world::WorldMap;

use crate::{SelectableObstacleIndexTag, SpatialGridOps, SpatialIndex};

/// Tree/Rock index used by pointer selection. It is separate from task designation indexes.
pub type SelectableObstacleSpatialGrid = SpatialIndex<SelectableObstacleIndexTag>;

type ChangedObstacleQuery<'w, 's> = Query<
    'w,
    's,
    (Entity, &'static ObstaclePosition),
    (
        Or<(With<Tree>, With<Rock>)>,
        Or<(
            Added<Tree>,
            Added<Rock>,
            Added<ObstaclePosition>,
            Changed<ObstaclePosition>,
        )>,
    ),
>;

pub fn update_selectable_obstacle_spatial_grid_system(
    mut grid: ResMut<SelectableObstacleSpatialGrid>,
    changed: ChangedObstacleQuery,
    mut removed_trees: RemovedComponents<Tree>,
    mut removed_rocks: RemovedComponents<Rock>,
    mut removed_positions: RemovedComponents<ObstaclePosition>,
) {
    for (entity, position) in &changed {
        grid.update(entity, WorldMap::grid_to_world(position.0, position.1));
    }
    for entity in removed_trees
        .read()
        .chain(removed_rocks.read())
        .chain(removed_positions.read())
    {
        grid.remove(entity);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tracks_add_move_and_remove_without_a_full_scan() {
        let mut app = App::new();
        app.init_resource::<SelectableObstacleSpatialGrid>()
            .add_systems(Update, update_selectable_obstacle_spatial_grid_system);
        let entity = app.world_mut().spawn((Tree, ObstaclePosition(2, 3))).id();
        app.update();
        assert_eq!(
            app.world()
                .resource::<SelectableObstacleSpatialGrid>()
                .get_nearby_in_radius(WorldMap::grid_to_world(2, 3), 1.0),
            vec![entity]
        );

        app.world_mut()
            .entity_mut(entity)
            .insert(ObstaclePosition(8, 9));
        app.update();
        assert!(
            app.world()
                .resource::<SelectableObstacleSpatialGrid>()
                .get_nearby_in_radius(WorldMap::grid_to_world(2, 3), 1.0)
                .is_empty()
        );

        app.world_mut().despawn(entity);
        app.update();
        assert!(
            app.world()
                .resource::<SelectableObstacleSpatialGrid>()
                .get_nearby_in_radius(WorldMap::grid_to_world(8, 9), 1.0)
                .is_empty()
        );
    }
}
