//! 会話システム (Soul Conversation System)

use super::world::TILE_SIZE;

/// 会話の感知半径
pub const CONVERSATION_RADIUS: f32 = 2.5 * TILE_SIZE;
/// 会話開始の試行間隔 (秒)
pub const CONVERSATION_CHECK_INTERVAL: f32 = 3.0;
/// 会話開始確率 (Idle時)
pub const CONVERSATION_CHANCE_IDLE: f32 = 0.2;
/// 会話開始確率 (Gathering時)
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
