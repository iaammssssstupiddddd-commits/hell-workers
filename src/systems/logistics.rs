use crate::assets::GameAssets;
use crate::constants::*;
use crate::game_state::ZoneContext;
use crate::systems::jobs::{Rock, Tree};
use crate::world::map::WorldMap;
use bevy::prelude::*;
use rand::Rng;
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect)]
pub enum ResourceType {
    Wood,
    Stone,
}

#[derive(Component, Reflect)]
#[reflect(Component)]
pub struct ResourceItem(pub ResourceType);

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

#[derive(Resource, Default)]
pub struct ResourceLabels(pub HashMap<(i32, i32), Entity>);

#[derive(Component)]
pub struct ResourceCountLabel;

pub fn zone_placement(
    buttons: Res<ButtonInput<MouseButton>>,
    q_window: Query<&Window, With<bevy::window::PrimaryWindow>>,
    q_camera: Query<(&Camera, &GlobalTransform), With<crate::interface::camera::MainCamera>>,
    q_ui: Query<&Interaction, With<Button>>,
    zone_context: Res<ZoneContext>,
    mut world_map: ResMut<WorldMap>,
    mut commands: Commands,
) {
    if let Some(zone_type) = zone_context.0 {
        for interaction in q_ui.iter() {
            if *interaction != Interaction::None {
                return;
            }
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
                                        Transform::from_xyz(pos.x, pos.y, 0.01),
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

pub fn item_spawner_system(
    mut commands: Commands,
    game_assets: Res<GameAssets>,
    time: Res<Time>,
    mut timer: Local<f32>,
    world_map: Res<WorldMap>,
) {
    *timer += time.delta_secs();
    if *timer > 5.0 {
        let mut rng = rand::thread_rng();
        let gx = rng.gen_range(5..MAP_WIDTH - 5);
        let gy = rng.gen_range(5..MAP_HEIGHT - 5);

        if world_map.is_walkable(gx, gy) {
            let spawn_pos = WorldMap::grid_to_world(gx, gy);
            commands.spawn((
                ResourceItem(ResourceType::Wood),
                Sprite {
                    image: game_assets.wood.clone(),
                    custom_size: Some(Vec2::splat(TILE_SIZE * 0.5)),
                    color: Color::srgb(0.5, 0.35, 0.05),
                    ..default()
                },
                Transform::from_xyz(spawn_pos.x, spawn_pos.y, 0.6),
            ));
            *timer = 0.0;
            debug!("SPAWNER: Wood spawned randomly at {:?}", spawn_pos);
        }
    }
}

pub fn initial_resource_spawner(
    mut commands: Commands,
    game_assets: Res<GameAssets>,
    world_map: Res<WorldMap>,
) {
    let mut rng = rand::thread_rng();

    // 木のスポーン
    for _ in 0..15 {
        let gx = rng.gen_range(5..MAP_WIDTH - 5);
        let gy = rng.gen_range(5..MAP_HEIGHT - 5);
        if world_map.is_walkable(gx, gy) {
            let pos = WorldMap::grid_to_world(gx, gy);
            commands.spawn((
                Tree,
                Sprite {
                    image: game_assets.wood.clone(), // TODO: 木のテクスチャ
                    custom_size: Some(Vec2::splat(TILE_SIZE * 0.8)),
                    color: Color::srgb(0.2, 0.5, 0.2),
                    ..default()
                },
                Transform::from_xyz(pos.x, pos.y, 0.5),
            ));
        }
    }

    // 岩のスポーン
    for _ in 0..10 {
        let gx = rng.gen_range(5..MAP_WIDTH - 5);
        let gy = rng.gen_range(5..MAP_HEIGHT - 5);
        if world_map.is_walkable(gx, gy) {
            let pos = WorldMap::grid_to_world(gx, gy);
            commands.spawn((
                Rock,
                Sprite {
                    image: game_assets.stone.clone(),
                    custom_size: Some(Vec2::splat(TILE_SIZE * 0.7)),
                    color: Color::srgb(0.5, 0.5, 0.5),
                    ..default()
                },
                Transform::from_xyz(pos.x, pos.y, 0.5),
            ));
        }
    }

    // 既存の資材も少し撒く
    let mut count = 0;
    while count < 5 {
        let gx = rng.gen_range(5..MAP_WIDTH - 5);
        let gy = rng.gen_range(5..MAP_HEIGHT - 5);
        if world_map.is_walkable(gx, gy) {
            let spawn_pos = WorldMap::grid_to_world(gx, gy);
            commands.spawn((
                ResourceItem(ResourceType::Wood),
                Sprite {
                    image: game_assets.wood.clone(),
                    custom_size: Some(Vec2::splat(TILE_SIZE * 0.5)),
                    color: Color::srgb(0.5, 0.35, 0.05),
                    ..default()
                },
                Transform::from_xyz(spawn_pos.x, spawn_pos.y, 0.6),
            ));
            count += 1;
        }
    }
    info!("SPAWNER: Trees, Rocks, and Initial wood spawned");
}

pub fn resource_count_display_system(
    mut commands: Commands,
    q_items: Query<(&Transform, &Visibility), With<ResourceItem>>,
    mut labels: ResMut<ResourceLabels>,
    mut q_text: Query<&mut Text2d, With<ResourceCountLabel>>,
    mut q_transform: Query<&mut Transform, (With<ResourceCountLabel>, Without<ResourceItem>)>,
) {
    let mut grid_counts: HashMap<(i32, i32), usize> = HashMap::new();

    for (transform, visibility) in q_items.iter() {
        if matches!(visibility, Visibility::Visible | Visibility::Inherited) {
            let grid = WorldMap::world_to_grid(transform.translation.truncate());
            *grid_counts.entry(grid).or_insert(0) += 1;
        }
    }

    // ラベルの更新または作成
    for (grid, count) in grid_counts.iter() {
        let pos = WorldMap::grid_to_world(grid.0, grid.1);
        // 新しい座標系では pos は中心なので、右上端 (32*0.5=16) 寄りにオフセット
        // 0.35 * 32 = 11.2 なので正確にタイルの内側に収まる
        let target_transform =
            Transform::from_xyz(pos.x + TILE_SIZE * 0.35, pos.y + TILE_SIZE * 0.35, 1.0);

        if let Some(&entity) = labels.0.get(grid) {
            if let Ok(mut transform) = q_transform.get_mut(entity) {
                if let Ok(mut text) = q_text.get_mut(entity) {
                    text.0 = count.to_string();
                }
                *transform = target_transform;
            } else {
                // エンティティが存在しないか、Transformを持っていない場合は再作成フラグ
                labels.0.remove(grid);
            }
        }

        // 存在しない、または上記で remove された場合は作成
        if !labels.0.contains_key(grid) {
            let entity = commands
                .spawn((
                    ResourceCountLabel,
                    Text2d::new(count.to_string()),
                    TextFont {
                        font_size: 14.0,
                        ..default()
                    },
                    TextColor(Color::WHITE),
                    TextLayout::new_with_justify(Justify::Center),
                    target_transform,
                ))
                .id();
            labels.0.insert(*grid, entity);
        }
    }

    // 不要なラベルの削除
    let mut to_remove = Vec::new();
    for (&grid, &entity) in labels.0.iter() {
        if !grid_counts.contains_key(&grid) {
            if let Ok(mut e) = commands.get_entity(entity) {
                e.despawn();
            }
            to_remove.push(grid);
        }
    }
    for grid in to_remove {
        labels.0.remove(&grid);
    }
}
