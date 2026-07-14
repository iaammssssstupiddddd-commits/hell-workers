use super::components::{BubbleEmotion, BubblePriority, FamiliarBubble, SpeechBubble};
use super::cooldown::SpeechHistory;
use super::emitter::{SoulSpeechContent, emit_familiar_with_history, emit_soul_with_history};
use super::phrases::LatinPhrase;
use super::spawn::*;
use super::voice::FamiliarVoice;
use crate::handles::SpeechHandles;
use bevy::prelude::*;
use hw_core::constants::COMMAND_REACTION_NEGATIVE_EVENT_CHANCE;
use hw_core::events::{
    OnGatheringJoined, OnReleasedFromService, OnTaskAbandoned, OnTaskAssigned,
    SoulEncouragedVisualMessage, SoulExhaustedVisualMessage, SoulRecruitedVisualMessage,
    SoulStressBreakdownVisualMessage, TaskCompletedVisualMessage,
};
use hw_core::familiar::Familiar;
use hw_core::relationships::CommandedBy;
use hw_core::soul::DamnedSoul;
use rand::Rng;

use super::conversation::events::{ConversationTone, ConversationToneTriggered};

/// リアクションバブルの遅延秒数
const REACTION_BUBBLE_DELAY_SECS: f32 = 0.3;

/// 遅延リアクションバブルを Delayed Commands で発行する
///
/// バブルは `ChildOf(soul_entity)` で Soul に追従するため、発火時に位置を読む必要はない。
/// 発火前に Soul が despawn した場合は何もしない。
fn queue_delayed_reaction_bubble(
    commands: &mut Commands,
    soul_entity: Entity,
    emoji: &'static str,
    emotion: BubbleEmotion,
) {
    commands
        .delayed()
        .secs(REACTION_BUBBLE_DELAY_SECS)
        .queue(move |world: &mut World| {
            if world.get_entity(soul_entity).is_err() {
                return;
            }
            world.resource_scope(|world, handles: Mut<SpeechHandles>| {
                let mut commands = world.commands();
                spawn_soul_bubble(
                    &mut commands,
                    soul_entity,
                    emoji,
                    Vec3::ZERO,
                    &handles,
                    emotion,
                    BubblePriority::Normal,
                );
            });
        });
}

type SoulTaskSpeechQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static GlobalTransform,
        Option<&'static CommandedBy>,
        Option<&'static mut SpeechHistory>,
    ),
    (With<DamnedSoul>, Without<Familiar>),
>;

type FamiliarTaskSpeechQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static GlobalTransform,
        Option<&'static FamiliarVoice>,
        Option<&'static mut SpeechHistory>,
    ),
    (With<Familiar>, Without<DamnedSoul>),
>;

type SoulHistoryQuery<'w, 's> = Query<
    'w,
    's,
    (&'static GlobalTransform, Option<&'static mut SpeechHistory>),
    (With<DamnedSoul>, Without<Familiar>),
>;

type SoulExistsQuery<'w, 's> = Query<'w, 's, (), With<DamnedSoul>>;

type FamiliarVoiceQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static GlobalTransform,
        Option<&'static FamiliarVoice>,
        Option<&'static mut SpeechHistory>,
    ),
    With<Familiar>,
>;

use bevy::ecs::system::SystemParam;

