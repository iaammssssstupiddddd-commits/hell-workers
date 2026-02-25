//! Floor construction drag-drop placement system

use crate::constants::*;
use crate::game_state::{PlayMode, TaskContext};
use crate::interface::camera::MainCamera;
use crate::interface::ui::{PlacementFailureTooltip, UiInputState};
use crate::systems::command::area_selection::wall_line_area;
use crate::systems::command::{TaskArea, TaskMode};
use crate::systems::jobs::floor_construction::{FloorConstructionSite, FloorTileBlueprint};
use crate::systems::jobs::wall_construction::{WallConstructionSite, WallTileBlueprint};
use crate::systems::jobs::{Building, BuildingType, TaskSlots};
use crate::world::map::WorldMap;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use std::collections::HashSet;

pub fn floor_placement_system(
    buttons: Res<ButtonInput<MouseButton>>,
    q_window: Query<&Window, With<PrimaryWindow>>,
    q_camera: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    ui_input_state: Res<UiInputState>,
    q_existing_floor_tiles: Query<&FloorTileBlueprint>,
    q_floor_buildings: Query<(&Building, &Transform)>,
    mut task_context: ResMut<TaskContext>,
    mut next_play_mode: ResMut<NextState<PlayMode>>,
    mut world_map: ResMut<WorldMap>,
    mut placement_failure_tooltip: ResMut<PlacementFailureTooltip>,
    mut commands: Commands,
) {
    if ui_input_state.pointer_over_ui {
        return;
    }

    let (is_floor_mode, start_pos_opt) = match task_context.0 {
        TaskMode::FloorPlace(start_pos_opt) => (true, start_pos_opt),
        TaskMode::WallPlace(start_pos_opt) => (false, start_pos_opt),
        _ => return,
    };

    let Some(world_pos) = world_cursor_pos(&q_window, &q_camera) else {
        return;
    };
    let snapped_pos = WorldMap::snap_to_grid_edge(world_pos);

    // Start drag
    if buttons.just_pressed(MouseButton::Left) {
        task_context.0 = if is_floor_mode {
            TaskMode::FloorPlace(Some(snapped_pos))
        } else {
            TaskMode::WallPlace(Some(snapped_pos))
        };
        return;
    }

    // Complete placement
    if buttons.just_released(MouseButton::Left) {
        if let Some(start_pos) = start_pos_opt {
            if is_floor_mode {
                let area = TaskArea::from_points(start_pos, snapped_pos);
                let existing_floor_tile_grids: HashSet<(i32, i32)> = q_existing_floor_tiles
                    .iter()
                    .map(|tile| tile.grid_pos)
                    .collect();
                let existing_floor_building_grids: HashSet<(i32, i32)> = q_floor_buildings
                    .iter()
                    .filter_map(|(building, transform)| {
                        (building.kind == BuildingType::Floor)
                            .then(|| WorldMap::world_to_grid(transform.translation.truncate()))
                    })
                    .collect();
                apply_floor_placement(
                    &mut commands,
                    &world_map,
                    &area,
                    &existing_floor_tile_grids,
                    &existing_floor_building_grids,
                    &mut placement_failure_tooltip,
                );
            } else {
                let area = wall_line_area(start_pos, snapped_pos);
                let existing_floor_building_grids: HashSet<(i32, i32)> = q_floor_buildings
                    .iter()
                    .filter_map(|(building, transform)| {
                        (building.kind == BuildingType::Floor)
                            .then(|| WorldMap::world_to_grid(transform.translation.truncate()))
                    })
                    .collect();
                apply_wall_placement(
                    &mut commands,
                    &mut world_map,
                    &area,
                    &existing_floor_building_grids,
                    &mut placement_failure_tooltip,
                );
            }

            // Reset mode (continue placing if shift held - TODO)
            task_context.0 = if is_floor_mode {
                TaskMode::FloorPlace(None)
            } else {
                TaskMode::WallPlace(None)
            };
        }
        return;
    }

    // Cancel (right click)
    if buttons.just_pressed(MouseButton::Right) {
        task_context.0 = TaskMode::None;
        next_play_mode.set(PlayMode::Normal);
    }
}

fn world_cursor_pos(
    q_window: &Query<&Window, With<PrimaryWindow>>,
    q_camera: &Query<(&Camera, &GlobalTransform), With<MainCamera>>,
) -> Option<Vec2> {
    let Ok((camera, camera_transform)) = q_camera.single() else {
        return None;
    };
    let Ok(window) = q_window.single() else {
        return None;
    };
    let cursor_pos: Vec2 = window.cursor_position()?;
    camera
        .viewport_to_world_2d(camera_transform, cursor_pos)
        .ok()
}

