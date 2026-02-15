use bevy::prelude::*;

pub const TILE_SIZE: f32 = 32.0;
pub const MAP_WIDTH: i32 = 100;
pub const MAP_HEIGHT: i32 = 100;

/// 使い魔ごとの疲労閾値のデフォルト値
/// 使い魔はこの値をUIで個別に調整可能
pub const FATIGUE_THRESHOLD: f32 = 0.8;

/// モチベーション閾値: この値以上の場合、ワーカーは次のタスクを探し続ける
pub const MOTIVATION_THRESHOLD: f32 = 0.3;

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
/// 地形境界オーバーレイ: Sand（Riverの上）
pub const Z_MAP_SAND: f32 = 0.01;
/// 地形境界オーバーレイ: Dirt（Sandの上）
pub const Z_MAP_DIRT: f32 = 0.02;
/// 地形境界オーバーレイ: Grass（最高優先度）
pub const Z_MAP_GRASS: f32 = 0.03;
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
pub const STRESS_WORK_RATE: f32 = 0.005;
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
pub const SUPERVISION_STRESS_SCALE: f32 = 0.01;
/// 監視によるモチベーション増加係数
pub const SUPERVISION_MOTIVATION_SCALE: f32 = 0.4;
/// 監視による怠惰減少係数
pub const SUPERVISION_LAZINESS_SCALE: f32 = 2.5;

// ============================================================
// AI ロジック定数 - やる気と怠惰 (Motivation & Laziness)
// ============================================================

/// 作業・使役中のモチベーション自然減少率 (毎秒)
pub const MOTIVATION_LOSS_RATE_ACTIVE: f32 = 0.05;
/// 通常待機中のモチベーション自然減少率 (毎秒)
pub const MOTIVATION_LOSS_RATE_IDLE: f32 = 0.1;
/// 作業・使役中の怠惰減少率 (毎秒)
pub const LAZINESS_LOSS_RATE_ACTIVE: f32 = 0.1;
/// 通常待機中の怠惰増加率 (毎秒)
pub const LAZINESS_GAIN_RATE_IDLE: f32 = 0.05;

/// タスク完了時のモチベーション回復量
pub const MOTIVATION_BONUS_GATHER: f32 = 0.02;
pub const MOTIVATION_BONUS_HAUL: f32 = 0.01;
pub const MOTIVATION_BONUS_BUILD: f32 = 0.05;

/// Soul会話（サボり）によるモチベーションペナルティ
pub const MOTIVATION_PENALTY_CONVERSATION: f32 = 0.02;

/// 激励システム
pub const ENCOURAGEMENT_INTERVAL_MIN: f32 = 5.0;
pub const ENCOURAGEMENT_INTERVAL_MAX: f32 = 10.0;

pub const ENCOURAGEMENT_COOLDOWN: f32 = 30.0;
pub const ENCOURAGEMENT_MOTIVATION_BONUS: f32 = 0.025;
pub const ENCOURAGEMENT_STRESS_PENALTY: f32 = 0.0125;

/// リクルート時のバイタル変化
pub const RECRUIT_MOTIVATION_BONUS: f32 = 0.3;
pub const RECRUIT_STRESS_PENALTY: f32 = 0.1;

/// 激励用絵文字セット
pub const EMOJIS_ENCOURAGEMENT: &[&str] = &["👊", "💪", "📢", "⚡", "🔥"];

// = ==========================================================
// AI ロジック定数 - 怠惰行動 (Idle Behavior)
// ============================================================

/// 強制集会へ移行するアイドル時間 (秒)
pub const IDLE_TIME_TO_GATHERING: f32 = 30.0;
/// 怠惰行動の判定に使用する閾値
pub const LAZINESS_THRESHOLD_HIGH: f32 = 0.8;
pub const LAZINESS_THRESHOLD_MID: f32 = 0.5;
/// 集会エリアへの到着判定半径
pub const GATHERING_ARRIVAL_RADIUS_BASE: f32 = 5.0; // TILE_SIZE 倍 (この範囲に入れば集会参加とみなす)
pub const GATHERING_KEEP_DISTANCE_MIN: f32 = 3.0; // 中心から最低限離れる距離（オブジェクトサイズ1.5タイル+バッファ）
pub const GATHERING_KEEP_DISTANCE_TARGET_MIN: f32 = 3.0; // 移動先の最小距離 (バッファ込)
pub const GATHERING_KEEP_DISTANCE_TARGET_MAX: f32 = 4.5; // 移動先の最大距離

