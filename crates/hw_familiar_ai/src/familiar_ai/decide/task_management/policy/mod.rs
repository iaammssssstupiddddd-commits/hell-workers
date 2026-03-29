mod basic;
mod floor;
mod haul;
mod water;

pub use haul::take_source_selector_scan_snapshot;

use bevy::prelude::*;
use hw_jobs::WorkType;

use crate::familiar_ai::decide::task_management::context::ConstructionSitePositions;
use crate::familiar_ai::decide::task_management::{
    AssignTaskContext, FamiliarTaskAssignmentQueries, ReservationShadow,
};

pub fn assign_by_work_type(
    work_type: WorkType,
    task_pos: Vec2,
    already_commanded: bool,
    ctx: &AssignTaskContext<'_>,
    queries: &mut FamiliarTaskAssignmentQueries,
    construction_sites: &impl ConstructionSitePositions,
    shadow: &mut ReservationShadow,
) -> bool {
    match work_type {
        WorkType::Chop | WorkType::Mine => {
            basic::assign_gather(work_type, task_pos, already_commanded, ctx, queries, shadow)
        }
        WorkType::Build => basic::assign_build(task_pos, already_commanded, ctx, queries, shadow),
        WorkType::Move => basic::assign_move(task_pos, already_commanded, ctx, queries, shadow),
        WorkType::CollectSand => {
            basic::assign_collect_sand(task_pos, already_commanded, ctx, queries, shadow)
        }
        WorkType::CollectBone => {
            basic::assign_collect_bone(task_pos, already_commanded, ctx, queries, shadow)
        }
        WorkType::Refine => basic::assign_refine(task_pos, already_commanded, ctx, queries, shadow),
        WorkType::Haul | WorkType::WheelbarrowHaul => haul::assign_haul(
            task_pos,
            already_commanded,
            ctx,
            queries,
            construction_sites,
            shadow,
        ),
        WorkType::HaulToMixer => {
            haul::assign_haul_to_mixer(task_pos, already_commanded, ctx, queries, shadow)
        }
        WorkType::GatherWater => {
            water::assign_gather_water(task_pos, already_commanded, ctx, queries, shadow)
        }
        WorkType::HaulWaterToMixer => {
            water::assign_haul_water_to_mixer(task_pos, already_commanded, ctx, queries, shadow)
        }
        WorkType::ReinforceFloorTile => {
            floor::assign_reinforce_floor(task_pos, already_commanded, ctx, queries, shadow)
        }
        WorkType::PourFloorTile => {
            floor::assign_pour_floor(task_pos, already_commanded, ctx, queries, shadow)
        }
        WorkType::FrameWallTile => {
            floor::assign_frame_wall(task_pos, already_commanded, ctx, queries, shadow)
        }
        WorkType::CoatWall => {
            floor::assign_coat_wall(task_pos, already_commanded, ctx, queries, shadow)
        }
        WorkType::GeneratePower => {
            basic::assign_generate_power(task_pos, already_commanded, ctx, queries, shadow)
        }
    }
}
