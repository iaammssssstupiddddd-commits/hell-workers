use super::types::ResourceType;
use crate::constants::*;
use crate::game_state::ZoneContext;
use crate::interface::ui::UiInputState;
use crate::world::map::WorldMap;
use bevy::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect)]
pub enum ZoneType {
    Stockpile,
}

#[derive(Component, Reflect)]
#[reflect(Component)]
pub struct Stockpile {
    pub capacity: usize,
    /// 最初に格納された資源の種類。空の場合は None。
    pub resource_type: Option<ResourceType>,
}

pub fn zone_placement(
    buttons: Res<ButtonInput<MouseButton>>,
    q_window: Query<&Window, With<bevy::window::PrimaryWindow>>,
    q_camera: Query<(&Camera, &GlobalTransform), With<crate::interface::camera::MainCamera>>,
    ui_input_state: Res<UiInputState>,
    zone_context: Res<ZoneContext>,
    mut world_map: ResMut<WorldMap>,
    mut commands: Commands,
) {
    if let Some(zone_type) = zone_context.0 {
        if ui_input_state.pointer_over_ui {
            return;
        }

        if buttons.pressed(MouseButton::Left) {
            let Ok((camera, camera_transform)) = q_camera.single() else {
                return;
            };
            let Ok(window) = q_window.single() else {
                return;
            };

            if let Some(cursor_pos) = window.cursor_position() {
                if let Ok(world_pos) = camera.viewport_to_world_2d(camera_transform, cursor_pos) {
                    let grid = WorldMap::world_to_grid(world_pos);

                    if !world_map.stockpiles.contains_key(&grid) {
                        let pos = WorldMap::grid_to_world(grid.0, grid.1);

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
                                    ))
                                    .id();
                                world_map.stockpiles.insert(grid, entity);
                            }
                        }
                    }
                }
            }
        }
    }
}
