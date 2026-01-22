use bevy::prelude::*;

use crate::entities::damned_soul::{DamnedSoul, IdleBehavior, IdleState};
use crate::systems::soul_ai::gathering::*;
use crate::systems::soul_ai::task_execution::AssignedTask;
use crate::systems::spatial::{SpatialGrid, SpatialGridOps};

/// 集会スポットの維持・消滅システム
/// 参加者数はObserverで自動更新されるため、ここでは猶予タイマーのみ管理
pub fn gathering_maintenance_system(
    mut commands: Commands,
    mut q_spots: Query<(Entity, &mut GatheringSpot, &GatheringVisuals)>,
    update_timer: Res<GatheringUpdateTimer>,
) {
    if !update_timer.timer.just_finished() {
        return;
    }
    let dt = update_timer.timer.duration().as_secs_f32();

    for (spot_entity, mut spot, visuals) in q_spots.iter_mut() {
        // 人数が最低未満の場合
        if spot.participants < GATHERING_MIN_PARTICIPANTS {
            if !spot.grace_active {
                spot.grace_active = true;
                spot.grace_timer = GATHERING_GRACE_PERIOD;
            }
            spot.grace_timer -= dt;

            if spot.grace_timer <= 0.0 {
                // 集会消滅
                info!(
                    "GATHERING: Spot at {:?} dissolved (insufficient participants)",
                    spot.center
                );
                commands.entity(visuals.aura_entity).despawn();
                if let Some(obj) = visuals.object_entity {
                    commands.entity(obj).despawn();
                }
                commands.entity(spot_entity).despawn();
            }
        } else {
            spot.grace_active = false;
            spot.grace_timer = GATHERING_GRACE_PERIOD;
        }
    }
}

/// 集会スポットの統合システム
pub fn gathering_merge_system(
    time: Res<Time>,
    mut commands: Commands,
    q_spots: Query<(Entity, &GatheringSpot, &GatheringVisuals)>,
    q_participants: Query<(Entity, &ParticipatingIn)>,
    update_timer: Res<GatheringUpdateTimer>,
) {
    if !update_timer.timer.just_finished() {
        return;
    }
    let current_time = time.elapsed_secs();
    let spots: Vec<_> = q_spots.iter().collect();

    for i in 0..spots.len() {
        for j in (i + 1)..spots.len() {
            let (entity_a, spot_a, visuals_a) = &spots[i];
            let (entity_b, spot_b, visuals_b) = &spots[j];

            // 統合後の合計人数が定員を超える場合はスキップ
            let combined_participants = spot_a.participants + spot_b.participants;
            if combined_participants > GATHERING_MAX_CAPACITY {
                continue;
            }

            let distance = (spot_a.center - spot_b.center).length();
            let elapsed_a = current_time - spot_a.created_at;
            let elapsed_b = current_time - spot_b.created_at;
            let merge_distance_a = calculate_merge_distance(spot_a.participants, elapsed_a);
            let merge_distance_b = calculate_merge_distance(spot_b.participants, elapsed_b);

            // どちらかの統合距離内にあるか
            if distance < merge_distance_a.max(merge_distance_b) {
                // 小さい方を大きい方に吸収
                let (absorber, absorbed, absorbed_visuals) =
                    if spot_a.participants > spot_b.participants {
                        (*entity_a, *entity_b, visuals_b)
                    } else if spot_b.participants > spot_a.participants {
                        (*entity_b, *entity_a, visuals_a)
                    } else {
                        // 同数の場合は古い方が残る
                        if spot_a.created_at < spot_b.created_at {
                            (*entity_a, *entity_b, visuals_b)
                        } else {
                            (*entity_b, *entity_a, visuals_a)
                        }
                    };

                info!("GATHERING: Merging spot {:?} into {:?}", absorbed, absorber);

                // 参加者のターゲットを変更 (Observerを発火させるためにイベントをトリガー)
                for (soul_entity, participating) in q_participants.iter() {
                    if participating.0 == absorbed {
                        // 古いスポットから離脱
                        commands.trigger(crate::events::OnGatheringLeft {
                            entity: soul_entity,
                            spot_entity: absorbed,
                        });
                        // 新しいスポットに参加
                        commands
                            .entity(soul_entity)
                            .insert(ParticipatingIn(absorber));
                        commands.trigger(crate::events::OnGatheringParticipated {
                            entity: soul_entity,
                            spot_entity: absorber,
                        });
                    }
                }

                // 吸収された側のビジュアルを削除
                commands.entity(absorbed_visuals.aura_entity).despawn();
                if let Some(obj) = absorbed_visuals.object_entity {
                    commands.entity(obj).despawn();
                }
                commands.entity(absorbed).despawn();

                // 今の反復を終了（状態が変わったため）
                return;
            }
        }
    }
}

