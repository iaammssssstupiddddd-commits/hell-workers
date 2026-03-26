//! 使役数上限変更時のビジュアル演出：Familiar の "Abi" セリフ表示。

use bevy::prelude::*;
use hw_core::events::FamiliarOperationMaxSoulChangedEvent;
use hw_core::familiar::Familiar;

use super::components::{BubbleEmotion, BubblePriority, FamiliarBubble, SpeechBubble};
use super::cooldown::SpeechHistory;
use super::phrases::LatinPhrase;
use super::spawn::{FamiliarBubbleSpec, spawn_familiar_bubble};
use super::voice::FamiliarVoice;
use crate::handles::SpeechHandles;

/// 使役数上限変更時のビジュアル演出システム（"Abi" セリフを表示）
pub fn max_soul_visual_system(
    mut ev_max_soul_changed: MessageReader<FamiliarOperationMaxSoulChangedEvent>,
    mut q_familiars: Query<
        (&Transform, &FamiliarVoice, Option<&mut SpeechHistory>),
        With<Familiar>,
    >,
    speech_handles: Res<SpeechHandles>,
    q_bubbles: Query<(Entity, &SpeechBubble), With<FamiliarBubble>>,
    time: Res<Time>,
    mut commands: Commands,
) {
    for event in ev_max_soul_changed.read() {
        if event.new_value >= event.old_value {
            continue;
        }

        let Ok((_fam_transform, voice_opt, history_opt)) =
            q_familiars.get_mut(event.familiar_entity)
        else {
            continue;
        };

        let current_time = time.elapsed_secs();
        let can_speak = if let Some(history) = &history_opt {
            history.can_speak(BubblePriority::Normal, current_time)
        } else {
            true
        };

        if !can_speak {
            continue;
        }

        spawn_familiar_bubble(
            &mut commands,
            event.familiar_entity,
            FamiliarBubbleSpec {
                phrase: LatinPhrase::Abi,
                emotion: BubbleEmotion::Neutral,
                priority: BubblePriority::Normal,
                voice: Some(voice_opt),
            },
            &speech_handles,
            &q_bubbles,
        );

        if let Some(mut history) = history_opt {
            history.record_speech(BubblePriority::Normal, current_time);
        } else {
            commands
                .entity(event.familiar_entity)
                .insert(SpeechHistory {
                    last_time: current_time,
                    last_priority: BubblePriority::Normal,
                });
        }
    }
}
