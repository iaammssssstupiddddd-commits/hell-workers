use super::grid::{sync_grid_timed, GridData, SpatialGridOps, SpatialGridSyncTimer, SyncGridClear};
use crate::systems::soul_ai::helpers::gathering::GatheringSpot;
use bevy::prelude::*;

/// 集会スポット用の空間グリッド
#[derive(Resource, Default)]
pub struct GatheringSpotSpatialGrid(pub GridData);

impl SpatialGridOps for GatheringSpotSpatialGrid {
    fn insert(&mut self, entity: Entity, pos: Vec2) {
        self.0.insert(entity, pos);
    }
    fn remove(&mut self, entity: Entity) {
        self.0.remove(entity);
    }
    fn update(&mut self, entity: Entity, pos: Vec2) {
        self.0.update(entity, pos);
    }
    fn get_nearby_in_radius(&self, pos: Vec2, radius: f32) -> Vec<Entity> {
        self.0.get_nearby_in_radius(pos, radius)
    }
}

impl SyncGridClear for GatheringSpotSpatialGrid {
    fn clear_and_sync<I>(&mut self, entities: I)
    where
        I: Iterator<Item = (Entity, Vec2)>,
    {
        self.0.clear();
        for (entity, pos) in entities {
            self.0.insert(entity, pos);
        }
    }
}

pub fn update_gathering_spot_spatial_grid_system(
    mut sync_timer: ResMut<SpatialGridSyncTimer>,
    mut grid: ResMut<GatheringSpotSpatialGrid>,
    query: Query<(Entity, &GatheringSpot)>,
) {
    sync_grid_timed(
        &mut sync_timer,
        &mut *grid,
        query.iter().map(|(e, s)| (e, s.center)),
    );
}
