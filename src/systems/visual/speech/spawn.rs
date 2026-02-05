use super::components::*;
use super::phrases::LatinPhrase;
use crate::assets::GameAssets;
use crate::constants::*;
use crate::entities::familiar::FamiliarVoice;
use bevy::prelude::*;

/// Soul用の絵文字吹き出しをスポーンする
pub fn spawn_soul_bubble(
    commands: &mut Commands,
    soul_entity: Entity,
    emoji: &str,
    _pos: Vec3,
    assets: &Res<GameAssets>,
    emotion: BubbleEmotion,
    priority: BubblePriority,
) {
    // 優先度に応じた生存時間の決定
    let duration = match priority {
        BubblePriority::Low => BUBBLE_DURATION_LOW,
        BubblePriority::Normal => BUBBLE_DURATION_NORMAL,
        BubblePriority::High => BUBBLE_DURATION_HIGH,
        BubblePriority::Critical => BUBBLE_DURATION_CRITICAL,
    };

    // 優先度に応じたフォントサイズの決定
    let font_size = match priority {
        BubblePriority::Low => BUBBLE_SIZE_SOUL_LOW,
        BubblePriority::Normal => BUBBLE_SIZE_SOUL_NORMAL,
        BubblePriority::High => BUBBLE_SIZE_SOUL_HIGH,
        BubblePriority::Critical => BUBBLE_SIZE_SOUL_CRITICAL,
    };

    // 感情に応じたカラーの決定
    let glow_color = match emotion {
        BubbleEmotion::Motivated => BUBBLE_COLOR_MOTIVATED,
        BubbleEmotion::Happy => BUBBLE_COLOR_HAPPY,
        BubbleEmotion::Exhausted => BUBBLE_COLOR_EXHAUSTED,
        BubbleEmotion::Stressed => BUBBLE_COLOR_STRESSED,
        BubbleEmotion::Fearful => BUBBLE_COLOR_FEARFUL,
        BubbleEmotion::Relieved => BUBBLE_COLOR_RELIEVED,
        BubbleEmotion::Relaxed => BUBBLE_COLOR_RELAXED,
        BubbleEmotion::Frustrated => BUBBLE_COLOR_FRUSTRATED,
        BubbleEmotion::Unmotivated => BUBBLE_COLOR_UNMOTIVATED,
        BubbleEmotion::Bored => BUBBLE_COLOR_BORED,
        BubbleEmotion::Slacking => BUBBLE_COLOR_SLACKING,
        BubbleEmotion::Chatting => BUBBLE_COLOR_CHATTING,
        BubbleEmotion::Neutral => Color::srgba(1.0, 1.0, 1.0, 0.5),
    };

    let bg_entity = commands.spawn((
        SpeechBubbleBackground,
        Sprite {
            image: assets.glow_circle.clone(),
            color: glow_color.with_alpha(0.6),
            custom_size: Some(Vec2::splat(64.0)),
            ..default()
        },
        Transform::from_xyz(
            0.0,
            0.0,
            Z_SPEECH_BUBBLE_BG - Z_SPEECH_BUBBLE, // 実際にはBubbleの子にするので相対
        ),
    )).id();

    let bubble_entity = commands.spawn((
        SpeechBubble {
            elapsed: 0.0,
            duration,
            speaker: soul_entity,
            offset: SPEECH_BUBBLE_OFFSET,
            emotion,
            background: Some(bg_entity),
        },
        BubbleAnimation {
            phase: AnimationPhase::PopIn,
            elapsed: 0.0,
        },
        SoulBubble,
        Text2d::new(emoji),
        TextFont {
            font: assets.font_soul_emoji.clone(),
            font_size,
            ..default()
        },
        TextColor(Color::WHITE),
        TextLayout::new_with_justify(Justify::Center),
        Transform::from_xyz(
            SPEECH_BUBBLE_OFFSET.x,
            SPEECH_BUBBLE_OFFSET.y,
            Z_SPEECH_BUBBLE - Z_CHARACTER,
        )
        .with_scale(Vec3::ZERO),
    )).id();

    // 背景を吹き出しの子にする
    commands.entity(bubble_entity).add_child(bg_entity);
    // 吹き出しをソウルの子にする
    commands.entity(soul_entity).add_child(bubble_entity);
}

