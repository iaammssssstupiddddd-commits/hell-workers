use bevy::prelude::*;

use crate::entities::damned_soul::{DamnedSoul, IdleBehavior, IdleState};
use crate::events::{GatheringSpawnRequest, OnGatheringParticipated};
use hw_core::relationships::{CommandedBy, ParticipatingIn};
use crate::systems::soul_ai::execute::task_execution::AssignedTask;
use hw_visual::{soul::gathering_spawn::spawn_gathering_spot, GatheringVisualHandles};

/// 集会スポットの視覚アダプターシステム (Execute Phase)
///
/// `GatheringSpawnRequest` を受け取り、stale request を再検証したうえで
/// `hw_visual` の helper に集会スポット visual/entity 生成を委譲する。
/// 発生判定ロジックは hw_ai の `gathering_spawn_logic_system` が担う。
pub fn gathering_spawn_system(
    mut commands: Commands,
    gathering_visual_handles: Res<GatheringVisualHandles>,
    q_initiators: Query<
        (&IdleState, &AssignedTask),
        (
            With<DamnedSoul>,
            Without<ParticipatingIn>,
            Without<CommandedBy>,
        ),
    >,
    mut spawn_requests: MessageReader<GatheringSpawnRequest>,
) {
    for request in spawn_requests.read() {
        let Ok((idle, task)) = q_initiators.get(request.initiator_entity) else {
            debug!(
                "GATHERING: Drop stale spawn request for missing or unavailable initiator {:?}",
                request.initiator_entity
            );
            continue;
        };

        if !matches!(task, AssignedTask::None) {
            debug!(
                "GATHERING: Drop stale spawn request for busy initiator {:?}",
                request.initiator_entity
            );
            continue;
        }

        if !matches!(
            idle.behavior,
            IdleBehavior::Wandering | IdleBehavior::Sitting | IdleBehavior::Sleeping
        ) {
            debug!(
                "GATHERING: Drop stale spawn request for initiator {:?} in {:?}",
                request.initiator_entity, idle.behavior
            );
            continue;
        }

        let spot_entity = spawn_gathering_spot(
            &mut commands,
            gathering_visual_handles.as_ref(),
            request.pos,
            request.object_type,
            request.created_at,
        );
        commands
            .entity(request.initiator_entity)
            .insert(ParticipatingIn(spot_entity));
        commands.trigger(OnGatheringParticipated {
            entity: request.initiator_entity,
            spot_entity,
        });
    }
}
