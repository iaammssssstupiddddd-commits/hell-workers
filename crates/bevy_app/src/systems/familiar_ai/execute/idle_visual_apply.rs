use crate::entities::familiar::{Familiar, FamiliarVoice};
use crate::events::FamiliarIdleVisualRequest;
use hw_visual::speech::components::{
    BubbleEmotion, BubblePriority, FamiliarBubble, SpeechBubble,
};
use bevy::prelude::*;

/// Idle遷移時のビジュアル演出を適用する（Execute Phase）
pub fn familiar_idle_visual_apply_system(
    mut commands: Commands,
    time: Res<Time>,
    mut request_reader: MessageReader<FamiliarIdleVisualRequest>,
    q_bubbles: Query<(Entity, &SpeechBubble), With<FamiliarBubble>>,
    speech_handles: Res<hw_visual::SpeechHandles>,
    mut q_familiars: Query<
        (
            &Transform,
            Option<&FamiliarVoice>,
            Option<&mut hw_visual::speech::cooldown::SpeechHistory>,
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

        hw_visual::speech::spawn::spawn_familiar_bubble(
            &mut commands,
            request.familiar_entity,
            hw_visual::speech::phrases::LatinPhrase::Requiesce,
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
            commands.entity(request.familiar_entity).insert(
                hw_visual::speech::cooldown::SpeechHistory {
                    last_time: current_time,
                    last_priority: BubblePriority::Normal,
                },
            );
        }
    }
}
