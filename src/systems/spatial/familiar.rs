use super::grid::{GridData, SpatialGridOps};
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
    fn get_nearby_in_radius_into(&self, pos: Vec2, radius: f32, out: &mut Vec<Entity>) {
        self.0.get_nearby_in_radius_into(pos, radius, out);
    }
}

pub fn update_familiar_spatial_grid_system(
    mut grid: ResMut<FamiliarSpatialGrid>,
    query: Query<
        (Entity, &Transform),
        (With<Familiar>, Or<(Added<Familiar>, Changed<Transform>)>),
    >,
    mut removed: RemovedComponents<Familiar>,
) {
    for (entity, transform) in query.iter() {
        grid.update(entity, transform.translation.truncate());
    }
    for entity in removed.read() {
        grid.remove(entity);
    }
}
