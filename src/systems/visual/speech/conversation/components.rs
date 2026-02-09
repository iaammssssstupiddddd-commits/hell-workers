use bevy::prelude::*;

/// 会話を開始しようとするタイマー
#[derive(Component, Reflect)]
pub struct ConversationInitiator {
    pub timer: Timer,
}

/// 会話への参加状態
#[derive(Component, Reflect)]
pub struct ConversationParticipant {
    /// 相手のエンティティ
    pub target: Entity,
    /// 会話の役割
    pub role: ConversationRole,
    /// 現在の会話フェーズ
    pub phase: ConversationPhase,
    /// フェーズ切り替え用のタイマー
    pub timer: Timer,
    /// 会話のターン数
    pub turns: u32,
    /// ポジティブ寄りの発話数
    pub positive_turns: u8,
    /// ネガティブ寄りの発話数
    pub negative_turns: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect)]
pub enum ConversationRole {
    Initiator,
    Responder,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect)]
pub enum ConversationPhase {
    Greeting, // 挨拶
    Chatting, // 雑談中
    Closing,  // 終わり
}

/// 会話のクールダウン
#[derive(Component, Reflect)]
pub struct ConversationCooldown {
    pub timer: Timer,
}
