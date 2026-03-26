use super::bubble_spawn_helpers;
use super::components::*;
use super::events::*;
use crate::handles::SpeechHandles;
use bevy::prelude::*;
use hw_core::constants::*;
use hw_core::soul::DamnedSoul;
use rand::Rng;
use std::collections::HashMap;

type AvailableSoulQuery<'w, 's> = Query<
    'w,
    's,
    Entity,
    (
        With<DamnedSoul>,
        Without<ConversationParticipant>,
        Without<ConversationCooldown>,
    ),
>;

pub fn end_conversation(commands: &mut Commands, entity: Entity, cooldown: Option<f32>) {
    commands.entity(entity).remove::<ConversationParticipant>();
    if let Some(dur) = cooldown {
        commands.entity(entity).insert(ConversationCooldown {
            timer: Timer::from_seconds(dur, TimerMode::Once),
        });
    }
}

pub fn handle_greeting_phase(
    bubble_ctx: bubble_spawn_helpers::BubbleSpawnCtx<'_, '_, '_, impl Rng>,
    handles: &Res<SpeechHandles>,
    participant: &mut ConversationParticipant,
) {
    bubble_spawn_helpers::spawn_greeting_bubble(bubble_ctx, handles);
    participant.phase = ConversationPhase::Chatting;
    participant.timer = Timer::from_seconds(CONVERSATION_TURN_DURATION, TimerMode::Once);
    participant.turns += 1;
}

pub fn handle_chatting_phase(
    bubble_ctx: bubble_spawn_helpers::BubbleSpawnCtx<'_, '_, '_, impl Rng>,
    handles: &Res<SpeechHandles>,
    participant: &mut ConversationParticipant,
    is_gathering: bool,
) -> Option<ConversationToneTriggered> {
    let max_turns = if is_gathering { 2 } else { 1 };
    if participant.turns <= max_turns {
        let entity = bubble_ctx.entity;
        let emoji_set = select_chatting_emoji_set(is_gathering, bubble_ctx.rng);
        let tone = bubble_spawn_helpers::spawn_chatting_bubble(bubble_ctx, emoji_set, handles);
        match tone {
            bubble_spawn_helpers::ChatBubbleTone::Positive => {
                participant.positive_turns = participant.positive_turns.saturating_add(1);
                participant.timer =
                    Timer::from_seconds(CONVERSATION_TURN_DURATION * 1.5, TimerMode::Once);
                participant.turns += 1;
                Some(ConversationToneTriggered {
                    speaker: entity,
                    tone: ConversationTone::Positive,
                })
            }
            bubble_spawn_helpers::ChatBubbleTone::Negative => {
                participant.negative_turns = participant.negative_turns.saturating_add(1);
                participant.timer =
                    Timer::from_seconds(CONVERSATION_TURN_DURATION * 1.5, TimerMode::Once);
                participant.turns += 1;
                Some(ConversationToneTriggered {
                    speaker: entity,
                    tone: ConversationTone::Negative,
                })
            }
            bubble_spawn_helpers::ChatBubbleTone::Slacking
            | bubble_spawn_helpers::ChatBubbleTone::Neutral => {
                participant.timer =
                    Timer::from_seconds(CONVERSATION_TURN_DURATION * 1.5, TimerMode::Once);
                participant.turns += 1;
                None
            }
        }
    } else {
        participant.phase = ConversationPhase::Closing;
        participant.timer = Timer::from_seconds(CONVERSATION_TURN_DURATION, TimerMode::Once);
        None
    }
}

pub struct ClosingPhaseResult {
    pub tone_trigger: Option<ConversationToneTriggered>,
    pub completed: Option<ConversationCompleted>,
}

