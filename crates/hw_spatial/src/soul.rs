use super::grid::{GridData, SpatialGridOps};
use bevy::prelude::*;

type SpatialUpdateQuery<'w, 's, T> = Query<
    'w,
    's,
    (Entity, &'static Transform),
    (With<T>, Or<(Added<T>, Changed<Transform>)>),
>;

/// 空間グリッド - 魂位置の高速検索用
#[derive(Resource, Default)]
pub struct SpatialGrid(pub GridData);

impl SpatialGrid {
    // SpatialGrid は現状では Resource::default() で初期化されています
}

impl SpatialGridOps for SpatialGrid {
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

pub fn update_spatial_grid_system<T: Component>(
    mut grid: ResMut<SpatialGrid>,
    query: SpatialUpdateQuery<T>,
    mut removed: RemovedComponents<T>,
) {
    for (entity, transform) in query.iter() {
        grid.update(entity, transform.translation.truncate());
    }

    for entity in removed.read() {
        grid.remove(entity);
    }
}

/// `DamnedSoul` 専用のグリッド更新システム（bevy_app への re-export 用）。
pub fn update_damned_soul_spatial_grid_system(
    grid: ResMut<SpatialGrid>,
    query: SpatialUpdateQuery<hw_core::soul::DamnedSoul>,
    removed: RemovedComponents<hw_core::soul::DamnedSoul>,
) {
    update_spatial_grid_system::<hw_core::soul::DamnedSoul>(grid, query, removed);
}
