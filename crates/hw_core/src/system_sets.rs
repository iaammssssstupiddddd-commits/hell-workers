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

/// Familiar AI の思考・行動サイクルを管理するシステムセット
///
/// Soul AI より先に実行され、指揮系統の決定を行う。
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum FamiliarAiSystemSet {
    /// 環境情報の読み取り、変化の検出
    Perceive,
    /// 時間経過による内部状態の変化
    Update,
    /// 次の行動の選択、要求の生成
    Decide,
    /// 決定された行動の実行
    Execute,
}

/// Soul AI の思考・行動サイクルを管理するシステムセット
///
/// Familiar AI の後に実行され、指示に従った行動を行う。
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum SoulAiSystemSet {
    /// 環境情報の読み取り、変化の検出
    Perceive,
    /// 時間経過による内部状態の変化
    Update,
    /// 次の行動の選択、要求の生成
    Decide,
    /// 決定された行動の実行
    Execute,
}
