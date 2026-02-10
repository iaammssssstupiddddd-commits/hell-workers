//! GoingToBucket phase: Navigate to pick up a bucket

use crate::systems::logistics::ResourceType;
use crate::systems::soul_ai::execute::task_execution::common;
use crate::systems::soul_ai::execute::task_execution::context::TaskExecutionContext;
use crate::systems::soul_ai::execute::task_execution::types::GatherWaterPhase;
use crate::world::map::WorldMap;
use bevy::prelude::*;

use super::super::guards;
use super::super::helpers::abort_task_with_item;
use super::super::routing;

pub fn handle(
    ctx: &mut TaskExecutionContext,
    bucket_entity: Entity,
    tank_entity: Entity,
    commands: &mut Commands,
    world_map: &WorldMap,
    soul_pos: Vec2,
) {
    if guards::has_bucket_in_inventory(ctx, bucket_entity) {
        if routing::set_path_to_river(ctx, world_map, bucket_entity, tank_entity).is_none() {
            abort_task_with_item(commands, ctx, world_map);
        }
        return;
    }

    let Ok((bucket_transform, _, _, res_item_opt, _, stored_in_opt)) =
        ctx.queries.designation.targets.get(bucket_entity)
    else {
        abort_task_with_item(commands, ctx, world_map);
        return;
    };

    let res_type = res_item_opt.map(|res| res.0);
    let stored_in_entity = stored_in_opt.map(|stored| stored.0);

    if let Some(rt) = res_type {
        if !matches!(rt, ResourceType::BucketEmpty | ResourceType::BucketWater) {
            abort_task_with_item(commands, ctx, world_map);
            return;
        }
    }

    let bucket_pos = bucket_transform.translation.truncate();

    if common::can_pickup_item(soul_pos, bucket_pos) {
        if !common::try_pickup_item(
            commands,
            ctx.soul_entity,
            bucket_entity,
            ctx.inventory,
            soul_pos,
            bucket_pos,
            ctx.task,
            ctx.path,
        ) {
            return;
        }

        ctx.queue_reservation(crate::events::ResourceReservationOp::RecordPickedSource {
            source: bucket_entity,
            amount: 1,
        });

        if let Some(stored_in) = stored_in_entity {
            let q_stockpiles = &mut ctx.queries.storage.stockpiles;
            common::update_stockpile_on_item_removal(stored_in, q_stockpiles);
        }

        let is_already_full = res_type == Some(ResourceType::BucketWater);
        if is_already_full {
            if let Ok((tank_transform, _, _, _, _, _)) =
                ctx.queries.designation.targets.get(tank_entity)
            {
                let tank_pos = tank_transform.translation.truncate();
                if routing::set_path_to_tank_boundary(
                    ctx,
                    world_map,
                    tank_pos,
                    bucket_entity,
                    tank_entity,
                    GatherWaterPhase::GoingToTank,
                )
                .is_some()
                {
                    return;
                }
            }
        }

        if routing::set_path_to_river(ctx, world_map, bucket_entity, tank_entity).is_none() {
            abort_task_with_item(commands, ctx, world_map);
        }
        return;
    }

    let bucket_grid = WorldMap::world_to_grid(bucket_pos);
    if ctx.path.waypoints.is_empty() {
        if routing::set_path_to_grid_boundary(ctx, world_map, bucket_grid, bucket_pos).is_none() {
            ctx.dest.0 = bucket_pos;
        }
    }
}
