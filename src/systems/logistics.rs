use crate::assets::GameAssets;
use crate::constants::*;
use crate::game_state::ZoneContext;
use crate::systems::jobs::{Rock, Tree};
use crate::world::map::{WorldMap, TREE_POSITIONS, ROCK_POSITIONS, INITIAL_WOOD_POSITIONS};
use bevy::prelude::*;
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect)]
pub enum ResourceType {
    Wood,
    Rock, // 旧Stone（岩採掘でのみ入手可能）
    Water,
    BucketEmpty,
    BucketWater,
    Sand,
    StasisMud,
}

#[derive(Component, Reflect)]
#[reflect(Component)]
pub struct ResourceItem(pub ResourceType);

/// アイテムがタスク発行済み（占有中）であることを示す
#[derive(Component, Reflect, Debug, Clone, Copy, PartialEq, Eq)]
#[reflect(Component)]
pub struct ReservedForTask;

/// エンティティが特定の親（タンクなど）に属することを示す
#[derive(Component, Reflect, Debug, Clone, Copy, PartialEq, Eq)]
#[reflect(Component)]
pub struct BelongsTo(pub Entity);

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

/// ソウルが持っているアイテムのエンティティ
#[derive(Component, Default, Reflect, Clone, Copy, Debug)]
#[reflect(Component)]
pub struct Inventory(pub Option<Entity>);

/// アイテムがストックパイルに格納されていることを示すコンポーネント
#[derive(Component, Reflect)]
#[reflect(Component)]
pub struct InStockpile(pub Entity);

/// アイテムがソウルに要求されていることを示すコンポーネント
#[derive(Component, Reflect)]
#[reflect(Component)]
pub struct ClaimedBy(pub Entity);

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



pub fn initial_resource_spawner(
    mut commands: Commands,
    game_assets: Res<GameAssets>,
    mut world_map: ResMut<WorldMap>,
) {
    // 木のスポーン（障害物として登録）
    for &(gx, gy) in TREE_POSITIONS {
        // 地形が通行可能な場合のみスポーン（障害物チェックなし）
        if let Some(idx) = world_map.pos_to_idx(gx, gy) {
            if world_map.tiles[idx].is_walkable() {
                let pos = WorldMap::grid_to_world(gx, gy);
                commands.spawn((
                    Tree,
                    crate::systems::jobs::ObstaclePosition(gx, gy),
                    Sprite {
                        image: game_assets.tree.clone(),
                        custom_size: Some(Vec2::splat(TILE_SIZE * 1.5)),
                        ..default()
                    },
                    Transform::from_xyz(pos.x, pos.y, Z_ITEM_OBSTACLE),
                ));
                world_map.add_obstacle(gx, gy);
            }
        }
    }

    // 岩のスポーン（障害物として登録）
    for &(gx, gy) in ROCK_POSITIONS {
        if let Some(idx) = world_map.pos_to_idx(gx, gy) {
            if world_map.tiles[idx].is_walkable() {
                let pos = WorldMap::grid_to_world(gx, gy);
                commands.spawn((
                    Rock,
                    crate::systems::jobs::ObstaclePosition(gx, gy),
                    Sprite {
                        image: game_assets.rock.clone(),
                        custom_size: Some(Vec2::splat(TILE_SIZE * 1.2)),
                        ..default()
                    },
                    Transform::from_xyz(pos.x, pos.y, Z_ITEM_OBSTACLE),
                ));
                world_map.add_obstacle(gx, gy);
            }
        }
    }

    // 既存の資材（木材）も中央付近に少し撒く
    for &(gx, gy) in INITIAL_WOOD_POSITIONS {
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
                Transform::from_xyz(spawn_pos.x, spawn_pos.y, Z_ITEM_PICKUP),
            ));
        }
    }

    let rock_count = ROCK_POSITIONS.len();
    let obstacle_count = world_map.obstacles.iter().filter(|&&b| b).count();
    info!("SPAWNER: Fixed Trees ({}), Rocks ({}) spawned. WorldMap active obstacles: {}", TREE_POSITIONS.len(), rock_count, obstacle_count);
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
        let target_transform = Transform::from_xyz(
            pos.x + TILE_SIZE * 0.35,
            pos.y + TILE_SIZE * 0.35,
            Z_CHARACTER,
        );

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
