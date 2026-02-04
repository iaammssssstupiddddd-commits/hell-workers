//! 使役数上限変更イベントの処理
//!
//! UIで使役数が減少した場合、超過分の魂をリリースします。

use crate::entities::damned_soul::{DamnedSoul, Path};
use crate::entities::familiar::{Familiar, FamiliarVoice, UnderCommand};
use crate::events::FamiliarOperationMaxSoulChangedEvent;
use crate::relationships::Commanding;
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
    mut q_familiars: Query<(&Transform, &FamiliarVoice, Option<&mut crate::systems::visual::speech::cooldown::SpeechHistory>), With<Familiar>>,
    q_commanding: Query<&Commanding, With<Familiar>>,
    mut q_souls: Query<
        (
            Entity,
            &Transform,
            &mut AssignedTask,
            &mut Path,
            Option<&mut crate::systems::logistics::Inventory>,
            Option<&mut crate::systems::visual::speech::cooldown::SpeechHistory>,
        ),
        (With<DamnedSoul>, Without<Familiar>),
    >,
    mut queries: crate::systems::soul_ai::task_execution::context::TaskAssignmentQueries,
    game_assets: Res<crate::assets::GameAssets>,
    q_bubbles: Query<(Entity, &SpeechBubble), With<FamiliarBubble>>,
    time: Res<Time>,
    world_map: Res<crate::world::map::WorldMap>,
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
                        if let Ok((entity, transform, mut task, mut path, mut inventory_opt, _history)) =
                            q_souls.get_mut(member_entity)
                        {
                            // タスクを解除
                            unassign_task(
                                &mut commands,
                                entity,
                                transform.translation.truncate(),
                                &mut task,
                                &mut path,
                                inventory_opt.as_deref_mut(),
                                None,
                                &mut queries,
                                &world_map,
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
                    if let Ok((fam_transform, voice_opt, history_opt)) =
                        q_familiars.get_mut(event.familiar_entity)
                    {
                        let current_time = time.elapsed_secs();
                        let can_speak = if let Some(history) = &history_opt {
                            history.can_speak(BubblePriority::Normal, current_time)
                        } else {
                            true
                        };

                        if can_speak {
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
                            if let Some(mut history) = history_opt {
                                history.record_speech(BubblePriority::Normal, current_time);
                            } else {
                                commands.entity(event.familiar_entity).insert(crate::systems::visual::speech::cooldown::SpeechHistory {
                                    last_time: current_time,
                                    last_priority: BubblePriority::Normal,
                                });
                            }
                        }
                    }
                }
            }
        }
    }
}
