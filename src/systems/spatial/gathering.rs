use super::grid::{GridData, SpatialGridOps};
use crate::systems::soul_ai::helpers::gathering::GatheringSpot;
use crate::systems::spatial::SpatialGridSyncTimer;
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

pub fn update_gathering_spot_spatial_grid_system(
    mut sync_timer: ResMut<SpatialGridSyncTimer>,
    mut grid: ResMut<GatheringSpotSpatialGrid>,
    query: Query<(Entity, &GatheringSpot)>,
) {
    let timer_finished = sync_timer.timer.just_finished();
    if sync_timer.first_run_done && !timer_finished {
        return;
    }
    sync_timer.first_run_done = true;

    grid.0.clear();
    for (entity, spot) in query.iter() {
        grid.0.insert(entity, spot.center);
    }
}
