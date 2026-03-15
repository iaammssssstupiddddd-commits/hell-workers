use crate::app_contexts::{
    CompanionPlacementKind, CompanionPlacementState, MoveContext, MovePlacementState,
};
use crate::systems::jobs::{Building, BuildingType};
use crate::systems::visual::placement_ghost::{PlacementGhost, PlacementPartnerGhost};
use crate::world::map::{WorldMap, WorldMapRead, WorldMapRef};
use bevy::prelude::*;
use hw_core::constants::TILE_SIZE;
use hw_core::game_state::PlayMode;
use hw_ui::camera::MainCamera;
use hw_ui::selection::{
    can_place_moved_building, move_anchor_grid, move_occupied_grids, move_spawn_pos,
};

use super::placement::validate_tank_companion_for_move;

pub fn building_move_preview_system(
    mut commands: Commands,
    play_mode: Res<State<PlayMode>>,
    move_context: Res<MoveContext>,
    move_placement_state: Res<MovePlacementState>,
    companion_state: Res<CompanionPlacementState>,
    q_window: Query<&Window, With<bevy::window::PrimaryWindow>>,
    q_camera: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    world_map: WorldMapRead,
    game_assets: Res<crate::assets::GameAssets>,
    q_buildings: Query<
        (Entity, &Building, &Transform),
        (Without<PlacementGhost>, Without<PlacementPartnerGhost>),
    >,
    q_bucket_storages: Query<
        (Entity, &crate::systems::logistics::BelongsTo),
        With<crate::systems::logistics::BucketStorage>,
    >,
    mut q_ghost: Query<(Entity, &mut Transform, &mut Sprite), With<PlacementGhost>>,
    mut q_partner_ghost: Query<
        (Entity, &mut Transform, &mut Sprite),
        (With<PlacementPartnerGhost>, Without<PlacementGhost>),
    >,
) {
    if *play_mode.get() != PlayMode::BuildingMove {
        despawn_move_ghosts(&mut commands, &q_ghost, &q_partner_ghost);
        return;
    }

    let Some(target_entity) = move_context.0 else {
        despawn_move_ghosts(&mut commands, &q_ghost, &q_partner_ghost);
        return;
    };

    let Ok((_, building, transform)) = q_buildings.get(target_entity) else {
        despawn_move_ghosts(&mut commands, &q_ghost, &q_partner_ghost);
        return;
    };

    let Ok((camera, camera_transform)) = q_camera.single() else {
        return;
    };
    let Ok(window) = q_window.single() else {
        return;
    };
    let Some(cursor_pos) = window.cursor_position() else {
        return;
    };
    let Ok(world_pos) = camera.viewport_to_world_2d(camera_transform, cursor_pos) else {
        return;
    };

    let destination_grid = WorldMap::world_to_grid(world_pos);
    if let (Some(active_companion), Some(pending)) =
        (companion_state.0.as_ref(), move_placement_state.0)
    {
        if active_companion.kind == CompanionPlacementKind::BucketStorage
            && pending.building == target_entity
        {
            let old_anchor = move_anchor_grid(building.kind, transform.translation.truncate());
            let old_occupied = move_occupied_grids(building.kind, old_anchor);
            let destination_occupied = move_occupied_grids(building.kind, pending.destination_grid);
            let can_place = can_place_moved_building(
                &WorldMapRef(world_map.as_ref()),
                target_entity,
                &old_occupied,
                &destination_occupied,
            ) && validate_tank_companion_for_move(
                &world_map,
                target_entity,
                pending.destination_grid,
                destination_grid,
                &old_occupied,
                &q_bucket_storages,
            )
            .can_place;
            let draw_base = WorldMap::grid_to_world(destination_grid.0, destination_grid.1);
            let draw_pos = draw_base + Vec2::new(TILE_SIZE * 0.5, 0.0);
            let color = if can_place {
                Color::srgba(0.5, 1.0, 0.5, 0.5)
            } else {
                Color::srgba(1.0, 0.2, 0.2, 0.5)
            };

            upsert_move_ghost(
                &mut commands,
                &mut q_ghost,
                game_assets.bucket_empty.clone(),
                Vec2::new(TILE_SIZE * 2.0, TILE_SIZE),
                draw_pos,
                color,
            );

            let partner_pos = move_spawn_pos(BuildingType::Tank, pending.destination_grid);
            let partner_color = Color::srgba(0.8, 0.9, 1.0, 0.35);
            upsert_partner_ghost(
                &mut commands,
                &mut q_partner_ghost,
                game_assets.tank_empty.clone(),
                Vec2::splat(TILE_SIZE * 2.0),
                partner_pos,
                partner_color,
            );
            return;
        }
    }

    despawn_partner_ghost(&mut commands, &q_partner_ghost);

    let old_anchor = move_anchor_grid(building.kind, transform.translation.truncate());
    let old_occupied = move_occupied_grids(building.kind, old_anchor);
    let destination_occupied = move_occupied_grids(building.kind, destination_grid);
    let can_place = can_place_moved_building(
        &WorldMapRef(world_map.as_ref()),
        target_entity,
        &old_occupied,
        &destination_occupied,
    );

    let draw_pos = move_spawn_pos(building.kind, destination_grid);
    let (texture, size) = match building.kind {
        BuildingType::Tank => (
            game_assets.tank_empty.clone(),
            Vec2::splat(hw_core::constants::TILE_SIZE * 2.0),
        ),
        BuildingType::MudMixer => (
            game_assets.mud_mixer.clone(),
            Vec2::splat(hw_core::constants::TILE_SIZE * 2.0),
        ),
        _ => return,
    };
    let color = if can_place {
        Color::srgba(0.5, 1.0, 0.5, 0.5)
    } else {
        Color::srgba(1.0, 0.2, 0.2, 0.5)
    };

    upsert_move_ghost(&mut commands, &mut q_ghost, texture, size, draw_pos, color);
}

