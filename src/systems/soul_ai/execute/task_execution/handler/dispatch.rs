//! AssignedTask ごとのディスパッチロジック

use crate::entities::damned_soul::StressBreakdown;
use crate::systems::logistics::Wheelbarrow;
use crate::systems::soul_ai::execute::task_execution::context::TaskExecutionContext;
use crate::world::map::WorldMap;
use bevy::prelude::*;

use super::task_handler::TaskHandler;
use crate::systems::soul_ai::execute::task_execution::types::{
    AssignedTask, HaulWithWheelbarrowData,
};

/// タスクタイプに応じて適切なハンドラにルーティングする。
/// 標準ハンドラは TaskHandler 経由、HaulWithWheelbarrow は q_wheelbarrows を渡すため特別扱い。
pub fn run_task_handler(
    ctx: &mut TaskExecutionContext,
    commands: &mut Commands,
    game_assets: &Res<crate::assets::GameAssets>,
    time: &Res<Time>,
    world_map: &Res<WorldMap>,
    breakdown_opt: Option<&StressBreakdown>,
    q_wheelbarrows: &Query<
        (&Transform, Option<&crate::relationships::ParkedAt>),
        With<Wheelbarrow>,
    >,
) {
    match &*ctx.task {
        AssignedTask::Gather(data) => {
            AssignedTask::execute(ctx, data.clone(), commands, game_assets, time, world_map, breakdown_opt);
        }
        AssignedTask::Haul(data) => {
            AssignedTask::execute(ctx, data.clone(), commands, game_assets, time, world_map, breakdown_opt);
        }
        AssignedTask::Build(data) => {
            AssignedTask::execute(ctx, data.clone(), commands, game_assets, time, world_map, breakdown_opt);
        }
        AssignedTask::HaulToBlueprint(data) => {
            AssignedTask::execute(ctx, data.clone(), commands, game_assets, time, world_map, breakdown_opt);
        }
        AssignedTask::GatherWater(data) => {
            AssignedTask::execute(ctx, data.clone(), commands, game_assets, time, world_map, breakdown_opt);
        }
        AssignedTask::CollectSand(data) => {
            AssignedTask::execute(ctx, data.clone(), commands, game_assets, time, world_map, breakdown_opt);
        }
        AssignedTask::CollectBone(data) => {
            AssignedTask::execute(ctx, data.clone(), commands, game_assets, time, world_map, breakdown_opt);
        }
        AssignedTask::Refine(data) => {
            AssignedTask::execute(ctx, data.clone(), commands, game_assets, time, world_map, breakdown_opt);
        }
        AssignedTask::HaulToMixer(data) => {
            AssignedTask::execute(ctx, data.clone(), commands, game_assets, time, world_map, breakdown_opt);
        }
        AssignedTask::HaulWaterToMixer(data) => {
            AssignedTask::execute(ctx, data.clone(), commands, game_assets, time, world_map, breakdown_opt);
        }
        AssignedTask::HaulWithWheelbarrow(data) => {
            execute_haul_with_wheelbarrow(ctx, data.clone(), commands, world_map, q_wheelbarrows);
        }
        AssignedTask::ReinforceFloorTile(data) => {
            AssignedTask::execute(ctx, data.clone(), commands, game_assets, time, world_map, breakdown_opt);
        }
        AssignedTask::PourFloorTile(data) => {
            AssignedTask::execute(ctx, data.clone(), commands, game_assets, time, world_map, breakdown_opt);
        }
        AssignedTask::CoatWall(data) => {
            AssignedTask::execute(ctx, data.clone(), commands, game_assets, time, world_map, breakdown_opt);
        }
        AssignedTask::None => {}
    }
}

/// HaulWithWheelbarrow 専用: q_wheelbarrows を追加で受け取る
pub fn execute_haul_with_wheelbarrow(
    ctx: &mut TaskExecutionContext,
    data: HaulWithWheelbarrowData,
    commands: &mut Commands,
    world_map: &Res<WorldMap>,
    q_wheelbarrows: &Query<
        (&Transform, Option<&crate::relationships::ParkedAt>),
        With<Wheelbarrow>,
    >,
) {
    crate::systems::soul_ai::execute::task_execution::haul_with_wheelbarrow::handle_haul_with_wheelbarrow_task(
        ctx,
        data,
        commands,
        world_map,
        q_wheelbarrows,
    );
}