/// 集会中の行動変化間隔 (最小/最大)
pub const GATHERING_BEHAVIOR_DURATION_MIN: f32 = 10.0;
pub const GATHERING_BEHAVIOR_DURATION_MAX: f32 = 20.0;

/// 怠惰行動の持続時間 (最小/最大)
pub const IDLE_DURATION_SLEEP_MIN: f32 = 5.0;
pub const IDLE_DURATION_SLEEP_MAX: f32 = 10.0;
pub const IDLE_DURATION_SIT_MIN: f32 = 3.0;
pub const IDLE_DURATION_SIT_MAX: f32 = 6.0;
pub const IDLE_DURATION_WANDER_MIN: f32 = 2.0;
pub const IDLE_DURATION_WANDER_MAX: f32 = 4.0;

// ============================================================
// AI ロジック定数 - 逃走システム (Escape System)
// ============================================================

/// 逃走を開始する距離（command_radiusの何倍か）
pub const ESCAPE_TRIGGER_DISTANCE_MULTIPLIER: f32 = 1.5;
/// 逃走を終了する距離（command_radiusの何倍か）
pub const ESCAPE_SAFE_DISTANCE_MULTIPLIER: f32 = 2.0;
/// 逃走時のスピード倍率
pub const ESCAPE_SPEED_MULTIPLIER: f32 = 1.3;
/// 逃走を開始するストレス閾値
pub const ESCAPE_STRESS_THRESHOLD: f32 = 0.3;
/// 警戒圏内でのストレス増加率 (毎秒)
pub const ESCAPE_PROXIMITY_STRESS_RATE: f32 = 0.005;
/// Escaping状態のSoulの集会参加距離（通常より遠くから参加可能）
pub const ESCAPE_GATHERING_JOIN_RADIUS: f32 = TILE_SIZE * 7.5;
/// 逃走検出システムの実行間隔（秒）
pub const ESCAPE_DETECTION_INTERVAL: f32 = 0.5;
/// 逃走中行動（A*再評価）の実行間隔（秒）
pub const ESCAPE_BEHAVIOR_INTERVAL: f32 = 0.5;

// ============================================================
// AI ロジック定数 - スケーラビリティ最適化
// ============================================================

/// Familiar のタスク委譲システム実行間隔（秒）
pub const FAMILIAR_TASK_DELEGATION_INTERVAL: f32 = 0.5;
/// 予約キャッシュ同期システム実行間隔（秒）
pub const RESERVATION_SYNC_INTERVAL: f32 = 0.2;
/// 空間グリッド（Designation/Familiar等）の同期間隔（秒）
pub const SPATIAL_GRID_SYNC_INTERVAL: f32 = 0.15;

// ============================================================
// AI ロジック定数 - 作業 (Work)
// ============================================================

/// 資源採取の基本速度 (進捗/秒)
pub const GATHER_SPEED_BASE: f32 = 0.5;
/// 岩採掘の速度倍率（基本速度に対して）- 2倍の時間がかかる
pub const GATHER_SPEED_ROCK_MULTIPLIER: f32 = 0.5;

/// 伐採報酬: 木1本あたりのWood数
pub const WOOD_DROP_AMOUNT: u32 = 5;
/// 採掘報酬: 岩1つあたりのRock数
pub const ROCK_DROP_AMOUNT: u32 = 10;
/// バケツ一度に汲める・運べる水の量
pub const BUCKET_CAPACITY: u32 = 5;

// ============================================================
// AI ロジック定数 - 床建築 (Floor Construction)
// ============================================================

/// 床建築の最大選択サイズ（タイル数）
pub const FLOOR_MAX_AREA_SIZE: i32 = 10;
/// 1 タイルあたりに必要な Bone 数
pub const FLOOR_BONES_PER_TILE: u32 = 2;
/// 1 タイルあたりに必要な StasisMud 数
pub const FLOOR_MUD_PER_TILE: u32 = 1;
/// 補強フェーズの所要時間（秒）
pub const FLOOR_REINFORCE_DURATION_SECS: f32 = 3.0;
/// 打設フェーズの所要時間（秒）
pub const FLOOR_POUR_DURATION_SECS: f32 = 2.0;
/// 床建築関連タスクの優先度
pub const FLOOR_CONSTRUCTION_PRIORITY: u32 = 10;

