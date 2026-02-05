use bevy::prelude::*;

/// AIの思考・行動サイクルを管理するシステムセット
///
/// 1フレーム内で以下の順序で実行されることを保証する。
/// 1. Perceive: 環境認識、変化の検出、キャッシュ再構築
/// 2. Update: 時間経過による内部状態の変化（バイタル、タイマー）
/// 3. Decide: 次の行動の選択、要求の生成
/// 4. Execute: 決定された行動の実行、コマンド発行
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum AiSystemSet {
    /// 環境情報の読み取り、変化の検出
    /// - コンポーネントの読み取りのみ
    /// - キャッシュの再構築
    /// - 変化フラグの設定
    Perceive,

    /// 時間経過による内部状態の変化
    /// - バイタル更新（疲労、ストレス、やる気）
    /// - タイマー更新
    /// - メンテナンス処理
    Update,

    /// 次の行動の選択、要求の生成
    /// - TaskAssignmentRequest等の要求を生成
    /// - ReservationShadowで予約増分を追跡
    /// - 目的地の決定
    Decide,

    /// 決定された行動の実行
    /// - Commands発行
    /// - エンティティの生成/削除
    /// - イベントの発火
    /// - 予約の確定
    Execute,
}

