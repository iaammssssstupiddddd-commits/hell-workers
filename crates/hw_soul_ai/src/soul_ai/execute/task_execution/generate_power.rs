//! GeneratePower タスク実行ハンドラ
//!
//! Soul を SoulSpaTile に移動させ、発電（Dream 消費）を行う。

use crate::soul_ai::execute::task_execution::{
    common::*,
    context::TaskExecutionContext,
    types::{AssignedTask, GeneratePowerData, GeneratePowerPhase},
};
use bevy::prelude::*;
use hw_core::relationships::WorkingOn;
use hw_energy::constants::{
    DREAM_CONSUME_RATE_GENERATING, DREAM_GENERATE_FLOOR, FATIGUE_RATE_GENERATING,
};

pub fn handle_generate_power_task(
    ctx: &mut TaskExecutionContext,
    data: GeneratePowerData,
    commands: &mut Commands,
) {
    let soul_pos = ctx.soul_pos();
    let tile_entity = data.tile;

    match data.phase {
        GeneratePowerPhase::GoingToTile => {
            if ctx
                .queries
                .designation
                .designations
                .get(tile_entity)
                .is_err()
            {
                debug!(
                    "GENERATE_POWER: Soul {:?} - tile {:?} lost Designation, canceling",
                    ctx.soul_entity, tile_entity
                );
                ctx.abort_closed(commands, "generate power designation missing");
                return;
            }

            let tile_pos = data.tile_pos;

            let reachable = update_destination_to_adjacent(
                ctx.dest,
                tile_pos,
                ctx.path,
                soul_pos,
                ctx.env.world_map,
                ctx.pf_context,
            );

            if !reachable {
                debug!(
                    "GENERATE_POWER: Soul {:?} cannot reach tile {:?}, canceling",
                    ctx.soul_entity, tile_entity
                );
                ctx.queue_reservation(hw_core::events::ResourceReservationOp::ReleaseSource {
                    source: tile_entity,
                    amount: 1,
                });
                ctx.abort_retryable(commands, "generate power tile unreachable");
                return;
            }

            if is_near_target(soul_pos, tile_pos) {
                commands
                    .entity(ctx.soul_entity)
                    .insert(WorkingOn(tile_entity));
                ctx.path.waypoints.clear();
                *ctx.task = AssignedTask::GeneratePower(GeneratePowerData {
                    tile: tile_entity,
                    tile_pos,
                    phase: GeneratePowerPhase::Generating,
                });
                debug!(
                    "GENERATE_POWER: Soul {:?} started generating at tile {:?}",
                    ctx.soul_entity, tile_entity
                );
            }
        }

        GeneratePowerPhase::Generating => {
            if ctx
                .queries
                .designation
                .designations
                .get(tile_entity)
                .is_err()
            {
                debug!(
                    "GENERATE_POWER: Soul {:?} - tile {:?} lost Designation, stopping",
                    ctx.soul_entity, tile_entity
                );
                ctx.abort_closed(commands, "generate power designation removed");
                return;
            }

            if ctx.soul.dream < DREAM_GENERATE_FLOOR {
                debug!(
                    "GENERATE_POWER: Soul {:?} ran out of Dream ({:.1}), stopping",
                    ctx.soul_entity, ctx.soul.dream
                );
                ctx.queue_reservation(hw_core::events::ResourceReservationOp::ReleaseSource {
                    source: tile_entity,
                    amount: 1,
                });
                ctx.abort_retryable(commands, "generate power dream depleted");
                return;
            }

            let dt = ctx.env.time.delta_secs();

            ctx.soul.dream = (ctx.soul.dream - DREAM_CONSUME_RATE_GENERATING * dt).max(0.0);
            ctx.soul.fatigue = (ctx.soul.fatigue + FATIGUE_RATE_GENERATING * dt).min(1.0);
        }
    }
}
