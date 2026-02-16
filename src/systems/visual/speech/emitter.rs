//! 発話の共通化: can_speak 判定 + SpeechHistory 更新/insert + bubble spawn

use super::components::{BubbleEmotion, BubblePriority, FamiliarBubble, SpeechBubble};
use super::cooldown::SpeechHistory;
use super::phrases::LatinPhrase;
use super::spawn::{spawn_familiar_bubble, spawn_soul_bubble};
use crate::assets::GameAssets;
use crate::entities::familiar::FamiliarVoice;
use bevy::prelude::*;

/// Soul の発話を、履歴更新込みで発火する。
/// can_speak が false なら何もしない。
pub fn emit_soul_with_history(
    commands: &mut Commands,
    soul_entity: Entity,
    emoji: &str,
    pos: Vec3,
    assets: &Res<GameAssets>,
    emotion: BubbleEmotion,
    priority: BubblePriority,
    history_opt: Option<impl std::ops::Deref<Target = SpeechHistory> + std::ops::DerefMut>,
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
        assets,
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

/// Familiar の発話を、履歴更新込みで発火する。
/// can_speak が false なら何もしない。
pub fn emit_familiar_with_history(
    commands: &mut Commands,
    fam_entity: Entity,
    phrase: LatinPhrase,
    pos: Vec3,
    assets: &Res<GameAssets>,
    q_bubbles: &Query<(Entity, &SpeechBubble), With<FamiliarBubble>>,
    emotion: BubbleEmotion,
    priority: BubblePriority,
    voice: Option<&FamiliarVoice>,
    history_opt: Option<impl std::ops::Deref<Target = SpeechHistory> + std::ops::DerefMut>,
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
        commands,
        fam_entity,
        phrase,
        pos,
        assets,
        q_bubbles,
        emotion,
        priority,
        voice,
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
