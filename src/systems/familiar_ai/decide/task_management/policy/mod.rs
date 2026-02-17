mod basic;
mod floor;
mod haul;
mod water;

pub(crate) use haul::take_source_selector_scan_snapshot;

use crate::systems::familiar_ai::decide::task_management::{AssignTaskContext, ReservationShadow};
use crate::systems::jobs::WorkType;
use bevy::prelude::*;

pub fn assign_by_work_type(
    work_type: WorkType,
    task_pos: Vec2,
    already_commanded: bool,
    ctx: &AssignTaskContext<'_>,
    queries: &mut crate::systems::soul_ai::execute::task_execution::context::TaskAssignmentQueries,
    shadow: &mut ReservationShadow,
) -> bool {
    match work_type {
        WorkType::Chop | WorkType::Mine => {
            basic::assign_gather(work_type, task_pos, already_commanded, ctx, queries, shadow)
        }
        WorkType::Build => basic::assign_build(task_pos, already_commanded, ctx, queries, shadow),
        WorkType::CollectSand => {
            basic::assign_collect_sand(task_pos, already_commanded, ctx, queries, shadow)
        }
        WorkType::CollectBone => {
            basic::assign_collect_bone(task_pos, already_commanded, ctx, queries, shadow)
        }
        WorkType::Refine => basic::assign_refine(task_pos, already_commanded, ctx, queries, shadow),
        WorkType::Haul | WorkType::WheelbarrowHaul => {
            haul::assign_haul(task_pos, already_commanded, ctx, queries, shadow)
        }
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
        WorkType::CoatWall => {
            floor::assign_coat_wall(task_pos, already_commanded, ctx, queries, shadow)
        }
    }
}
