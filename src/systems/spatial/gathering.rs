use super::grid::{GridData, SpatialGridOps};
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
    fn get_nearby_in_radius_into(&self, pos: Vec2, radius: f32, out: &mut Vec<Entity>) {
        self.0.get_nearby_in_radius_into(pos, radius, out);
    }
}

pub fn update_gathering_spot_spatial_grid_system(
    mut grid: ResMut<GatheringSpotSpatialGrid>,
    query: Query<
        (Entity, &GatheringSpot),
        Or<(Added<GatheringSpot>, Changed<GatheringSpot>)>,
    >,
    mut removed: RemovedComponents<GatheringSpot>,
) {
    for (entity, spot) in query.iter() {
        grid.update(entity, spot.center);
    }
    for entity in removed.read() {
        grid.remove(entity);
    }
}
