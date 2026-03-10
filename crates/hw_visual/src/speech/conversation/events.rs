use bevy::prelude::*;

/// 会話リクエストイベント
#[derive(Message)]
pub struct RequestConversation {
    pub initiator: Entity,
    pub target: Entity,
}

/// 会話のトーン（画像トリガー用）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConversationTone {
    Positive,
    Negative,
    Neutral,
}

/// 会話中の発話トーンイベント（即時ビジュアル反映用）
#[derive(Message)]
pub struct ConversationToneTriggered {
    pub speaker: Entity,
    pub tone: ConversationTone,
}

/// 会話完了イベント
#[derive(Message)]
pub struct ConversationCompleted {
    pub participants: Vec<Entity>,
    pub turns: u32,
    pub tone: ConversationTone,
}
