use super::components::{BubbleEmotion, BubblePriority, FamiliarBubble, ReactionDelay, SpeechBubble};
use super::cooldown::SpeechHistory;
use super::phrases::LatinPhrase;
use super::spawn::*;
use crate::assets::GameAssets;
use crate::entities::damned_soul::DamnedSoul;
use crate::entities::familiar::{Familiar, FamiliarVoice};
use crate::relationships::CommandedBy;
use crate::events::*;
use crate::systems::jobs::WorkType;
use bevy::prelude::*;

/// タスク開始時のオブザーバー
pub fn on_task_assigned(
    on: On<OnTaskAssigned>,
    mut commands: Commands,
    assets: Res<GameAssets>,
    mut q_souls: Query<(&GlobalTransform, Option<&CommandedBy>, Option<&mut SpeechHistory>), (With<DamnedSoul>, Without<Familiar>)>,
    mut q_familiars: Query<
        (&GlobalTransform, Option<&FamiliarVoice>, Option<&mut SpeechHistory>),
        (With<Familiar>, Without<DamnedSoul>),
    >,
    q_bubbles: Query<(Entity, &SpeechBubble), With<FamiliarBubble>>,
    time: Res<Time>,
) {
    let soul_entity = on.entity;
    let event = on.event();
    let current_time = time.elapsed_secs();

    if let Ok((soul_transform, under_command, soul_history_opt)) = q_souls.get_mut(soul_entity) {
        let soul_pos = soul_transform.translation();

        let can_speak = if let Some(history) = &soul_history_opt {
            history.can_speak(BubblePriority::Low, current_time)
        } else {
            true
        };

        if can_speak {
            spawn_soul_bubble(
                &mut commands,
                soul_entity,
                "💪",
                soul_pos,
                &assets,
                BubbleEmotion::Motivated,
                BubblePriority::Low,
            );
            if let Some(mut history) = soul_history_opt {
                history.record_speech(BubblePriority::Low, current_time);
            } else {
                commands.entity(soul_entity).insert(SpeechHistory {
                    last_time: current_time,
                    last_priority: BubblePriority::Low,
                });
            }
        }

        if let Some(uc) = under_command {
            if let Ok((fam_transform, voice, fam_history_opt)) = q_familiars.get_mut(uc.0) {
                let fam_can_speak = if let Some(history) = &fam_history_opt {
                    history.can_speak(BubblePriority::Low, current_time)
                } else {
                    true
                };

                if fam_can_speak {
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
                    if let Some(mut history) = fam_history_opt {
                        history.record_speech(BubblePriority::Low, current_time);
                    } else {
                        commands.entity(uc.0).insert(SpeechHistory {
                            last_time: current_time,
                            last_priority: BubblePriority::Low,
                        });
                    }
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
    mut q_souls: Query<(&GlobalTransform, Option<&mut SpeechHistory>), (With<DamnedSoul>, Without<Familiar>)>,
    time: Res<Time>,
) {
    let soul_entity = on.entity;
    let current_time = time.elapsed_secs();
    if let Ok((transform, history_opt)) = q_souls.get_mut(soul_entity) {
        let can_speak = if let Some(history) = &history_opt {
            history.can_speak(BubblePriority::Low, current_time)
        } else {
            true
        };

        if can_speak {
            spawn_soul_bubble(
                &mut commands,
                soul_entity,
                "😊",
                transform.translation(),
                &assets,
                BubbleEmotion::Happy,
                BubblePriority::Low,
            );
            if let Some(mut history) = history_opt {
                history.record_speech(BubblePriority::Low, current_time);
            } else {
                commands.entity(soul_entity).insert(SpeechHistory {
                    last_time: current_time,
                    last_priority: BubblePriority::Low,
                });
            }
        }
    }
}

/// 勧誘時のオブザーバー（使い魔の発言）
pub fn on_soul_recruited(
    on: On<OnSoulRecruited>,
    mut commands: Commands,
    assets: Res<GameAssets>,
    mut q_familiars: Query<
        (&GlobalTransform, Option<&FamiliarVoice>, Option<&mut SpeechHistory>),
        With<Familiar>,
    >,
    q_bubbles: Query<(Entity, &SpeechBubble), With<FamiliarBubble>>,
    time: Res<Time>,
) {
    let fam_entity = on.event().familiar_entity;
    let soul_entity = on.entity;
    let current_time = time.elapsed_secs();

    if let Ok((transform, voice, history_opt)) = q_familiars.get_mut(fam_entity) {
        let can_speak = if let Some(history) = &history_opt {
            history.can_speak(BubblePriority::Normal, current_time)
        } else {
            true
        };

        if can_speak {
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
    mut q_souls: Query<(&GlobalTransform, Option<&mut SpeechHistory>), (With<DamnedSoul>, Without<Familiar>)>,
    time: Res<Time>,
) {
    let soul_entity = on.entity;
    let current_time = time.elapsed_secs();
    if let Ok((transform, history_opt)) = q_souls.get_mut(soul_entity) {
        let can_speak = if let Some(history) = &history_opt {
            history.can_speak(BubblePriority::High, current_time)
        } else {
            true
        };

        if can_speak {
            spawn_soul_bubble(
                &mut commands,
                soul_entity,
                "😴",
                transform.translation(),
                &assets,
                BubbleEmotion::Exhausted,
                BubblePriority::High,
            );
            if let Some(mut history) = history_opt {
                history.record_speech(BubblePriority::High, current_time);
            } else {
                commands.entity(soul_entity).insert(SpeechHistory {
                    last_time: current_time,
                    last_priority: BubblePriority::High,
                });
            }
        }
    }
}

/// ストレス崩壊時のオブザーバー
pub fn on_stress_breakdown(
    on: On<OnStressBreakdown>,
    mut commands: Commands,
    assets: Res<GameAssets>,
    mut q_souls: Query<(&GlobalTransform, Option<&mut SpeechHistory>), (With<DamnedSoul>, Without<Familiar>)>,
    time: Res<Time>,
) {
    let soul_entity = on.entity;
    let current_time = time.elapsed_secs();
    if let Ok((transform, history_opt)) = q_souls.get_mut(soul_entity) {
        let can_speak = if let Some(history) = &history_opt {
            history.can_speak(BubblePriority::Critical, current_time)
        } else {
            true
        };

        if can_speak {
            spawn_soul_bubble(
                &mut commands,
                soul_entity,
                "😰",
                transform.translation(),
                &assets,
                BubbleEmotion::Stressed,
                BubblePriority::Critical,
            );
            if let Some(mut history) = history_opt {
                history.record_speech(BubblePriority::Critical, current_time);
            } else {
                commands.entity(soul_entity).insert(SpeechHistory {
                    last_time: current_time,
                    last_priority: BubblePriority::Critical,
                });
            }
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
        "🙅‍♂️",
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
    mut q_familiars: Query<
        (&GlobalTransform, Option<&FamiliarVoice>, Option<&mut SpeechHistory>),
        With<Familiar>,
    >,
    q_bubbles: Query<(Entity, &SpeechBubble), With<FamiliarBubble>>,
    time: Res<Time>,
) {
    let event = on.event();
    let fam_entity = event.familiar_entity;
    let soul_entity = event.soul_entity;
    let current_time = time.elapsed_secs();

    if let Ok((transform, voice, history_opt)) = q_familiars.get_mut(fam_entity) {
        let can_speak = if let Some(history) = &history_opt {
            history.can_speak(BubblePriority::Normal, current_time)
        } else {
            true
        };

        if can_speak {
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

    commands.entity(soul_entity).insert(ReactionDelay {
        timer: Timer::from_seconds(0.3, TimerMode::Once),
        emotion: BubbleEmotion::Stressed,
        text: "😓".to_string(),
    });
}
