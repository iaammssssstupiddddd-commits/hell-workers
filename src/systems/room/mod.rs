//! Room detection and visualization systems.

pub mod components;
pub mod detection;
pub mod dirty_mark;
pub mod resources;
pub mod validation;
pub mod visual;

pub use components::{Room, RoomBounds, RoomOverlayTile};
pub use detection::detect_rooms_system;
pub use dirty_mark::{
    mark_room_dirty_from_building_changes_system, mark_room_dirty_from_world_map_diff_system,
};
pub use resources::{RoomDetectionState, RoomTileLookup, RoomValidationState};
pub use validation::validate_rooms_system;
pub use visual::sync_room_overlay_tiles_system;
