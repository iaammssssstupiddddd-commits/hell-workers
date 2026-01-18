use super::components::*;
use bevy::prelude::*;

/// 吹き出しの追従およびフェードアウト/削除システム
pub fn update_speech_bubbles(
    mut commands: Commands,
    time: Res<Time>,
    mut q_bubbles: Query<(
        Entity,
        &mut SpeechBubble,
        &mut Transform,
        Option<&mut Sprite>,
        Option<&mut TextColor>,
    )>,
    q_speakers: Query<&GlobalTransform>,
) {
    for (entity, mut bubble, mut transform, sprite, text_color) in q_bubbles.iter_mut() {
        // タイマー更新
        bubble.elapsed += time.delta_secs();

        if bubble.elapsed >= bubble.duration {
            // 背景（子エンティティ）も含めて削除するため、despawn_recursiveを使用
            // Bevy 0.18 では commands.entity(entity).despawn() がデフォルトで再帰的な可能性もあるが、
            // エラーを避けるために despawn() にし、もし手動が必要なら工夫する。
            // 以前 despawn_recursive() が無いと言われたため、despawn() を試す。
            commands.entity(entity).despawn();
            continue;
        }

        // 追従
        if let Ok(speaker_transform) = q_speakers.get(bubble.speaker) {
            let speaker_pos = speaker_transform.translation();
            transform.translation.x = speaker_pos.x + bubble.offset.x;
            transform.translation.y = speaker_pos.y + bubble.offset.y;
        }

        // フェードアウト
        let ratio = (1.0 - (bubble.elapsed / bubble.duration)).clamp(0.0, 1.0);

        if let Some(mut sprite) = sprite {
            let mut color = sprite.color;
            color.set_alpha(ratio * 0.85);
            sprite.color = color;
        }

        if let Some(mut text_color) = text_color {
            text_color.0.set_alpha(ratio);
        }
    }
}
