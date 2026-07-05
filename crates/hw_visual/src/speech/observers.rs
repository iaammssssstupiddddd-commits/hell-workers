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
    OnEncouraged, OnExhausted, OnGatheringJoined, OnReleasedFromService, OnSoulRecruited,
    OnStressBreakdown, OnTaskAbandoned, OnTaskAssigned, OnTaskCompleted,
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
                let phrase = LatinPhrase::from_work_type(event.work_type);
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
    mut reader: MessageReader<OnTaskCompleted>,
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

/// 勧誘時のオブザーバー（使い魔の発言）
pub fn on_soul_recruited(
    on: On<OnSoulRecruited>,
    mut commands: Commands,
    handles: Res<SpeechHandles>,
    mut tone_writer: MessageWriter<ConversationToneTriggered>,
    mut q_familiars: FamiliarVoiceQuery,
    q_bubbles: Query<(Entity, &SpeechBubble), With<FamiliarBubble>>,
    time: Res<Time>,
) {
    let fam_entity = on.event().familiar_entity;
    let soul_entity = on.entity;
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

/// 疲労限界時のオブザーバー
pub fn on_exhausted(
    on: On<OnExhausted>,
    mut commands: Commands,
    handles: Res<SpeechHandles>,
    mut q_souls: SoulHistoryQuery,
    time: Res<Time>,
) {
    let soul_entity = on.entity;
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

/// ストレス崩壊時のオブザーバー
pub fn on_stress_breakdown(
    on: On<OnStressBreakdown>,
    mut commands: Commands,
    handles: Res<SpeechHandles>,
    mut q_souls: SoulHistoryQuery,
    time: Res<Time>,
) {
    let soul_entity = on.entity;
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

/// 使役解放時のリアクション
pub fn on_released_from_service(
    on: On<OnReleasedFromService>,
    mut commands: Commands,
    handles: Res<SpeechHandles>,
) {
    spawn_soul_bubble(
        &mut commands,
        on.entity,
        "😅",
        Vec3::ZERO,
        &handles,
        BubbleEmotion::Relieved,
        BubblePriority::Normal,
    );
}

/// 集会参加時のリアクション
pub fn on_gathering_joined(
    on: On<OnGatheringJoined>,
    mut commands: Commands,
    handles: Res<SpeechHandles>,
) {
    spawn_soul_bubble(
        &mut commands,
        on.entity,
        "😌",
        Vec3::ZERO,
        &handles,
        BubbleEmotion::Relaxed,
        BubblePriority::Normal,
    );
}

/// タスク中断・失敗時のリアクション
pub fn on_task_abandoned(
    on: On<OnTaskAbandoned>,
    mut commands: Commands,
    handles: Res<SpeechHandles>,
) {
    spawn_soul_bubble(
        &mut commands,
        on.entity,
        "🙅‍♂️",
        Vec3::ZERO,
        &handles,
        BubbleEmotion::Unmotivated,
        BubblePriority::Normal,
    );
}

/// 激励時のリアクション
pub fn on_encouraged(
    on: On<OnEncouraged>,
    mut commands: Commands,
    handles: Res<SpeechHandles>,
    mut q_familiars: FamiliarVoiceQuery,
    q_bubbles: Query<(Entity, &SpeechBubble), With<FamiliarBubble>>,
    time: Res<Time>,
) {
    let event = on.event();
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
