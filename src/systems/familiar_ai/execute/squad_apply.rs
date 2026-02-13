use crate::entities::familiar::{Familiar, FamiliarVoice};
use crate::events::{ReleaseReason, SquadManagementOperation, SquadManagementRequest};
use crate::relationships::CommandedBy;
use crate::systems::familiar_ai::FamiliarSoulQuery;
use crate::systems::soul_ai::helpers::gathering::ParticipatingIn;
use crate::systems::visual::speech::components::{
    BubbleEmotion, BubblePriority, FamiliarBubble, SpeechBubble,
};
use crate::world::map::WorldMap;
use bevy::prelude::*;

/// 分隊管理要求を適用するシステム（Execute Phase）
pub fn apply_squad_management_requests_system(
    mut commands: Commands,
    mut request_reader: MessageReader<SquadManagementRequest>,
    mut q_souls: FamiliarSoulQuery,
    mut queries: crate::systems::soul_ai::execute::task_execution::context::TaskAssignmentQueries,
    world_map: Res<WorldMap>,
    time: Res<Time>,
    game_assets: Res<crate::assets::GameAssets>,
    q_bubbles: Query<(Entity, &SpeechBubble), With<FamiliarBubble>>,
    mut q_familiars: Query<
        (
            &Transform,
            Option<&FamiliarVoice>,
            Option<&mut crate::systems::visual::speech::cooldown::SpeechHistory>,
        ),
        With<Familiar>,
    >,
) {
    for request in request_reader.read() {
        let fam_entity = request.familiar_entity;
        match &request.operation {
            SquadManagementOperation::AddMember { soul_entity } => {
                let soul_entity = *soul_entity;
                commands.entity(soul_entity).insert(CommandedBy(fam_entity));

                if let Ok((_, _, _, _, _, _, _, _, _, Some(p))) = q_souls.get(soul_entity) {
                    commands.entity(soul_entity).remove::<ParticipatingIn>();
                    commands.trigger(crate::events::OnGatheringLeft {
                        entity: soul_entity,
                        spot_entity: p.0,
                    });
                }

                commands.trigger(crate::events::OnSoulRecruited {
                    entity: soul_entity,
                    familiar_entity: fam_entity,
                });
            }
            SquadManagementOperation::ReleaseMember {
                soul_entity,
                reason,
            } => {
                let soul_entity = *soul_entity;
                if let Ok((
                    entity,
                    transform,
                    _soul,
                    mut task,
                    _,
                    mut path,
                    _idle,
                    mut inventory_opt,
                    _,
                    _,
                )) = q_souls.get_mut(soul_entity)
                {
                    let dropped_res = inventory_opt.as_ref().and_then(|i| {
                        i.0.and_then(|e| {
                            queries
                                .designation
                                .targets
                                .get(e)
                                .ok()
                                .and_then(|(_, _, _, _, ri, _, _)| ri.map(|r| r.0))
                        })
                    });

                    let emit_abandoned = false;

                    crate::systems::soul_ai::helpers::work::unassign_task(
                        &mut commands,
                        entity,
                        transform.translation.truncate(),
                        &mut task,
                        &mut path,
                        inventory_opt.as_deref_mut(),
                        dropped_res,
                        &mut queries,
                        &world_map,
                        emit_abandoned,
                    );

                    if matches!(reason, ReleaseReason::Fatigued) {
                        if let Ok((fam_transform, voice_opt, mut history_opt)) =
                            q_familiars.get_mut(fam_entity)
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
                                    fam_entity,
                                    crate::systems::visual::speech::phrases::LatinPhrase::Abi,
                                    fam_transform.translation,
                                    &game_assets,
                                    &q_bubbles,
                                    BubbleEmotion::Neutral,
                                    BubblePriority::Normal,
                                    voice_opt,
                                );
                                if let Some(history) = history_opt.as_mut() {
                                    history.record_speech(BubblePriority::Normal, current_time);
                                } else {
                                    commands.entity(fam_entity).insert(
                                        crate::systems::visual::speech::cooldown::SpeechHistory {
                                            last_time: current_time,
                                            last_priority: BubblePriority::Normal,
                                        },
                                    );
                                }
                            }
                        }
                    }

                    commands.entity(soul_entity).remove::<CommandedBy>();
                    commands.trigger(crate::events::OnReleasedFromService {
                        entity: soul_entity,
                    });
                }
            }
        }
    }
}
