use crate::systems::logistics::ResourceType;
use crate::systems::soul_ai::execute::task_execution::common::clear_task_and_path;
use crate::systems::soul_ai::execute::task_execution::context::TaskExecutionContext;
use crate::systems::soul_ai::execute::task_execution::types::{AssignedTask, HaulWaterToMixerPhase};
use crate::world::map::WorldMap;
use bevy::prelude::*;

pub(super) fn abort_and_drop_bucket(
    commands: &mut Commands,
    ctx: &mut TaskExecutionContext,
    bucket_entity: Entity,
    tank_entity: Entity,
    mixer_entity: Entity,
    pos: Vec2,
) {
    ctx.queue_reservation(
        crate::events::ResourceReservationOp::ReleaseMixerDestination {
            target: mixer_entity,
            resource_type: ResourceType::Water,
        },
    );
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
        ctx.queue_reservation(crate::events::ResourceReservationOp::ReleaseSource {
            source: tank_entity,
            amount: 1,
        });
    }

    // バケツを地面にドロップして、関連コンポーネントをクリーンアップ
    let drop_grid = WorldMap::world_to_grid(pos);
    let drop_pos = WorldMap::grid_to_world(drop_grid.0, drop_grid.1);
    commands.entity(bucket_entity).insert((
        Visibility::Visible,
        Transform::from_xyz(drop_pos.x, drop_pos.y, crate::constants::Z_ITEM_PICKUP),
    ));
    commands
        .entity(bucket_entity)
        .remove::<crate::relationships::StoredIn>();
    commands
        .entity(bucket_entity)
        .remove::<crate::systems::logistics::InStockpile>();
    commands
        .entity(bucket_entity)
        .remove::<crate::systems::jobs::IssuedBy>();
    commands
        .entity(bucket_entity)
        .remove::<crate::relationships::TaskWorkers>();
    commands
        .entity(bucket_entity)
        .remove::<crate::systems::jobs::Designation>();
    commands
        .entity(bucket_entity)
        .remove::<crate::systems::jobs::TaskSlots>();
    commands
        .entity(bucket_entity)
        .remove::<crate::systems::jobs::TargetMixer>();

    ctx.inventory.0 = None;
    clear_task_and_path(ctx.task, ctx.path);
}
