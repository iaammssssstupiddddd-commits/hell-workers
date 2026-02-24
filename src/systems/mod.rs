pub mod command;
pub mod dream_tree_planting;
pub mod familiar_ai;
pub mod jobs;
pub mod logistics;
pub mod obstacle;
pub mod room;
pub mod soul_ai;
pub mod spatial;
pub mod time;
pub mod utils;
pub mod visual;

use bevy::prelude::*;

/// ゲームシステムの実行順序を制御するセット
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum GameSystemSet {
    /// 入力およびカメラの更新
    Input,
    /// UI・エンティティ選択・インタラクション
    Interface,
    /// 空間グリッドの更新 (最優先のデータ更新)
    Spatial,
    /// AI・タスク管理・リソース配分などのコアロジック
    Logic,
    /// エンティティの移動・アニメーション (ロジックに基づく実際のアクション)
    Actor,
    /// 視覚的な同期処理 (移動完了後の描画追従)
    Visual,
}
