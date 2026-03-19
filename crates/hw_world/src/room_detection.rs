//! Room detection facade.
//!
//! This module contains no ECS system logic.
//! ECS system logic (query adapter layer) is in [`crate::room_systems`].

mod core;
mod ecs;
#[cfg(test)]
mod tests;

pub use self::core::{
    build_detection_input, detect_rooms, room_is_valid_against_input, DetectedRoom,
    RoomBounds, RoomDetectionBuildingTile, RoomDetectionInput,
};
pub use self::ecs::{
    Room, RoomDetectionState, RoomOverlayTile, RoomTileLookup, RoomValidationState,
};
