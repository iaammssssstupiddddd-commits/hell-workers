use super::grid::{GridData, SpatialGridOps};
use bevy::prelude::*;
use hw_jobs::model::Blueprint;

/// ブループリント用の空間グリッド
#[derive(Resource, Default)]
pub struct BlueprintSpatialGrid(pub GridData);

impl BlueprintSpatialGrid {
    /// 指定範囲内のブループリントを取得（TaskAreaとの連携用）
    pub fn get_in_area(&self, min: Vec2, max: Vec2) -> Vec<Entity> {
        self.0.get_in_area(min, max)
    }
}

impl SpatialGridOps for BlueprintSpatialGrid {
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

pub fn update_blueprint_spatial_grid_system<T: Component>(
    mut grid: ResMut<BlueprintSpatialGrid>,
    query: Query<(Entity, &Transform), (With<T>, Or<(Added<T>, Changed<Transform>)>)>,
    mut removed: RemovedComponents<T>,
) {
    for (entity, transform) in query.iter() {
        grid.update(entity, transform.translation.truncate());
    }
    for entity in removed.read() {
        grid.remove(entity);
    }
}

/// `Blueprint` コンポーネントに特化した空間グリッド更新システム。
pub fn update_blueprint_spatial_grid_system_blueprint(
    grid: ResMut<BlueprintSpatialGrid>,
    query: Query<
        (Entity, &Transform),
        (With<Blueprint>, Or<(Added<Blueprint>, Changed<Transform>)>),
    >,
    removed: RemovedComponents<Blueprint>,
) {
    update_blueprint_spatial_grid_system::<Blueprint>(grid, query, removed);
}
