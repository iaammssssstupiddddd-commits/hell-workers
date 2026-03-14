//! 分隊管理要求のロジック処理：AddMember/ReleaseMember の ECS 操作。

use std::collections::HashSet;

use bevy::prelude::*;
use hw_core::events::{
    OnGatheringLeft, OnReleasedFromService, OnSoulRecruited,
    SquadManagementOperation, SquadManagementRequest,
};
use hw_core::relationships::{CommandedBy, ParticipatingIn};
use hw_soul_ai::soul_ai::execute::task_execution::TaskAssignmentQueries;
use hw_soul_ai::soul_ai::helpers::work::unassign_task;
use hw_world::WorldMapRead;

use crate::familiar_ai::decide::query_types::FamiliarSoulQuery;

/// 分隊管理要求を適用するロジックシステム（Execute Phase）
///
/// ビジュアル演出（Fatigued リリース時のセリフ）は `hw_visual::squad_visual_system` が担当。
pub fn squad_logic_system(
    mut commands: Commands,
    mut request_reader: MessageReader<SquadManagementRequest>,
    mut q_souls: FamiliarSoulQuery,
    mut queries: TaskAssignmentQueries,
    world_map: WorldMapRead,
) {
    let mut recruited_this_frame: HashSet<Entity> = HashSet::new();

    for request in request_reader.read() {
        let fam_entity = request.familiar_entity;
        match &request.operation {
            SquadManagementOperation::AddMember { soul_entity } => {
                let soul_entity = *soul_entity;
                if !recruited_this_frame.insert(soul_entity) {
                    continue;
                }
                commands.entity(soul_entity).insert(CommandedBy(fam_entity));

                if let Ok((_, _, _, _, _, _, _, _, _, Some(_p))) = q_souls.get(soul_entity) {
                    commands.entity(soul_entity).remove::<ParticipatingIn>();
                    commands.trigger(OnGatheringLeft { entity: soul_entity });
                }

                commands.trigger(OnSoulRecruited {
                    entity: soul_entity,
                    familiar_entity: fam_entity,
                });
            }
            SquadManagementOperation::ReleaseMember { soul_entity, reason } => {
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

                    unassign_task(
                        &mut commands,
                        entity,
                        transform.translation.truncate(),
                        &mut task,
                        &mut path,
                        inventory_opt.as_deref_mut(),
                        dropped_res,
                        &mut queries,
                        world_map.as_ref(),
                        emit_abandoned,
                    );

                    // Fatigued以外のリリース理由は unused だが将来拡張のため受け取る
                    let _ = reason;
                }

                commands.entity(soul_entity).remove::<CommandedBy>();
                commands.trigger(OnReleasedFromService { entity: soul_entity });
            }
        }
    }
}
