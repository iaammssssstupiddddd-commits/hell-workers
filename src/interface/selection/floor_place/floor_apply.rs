use crate::constants::*;
use crate::interface::ui::PlacementFailureTooltip;
use crate::systems::jobs::floor_construction::{FloorConstructionSite, FloorTileBlueprint};
use crate::systems::jobs::TaskSlots;
use crate::world::map::WorldMap;
use bevy::prelude::*;
use std::collections::HashSet;

use super::validation::validate_floor_tile;

pub(super) fn apply_floor_placement(
    commands: &mut Commands,
    world_map: &WorldMap,
    area: &crate::systems::command::TaskArea,
    existing_floor_tile_grids: &HashSet<(i32, i32)>,
    existing_floor_building_grids: &HashSet<(i32, i32)>,
    placement_failure_tooltip: &mut PlacementFailureTooltip,
) {
    let min_grid = WorldMap::world_to_grid(area.min + Vec2::splat(0.1));
    let max_grid = WorldMap::world_to_grid(area.max - Vec2::splat(0.1));

    let width = (max_grid.0 - min_grid.0 + 1).abs();
    let height = (max_grid.1 - min_grid.1 + 1).abs();

    if width > FLOOR_MAX_AREA_SIZE || height > FLOOR_MAX_AREA_SIZE {
        let reason = format!(
            "Floor placement area is too large: {}x{} (max {}x{})",
            width, height, FLOOR_MAX_AREA_SIZE, FLOOR_MAX_AREA_SIZE
        );
        placement_failure_tooltip.show(reason.clone());
        warn!(
            "Floor area too large: {}x{} (max {}x{})",
            width, height, FLOOR_MAX_AREA_SIZE, FLOOR_MAX_AREA_SIZE
        );
        return;
    }

    let mut valid_tiles = Vec::new();
    let mut first_reject_reason: Option<String> = None;
    for gy in min_grid.1..=max_grid.1 {
        for gx in min_grid.0..=max_grid.0 {
            if let Some(reason) = validate_floor_tile(
                gx,
                gy,
                world_map,
                existing_floor_tile_grids,
                existing_floor_building_grids,
            ) {
                if first_reject_reason.is_none() {
                    first_reject_reason = Some(reason.message(gx, gy));
                }
                continue;
            }
            valid_tiles.push((gx, gy));
        }
    }

    if valid_tiles.is_empty() {
        let reason = first_reject_reason
            .unwrap_or_else(|| "No valid floor tile in selected area".to_string());
        placement_failure_tooltip.show(reason.clone());
        warn!("No valid tiles for floor placement in selected area: {}", reason);
        return;
    }

    placement_failure_tooltip.clear();

    let tiles_total = valid_tiles.len() as u32;
    let center_grid = valid_tiles[valid_tiles.len() / 2];
    let material_center = WorldMap::grid_to_world(center_grid.0, center_grid.1);

    let site_entity = commands
        .spawn((
            FloorConstructionSite::new(area.clone(), material_center, tiles_total),
            Transform::from_translation(material_center.extend(Z_MAP + 0.01)),
            Visibility::default(),
            Name::new("FloorConstructionSite"),
        ))
        .id();

    for (gx, gy) in valid_tiles {
        let world_pos = WorldMap::grid_to_world(gx, gy);

        commands.spawn((
            FloorTileBlueprint::new(site_entity, (gx, gy)),
            TaskSlots::new(1),
            Sprite {
                color: Color::srgba(0.5, 0.5, 0.8, 0.2),
                custom_size: Some(Vec2::splat(TILE_SIZE)),
                ..default()
            },
            Transform::from_translation(world_pos.extend(Z_MAP + 0.02)),
            Visibility::default(),
            Name::new(format!("FloorTile({},{})", gx, gy)),
        ));
    }
}