fn despawn_move_ghosts(
    commands: &mut Commands,
    q_ghost: &Query<(Entity, &mut Transform, &mut Sprite), With<PlacementGhost>>,
    q_partner_ghost: &Query<
        (Entity, &mut Transform, &mut Sprite),
        (With<PlacementPartnerGhost>, Without<PlacementGhost>),
    >,
) {
    for (entity, _, _) in q_ghost.iter() {
        commands.entity(entity).despawn();
    }
    despawn_partner_ghost(commands, q_partner_ghost);
}

fn despawn_partner_ghost(
    commands: &mut Commands,
    q_partner_ghost: &Query<
        (Entity, &mut Transform, &mut Sprite),
        (With<PlacementPartnerGhost>, Without<PlacementGhost>),
    >,
) {
    for (entity, _, _) in q_partner_ghost.iter() {
        commands.entity(entity).despawn();
    }
}

fn upsert_move_ghost(
    commands: &mut Commands,
    q_ghost: &mut Query<(Entity, &mut Transform, &mut Sprite), With<PlacementGhost>>,
    texture: Handle<Image>,
    size: Vec2,
    draw_pos: Vec2,
    color: Color,
) {
    if let Some((_, mut ghost_transform, mut sprite)) = q_ghost.iter_mut().next() {
        ghost_transform.translation =
            Vec3::new(draw_pos.x, draw_pos.y, hw_core::constants::Z_SELECTION);
        sprite.image = texture;
        sprite.custom_size = Some(size);
        sprite.color = color;
        return;
    }

    commands.spawn((
        PlacementGhost,
        Sprite {
            image: texture,
            custom_size: Some(size),
            color,
            ..default()
        },
        Transform::from_xyz(draw_pos.x, draw_pos.y, hw_core::constants::Z_SELECTION),
    ));
}

fn upsert_partner_ghost(
    commands: &mut Commands,
    q_partner_ghost: &mut Query<
        (Entity, &mut Transform, &mut Sprite),
        (With<PlacementPartnerGhost>, Without<PlacementGhost>),
    >,
    texture: Handle<Image>,
    size: Vec2,
    draw_pos: Vec2,
    color: Color,
) {
    if let Some((_, mut transform, mut sprite)) = q_partner_ghost.iter_mut().next() {
        transform.translation = Vec3::new(
            draw_pos.x,
            draw_pos.y,
            hw_core::constants::Z_SELECTION - 0.001,
        );
        sprite.image = texture;
        sprite.custom_size = Some(size);
        sprite.color = color;
        return;
    }

    commands.spawn((
        PlacementPartnerGhost,
        Sprite {
            image: texture,
            custom_size: Some(size),
            color,
            ..default()
        },
        Transform::from_xyz(
            draw_pos.x,
            draw_pos.y,
            hw_core::constants::Z_SELECTION - 0.001,
        ),
    ));
}
