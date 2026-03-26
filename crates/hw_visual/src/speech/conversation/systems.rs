use super::bubble_spawn_helpers;
use super::components::*;
use super::events::*;
use super::phase_handlers;
use crate::handles::SpeechHandles;
use bevy::prelude::*;
use hw_core::constants::*;
use hw_core::soul::{DamnedSoul, IdleBehavior, IdleState};
use hw_spatial::{SpatialGrid, SpatialGridOps};
use rand::Rng;
use std::collections::HashMap;

type ConversationInitiatorQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Transform,
        &'static IdleState,
        &'static mut ConversationInitiator,
    ),
    (
        With<DamnedSoul>,
        Without<ConversationParticipant>,
        Without<ConversationCooldown>,
    ),
>;

type ConversationTargetQuery<'w, 's> = Query<
    'w,
    's,
    (Entity, &'static IdleState),
    (
        With<DamnedSoul>,
        Without<ConversationParticipant>,
        Without<ConversationCooldown>,
    ),
>;
pub fn check_conversation_triggers(
    time: Res<Time>,
    grid: Res<SpatialGrid>,
    mut nearby_buf: Local<Vec<Entity>>,
    mut q_initiator: ConversationInitiatorQuery,
    q_target: ConversationTargetQuery,
    mut ev_writer: MessageWriter<RequestConversation>,
) {
    let dt = time.delta_secs();
    let mut rng = rand::thread_rng();

    for (entity, transform, idle_state, mut initiator) in q_initiator.iter_mut() {
        initiator.timer.tick(std::time::Duration::from_secs_f32(dt));

        if initiator.timer.just_finished() {
            let pos = transform.translation.truncate();
            grid.get_nearby_in_radius_into(pos, CONVERSATION_RADIUS, &mut nearby_buf);

            for &target_entity in nearby_buf.iter() {
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

pub fn process_conversation_logic(
    time: Res<Time>,
    mut commands: Commands,
    handles: Res<SpeechHandles>,
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
            phase_handlers::end_conversation(&mut commands, entity, None);
            continue;
        }

        if participant.timer.just_finished() {
            let pos = transform.translation;
            let is_gathering = matches!(
                idle_state.behavior,
                IdleBehavior::Gathering | IdleBehavior::ExhaustedGathering
            );

            match participant.phase {
                ConversationPhase::Greeting => {
                    phase_handlers::handle_greeting_phase(
                        bubble_spawn_helpers::BubbleSpawnCtx {
                            commands: &mut commands,
                            entity,
                            pos,
                            rng: &mut rng,
                        },
                        &handles,
                        &mut participant,
                    );
                }
                ConversationPhase::Chatting => {
                    if let Some(tone_event) = phase_handlers::handle_chatting_phase(
                        bubble_spawn_helpers::BubbleSpawnCtx {
                            commands: &mut commands,
                            entity,
                            pos,
                            rng: &mut rng,
                        },
                        &handles,
                        &mut participant,
                        is_gathering,
                    ) {
                        ev_tone.write(tone_event);
                    }
                }
                ConversationPhase::Closing => {
                    let result = phase_handlers::handle_closing_phase(
                        bubble_spawn_helpers::BubbleSpawnCtx {
                            commands: &mut commands,
                            entity,
                            pos,
                            rng: &mut rng,
                        },
                        &handles,
                        &mut participant,
                        &participant_tone_snapshot,
                        is_gathering,
                    );
                    if let Some(tone_event) = result.tone_trigger {
                        ev_tone.write(tone_event);
                    }
                    if let Some(completed_event) = result.completed {
                        ev_completed.write(completed_event);
                    }
                }
            }
        }
    }
}
