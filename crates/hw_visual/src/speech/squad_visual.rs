//! 分隊管理ビジュアル演出：Fatigued リリース時の Familiar の "Abi" セリフ表示。

use bevy::prelude::*;
use hw_core::events::{ReleaseReason, SquadManagementOperation, SquadManagementRequest};
use hw_core::familiar::Familiar;

use super::components::{BubbleEmotion, BubblePriority, FamiliarBubble, SpeechBubble};
use super::cooldown::SpeechHistory;
use super::phrases::LatinPhrase;
use super::spawn::{FamiliarBubbleSpec, spawn_familiar_bubble};
use super::voice::FamiliarVoice;
use crate::handles::SpeechHandles;

type FamiliarsQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static Transform,
        Option<&'static FamiliarVoice>,
        Option<&'static mut SpeechHistory>,
    ),
    With<Familiar>,
>;

/// Fatigued リリース時に Familiar の "Abi" セリフバブルを表示するビジュアルシステム
///
/// ECS 操作は `hw_familiar_ai::squad_logic_system` が担当。
pub fn squad_visual_system(
    mut request_reader: MessageReader<SquadManagementRequest>,
    mut q_familiars: FamiliarsQuery,
    speech_handles: Res<SpeechHandles>,
    q_bubbles: Query<(Entity, &SpeechBubble), With<FamiliarBubble>>,
    time: Res<Time>,
    mut commands: Commands,
) {
    for request in request_reader.read() {
        let SquadManagementOperation::ReleaseMember { reason, .. } = &request.operation else {
            continue;
        };
        if !matches!(reason, ReleaseReason::Fatigued) {
            continue;
        }

        let fam_entity = request.familiar_entity;
        let Ok((_fam_transform, voice_opt, history_opt)) = q_familiars.get_mut(fam_entity) else {
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
            fam_entity,
            FamiliarBubbleSpec { phrase: LatinPhrase::Abi, emotion: BubbleEmotion::Neutral, priority: BubblePriority::Normal, voice: voice_opt },
            &speech_handles,
            &q_bubbles,
        );

        if let Some(mut history) = history_opt {
            history.record_speech(BubblePriority::Normal, current_time);
        } else {
            commands.entity(fam_entity).insert(SpeechHistory {
                last_time: current_time,
                last_priority: BubblePriority::Normal,
            });
        }
    }
}
