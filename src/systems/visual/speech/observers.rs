use super::components::*;
use super::phrases::LatinPhrase;
use super::spawn::*;
use crate::assets::GameAssets;
use crate::entities::familiar::{Familiar, UnderCommand};
use crate::events::*;
use crate::systems::jobs::WorkType;
use bevy::prelude::*;

/// タスク開始時のオブザーバー
pub fn on_task_assigned(
    on: On<OnTaskAssigned>,
    mut commands: Commands,
    assets: Res<GameAssets>,
    q_souls: Query<(&GlobalTransform, Option<&UnderCommand>)>,
    q_familiars: Query<&GlobalTransform, With<Familiar>>,
    q_bubbles: Query<(Entity, &SpeechBubble), With<FamiliarBubble>>,
) {
    let soul_entity = on.entity;
    let event = on.event();

    if let Ok((soul_transform, under_command)) = q_souls.get(soul_entity) {
        let soul_pos = soul_transform.translation();

        // Soul: 「やる気」絵文字
        spawn_soul_bubble(
            &mut commands,
            soul_entity,
            "💪",
            soul_pos,
            &assets,
            BubbleEmotion::Motivated,
        );

        // Familiar: 命令フレーズ
        if let Some(uc) = under_command {
            if let Ok(fam_transform) = q_familiars.get(uc.0) {
                let fam_pos = fam_transform.translation();
                let phrase = match event.work_type {
                    WorkType::Chop => LatinPhrase::Caede,
                    WorkType::Mine => LatinPhrase::Fodere,
                    WorkType::Haul => LatinPhrase::Portare,
                    WorkType::Build => LatinPhrase::Laborare,
                };
                spawn_familiar_bubble(
                    &mut commands,
                    uc.0,
                    phrase,
                    fam_pos,
                    &assets,
                    &q_bubbles,
                    BubbleEmotion::Motivated,
                );
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
) {
    let soul_entity = on.entity;
    if let Ok(transform) = q_souls.get(soul_entity) {
        spawn_soul_bubble(
            &mut commands,
            soul_entity,
            "😊",
            transform.translation(),
            &assets,
            BubbleEmotion::Happy,
        );
    }
}

/// 勧誘時のオブザーバー
pub fn on_soul_recruited(
    on: On<OnSoulRecruited>,
    mut commands: Commands,
    assets: Res<GameAssets>,
    q_familiars: Query<&GlobalTransform, With<Familiar>>,
    q_bubbles: Query<(Entity, &SpeechBubble), With<FamiliarBubble>>,
) {
    let fam_entity = on.event().familiar_entity;
    if let Ok(transform) = q_familiars.get(fam_entity) {
        spawn_familiar_bubble(
            &mut commands,
            fam_entity,
            LatinPhrase::Veni,
            transform.translation(),
            &assets,
            &q_bubbles,
            BubbleEmotion::Neutral,
        );
    }
}

/// 疲労限界時のオブザーバー
pub fn on_exhausted(
    on: On<OnExhausted>,
    mut commands: Commands,
    assets: Res<GameAssets>,
    q_souls: Query<&GlobalTransform>,
) {
    let soul_entity = on.entity;
    if let Ok(transform) = q_souls.get(soul_entity) {
        spawn_soul_bubble(
            &mut commands,
            soul_entity,
            "😴",
            transform.translation(),
            &assets,
            BubbleEmotion::Exhausted,
        );
    }
}

/// ストレス崩壊時のオブザーバー
pub fn on_stress_breakdown(
    on: On<OnStressBreakdown>,
    mut commands: Commands,
    assets: Res<GameAssets>,
    q_souls: Query<&GlobalTransform>,
) {
    let soul_entity = on.entity;
    if let Ok(transform) = q_souls.get(soul_entity) {
        spawn_soul_bubble(
            &mut commands,
            soul_entity,
            "😰",
            transform.translation(),
            &assets,
            BubbleEmotion::Stressed,
        );
    }
}
