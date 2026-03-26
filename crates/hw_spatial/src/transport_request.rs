use super::grid::{GridData, SpatialGridOps};
use bevy::prelude::*;

type SpatialUpdateQuery<'w, 's, T> =
    Query<'w, 's, (Entity, &'static Transform), (With<T>, Or<(Added<T>, Changed<Transform>)>)>;

/// TransportRequest 用の空間グリッド
#[derive(Resource, Default)]
pub struct TransportRequestSpatialGrid(pub GridData);

impl TransportRequestSpatialGrid {
    pub fn get_in_area(&self, min: Vec2, max: Vec2) -> Vec<Entity> {
        self.0.get_in_area(min, max)
    }
}

impl SpatialGridOps for TransportRequestSpatialGrid {
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

pub fn update_transport_request_spatial_grid_system<T: Component>(
    mut grid: ResMut<TransportRequestSpatialGrid>,
    query: SpatialUpdateQuery<T>,
    mut removed: RemovedComponents<T>,
) {
    // 変更差分のみを反映する。スポーン直後は次フレームで取り込まれる。
    for (entity, transform) in query.iter() {
        grid.update(entity, transform.translation.truncate());
    }
    for entity in removed.read() {
        grid.remove(entity);
    }
}
