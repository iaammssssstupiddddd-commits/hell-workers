use super::super::{Blueprint, BonePile, BuildingType, MudMixerStorage, SandPile, TaskSlots};
use crate::assets::GameAssets;
use crate::constants::{
    MUD_MIXER_CAPACITY, TILE_SIZE, WHEELBARROW_CAPACITY, Z_FLOATING_TEXT, Z_ITEM_PICKUP,
};
use crate::world::map::WorldMap;
use bevy::prelude::*;

pub(super) fn apply_building_specific_post_process(
    commands: &mut Commands,
    blueprint_entity: Entity,
    building_entity: Entity,
    bp: &Blueprint,
    transform: &Transform,
    game_assets: &GameAssets,
    world_map: &mut WorldMap,
    promoted_bucket_storage: &[Entity],
) {
    if bp.kind == BuildingType::Tank {
        setup_tank(
            commands,
            blueprint_entity,
            building_entity,
            transform,
            game_assets,
            world_map,
            promoted_bucket_storage,
        );
    }

    if bp.kind == BuildingType::MudMixer {
        setup_mud_mixer(commands, building_entity);
    }

    if bp.kind == BuildingType::SandPile {
        setup_sand_pile(commands, building_entity);
    }

    if bp.kind == BuildingType::BonePile {
        setup_bone_pile(commands, building_entity);
    }

    if bp.kind == BuildingType::WheelbarrowParking {
        setup_wheelbarrow_parking(commands, building_entity, transform, game_assets);
    }

    spawn_completion_text(commands, transform, game_assets);
}

fn setup_tank(
    commands: &mut Commands,
    blueprint_entity: Entity,
    building_entity: Entity,
    transform: &Transform,
    game_assets: &GameAssets,
    world_map: &mut WorldMap,
    promoted_bucket_storage: &[Entity],
) {
    commands
        .entity(building_entity)
        .insert(crate::systems::logistics::Stockpile {
            capacity: 50,
            resource_type: Some(crate::systems::logistics::ResourceType::Water),
        });

    let mut storage_entities = promoted_bucket_storage.to_vec();
    if storage_entities.is_empty() {
        warn!(
            "TANK_SETUP: no companion bucket storage linked for {:?}; skip bucket spawn",
            building_entity
        );
        return;
    }

    storage_entities.sort_by_key(|entity| {
        find_stockpile_grid(world_map, *entity).unwrap_or((i32::MAX, i32::MAX))
    });

    let bucket_count = 5usize;
    for i in 0..bucket_count {
        let storage_entity = storage_entities[i % storage_entities.len()];
        let spawn_pos = find_stockpile_grid(world_map, storage_entity)
            .map(|(gx, gy)| WorldMap::grid_to_world(gx, gy))
            .unwrap_or_else(|| transform.translation.truncate());
        commands.spawn((
            crate::systems::logistics::ResourceItem(
                crate::systems::logistics::ResourceType::BucketEmpty,
            ),
            crate::systems::logistics::BelongsTo(building_entity),
            crate::relationships::StoredIn(storage_entity),
            TaskSlots::new(1),
            Sprite {
                image: game_assets.bucket_empty.clone(),
                custom_size: Some(Vec2::splat(TILE_SIZE * 0.6)),
                ..default()
            },
            Transform::from_xyz(spawn_pos.x, spawn_pos.y, Z_ITEM_PICKUP),
            Name::new(format!(
                "Empty Bucket (Tank Dedicated, from {:?})",
                blueprint_entity
            )),
        ));
    }
}

fn setup_mud_mixer(commands: &mut Commands, building_entity: Entity) {
    commands.entity(building_entity).insert((
        MudMixerStorage::default(),
        crate::systems::logistics::Stockpile {
            capacity: MUD_MIXER_CAPACITY as usize,
            resource_type: Some(crate::systems::logistics::ResourceType::Water),
        },
    ));
}

fn setup_sand_pile(commands: &mut Commands, building_entity: Entity) {
    commands.entity(building_entity).insert(SandPile);
}

fn setup_bone_pile(commands: &mut Commands, building_entity: Entity) {
    commands.entity(building_entity).insert(BonePile);
}

fn setup_wheelbarrow_parking(
    commands: &mut Commands,
    building_entity: Entity,
    transform: &Transform,
    game_assets: &GameAssets,
) {
    let parking_capacity = 2usize;
    commands.entity(building_entity).insert(
        crate::systems::logistics::WheelbarrowParking {
            capacity: parking_capacity,
        },
    );

    let pos = transform.translation.truncate();
    for i in 0..parking_capacity {
        commands.spawn((
            crate::systems::logistics::ResourceItem(
                crate::systems::logistics::ResourceType::Wheelbarrow,
            ),
            crate::systems::logistics::Wheelbarrow {
                capacity: WHEELBARROW_CAPACITY,
            },
            crate::systems::logistics::BelongsTo(building_entity),
            crate::relationships::ParkedAt(building_entity),
            crate::relationships::LoadedItems::default(),
            TaskSlots::new(1),
            Sprite {
                image: game_assets.wheelbarrow_empty.clone(),
                custom_size: Some(Vec2::splat(TILE_SIZE * 0.6)),
                ..default()
            },
            Transform::from_xyz(pos.x, pos.y, Z_ITEM_PICKUP),
            Visibility::Visible,
            Name::new(format!("Wheelbarrow #{}", i)),
        ));
    }
}

fn find_stockpile_grid(world_map: &WorldMap, stockpile_entity: Entity) -> Option<(i32, i32)> {
    world_map
        .stockpiles
        .iter()
        .find_map(|(grid, entity)| (*entity == stockpile_entity).then_some(*grid))
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
