//! Familiar 空間グリッド更新 facade — 実装は hw_spatial に移設済み。

pub use hw_spatial::{
    FamiliarSpatialGrid,
    update_familiar_entity_spatial_grid_system as update_familiar_spatial_grid_system,
    update_familiar_spatial_grid_system as _update_familiar_spatial_grid_system,
};
