use super::grid::{sync_grid_timed, GridData, SpatialGridOps, SpatialGridSyncTimer, SyncGridClear};
use crate::entities::familiar::Familiar;
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

impl SyncGridClear for FamiliarSpatialGrid {
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

pub fn update_familiar_spatial_grid_system(
    mut sync_timer: ResMut<SpatialGridSyncTimer>,
    mut grid: ResMut<FamiliarSpatialGrid>,
    query: Query<(Entity, &Transform), With<Familiar>>,
) {
    sync_grid_timed(
        &mut sync_timer,
        &mut *grid,
        query.iter().map(|(e, t)| (e, t.translation.truncate())),
    );
}
