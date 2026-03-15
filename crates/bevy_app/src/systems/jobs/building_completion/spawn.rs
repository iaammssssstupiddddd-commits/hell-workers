use super::super::{Blueprint, Building, BuildingType, Door, DoorState, ProvisionalWall};
use crate::assets::GameAssets;
use bevy::prelude::*;
use hw_core::constants::{
    TILE_SIZE, Z_BUILDING_FLOOR, Z_BUILDING_STRUCT,
};
use hw_visual::layer::VisualLayerKind;

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

    let (z, layer_kind) = match bp.kind {
        BuildingType::Floor | BuildingType::SandPile | BuildingType::BonePile => {
            (Z_BUILDING_FLOOR, VisualLayerKind::Floor)
        }
        _ => (Z_BUILDING_STRUCT, VisualLayerKind::Struct),
    };

    let parent_transform =
        Transform::from_xyz(transform.translation.x, transform.translation.y, z);

    let building_entity = commands
        .spawn((
            Building {
                kind: bp.kind,
                is_provisional,
            },
            parent_transform,
            Name::new(format!("Building ({:?})", bp.kind)),
            hw_visual::blueprint::BuildingBounceEffect {
                bounce_animation: hw_visual::animations::BounceAnimation {
                    timer: 0.0,
                    config: hw_visual::animations::BounceAnimationConfig {
                        duration: hw_visual::blueprint::BOUNCE_DURATION,
                        min_scale: 1.0,
                        max_scale: 1.2,
                    },
                },
            },
        ))
        .with_children(|parent| {
            parent.spawn((
                layer_kind,
                Sprite {
                    image: sprite_image,
                    custom_size: Some(custom_size),
                    ..default()
                },
                Transform::default(),
                Name::new(format!("VisualLayer ({:?})", layer_kind)),
            ));
        })
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
