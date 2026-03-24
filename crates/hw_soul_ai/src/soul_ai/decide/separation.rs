use bevy::prelude::*;

use hw_core::constants::*;
use hw_core::relationships::ParticipatingIn;
use hw_core::soul::{DamnedSoul, Destination, IdleBehavior, IdleState, Path};
use hw_jobs::AssignedTask;
use hw_spatial::{SpatialGrid, SpatialGridOps};
use hw_world::WorldMap;

use crate::soul_ai::helpers::gathering::{GatheringSpot, GatheringUpdateTimer};
use crate::soul_ai::helpers::gathering_positions::{
    find_position_fallback_away, find_position_with_separation,
};

/// 重なり回避の最小間隔
const GATHERING_MIN_SEPARATION: f32 = TILE_SIZE * 1.2;

type GatheringSeparationQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Transform,
        &'static mut Destination,
        &'static mut Path,
        &'static AssignedTask,
        Option<&'static ParticipatingIn>,
        &'static IdleState,
    ),
    With<DamnedSoul>,
>;

/// 集会中のSoul同士の重なりを防ぐシステム（0.5秒間隔）
pub fn gathering_separation_system(
    world_map: Res<WorldMap>,
    q_spots: Query<&GatheringSpot>,
    update_timer: Res<GatheringUpdateTimer>,
    soul_grid: Res<SpatialGrid>,
    mut nearby_buf: Local<Vec<Entity>>,
    mut q_souls: GatheringSeparationQuery,
) {
    if !update_timer.timer.just_finished() {
        return;
    }

    for (entity, transform, mut dest, mut path, task, participating_in_opt, idle) in
        q_souls.iter_mut()
    {
        if !matches!(task, AssignedTask::None) {
            continue;
        }
        if idle.behavior == IdleBehavior::GoingToRest {
            continue;
        }
        let Some(participating_in) = participating_in_opt else {
            continue;
        };
        let Ok(spot) = q_spots.get(participating_in.0) else {
            continue;
        };

        let center = spot.center;
        let current_pos = transform.translation.truncate();

        soul_grid.get_nearby_in_radius_into(current_pos, GATHERING_MIN_SEPARATION, &mut nearby_buf);
        let has_overlap = nearby_buf.iter().any(|&other| other != entity);
        let dist_from_center = (center - current_pos).length();
        let too_close_to_center = dist_from_center < TILE_SIZE * GATHERING_KEEP_DISTANCE_MIN;

        if too_close_to_center || has_overlap {
            let min_dist = TILE_SIZE * GATHERING_KEEP_DISTANCE_TARGET_MIN;
            let max_dist = TILE_SIZE * GATHERING_KEEP_DISTANCE_TARGET_MAX;

            if let Some(new_pos) = find_position_with_separation(
                center,
                entity,
                &*soul_grid,
                world_map.as_ref(),
                &mut nearby_buf,
                min_dist,
                max_dist,
                GATHERING_MIN_SEPARATION,
                30,
            ) {
                dest.0 = new_pos;
                path.waypoints.clear();
                path.current_index = 0;
            } else if let Some(new_pos) = find_position_fallback_away(
                center,
                current_pos,
                entity,
                &*soul_grid,
                world_map.as_ref(),
                &mut nearby_buf,
            ) {
                dest.0 = new_pos;
                path.waypoints.clear();
                path.current_index = 0;
            }
        }
    }
}
