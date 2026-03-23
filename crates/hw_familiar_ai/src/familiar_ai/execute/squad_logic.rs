//! 分隊管理要求のロジック処理：AddMember/ReleaseMember の ECS 操作。

use std::collections::HashSet;

use bevy::prelude::*;
use hw_core::events::{
    OnReleasedFromService, OnSoulRecruited, SoulTaskUnassignRequest, SquadManagementOperation,
    SquadManagementRequest,
};
use hw_core::familiar::Familiar;
use hw_core::relationships::{CommandedBy, ParticipatingIn};
use hw_core::soul::DamnedSoul;

/// 分隊管理要求を適用するロジックシステム（Execute Phase）
///
/// ビジュアル演出（Fatigued リリース時のセリフ）は `hw_visual::squad_visual_system` が担当。
pub fn squad_logic_system(
    mut commands: Commands,
    mut request_reader: MessageReader<SquadManagementRequest>,
    q_participating: Query<(), (With<ParticipatingIn>, With<DamnedSoul>, Without<Familiar>)>,
    mut task_unassign_writer: MessageWriter<SoulTaskUnassignRequest>,
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

                if q_participating.contains(soul_entity) {
                    commands.entity(soul_entity).remove::<ParticipatingIn>();
                }

                commands.trigger(OnSoulRecruited {
                    entity: soul_entity,
                    familiar_entity: fam_entity,
                });
            }
            SquadManagementOperation::ReleaseMember {
                soul_entity,
                reason,
            } => {
                let soul_entity = *soul_entity;

                task_unassign_writer.write(SoulTaskUnassignRequest {
                    soul_entity,
                    emit_abandoned: false,
                });

                // Fatigued以外のリリース理由は unused だが将来拡張のため受け取る
                let _ = reason;

                commands.entity(soul_entity).remove::<CommandedBy>();
                commands.trigger(OnReleasedFromService {
                    entity: soul_entity,
                });
            }
        }
    }
}
