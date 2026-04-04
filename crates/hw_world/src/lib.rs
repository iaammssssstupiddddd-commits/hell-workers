pub mod anchor;
pub mod coords;
pub mod door_systems;
pub mod layout;
pub mod map;
pub mod mapgen;
pub mod pathfinding;
pub mod query;
pub mod regrowth;
pub mod river;
pub mod rock_fields;
pub mod room_detection;
pub mod room_systems;
pub mod spatial;
pub mod spawn;
pub mod terrain;
pub mod terrain_visual;
pub mod terrain_zones;
pub mod tree_planting;
pub mod world_masks;
pub mod zone_ops;
pub mod zones;
pub use anchor::{AnchorLayout, AnchorLayoutError, GridRect};
pub use coords::{
    grid_to_world, idx_to_pos, snap_to_grid_center, snap_to_grid_edge, world_to_grid,
};
pub use door_systems::{
    DoorVisualHandles, apply_door_state, door_auto_close_system, door_auto_open_system,
};
pub use layout::{RIVER_X_MAX, RIVER_X_MIN, RIVER_Y_MAX, RIVER_Y_MIN, SAND_WIDTH};
pub use map::{WorldMap, WorldMapRead, WorldMapWrite};
pub use mapgen::generate_base_terrain_tiles;
pub use mapgen::generate_world_layout;
pub use mapgen::types::{GeneratedWorldLayout, ResourceSpawnCandidates, WfcForestZone};
pub use pathfinding::{
    PathGoalPolicy, PathNode, PathWorld, PathfindingContext, can_reach_target, find_path,
    find_path_to_adjacent, find_path_to_boundary, find_path_world_waypoints,
};
pub use query::{find_nearest_river_grid, find_nearest_walkable_grid};
pub use regrowth::{ForestZone, default_forest_zones, find_regrowth_position};
pub use river::{generate_fixed_river_tiles, generate_sand_tiles};
pub use room_detection::{
    DetectedRoom, Room, RoomBounds, RoomDetectionBuildingTile, RoomDetectionInput,
    RoomDetectionState, RoomOverlayTile, RoomTileLookup, RoomValidationState,
    build_detection_input, detect_rooms, room_is_valid_against_input,
};
pub use room_systems::{
    detect_rooms_system, mark_room_dirty_from_building_changes_system, on_building_added,
    on_building_removed, on_door_added, on_door_removed, sync_room_overlay_tiles_system,
    validate_rooms_system,
};
pub use spatial::SpatialGridOps;
pub use spawn::{find_nearby_walkable_grid, pick_random_walkable_grid_in_rect};
pub use terrain::TerrainType;
pub use terrain_visual::{TerrainChangedEvent, TerrainVisualHandles, obstacle_cleanup_system};
pub use tree_planting::DreamTreePlantingPlan;
pub use world_masks::{BitGrid, WorldMasks};
pub use zone_ops::{
    area_tile_size, expand_yard_area, identify_removal_targets, rectangles_overlap,
    rectangles_overlap_site,
};
pub use zones::{PairedSite, PairedYard, Site, Yard};