fn apply_floor_placement(
    commands: &mut Commands,
    world_map: &WorldMap,
    area: &TaskArea,
    existing_floor_tile_grids: &HashSet<(i32, i32)>,
    existing_floor_building_grids: &HashSet<(i32, i32)>,
    placement_failure_tooltip: &mut PlacementFailureTooltip,
) {
    let min_grid = WorldMap::world_to_grid(area.min + Vec2::splat(0.1));
    let max_grid = WorldMap::world_to_grid(area.max - Vec2::splat(0.1));

    // Validate area size
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

    // Collect valid tiles
    let mut valid_tiles = Vec::new();
    let mut first_reject_reason: Option<String> = None;
    for gy in min_grid.1..=max_grid.1 {
        for gx in min_grid.0..=max_grid.0 {
            // Check if walkable and no existing buildings/stockpiles
            if !world_map.is_walkable(gx, gy) {
                if first_reject_reason.is_none() {
                    first_reject_reason = Some(format!(
                        "Tile ({},{}) is not walkable",
                        gx, gy
                    ));
                }
                continue;
            }
            if world_map.buildings.contains_key(&(gx, gy)) {
                if first_reject_reason.is_none() {
                    first_reject_reason = Some(format!(
                        "Tile ({},{}) is already occupied by a building",
                        gx, gy
                    ));
                }
                continue;
            }
            if world_map.stockpiles.contains_key(&(gx, gy)) {
                if first_reject_reason.is_none() {
                    first_reject_reason = Some(format!(
                        "Tile ({},{}) is already occupied by a stockpile",
                        gx, gy
                    ));
                }
                continue;
            }
            if existing_floor_tile_grids.contains(&(gx, gy)) {
                if first_reject_reason.is_none() {
                    first_reject_reason = Some(format!(
                        "Tile ({},{}) already has a floor blueprint",
                        gx, gy
                    ));
                }
                continue;
            }
            if existing_floor_building_grids.contains(&(gx, gy)) {
                if first_reject_reason.is_none() {
                    first_reject_reason = Some(format!(
                        "Tile ({},{}) already has a completed floor",
                        gx, gy
                    ));
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

    // Create parent FloorConstructionSite entity
    let site_entity = commands
        .spawn((
            FloorConstructionSite::new(area.clone(), material_center, tiles_total),
            Transform::from_translation(material_center.extend(Z_MAP + 0.01)),
            Visibility::default(),
            Name::new("FloorConstructionSite"),
        ))
        .id();

    // Spawn FloorTileBlueprint children
    for (gx, gy) in valid_tiles {
        let world_pos = WorldMap::grid_to_world(gx, gy);

        commands.spawn((
            FloorTileBlueprint::new(site_entity, (gx, gy)),
            TaskSlots::new(1), // One worker per tile
            Sprite {
                color: Color::srgba(0.5, 0.5, 0.8, 0.2), // Light blue overlay
                custom_size: Some(Vec2::splat(TILE_SIZE)),
                ..default()
            },
            Transform::from_translation(world_pos.extend(Z_MAP + 0.02)),
            Visibility::default(),
            Name::new(format!("FloorTile({},{})", gx, gy)),
        ));
    }
}

fn apply_wall_placement(
    commands: &mut Commands,
    world_map: &mut WorldMap,
    area: &TaskArea,
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
            if !world_map.is_walkable(gx, gy) {
                if first_reject_reason.is_none() {
                    first_reject_reason = Some(format!(
                        "Tile ({},{}) is not walkable",
                        gx, gy
                    ));
                }
                continue;
            }
            if world_map.buildings.contains_key(&(gx, gy)) {
                if first_reject_reason.is_none() {
                    first_reject_reason = Some(format!(
                        "Tile ({},{}) is already occupied by a building",
                        gx, gy
                    ));
                }
                continue;
            }
            if world_map.stockpiles.contains_key(&(gx, gy)) {
                if first_reject_reason.is_none() {
                    first_reject_reason = Some(format!(
                        "Tile ({},{}) is already occupied by a stockpile",
                        gx, gy
                    ));
                }
                continue;
            }
            if !existing_floor_building_grids.contains(&(gx, gy)) {
                if first_reject_reason.is_none() {
                    first_reject_reason = Some(format!(
                        "Tile ({},{}) has no completed floor",
                        gx, gy
                    ));
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