/// Familiar用のラテン語フレーズ吹き出しをスポーンする
pub fn spawn_familiar_bubble(
    commands: &mut Commands,
    fam_entity: Entity,
    phrase: LatinPhrase,
    _pos: Vec3,
    assets: &Res<GameAssets>,
    q_bubbles: &Query<(Entity, &SpeechBubble), With<FamiliarBubble>>,
    emotion: BubbleEmotion,
    priority: BubblePriority,
    voice: Option<&FamiliarVoice>,
) {
    // 優先度に応じた生存時間の決定
    let duration = match priority {
        BubblePriority::Low => BUBBLE_DURATION_LOW,
        BubblePriority::Normal => BUBBLE_DURATION_NORMAL,
        BubblePriority::High => BUBBLE_DURATION_HIGH,
        BubblePriority::Critical => BUBBLE_DURATION_CRITICAL,
    };

    // 優先度に応じたフォントサイズの決定
    let font_size = match priority {
        BubblePriority::Low => BUBBLE_SIZE_FAMILIAR_LOW,
        BubblePriority::Normal => BUBBLE_SIZE_FAMILIAR_NORMAL,
        BubblePriority::High => BUBBLE_SIZE_FAMILIAR_HIGH,
        BubblePriority::Critical => BUBBLE_SIZE_FAMILIAR_CRITICAL,
    };

    // 既存の吹き出しを削除
    for (bubble_entity, bubble) in q_bubbles.iter() {
        if bubble.speaker == fam_entity {
            commands.entity(bubble_entity).despawn();
        }
    }
    // テキスト長に応じたサイズ計算 (概算: 1文字平均 8px + 左右余白 16px)
    let text_str = if let Some(v) = voice {
        phrase.select_with_preference(v.get_preference(phrase.clone()), v.preference_weight)
    } else {
        phrase.random_str()
    };
    let text_width = (text_str.len() as f32 * 8.0).max(32.0);
    let bubble_width = text_width + 16.0;
    let bubble_height = 32.0;

    // 感情に応じたカラーの決定
    let bubble_color = match emotion {
        BubbleEmotion::Motivated => BUBBLE_COLOR_MOTIVATED,
        BubbleEmotion::Happy => BUBBLE_COLOR_HAPPY,
        BubbleEmotion::Exhausted => BUBBLE_COLOR_EXHAUSTED,
        BubbleEmotion::Stressed => BUBBLE_COLOR_STRESSED,
        BubbleEmotion::Fearful => BUBBLE_COLOR_FEARFUL,
        BubbleEmotion::Relieved => BUBBLE_COLOR_RELIEVED,
        BubbleEmotion::Relaxed => BUBBLE_COLOR_RELAXED,
        BubbleEmotion::Frustrated => BUBBLE_COLOR_FRUSTRATED,
        BubbleEmotion::Unmotivated => BUBBLE_COLOR_UNMOTIVATED,
        BubbleEmotion::Bored => BUBBLE_COLOR_BORED,
        BubbleEmotion::Slacking => BUBBLE_COLOR_SLACKING,
        BubbleEmotion::Chatting => BUBBLE_COLOR_CHATTING,
        BubbleEmotion::Neutral => Color::WHITE,
    };

    let bg_entity = commands.spawn((
        SpeechBubbleBackground,
        Sprite {
            image: assets.bubble_9slice.clone(),
            color: bubble_color.with_alpha(0.85),
            image_mode: SpriteImageMode::Sliced(TextureSlicer {
                border: BorderRect::all(12.0),
                center_scale_mode: SliceScaleMode::Stretch,
                sides_scale_mode: SliceScaleMode::Stretch,
                max_corner_scale: 1.0,
            }),
            custom_size: Some(Vec2::new(bubble_width, bubble_height)),
            ..default()
        },
        Transform::from_xyz(0.0, 0.0, Z_SPEECH_BUBBLE_BG - Z_SPEECH_BUBBLE),
    )).id();

    let bubble_entity = commands.spawn((
        SpeechBubble {
            elapsed: 0.0,
            duration,
            speaker: fam_entity,
            offset: SPEECH_BUBBLE_OFFSET,
            emotion,
            background: Some(bg_entity),
        },
        BubbleAnimation {
            phase: AnimationPhase::PopIn,
            elapsed: 0.0,
        },
        FamiliarBubble,
        Text2d::new(text_str),
        TextFont {
            font: assets.font_ui.clone(),
            font_size,
            ..default()
        },
        TextColor(Color::BLACK),
        TextLayout::new_with_justify(Justify::Center),
        Transform::from_xyz(
            SPEECH_BUBBLE_OFFSET.x,
            SPEECH_BUBBLE_OFFSET.y,
            Z_SPEECH_BUBBLE - Z_CHARACTER,
        )
        .with_scale(Vec3::ZERO),
    )).id();

    commands.entity(bubble_entity).add_child(bg_entity);
    commands.entity(fam_entity).add_child(bubble_entity);
}
