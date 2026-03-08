use bevy::prelude::*;

/// 空間グリッドの共通操作を定義するトレイト
pub trait SpatialGridOps {
    fn insert(&mut self, entity: Entity, pos: Vec2);
    fn remove(&mut self, entity: Entity);
    fn update(&mut self, entity: Entity, pos: Vec2);
    fn get_nearby_in_radius(&self, pos: Vec2, radius: f32) -> Vec<Entity>;
    fn get_nearby_in_radius_into(&self, pos: Vec2, radius: f32, out: &mut Vec<Entity>);
}
