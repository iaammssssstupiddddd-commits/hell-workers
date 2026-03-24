//! 発話の共通化: can_speak 判定 + SpeechHistory 更新/insert + bubble spawn

use super::components::{BubbleEmotion, BubblePriority, FamiliarBubble, SpeechBubble};
use super::cooldown::SpeechHistory;
use super::phrases::LatinPhrase;
use super::spawn::{spawn_familiar_bubble, spawn_soul_bubble};
use super::voice::FamiliarVoice;
use crate::handles::SpeechHandles;
use bevy::prelude::*;

#[allow(clippy::too_many_arguments)]
/// Soul の発話を、履歴更新込みで発火する。
/// can_speak が false なら何もしない。
pub fn emit_soul_with_history(
    commands: &mut Commands,
    soul_entity: Entity,
    emoji: &str,
    pos: Vec3,
    handles: &Res<SpeechHandles>,
    emotion: BubbleEmotion,
    priority: BubblePriority,
    history_opt: Option<impl std::ops::DerefMut<Target = SpeechHistory>>,
    current_time: f32,
) -> bool {
    let can_speak = history_opt
        .as_ref()
        .map(|h| h.can_speak(priority, current_time))
        .unwrap_or(true);

    if !can_speak {
        return false;
    }

    spawn_soul_bubble(
        commands,
        soul_entity,
        emoji,
        pos,
        handles,
        emotion,
        priority,
    );

    if let Some(mut history) = history_opt {
        history.record_speech(priority, current_time);
    } else {
        commands.entity(soul_entity).insert(SpeechHistory {
            last_time: current_time,
            last_priority: priority,
        });
    }

    true
}

#[allow(clippy::too_many_arguments)]
/// Familiar の発話を、履歴更新込みで発火する。
/// can_speak が false なら何もしない。
pub fn emit_familiar_with_history(
    commands: &mut Commands,
    fam_entity: Entity,
    phrase: LatinPhrase,
    pos: Vec3,
    handles: &Res<SpeechHandles>,
    q_bubbles: &Query<(Entity, &SpeechBubble), With<FamiliarBubble>>,
    emotion: BubbleEmotion,
    priority: BubblePriority,
    voice: Option<&FamiliarVoice>,
    history_opt: Option<impl std::ops::DerefMut<Target = SpeechHistory>>,
    current_time: f32,
) -> bool {
    let can_speak = history_opt
        .as_ref()
        .map(|h| h.can_speak(priority, current_time))
        .unwrap_or(true);

    if !can_speak {
        return false;
    }

    spawn_familiar_bubble(
        commands, fam_entity, phrase, pos, handles, q_bubbles, emotion, priority, voice,
    );

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
