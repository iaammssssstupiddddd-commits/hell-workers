//! Soul 空間グリッド更新 facade — 実装は hw_spatial に移設済み。

pub use hw_spatial::{
    SpatialGrid, update_damned_soul_spatial_grid_system as update_spatial_grid_system,
    update_spatial_grid_system as _update_spatial_grid_system,
};
