//! 会話イベント起点の表情オーバーレイ

use crate::constants::*;
use crate::entities::damned_soul::{
    ConversationExpression, ConversationExpressionKind, DamnedSoul,
};
use crate::events::{OnExhausted, OnGatheringParticipated};
use crate::systems::soul_ai::helpers::gathering::{GatheringObjectType, GatheringSpot};
use crate::systems::visual::speech::conversation::events::{
    ConversationCompleted, ConversationTone, ConversationToneTriggered,
};
use bevy::prelude::*;

const EXPRESSION_PRIORITY_CONVERSATION_TONE: u8 = 20;
const EXPRESSION_PRIORITY_CONVERSATION_COMPLETED: u8 = 10;
const EXPRESSION_PRIORITY_GATHERING_OBJECT: u8 = 15;
const EXPRESSION_PRIORITY_EXHAUSTED: u8 = 30;

fn tone_to_expression_kind(tone: ConversationTone) -> Option<ConversationExpressionKind> {
    match tone {
        ConversationTone::Positive => Some(ConversationExpressionKind::Positive),
        ConversationTone::Negative => Some(ConversationExpressionKind::Negative),
        ConversationTone::Neutral => None,
    }
}

fn lock_seconds_for_tone_event(tone: ConversationTone) -> Option<f32> {
    match tone {
        ConversationTone::Positive => Some(SOUL_EVENT_LOCK_TONE_POSITIVE),
        ConversationTone::Negative => Some(SOUL_EVENT_LOCK_TONE_NEGATIVE),
        ConversationTone::Neutral => None,
    }
}

fn lock_seconds_for_completed_event(tone: ConversationTone) -> Option<f32> {
    match tone {
        ConversationTone::Positive => Some(SOUL_EVENT_LOCK_COMPLETED_POSITIVE),
        ConversationTone::Negative => Some(SOUL_EVENT_LOCK_COMPLETED_NEGATIVE),
        ConversationTone::Neutral => None,
    }
}

fn apply_expression_lock(
    commands: &mut Commands,
    entity: Entity,
    kind: ConversationExpressionKind,
    lock_secs: f32,
    priority: u8,
    q_expression: &mut Query<&mut ConversationExpression, With<DamnedSoul>>,
) {
    if let Ok(mut expression) = q_expression.get_mut(entity) {
        if priority > expression.priority {
            expression.kind = kind;
            expression.priority = priority;
            expression.remaining_secs = lock_secs;
        } else if priority == expression.priority {
            if expression.kind == kind {
                expression.remaining_secs = expression.remaining_secs.max(lock_secs);
            } else {
                expression.kind = kind;
                expression.remaining_secs = lock_secs;
            }
        }
        return;
    }

    commands.entity(entity).insert(ConversationExpression {
        kind,
        priority,
        remaining_secs: lock_secs,
    });
}

pub fn apply_conversation_expression_event_system(
    mut commands: Commands,
    q_souls: Query<(), With<DamnedSoul>>,
    q_spots: Query<&GatheringSpot>,
    mut q_expression: Query<&mut ConversationExpression, With<DamnedSoul>>,
    mut ev_exhausted_reader: MessageReader<OnExhausted>,
    mut ev_gathering_participated_reader: MessageReader<OnGatheringParticipated>,
    mut ev_tone_reader: MessageReader<ConversationToneTriggered>,
    mut ev_reader: MessageReader<ConversationCompleted>,
) {
    for event in ev_exhausted_reader.read() {
        if q_souls.get(event.entity).is_err() {
            continue;
        }
        apply_expression_lock(
            &mut commands,
            event.entity,
            ConversationExpressionKind::Exhausted,
            SOUL_EVENT_LOCK_EXHAUSTED,
            EXPRESSION_PRIORITY_EXHAUSTED,
            &mut q_expression,
        );
    }

    for event in ev_gathering_participated_reader.read() {
        if q_souls.get(event.entity).is_err() {
            continue;
        }
        let Ok(spot) = q_spots.get(event.spot_entity) else {
            continue;
        };
        let kind = match spot.object_type {
            GatheringObjectType::Barrel => Some(ConversationExpressionKind::GatheringWine),
            GatheringObjectType::CardTable => Some(ConversationExpressionKind::GatheringTrump),
            GatheringObjectType::Nothing | GatheringObjectType::Campfire => None,
        };
        let Some(kind) = kind else {
            continue;
        };

        apply_expression_lock(
            &mut commands,
            event.entity,
            kind,
            SOUL_EVENT_LOCK_GATHERING_OBJECT,
            EXPRESSION_PRIORITY_GATHERING_OBJECT,
            &mut q_expression,
        );
    }

    for event in ev_tone_reader.read() {
        if q_souls.get(event.speaker).is_err() {
            continue;
        }
        let Some(kind) = tone_to_expression_kind(event.tone) else {
            continue;
        };
        let Some(lock_secs) = lock_seconds_for_tone_event(event.tone) else {
            continue;
        };

        apply_expression_lock(
            &mut commands,
            event.speaker,
            kind,
            lock_secs,
            EXPRESSION_PRIORITY_CONVERSATION_TONE,
            &mut q_expression,
        );
    }

    for event in ev_reader.read() {
        let Some(kind) = tone_to_expression_kind(event.tone) else {
            continue;
        };
        let Some(lock_secs) = lock_seconds_for_completed_event(event.tone) else {
            continue;
        };

        for &entity in &event.participants {
            if q_souls.get(entity).is_err() {
                continue;
            }
            apply_expression_lock(
                &mut commands,
                entity,
                kind,
                lock_secs,
                EXPRESSION_PRIORITY_CONVERSATION_COMPLETED,
                &mut q_expression,
            );
        }
    }
}

pub fn update_conversation_expression_timer_system(
    time: Res<Time>,
    mut commands: Commands,
    mut query: Query<(Entity, &mut ConversationExpression), With<DamnedSoul>>,
) {
    let dt = time.delta_secs();
    for (entity, mut expression) in query.iter_mut() {
        expression.remaining_secs -= dt;
        if expression.remaining_secs <= 0.0 {
            commands.entity(entity).remove::<ConversationExpression>();
        }
    }
}
