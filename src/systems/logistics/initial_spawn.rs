use super::types::{ResourceItem, ResourceType};
use crate::assets::GameAssets;
use crate::constants::*;
use crate::relationships::{LoadedItems, ParkedAt};
use crate::systems::jobs::{Building, BuildingType, ObstaclePosition, Rock, TaskSlots, Tree};
use crate::systems::logistics::{BelongsTo, Wheelbarrow, WheelbarrowParking};
use crate::world::map::{INITIAL_WOOD_POSITIONS, ROCK_POSITIONS, TREE_POSITIONS, WorldMap};
use bevy::prelude::*;

const INITIAL_WHEELBARROW_PARKING_GRID: (i32, i32) = (58, 58);
const INITIAL_WHEELBARROW_PARKING_CAPACITY: usize = 2;

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
                let variant_index = rand::random::<usize>() % game_assets.trees.len();
                commands.spawn((
                    Tree,
                    crate::systems::jobs::TreeVariant(variant_index),
                    crate::systems::jobs::ObstaclePosition(gx, gy),
                    Sprite {
                        image: game_assets.trees[variant_index].clone(),
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
                    ..default()
                },
                Transform::from_xyz(spawn_pos.x, spawn_pos.y, Z_ITEM_PICKUP),
            ));
        }
    }

    spawn_initial_wheelbarrow_parking(&mut commands, &game_assets, &mut world_map);

    let rock_count = ROCK_POSITIONS.len();
    let obstacle_count = world_map.obstacles.iter().filter(|&&b| b).count();
    info!(
        "SPAWNER: Fixed Trees ({}), Rocks ({}) spawned. WorldMap active obstacles: {}",
        TREE_POSITIONS.len(),
        rock_count,
        obstacle_count
    );
}

fn spawn_initial_wheelbarrow_parking(
    commands: &mut Commands,
    game_assets: &GameAssets,
    world_map: &mut WorldMap,
) {
    let base = INITIAL_WHEELBARROW_PARKING_GRID;
    let occupied = [
        base,
        (base.0 + 1, base.1),
        (base.0, base.1 + 1),
        (base.0 + 1, base.1 + 1),
    ];

    if occupied
        .iter()
        .any(|(gx, gy)| !world_map.is_walkable(*gx, *gy))
    {
        warn!(
            "INITIAL_SPAWN: skipped initial wheelbarrow parking at {:?} (not walkable)",
            base
        );
        return;
    }

    let building_pos = WorldMap::grid_to_world(base.0, base.1) + Vec2::splat(TILE_SIZE * 0.5);
    let building_entity = commands
        .spawn((
            Building {
                kind: BuildingType::WheelbarrowParking,
                is_provisional: false,
            },
            WheelbarrowParking {
                capacity: INITIAL_WHEELBARROW_PARKING_CAPACITY,
            },
            Sprite {
                image: game_assets.wheelbarrow_parking.clone(),
                custom_size: Some(Vec2::splat(TILE_SIZE * 2.0)),
                ..default()
            },
            Transform::from_xyz(building_pos.x, building_pos.y, Z_ITEM_OBSTACLE),
            Name::new("Initial Wheelbarrow Parking"),
        ))
        .id();

    commands.entity(building_entity).with_children(|parent| {
        for (gx, gy) in occupied {
            parent.spawn((ObstaclePosition(gx, gy), Name::new("Building Obstacle")));
        }
    });

    for (gx, gy) in occupied {
        world_map.add_obstacle(gx, gy);
        world_map.buildings.insert((gx, gy), building_entity);
    }

    let offsets = [Vec2::new(-8.0, -8.0), Vec2::new(8.0, 8.0)];
    for i in 0..INITIAL_WHEELBARROW_PARKING_CAPACITY {
        let offset = offsets
            .get(i % offsets.len())
            .copied()
            .unwrap_or(Vec2::ZERO);
        let pos = building_pos + offset;

        commands.spawn((
            ResourceItem(ResourceType::Wheelbarrow),
            Wheelbarrow {
                capacity: WHEELBARROW_CAPACITY,
            },
            BelongsTo(building_entity),
            ParkedAt(building_entity),
            LoadedItems::default(),
            TaskSlots::new(1),
            Sprite {
                image: game_assets.wheelbarrow_empty.clone(),
                custom_size: Some(Vec2::splat(TILE_SIZE * 0.6)),
                ..default()
            },
            Transform::from_xyz(pos.x, pos.y, Z_ITEM_PICKUP),
            Visibility::Visible,
            Name::new(format!("Initial Wheelbarrow #{}", i)),
        ));
    }
}