// ============================================================
// AI ロジック定数 - 精製 (Refining)
// ============================================================

/// 砂採取報酬: 1回あたりのSand数
pub const SAND_DROP_AMOUNT: u32 = 1;
pub const BONE_DROP_AMOUNT: u32 = 1;
/// 精製出力: 1レシピあたりのStasisMud数
pub const STASIS_MUD_OUTPUT: u32 = 5;
/// MudMixer の原料最大保存数
pub const MUD_MIXER_CAPACITY: u32 = 5;
/// MudMixer の mud 最大保存数
pub const MUD_MIXER_MUD_CAPACITY: u32 = 10;
/// Wall の本設化に必要な StasisMud 数
pub const _STASIS_MUD_REQUIREMENT: u32 = 1;

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
/// 手押し車使用時の速度デバフ倍率
pub const SOUL_SPEED_WHEELBARROW_MULTIPLIER: f32 = 0.7;
/// 手押し車の最大積載数（混載含む合計）
pub const WHEELBARROW_CAPACITY: usize = 10;
/// 手押し車の追従オフセット距離
pub const WHEELBARROW_OFFSET: f32 = TILE_SIZE * 0.5;
/// 手押し車使用の最小アイテム数（これ以上あれば手押し車を使う）
pub const WHEELBARROW_MIN_BATCH_SIZE: usize = 3;
/// 猫車必須資源で優先する最小バッチサイズ（これ未満は一定時間待機してから許可）
pub const WHEELBARROW_PREFERRED_MIN_BATCH_SIZE: usize = 3;
/// 猫車必須資源で 1〜2 個搬送を許可するまでの待機秒数
pub const SINGLE_BATCH_WAIT_SECS: f64 = 5.0;
/// 手押し車リースの有効期間（秒）
pub const WHEELBARROW_LEASE_DURATION_SECS: f64 = 30.0;
/// 仲裁スコア: バッチサイズの重み
pub const WHEELBARROW_SCORE_BATCH_SIZE: f32 = 10.0;
/// 仲裁スコア: 優先度の重み
pub const WHEELBARROW_SCORE_PRIORITY: f32 = 5.0;
/// 仲裁スコア: 距離のペナルティ重み
pub const WHEELBARROW_SCORE_DISTANCE: f32 = 0.1;
/// 仲裁スコア: 1〜2 個バッチに対する減点
pub const WHEELBARROW_SCORE_SMALL_BATCH_PENALTY: f32 = 20.0;
/// 猫車仲裁で request ごとに評価する近傍候補の上限件数
pub const WHEELBARROW_ARBITRATION_TOP_K: usize = 24;
/// 運搬中の手押し車のスケール倍率（駐車時は1.0、運搬中は大きく表示）
pub const WHEELBARROW_ACTIVE_SCALE: f32 = 1.8;

