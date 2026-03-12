//! バケツ搬送共通ガード

use hw_logistics::tank_has_capacity_for_full_bucket;
use crate::soul_ai::execute::task_execution::context::TaskExecutionContext;
use bevy::prelude::*;

pub fn has_bucket_in_inventory(ctx: &TaskExecutionContext, bucket_entity: Entity) -> bool {
    ctx.inventory.0 == Some(bucket_entity)
}

pub fn tank_can_accept_full_bucket(ctx: &mut TaskExecutionContext, tank_entity: Entity) -> bool {
    let q_stockpiles = &mut ctx.queries.storage.stockpiles;
    if let Ok((_, _, stock, Some(stored))) = q_stockpiles.get(tank_entity) {
        tank_has_capacity_for_full_bucket(stored.len(), stock.capacity)
    } else if let Ok((_, _, stock, None)) = q_stockpiles.get(tank_entity) {
        tank_has_capacity_for_full_bucket(0, stock.capacity)
    } else {
        false
    }
}
