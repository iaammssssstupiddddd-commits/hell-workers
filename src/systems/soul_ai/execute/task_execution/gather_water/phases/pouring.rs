//! Pouring phase: Pour water into tank

use crate::constants::{BUCKET_CAPACITY, TILE_SIZE};
use crate::systems::logistics::{ResourceItem, ResourceType};
use crate::systems::soul_ai::execute::task_execution::context::TaskExecutionContext;
use crate::systems::soul_ai::execute::task_execution::types::GatherWaterPhase;
use crate::world::map::WorldMap;
use bevy::prelude::*;

use super::super::guards;
use super::super::helpers::{abort_task_without_item, drop_bucket_for_auto_haul};
use super::assigned_task;

pub fn handle(
    ctx: &mut TaskExecutionContext,
    bucket_entity: Entity,
    tank_entity: Entity,
    progress: f32,
    commands: &mut Commands,
    game_assets: &Res<crate::assets::GameAssets>,
    world_map: &WorldMap,
    _soul_pos: Vec2,
) {
    if !guards::has_bucket_in_inventory(ctx, bucket_entity) {
        warn!(
            "Pouring: Bucket not in inventory, aborting task for soul {:?}",
            ctx.soul_entity
        );
        abort_task_without_item(commands, ctx, world_map);
        return;
    }

    let time_delta = 1.0;
    let new_progress = progress + time_delta * 1.0;

    if new_progress >= 1.0 {
        commands
            .entity(bucket_entity)
            .try_insert(ResourceItem(ResourceType::BucketEmpty));
        commands.entity(bucket_entity).try_insert(Sprite {
            image: game_assets.bucket_empty.clone(),
            custom_size: Some(Vec2::splat(TILE_SIZE * 0.6)),
            ..default()
        });

        for _ in 0..BUCKET_CAPACITY {
            commands.spawn((
                ResourceItem(ResourceType::Water),
                crate::relationships::StoredIn(tank_entity),
                Visibility::Hidden,
            ));
        }

        commands
            .entity(bucket_entity)
            .remove::<crate::relationships::DeliveringTo>();
        drop_bucket_for_auto_haul(commands, ctx, bucket_entity, tank_entity, world_map);
    } else {
        *ctx.task = assigned_task(
            bucket_entity,
            tank_entity,
            GatherWaterPhase::Pouring {
                progress: new_progress,
            },
        );
    }
}
