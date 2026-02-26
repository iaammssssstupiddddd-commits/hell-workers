use crate::constants::*;
use crate::interface::ui::PlacementFailureTooltip;
use crate::systems::jobs::wall_construction::{WallConstructionSite, WallTileBlueprint};
use crate::systems::jobs::TaskSlots;
use crate::world::map::WorldMap;
use bevy::prelude::*;
use std::collections::HashSet;

use super::validation::validate_wall_tile;

pub(super) fn apply_wall_placement(
    commands: &mut Commands,
    world_map: &mut WorldMap,
    area: &crate::systems::command::TaskArea,
    existing_floor_building_grids: &HashSet<(i32, i32)>,
    placement_failure_tooltip: &mut PlacementFailureTooltip,
) {
    let min_grid = WorldMap::world_to_grid(area.min + Vec2::splat(0.1));
    let max_grid = WorldMap::world_to_grid(area.max - Vec2::splat(0.1));

    let width = (max_grid.0 - min_grid.0 + 1).abs();
    let height = (max_grid.1 - min_grid.1 + 1).abs();

    if width > FLOOR_MAX_AREA_SIZE || height > FLOOR_MAX_AREA_SIZE {
        let reason = format!(
            "Wall placement area is too large: {}x{} (max {}x{})",
            width, height, FLOOR_MAX_AREA_SIZE, FLOOR_MAX_AREA_SIZE
        );
        placement_failure_tooltip.show(reason.clone());
        warn!(
            "Wall area too large: {}x{} (max {}x{}): {}",
            width, height, FLOOR_MAX_AREA_SIZE, FLOOR_MAX_AREA_SIZE, reason
        );
        return;
    }

    if width < 1 || height < 1 || (width != 1 && height != 1) {
        let reason = format!(
            "Wall must be placed as a straight 1xn line (selected {}x{})",
            width, height
        );
        placement_failure_tooltip.show(reason.clone());
        warn!("Wall placement must be 1 x n, got {}x{}", width, height);
        return;
    }

    let mut valid_tiles = Vec::new();
    let mut first_reject_reason: Option<String> = None;
    for gy in min_grid.1..=max_grid.1 {
        for gx in min_grid.0..=max_grid.0 {
            if let Some(reason) = validate_wall_tile(gx, gy, world_map, existing_floor_building_grids) {
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
            .unwrap_or_else(|| "No valid wall tile in selected area".to_string());
        placement_failure_tooltip.show(reason.clone());
        warn!("No valid tiles for wall placement in selected area: {}", reason);
        return;
    }

    placement_failure_tooltip.clear();

    let tiles_total = valid_tiles.len() as u32;
    let center_grid = valid_tiles[valid_tiles.len() / 2];
    let material_center = WorldMap::grid_to_world(center_grid.0, center_grid.1);

    let site_entity = commands
        .spawn((
            WallConstructionSite::new(area.clone(), material_center, tiles_total),
            Transform::from_translation(material_center.extend(Z_MAP + 0.01)),
            Visibility::default(),
            Name::new("WallConstructionSite"),
        ))
        .id();

    for (gx, gy) in valid_tiles {
        let world_pos = WorldMap::grid_to_world(gx, gy);

        commands.spawn((
            WallTileBlueprint::new(site_entity, (gx, gy)),
            TaskSlots::new(1),
            Sprite {
                color: Color::srgba(0.8, 0.55, 0.3, 0.25),
                custom_size: Some(Vec2::splat(TILE_SIZE)),
                ..default()
            },
            Transform::from_translation(world_pos.extend(Z_MAP + 0.02)),
            Visibility::default(),
            Name::new(format!("WallTile({},{})", gx, gy)),
        ));

        world_map.buildings.insert((gx, gy), site_entity);
        world_map.add_obstacle(gx, gy);
    }
}
