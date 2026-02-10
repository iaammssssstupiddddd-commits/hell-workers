//! 所持チェック・中断条件

use crate::systems::soul_ai::execute::task_execution::context::TaskExecutionContext;
use bevy::prelude::*;

pub fn has_bucket_in_inventory(ctx: &TaskExecutionContext, bucket_entity: Entity) -> bool {
    ctx.inventory.0 == Some(bucket_entity)
}

pub fn is_tank_full(ctx: &mut TaskExecutionContext, tank_entity: Entity) -> bool {
    let q_stockpiles = &mut ctx.queries.storage.stockpiles;
    if let Ok((_, _, stock, Some(stored))) = q_stockpiles.get(tank_entity) {
        stored.len() >= stock.capacity
    } else {
        false
    }
}
