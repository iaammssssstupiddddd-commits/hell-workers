use bevy::prelude::*;

/// 吹き出しの基本コンポーネント
#[derive(Component)]
pub struct SpeechBubble {
    /// 経過時間
    pub elapsed: f32,
    /// 生存期間
    pub duration: f32,
    /// 追従対象のエンティティ
    pub speaker: Entity,
    /// 話者からのオフセット
    pub offset: Vec2,
    /// 感情タイプ
    pub emotion: BubbleEmotion,
    /// 優先度
    pub priority: BubblePriority,
}

/// 吹き出しの優先度
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Reflect)]
pub enum BubblePriority {
    Low, // タスク開始・完了（頻出）
    #[default]
    Normal, // 勧誘、待機
    High, // 疲労限界
    Critical, // ストレス崩壊
}

/// 吹き出しの感情タイプ
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Reflect)]
pub enum BubbleEmotion {
    #[default]
    Neutral, // 通常
    Motivated, // やる気（💪）
    Happy,     // 満足（😊）
    Exhausted, // 疲労（😴）
    Stressed,  // ストレス（😰）
}

/// アニメーション状態
#[derive(Component, Reflect)]
pub struct BubbleAnimation {
    pub phase: AnimationPhase,
    pub elapsed: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect)]
pub enum AnimationPhase {
    PopIn,  // 出現中（0 → 1.2 → 1.0）
    Idle,   // 通常表示
    PopOut, // 消失中（1.0 → 0）
}

/// タイプライター効果用
#[derive(Component, Reflect)]
pub struct TypewriterEffect {
    pub full_text: String,
    pub current_len: usize,
    pub char_interval: f32,
    pub elapsed: f32,
}

/// Soul用の吹き出しマーカー（テキストのみ）
#[derive(Component)]
pub struct SoulBubble;

/// Familiar用の吹き出しマーカー（背景付き）
#[derive(Component)]
pub struct FamiliarBubble;

/// 吹き出しの背景スプライト用マーカー
#[derive(Component)]
pub struct SpeechBubbleBackground;
