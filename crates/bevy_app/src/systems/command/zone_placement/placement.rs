use crate::app_contexts::TaskContext;
use crate::interface::ui::UiInputState;
use crate::systems::command::TaskMode;
use crate::systems::command::TaskModeZoneType;
use crate::systems::logistics::{BelongsTo, Stockpile};
use crate::world::map::{WorldMap, WorldMapWrite};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use hw_core::constants::*;
use hw_core::game_state::PlayMode;
use hw_ui::camera::MainCamera;
use hw_world::zones::Site;
use hw_world::zones::{AreaBounds, Yard};
use hw_world::{area_tile_size, expand_yard_area, rectangles_overlap, rectangles_overlap_site};

#[allow(clippy::too_many_arguments)]
pub fn zone_placement_system(
    buttons: Res<ButtonInput<MouseButton>>,
    q_window: Query<&Window, With<PrimaryWindow>>,
    q_camera: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    ui_input_state: Res<UiInputState>,
    mut task_context: ResMut<TaskContext>,
    mut next_play_mode: ResMut<NextState<PlayMode>>,
    mut world_map: WorldMapWrite,
    mut commands: Commands,
    q_yards: Query<(Entity, &Yard)>,
    q_sites: Query<&Site>,
) {
    if ui_input_state.pointer_over_ui {
        return;
    }

    let TaskMode::ZonePlacement(zone_type, start_pos_opt) = task_context.0 else {
        return;
    };

    let Some(world_pos) = super::world_cursor_pos(&q_window, &q_camera) else {
        return;
    };
    let snapped_pos = WorldMap::snap_to_grid_edge(world_pos);

    // 開始
    if buttons.just_pressed(MouseButton::Left) {
        task_context.0 = TaskMode::ZonePlacement(zone_type, Some(snapped_pos));
        return;
    }

    // 確定
    if buttons.just_released(MouseButton::Left) {
        if let Some(start_pos) = start_pos_opt {
            let area = AreaBounds::from_points(start_pos, snapped_pos);
            if matches!(zone_type, TaskModeZoneType::Stockpile)
                && !is_stockpile_area_within_yards(&area, &q_yards)
            {
                return;
            }
            if matches!(zone_type, TaskModeZoneType::Yard)
                && !is_yard_expansion_area_valid(start_pos, &area, &q_sites, &q_yards)
            {
                return;
            }
            if matches!(zone_type, TaskModeZoneType::Yard) {
                apply_yard_expansion(&mut commands, start_pos, &area, &q_sites, &q_yards);
            } else {
                apply_zone_placement(&mut commands, &mut world_map, zone_type, &area, &q_yards);
            }

            // Shift押下で継続、そうでなければ解除
            // FIXME: keyboard リソースが必要だが、一旦シンプルに解除
            task_context.0 = TaskMode::ZonePlacement(zone_type, None);
        }
        return;
    }

    // キャンセル (右クリック)
    if buttons.just_pressed(MouseButton::Right) {
        task_context.0 = TaskMode::None;
        next_play_mode.set(PlayMode::Normal);
    }
}

