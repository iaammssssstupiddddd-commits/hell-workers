use bevy::prelude::*;
use crate::constants::*;
use crate::interface::camera::MainCamera;
use crate::entities::damned_soul::{DamnedSoul, Destination};
use crate::entities::familiar::Familiar;
use crate::systems::jobs::{BuildingType, Blueprint};
use crate::assets::GameAssets;
use crate::world::map::WorldMap;

#[derive(Resource, Default)]
pub struct SelectedEntity(pub Option<Entity>);

#[derive(Component)]
pub struct SelectionIndicator;

#[derive(Resource, Default)]
pub struct BuildMode(pub Option<BuildingType>);

pub fn handle_mouse_input(
    buttons: Res<ButtonInput<MouseButton>>,
    q_window: Query<&Window, With<bevy::window::PrimaryWindow>>,
    q_camera: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    q_souls: Query<(Entity, &GlobalTransform), With<DamnedSoul>>,
    q_familiars: Query<(Entity, &GlobalTransform), With<Familiar>>,
    q_ui: Query<&Interaction, With<Button>>,
    mut selected_entity: ResMut<SelectedEntity>,
    mut q_dest: Query<&mut Destination>,
    task_mode: Res<crate::systems::command::TaskMode>,
) {
    if *task_mode != crate::systems::command::TaskMode::None {
        return;
    }

    for interaction in q_ui.iter() {
        if *interaction != Interaction::None {
            return;
        }
    }

    let (camera, camera_transform) = q_camera.single();
    let window = q_window.single();

    if let Some(cursor_pos) = window.cursor_position() {
        if let Ok(world_pos) = camera.viewport_to_world_2d(camera_transform, cursor_pos) {
            if buttons.just_pressed(MouseButton::Left) {
                let mut found = false;
                
                // まず使い魔をチェック（優先）
                for (entity, transform) in q_familiars.iter() {
                    let pos = transform.translation().truncate();
                    if pos.distance(world_pos) < TILE_SIZE / 2.0 {
                        selected_entity.0 = Some(entity);
                        found = true;
                        info!("SELECTED: Familiar");
                        break;
                    }
                }
                
                // 次に人間をチェック
                if !found {
                    for (entity, transform) in q_souls.iter() {
                        let pos = transform.translation().truncate();
                        if pos.distance(world_pos) < TILE_SIZE / 2.0 {
                            selected_entity.0 = Some(entity);
                            found = true;
                            info!("SELECTED: DamnedSoul");
                            break;
                        }
                    }
                }
                
                if !found {
                    selected_entity.0 = None;
                }
            }

            if buttons.just_pressed(MouseButton::Right) {
                if let Some(selected) = selected_entity.0 {
                    if let Ok(mut dest) = q_dest.get_mut(selected) {
                        dest.0 = world_pos;
                        info!("ORDER: Move to {:?}", world_pos);
                    }
                }
            }
        }
    }
}

pub fn blueprint_placement(
    buttons: Res<ButtonInput<MouseButton>>,
    q_window: Query<&Window, With<bevy::window::PrimaryWindow>>,
    q_camera: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    q_ui: Query<&Interaction, With<Button>>,
    mut world_map: ResMut<WorldMap>,
    build_mode: Res<BuildMode>,
    game_assets: Res<GameAssets>,
    mut commands: Commands,
) {
    for interaction in q_ui.iter() {
        if *interaction != Interaction::None {
            return;
        }
    }

    if let Some(building_type) = build_mode.0 {
        if buttons.just_pressed(MouseButton::Left) {
            let (camera, camera_transform) = q_camera.single();
            let window = q_window.single();

            if let Some(cursor_pos) = window.cursor_position() {
                if let Ok(world_pos) = camera.viewport_to_world_2d(camera_transform, cursor_pos) {
                    let grid = WorldMap::world_to_grid(world_pos);
                    
                    if !world_map.buildings.contains_key(&grid) {
                        let pos = WorldMap::grid_to_world(grid.0, grid.1);
                        
                        let texture = match building_type {
                            BuildingType::Wall => game_assets.wall.clone(),
                            BuildingType::Floor => game_assets.dirt.clone(),
                        };

                        let entity = commands.spawn((
                            Blueprint {
                                kind: building_type,
                                progress: 0.0,
                            },
                            Sprite {
                                image: texture,
                                color: Color::srgba(1.0, 1.0, 1.0, 0.5),
                                custom_size: Some(Vec2::splat(TILE_SIZE)),
                                ..default()
                            },
                            Transform::from_xyz(pos.x, pos.y, 0.1),
                        )).id();
                        
                        world_map.buildings.insert(grid, entity);
                        info!("BLUEPRINT: Placed {:?} at {:?}", building_type, grid);
                    }
                }
            }
        }
    }
}

pub fn update_selection_indicator(
    selected: Res<SelectedEntity>,
    mut q_indicator: Query<(Entity, &mut Transform), With<SelectionIndicator>>,
    q_transforms: Query<&GlobalTransform>,
    mut commands: Commands,
) {
    if let Some(entity) = selected.0 {
        if let Ok(target_transform) = q_transforms.get(entity) {
            if let Ok((_, mut indicator_transform)) = q_indicator.get_single_mut() {
                indicator_transform.translation = target_transform.translation().truncate().extend(0.5);
            } else {
                commands.spawn((
                    SelectionIndicator,
                    Sprite {
                        color: Color::srgba(1.0, 1.0, 0.0, 0.4),
                        custom_size: Some(Vec2::splat(TILE_SIZE * 1.1)),
                        ..default()
                    },
                    Transform::from_translation(target_transform.translation().truncate().extend(0.5)),
                ));
            }
        }
    } else {
        for (indicator_entity, _) in q_indicator.iter() {
            commands.entity(indicator_entity).despawn();
        }
    }
}
