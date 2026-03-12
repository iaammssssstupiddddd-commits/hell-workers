pub mod abort;
pub mod guards;
pub mod helpers;
pub mod phases;
pub mod routing;

use crate::systems::soul_ai::execute::task_execution::context::TaskExecutionContext;
use crate::systems::soul_ai::execute::task_execution::types::{
    BucketTransportData, BucketTransportPhase,
};
use crate::world::map::WorldMap;
use bevy::prelude::*;

/// バケツ輸送共通ハンドラ
pub fn handle_bucket_transport_task(
    ctx: &mut TaskExecutionContext,
    data: BucketTransportData,
    commands: &mut Commands,
    soul_handles: &hw_visual::SoulTaskHandles,
    time: &Res<Time>,
    world_map: &WorldMap,
) {
    match data.phase {
        BucketTransportPhase::GoingToBucket => {
            phases::going_to_bucket::handle(ctx, &data, commands, world_map);
        }
        BucketTransportPhase::GoingToSource => {
            phases::going_to_source::handle(ctx, &data, commands, world_map);
        }
        BucketTransportPhase::Filling { progress } => {
            phases::filling::handle(ctx, &data, progress, commands, soul_handles, time, world_map);
        }
        BucketTransportPhase::GoingToDestination => {
            phases::going_to_destination::handle(ctx, &data, commands, world_map);
        }
        BucketTransportPhase::Pouring { progress } => {
            phases::pouring::handle(ctx, &data, progress, commands, soul_handles, world_map);
        }
        BucketTransportPhase::ReturningBucket => {
            phases::returning_bucket::handle(ctx, commands, world_map);
        }
    }
}
