//! ソウル用ビジュアルシステム
//!
//! サブモジュール:
//! - `idle`: IdleBehavior ビジュアルフィードバック
//! - `gathering`: 集会オーラ・デバッグ可視化
//! - `vitals`: 使い魔ホバー線描画
//! - プログレスバー、ステータスアイコン、タスクリンク表示

pub mod gathering;
pub mod gathering_spawn;
pub mod idle;
mod systems;
pub mod vitals;

pub use systems::{
    progress_bar_system, soul_status_visual_system, sync_progress_bar_position_system,
    task_link_system, update_progress_bar_fill_system,
};

/// ソウル用プログレスバーのラッパーコンポーネント
#[derive(bevy::prelude::Component)]
pub struct SoulProgressBar;

#[derive(bevy::prelude::Component)]
pub struct StatusIcon;
