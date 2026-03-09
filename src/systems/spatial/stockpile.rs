use crate::systems::logistics::Stockpile;
use bevy::prelude::*;

/// ストックパイル用の空間グリッド
pub use hw_spatial::StockpileSpatialGrid;
pub use hw_spatial::update_stockpile_spatial_grid_system as _update_stockpile_spatial_grid_system;

pub fn update_stockpile_spatial_grid_system(
    grid: ResMut<StockpileSpatialGrid>,
    query: Query<
        (Entity, &Transform),
        (With<Stockpile>, Or<(Added<Stockpile>, Changed<Transform>)>),
    >,
    removed: RemovedComponents<Stockpile>,
) {
    _update_stockpile_spatial_grid_system::<Stockpile>(grid, query, removed);
}
