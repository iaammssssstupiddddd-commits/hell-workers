use crate::constants::TILE_SIZE;
use crate::game_state::{CompanionParentKind, CompanionPlacement, CompanionPlacementKind};
use crate::systems::jobs::{Blueprint, BuildingType, SandPile};
use crate::world::map::WorldMap;
use bevy::prelude::*;

use super::geometry::grid_is_nearby;

const COMPANION_PLACEMENT_RADIUS_TILES: f32 = 5.0;
const MUD_MIXER_NEARBY_SANDPILE_TILES: i32 = 3;

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
        CompanionParentKind::MudMixer => BuildingType::MudMixer,
    }
}

pub(super) fn has_nearby_sandpile(
    mixer_occupied_grids: &[(i32, i32)],
    q_sand_piles: &Query<&Transform, With<SandPile>>,
    q_blueprints: &Query<(Entity, &Blueprint, &Transform)>,
    ignore_blueprint: Option<Entity>,
) -> bool {
    if q_sand_piles.iter().any(|transform| {
        let sand_grid = WorldMap::world_to_grid(transform.translation.truncate());
        mixer_occupied_grids
            .iter()
            .any(|&(mx, my)| grid_is_nearby((mx, my), sand_grid, MUD_MIXER_NEARBY_SANDPILE_TILES))
    }) {
        return true;
    }

    q_blueprints.iter().any(|(entity, bp, transform)| {
        if Some(entity) == ignore_blueprint {
            return false;
        }
        if bp.kind != BuildingType::SandPile {
            return false;
        }
        let sand_grid = WorldMap::world_to_grid(transform.translation.truncate());
        mixer_occupied_grids
            .iter()
            .any(|&(mx, my)| grid_is_nearby((mx, my), sand_grid, MUD_MIXER_NEARBY_SANDPILE_TILES))
    })
}
