use super::components::*;
use super::events::*;
use crate::assets::GameAssets;
use crate::constants::*;
use crate::entities::damned_soul::{DamnedSoul, IdleBehavior, IdleState};
use crate::systems::spatial::grid::SpatialGridOps;
use crate::systems::spatial::soul::SpatialGrid;
use crate::systems::visual::speech::components::{BubbleEmotion, BubblePriority};
use crate::systems::visual::speech::spawn::spawn_soul_bubble;
use bevy::prelude::*;
use rand::Rng;
use rand::seq::SliceRandom;
use std::collections::HashMap;

pub fn check_conversation_triggers(
    time: Res<Time>,
    grid: Res<SpatialGrid>,
    mut q_initiator: Query<
        (Entity, &Transform, &IdleState, &mut ConversationInitiator),
        (
            With<DamnedSoul>,
            Without<ConversationParticipant>,
            Without<ConversationCooldown>,
        ),
    >,
    q_target: Query<
        (Entity, &IdleState),
        (
            With<DamnedSoul>,
            Without<ConversationParticipant>,
            Without<ConversationCooldown>,
        ),
    >,
    mut ev_writer: MessageWriter<RequestConversation>,
) {
    let dt = time.delta_secs();
    let mut rng = rand::thread_rng();

    for (entity, transform, idle_state, mut initiator) in q_initiator.iter_mut() {
        initiator.timer.tick(std::time::Duration::from_secs_f32(dt));

        if initiator.timer.just_finished() {
            let pos = transform.translation.truncate();
            let nearby = grid.get_nearby_in_radius(pos, CONVERSATION_RADIUS);

            for &target_entity in nearby.iter() {
                if target_entity == entity {
                    continue;
                }

                if let Ok((_target_entity, target_idle)) = q_target.get(target_entity) {
                    let initiator_can_chat = matches!(
                        idle_state.behavior,
                        IdleBehavior::Wandering
                            | IdleBehavior::Sitting
                            | IdleBehavior::Sleeping
                            | IdleBehavior::Gathering
                            | IdleBehavior::ExhaustedGathering
                    );
                    let target_can_chat = matches!(
                        target_idle.behavior,
                        IdleBehavior::Wandering
                            | IdleBehavior::Sitting
                            | IdleBehavior::Sleeping
                            | IdleBehavior::Gathering
                            | IdleBehavior::ExhaustedGathering
                    );

                    if initiator_can_chat && target_can_chat {
                        let is_gathering = matches!(
                            idle_state.behavior,
                            IdleBehavior::Gathering | IdleBehavior::ExhaustedGathering
                        );
                        let chance = if is_gathering {
                            CONVERSATION_CHANCE_GATHERING
                        } else {
                            CONVERSATION_CHANCE_IDLE
                        };

                        if rng.gen_bool(chance as f64) {
                            ev_writer.write(RequestConversation {
                                initiator: entity,
                                target: target_entity,
                            });
                            break;
                        }
                    }
                }
            }
        }
    }
}

