use crate::interface::ui::PlacementFailureTooltip;
use crate::systems::jobs::TaskSlots;
use crate::systems::jobs::wall_construction::{WallConstructionSite, WallTileBlueprint};
use crate::world::map::WorldMap;
use bevy::prelude::*;
use hw_core::constants::*;
use hw_core::visual_mirror::construction::{WallSiteVisualState, WallTileVisualMirror};
use hw_ui::selection::validate_wall_area;
use std::collections::HashSet;

use super::validation::validate_wall_tile;
use super::validation::validate_wall_tile_no_floor_check;

pub(super) fn apply_wall_placement(
    commands: &mut Commands,
    world_map: &mut WorldMap,
    area: &crate::systems::command::TaskArea,
    existing_floor_building_grids: &HashSet<(i32, i32)>,
    placement_failure_tooltip: &mut PlacementFailureTooltip,
    bypass_floor_check: bool,
) {
    let min_grid = WorldMap::world_to_grid(area.min() + Vec2::splat(0.1));
    let max_grid = WorldMap::world_to_grid(area.max() - Vec2::splat(0.1));

    let width = (max_grid.0 - min_grid.0 + 1).abs();
    let height = (max_grid.1 - min_grid.1 + 1).abs();

    if validate_wall_area(width, height).is_some() {
        let reason = if width > FLOOR_MAX_AREA_SIZE || height > FLOOR_MAX_AREA_SIZE {
            format!(
                "Wall placement area is too large: {}x{} (max {}x{})",
                width, height, FLOOR_MAX_AREA_SIZE, FLOOR_MAX_AREA_SIZE
            )
        } else {
            format!(
                "Wall must be placed as a straight 1xn line (selected {}x{})",
                width, height
            )
        };
        placement_failure_tooltip.show(reason.clone());
        warn!("Wall area rejected ({}x{}): {}", width, height, reason);
        return;
    }

    let mut valid_tiles = Vec::new();
    let mut first_reject_reason: Option<String> = None;
    for gy in min_grid.1..=max_grid.1 {
        for gx in min_grid.0..=max_grid.0 {
            let reject = if bypass_floor_check {
                validate_wall_tile_no_floor_check(gx, gy, world_map)
            } else {
                validate_wall_tile(gx, gy, world_map, existing_floor_building_grids)
            };
            if let Some(reason) = reject {
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
        warn!(
            "No valid tiles for wall placement in selected area: {}",
            reason
        );
        return;
    }

    placement_failure_tooltip.clear();

    let tiles_total = valid_tiles.len() as u32;
    let center_grid = valid_tiles[valid_tiles.len() / 2];
    let material_center = WorldMap::grid_to_world(center_grid.0, center_grid.1);

    let site_entity = commands
        .spawn((
            WallConstructionSite::new(area.clone(), material_center, tiles_total),
            WallSiteVisualState::default(),
            Transform::from_translation(material_center.extend(Z_MAP + 0.01)),
            Visibility::default(),
            Name::new("WallConstructionSite"),
        ))
        .id();

    for (gx, gy) in valid_tiles {
        let world_pos = WorldMap::grid_to_world(gx, gy);

        commands.spawn((
            WallTileBlueprint::new(site_entity, (gx, gy)),
            WallTileVisualMirror::default(),
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

        world_map.set_building_occupancy((gx, gy), site_entity);
    }
}
