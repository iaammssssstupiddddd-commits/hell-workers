use bevy::prelude::*;
use rand::Rng;

use crate::constants::*;
use crate::relationships::ParticipatingIn;
use crate::entities::damned_soul::{IdleBehavior, IdleState};
use crate::systems::soul_ai::execute::task_execution::AssignedTask;
use crate::systems::soul_ai::helpers::gathering::{GatheringSpot, GatheringUpdateTimer};
use crate::systems::spatial::{SpatialGrid, SpatialGridOps};
use crate::world::map::WorldMap;

/// 重なり回避の最小間隔
const GATHERING_MIN_SEPARATION: f32 = TILE_SIZE * 1.2;

/// 集会中のSoul同士の重なりを防ぐシステム（0.5秒間隔）
pub fn gathering_separation_system(
    world_map: Res<WorldMap>,
    q_spots: Query<&GatheringSpot>,
    update_timer: Res<GatheringUpdateTimer>,
    soul_grid: Res<SpatialGrid>,
    mut q_souls: Query<
        (
            Entity,
            &Transform,
            &mut crate::entities::damned_soul::Destination,
            &mut crate::entities::damned_soul::Path,
            &AssignedTask,
            Option<&ParticipatingIn>,
            &IdleState,
        ),
        With<crate::entities::damned_soul::DamnedSoul>,
    >,
) {
    // 0.5秒間隔でのみ実行（パフォーマンス最適化）
    if !update_timer.timer.just_finished() {
        return;
    }

    for (entity, transform, mut dest, mut path, task, participating_in_opt, idle) in q_souls.iter_mut() {
        // タスク実行中は重なり回避しない
        if !matches!(task, AssignedTask::None) {
            continue;
        }

        // 休憩所へ移動中は重なり回避しない
        if idle.behavior == IdleBehavior::GoingToRest {
            continue;
        }

        // 集会に参加している場合のみ処理
        let Some(participating_in) = participating_in_opt else {
            continue;
        };

        let Ok(spot) = q_spots.get(participating_in.0) else {
            continue;
        };

        let center = spot.center;
        let current_pos = transform.translation.truncate();

        // Soul同士の重なりをチェック
        let nearby_souls = soul_grid.get_nearby_in_radius(current_pos, GATHERING_MIN_SEPARATION);
        let has_overlap = nearby_souls.iter().any(|&other| other != entity);

        // 中心に近すぎるかチェック
        let dist_from_center = (center - current_pos).length();
        let too_close_to_center = dist_from_center < TILE_SIZE * GATHERING_KEEP_DISTANCE_MIN;

        if too_close_to_center || has_overlap {
            let mut rng = rand::thread_rng();
            let mut found_valid_position = false;

            // 適切な位置を探す
            for _attempt in 0..30 {
                let angle: f32 = rng.gen_range(0.0..std::f32::consts::TAU);
                let dist: f32 = rng.gen_range(
                    TILE_SIZE * GATHERING_KEEP_DISTANCE_TARGET_MIN
                        ..TILE_SIZE * GATHERING_KEEP_DISTANCE_TARGET_MAX,
                );
                let offset = Vec2::new(angle.cos() * dist, angle.sin() * dist);
                let new_pos = center + offset;

                // 他のSoulと被らないかチェック
                let nearby_at_new =
                    soul_grid.get_nearby_in_radius(new_pos, GATHERING_MIN_SEPARATION);
                let position_occupied = nearby_at_new.iter().any(|&other| other != entity);

                if !position_occupied {
                    let target_grid = WorldMap::world_to_grid(new_pos);
                    if world_map.is_walkable(target_grid.0, target_grid.1) {
                        dest.0 = new_pos;
                        path.waypoints.clear();
                        path.current_index = 0;
                        found_valid_position = true;
                        break;
                    }
                }
            }

            if !found_valid_position {
                // 見つからない場合、中心の反対方向に強制移動
                let away = if dist_from_center > 0.1 {
                    (current_pos - center).normalize()
                } else {
                    // 完全に中心にいる場合はランダムな方向
                    let angle: f32 = rng.gen_range(0.0..std::f32::consts::TAU);
                    Vec2::new(angle.cos(), angle.sin())
                };
                let new_pos = center + away * TILE_SIZE * GATHERING_KEEP_DISTANCE_TARGET_MAX;

                // フォールバック位置が歩行可能かチェック
                let target_grid = WorldMap::world_to_grid(new_pos);
                if world_map.is_walkable(target_grid.0, target_grid.1) {
                    dest.0 = new_pos;
                    path.waypoints.clear();
                    path.current_index = 0;
                }
            }
        }
    }
}