/// 移動アニメーション (Bob) の振幅
pub const ANIM_BOB_AMPLITUDE: f32 = 0.05;
/// Familiar 移動時のスプライト切り替え速度（FPS）
pub const FAMILIAR_MOVE_ANIMATION_FPS: f32 = 5.0;
/// Familiar 移動アニメーションのフレーム数
pub const FAMILIAR_MOVE_ANIMATION_FRAMES: usize = 4;
/// Familiar 浮遊アニメーションの速度係数
pub const FAMILIAR_HOVER_SPEED: f32 = 2.8;
/// Familiar の上下浮遊量（Idle時）
pub const FAMILIAR_HOVER_AMPLITUDE_IDLE: f32 = 4.5;
/// Familiar の上下浮遊量（移動時）
pub const FAMILIAR_HOVER_AMPLITUDE_MOVE: f32 = 3.0;
/// Familiar の傾き振幅（ラジアン）
pub const FAMILIAR_HOVER_TILT_AMPLITUDE: f32 = 0.03;
/// Soul 浮遊の左右スウェイ速度
pub const SOUL_FLOAT_SWAY_SPEED: f32 = 2.4;
/// Soul 浮遊の回転角（待機時）
pub const SOUL_FLOAT_SWAY_TILT_IDLE: f32 = 0.06;
/// Soul 浮遊の回転角（移動時）
pub const SOUL_FLOAT_SWAY_TILT_MOVE: f32 = 0.12;
/// Soul 浮遊の脈動速度の基準値
pub const SOUL_FLOAT_PULSE_SPEED_BASE: f32 = 2.2;
/// Soul 浮遊の脈動振幅（待機時）
pub const SOUL_FLOAT_PULSE_AMPLITUDE_IDLE: f32 = 0.025;
/// Soul 浮遊の脈動振幅（移動時）
pub const SOUL_FLOAT_PULSE_AMPLITUDE_MOVE: f32 = 0.04;
/// 会話中トーンイベント（即時）の表情ロック時間: Positive（秒）
pub const SOUL_EVENT_LOCK_TONE_POSITIVE: f32 = 3.0;
/// 会話中トーンイベント（即時）の表情ロック時間: Negative（秒）
pub const SOUL_EVENT_LOCK_TONE_NEGATIVE: f32 = 3.4;
/// 会話完了イベントの表情ロック時間: Positive（秒）
pub const SOUL_EVENT_LOCK_COMPLETED_POSITIVE: f32 = 1.4;
/// 会話完了イベントの表情ロック時間: Negative（秒）
pub const SOUL_EVENT_LOCK_COMPLETED_NEGATIVE: f32 = 1.8;
/// 疲労限界イベントの表情ロック時間（秒）
pub const SOUL_EVENT_LOCK_EXHAUSTED: f32 = 4.0;
/// 集会オブジェクト起点（wine/trump）の表情ロック時間（秒）
pub const SOUL_EVENT_LOCK_GATHERING_OBJECT: f32 = 2.2;

// UI font constants moved to UiTheme resource (src/interface/ui/theme.rs)
// Keep FONT_SIZE_BODY for in-game visual elements (not UI)
/// 本文用フォントサイズ（ゲーム内ビジュアル用）
pub const FONT_SIZE_BODY: f32 = 16.0;

// ============================================================
// 吹き出しシステム (Speech Bubble)
// ============================================================

/// 吹き出しの生存時間 (秒)
pub const BUBBLE_DURATION_LOW: f32 = 0.8;
pub const BUBBLE_DURATION_NORMAL: f32 = 1.5;
pub const BUBBLE_DURATION_HIGH: f32 = 2.5;
pub const BUBBLE_DURATION_CRITICAL: f32 = 3.5;

/// 吹き出しの話者からのオフセット
pub const SPEECH_BUBBLE_OFFSET: Vec2 = Vec2::new(16.0, 16.0);

/// Soul吹き出し（絵文字）のフォントサイズ
pub const BUBBLE_SIZE_SOUL_LOW: f32 = 18.0;
pub const BUBBLE_SIZE_SOUL_NORMAL: f32 = 24.0;
pub const BUBBLE_SIZE_SOUL_HIGH: f32 = 28.0;
pub const BUBBLE_SIZE_SOUL_CRITICAL: f32 = 32.0;

/// Familiar吹き出しのフォントサイズ
pub const BUBBLE_SIZE_FAMILIAR_LOW: f32 = 10.0;
pub const BUBBLE_SIZE_FAMILIAR_NORMAL: f32 = 12.0;
pub const BUBBLE_SIZE_FAMILIAR_HIGH: f32 = 14.0;
pub const BUBBLE_SIZE_FAMILIAR_CRITICAL: f32 = 16.0;
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
pub const BUBBLE_COLOR_FEARFUL: Color = Color::srgba(0.5, 0.4, 0.7, 1.0); // 紫
pub const BUBBLE_COLOR_RELIEVED: Color = Color::srgba(0.4, 0.8, 1.0, 1.0); // 水色
pub const BUBBLE_COLOR_RELAXED: Color = Color::srgba(0.4, 1.0, 0.7, 1.0); // ミント
pub const BUBBLE_COLOR_FRUSTRATED: Color = Color::srgba(0.7, 0.7, 0.7, 1.0); // 濁ったグレー
pub const BUBBLE_COLOR_UNMOTIVATED: Color = Color::srgba(0.8, 0.8, 0.5, 1.0); // 鈍い黄色
pub const BUBBLE_COLOR_BORED: Color = Color::srgba(0.7, 0.7, 1.0, 0.8); // 薄い青
pub const BUBBLE_COLOR_SLACKING: Color = Color::srgba(0.5, 0.7, 0.5, 1.0); // 落ち着いた緑
pub const BUBBLE_COLOR_CHATTING: Color = Color::srgba(1.0, 0.9, 0.6, 1.0); // 薄いオレンジ/クリーム