pub fn handle_closing_phase(
    bubble_ctx: bubble_spawn_helpers::BubbleSpawnCtx<'_, '_, '_, impl Rng>,
    handles: &Res<SpeechHandles>,
    participant: &mut ConversationParticipant,
    tone_snapshot: &HashMap<Entity, (u8, u8)>,
    is_gathering: bool,
) -> ClosingPhaseResult {
    let bubble_spawn_helpers::BubbleSpawnCtx {
        commands,
        entity,
        pos,
        rng,
    } = bubble_ctx;
    let mut tone_trigger = None;
    let agreement_chance = if is_gathering { 0.95 } else { 0.5 };
    if rng.gen_bool(agreement_chance) {
        bubble_spawn_helpers::spawn_agreement_bubble(
            bubble_spawn_helpers::BubbleSpawnCtx {
                commands,
                entity,
                pos,
                rng,
            },
            handles,
        );
        participant.positive_turns = participant.positive_turns.saturating_add(1);
        tone_trigger = Some(ConversationToneTriggered {
            speaker: entity,
            tone: ConversationTone::Positive,
        });
    }

    let completed = if participant.role == ConversationRole::Initiator {
        let mut positive_turns = participant.positive_turns;
        let mut negative_turns = participant.negative_turns;
        if let Some((target_pos, target_neg)) = tone_snapshot.get(&participant.target) {
            positive_turns = positive_turns.saturating_add(*target_pos);
            negative_turns = negative_turns.saturating_add(*target_neg);
        }
        let tone = if positive_turns > negative_turns {
            ConversationTone::Positive
        } else if negative_turns > positive_turns {
            ConversationTone::Negative
        } else if is_gathering {
            ConversationTone::Positive
        } else {
            ConversationTone::Neutral
        };
        Some(ConversationCompleted {
            participants: vec![entity, participant.target],
            turns: participant.turns,
            tone,
        })
    } else {
        None
    };

    end_conversation(commands, entity, Some(CONVERSATION_COOLDOWN));
    ClosingPhaseResult {
        tone_trigger,
        completed,
    }
}

fn select_chatting_emoji_set(is_gathering: bool, rng: &mut impl Rng) -> &'static [&'static str] {
    if is_gathering {
        // 集会中はポジティブ寄りの会話を増やす
        let roll: f32 = rng.gen_range(0.0..1.0);
        if roll < 0.75 {
            EMOJIS_FOOD
        } else if roll < 0.88 {
            EMOJIS_QUESTION
        } else if roll < 0.98 {
            EMOJIS_SLACKING
        } else {
            EMOJIS_COMPLAINING
        }
    } else if rng.gen_bool(0.2) {
        EMOJIS_QUESTION
    } else if rng.gen_bool(0.3) {
        EMOJIS_FOOD
    } else if rng.gen_bool(0.4) {
        EMOJIS_SLACKING
    } else {
        EMOJIS_COMPLAINING
    }
}

pub fn handle_conversation_requests(
    mut commands: Commands,
    mut ev_reader: MessageReader<RequestConversation>,
    q_souls: AvailableSoulQuery,
) {
    for event in ev_reader.read() {
        if q_souls.get(event.initiator).is_ok() && q_souls.get(event.target).is_ok() {
            commands
                .entity(event.initiator)
                .insert(ConversationParticipant {
                    target: event.target,
                    role: ConversationRole::Initiator,
                    phase: ConversationPhase::Greeting,
                    timer: Timer::from_seconds(0.1, TimerMode::Once),
                    turns: 0,
                    positive_turns: 0,
                    negative_turns: 0,
                });
            commands
                .entity(event.target)
                .insert(ConversationParticipant {
                    target: event.initiator,
                    role: ConversationRole::Responder,
                    phase: ConversationPhase::Greeting,
                    timer: Timer::from_seconds(1.0, TimerMode::Once),
                    turns: 0,
                    positive_turns: 0,
                    negative_turns: 0,
                });
        }
    }
}

pub fn apply_conversation_rewards(
    mut ev_reader: MessageReader<ConversationCompleted>,
    mut q_souls: Query<&mut DamnedSoul>,
) {
    for event in ev_reader.read() {
        let is_long_chat = event.turns > 2;
        let relief = if is_long_chat {
            CONVERSATION_STRESS_RELIEF + CONVERSATION_LONG_CHAT_BONUS
        } else {
            CONVERSATION_STRESS_RELIEF
        };

        for &entity in &event.participants {
            if let Ok(mut soul) = q_souls.get_mut(entity) {
                soul.stress = (soul.stress - relief / 100.0).max(0.0);
                // 会話によるモチベーション減少（サボり）
                soul.motivation = (soul.motivation - MOTIVATION_PENALTY_CONVERSATION).max(0.0);
            }
        }
    }
}

pub fn update_conversation_cooldowns(
    time: Res<Time>,
    mut commands: Commands,
    mut q_cooldowns: Query<(Entity, &mut ConversationCooldown)>,
) {
    let dt = time.delta_secs();
    for (entity, mut cooldown) in q_cooldowns.iter_mut() {
        cooldown.timer.tick(std::time::Duration::from_secs_f32(dt));
        if cooldown.timer.just_finished() {
            commands.entity(entity).remove::<ConversationCooldown>();
        }
    }
}
