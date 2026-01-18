use super::components::*;
use super::phrases::LatinPhrase;
use crate::assets::GameAssets;
use crate::constants::*;
use bevy::prelude::*;

/// Soul用の絵文字吹き出しをスポーンする
pub fn spawn_soul_bubble(
    commands: &mut Commands,
    soul_entity: Entity,
    emoji: &str,
    pos: Vec3,
    assets: &Res<GameAssets>,
    emotion: BubbleEmotion,
) {
    // 感情に応じたカラーの決定
    let glow_color = match emotion {
        BubbleEmotion::Motivated => BUBBLE_COLOR_MOTIVATED,
        BubbleEmotion::Happy => BUBBLE_COLOR_HAPPY,
        BubbleEmotion::Exhausted => BUBBLE_COLOR_EXHAUSTED,
        BubbleEmotion::Stressed => BUBBLE_COLOR_STRESSED,
        BubbleEmotion::Neutral => Color::srgba(1.0, 1.0, 1.0, 0.5),
    };

    commands
        .spawn((
            SpeechBubble {
                elapsed: 0.0,
                duration: SPEECH_BUBBLE_DURATION,
                speaker: soul_entity,
                offset: SPEECH_BUBBLE_OFFSET,
                emotion,
            },
            BubbleAnimation {
                phase: AnimationPhase::PopIn,
                elapsed: 0.0,
            },
            SoulBubble,
            Text2d::new(emoji),
            TextFont {
                font: assets.font_soul_emoji.clone(),
                font_size: FONT_SIZE_BUBBLE_SOUL,
                ..default()
            },
            TextColor(Color::WHITE),
            TextLayout::new_with_justify(Justify::Center),
            Transform::from_xyz(
                pos.x + SPEECH_BUBBLE_OFFSET.x,
                pos.y + SPEECH_BUBBLE_OFFSET.y,
                Z_SPEECH_BUBBLE,
            )
            .with_scale(Vec3::ZERO), // PopInアニメーションのためにスケール0から開始
        ))
        .with_child((
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
                Z_SPEECH_BUBBLE_BG - Z_SPEECH_BUBBLE, // Z軸も相対
            ),
        ));
}

/// Familiar用のラテン語フレーズ吹き出しをスポーンする
pub fn spawn_familiar_bubble(
    commands: &mut Commands,
    fam_entity: Entity,
    phrase: LatinPhrase,
    pos: Vec3,
    assets: &Res<GameAssets>,
    q_bubbles: &Query<(Entity, &SpeechBubble), With<FamiliarBubble>>,
    emotion: BubbleEmotion,
) {
    // 既存の吹き出しを削除
    for (bubble_entity, bubble) in q_bubbles.iter() {
        if bubble.speaker == fam_entity {
            commands.entity(bubble_entity).despawn();
        }
    }
    // テキスト長に応じたサイズ計算 (概算: 1文字平均 8px + 左右余白 16px)
    let text_str = phrase.as_str();
    let text_width = (text_str.len() as f32 * 8.0).max(32.0);
    let bubble_width = text_width + 16.0;
    let bubble_height = 32.0;

    // 感情に応じたカラーの決定
    let bubble_color = match emotion {
        BubbleEmotion::Motivated => BUBBLE_COLOR_MOTIVATED,
        BubbleEmotion::Happy => BUBBLE_COLOR_HAPPY,
        BubbleEmotion::Exhausted => BUBBLE_COLOR_EXHAUSTED,
        BubbleEmotion::Stressed => BUBBLE_COLOR_STRESSED,
        BubbleEmotion::Neutral => Color::WHITE,
    };

    commands
        .spawn((
            SpeechBubble {
                elapsed: 0.0,
                duration: SPEECH_BUBBLE_DURATION,
                speaker: fam_entity,
                offset: SPEECH_BUBBLE_OFFSET,
                emotion,
            },
            BubbleAnimation {
                phase: AnimationPhase::PopIn,
                elapsed: 0.0,
            },
            FamiliarBubble,
            Text2d::new(text_str),
            TextFont {
                font: assets.font_ui.clone(),
                font_size: FONT_SIZE_BUBBLE_FAMILIAR,
                ..default()
            },
            TextColor(Color::BLACK),
            TextLayout::new_with_justify(Justify::Center),
            Transform::from_xyz(
                pos.x + SPEECH_BUBBLE_OFFSET.x,
                pos.y + SPEECH_BUBBLE_OFFSET.y,
                Z_SPEECH_BUBBLE,
            )
            .with_scale(Vec3::ZERO), // PopInアニメーションのためにスケール0から開始
        ))
        .with_child((
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
        ));
}
