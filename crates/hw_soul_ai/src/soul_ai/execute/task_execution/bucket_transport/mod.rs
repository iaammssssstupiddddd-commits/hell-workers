pub mod abort;
pub mod guards;
pub mod helpers;
pub mod phases;
pub mod routing;

use crate::soul_ai::execute::task_execution::context::{TaskExecutionContext, TaskHandlerControl};
use crate::soul_ai::execute::task_execution::types::{BucketTransportData, BucketTransportPhase};
use bevy::prelude::*;

/// バケツ輸送共通ハンドラ
pub fn handle_bucket_transport_task(
    ctx: &mut TaskExecutionContext,
    data: BucketTransportData,
    commands: &mut Commands,
) -> TaskHandlerControl {
    match data.phase {
        BucketTransportPhase::GoingToBucket => {
            phases::going_to_bucket::handle(ctx, &data, commands)
        }
        BucketTransportPhase::GoingToSource => {
            phases::going_to_source::handle(ctx, &data, commands)
        }
        BucketTransportPhase::Filling { progress } => {
            phases::filling::handle(ctx, &data, progress, commands)
        }
        BucketTransportPhase::GoingToDestination => {
            phases::going_to_destination::handle(ctx, &data, commands)
        }
        BucketTransportPhase::Pouring { progress } => {
            phases::pouring::handle(ctx, &data, progress, commands)
        }
        BucketTransportPhase::ReturningBucket => phases::returning_bucket::handle(ctx, commands),
    }
}
