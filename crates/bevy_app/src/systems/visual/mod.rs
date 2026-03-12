//! ビジュアルシステム
//!
//! ゲーム内の視覚的フィードバックを管理するモジュール群
//!
//! hw_visual クレートに移行済みのサブシステムは hw_visual::* を直接参照すること。
//! root 残留ファイルは app_contexts / root 専有型への依存によるもの。

pub mod floor_construction;
pub mod placement_ghost;
pub mod task_area_visual;
pub mod wall_construction;

pub use task_area_visual::TaskAreaMaterial;
