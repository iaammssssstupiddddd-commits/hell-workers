use bevy::ecs::lifecycle::Add;
use bevy::prelude::*;

use hw_core::visual_mirror::logistics::{InventoryItemVisual, WheelbarrowMarker};

use crate::types::{Inventory, ResourceItem, Wheelbarrow};

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
