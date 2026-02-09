//! ビジュアルシステム
//!
//! ゲーム内の視覚的フィードバックを管理するモジュール群

pub mod blueprint;
pub mod gather;
pub mod haul;
pub mod mud_mixer;
pub mod placement_ghost;
pub mod soul;
pub mod speech;
pub mod tank;
pub mod wall_connection;
pub mod task_area_visual;

pub use task_area_visual::TaskAreaMaterial;
