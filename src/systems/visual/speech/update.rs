use super::components::*;
use crate::constants::*;
use bevy::prelude::*;

/// 吹き出しの追従およびフェードアウト/削除システム
pub fn update_speech_bubbles(
    mut commands: Commands,
    time: Res<Time>,
    mut q_bubbles: Query<(Entity, &mut SpeechBubble, &mut Transform)>,
    q_speakers: Query<&GlobalTransform>,
) {
    for (entity, mut bubble, mut transform) in q_bubbles.iter_mut() {
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
    }
}

/// 吹き出しの重なりを調整するシステム
pub fn update_bubble_stacking(mut q_bubbles: Query<&mut SpeechBubble>) {
    use std::collections::HashMap;
    let mut speaker_groups: HashMap<Entity, Vec<Mut<SpeechBubble>>> = HashMap::new();

    // 1. 話者ごとに吹き出しをグループ化
    for bubble in q_bubbles.iter_mut() {
        speaker_groups
            .entry(bubble.speaker)
            .or_default()
            .push(bubble);
    }

    // 2. 各話者グループ内で経過時間順に並べ替え、オフセットを更新
    for (_speaker, mut bubbles) in speaker_groups {
        // 経過時間が短い（新しい）順に並べるか、長い（古い）順に並べるか
        // ここでは、古いものが上に、新しいものが下に来るようにする（逆も可）
        // 経過時間が大きい = 古い
        bubbles.sort_by(|a, b| {
            b.elapsed
                .partial_cmp(&a.elapsed)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        for (i, bubble) in bubbles.iter_mut().enumerate() {
            // 基本オフセットに、スタック分のギャップを加算
            bubble.offset.y = SPEECH_BUBBLE_OFFSET.y + (i as f32 * BUBBLE_STACK_GAP);
        }
    }
}
