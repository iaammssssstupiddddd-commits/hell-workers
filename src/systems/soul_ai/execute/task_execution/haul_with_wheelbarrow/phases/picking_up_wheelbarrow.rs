//! 手押し車を取得するフェーズ

use crate::relationships::{ParkedAt, PushedBy};
use crate::systems::soul_ai::execute::task_execution::{
    context::TaskExecutionContext,
    types::{AssignedTask, HaulWithWheelbarrowData, HaulWithWheelbarrowPhase},
};
use bevy::prelude::*;

pub fn handle(
    ctx: &mut TaskExecutionContext,
    data: HaulWithWheelbarrowData,
    commands: &mut Commands,
) {
    commands.entity(data.wheelbarrow).remove::<ParkedAt>();
    commands
        .entity(data.wheelbarrow)
        .insert(PushedBy(ctx.soul_entity));
    commands
        .entity(data.wheelbarrow)
        .insert(Visibility::Visible);
    ctx.inventory.0 = Some(data.wheelbarrow);

    info!(
        "WB_HAUL: Soul {:?} picked up wheelbarrow {:?}",
        ctx.soul_entity, data.wheelbarrow
    );

    let next_phase = if data.items.is_empty() && data.collect_source.is_none() {
        HaulWithWheelbarrowPhase::ReturningWheelbarrow
    } else {
        HaulWithWheelbarrowPhase::GoingToSource
    };

    *ctx.task = AssignedTask::HaulWithWheelbarrow(HaulWithWheelbarrowData {
        phase: next_phase,
        ..data
    });
}
