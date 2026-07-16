#[cfg(feature = "profiling")]
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;

use hw_core::constants::*;
use hw_core::relationships::ParticipatingIn;
#[cfg(feature = "profiling")]
use hw_core::simulation_rng::{FixedAuditSeed, SimulationRandomState, SimulationRng};
use hw_core::soul::{DamnedSoul, Destination, IdleBehavior, IdleState, Path};
use hw_jobs::AssignedTask;
use hw_spatial::{SpatialGrid, SpatialGridOps};
use hw_world::WorldMap;

use crate::soul_ai::helpers::gathering::{GatheringSpot, GatheringUpdateTimer};
use crate::soul_ai::helpers::gathering_positions::SeparationParams;
#[cfg(not(feature = "profiling"))]
use crate::soul_ai::helpers::gathering_positions::{
    find_position_fallback_away, find_position_with_separation,
};
#[cfg(feature = "profiling")]
use crate::soul_ai::helpers::gathering_positions::{
    find_position_fallback_away_with_rng, find_position_with_separation_with_rng,
};

/// 重なり回避の最小間隔
const GATHERING_MIN_SEPARATION: f32 = TILE_SIZE * 1.2;

#[cfg(feature = "profiling")]
const GATHERING_SEPARATION_POSITION_STREAM: u64 = 0x6761_7468_5f73_6570;

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

#[cfg(feature = "profiling")]
#[derive(SystemParam)]
pub(crate) struct GatheringSeparationProfiling<'w, 's> {
    audit_seed: Option<Res<'w, FixedAuditSeed>>,
    random_states: Query<'w, 's, &'static mut SimulationRandomState>,
}

/// 集会中のSoul同士の重なりを防ぐシステム（0.5秒間隔）
pub(crate) fn gathering_separation_system(
    world_map: Res<WorldMap>,
    q_spots: Query<&GatheringSpot>,
    update_timer: Res<GatheringUpdateTimer>,
    soul_grid: Res<SpatialGrid>,
    mut nearby_buf: Local<Vec<Entity>>,
    mut q_souls: GatheringSeparationQuery,
    #[cfg(feature = "profiling")] profiling: GatheringSeparationProfiling,
) {
    #[cfg(feature = "profiling")]
    let GatheringSeparationProfiling {
        audit_seed,
        mut random_states,
    } = profiling;

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

            #[cfg(feature = "profiling")]
            let mut random_state = random_states.get_mut(entity).ok();
            #[cfg(feature = "profiling")]
            let mut rng = SimulationRng::for_actor(
                audit_seed.as_deref(),
                random_state.as_deref_mut(),
                GATHERING_SEPARATION_POSITION_STREAM,
            );
            #[cfg(feature = "profiling")]
            let new_position = find_position_with_separation_with_rng(
                center,
                entity,
                &*soul_grid,
                world_map.as_ref(),
                &mut nearby_buf,
                SeparationParams {
                    min_dist,
                    max_dist,
                    min_separation: GATHERING_MIN_SEPARATION,
                    max_attempts: 30,
                },
                &mut rng,
            );
            #[cfg(not(feature = "profiling"))]
            let new_position = find_position_with_separation(
                center,
                entity,
                &*soul_grid,
                world_map.as_ref(),
                &mut nearby_buf,
                SeparationParams {
                    min_dist,
                    max_dist,
                    min_separation: GATHERING_MIN_SEPARATION,
                    max_attempts: 30,
                },
            );

            if let Some(new_pos) = new_position {
                dest.0 = new_pos;
                path.waypoints.clear();
                path.current_index = 0;
                continue;
            }

            #[cfg(feature = "profiling")]
            let fallback_position = find_position_fallback_away_with_rng(
                center,
                current_pos,
                entity,
                &*soul_grid,
                world_map.as_ref(),
                &mut nearby_buf,
                &mut rng,
            );
            #[cfg(not(feature = "profiling"))]
            let fallback_position = find_position_fallback_away(
                center,
                current_pos,
                entity,
                &*soul_grid,
                world_map.as_ref(),
                &mut nearby_buf,
            );

            if let Some(new_pos) = fallback_position {
                dest.0 = new_pos;
                path.waypoints.clear();
                path.current_index = 0;
            }
        }
    }
}
