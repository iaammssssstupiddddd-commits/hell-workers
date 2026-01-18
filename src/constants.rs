use bevy::prelude::*;

pub const TILE_SIZE: f32 = 32.0;
pub const MAP_WIDTH: i32 = 50;
pub const MAP_HEIGHT: i32 = 50;

/// 使い魔ごとの疲労閾値のデフォルト値
/// 使い魔はこの値をUIで個別に調整可能
pub const FATIGUE_THRESHOLD: f32 = 0.8;

/// モチベーション閾値: この値以上の場合、ワーカーは次のタスクを探し続ける
pub const MOTIVATION_THRESHOLD: f32 = 0.1;

/// 集会閾値: 疲労がこの値を超えると強制的に集会へ向かう（グローバル）
pub const FATIGUE_GATHERING_THRESHOLD: f32 = 0.9;

/// 怠惰行動閾値: 疲労がこの値以上になると怠惰行動を開始（グローバル）
/// 使い魔ごとの閾値とは独立して機能する
pub const FATIGUE_IDLE_THRESHOLD: f32 = 0.8;

// ============================================================
// Z軸レイヤー管理 (RenderLayers)
// ============================================================

/// 背景マップのレイヤー
pub const Z_MAP: f32 = 0.0;
/// 地面にあるアイテム（資材など）のベースレイヤー
pub const Z_ITEM: f32 = 0.1;
/// オーラや範囲表示のレイヤー（地面とキャラクターの間）
pub const Z_AURA: f32 = 0.2;
/// 障害物アイテム（木、岩など）のレイヤー
pub const Z_ITEM_OBSTACLE: f32 = 0.5;
/// 拾えるアイテム（伐採後の木材など）のレイヤー
pub const Z_ITEM_PICKUP: f32 = 0.6;
/// キャラクター（魂、使い魔）のレイヤー
pub const Z_CHARACTER: f32 = 1.0;
/// 選択インジケータやオーラのレイヤー
pub const Z_SELECTION: f32 = 2.0;
/// 作業ライン等のビジュアル効果のレイヤー
pub const Z_VISUAL_EFFECT: f32 = 3.0;
/// プログレスバー（枠）のレイヤー
pub const Z_BAR_BG: f32 = 4.0;
/// プログレスバー（中身）のレイヤー
pub const Z_BAR_FILL: f32 = 4.1;
/// 空飛ぶ文字（FloatingText）のレイヤー
pub const Z_FLOATING_TEXT: f32 = 10.0;

// ============================================================
// AI ロジック定数 - 疲労 (Fatigue)
// ============================================================

/// 作業中の疲労増加率 (毎秒)
pub const FATIGUE_WORK_RATE: f32 = 0.01;
/// 使役待機中の疲労回復率 (毎秒)
pub const FATIGUE_RECOVERY_RATE_COMMANDED: f32 = 0.01;
/// 通常待機中の疲労回復率 (毎秒)
pub const FATIGUE_RECOVERY_RATE_IDLE: f32 = 0.05;
/// やる気デバフが発生する疲労閾値
pub const FATIGUE_MOTIVATION_PENALTY_THRESHOLD: f32 = 0.9;
/// 疲労限界時のやる気減少率 (毎秒)
pub const FATIGUE_MOTIVATION_PENALTY_RATE: f32 = 0.5;
/// タスク完了時の疲労増加
pub const FATIGUE_GAIN_ON_COMPLETION: f32 = 0.1;

// ============================================================
// AI ロジック定数 - ストレス (Stress)
// ============================================================

/// 作業中のストレス増加率 (毎秒)
pub const STRESS_WORK_RATE: f32 = 0.015;
/// 集会中のストレス減少率 (毎秒)
pub const STRESS_RECOVERY_RATE_GATHERING: f32 = 0.04;
/// 通常待機中のストレス減少率 (毎秒)
pub const STRESS_RECOVERY_RATE_IDLE: f32 = 0.02;
/// ストレスブレイクダウンからの回復閾値
pub const STRESS_RECOVERY_THRESHOLD: f32 = 0.7;
/// 凍結状態からの回復閾値
pub const STRESS_FREEZE_RECOVERY_THRESHOLD: f32 = 0.9;

