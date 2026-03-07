//! FillingFromTank phase: Fill bucket from tank water

use crate::constants::*;
use crate::systems::logistics::{ResourceItem, ResourceType};
use crate::systems::soul_ai::execute::task_execution::common::*;
use crate::systems::soul_ai::execute::task_execution::context::TaskExecutionContext;
use crate::systems::soul_ai::execute::task_execution::transport_common::reservation;
use crate::systems::soul_ai::execute::task_execution::types::{
    AssignedTask, HaulWaterToMixerData, HaulWaterToMixerPhase,
};
use crate::world::map::WorldMap;
use bevy::prelude::*;

use super::super::abort::abort_and_drop_bucket;

pub fn handle(
    ctx: &mut TaskExecutionContext,
    bucket_entity: Entity,
    tank_entity: Entity,
    mixer_entity: Entity,
    commands: &mut Commands,
    game_assets: &Res<crate::assets::GameAssets>,
    world_map: &Res<WorldMap>,
) {
    let soul_pos = ctx.soul_pos();

    // Tankから水を最大 BUCKET_CAPACITY 個取り出す
    let mut found_waters = Vec::new();
    for (res_entity, _, _, res_item, stored_in, _) in ctx.queries.resource_items.iter() {
        if res_item.0 == ResourceType::Water {
            if let Some(stored) = stored_in {
                if stored.0 == tank_entity {
                    found_waters.push(res_entity);
                    if found_waters.len() as u32 >= BUCKET_CAPACITY {
                        break;
                    }
                }
            }
        }
    }

    if !found_waters.is_empty() {
        let take_amount = found_waters.len() as u32;
        for water_entity in found_waters {
            commands.entity(water_entity).despawn();
        }
        // タンクからの取水フェーズを抜けるためロック解除
        reservation::release_source(ctx, tank_entity, 1);

        // バケツを水入りに変更
        commands.entity(bucket_entity).try_insert((
            ResourceItem(ResourceType::BucketWater),
            Sprite {
                image: game_assets.bucket_water.clone(),
                custom_size: Some(Vec2::splat(TILE_SIZE * 0.6)),
                ..default()
            },
        ));

        // Mixerへ
        if let Ok(mixer_data) = ctx.queries.storage.mixers.get(mixer_entity) {
            let (mixer_transform, _, _) = mixer_data;
            let mixer_pos = mixer_transform.translation.truncate();

            *ctx.task = AssignedTask::HaulWaterToMixer(HaulWaterToMixerData {
                bucket: bucket_entity,
                tank: tank_entity,
                mixer: mixer_entity,
                amount: take_amount,
                needs_tank_fill: true,
                phase: HaulWaterToMixerPhase::GoingToMixer,
            });
            commands
                .entity(bucket_entity)
                .try_insert(crate::relationships::DeliveringTo(mixer_entity));
            update_destination_to_adjacent(
                ctx.dest,
                mixer_pos,
                ctx.path,
                soul_pos,
                world_map,
                ctx.pf_context,
            );
        } else {
            abort_and_drop_bucket(
                commands,
                ctx,
                bucket_entity,
                tank_entity,
                mixer_entity,
                soul_pos,
            );
        }
    } else {
        // 水が尽きたら中断
        abort_and_drop_bucket(
            commands,
            ctx,
            bucket_entity,
            tank_entity,
            mixer_entity,
            soul_pos,
        );
    }
}
