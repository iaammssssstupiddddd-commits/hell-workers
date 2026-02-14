use crate::systems::logistics::ResourceType;
use crate::systems::soul_ai::execute::task_execution::common::clear_task_and_path;
use crate::systems::soul_ai::execute::task_execution::context::TaskExecutionContext;
use crate::systems::soul_ai::execute::task_execution::transport_common::{cancel, reservation};
use crate::systems::soul_ai::execute::task_execution::types::{AssignedTask, HaulWaterToMixerPhase};
use bevy::prelude::*;

pub(super) fn abort_and_drop_bucket(
    commands: &mut Commands,
    ctx: &mut TaskExecutionContext,
    bucket_entity: Entity,
    tank_entity: Entity,
    mixer_entity: Entity,
    pos: Vec2,
) {
    reservation::release_mixer_destination(ctx, mixer_entity, ResourceType::Water);
    let should_release_tank_lock = matches!(
        ctx.task,
        AssignedTask::HaulWaterToMixer(data)
            if matches!(
                data.phase,
                HaulWaterToMixerPhase::GoingToBucket
                    | HaulWaterToMixerPhase::GoingToTank
                    | HaulWaterToMixerPhase::FillingFromTank
            )
    );
    if should_release_tank_lock {
        reservation::release_source(ctx, tank_entity, 1);
    }

    // バケツを地面にドロップして、関連コンポーネントをクリーンアップ
    cancel::drop_bucket_with_cleanup(commands, bucket_entity, pos);

    ctx.inventory.0 = None;
    clear_task_and_path(ctx.task, ctx.path);
}
