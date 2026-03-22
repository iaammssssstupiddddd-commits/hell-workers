pub mod construction_helpers;
pub mod floor_construction;
pub mod ground_resources;
pub mod item_lifetime;
pub mod manual_haul_selector;
pub mod plugin;
pub mod provisional_wall;
pub mod resource_cache;
pub mod spatial_sync;
pub mod tile_index;
pub mod transport_request;
pub mod types;
pub mod visual_sync;
pub mod wall_construction;
pub mod water;
pub mod zone;

pub use plugin::LogisticsPlugin;
pub use resource_cache::SharedResourceCache;
pub use resource_cache::{apply_reservation_op, apply_reservation_requests_system};

pub use construction_helpers::{ResourceItemVisualHandles, spawn_refund_items};

// Convenience re-exports for task_execution handlers
pub use hw_core::logistics::ResourceType;
pub use spatial_sync::{
    update_resource_spatial_grid_system_resource_item,
    update_stockpile_spatial_grid_system_stockpile,
    update_transport_request_spatial_grid_system_transport_request,
};
pub use types::{BelongsTo, BucketStorage, Inventory, ReservedForTask, ResourceItem, Wheelbarrow};
pub use water::tank_has_capacity_for_full_bucket;
pub use zone::Stockpile;
// Functions used in dropping / unloading handlers
pub use floor_construction::{floor_site_tile_demand, floor_site_tile_demand_from_index};
pub use ground_resources::count_nearby_ground_resources;
pub use provisional_wall::provisional_wall_mud_demand;
pub use wall_construction::{wall_site_tile_demand, wall_site_tile_demand_from_index};