pub fn handle_conversation_requests(
    mut commands: Commands,
    mut ev_reader: MessageReader<RequestConversation>,
    q_souls: Query<
        Entity,
        (
            With<DamnedSoul>,
            Without<ConversationParticipant>,
            Without<ConversationCooldown>,
        ),
    >,
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

pub fn process_conversation_logic(
    time: Res<Time>,
    mut commands: Commands,
    assets: Res<GameAssets>,
    mut q_participants: Query<(Entity, &mut ConversationParticipant, &Transform, &IdleState)>,
    mut ev_tone: MessageWriter<ConversationToneTriggered>,
    mut ev_completed: MessageWriter<ConversationCompleted>,
) {
    let dt = time.delta_secs();
    let mut rng = rand::thread_rng();

    let participant_entities: Vec<Entity> = q_participants.iter().map(|(e, _, _, _)| e).collect();
    let participant_tone_snapshot: HashMap<Entity, (u8, u8)> = q_participants
        .iter()
        .map(|(e, p, _, _)| (e, (p.positive_turns, p.negative_turns)))
        .collect();

    for (entity, mut participant, transform, idle_state) in q_participants.iter_mut() {
        participant
            .timer
            .tick(std::time::Duration::from_secs_f32(dt));

        if !participant_entities.contains(&participant.target) {
            end_conversation(&mut commands, entity, None);
            continue;
        }

        if participant.timer.just_finished() {
            let pos = transform.translation;

            match participant.phase {
                ConversationPhase::Greeting => {
                    let emoji = EMOJIS_GREETING.choose(&mut rng).unwrap();
                    spawn_soul_bubble(
                        &mut commands,
                        entity,
                        emoji,
                        pos,
                        &assets,
                        BubbleEmotion::Chatting,
                        BubblePriority::Normal,
                    );

                    participant.phase = ConversationPhase::Chatting;
                    participant.timer =
                        Timer::from_seconds(CONVERSATION_TURN_DURATION, TimerMode::Once);
                    participant.turns += 1;
                }
                ConversationPhase::Chatting => {
                    let is_gathering = matches!(
                        idle_state.behavior,
                        IdleBehavior::Gathering | IdleBehavior::ExhaustedGathering
                    );
                    let max_turns = if is_gathering { 2 } else { 1 };

                    if participant.turns <= max_turns {
                        let emoji_set = if is_gathering {
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
                        };
                        let emoji = emoji_set.choose(&mut rng).unwrap();

                        let emotion = if emoji_set == EMOJIS_FOOD {
                            participant.positive_turns =
                                participant.positive_turns.saturating_add(1);
                            ev_tone.write(ConversationToneTriggered {
                                speaker: entity,
                                tone: ConversationTone::Positive,
                            });
                            BubbleEmotion::Happy
                        } else if emoji_set == EMOJIS_SLACKING {
                            BubbleEmotion::Slacking
                        } else if emoji_set == EMOJIS_COMPLAINING {
                            participant.negative_turns =
                                participant.negative_turns.saturating_add(1);
                            ev_tone.write(ConversationToneTriggered {
                                speaker: entity,
                                tone: ConversationTone::Negative,
                            });
                            BubbleEmotion::Chatting
                        } else {
                            BubbleEmotion::Chatting
                        };

                        spawn_soul_bubble(
                            &mut commands,
                            entity,
                            emoji,
                            pos,
                            &assets,
                            emotion,
                            BubblePriority::Normal,
                        );

                        participant.timer =
                            Timer::from_seconds(CONVERSATION_TURN_DURATION * 1.5, TimerMode::Once);
                        participant.turns += 1;
                    } else {
                        participant.phase = ConversationPhase::Closing;
                        participant.timer =
                            Timer::from_seconds(CONVERSATION_TURN_DURATION, TimerMode::Once);
                    }
                }
                ConversationPhase::Closing => {
                    let is_gathering = matches!(
                        idle_state.behavior,
                        IdleBehavior::Gathering | IdleBehavior::ExhaustedGathering
                    );
                    let agreement_chance = if is_gathering { 0.95 } else { 0.5 };
                    if rng.gen_bool(agreement_chance) {
                        participant.positive_turns = participant.positive_turns.saturating_add(1);
                        ev_tone.write(ConversationToneTriggered {
                            speaker: entity,
                            tone: ConversationTone::Positive,
                        });
                        let emoji = EMOJIS_AGREEMENT.choose(&mut rng).unwrap();
                        spawn_soul_bubble(
                            &mut commands,
                            entity,
                            emoji,
                            pos,
                            &assets,
                            BubbleEmotion::Relieved,
                            BubblePriority::Normal,
                        );
                    }

                    if participant.role == ConversationRole::Initiator {
                        let mut positive_turns = participant.positive_turns;
                        let mut negative_turns = participant.negative_turns;
                        if let Some((target_pos, target_neg)) =
                            participant_tone_snapshot.get(&participant.target)
                        {
                            positive_turns = positive_turns.saturating_add(*target_pos);
                            negative_turns = negative_turns.saturating_add(*target_neg);
                        }
                        let is_gathering = matches!(
                            idle_state.behavior,
                            IdleBehavior::Gathering | IdleBehavior::ExhaustedGathering
                        );
                        let tone = if positive_turns > negative_turns {
                            ConversationTone::Positive
                        } else if negative_turns > positive_turns {
                            ConversationTone::Negative
                        } else if is_gathering {
                            ConversationTone::Positive
                        } else {
                            ConversationTone::Neutral
                        };
                        ev_completed.write(ConversationCompleted {
                            participants: vec![entity, participant.target],
                            turns: participant.turns,
                            tone,
                        });
                    }

                    end_conversation(&mut commands, entity, Some(CONVERSATION_COOLDOWN));
                }
            }
        }
    }
}

fn end_conversation(commands: &mut Commands, entity: Entity, cooldown: Option<f32>) {
    commands.entity(entity).remove::<ConversationParticipant>();
    if let Some(dur) = cooldown {
        commands.entity(entity).insert(ConversationCooldown {
            timer: Timer::from_seconds(dur, TimerMode::Once),
        });
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
