//! タスクハンドラのトレイト定義
//!
//! 各 AssignedTask バリアントの実行ロジックをトレイトでグループ化する。
//! Bevy の SystemParam 制約のため、Query はトレイト経由で渡さず、
//! 呼び出し元の match で直接ハンドラに渡す形にする。

use crate::assets::GameAssets;
use crate::entities::damned_soul::StressBreakdown;
use crate::systems::logistics::Wheelbarrow;
use crate::systems::soul_ai::execute::task_execution::context::TaskExecutionContext;
use crate::world::map::WorldMap;
use bevy::prelude::*;

use super::types::{
    AssignedTask, BuildData, CollectSandData, GatherData, GatherWaterData, HaulData,
    HaulToBlueprintData, HaulWaterToMixerData, HaulWithWheelbarrowData, RefineData,
};

/// タスクタイプごとの実行ロジックを表すトレイト
///
/// Query のライフタイム制約のため、q_wheelbarrows はトレイトの外で渡す。
pub trait TaskHandler<T> {
    fn execute(
        ctx: &mut TaskExecutionContext,
        data: T,
        commands: &mut Commands,
        game_assets: &Res<GameAssets>,
        time: &Res<Time>,
        world_map: &Res<WorldMap>,
        breakdown_opt: Option<&StressBreakdown>,
    );
}

impl TaskHandler<GatherData> for AssignedTask {
    fn execute(
        ctx: &mut TaskExecutionContext,
        data: GatherData,
        commands: &mut Commands,
        game_assets: &Res<GameAssets>,
        time: &Res<Time>,
        world_map: &Res<WorldMap>,
        _breakdown_opt: Option<&StressBreakdown>,
    ) {
        super::gather::handle_gather_task(
            ctx,
            data.target,
            &data.work_type,
            data.phase,
            commands,
            game_assets,
            time,
            world_map,
        );
    }
}

impl TaskHandler<HaulData> for AssignedTask {
    fn execute(
        ctx: &mut TaskExecutionContext,
        data: HaulData,
        commands: &mut Commands,
        _game_assets: &Res<GameAssets>,
        _time: &Res<Time>,
        world_map: &Res<WorldMap>,
        _breakdown_opt: Option<&StressBreakdown>,
    ) {
        super::haul::handle_haul_task(
            ctx,
            data.item,
            data.stockpile,
            data.phase,
            commands,
            world_map,
        );
    }
}

impl TaskHandler<BuildData> for AssignedTask {
    fn execute(
        ctx: &mut TaskExecutionContext,
        data: BuildData,
        commands: &mut Commands,
        _game_assets: &Res<GameAssets>,
        time: &Res<Time>,
        world_map: &Res<WorldMap>,
        _breakdown_opt: Option<&StressBreakdown>,
    ) {
        super::build::handle_build_task(
            ctx,
            data.blueprint,
            data.phase,
            commands,
            time,
            world_map,
        );
    }
}

impl TaskHandler<HaulToBlueprintData> for AssignedTask {
    fn execute(
        ctx: &mut TaskExecutionContext,
        data: HaulToBlueprintData,
        commands: &mut Commands,
        _game_assets: &Res<GameAssets>,
        _time: &Res<Time>,
        world_map: &Res<WorldMap>,
        breakdown_opt: Option<&StressBreakdown>,
    ) {
        super::haul_to_blueprint::handle_haul_to_blueprint_task(
            ctx,
            breakdown_opt,
            data.item,
            data.blueprint,
            data.phase,
            commands,
            world_map,
        );
    }
}

impl TaskHandler<GatherWaterData> for AssignedTask {
    fn execute(
        ctx: &mut TaskExecutionContext,
        data: GatherWaterData,
        commands: &mut Commands,
        game_assets: &Res<GameAssets>,
        time: &Res<Time>,
        world_map: &Res<WorldMap>,
        _breakdown_opt: Option<&StressBreakdown>,
    ) {
        super::gather_water::handle_gather_water_task(
            ctx,
            data.bucket,
            data.tank,
            data.phase,
            commands,
            game_assets,
            time,
            world_map,
        );
    }
}

impl TaskHandler<CollectSandData> for AssignedTask {
    fn execute(
        ctx: &mut TaskExecutionContext,
        data: CollectSandData,
        commands: &mut Commands,
        game_assets: &Res<GameAssets>,
        time: &Res<Time>,
        world_map: &Res<WorldMap>,
        _breakdown_opt: Option<&StressBreakdown>,
    ) {
        super::collect_sand::handle_collect_sand_task(
            ctx,
            data.target,
            data.phase,
            commands,
            game_assets,
            time,
            world_map,
        );
    }
}

impl TaskHandler<RefineData> for AssignedTask {
    fn execute(
        ctx: &mut TaskExecutionContext,
        data: RefineData,
        commands: &mut Commands,
        game_assets: &Res<GameAssets>,
        time: &Res<Time>,
        world_map: &Res<WorldMap>,
        _breakdown_opt: Option<&StressBreakdown>,
    ) {
        super::refine::handle_refine_task(
            ctx,
            data.mixer,
            data.phase,
            commands,
            game_assets,
            time,
            world_map,
        );
    }
}

impl TaskHandler<super::types::HaulToMixerData> for AssignedTask {
    fn execute(
        ctx: &mut TaskExecutionContext,
        data: super::types::HaulToMixerData,
        commands: &mut Commands,
        _game_assets: &Res<GameAssets>,
        _time: &Res<Time>,
        world_map: &Res<WorldMap>,
        _breakdown_opt: Option<&StressBreakdown>,
    ) {
        super::haul_to_mixer::handle_haul_to_mixer_task(
            ctx,
            data.item,
            data.mixer,
            data.resource_type,
            data.phase,
            commands,
            world_map,
        );
    }
}

impl TaskHandler<HaulWaterToMixerData> for AssignedTask {
    fn execute(
        ctx: &mut TaskExecutionContext,
        data: HaulWaterToMixerData,
        commands: &mut Commands,
        game_assets: &Res<GameAssets>,
        time: &Res<Time>,
        world_map: &Res<WorldMap>,
        _breakdown_opt: Option<&StressBreakdown>,
    ) {
        super::haul_water_to_mixer::handle_haul_water_to_mixer_task(
            ctx,
            data.bucket,
            data.tank,
            data.mixer,
            data.phase,
            commands,
            game_assets,
            time,
            world_map,
        );
    }
}

/// Phase 4: タスクルーティングの共通ディスパッチ
/// 標準ハンドラは TaskHandler 経由、HaulWithWheelbarrow は API 境界で特別扱い
pub fn run_task_handler(
    ctx: &mut TaskExecutionContext,
    commands: &mut Commands,
    game_assets: &Res<GameAssets>,
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
        AssignedTask::None => {}
    }
}

/// HaulWithWheelbarrow 専用: q_wheelbarrows を追加で受け取る（API 境界で明示）
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
    super::haul_with_wheelbarrow::handle_haul_with_wheelbarrow_task(
        ctx,
        data,
        commands,
        world_map,
        q_wheelbarrows,
    );
}
