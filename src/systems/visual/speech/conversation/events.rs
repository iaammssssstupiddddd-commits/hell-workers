use bevy::prelude::*;

/// 会話リクエストイベント
#[derive(Message)]
pub struct RequestConversation {
    pub initiator: Entity,
    pub target: Entity,
}

/// 会話完了イベント
#[derive(Message)]
pub struct ConversationCompleted {
    pub participants: Vec<Entity>,
    pub turns: u32,
}
