use super::components::*;
use super::phrases::LatinPhrase;
use super::spawn::*;
use crate::assets::GameAssets;
use crate::entities::familiar::{Familiar, FamiliarVoice, UnderCommand};
use crate::events::*;
use crate::systems::jobs::WorkType;
use bevy::prelude::*;

/// タスク開始時のオブザーバー
pub fn on_task_assigned(
    on: On<OnTaskAssigned>,
    mut commands: Commands,
    assets: Res<GameAssets>,
    q_souls: Query<(&GlobalTransform, Option<&UnderCommand>)>,
    q_familiars: Query<(&GlobalTransform, Option<&FamiliarVoice>), With<Familiar>>,
    q_bubbles: Query<(Entity, &SpeechBubble), With<FamiliarBubble>>,
    time: Res<Time>,
    mut cooldowns: ResMut<crate::systems::visual::speech::cooldown::BubbleCooldowns>,
) {
    let soul_entity = on.entity;
    let event = on.event();
    let current_time = time.elapsed_secs();

    if let Ok((soul_transform, under_command)) = q_souls.get(soul_entity) {
        let soul_pos = soul_transform.translation();

        // Soul: 「やる気」絵文字 (Low)
        if cooldowns.can_speak(soul_entity, BubblePriority::Low, current_time) {
            spawn_soul_bubble(
                &mut commands,
                soul_entity,
                "💪",
                soul_pos,
                &assets,
                BubbleEmotion::Motivated,
                BubblePriority::Low,
            );
            cooldowns.record_speech(soul_entity, BubblePriority::Low, current_time);
        }

        // Familiar: 命令フレーズ (Low)
        if let Some(uc) = under_command {
            if let Ok((fam_transform, voice)) = q_familiars.get(uc.0) {
                if cooldowns.can_speak(uc.0, BubblePriority::Low, current_time) {
                    let fam_pos = fam_transform.translation();
                    let phrase = match event.work_type {
                        WorkType::Chop => LatinPhrase::Caede,
                        WorkType::Mine => LatinPhrase::Fodere,
                        WorkType::Haul => LatinPhrase::Portare,
                        WorkType::Build => LatinPhrase::Laborare,
                        WorkType::GatherWater => LatinPhrase::Haurire,
                        WorkType::CollectSand => LatinPhrase::Colligere,
                        WorkType::Refine => LatinPhrase::Misce,
                        WorkType::HaulWaterToMixer => LatinPhrase::Haurire,
                    };
                    spawn_familiar_bubble(
                        &mut commands,
                        uc.0,
                        phrase,
                        fam_pos,
                        &assets,
                        &q_bubbles,
                        BubbleEmotion::Motivated,
                        BubblePriority::Low,
                        voice,
                    );
                    cooldowns.record_speech(uc.0, BubblePriority::Low, current_time);
                }
            }
        }
    }
}

/// タスク完了時のオブザーバー
pub fn on_task_completed(
    on: On<OnTaskCompleted>,
    mut commands: Commands,
    assets: Res<GameAssets>,
    q_souls: Query<&GlobalTransform>,
    time: Res<Time>,
    mut cooldowns: ResMut<crate::systems::visual::speech::cooldown::BubbleCooldowns>,
) {
    let soul_entity = on.entity;
    let current_time = time.elapsed_secs();
    if let Ok(transform) = q_souls.get(soul_entity) {
        if cooldowns.can_speak(soul_entity, BubblePriority::Low, current_time) {
            spawn_soul_bubble(
                &mut commands,
                soul_entity,
                "😊",
                transform.translation(),
                &assets,
                BubbleEmotion::Happy,
                BubblePriority::Low,
            );
            cooldowns.record_speech(soul_entity, BubblePriority::Low, current_time);
        }
    }
}

/// 勧誘時のオブザーバー（使い魔の発言）
pub fn on_soul_recruited(
    on: On<OnSoulRecruited>,
    mut commands: Commands,
    assets: Res<GameAssets>,
    q_familiars: Query<(&GlobalTransform, Option<&FamiliarVoice>), With<Familiar>>,
    q_bubbles: Query<(Entity, &SpeechBubble), With<FamiliarBubble>>,
    time: Res<Time>,
    mut cooldowns: ResMut<crate::systems::visual::speech::cooldown::BubbleCooldowns>,
) {
    let fam_entity = on.event().familiar_entity;
    let soul_entity = on.entity;
    let current_time = time.elapsed_secs();

    // 使い魔の発言（即時）
    if let Ok((transform, voice)) = q_familiars.get(fam_entity) {
        if cooldowns.can_speak(fam_entity, BubblePriority::Normal, current_time) {
            spawn_familiar_bubble(
                &mut commands,
                fam_entity,
                LatinPhrase::Veni,
                transform.translation(),
                &assets,
                &q_bubbles,
                BubbleEmotion::Neutral,
                BubblePriority::Normal,
                voice,
            );
            cooldowns.record_speech(fam_entity, BubblePriority::Normal, current_time);
        }
    }

    // [NEW] Soulのリアクションを遅延予約
    commands.entity(soul_entity).insert(ReactionDelay {
        timer: Timer::from_seconds(0.3, TimerMode::Once),
        emotion: BubbleEmotion::Fearful,
        text: "😨".to_string(),
    });
}

