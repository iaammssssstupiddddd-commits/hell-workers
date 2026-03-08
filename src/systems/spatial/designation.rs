use bevy::prelude::*;
use crate::systems::jobs::Designation;

/// タスク（Designation）用の空間グリッド
pub use hw_spatial::DesignationSpatialGrid;
pub use hw_spatial::update_designation_spatial_grid_system as _update_designation_spatial_grid_system;

/// 変更差分のみを反映する。スポーン直後は次フレームで取り込まれる。
pub fn update_designation_spatial_grid_system(
    grid: ResMut<DesignationSpatialGrid>,
    query: Query<
        (Entity, &Transform),
        (With<Designation>, Or<(Added<Designation>, Changed<Transform>)>),
    >,
    removed: RemovedComponents<Designation>,
) {
    _update_designation_spatial_grid_system::<Designation>(grid, query, removed);
}
