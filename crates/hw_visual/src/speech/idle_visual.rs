//! Familiar のアイドル遷移時ビジュアル演出システム。

use bevy::prelude::*;
use hw_core::events::FamiliarIdleVisualRequest;
use hw_core::familiar::Familiar;

use super::components::{BubbleEmotion, BubblePriority, FamiliarBubble, SpeechBubble};
use super::cooldown::SpeechHistory;
use super::phrases::LatinPhrase;
use super::spawn::spawn_familiar_bubble;
use super::voice::FamiliarVoice;
use crate::handles::SpeechHandles;

/// Idle 遷移時のビジュアル演出を適用する（Execute Phase）
pub fn familiar_idle_visual_apply_system(
    mut commands: Commands,
    time: Res<Time>,
    mut request_reader: MessageReader<FamiliarIdleVisualRequest>,
    q_bubbles: Query<(Entity, &SpeechBubble), With<FamiliarBubble>>,
    speech_handles: Res<SpeechHandles>,
    mut q_familiars: Query<
        (
            &Transform,
            Option<&FamiliarVoice>,
            Option<&mut SpeechHistory>,
        ),
        With<Familiar>,
    >,
) {
    for request in request_reader.read() {
        let Ok((fam_transform, voice_opt, mut history_opt)) =
            q_familiars.get_mut(request.familiar_entity)
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
            request.familiar_entity,
            LatinPhrase::Requiesce,
            fam_transform.translation,
            &speech_handles,
            &q_bubbles,
            BubbleEmotion::Neutral,
            BubblePriority::Normal,
            voice_opt,
        );

        if let Some(history) = history_opt.as_mut() {
            history.record_speech(BubblePriority::Normal, current_time);
        } else {
            commands
                .entity(request.familiar_entity)
                .insert(SpeechHistory {
                    last_time: current_time,
                    last_priority: BubblePriority::Normal,
                });
        }
    }
}