// ============================================================
// AI ロジック定数 - 監視 (Supervision)
// ============================================================

/// 待機中（非コマンド中）の使い魔の監視効率マルチプライヤー
pub const SUPERVISION_IDLE_MULTIPLIER: f32 = 0.4;
/// 監視によるストレス増加係数
pub const SUPERVISION_STRESS_SCALE: f32 = 0.0375;
/// 監視によるモチベーション増加係数
pub const SUPERVISION_MOTIVATION_SCALE: f32 = 4.0;
/// 監視による怠惰減少係数
pub const SUPERVISION_LAZINESS_SCALE: f32 = 2.5;

// ============================================================
// AI ロジック定数 - やる気と怠惰 (Motivation & Laziness)
// ============================================================

/// 作業・使役中のモチベーション自然減少率 (毎秒)
pub const MOTIVATION_LOSS_RATE_ACTIVE: f32 = 0.02;
/// 通常待機中のモチベーション自然減少率 (毎秒)
pub const MOTIVATION_LOSS_RATE_IDLE: f32 = 0.1;
/// 作業・使役中の怠惰減少率 (毎秒)
pub const LAZINESS_LOSS_RATE_ACTIVE: f32 = 0.1;
/// 通常待機中の怠惰増加率 (毎秒)
pub const LAZINESS_GAIN_RATE_IDLE: f32 = 0.05;

// = ==========================================================
// AI ロジック定数 - 怠惰行動 (Idle Behavior)
// ============================================================

/// 強制集会へ移行するアイドル時間 (秒)
pub const IDLE_TIME_TO_GATHERING: f32 = 30.0;
/// 怠惰行動の判定に使用する閾値
pub const LAZINESS_THRESHOLD_HIGH: f32 = 0.8;
pub const LAZINESS_THRESHOLD_MID: f32 = 0.5;
/// 集会エリアへの到着判定半径
pub const GATHERING_ARRIVAL_RADIUS_BASE: f32 = 3.0; // TILE_SIZE 倍

/// 集会中の行動変化間隔 (最小/最大)
pub const GATHERING_BEHAVIOR_DURATION_MIN: f32 = 60.0;
pub const GATHERING_BEHAVIOR_DURATION_MAX: f32 = 90.0;

/// 怠惰行動の持続時間 (最小/最大)
pub const IDLE_DURATION_SLEEP_MIN: f32 = 5.0;
pub const IDLE_DURATION_SLEEP_MAX: f32 = 10.0;
pub const IDLE_DURATION_SIT_MIN: f32 = 3.0;
pub const IDLE_DURATION_SIT_MAX: f32 = 6.0;
pub const IDLE_DURATION_WANDER_MIN: f32 = 2.0;
pub const IDLE_DURATION_WANDER_MAX: f32 = 4.0;

// ============================================================
// AI ロジック定数 - 作業 (Work)
// ============================================================

/// 資源採取の基本速度 (進捗/秒)
pub const GATHER_SPEED_BASE: f32 = 0.5;

// ============================================================
// キャラクター移動・アニメーション
// ============================================================

/// ソウルの基本移動速度
pub const SOUL_SPEED_BASE: f32 = 60.0;
/// ソウルの最低移動速度
pub const SOUL_SPEED_MIN: f32 = 20.0;
/// モチベーションによる速度ボーナス係数
pub const SOUL_SPEED_MOTIVATION_BONUS: f32 = 40.0;
/// 怠惰による速度ペナルティ係数
pub const SOUL_SPEED_LAZINESS_PENALTY: f32 = 30.0;
/// 疲労困憊時の速度デバフ倍率
pub const SOUL_SPEED_EXHAUSTED_MULTIPLIER: f32 = 0.7;