/// 疲労限界時のオブザーバー
pub fn on_exhausted(
    on: On<OnExhausted>,
    mut commands: Commands,
    assets: Res<GameAssets>,
    q_souls: Query<&GlobalTransform>,
    time: Res<Time>,
    mut cooldowns: ResMut<crate::systems::visual::speech::cooldown::BubbleCooldowns>,
) {
    let soul_entity = on.entity;
    let current_time = time.elapsed_secs();
    if let Ok(transform) = q_souls.get(soul_entity) {
        if cooldowns.can_speak(soul_entity, BubblePriority::High, current_time) {
            spawn_soul_bubble(
                &mut commands,
                soul_entity,
                "😴",
                transform.translation(),
                &assets,
                BubbleEmotion::Exhausted,
                BubblePriority::High,
            );
            cooldowns.record_speech(soul_entity, BubblePriority::High, current_time);
        }
    }
}

/// ストレス崩壊時のオブザーバー
pub fn on_stress_breakdown(
    on: On<OnStressBreakdown>,
    mut commands: Commands,
    assets: Res<GameAssets>,
    q_souls: Query<&GlobalTransform>,
    time: Res<Time>,
    mut cooldowns: ResMut<crate::systems::visual::speech::cooldown::BubbleCooldowns>,
) {
    let soul_entity = on.entity;
    let current_time = time.elapsed_secs();
    if let Ok(transform) = q_souls.get(soul_entity) {
        if cooldowns.can_speak(soul_entity, BubblePriority::Critical, current_time) {
            spawn_soul_bubble(
                &mut commands,
                soul_entity,
                "😰",
                transform.translation(),
                &assets,
                BubbleEmotion::Stressed,
                BubblePriority::Critical,
            );
            cooldowns.record_speech(soul_entity, BubblePriority::Critical, current_time);
        }
    }
}

/// リアクションの遅延実行を行うシステム
pub fn reaction_delay_system(
    time: Res<Time>,
    mut commands: Commands,
    assets: Res<GameAssets>,
    mut query: Query<(Entity, &GlobalTransform, &mut ReactionDelay)>,
) {
    for (entity, transform, mut delay) in query.iter_mut() {
        delay.timer.tick(time.delta());
        if delay.timer.just_finished() {
            spawn_soul_bubble(
                &mut commands,
                entity,
                &delay.text,
                transform.translation(),
                &assets,
                delay.emotion,
                BubblePriority::Normal,
            );
            commands.entity(entity).remove::<ReactionDelay>();
        }
    }
}

/// 使役解放時のリアクション
pub fn on_released_from_service(
    on: On<crate::events::OnReleasedFromService>,
    mut commands: Commands,
    assets: Res<GameAssets>,
) {
    spawn_soul_bubble(
        &mut commands,
        on.entity,
        "😅",
        Vec3::ZERO,
        &assets,
        BubbleEmotion::Relieved,
        BubblePriority::Normal,
    );
}

/// 集会参加時のリアクション
pub fn on_gathering_joined(
    on: On<crate::events::OnGatheringJoined>,
    mut commands: Commands,
    assets: Res<GameAssets>,
) {
    spawn_soul_bubble(
        &mut commands,
        on.entity,
        "😌",
        Vec3::ZERO,
        &assets,
        BubbleEmotion::Relaxed,
        BubblePriority::Normal,
    );
}

/// タスク中断・失敗時のリアクション
pub fn on_task_abandoned(
    on: On<crate::events::OnTaskAbandoned>,
    mut commands: Commands,
    assets: Res<GameAssets>,
) {
    spawn_soul_bubble(
        &mut commands,
        on.entity,
        "🙅‍♂️", // 拒否/放棄
        Vec3::ZERO,
        &assets,
        BubbleEmotion::Unmotivated,
        BubblePriority::Normal,
    );
}

/// 激励時のリアクション
pub fn on_encouraged(
    on: On<OnEncouraged>,
    mut commands: Commands,
    assets: Res<GameAssets>,
    q_familiars: Query<(&GlobalTransform, Option<&FamiliarVoice>), With<Familiar>>,
    q_bubbles: Query<(Entity, &SpeechBubble), With<FamiliarBubble>>,
    time: Res<Time>,
    mut cooldowns: ResMut<crate::systems::visual::speech::cooldown::BubbleCooldowns>,
) {
    let event = on.event();
    let fam_entity = event.familiar_entity;
    let soul_entity = event.soul_entity;
    let current_time = time.elapsed_secs();

    // 使い魔の激励（即時）
    if let Ok((transform, voice)) = q_familiars.get(fam_entity) {
        if cooldowns.can_speak(fam_entity, BubblePriority::Normal, current_time) {
            use rand::seq::SliceRandom;
            let mut rng = rand::thread_rng();
            let emoji = crate::constants::EMOJIS_ENCOURAGEMENT
                .choose(&mut rng)
                .unwrap_or(&"💪");

            spawn_familiar_bubble(
                &mut commands,
                fam_entity,
                crate::systems::visual::speech::phrases::LatinPhrase::Custom(emoji.to_string()),
                transform.translation(),
                &assets,
                &q_bubbles,
                BubbleEmotion::Motivated,
                BubblePriority::Normal,
                voice,
            );
            cooldowns.record_speech(fam_entity, BubblePriority::Normal, current_time);
        }
    }

    // Soulのリアクション（遅延）
    commands.entity(soul_entity).insert(ReactionDelay {
        timer: Timer::from_seconds(0.3, TimerMode::Once),
        emotion: BubbleEmotion::Stressed, // ストレスも溜まる
        text: "😓".to_string(),
    });
}
