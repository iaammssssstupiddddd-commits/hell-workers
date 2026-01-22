use bevy::prelude::*;
use rand::Rng;

use crate::constants::*;
use crate::entities::damned_soul::{Destination, GatheringBehavior, IdleState, Path};
use crate::systems::soul_ai::gathering::{GatheringSpot, GatheringUpdateTimer, ParticipatingIn};
use crate::systems::soul_ai::task_execution::AssignedTask;
use crate::systems::spatial::{SpatialGrid, SpatialGridOps};
use crate::world::map::WorldMap;

/// 重なり回避の最小間隔
const GATHERING_MIN_SEPARATION: f32 = TILE_SIZE * 1.2;

pub fn gathering_separation_system(
    world_map: Res<WorldMap>,
    q_spots: Query<&GatheringSpot>,
    update_timer: Res<GatheringUpdateTimer>,
    mut query: Query<(
        Entity,
        &Transform,
        &mut Destination,
        &mut IdleState,
        &Path,
        &AssignedTask,
        &ParticipatingIn,
    )>,
    soul_grid: Res<SpatialGrid>,
) {
    if !update_timer.timer.just_finished() {
        return;
    }
    // 重なり判定用に、クエリから位置情報を取得する（このシステム内で位置が変わるため、グリッドより現在のクエリ結果が重要）
    // ただし、グリッド更新はこのシステムの前に終わっているため、soul_grid を活用する。
    // 完全に O(N) にするため、gathering_positions の全走査を soul_grid.get_nearby_in_radius に置き換える。

    for (entity, transform, mut dest, mut idle, soul_path, soul_task, participating_in) in
        query.iter_mut()
    {
        if !idle.needs_separation {
            continue;
        }

        // タスク実行中は重なり回避しない
        if !matches!(soul_task, AssignedTask::None) {
            idle.needs_separation = false;
            continue;
        }

        // 集会中のうろうろ状態（ターゲットに向かって歩いている最中）は回避イベントを発生させない
        if idle.gathering_behavior == GatheringBehavior::Wandering {
            idle.needs_separation = false;
            continue;
        }

        if let Ok(spot) = q_spots.get(participating_in.0) {
            let center = spot.center;
            let current_pos = transform.translation.truncate();

            // 目的地にまだ到達していない、またはパス移動中の場合は回避をスキップ
            if !soul_path.waypoints.is_empty()
                && soul_path.current_index < soul_path.waypoints.len()
            {
                continue;
            }

            let mut is_overlapping =
                (center - current_pos).length() < TILE_SIZE * GATHERING_KEEP_DISTANCE_MIN;

            if !is_overlapping {
                // 空間グリッドで近傍のSoulとの重なりをチェック
                let nearby_souls =
                    soul_grid.get_nearby_in_radius(current_pos, GATHERING_MIN_SEPARATION);
                for &other_entity in &nearby_souls {
                    if other_entity == entity {
                        continue;
                    }
                    is_overlapping = true;
                    break;
                }
            }

            if is_overlapping {
                let mut rng = rand::thread_rng();
                for _ in 0..10 {
                    let angle: f32 = rng.gen_range(0.0..std::f32::consts::TAU);
                    let dist: f32 = rng.gen_range(
                        TILE_SIZE * GATHERING_KEEP_DISTANCE_TARGET_MIN
                            ..TILE_SIZE * GATHERING_KEEP_DISTANCE_TARGET_MAX,
                    );
                    let offset = Vec2::new(angle.cos() * dist, angle.sin() * dist);
                    let new_pos = center + offset;

                    let mut valid = true;
                    // 空間グリッドで移動先が他のSoulと被らないかチェック
                    let nearby_at_new =
                        soul_grid.get_nearby_in_radius(new_pos, GATHERING_MIN_SEPARATION);
                    for &other_entity in &nearby_at_new {
                        if other_entity == entity {
                            continue;
                        }
                        valid = false;
                        break;
                    }

                    if valid {
                        let target_grid = WorldMap::world_to_grid(new_pos);
                        if world_map.is_walkable(target_grid.0, target_grid.1) {
                            dest.0 = new_pos;
                            break;
                        }
                    }
                }
            }
        }

        idle.needs_separation = false;
    }
}
