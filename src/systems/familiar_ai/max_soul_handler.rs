//! 使役数上限変更イベントの処理
//!
//! UIで使役数が減少した場合、超過分の魂をリリースします。

use crate::entities::damned_soul::Path;
use crate::entities::familiar::{Familiar, FamiliarVoice, UnderCommand};
use crate::events::FamiliarOperationMaxSoulChangedEvent;
use crate::relationships::{Commanding, Holding, TaskWorkers};
use crate::systems::jobs::{Designation, DesignationCreatedEvent, IssuedBy, TaskSlots};
use crate::systems::soul_ai::task_execution::AssignedTask;
use crate::systems::soul_ai::work::unassign_task;
use crate::systems::visual::speech::components::{
    BubbleEmotion, BubblePriority, FamiliarBubble, SpeechBubble,
};
use bevy::prelude::*;

/// 使役数上限変更イベントを処理するシステム
/// UIで使役数が減少した場合、超過分の魂をリリースする
pub fn handle_max_soul_changed_system(
    mut ev_max_soul_changed: MessageReader<FamiliarOperationMaxSoulChangedEvent>,
    q_familiars: Query<(&Transform, &FamiliarVoice, Option<&Familiar>), With<Familiar>>,
    q_commanding: Query<&Commanding, With<Familiar>>,
    mut q_souls: Query<
        (
            Entity,
            &Transform,
            &mut AssignedTask,
            &mut Path,
            Option<&Holding>,
        ),
    >,
    q_designations: Query<(
        Entity,
        &Transform,
        &Designation,
        Option<&IssuedBy>,
        Option<&TaskSlots>,
        Option<&TaskWorkers>,
    )>,
    mut haul_cache: ResMut<crate::systems::familiar_ai::haul_cache::HaulReservationCache>,
    mut ev_created: MessageWriter<DesignationCreatedEvent>,
    game_assets: Res<crate::assets::GameAssets>,
    q_bubbles: Query<(Entity, &SpeechBubble), With<FamiliarBubble>>,
    mut cooldowns: ResMut<crate::systems::visual::speech::cooldown::BubbleCooldowns>,
    time: Res<Time>,
    mut commands: Commands,
) {
    for event in ev_max_soul_changed.read() {
        // 使役数が減少した場合のみ処理
        if event.new_value < event.old_value {
            if let Ok(commanding) = q_commanding.get(event.familiar_entity) {
                let squad_entities: Vec<Entity> = commanding.iter().copied().collect();

                if squad_entities.len() > event.new_value {
                    let excess_count = squad_entities.len() - event.new_value;
                    info!(
                        "FAM_AI: {:?} max_soul decreased from {} to {}, releasing {} excess members",
                        event.familiar_entity, event.old_value, event.new_value, excess_count
                    );

                    // 超過分をリリース（後ろから順にリリース）
                    let mut released_count = 0;
                    for i in (0..squad_entities.len()).rev() {
                        if released_count >= excess_count {
                            break;
                        }
                        let member_entity = squad_entities[i];
                        if let Ok((entity, transform, mut task, mut path, holding_opt)) =
                            q_souls.get_mut(member_entity)
                        {
                            // タスクを解除
                            unassign_task(
                                &mut commands,
                                entity,
                                transform.translation.truncate(),
                                &mut task,
                                &mut path,
                                holding_opt,
                                &q_designations,
                                &mut *haul_cache,
                                Some(&mut ev_created),
                                false, // emit_abandoned_event: 上限超過リリース時は個別のタスク中断セリフを出さない
                            );
                        }

                        commands.entity(member_entity).remove::<UnderCommand>();
                        released_count += 1;

                        info!(
                            "FAM_AI: {:?} released excess member {:?} (limit: {} -> {})",
                            event.familiar_entity, member_entity, event.old_value, event.new_value
                        );
                    }

                    // リリースフレーズを表示（一度だけ）
                    if let Ok((fam_transform, voice_opt, _)) =
                        q_familiars.get(event.familiar_entity)
                    {
                        if cooldowns.can_speak(
                            event.familiar_entity,
                            BubblePriority::Normal,
                            time.elapsed_secs(),
                        ) {
                            crate::systems::visual::speech::spawn::spawn_familiar_bubble(
                                &mut commands,
                                event.familiar_entity,
                                crate::systems::visual::speech::phrases::LatinPhrase::Abi,
                                fam_transform.translation,
                                &game_assets,
                                &q_bubbles,
                                BubbleEmotion::Neutral,
                                BubblePriority::Normal,
                                Some(voice_opt),
                            );
                            cooldowns.record_speech(
                                event.familiar_entity,
                                BubblePriority::Normal,
                                time.elapsed_secs(),
                            );
                        }
                    }
                }
            }
        }
    }
}
