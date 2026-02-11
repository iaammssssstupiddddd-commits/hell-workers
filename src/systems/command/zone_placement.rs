use crate::constants::*;
use crate::game_state::{PlayMode, TaskContext};
use crate::interface::camera::MainCamera;
use crate::interface::ui::UiInputState;
use crate::systems::command::{TaskArea, TaskMode};
use crate::systems::logistics::{Stockpile, ZoneType};
use crate::world::map::WorldMap;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

pub fn zone_placement_system(
    buttons: Res<ButtonInput<MouseButton>>,
    q_window: Query<&Window, With<PrimaryWindow>>,
    q_camera: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    ui_input_state: Res<UiInputState>,
    mut task_context: ResMut<TaskContext>,
    mut next_play_mode: ResMut<NextState<PlayMode>>,
    mut world_map: ResMut<WorldMap>,
    mut commands: Commands,
) {
    if ui_input_state.pointer_over_ui {
        return;
    }

    let TaskMode::ZonePlacement(zone_type, start_pos_opt) = task_context.0 else {
        return;
    };

    let Some(world_pos) = world_cursor_pos(&q_window, &q_camera) else {
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
            let area = TaskArea::from_points(start_pos, snapped_pos);
            apply_zone_placement(&mut commands, &mut world_map, zone_type, &area);

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

fn apply_zone_placement(
    commands: &mut Commands,
    world_map: &mut WorldMap,
    zone_type: ZoneType,
    area: &TaskArea,
) {
    let min_grid = WorldMap::world_to_grid(area.min + Vec2::splat(0.1));
    let max_grid = WorldMap::world_to_grid(area.max - Vec2::splat(0.1));

    for gy in min_grid.1..=max_grid.1 {
        for gx in min_grid.0..=max_grid.0 {
            let grid = (gx, gy);
            
            // 既に存在するか、建築物がある場合はスキップ
            if world_map.stockpiles.contains_key(&grid) || world_map.buildings.contains_key(&grid) {
                continue;
            }
            // 通行不能な場所もスキップ
            if !world_map.is_walkable(gx, gy) {
                continue;
            }

            let pos = WorldMap::grid_to_world(gx, gy);
            match zone_type {
                ZoneType::Stockpile => {
                    let entity = commands
                        .spawn((
                            Stockpile {
                                capacity: 10,
                                resource_type: None,
                            },
                            Sprite {
                                color: Color::srgba(1.0, 1.0, 0.0, 0.2),
                                custom_size: Some(Vec2::splat(TILE_SIZE)),
                                ..default()
                            },
                            Transform::from_xyz(pos.x, pos.y, Z_MAP + 0.01),
                            Name::new("Stockpile"),
                        ))
                        .id();
                    world_map.stockpiles.insert(grid, entity);
                }
            }
        }
    }
}