/// 集会エリア内の未参加Soulを自動的に参加させるシステム
pub fn gathering_recruitment_system(
    mut commands: Commands,
    q_spots: Query<(Entity, &GatheringSpot)>,
    soul_grid: Res<SpatialGrid>,
    q_souls: Query<
        (Entity, &Transform, &AssignedTask),
        (
            With<DamnedSoul>,
            Without<ParticipatingIn>,
            Without<crate::entities::familiar::UnderCommand>,
        ),
    >,
    update_timer: Res<GatheringUpdateTimer>,
) {
    if !update_timer.timer.just_finished() {
        return;
    }
    for (spot_entity, spot) in q_spots.iter() {
        // 定員オーバーならスキップ
        if spot.participants >= spot.max_capacity {
            continue;
        }

        // 空間グリッドで近傍のSoulを検索
        let nearby_souls = soul_grid.get_nearby_in_radius(spot.center, GATHERING_DETECTION_RADIUS);

        // 空き容量の分だけ参加させる
        let mut current_participants = spot.participants;
        for soul_entity in nearby_souls {
            if current_participants >= spot.max_capacity {
                break;
            }

            if let Ok((_ent, _transform, task)) = q_souls.get(soul_entity) {
                // タスク実行中は除外
                if !matches!(task, AssignedTask::None) {
                    continue;
                }

                // q_souls のフィルタ (Without<ParticipatingIn> 等) により、条件に合うSoulのみが対象
                current_participants += 1;
                commands
                    .entity(soul_entity)
                    .insert(ParticipatingIn(spot_entity));
                commands.trigger(crate::events::OnGatheringParticipated {
                    entity: soul_entity,
                    spot_entity,
                });
                info!(
                    "GATHERING: Soul {:?} automatically recruited to spot {:?}",
                    soul_entity, spot_entity
                );
            }
        }
    }
}

/// 集会中でない参加者が中心から離れた時に参加を解除するシステム
pub fn gathering_leave_system(
    mut commands: Commands,
    q_spots: Query<&GatheringSpot>,
    q_participants: Query<(Entity, &Transform, &IdleState, &ParticipatingIn), With<DamnedSoul>>,
    update_timer: Res<GatheringUpdateTimer>,
) {
    if !update_timer.timer.just_finished() {
        return;
    }
    for (entity, transform, idle, participating_in) in q_participants.iter() {
        // 自発的に参加中（集会行動中）のSoulは離脱判定をしない
        if matches!(
            idle.behavior,
            IdleBehavior::Gathering | IdleBehavior::ExhaustedGathering
        ) {
            continue;
        }

        if let Ok(spot) = q_spots.get(participating_in.0) {
            let dist = (spot.center - transform.translation.truncate()).length();
            // 一定距離以上離れたら参加を解除
            if dist > GATHERING_LEAVE_RADIUS {
                commands.trigger(crate::events::OnGatheringLeft {
                    entity,
                    spot_entity: participating_in.0,
                });
                commands.entity(entity).remove::<ParticipatingIn>();
                info!(
                    "GATHERING: Soul {:?} left spot {:?} (too far away)",
                    entity, participating_in.0
                );
            }
        } else {
            // スポット自体が消滅している場合は、イベントなしでコンポーネントのみ削除
            // (スポット消滅時に参加者全員をトリガーするのは重いため、残留成分の掃除とみなす)
            commands.entity(entity).remove::<ParticipatingIn>();
        }
    }
}
