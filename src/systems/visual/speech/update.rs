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

/// 吹き出しの重なりを調整するシステム（ParamSet最適化版）
/// 吹き出しの追加・削除時のみ計算を実行し、Query競合を回避する
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

    // 2. 情報を収集 (ParamSet p1 を読み取り専用で使用)
    // 注意: p1() を呼ぶと他の Query と競合する可能性があるため、スコープを限定する
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
        // 経過時間が大きい = 古い → 上に配置
        data.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        for (stack_idx, (entity, _)) in data.iter().enumerate() {
            let new_offset_y = SPEECH_BUBBLE_OFFSET.y + (stack_idx as f32 * BUBBLE_STACK_GAP);
            offset_updates.push((*entity, new_offset_y));
        }
    }

    // 5. オフセットを適用 (ParamSet p1 を可変で使用)
    let mut q_bubbles = set.p1();
    for (entity, new_y) in offset_updates {
        if let Ok((_e, mut bubble)) = q_bubbles.get_mut(entity) {
            bubble.offset.y = new_y;
        }
    }
}
