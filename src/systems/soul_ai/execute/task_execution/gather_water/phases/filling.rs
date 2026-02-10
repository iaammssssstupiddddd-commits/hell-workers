//! Filling phase: Fill the bucket with water

use crate::constants::TILE_SIZE;
use crate::systems::logistics::{ResourceItem, ResourceType};
use crate::systems::soul_ai::execute::task_execution::context::TaskExecutionContext;
use crate::systems::soul_ai::execute::task_execution::types::GatherWaterPhase;
use crate::world::map::WorldMap;
use bevy::prelude::*;

use super::assigned_task;
use super::super::guards;
use super::super::helpers::{abort_task_with_item, abort_task_without_item};
use super::super::routing;

pub fn handle(
    ctx: &mut TaskExecutionContext,
    bucket_entity: Entity,
    tank_entity: Entity,
    progress: f32,
    commands: &mut Commands,
    game_assets: &Res<crate::assets::GameAssets>,
    time: &Res<Time>,
    world_map: &WorldMap,
    _soul_pos: Vec2,
) {
    if !guards::has_bucket_in_inventory(ctx, bucket_entity) {
        warn!(
            "Filling: Bucket not in inventory, aborting task for soul {:?}",
            ctx.soul_entity
        );
        abort_task_without_item(commands, ctx, world_map);
        return;
    }

    let q_targets = &ctx.queries.designation.targets;
    let new_progress = progress + time.delta_secs() * 0.5;

    if new_progress >= 1.0 {
        commands.entity(bucket_entity).insert((
            ResourceItem(ResourceType::BucketWater),
            Sprite {
                image: game_assets.bucket_water.clone(),
                custom_size: Some(Vec2::splat(TILE_SIZE * 0.6)),
                ..default()
            },
        ));

        if let Ok((tank_transform, _, _, _, _, _)) = q_targets.get(tank_entity) {
            let tank_pos = tank_transform.translation.truncate();
            if routing::set_path_to_tank_boundary(
                ctx,
                world_map,
                tank_pos,
                bucket_entity,
                tank_entity,
                GatherWaterPhase::GoingToTank,
            )
            .is_none()
            {
                abort_task_with_item(commands, ctx, world_map);
            }
        } else {
            abort_task_with_item(commands, ctx, world_map);
        }
    } else {
        *ctx.task = assigned_task(
            bucket_entity,
            tank_entity,
            GatherWaterPhase::Filling {
                progress: new_progress,
            },
        );
    }
}
