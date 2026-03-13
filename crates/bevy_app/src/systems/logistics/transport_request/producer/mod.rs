pub mod floor_construction {
    pub use hw_logistics::transport_request::producer::floor_construction::{
        floor_construction_auto_haul_system, floor_material_delivery_sync_system,
        floor_tile_designation_system,
    };
}
pub mod wall_construction {
    pub use hw_logistics::transport_request::producer::wall_construction::{
        wall_construction_auto_haul_system, wall_material_delivery_sync_system,
        wall_tile_designation_system,
    };
}
