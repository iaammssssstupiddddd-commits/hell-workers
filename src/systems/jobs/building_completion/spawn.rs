use super::super::{Blueprint, Building, BuildingType, Door, DoorState, ProvisionalWall};
use crate::assets::GameAssets;
use crate::constants::TILE_SIZE;
use bevy::prelude::*;

pub(super) fn spawn_completed_building(
    commands: &mut Commands,
    bp: &Blueprint,
    transform: &Transform,
    game_assets: &GameAssets,
) -> Entity {
    let is_provisional = !bp.is_fully_complete();
    let (sprite_image, custom_size) = match bp.kind {
        BuildingType::Wall => (game_assets.wall_isolated.clone(), Vec2::splat(TILE_SIZE)),
        BuildingType::Door => (game_assets.door_closed.clone(), Vec2::splat(TILE_SIZE)),
        BuildingType::Floor => (game_assets.mud_floor.clone(), Vec2::splat(TILE_SIZE)),
        BuildingType::Tank => (game_assets.tank_empty.clone(), Vec2::splat(TILE_SIZE * 2.0)),
        BuildingType::MudMixer => (game_assets.mud_mixer.clone(), Vec2::splat(TILE_SIZE * 2.0)),
        BuildingType::RestArea => (game_assets.rest_area.clone(), Vec2::splat(TILE_SIZE * 2.0)),
        BuildingType::Bridge => (
            game_assets.bridge.clone(),
            Vec2::new(TILE_SIZE * 2.0, TILE_SIZE * 5.0),
        ),
        BuildingType::SandPile => (game_assets.sand_pile.clone(), Vec2::splat(TILE_SIZE)),
        BuildingType::BonePile => (game_assets.bone_pile.clone(), Vec2::splat(TILE_SIZE)),
        BuildingType::WheelbarrowParking => (
            game_assets.wheelbarrow_parking.clone(),
            Vec2::splat(TILE_SIZE * 2.0),
        ),
    };

    let building_entity = commands
        .spawn((
            Building {
                kind: bp.kind,
                is_provisional,
            },
            Sprite {
                image: sprite_image,
                custom_size: Some(custom_size),
                ..default()
            },
            *transform,
            Name::new(format!("Building ({:?})", bp.kind)),
            crate::systems::visual::blueprint::BuildingBounceEffect {
                bounce_animation: crate::systems::utils::animations::BounceAnimation {
                    timer: 0.0,
                    config: crate::systems::utils::animations::BounceAnimationConfig {
                        duration: crate::systems::visual::blueprint::BOUNCE_DURATION,
                        min_scale: 1.0,
                        max_scale: 1.2,
                    },
                },
            },
        ))
        .id();

    if bp.kind == BuildingType::Wall && is_provisional {
        commands
            .entity(building_entity)
            .insert(ProvisionalWall::default());
    }

    if bp.kind == BuildingType::Door {
        commands.entity(building_entity).insert(Door {
            state: DoorState::Closed,
        });
    }

    building_entity
}
