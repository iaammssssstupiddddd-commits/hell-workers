//! 発話の共通化: can_speak 判定 + SpeechHistory 更新/insert + bubble spawn

use super::components::{BubbleEmotion, BubblePriority, FamiliarBubble, SpeechBubble};
use super::cooldown::SpeechHistory;
use super::spawn::{FamiliarBubbleSpec, spawn_familiar_bubble, spawn_soul_bubble};
use crate::handles::SpeechHandles;
use bevy::prelude::*;

/// emit_soul_with_history に渡すコンテンツをグループ化する構造体
pub struct SoulSpeechContent<'a> {
    pub emoji: &'a str,
    pub emotion: BubbleEmotion,
    pub priority: BubblePriority,
}

/// Soul の発話を、履歴更新込みで発火する。
/// can_speak が false なら何もしない。
pub fn emit_soul_with_history(
    commands: &mut Commands,
    soul_entity: Entity,
    content: SoulSpeechContent<'_>,
    pos: Vec3,
    handles: &Res<SpeechHandles>,
    history_opt: Option<impl std::ops::DerefMut<Target = SpeechHistory>>,
    current_time: f32,
) -> bool {
    let can_speak = history_opt
        .as_ref()
        .map(|h| h.can_speak(content.priority, current_time))
        .unwrap_or(true);

    if !can_speak {
        return false;
    }

    spawn_soul_bubble(
        commands,
        soul_entity,
        content.emoji,
        pos,
        handles,
        content.emotion,
        content.priority,
    );

    if let Some(mut history) = history_opt {
        history.record_speech(content.priority, current_time);
    } else {
        commands.entity(soul_entity).insert(SpeechHistory {
            last_time: current_time,
            last_priority: content.priority,
        });
    }

    true
}

/// Familiar の発話を、履歴更新込みで発火する。
/// can_speak が false なら何もしない。
pub fn emit_familiar_with_history(
    commands: &mut Commands,
    fam_entity: Entity,
    spec: FamiliarBubbleSpec<'_>,
    handles: &Res<SpeechHandles>,
    q_bubbles: &Query<(Entity, &SpeechBubble), With<FamiliarBubble>>,
    history_opt: Option<impl std::ops::DerefMut<Target = SpeechHistory>>,
    current_time: f32,
) -> bool {
    let can_speak = history_opt
        .as_ref()
        .map(|h| h.can_speak(spec.priority, current_time))
        .unwrap_or(true);

    if !can_speak {
        return false;
    }

    let priority = spec.priority;
    spawn_familiar_bubble(commands, fam_entity, spec, handles, q_bubbles);

    if let Some(mut history) = history_opt {
        history.record_speech(priority, current_time);
    } else {
        commands.entity(fam_entity).insert(SpeechHistory {
            last_time: current_time,
            last_priority: priority,
        });
    }

    true
}
