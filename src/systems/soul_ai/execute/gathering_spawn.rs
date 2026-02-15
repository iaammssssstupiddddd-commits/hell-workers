use bevy::prelude::*;

use crate::assets::GameAssets;
use crate::constants::*;
use crate::entities::damned_soul::{DamnedSoul, IdleBehavior, IdleState};
use crate::systems::soul_ai::execute::task_execution::AssignedTask;
use crate::relationships::ParticipatingIn;
use crate::systems::soul_ai::helpers::gathering::*;
use crate::systems::spatial::{GatheringSpotSpatialGrid, SpatialGrid, SpatialGridOps};

/// 集会スポットの発生システム
/// アイドル状態のSoulが一定時間経過すると新しい集会を発生させる
pub fn gathering_spawn_system(
    time: Res<Time>,
    mut commands: Commands,
    game_assets: Res<GameAssets>,
    q_souls: Query<
        (Entity, &Transform, &IdleState, &AssignedTask),
        (
            With<DamnedSoul>,
            Without<ParticipatingIn>,
            Without<crate::relationships::CommandedBy>,
        ),
    >,
    spot_grid: Res<GatheringSpotSpatialGrid>,
    soul_grid: Res<SpatialGrid>,
    mut q_readiness: Query<&mut GatheringReadiness>,
    update_timer: Res<GatheringUpdateTimer>,
) {
    if !update_timer.timer.just_finished() {
        return;
    }

    let dt = update_timer.timer.duration().as_secs_f32();
    let current_time = time.elapsed_secs();

    for (entity, transform, idle, task) in q_souls.iter() {
        // タスクなし & Idle/Wandering 状態のみ対象
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

        // 既存の集会所が近くにあるか空間グリッドでチェック
        let nearby_spots = spot_grid.get_nearby_in_radius(pos, GATHERING_DETECTION_RADIUS);
        if !nearby_spots.is_empty() {
            continue;
        }

        // 近傍のSoul数を空間グリッドでカウント
        let nearby_soul_entities = soul_grid.get_nearby_in_radius(pos, GATHERING_DETECTION_RADIUS);
        let nearby_souls = nearby_soul_entities.len().saturating_sub(1); // 自分を除く

        // 発生時間を計算
        let spawn_time = (GATHERING_SPAWN_BASE_TIME
            - nearby_souls as f32 * GATHERING_SPAWN_TIME_REDUCTION_PER_SOUL)
            .max(2.0);

        // GatheringReadiness を更新または追加
        if let Ok(mut readiness) = q_readiness.get_mut(entity) {
            readiness.idle_time += dt;
            if readiness.idle_time >= spawn_time {
                // 集会発生!
                let object_type = GatheringObjectType::random_weighted(nearby_souls + 1);
                let spot_entity = spawn_gathering_spot(
                    &mut commands,
                    &game_assets,
                    pos,
                    object_type,
                    current_time,
                );
                // 発起人を参加者として登録
                commands.entity(entity).insert(ParticipatingIn(spot_entity));
                commands.trigger(crate::events::OnGatheringParticipated {
                    entity,
                    spot_entity,
                });
                readiness.idle_time = 0.0;
                debug!(
                    "GATHERING: New spot spawned at {:?} with {:?}, initiator {:?}",
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

/// 集会スポットをスポーン
pub(crate) fn spawn_gathering_spot(
    commands: &mut Commands,
    game_assets: &Res<GameAssets>,
    center: Vec2,
    object_type: GatheringObjectType,
    created_at: f32,
) -> Entity {
    let spot = GatheringSpot {
        center,
        max_capacity: GATHERING_MAX_CAPACITY,
        grace_timer: GATHERING_GRACE_PERIOD,
        grace_active: true,
        object_type,
        created_at,
    };

    let aura_size = calculate_aura_size(0);

    // オーラエンティティ
    let aura_entity = commands
        .spawn((
            Sprite {
                image: game_assets.aura_circle.clone(),
                custom_size: Some(Vec2::splat(aura_size)),
                color: Color::srgba(0.5, 0.2, 0.8, 0.3),
                ..default()
            },
            Transform::from_xyz(center.x, center.y, Z_AURA),
        ))
        .id();

    // 中心オブジェクトエンティティ (もしあれば)
    let object_image = match object_type {
        GatheringObjectType::Nothing => None,
        GatheringObjectType::CardTable => Some(game_assets.gathering_card_table.clone()),
        GatheringObjectType::Campfire => Some(game_assets.gathering_campfire.clone()),
        GatheringObjectType::Barrel => Some(game_assets.gathering_barrel.clone()),
    };
    let object_entity = object_image.map(|image| {
        commands
            .spawn((
                Sprite {
                    image,
                    custom_size: Some(Vec2::splat(TILE_SIZE * 1.5)),
                    ..default()
                },
                Transform::from_xyz(center.x, center.y, Z_ITEM),
            ))
            .id()
    });

    let visuals = GatheringVisuals {
        aura_entity,
        object_entity,
    };

    commands.spawn((spot, visuals)).id()
}
