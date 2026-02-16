//! 手押し車運搬タスクの各フェーズハンドラ

mod going_to_destination;
mod going_to_parking;
mod going_to_source;
mod loading;
mod picking_up_wheelbarrow;
mod returning_wheelbarrow;
mod unloading;

use crate::systems::logistics::Wheelbarrow;
use crate::systems::soul_ai::execute::task_execution::{
    context::TaskExecutionContext,
    types::{HaulWithWheelbarrowData, HaulWithWheelbarrowPhase},
};
use crate::world::map::WorldMap;
use bevy::prelude::*;

pub fn handle_haul_with_wheelbarrow_task(
    ctx: &mut TaskExecutionContext,
    data: HaulWithWheelbarrowData,
    commands: &mut Commands,
    world_map: &Res<WorldMap>,
    q_wheelbarrows: &Query<
        (&Transform, Option<&crate::relationships::ParkedAt>),
        With<Wheelbarrow>,
    >,
) {
    let soul_pos = ctx.soul_pos();

    match data.phase {
        HaulWithWheelbarrowPhase::GoingToParking => {
            going_to_parking::handle(ctx, data, commands, world_map, q_wheelbarrows, soul_pos);
        }
        HaulWithWheelbarrowPhase::PickingUpWheelbarrow => {
            picking_up_wheelbarrow::handle(ctx, data, commands);
        }
        HaulWithWheelbarrowPhase::GoingToSource => {
            going_to_source::handle(ctx, data, commands, world_map, soul_pos);
        }
        HaulWithWheelbarrowPhase::Loading => {
            loading::handle(ctx, data, commands);
        }
        HaulWithWheelbarrowPhase::GoingToDestination => {
            going_to_destination::handle(ctx, data, commands, world_map, soul_pos);
        }
        HaulWithWheelbarrowPhase::Unloading => {
            unloading::handle(ctx, data, commands, soul_pos);
        }
        HaulWithWheelbarrowPhase::ReturningWheelbarrow => {
            returning_wheelbarrow::handle(ctx, data, commands, world_map, q_wheelbarrows, soul_pos);
        }
    }
}