fn apply_zone_placement(
    commands: &mut Commands,
    world_map: &mut WorldMap,
    zone_type: TaskModeZoneType,
    area: &AreaBounds,
    q_yards: &Query<(Entity, &Yard)>,
) {
    let min_grid = WorldMap::world_to_grid(area.min + Vec2::splat(0.1));
    let max_grid = WorldMap::world_to_grid(area.max - Vec2::splat(0.1));

    for gy in min_grid.1..=max_grid.1 {
        for gx in min_grid.0..=max_grid.0 {
            let grid = (gx, gy);
            let grid_pos = WorldMap::grid_to_world(gx, gy);
            let Some(yard_entity) = pick_stockpile_owner_yard(grid_pos, q_yards) else {
                continue;
            };

            // 既に存在するか、建築物がある場合はスキップ
            if world_map.has_stockpile(grid) || world_map.has_building(grid) {
                continue;
            }
            // 通行不能な場所もスキップ
            if !world_map.is_walkable(gx, gy) {
                continue;
            }

            match zone_type {
                TaskModeZoneType::Stockpile => {
                    let entity = commands
                        .spawn((
                            Stockpile {
                                capacity: 10,
                                resource_type: None,
                            },
                            BelongsTo(yard_entity),
                            Sprite {
                                color: Color::srgba(1.0, 1.0, 0.0, 0.2),
                                custom_size: Some(Vec2::splat(TILE_SIZE)),
                                ..default()
                            },
                            Transform::from_xyz(grid_pos.x, grid_pos.y, Z_MAP + 0.01),
                            Name::new("Stockpile"),
                        ))
                        .id();
                    world_map.register_stockpile_tile(grid, entity);
                }
                TaskModeZoneType::Yard => {}
            }
        }
    }
}

fn apply_yard_expansion(
    commands: &mut Commands,
    start_pos: Vec2,
    area: &AreaBounds,
    q_sites: &Query<&Site>,
    q_yards: &Query<(Entity, &Yard)>,
) {
    let Some((yard_entity, source_yard)) = pick_yard_for_position(start_pos, q_yards) else {
        return;
    };
    let expanded_area = expand_yard_area(&source_yard, area);
    if !is_yard_expansion_area_valid(start_pos, area, q_sites, q_yards) {
        return;
    }
    commands.entity(yard_entity).insert(Yard {
        min: expanded_area.min,
        max: expanded_area.max,
    });
}

pub(crate) fn is_stockpile_area_within_yards(
    area: &AreaBounds,
    q_yards: &Query<(Entity, &Yard)>,
) -> bool {
    let min_grid = WorldMap::world_to_grid(area.min + Vec2::splat(0.1));
    let max_grid = WorldMap::world_to_grid(area.max - Vec2::splat(0.1));

    for gy in min_grid.1..=max_grid.1 {
        for gx in min_grid.0..=max_grid.0 {
            let grid_pos = WorldMap::grid_to_world(gx, gy);
            if q_yards.iter().all(|(_, yard)| !yard.contains(grid_pos)) {
                return false;
            }
        }
    }
    true
}

pub(crate) fn is_yard_expansion_area_valid(
    start_pos: Vec2,
    drag_area: &AreaBounds,
    q_sites: &Query<&Site>,
    q_yards: &Query<(Entity, &Yard)>,
) -> bool {
    let Some((source_entity, source_yard)) = pick_yard_for_position(start_pos, q_yards) else {
        return false;
    };
    let expanded_area = expand_yard_area(&source_yard, drag_area);
    let expanded_tiles = area_tile_size(&expanded_area);

    if expanded_tiles.0 < YARD_MIN_WIDTH_TILES as usize
        || expanded_tiles.1 < YARD_MIN_HEIGHT_TILES as usize
    {
        return false;
    }

    let overlaps_site = q_sites
        .iter()
        .any(|site| rectangles_overlap_site(&expanded_area, site));
    if overlaps_site {
        return false;
    }

    let overlaps_other_yard = q_yards
        .iter()
        .any(|(entity, yard)| entity != source_entity && rectangles_overlap(&expanded_area, yard));
    if overlaps_other_yard {
        return false;
    }

    true
}

fn pick_yard_for_position(
    position: Vec2,
    q_yards: &Query<(Entity, &Yard)>,
) -> Option<(Entity, Yard)> {
    q_yards
        .iter()
        .find(|(_, yard)| yard.contains(position))
        .map(|(entity, yard)| (entity, yard.clone()))
}

fn pick_stockpile_owner_yard(grid_pos: Vec2, q_yards: &Query<(Entity, &Yard)>) -> Option<Entity> {
    if let Some((owner, _)) = q_yards.iter().find(|(_, yard)| yard.contains(grid_pos)) {
        return Some(owner);
    }
    None
}
