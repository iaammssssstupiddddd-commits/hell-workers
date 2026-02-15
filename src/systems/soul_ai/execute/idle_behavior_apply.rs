use bevy::prelude::*;

use crate::events::{IdleBehaviorOperation, IdleBehaviorRequest};
use crate::relationships::ParticipatingIn;

/// アイドル行動の適用システム (Execute Phase)
///
/// IdleBehaviorRequestを読み取り、実際のエンティティ操作を行う。
/// - ParticipatingInの追加/削除
/// - イベントのトリガー
pub fn idle_behavior_apply_system(
    mut commands: Commands,
    mut request_reader: MessageReader<IdleBehaviorRequest>,
) {
    for request in request_reader.read() {
        match &request.operation {
            IdleBehaviorOperation::JoinGathering { spot_entity } => {
                commands
                    .entity(request.entity)
                    .insert(ParticipatingIn(*spot_entity));
                commands.trigger(crate::events::OnGatheringParticipated {
                    entity: request.entity,
                    spot_entity: *spot_entity,
                });
            }
            IdleBehaviorOperation::LeaveGathering { spot_entity: _ } => {
                commands.entity(request.entity).remove::<ParticipatingIn>();
                commands.trigger(crate::events::OnGatheringLeft {
                    entity: request.entity,
                });
            }
            IdleBehaviorOperation::ArriveAtGathering { spot_entity } => {
                commands
                    .entity(request.entity)
                    .insert(ParticipatingIn(*spot_entity));
                commands.trigger(crate::events::OnGatheringParticipated {
                    entity: request.entity,
                    spot_entity: *spot_entity,
                });
                commands.trigger(crate::events::OnGatheringJoined {
                    entity: request.entity,
                });
            }
        }
    }
}
