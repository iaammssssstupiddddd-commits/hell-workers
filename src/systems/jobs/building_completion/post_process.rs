use super::super::{Blueprint, BuildingType, MudMixerStorage, SandPile, TaskSlots};
use crate::assets::GameAssets;
use crate::constants::{
    MUD_MIXER_CAPACITY, TILE_SIZE, Z_FLOATING_TEXT, Z_ITEM_OBSTACLE, Z_ITEM_PICKUP, Z_MAP,
};
use crate::world::map::WorldMap;
use bevy::prelude::*;

pub(super) fn apply_building_specific_post_process(
    commands: &mut Commands,
    building_entity: Entity,
    bp: &Blueprint,
    transform: &Transform,
    game_assets: &GameAssets,
    world_map: &mut WorldMap,
) {
    if bp.kind == BuildingType::Tank {
        setup_tank(commands, building_entity, transform, game_assets, world_map);
    }

    if bp.kind == BuildingType::MudMixer {
        setup_mud_mixer(commands, building_entity, transform, game_assets, world_map);
    }

    spawn_completion_text(commands, transform, game_assets);
}

fn setup_tank(
    commands: &mut Commands,
    building_entity: Entity,
    transform: &Transform,
    game_assets: &GameAssets,
    world_map: &mut WorldMap,
) {
    commands
        .entity(building_entity)
        .insert(crate::systems::logistics::Stockpile {
            capacity: 50,
            resource_type: Some(crate::systems::logistics::ResourceType::Water),
        });

    let (bx, by) = WorldMap::world_to_grid(transform.translation.truncate());
    let storage_grids = [(bx, by - 2), (bx + 1, by - 2)];
    let mut storage_entities = Vec::new();

    for (gx, gy) in storage_grids {
        let pos = WorldMap::grid_to_world(gx, gy);
        let storage_entity = commands
            .spawn((
                crate::systems::logistics::Stockpile {
                    capacity: 10,
                    resource_type: None,
                },
                crate::systems::logistics::BelongsTo(building_entity),
                Sprite {
                    color: Color::srgba(1.0, 1.0, 0.0, 0.2),
                    custom_size: Some(Vec2::splat(TILE_SIZE)),
                    ..default()
                },
                Transform::from_xyz(pos.x, pos.y, Z_MAP + 0.01),
                Name::new("Tank Bucket Storage"),
            ))
            .id();
        world_map.stockpiles.insert((gx, gy), storage_entity);
        storage_entities.push(storage_entity);
    }

    for i in 0..5 {
        let storage_idx = if i < 3 { 0 } else { 1 };
        let storage_entity = storage_entities[storage_idx];
        let grid = storage_grids[storage_idx];
        let spawn_pos = WorldMap::grid_to_world(grid.0, grid.1);

        commands.spawn((
            crate::systems::logistics::ResourceItem(
                crate::systems::logistics::ResourceType::BucketEmpty,
            ),
            crate::systems::logistics::BelongsTo(building_entity),
            crate::relationships::StoredIn(storage_entity),
            crate::systems::logistics::InStockpile(storage_entity),
            TaskSlots::new(1),
            Sprite {
                image: game_assets.bucket_empty.clone(),
                custom_size: Some(Vec2::splat(TILE_SIZE * 0.6)),
                ..default()
            },
            Transform::from_xyz(spawn_pos.x, spawn_pos.y, Z_ITEM_PICKUP),
            Name::new("Empty Bucket (Tank Dedicated)"),
        ));
    }
}

fn setup_mud_mixer(
    commands: &mut Commands,
    building_entity: Entity,
    transform: &Transform,
    game_assets: &GameAssets,
    world_map: &mut WorldMap,
) {
    commands.entity(building_entity).insert((
        MudMixerStorage::default(),
        crate::systems::logistics::Stockpile {
            capacity: MUD_MIXER_CAPACITY as usize,
            resource_type: Some(crate::systems::logistics::ResourceType::Water),
        },
    ));

    let (bx, by) = WorldMap::world_to_grid(transform.translation.truncate());
    let sand_positions = [(bx - 2, by - 1), (bx - 2, by)];

    for (sx, sy) in sand_positions {
        let pos = WorldMap::grid_to_world(sx, sy);
        commands.spawn((
            SandPile,
            super::super::ObstaclePosition(sx, sy),
            crate::systems::logistics::BelongsTo(building_entity),
            Sprite {
                image: game_assets.sand_pile.clone(),
                custom_size: Some(Vec2::splat(TILE_SIZE * 0.8)),
                ..default()
            },
            Transform::from_xyz(pos.x, pos.y, Z_ITEM_OBSTACLE),
            Name::new("SandPile"),
        ));
        world_map.add_obstacle(sx, sy);
    }
}

fn spawn_completion_text(commands: &mut Commands, transform: &Transform, game_assets: &GameAssets) {
    let completion_config = crate::systems::utils::floating_text::FloatingTextConfig {
        lifetime: crate::systems::visual::blueprint::COMPLETION_TEXT_LIFETIME,
        velocity: Vec2::new(0.0, 15.0),
        initial_color: Color::srgb(0.2, 1.0, 0.4),
        fade_out: true,
    };
    let completion_entity = crate::systems::utils::floating_text::spawn_floating_text(
        commands,
        "Construction Complete!",
        transform.translation.truncate().extend(Z_FLOATING_TEXT) + Vec3::new(0.0, 20.0, 0.0),
        completion_config.clone(),
        Some(16.0),
        game_assets.font_ui.clone(),
    );
    commands.entity(completion_entity).insert((
        crate::systems::visual::blueprint::CompletionText {
            floating_text: crate::systems::utils::floating_text::FloatingText {
                lifetime: completion_config.lifetime,
                config: completion_config,
            },
        },
        TextLayout::new_with_justify(Justify::Center),
    ));
}