/// 移動アニメーション (Bob) の速度
pub const ANIM_BOB_SPEED: f32 = 10.0;
/// 移動アニメーション (Bob) の振幅
pub const ANIM_BOB_AMPLITUDE: f32 = 0.05;
/// 待機中呼吸アニメーションの基本速度
pub const ANIM_BREATH_SPEED_BASE: f32 = 2.0;
/// 待機中呼吸アニメーションの振幅
pub const ANIM_BREATH_AMPLITUDE: f32 = 0.02;

// ============================================================
// UI・フォント定数
// ============================================================

/// タイトル用フォントサイズ
pub const FONT_SIZE_TITLE: f32 = 24.0;
/// ヘッダー用フォントサイズ
pub const FONT_SIZE_HEADER: f32 = 20.0;
/// 本文用フォントサイズ
pub const FONT_SIZE_BODY: f32 = 16.0;
/// 小サイズテキスト用フォントサイズ
pub const FONT_SIZE_SMALL: f32 = 14.0;
/// 極小サイズテキスト用フォントサイズ
pub const FONT_SIZE_TINY: f32 = 10.0;

// ============================================================
// 吹き出しシステム (Speech Bubble)
// ============================================================

/// 吹き出しの生存時間 (秒)
pub const SPEECH_BUBBLE_DURATION: f32 = 1.5;
/// 吹き出しの話者からのオフセット
pub const SPEECH_BUBBLE_OFFSET: Vec2 = Vec2::new(16.0, 16.0);
/// Soul吹き出し（絵文字）のフォントサイズ
pub const FONT_SIZE_BUBBLE_SOUL: f32 = 24.0;
/// Familiar吹き出しのフォントサイズ
pub const FONT_SIZE_BUBBLE_FAMILIAR: f32 = 12.0;
/// 吹き出しのZレイヤー
pub const Z_SPEECH_BUBBLE: f32 = 11.0;
/// 吹き出し背景のZレイヤー
pub const Z_SPEECH_BUBBLE_BG: f32 = 10.9;

// ============================================================
// 吹き出しアニメーション (Speech Bubble Animation)
// ============================================================

/// ポップインアニメーション時間
pub const BUBBLE_ANIM_POP_IN_DURATION: f32 = 0.15;
/// ポップイン時のオーバーシュート倍率
pub const BUBBLE_ANIM_POP_IN_OVERSHOOT: f32 = 1.2;
/// ポップアウトアニメーション時間
pub const BUBBLE_ANIM_POP_OUT_DURATION: f32 = 0.3;

/// スタッキング用のオフセット（複数吹き出し時）
pub const BUBBLE_STACK_GAP: f32 = 40.0;

/// 感情別エフェクトの定数
/// 震え（Stressed）の強度
pub const BUBBLE_SHAKE_INTENSITY: f32 = 1.5;
/// 震え（Stressed）の速度
pub const BUBBLE_SHAKE_SPEED: f32 = 40.0;
/// ボブ（Exhausted）の振幅
pub const BUBBLE_BOB_AMPLITUDE: f32 = 3.0;
/// ボブ（Exhausted）の速度
pub const BUBBLE_BOB_SPEED: f32 = 4.0;

// ============================================================
// 吹き出しカラー (Speech Bubble Colors)
// ============================================================

pub const BUBBLE_COLOR_MOTIVATED: Color = Color::srgba(0.6, 1.0, 0.4, 1.0); // 黄緑
pub const BUBBLE_COLOR_HAPPY: Color = Color::srgba(1.0, 0.7, 0.8, 1.0); // ピンク
pub const BUBBLE_COLOR_EXHAUSTED: Color = Color::srgba(0.6, 0.6, 0.7, 1.0); // グレー
pub const BUBBLE_COLOR_STRESSED: Color = Color::srgba(1.0, 0.4, 0.4, 1.0); // 赤
