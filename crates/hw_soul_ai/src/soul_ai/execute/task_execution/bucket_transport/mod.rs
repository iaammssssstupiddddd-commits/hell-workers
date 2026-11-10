pub mod abort;
pub(crate) mod claims;
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
    if ctx.water_claims.buckets.contains(&data.bucket) {
        // Another actor already owns this pass's conversion. Release only this
        // duplicate assignment; do not drop or rewrite the real holder's bucket.
        if ctx.inventory.0 == Some(data.bucket) {
            ctx.inventory.0 = None;
        }
        return ctx.abort_retryable_after_custom_cleanup(commands, "duplicate water bucket holder");
    }
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
