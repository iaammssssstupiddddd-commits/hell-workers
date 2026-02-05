use super::components::*;
use crate::constants::*;
use bevy::prelude::*;

/// 吹き出しの追従およびフェードアウト/削除システム
pub fn update_speech_bubbles(
    mut commands: Commands,
    time: Res<Time>,
    mut q_bubbles: Query<(Entity, &mut SpeechBubble, &mut Transform)>,
) {
    for (entity, mut bubble, mut transform) in q_bubbles.iter_mut() {
        // タイマー更新
        bubble.elapsed += time.delta_secs();

        if bubble.elapsed >= bubble.duration {
            if let Some(bg) = bubble.background {
                if let Ok(mut cmd) = commands.get_entity(bg) {
                    cmd.despawn();
                }
            }
            if let Ok(mut cmd) = commands.get_entity(entity) {
                cmd.despawn();
            }
            continue;
        }

        // Relative Offset follow (for stacking, which updates bubble.offset)
        transform.translation.x = bubble.offset.x;
        transform.translation.y = bubble.offset.y;
        transform.translation.z = Z_SPEECH_BUBBLE - Z_CHARACTER;
    }
}

/// 吹き出しの重なりを調整するシステム（ParamSet最適化版）
pub fn update_bubble_stacking(
    mut removed: RemovedComponents<SpeechBubble>,
    mut set: ParamSet<(
        Query<&SpeechBubble, Added<SpeechBubble>>,
        Query<(Entity, &mut SpeechBubble)>,
    )>,
) {
    // 1. 追加または削除があるかチェック
    let has_added = !set.p0().is_empty();
    let has_removed = removed.read().next().is_some();

    if !has_added && !has_removed {
        return;
    }

    use std::collections::HashMap;

    // 2. 情報を収集
    let bubble_info: Vec<(Entity, Entity, f32)> = set
        .p1()
        .iter()
        .map(|(e, b)| (e, b.speaker, b.elapsed))
        .collect();

    // 3. 話者ごとにグループ化
    let mut speaker_data: HashMap<Entity, Vec<(Entity, f32)>> = HashMap::new();
    for (entity, speaker, elapsed) in bubble_info {
        speaker_data
            .entry(speaker)
            .or_default()
            .push((entity, elapsed));
    }

    // 4. 各グループ内でソートし、オフセットを更新
    let mut offset_updates: Vec<(Entity, f32)> = Vec::new();
    for (_speaker, mut data) in speaker_data {
        data.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        for (stack_idx, (entity, _)) in data.iter().enumerate() {
            let new_offset_y = SPEECH_BUBBLE_OFFSET.y + (stack_idx as f32 * BUBBLE_STACK_GAP);
            offset_updates.push((*entity, new_offset_y));
        }
    }

    // 5. オフセットを適用
    let mut q_bubbles = set.p1();
    for (entity, new_y) in offset_updates {
        if let Ok((_e, mut bubble)) = q_bubbles.get_mut(entity) {
            bubble.offset.y = new_y;
        }
    }
}
