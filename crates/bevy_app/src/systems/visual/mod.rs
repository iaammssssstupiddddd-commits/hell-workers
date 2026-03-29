//! ビジュアルシステム
//!
//! ゲーム内の視覚的フィードバックを管理するモジュール群
//!
//! hw_visual クレートに移行済みのサブシステムは hw_visual::* を直接参照すること。
//! root 残留ファイルは app_contexts / root 専有型への依存によるもの。

pub mod building3d_cleanup;
pub mod camera_sync;
pub mod character_proxy_3d;
pub mod elevation_view;
pub mod soul_animation;
pub mod wall_orientation_aid;
pub mod floor_construction {
    pub use hw_visual::floor_construction::{
        FloorCuringProgressBar, FloorTileBoneVisual, manage_floor_curing_progress_bars_system,
        sync_floor_tile_bone_visuals_system, update_floor_curing_progress_bars_system,
        update_floor_tile_visuals_system,
    };
}
pub mod placement_ghost;
pub mod section_cut;
pub mod task_area_visual;
pub mod wall_construction {
    pub use hw_visual::wall_construction::{
        WallConstructionProgressBar, manage_wall_progress_bars_system,
        update_wall_progress_bars_system, update_wall_tile_visuals_system,
    };
}

pub use task_area_visual::TaskAreaMaterial;
