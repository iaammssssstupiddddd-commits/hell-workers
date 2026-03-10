use bevy::prelude::*;

use crate::assets::GameAssets;
use crate::entities::damned_soul::{DamnedSoul, IdleBehavior, IdleState};
use crate::events::{GatheringSpawnRequest, OnGatheringParticipated};
use crate::relationships::{CommandedBy, ParticipatingIn};
use crate::systems::soul_ai::execute::task_execution::AssignedTask;
use crate::systems::soul_ai::helpers::gathering::*;
use hw_core::constants::*;

/// 集会スポットの視覚アダプターシステム (Execute Phase)
///
/// `GatheringSpawnRequest` を受け取り、GameAssets を使って
/// aura・中心オブジェクトのスプライトエンティティをスポーンする。
/// 発生判定ロジックは hw_ai の `gathering_spawn_logic_system` が担う。
pub fn gathering_spawn_system(
    mut commands: Commands,
    game_assets: Res<GameAssets>,
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
            &game_assets,
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

/// 集会スポットをスポーン（GatheringSpot + visual entities）
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
