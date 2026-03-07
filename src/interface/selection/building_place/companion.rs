use hw_core::constants::TILE_SIZE;
use crate::app_contexts::{CompanionParentKind, CompanionPlacement, CompanionPlacementKind};
use crate::systems::jobs::BuildingType;
use bevy::prelude::*;

const COMPANION_PLACEMENT_RADIUS_TILES: f32 = 5.0;

pub(super) fn make_companion_placement(
    parent_kind: CompanionParentKind,
    parent_anchor: (i32, i32),
    kind: CompanionPlacementKind,
    center: Vec2,
) -> CompanionPlacement {
    CompanionPlacement {
        parent_kind,
        parent_anchor,
        kind,
        center,
        radius: TILE_SIZE * COMPANION_PLACEMENT_RADIUS_TILES,
        required: true,
    }
}

pub(super) fn parent_building_type(parent_kind: CompanionParentKind) -> BuildingType {
    match parent_kind {
        CompanionParentKind::Tank => BuildingType::Tank,
    }
}