// ============================================================
// 定期セリフシステム (Periodic Emotion System)
// ============================================================

pub const PERIODIC_EMOTION_LOCK_DURATION: f32 = 10.0; // 一度出た後のロック時間（秒）
pub const IDLE_EMOTION_MIN_DURATION: f32 = 10.0; // アイドル何秒以上でボアド判定に入るか

// 定期判定の確率
pub const PROBABILITY_PERIODIC_STRESSED: f32 = 0.2;
pub const PROBABILITY_PERIODIC_EXHAUSTED: f32 = 0.2;
pub const PROBABILITY_PERIODIC_UNMOTIVATED: f32 = 0.1;
pub const PROBABILITY_PERIODIC_BORED: f32 = 0.05;

/// 分散実行用のフレーム分割数（10フレームで全Soulを巡回）
pub const PERIODIC_EMOTION_FRAME_DIVISOR: u32 = 10;

// しきい値
pub const EMOTION_THRESHOLD_STRESSED: f32 = 0.6;
pub const EMOTION_THRESHOLD_EXHAUSTED: f32 = 0.7;
pub const EMOTION_THRESHOLD_UNMOTIVATED: f32 = 0.3;

// ============================================================
// 会話システム (Soul Conversation System)
// ============================================================

/// 会話の感知半径
pub const CONVERSATION_RADIUS: f32 = 2.5 * TILE_SIZE;
/// 会話開始の試行間隔 (秒)
pub const CONVERSATION_CHECK_INTERVAL: f32 = 3.0;
/// 会話開始確率 (Idle時)
pub const CONVERSATION_CHANCE_IDLE: f32 = 0.2;
/// 会話開始確率 (Gathering時) - 動的集会システムでさらに活発に
pub const CONVERSATION_CHANCE_GATHERING: f32 = 0.6;
/// 会話後のクールダウン (秒)
pub const CONVERSATION_COOLDOWN: f32 = 30.0;
/// 1ターンの表示時間
pub const CONVERSATION_TURN_DURATION: f32 = 2.0;
/// 会話成立によるストレス軽減量
pub const CONVERSATION_STRESS_RELIEF: f32 = 2.0;
/// 集会所での長期会話ボーナス
pub const CONVERSATION_LONG_CHAT_BONUS: f32 = 3.0;

/// 会話用絵文字セット
pub const EMOJIS_GREETING: &[&str] = &["👋", "🙋‍♂️"];
pub const EMOJIS_QUESTION: &[&str] = &["❓", "❔"];
pub const EMOJIS_AGREEMENT: &[&str] = &["🙆‍♂️", "👍", "👌"];
pub const EMOJIS_SLACKING: &[&str] = &["🛌", "🛑", "🐌"];
pub const EMOJIS_FOOD: &[&str] = &["🍖", "🍺", "🥤"];
pub const EMOJIS_COMPLAINING: &[&str] = &["😓", "😴", "😒", "🥱"];
/// 使い魔の指示リアクション時にネガティブトーンを発火する確率
pub const COMMAND_REACTION_NEGATIVE_EVENT_CHANCE: f32 = 0.75;

// ============================================================
// Dream システム
// ============================================================

/// VividDream の蓄積レート (ポイント/秒)
pub const DREAM_RATE_VIVID: f32 = 0.15;
/// NormalDream の蓄積レート (ポイント/秒)
pub const DREAM_RATE_NORMAL: f32 = 0.1;
/// 悪夢判定のストレス閾値（これ以上で NightTerror）
pub const DREAM_NIGHTMARE_STRESS_THRESHOLD: f32 = 0.7;
/// VividDream 判定のストレス閾値（これ以下＋集会中で VividDream）
pub const DREAM_VIVID_STRESS_THRESHOLD: f32 = 0.3;
