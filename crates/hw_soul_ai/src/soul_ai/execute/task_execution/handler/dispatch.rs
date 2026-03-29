//! AssignedTask ごとのディスパッチロジック

use crate::soul_ai::execute::task_execution::context::TaskExecutionContext;
use bevy::prelude::*;
use hw_core::soul::StressBreakdown;
use hw_core::visual::SoulTaskHandles;
use hw_logistics::Wheelbarrow;
use hw_world::WorldMap;

use super::task_handler::TaskHandler;
use crate::soul_ai::execute::task_execution::types::{AssignedTask, HaulWithWheelbarrowData};

/// タスクタイプに応じて適切なハンドラにルーティングする。
/// 標準ハンドラは TaskHandler 経由、HaulWithWheelbarrow は q_wheelbarrows を渡すため特別扱い。
pub fn run_task_handler(
    ctx: &mut TaskExecutionContext,
    commands: &mut Commands,
    soul_handles: &SoulTaskHandles,
    time: &Res<Time>,
    world_map: &WorldMap,
    breakdown_opt: Option<&StressBreakdown>,
    q_wheelbarrows: &Query<
        (&Transform, Option<&hw_core::relationships::ParkedAt>),
        With<Wheelbarrow>,
    >,
) {
    match &*ctx.task {
        AssignedTask::Gather(data) => {
            AssignedTask::execute(
                ctx,
                data.clone(),
                commands,
                soul_handles,
                time,
                world_map,
                breakdown_opt,
            );
        }
        AssignedTask::BucketTransport(data) => {
            crate::soul_ai::execute::task_execution::bucket_transport::handle_bucket_transport_task(
                ctx,
                data.clone(),
                commands,
                soul_handles,
                time,
                world_map,
            );
        }
        AssignedTask::Haul(data) => {
            AssignedTask::execute(
                ctx,
                data.clone(),
                commands,
                soul_handles,
                time,
                world_map,
                breakdown_opt,
            );
        }
        AssignedTask::Build(data) => {
            AssignedTask::execute(
                ctx,
                data.clone(),
                commands,
                soul_handles,
                time,
                world_map,
                breakdown_opt,
            );
        }
        AssignedTask::MovePlant(data) => {
            AssignedTask::execute(
                ctx,
                data.clone(),
                commands,
                soul_handles,
                time,
                world_map,
                breakdown_opt,
            );
        }
        AssignedTask::HaulToBlueprint(data) => {
            AssignedTask::execute(
                ctx,
                data.clone(),
                commands,
                soul_handles,
                time,
                world_map,
                breakdown_opt,
            );
        }
        AssignedTask::CollectSand(data) => {
            AssignedTask::execute(
                ctx,
                data.clone(),
                commands,
                soul_handles,
                time,
                world_map,
                breakdown_opt,
            );
        }
        AssignedTask::CollectBone(data) => {
            AssignedTask::execute(
                ctx,
                data.clone(),
                commands,
                soul_handles,
                time,
                world_map,
                breakdown_opt,
            );
        }
        AssignedTask::Refine(data) => {
            AssignedTask::execute(
                ctx,
                data.clone(),
                commands,
                soul_handles,
                time,
                world_map,
                breakdown_opt,
            );
        }
        AssignedTask::HaulToMixer(data) => {
            AssignedTask::execute(
                ctx,
                data.clone(),
                commands,
                soul_handles,
                time,
                world_map,
                breakdown_opt,
            );
        }
        AssignedTask::HaulWithWheelbarrow(data) => {
            execute_haul_with_wheelbarrow(ctx, data.clone(), commands, world_map, q_wheelbarrows);
        }
        AssignedTask::ReinforceFloorTile(data) => {
            AssignedTask::execute(
                ctx,
                data.clone(),
                commands,
                soul_handles,
                time,
                world_map,
                breakdown_opt,
            );
        }
        AssignedTask::PourFloorTile(data) => {
            AssignedTask::execute(
                ctx,
                data.clone(),
                commands,
                soul_handles,
                time,
                world_map,
                breakdown_opt,
            );
        }
        AssignedTask::CoatWall(data) => {
            AssignedTask::execute(
                ctx,
                data.clone(),
                commands,
                soul_handles,
                time,
                world_map,
                breakdown_opt,
            );
        }
        AssignedTask::FrameWallTile(data) => {
            AssignedTask::execute(
                ctx,
                data.clone(),
                commands,
                soul_handles,
                time,
                world_map,
                breakdown_opt,
            );
        }
        AssignedTask::GeneratePower(data) => {
            crate::soul_ai::execute::task_execution::generate_power::handle_generate_power_task(
                ctx,
                data.clone(),
                commands,
                time,
                world_map,
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
    world_map: &WorldMap,
    q_wheelbarrows: &Query<
        (&Transform, Option<&hw_core::relationships::ParkedAt>),
        With<Wheelbarrow>,
    >,
) {
    crate::soul_ai::execute::task_execution::haul_with_wheelbarrow::handle_haul_with_wheelbarrow_task(
        ctx,
        data,
        commands,
        world_map,
        q_wheelbarrows,
    );
}
