use crate::handles::SpeechHandles;
use crate::speech::components::{BubbleEmotion, BubblePriority};
use crate::speech::spawn::spawn_soul_bubble;
use bevy::prelude::*;
use hw_core::constants::*;
use rand::Rng;
use rand::seq::SliceRandom;

pub struct BubbleSpawnCtx<'a, 'w, 's, R: Rng + ?Sized> {
    pub commands: &'a mut Commands<'w, 's>,
    pub entity: Entity,
    pub pos: Vec3,
    pub rng: &'a mut R,
}

pub enum ChatBubbleTone {
    Positive,
    Negative,
    Slacking,
    Neutral,
}

pub fn spawn_greeting_bubble<R: Rng + ?Sized>(
    ctx: BubbleSpawnCtx<'_, '_, '_, R>,
    handles: &Res<SpeechHandles>,
) {
    let emoji = EMOJIS_GREETING
        .choose(ctx.rng)
        .expect("EMOJIS_GREETING is non-empty");
    spawn_soul_bubble(
        ctx.commands,
        ctx.entity,
        emoji,
        ctx.pos,
        handles,
        BubbleEmotion::Chatting,
        BubblePriority::Normal,
    );
}

pub fn spawn_chatting_bubble<R: Rng + ?Sized>(
    ctx: BubbleSpawnCtx<'_, '_, '_, R>,
    emoji_set: &[&str],
    handles: &Res<SpeechHandles>,
) -> ChatBubbleTone {
    let emoji = emoji_set.choose(ctx.rng).expect("emoji_set is non-empty");

    let emotion = if emoji_set == EMOJIS_FOOD {
        (BubbleEmotion::Happy, ChatBubbleTone::Positive)
    } else if emoji_set == EMOJIS_SLACKING {
        (BubbleEmotion::Slacking, ChatBubbleTone::Slacking)
    } else if emoji_set == EMOJIS_COMPLAINING {
        (BubbleEmotion::Chatting, ChatBubbleTone::Negative)
    } else {
        (BubbleEmotion::Chatting, ChatBubbleTone::Neutral)
    };

    spawn_soul_bubble(
        ctx.commands,
        ctx.entity,
        emoji,
        ctx.pos,
        handles,
        emotion.0,
        BubblePriority::Normal,
    );

    emotion.1
}

pub fn spawn_agreement_bubble<R: Rng + ?Sized>(
    ctx: BubbleSpawnCtx<'_, '_, '_, R>,
    handles: &Res<SpeechHandles>,
) {
    let emoji = EMOJIS_AGREEMENT
        .choose(ctx.rng)
        .expect("EMOJIS_AGREEMENT is non-empty");
    spawn_soul_bubble(
        ctx.commands,
        ctx.entity,
        emoji,
        ctx.pos,
        handles,
        BubbleEmotion::Relieved,
        BubblePriority::Normal,
    );
}
