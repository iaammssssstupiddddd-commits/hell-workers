//! 各 AssignedTask バリアントの TaskHandler 実装

use crate::soul_ai::execute::task_execution::context::TaskExecutionContext;
use bevy::prelude::*;
use hw_core::soul::StressBreakdown;
use hw_core::visual::SoulTaskHandles;
use hw_world::WorldMap;

use super::task_handler::TaskHandler;
use crate::soul_ai::execute::task_execution::types::{
    AssignedTask, BuildData, CoatWallData, CollectBoneData, CollectSandData, FrameWallTileData,
    GatherData, HaulData, HaulToBlueprintData, HaulToMixerData, MovePlantData, PourFloorTileData,
    RefineData, ReinforceFloorTileData,
};

impl TaskHandler<GatherData> for AssignedTask {
    fn execute(
        ctx: &mut TaskExecutionContext,
        data: GatherData,
        commands: &mut Commands,
        soul_handles: &SoulTaskHandles,
        time: &Res<Time>,
        world_map: &WorldMap,
        _breakdown_opt: Option<&StressBreakdown>,
    ) {
        crate::soul_ai::execute::task_execution::gather::handle_gather_task(
            ctx,
            crate::soul_ai::execute::task_execution::gather::GatherTaskArgs {
                target: data.target,
                work_type: &data.work_type,
                phase: data.phase,
            },
            commands,
            soul_handles,
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
        _soul_handles: &SoulTaskHandles,
        _time: &Res<Time>,
        world_map: &WorldMap,
        _breakdown_opt: Option<&StressBreakdown>,
    ) {
        crate::soul_ai::execute::task_execution::haul::handle_haul_task(
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
        _soul_handles: &SoulTaskHandles,
        time: &Res<Time>,
        world_map: &WorldMap,
        _breakdown_opt: Option<&StressBreakdown>,
    ) {
        crate::soul_ai::execute::task_execution::build::handle_build_task(
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
        _soul_handles: &SoulTaskHandles,
        _time: &Res<Time>,
        world_map: &WorldMap,
        breakdown_opt: Option<&StressBreakdown>,
    ) {
        crate::soul_ai::execute::task_execution::haul_to_blueprint::handle_haul_to_blueprint_task(
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

impl TaskHandler<CollectSandData> for AssignedTask {
    fn execute(
        ctx: &mut TaskExecutionContext,
        data: CollectSandData,
        commands: &mut Commands,
        soul_handles: &SoulTaskHandles,
        time: &Res<Time>,
        world_map: &WorldMap,
        _breakdown_opt: Option<&StressBreakdown>,
    ) {
        crate::soul_ai::execute::task_execution::collect_sand::handle_collect_sand_task(
            ctx,
            data.target,
            data.phase,
            commands,
            soul_handles,
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
        soul_handles: &SoulTaskHandles,
        time: &Res<Time>,
        world_map: &WorldMap,
        _breakdown_opt: Option<&StressBreakdown>,
    ) {
        crate::soul_ai::execute::task_execution::collect_bone::handle_collect_bone_task(
            ctx,
            data.target,
            data.phase,
            commands,
            soul_handles,
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
        soul_handles: &SoulTaskHandles,
        time: &Res<Time>,
        world_map: &WorldMap,
        _breakdown_opt: Option<&StressBreakdown>,
    ) {
        crate::soul_ai::execute::task_execution::refine::handle_refine_task(
            ctx,
            data.mixer,
            data.phase,
            commands,
            soul_handles,
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
        _soul_handles: &SoulTaskHandles,
        _time: &Res<Time>,
        world_map: &WorldMap,
        _breakdown_opt: Option<&StressBreakdown>,
    ) {
        crate::soul_ai::execute::task_execution::haul_to_mixer::handle_haul_to_mixer_task(
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

impl TaskHandler<ReinforceFloorTileData> for AssignedTask {
    fn execute(
        ctx: &mut TaskExecutionContext,
        data: ReinforceFloorTileData,
        commands: &mut Commands,
        _soul_handles: &SoulTaskHandles,
        time: &Res<Time>,
        world_map: &WorldMap,
        _breakdown_opt: Option<&StressBreakdown>,
    ) {
        crate::soul_ai::execute::task_execution::reinforce_floor::handle_reinforce_floor_task(
            ctx, data.tile, data.site, data.phase, commands, time, world_map,
        );
    }
}

impl TaskHandler<PourFloorTileData> for AssignedTask {
    fn execute(
        ctx: &mut TaskExecutionContext,
        data: PourFloorTileData,
        commands: &mut Commands,
        _soul_handles: &SoulTaskHandles,
        time: &Res<Time>,
        world_map: &WorldMap,
        _breakdown_opt: Option<&StressBreakdown>,
    ) {
        crate::soul_ai::execute::task_execution::pour_floor::handle_pour_floor_task(
            ctx, data.tile, data.site, data.phase, commands, time, world_map,
        );
    }
}

impl TaskHandler<CoatWallData> for AssignedTask {
    fn execute(
        ctx: &mut TaskExecutionContext,
        data: CoatWallData,
        commands: &mut Commands,
        _soul_handles: &SoulTaskHandles,
        time: &Res<Time>,
        world_map: &WorldMap,
        _breakdown_opt: Option<&StressBreakdown>,
    ) {
        crate::soul_ai::execute::task_execution::coat_wall::handle_coat_wall_task(
            ctx,
            crate::soul_ai::execute::task_execution::coat_wall::CoatWallArgs {
                tile_entity: data.tile,
                site_entity: data.site,
                wall_entity: data.wall,
                phase: data.phase,
            },
            commands,
            time,
            world_map,
        );
    }
}

impl TaskHandler<FrameWallTileData> for AssignedTask {
    fn execute(
        ctx: &mut TaskExecutionContext,
        data: FrameWallTileData,
        commands: &mut Commands,
        _soul_handles: &SoulTaskHandles,
        time: &Res<Time>,
        world_map: &WorldMap,
        _breakdown_opt: Option<&StressBreakdown>,
    ) {
        crate::soul_ai::execute::task_execution::frame_wall::handle_frame_wall_task(
            ctx, data.tile, data.site, data.phase, commands, time, world_map,
        );
    }
}

impl TaskHandler<MovePlantData> for AssignedTask {
    fn execute(
        ctx: &mut TaskExecutionContext,
        data: MovePlantData,
        commands: &mut Commands,
        _soul_handles: &SoulTaskHandles,
        _time: &Res<Time>,
        world_map: &WorldMap,
        _breakdown_opt: Option<&StressBreakdown>,
    ) {
        crate::soul_ai::execute::task_execution::move_plant::handle_move_plant_task(
            ctx, data, commands, world_map,
        );
    }
}
