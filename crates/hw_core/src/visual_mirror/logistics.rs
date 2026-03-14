use bevy::prelude::*;

use crate::logistics::ResourceType;

/// Marker attached to `Wheelbarrow` entities so `hw_visual` can filter without
/// importing `hw_logistics`. Attached by an `OnAdd<Wheelbarrow>` Observer in `hw_logistics`.
#[derive(Component)]
pub struct WheelbarrowMarker;

/// Mirror of a Soul entity's carried item for `hw_visual`.
/// Synced every time `Inventory` changes via `sync_inventory_item_visual_system` in `hw_logistics`.
///
/// **Note**: `hw_visual/src/haul/components.rs` already contains a local component also called
/// `CarryingItemVisual` (tracking the visual icon entity). This type is named differently to
/// avoid ambiguity.
#[derive(Component, Default)]
pub struct InventoryItemVisual {
    /// `None` = carrying nothing.
    pub resource_type: Option<ResourceType>,
}
