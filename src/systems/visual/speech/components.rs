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
