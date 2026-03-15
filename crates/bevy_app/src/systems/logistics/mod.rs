mod initial_spawn;
pub mod transport_request;
mod ui;

pub use hw_logistics::floor_construction::{
    floor_site_tile_demand, floor_site_tile_demand_from_index,
};
pub use hw_logistics::ground_resources::count_nearby_ground_resources;
pub use hw_logistics::provisional_wall::provisional_wall_mud_demand;
pub use hw_logistics::tile_index::{
    TileSiteIndex, sync_floor_tile_site_index_system, sync_removed_floor_tile_site_index_system,
    sync_removed_wall_tile_site_index_system, sync_wall_tile_site_index_system,
};
pub use hw_logistics::types::{
    BelongsTo, BucketStorage, Inventory, PendingBelongsToBlueprint, ReservedForTask, ResourceItem,
    ResourceType, Wheelbarrow, WheelbarrowParking,
};
pub use hw_logistics::wall_construction::{
    wall_site_tile_demand, wall_site_tile_demand_from_index,
};
pub use hw_logistics::water::{
    projected_tank_water, tank_can_accept_new_bucket, tank_has_capacity_for_full_bucket,
};
pub use hw_logistics::zone::{Stockpile, ZoneType};

pub use initial_spawn::initial_resource_spawner;
pub use ui::{
    ResourceCountDisplayTimer, ResourceCountLabel, ResourceLabels, resource_count_display_system,
};

// item_lifetime は他モジュールからパス指定で参照されるため pub mod として公開
pub mod item_lifetime {
    pub use hw_logistics::item_lifetime::*;
}
