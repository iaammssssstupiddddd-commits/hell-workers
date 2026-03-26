use bevy::ecs::system::SystemParam;
use bevy::prelude::*;

use hw_core::events::GatheringSpawnRequest;
use hw_core::gathering::*;
use hw_core::relationships::{CommandedBy, ParticipatingIn};
use hw_core::soul::{DamnedSoul, IdleBehavior, IdleState};
use hw_jobs::AssignedTask;
use hw_spatial::{GatheringSpotSpatialGrid, SpatialGrid, SpatialGridOps};

type GatheringSpawnSoulQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Transform,
        &'static IdleState,
        &'static AssignedTask,
    ),
    (
        With<DamnedSoul>,
        Without<ParticipatingIn>,
        Without<CommandedBy>,
    ),
>;

#[derive(SystemParam)]
pub struct GatheringSpawnResources<'w> {
    pub time: Res<'w, Time>,
    pub spot_grid: Res<'w, GatheringSpotSpatialGrid>,
    pub soul_grid: Res<'w, SpatialGrid>,
    pub update_timer: Res<'w, GatheringUpdateTimer>,
}

/// 集会スポット発生判定システム (純粋ロジック・Execute Phase)
///
/// GatheringReadiness をティックし、発生条件が揃ったら GatheringSpawnRequest を送信する。
/// 視覚エンティティのスポーンは root 側のアダプターが担う。
pub fn gathering_spawn_logic_system(
    mut commands: Commands,
    q_souls: GatheringSpawnSoulQuery,
    mut nearby_buf: Local<Vec<Entity>>,
    mut q_readiness: Query<&mut GatheringReadiness>,
    mut spawn_requests: MessageWriter<GatheringSpawnRequest>,
    res: GatheringSpawnResources,
) {
    if !res.update_timer.timer.just_finished() {
        return;
    }

    let dt = res.update_timer.timer.duration().as_secs_f32();
    let current_time = res.time.elapsed_secs();

    for (entity, transform, idle, task) in q_souls.iter() {
        if !matches!(task, AssignedTask::None) {
            continue;
        }
        if !matches!(
            idle.behavior,
            IdleBehavior::Wandering | IdleBehavior::Sitting | IdleBehavior::Sleeping
        ) {
            continue;
        }

        let pos = transform.translation.truncate();

        res.spot_grid
            .get_nearby_in_radius_into(pos, GATHERING_DETECTION_RADIUS, &mut nearby_buf);
        if !nearby_buf.is_empty() {
            continue;
        }

        res.soul_grid
            .get_nearby_in_radius_into(pos, GATHERING_DETECTION_RADIUS, &mut nearby_buf);
        let nearby_souls = nearby_buf.len().saturating_sub(1);

        let spawn_time = (GATHERING_SPAWN_BASE_TIME
            - nearby_souls as f32 * GATHERING_SPAWN_TIME_REDUCTION_PER_SOUL)
            .max(2.0);

        if let Ok(mut readiness) = q_readiness.get_mut(entity) {
            readiness.idle_time += dt;
            if readiness.idle_time >= spawn_time {
                let object_type = GatheringObjectType::random_weighted(nearby_souls + 1);
                spawn_requests.write(GatheringSpawnRequest {
                    pos,
                    object_type,
                    initiator_entity: entity,
                    created_at: current_time,
                });
                readiness.idle_time = 0.0;
                debug!(
                    "GATHERING: Spawn request emitted at {:?} with {:?}, initiator {:?}",
                    pos, object_type, entity
                );
            }
        } else {
            commands
                .entity(entity)
                .insert(GatheringReadiness::default());
        }
    }
}
