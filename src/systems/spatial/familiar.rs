use super::grid::{GridData, SpatialGridOps};
use crate::entities::familiar::Familiar;
use crate::systems::spatial::SpatialGridSyncTimer;
use bevy::prelude::*;

/// 使い魔用の空間グリッド - モチベーション計算の高速化用
#[derive(Resource, Default)]
pub struct FamiliarSpatialGrid(pub GridData);

impl FamiliarSpatialGrid {
    // FamiliarSpatialGrid は現状では Resource::default() で初期化されています
}

impl SpatialGridOps for FamiliarSpatialGrid {
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

pub fn update_familiar_spatial_grid_system(
    mut sync_timer: ResMut<SpatialGridSyncTimer>,
    mut grid: ResMut<FamiliarSpatialGrid>,
    query: Query<(Entity, &Transform), With<Familiar>>,
) {
    let timer_finished = sync_timer.timer.just_finished();
    if sync_timer.first_run_done && !timer_finished {
        return;
    }
    sync_timer.first_run_done = true;

    grid.0.clear();
    for (entity, transform) in query.iter() {
        grid.0.insert(entity, transform.translation.truncate());
    }
}
