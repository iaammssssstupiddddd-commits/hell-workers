//! AssignedTask ごとのディスパッチロジック

use crate::soul_ai::execute::task_execution::context::TaskExecutionContext;
use bevy::prelude::*;
use hw_logistics::Wheelbarrow;

use crate::soul_ai::execute::task_execution::types::{AssignedTask, HaulWithWheelbarrowData};

/// タスクタイプに応じて適切なハンドラにルーティングする。
pub fn run_task_handler(
    ctx: &mut TaskExecutionContext,
    commands: &mut Commands,
    q_wheelbarrows: &Query<
        (&Transform, Option<&hw_core::relationships::ParkedAt>),
        With<Wheelbarrow>,
    >,
) {
    match &*ctx.task {
        AssignedTask::Gather(data) => {
            crate::soul_ai::execute::task_execution::gather::handle_gather_task(
                ctx,
                data.clone(),
                commands,
            );
        }
        AssignedTask::BucketTransport(data) => {
            crate::soul_ai::execute::task_execution::bucket_transport::handle_bucket_transport_task(
                ctx,
                data.clone(),
                commands,
            );
        }
        AssignedTask::Haul(data) => {
            crate::soul_ai::execute::task_execution::haul::handle_haul_task(
                ctx,
                data.clone(),
                commands,
            );
        }
        AssignedTask::Build(data) => {
            crate::soul_ai::execute::task_execution::build::handle_build_task(
                ctx,
                data.clone(),
                commands,
            );
        }
        AssignedTask::MovePlant(data) => {
            crate::soul_ai::execute::task_execution::move_plant::handle_move_plant_task(
                ctx,
                data.clone(),
                commands,
            );
        }
        AssignedTask::HaulToBlueprint(data) => {
            crate::soul_ai::execute::task_execution::haul_to_blueprint::handle_haul_to_blueprint_task(
                ctx,
                data.clone(),
                commands,
            );
        }
        AssignedTask::CollectBone(data) => {
            crate::soul_ai::execute::task_execution::collect_bone::handle_collect_bone_task(
                ctx,
                data.clone(),
                commands,
            );
        }
        AssignedTask::Refine(data) => {
            crate::soul_ai::execute::task_execution::refine::handle_refine_task(
                ctx,
                data.clone(),
                commands,
            );
        }
        AssignedTask::HaulToMixer(data) => {
            crate::soul_ai::execute::task_execution::haul_to_mixer::handle_haul_to_mixer_task(
                ctx,
                data.clone(),
                commands,
            );
        }
        AssignedTask::HaulWithWheelbarrow(data) => {
            execute_haul_with_wheelbarrow(ctx, data.clone(), commands, q_wheelbarrows);
        }
        AssignedTask::ReinforceFloorTile(data) => {
            crate::soul_ai::execute::task_execution::reinforce_floor::handle_reinforce_floor_task(
                ctx,
                data.clone(),
                commands,
            );
        }
        AssignedTask::PourFloorTile(data) => {
            crate::soul_ai::execute::task_execution::pour_floor::handle_pour_floor_task(
                ctx,
                data.clone(),
                commands,
            );
        }
        AssignedTask::CoatWall(data) => {
            crate::soul_ai::execute::task_execution::coat_wall::handle_coat_wall_task(
                ctx,
                data.clone(),
                commands,
            );
        }
        AssignedTask::FrameWallTile(data) => {
            crate::soul_ai::execute::task_execution::frame_wall::handle_frame_wall_task(
                ctx,
                data.clone(),
                commands,
            );
        }
        AssignedTask::GeneratePower(data) => {
            crate::soul_ai::execute::task_execution::generate_power::handle_generate_power_task(
                ctx,
                data.clone(),
                commands,
            );
        }
        AssignedTask::None => {}
    }
}

/// HaulWithWheelbarrow 専用: q_wheelbarrows を追加で受け取る
pub fn execute_haul_with_wheelbarrow(
    ctx: &mut TaskExecutionContext,
    data: HaulWithWheelbarrowData,
    commands: &mut Commands,
    q_wheelbarrows: &Query<
        (&Transform, Option<&hw_core::relationships::ParkedAt>),
        With<Wheelbarrow>,
    >,
) {
    crate::soul_ai::execute::task_execution::haul_with_wheelbarrow::handle_haul_with_wheelbarrow_task(
        ctx,
        data,
        commands,
        q_wheelbarrows,
    );
}
