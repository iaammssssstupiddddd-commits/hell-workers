use bevy::prelude::*;

/// AIの思考・行動サイクルを管理するシステムセット
///
/// 1フレーム内で以下の順序で実行されることを保証する。
/// 1. Sense: 環境認識、キャッシュ更新、確定情報の処理
/// 2. Think: 次の行動決定、予約、経路計算
/// 3. Act: 行動実行、物理反映、コマンド発行
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum SoulAiSystemSet {
    /// 環境情報の収集、キャッシュの更新
    /// Queryの読み取りは「確定済みの過去」として扱う
    Sense,

    /// 次の行動の決定、タスク割り当て
    /// ここでの決定は「予約」として即時反映される
    Think,

    /// 決定された行動の実行
    /// 実際のCommands発行を行う
    Act,
}
