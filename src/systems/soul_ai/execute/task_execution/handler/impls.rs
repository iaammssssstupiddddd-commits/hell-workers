//! 各 AssignedTask バリアントの TaskHandler 実装

use crate::assets::GameAssets;
use crate::entities::damned_soul::StressBreakdown;
use crate::systems::soul_ai::execute::task_execution::context::TaskExecutionContext;
use crate::world::map::WorldMap;
use bevy::prelude::*;

use super::task_handler::TaskHandler;
use crate::systems::soul_ai::execute::task_execution::types::{
    AssignedTask, BuildData, CoatWallData, CollectBoneData, CollectSandData, GatherData,
    GatherWaterData, HaulData, HaulToBlueprintData, HaulToMixerData, HaulWaterToMixerData,
    PourFloorTileData, RefineData, ReinforceFloorTileData,
};

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
        crate::systems::soul_ai::execute::task_execution::gather::handle_gather_task(
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
        crate::systems::soul_ai::execute::task_execution::haul::handle_haul_task(
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
        crate::systems::soul_ai::execute::task_execution::build::handle_build_task(
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
        crate::systems::soul_ai::execute::task_execution::haul_to_blueprint::handle_haul_to_blueprint_task(
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
        crate::systems::soul_ai::execute::task_execution::gather_water::handle_gather_water_task(
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
        crate::systems::soul_ai::execute::task_execution::collect_sand::handle_collect_sand_task(
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

impl TaskHandler<CollectBoneData> for AssignedTask {
    fn execute(
        ctx: &mut TaskExecutionContext,
        data: CollectBoneData,
        commands: &mut Commands,
        game_assets: &Res<GameAssets>,
        time: &Res<Time>,
        world_map: &Res<WorldMap>,
        _breakdown_opt: Option<&StressBreakdown>,
    ) {
        crate::systems::soul_ai::execute::task_execution::collect_bone::handle_collect_bone_task(
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
        crate::systems::soul_ai::execute::task_execution::refine::handle_refine_task(
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

impl TaskHandler<HaulToMixerData> for AssignedTask {
    fn execute(
        ctx: &mut TaskExecutionContext,
        data: HaulToMixerData,
        commands: &mut Commands,
        _game_assets: &Res<GameAssets>,
        _time: &Res<Time>,
        world_map: &Res<WorldMap>,
        _breakdown_opt: Option<&StressBreakdown>,
    ) {
        crate::systems::soul_ai::execute::task_execution::haul_to_mixer::handle_haul_to_mixer_task(
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
        crate::systems::soul_ai::execute::task_execution::haul_water_to_mixer::handle_haul_water_to_mixer_task(
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

impl TaskHandler<ReinforceFloorTileData> for AssignedTask {
    fn execute(
        ctx: &mut TaskExecutionContext,
        data: ReinforceFloorTileData,
        commands: &mut Commands,
        _game_assets: &Res<GameAssets>,
        time: &Res<Time>,
        world_map: &Res<WorldMap>,
        _breakdown_opt: Option<&StressBreakdown>,
    ) {
        crate::systems::soul_ai::execute::task_execution::reinforce_floor::handle_reinforce_floor_task(
            ctx,
            data.tile,
            data.site,
            data.phase,
            commands,
            time,
            world_map,
        );
    }
}

impl TaskHandler<PourFloorTileData> for AssignedTask {
    fn execute(
        ctx: &mut TaskExecutionContext,
        data: PourFloorTileData,
        commands: &mut Commands,
        _game_assets: &Res<GameAssets>,
        time: &Res<Time>,
        world_map: &Res<WorldMap>,
        _breakdown_opt: Option<&StressBreakdown>,
    ) {
        crate::systems::soul_ai::execute::task_execution::pour_floor::handle_pour_floor_task(
            ctx,
            data.tile,
            data.site,
            data.phase,
            commands,
            time,
            world_map,
        );
    }
}

impl TaskHandler<CoatWallData> for AssignedTask {
    fn execute(
        ctx: &mut TaskExecutionContext,
        data: CoatWallData,
        commands: &mut Commands,
        _game_assets: &Res<GameAssets>,
        time: &Res<Time>,
        world_map: &Res<WorldMap>,
        _breakdown_opt: Option<&StressBreakdown>,
    ) {
        crate::systems::soul_ai::execute::task_execution::coat_wall::handle_coat_wall_task(
            ctx,
            data.wall,
            data.phase,
            commands,
            time,
            world_map,
        );
    }
}
