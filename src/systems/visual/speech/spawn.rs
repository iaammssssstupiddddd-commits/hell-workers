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
) {
    commands.spawn((
        SpeechBubble {
            elapsed: 0.0,
            duration: SPEECH_BUBBLE_DURATION,
            speaker: soul_entity,
            offset: SPEECH_BUBBLE_OFFSET,
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
) {
    commands
        .spawn((
            SpeechBubble {
                elapsed: 0.0,
                duration: SPEECH_BUBBLE_DURATION,
                speaker: fam_entity,
                offset: SPEECH_BUBBLE_OFFSET,
            },
            FamiliarBubble,
            Text2d::new(phrase.as_str()),
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
            ),
        ))
        .with_child((
            SpeechBubbleBackground,
            Sprite {
                image: assets.speech_bubble.clone(),
                color: Color::srgba(1.0, 1.0, 1.0, 0.85),
                ..default()
            },
            Transform::from_xyz(
                0.0, // 親（吹き出しテキスト）からの相対座標
                0.0,
                Z_SPEECH_BUBBLE_BG - Z_SPEECH_BUBBLE, // Z軸も相対
            ),
        ));
}
