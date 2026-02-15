use bevy::prelude::*;
use rand::Rng;

use crate::constants::*;
use crate::systems::soul_ai::execute::task_execution::AssignedTask;
use crate::systems::soul_ai::helpers::gathering::{GatheringSpot, GatheringUpdateTimer};
use crate::systems::spatial::{SpatialGrid, SpatialGridOps};
use crate::world::map::WorldMap;
use crate::relationships::ParticipatingIn;

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
        ),
        With<crate::entities::damned_soul::DamnedSoul>,
    >,
) {
    // 0.5秒間隔でのみ実行（パフォーマンス最適化）
    if !update_timer.timer.just_finished() {
        return;
    }

    info!("SEPARATION: System running, checking {} souls", q_souls.iter().count());
    let mut souls_needing_separation = 0;
    let mut souls_with_overlap = 0;
    let mut souls_too_close_to_center = 0;

    for (entity, transform, mut dest, mut path, task, participating_in_opt) in q_souls.iter_mut() {
        // タスク実行中は重なり回避しない
        if !matches!(task, AssignedTask::None) {
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
            souls_needing_separation += 1;
            if has_overlap {
                souls_with_overlap += 1;
            }
            if too_close_to_center {
                souls_too_close_to_center += 1;
            }
            let dist_to_dest = (dest.0 - current_pos).length();
            info!(
                "SEPARATION: Soul {:?} needs separation - pos: {:?}, dest: {:?}, dist_to_dest: {:.1}, center_dist: {:.1}, overlap: {}, nearby: {}",
                entity, current_pos, dest.0, dist_to_dest, dist_from_center, has_overlap, nearby_souls.len()
            );

            let mut rng = rand::thread_rng();
            let mut found_valid_position = false;
            let old_dest = dest.0;
            let has_waypoints = !path.waypoints.is_empty();
            let waypoint_count = path.waypoints.len();

            // 適切な位置を探す
            for attempt in 0..30 {
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
                        info!(
                            "SEPARATION: Soul {:?} SET NEW DEST at attempt {} - from {:?} to {:?} (dist: {:.1}, had_waypoints: {}, count: {})",
                            entity,
                            attempt + 1,
                            old_dest,
                            new_pos,
                            dist,
                            has_waypoints,
                            waypoint_count
                        );
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
                    warn!(
                        "SEPARATION: Soul {:?} FALLBACK - from {:?} to {:?} (had_waypoints: {}, count: {})",
                        entity, old_dest, new_pos, has_waypoints, waypoint_count
                    );
                } else {
                    warn!(
                        "SEPARATION: Soul {:?} FALLBACK FAILED - position {:?} not walkable, keeping old dest",
                        entity, new_pos
                    );
                }
            }
        }
    }

    if souls_needing_separation > 0 {
        info!(
            "SEPARATION: Found {} souls needing separation ({} with overlap, {} too close to center)",
            souls_needing_separation, souls_with_overlap, souls_too_close_to_center
        );
    }
}