#[derive(SystemParam)]
pub struct SpeechTaskParams<'w, 's> {
    handles: Res<'w, SpeechHandles>,
    tone_writer: MessageWriter<'w, ConversationToneTriggered>,
    q_souls: SoulTaskSpeechQuery<'w, 's>,
    q_familiars: FamiliarTaskSpeechQuery<'w, 's>,
    q_bubbles: Query<'w, 's, (Entity, &'static SpeechBubble), With<FamiliarBubble>>,
    time: Res<'w, Time>,
}

/// タスク開始時の speech bubble 発火システム（MessageReader ベース）
pub fn speech_on_task_assigned_system(
    mut reader: MessageReader<OnTaskAssigned>,
    mut commands: Commands,
    mut p: SpeechTaskParams,
) {
    for event in reader.read() {
        let soul_entity = event.entity;
        let current_time = p.time.elapsed_secs();

        if let Ok((soul_transform, under_command, soul_history_opt)) =
            p.q_souls.get_mut(soul_entity)
        {
            let soul_pos = soul_transform.translation();
            if under_command.is_some() {
                let mut rng = rand::thread_rng();
                if rng.gen_bool(COMMAND_REACTION_NEGATIVE_EVENT_CHANCE as f64) {
                    p.tone_writer.write(ConversationToneTriggered {
                        speaker: soul_entity,
                        tone: ConversationTone::Negative,
                    });
                }
            }

            emit_soul_with_history(
                &mut commands,
                soul_entity,
                SoulSpeechContent {
                    emoji: "💪",
                    emotion: BubbleEmotion::Motivated,
                    priority: BubblePriority::Low,
                },
                soul_pos,
                &p.handles,
                soul_history_opt,
                current_time,
            );

            if let Some(uc) = under_command
                && let Ok((_fam_transform, voice, fam_history_opt)) = p.q_familiars.get_mut(uc.0)
            {
                let _fam_pos = _fam_transform.translation();
                let phrase = LatinPhrase::from_work_type(event.current_work_type);
                emit_familiar_with_history(
                    &mut commands,
                    uc.0,
                    FamiliarBubbleSpec {
                        phrase,
                        emotion: BubbleEmotion::Motivated,
                        priority: BubblePriority::Low,
                        voice,
                    },
                    &p.handles,
                    &p.q_bubbles,
                    fam_history_opt,
                    current_time,
                );
            }
        }
    }
}

/// タスク完了時の speech bubble 発火システム（MessageReader ベース）
pub fn speech_on_task_completed_system(
    mut reader: MessageReader<TaskCompletedVisualMessage>,
    mut commands: Commands,
    handles: Res<SpeechHandles>,
    mut q_souls: SoulHistoryQuery,
    time: Res<Time>,
) {
    for event in reader.read() {
        let soul_entity = event.entity;
        let current_time = time.elapsed_secs();
        if let Ok((transform, history_opt)) = q_souls.get_mut(soul_entity) {
            emit_soul_with_history(
                &mut commands,
                soul_entity,
                SoulSpeechContent {
                    emoji: "😊",
                    emotion: BubbleEmotion::Happy,
                    priority: BubblePriority::Low,
                },
                transform.translation(),
                &handles,
                history_opt,
                current_time,
            );
        }
    }
}

/// 勧誘時の speech bubble 発火システム。
pub fn speech_on_soul_recruited_system(
    mut reader: MessageReader<SoulRecruitedVisualMessage>,
    mut commands: Commands,
    handles: Res<SpeechHandles>,
    mut tone_writer: MessageWriter<ConversationToneTriggered>,
    mut q_familiars: FamiliarVoiceQuery,
    q_bubbles: Query<(Entity, &SpeechBubble), With<FamiliarBubble>>,
    time: Res<Time>,
) {
    for event in reader.read() {
        let fam_entity = event.familiar_entity;
        let soul_entity = event.entity;
        let current_time = time.elapsed_secs();

        tone_writer.write(ConversationToneTriggered {
            speaker: soul_entity,
            tone: ConversationTone::Negative,
        });

        if let Ok((_transform, voice, history_opt)) = q_familiars.get_mut(fam_entity) {
            emit_familiar_with_history(
                &mut commands,
                fam_entity,
                FamiliarBubbleSpec {
                    phrase: LatinPhrase::Veni,
                    emotion: BubbleEmotion::Neutral,
                    priority: BubblePriority::Normal,
                    voice,
                },
                &handles,
                &q_bubbles,
                history_opt,
                current_time,
            );
        }

        queue_delayed_reaction_bubble(&mut commands, soul_entity, "😨", BubbleEmotion::Fearful);
    }
}

/// 疲労限界時の speech bubble 発火システム。
pub fn speech_on_exhausted_system(
    mut reader: MessageReader<SoulExhaustedVisualMessage>,
    mut commands: Commands,
    handles: Res<SpeechHandles>,
    mut q_souls: SoulHistoryQuery,
    time: Res<Time>,
) {
    for event in reader.read() {
        let soul_entity = event.entity;
        let current_time = time.elapsed_secs();
        if let Ok((transform, history_opt)) = q_souls.get_mut(soul_entity) {
            emit_soul_with_history(
                &mut commands,
                soul_entity,
                SoulSpeechContent {
                    emoji: "😴",
                    emotion: BubbleEmotion::Exhausted,
                    priority: BubblePriority::High,
                },
                transform.translation(),
                &handles,
                history_opt,
                current_time,
            );
        }
    }
}

/// ストレス崩壊時の speech bubble 発火システム。
pub fn speech_on_stress_breakdown_system(
    mut reader: MessageReader<SoulStressBreakdownVisualMessage>,
    mut commands: Commands,
    handles: Res<SpeechHandles>,
    mut q_souls: SoulHistoryQuery,
    time: Res<Time>,
) {
    for event in reader.read() {
        let soul_entity = event.entity;
        let current_time = time.elapsed_secs();
        if let Ok((transform, history_opt)) = q_souls.get_mut(soul_entity) {
            emit_soul_with_history(
                &mut commands,
                soul_entity,
                SoulSpeechContent {
                    emoji: "😰",
                    emotion: BubbleEmotion::Stressed,
                    priority: BubblePriority::Critical,
                },
                transform.translation(),
                &handles,
                history_opt,
                current_time,
            );
        }
    }
}

/// 使役解放時のリアクション
pub fn speech_on_released_from_service_system(
    mut reader: MessageReader<OnReleasedFromService>,
    mut commands: Commands,
    handles: Res<SpeechHandles>,
    q_souls: SoulExistsQuery,
) {
    for event in reader.read() {
        if q_souls.get(event.entity).is_err() {
            continue;
        }
        spawn_soul_bubble(
            &mut commands,
            event.entity,
            "😅",
            Vec3::ZERO,
            &handles,
            BubbleEmotion::Relieved,
            BubblePriority::Normal,
        );
    }
}

/// 集会参加時のリアクション
pub fn speech_on_gathering_joined_system(
    mut reader: MessageReader<OnGatheringJoined>,
    mut commands: Commands,
    handles: Res<SpeechHandles>,
    q_souls: SoulExistsQuery,
) {
    for event in reader.read() {
        if q_souls.get(event.entity).is_err() {
            continue;
        }
        spawn_soul_bubble(
            &mut commands,
            event.entity,
            "😌",
            Vec3::ZERO,
            &handles,
            BubbleEmotion::Relaxed,
            BubblePriority::Normal,
        );
    }
}

/// タスク中断・失敗時のリアクション
pub fn speech_on_task_abandoned_system(
    mut reader: MessageReader<OnTaskAbandoned>,
    mut commands: Commands,
    handles: Res<SpeechHandles>,
    q_souls: SoulExistsQuery,
) {
    for event in reader.read() {
        if q_souls.get(event.entity).is_err() {
            continue;
        }
        spawn_soul_bubble(
            &mut commands,
            event.entity,
            "🙅‍♂️",
            Vec3::ZERO,
            &handles,
            BubbleEmotion::Unmotivated,
            BubblePriority::Normal,
        );
    }
}

/// 激励時のリアクション
pub fn speech_on_encouraged_system(
    mut reader: MessageReader<SoulEncouragedVisualMessage>,
    mut commands: Commands,
    handles: Res<SpeechHandles>,
    mut q_familiars: FamiliarVoiceQuery,
    q_bubbles: Query<(Entity, &SpeechBubble), With<FamiliarBubble>>,
    time: Res<Time>,
) {
    for event in reader.read() {
        let fam_entity = event.familiar_entity;
        let soul_entity = event.soul_entity;
        let current_time = time.elapsed_secs();

        if let Ok((_transform, voice, history_opt)) = q_familiars.get_mut(fam_entity) {
            use rand::seq::SliceRandom;
            let mut rng = rand::thread_rng();
            let emoji = hw_core::constants::EMOJIS_ENCOURAGEMENT
                .choose(&mut rng)
                .unwrap_or(&"💪");

            emit_familiar_with_history(
                &mut commands,
                fam_entity,
                FamiliarBubbleSpec {
                    phrase: LatinPhrase::Custom(emoji.to_string()),
                    emotion: BubbleEmotion::Motivated,
                    priority: BubblePriority::Normal,
                    voice,
                },
                &handles,
                &q_bubbles,
                history_opt,
                current_time,
            );
        }

        queue_delayed_reaction_bubble(&mut commands, soul_entity, "😓", BubbleEmotion::Stressed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::app::ScheduleRunnerPlugin;

    fn speech_handles() -> SpeechHandles {
        SpeechHandles {
            bubble_9slice: Handle::default(),
            glow_circle: Handle::default(),
            font_familiar: Handle::default(),
            font_soul_name: Handle::default(),
            font_soul_emoji: Handle::default(),
        }
    }

    #[test]
    fn message_only_speech_notifications_ignore_despawned_souls() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins.set(ScheduleRunnerPlugin::run_once()))
            .add_message::<OnReleasedFromService>()
            .add_message::<OnGatheringJoined>()
            .add_message::<OnTaskAbandoned>()
            .insert_resource(speech_handles())
            .add_systems(
                Update,
                (
                    speech_on_released_from_service_system,
                    speech_on_gathering_joined_system,
                    speech_on_task_abandoned_system,
                ),
            );

        let soul = app.world_mut().spawn(DamnedSoul::default()).id();
        assert!(app.world_mut().despawn(soul));
        app.world_mut()
            .write_message(OnReleasedFromService { entity: soul });
        app.world_mut()
            .write_message(OnGatheringJoined { entity: soul });
        app.world_mut()
            .write_message(OnTaskAbandoned { entity: soul });

        app.update();

        let bubble_count = app
            .world()
            .iter_entities()
            .filter(|entity| entity.contains::<SpeechBubble>())
            .count();
        assert_eq!(bubble_count, 0);
    }
}
