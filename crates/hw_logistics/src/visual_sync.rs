use bevy::ecs::lifecycle::Add;
use bevy::prelude::*;

use hw_core::visual_mirror::StockpileVisualState;
use hw_core::visual_mirror::logistics::{InventoryItemVisual, WheelbarrowMarker};

use crate::types::{Inventory, ResourceItem, Wheelbarrow};
use crate::zone::Stockpile;

pub fn on_wheelbarrow_added(on: On<Add, Wheelbarrow>, mut commands: Commands) {
    commands.entity(on.entity).try_insert(WheelbarrowMarker);
}

pub fn sync_inventory_item_visual_system(
    mut q: Query<
        (&Inventory, &mut InventoryItemVisual),
        Or<(Changed<Inventory>, Added<Inventory>)>,
    >,
    q_items: Query<&ResourceItem>,
) {
    for (inventory, mut visual) in q.iter_mut() {
        visual.resource_type = inventory
            .0
            .and_then(|item_entity| q_items.get(item_entity).ok())
            .map(|item| item.0);
    }
}

// ── Stockpile Visual Sync ─────────────────────────────────────────────────────

/// Inserts `StockpileVisualState` when a `Stockpile` component is added.
pub fn on_stockpile_added_sync_visual(
    on: On<Add, Stockpile>,
    mut commands: Commands,
    q: Query<&Stockpile>,
) {
    if let Ok(stockpile) = q.get(on.entity) {
        commands.entity(on.entity).try_insert(StockpileVisualState {
            capacity: stockpile.capacity,
        });
    }
}

/// Updates `StockpileVisualState` whenever `Stockpile` changes.
pub fn sync_stockpile_visual_system(
    mut q: Query<(&Stockpile, &mut StockpileVisualState), Changed<Stockpile>>,
) {
    for (stockpile, mut state) in q.iter_mut() {
        state.capacity = stockpile.capacity;
    }
}
