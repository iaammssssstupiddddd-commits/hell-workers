use crate::app_contexts::{
    BuildContext, CompanionParentKind, CompanionPlacementKind, CompanionPlacementState,
};
use crate::interface::camera::MainCamera;
use crate::systems::jobs::{Blueprint, Building, BuildingType};
use crate::systems::world::zones::{Site, Yard};
use crate::world::map::{RIVER_Y_MIN, WorldMap, WorldMapRead, WorldMapRef};
use bevy::prelude::*;
use hw_core::constants::TILE_SIZE;
use hw_core::game_state::PlayMode;
use hw_ui::selection::{
    BuildingPlacementContext, TANK_NEARBY_BUCKET_STORAGE_TILES, bucket_storage_geometry,
    building_geometry, building_occupied_grids, building_size, building_spawn_pos,
    validate_bucket_storage_placement, validate_building_placement,
};

#[derive(Component)]
pub struct PlacementGhost;

#[derive(Component)]
pub struct PlacementPartnerGhost;

fn is_replaceable_wall_at(
    world_map: &WorldMap,
    q_buildings: &Query<&Building>,
    grid: (i32, i32),
) -> bool {
    world_map.building_entity(grid).is_some_and(|entity| {
        q_buildings
            .get(entity)
            .is_ok_and(|building| building.kind == BuildingType::Wall && !building.is_provisional)
    })
}

fn is_wall_or_door_at(
    world_map: &WorldMap,
    q_buildings: &Query<&Building>,
    q_blueprints: &Query<&Blueprint>,
    grid: (i32, i32),
) -> bool {
    let Some(entity) = world_map.building_entity(grid) else {
        return false;
    };
    if let Ok(building) = q_buildings.get(entity) {
        return matches!(building.kind, BuildingType::Wall | BuildingType::Door);
    }
    if let Ok(blueprint) = q_blueprints.get(entity) {
        return matches!(blueprint.kind, BuildingType::Wall | BuildingType::Door);
    }
    false
}

pub fn placement_ghost_system(
    mut commands: Commands,
    play_mode: Res<State<PlayMode>>,
    build_context: Res<BuildContext>,
    companion_state: Res<CompanionPlacementState>,
    mut q_ghost: Query<(Entity, &mut Transform, &mut Sprite), With<PlacementGhost>>,
    mut q_partner_ghost: Query<
        (Entity, &mut Transform, &mut Sprite),
        (With<PlacementPartnerGhost>, Without<PlacementGhost>),
    >,
    q_window: Query<&Window, With<bevy::window::PrimaryWindow>>,
    q_camera: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    game_assets: Res<crate::assets::GameAssets>,
    world_map: WorldMapRead,
    q_buildings: Query<&Building>,
    q_blueprints: Query<&Blueprint>,
    q_sites: Query<&Site>,
    q_yards: Query<&Yard>,
) {
    // 建築モード以外ならゴーストを削除して終了
    if *play_mode.get() != PlayMode::BuildingPlace {
        for (entity, _, _) in q_ghost.iter() {
            commands.entity(entity).despawn();
        }
        for (entity, _, _) in q_partner_ghost.iter() {
            commands.entity(entity).despawn();
        }
        return;
    }

    let companion_kind = companion_state.0.as_ref().map(|state| state.kind);
    let building_type_opt = build_context.0;
    if companion_kind != Some(CompanionPlacementKind::BucketStorage) && building_type_opt.is_none()
    {
        for (entity, _, _) in q_ghost.iter() {
            commands.entity(entity).despawn();
        }
        for (entity, _, _) in q_partner_ghost.iter() {
            commands.entity(entity).despawn();
        }
        return;
    }
    // バケツ置き場 companion のときは build_context が None でも表示する
    let building_type = building_type_opt.unwrap_or(BuildingType::Floor);

    let Ok((camera, camera_transform)) = q_camera.single() else {
        return;
    };
    let Ok(window) = q_window.single() else {
        return;
    };

    // マウス位置取得
    let Some(cursor_pos) = window.cursor_position() else {
        return;
    };
    let Ok(world_pos) = camera.viewport_to_world_2d(camera_transform, cursor_pos) else {
        return;
    };

    let grid_pos = WorldMap::world_to_grid(world_pos);
    let read_world = WorldMapRef(world_map.as_ref());

    let geometry = if companion_kind == Some(CompanionPlacementKind::BucketStorage) {
        bucket_storage_geometry(grid_pos)
    } else {
        building_geometry(building_type, grid_pos, RIVER_Y_MIN)
    };

    let validation = if companion_kind == Some(CompanionPlacementKind::BucketStorage) {
        let Some(active) = companion_state.0.as_ref() else {
            return;
        };
        let within_radius = companion_state
            .0
            .as_ref()
            .map(|state| world_pos.distance(state.center) <= state.radius)
            .unwrap_or(true);
        let parent_type = match active.parent_kind {
            CompanionParentKind::Tank => BuildingType::Tank,
        };
        let parent_geometry = building_geometry(parent_type, active.parent_anchor, RIVER_Y_MIN);
        let parent_ctx = BuildingPlacementContext {
            world: &read_world,
            in_site: q_sites
                .iter()
                .any(|site| site.contains(parent_geometry.draw_pos)),
            in_yard: q_yards
                .iter()
                .any(|yard| yard.contains(parent_geometry.draw_pos)),
            is_wall_or_door_at: &|candidate| {
                is_wall_or_door_at(world_map.as_ref(), &q_buildings, &q_blueprints, candidate)
            },
            is_replaceable_wall_at: &|candidate| {
                is_replaceable_wall_at(world_map.as_ref(), &q_buildings, candidate)
            },
        };
        let parent_validation = validate_building_placement(
            &parent_ctx,
            parent_type,
            active.parent_anchor,
            &parent_geometry,
        );
        if !parent_validation.can_place {
            parent_validation
        } else {
            let parent_occupied_grids =
                building_occupied_grids(BuildingType::Tank, active.parent_anchor, RIVER_Y_MIN);
            validate_bucket_storage_placement(
                &read_world,
                &geometry,
                &parent_occupied_grids,
                within_radius,
                TANK_NEARBY_BUCKET_STORAGE_TILES,
            )
        }
    } else {
        let ctx = BuildingPlacementContext {
            world: &read_world,
            in_site: q_sites.iter().any(|site| site.contains(geometry.draw_pos)),
            in_yard: q_yards.iter().any(|yard| yard.contains(geometry.draw_pos)),
            is_wall_or_door_at: &|candidate| {
                is_wall_or_door_at(world_map.as_ref(), &q_buildings, &q_blueprints, candidate)
            },
            is_replaceable_wall_at: &|candidate| {
                is_replaceable_wall_at(world_map.as_ref(), &q_buildings, candidate)
            },
        };
        validate_building_placement(&ctx, building_type, grid_pos, &geometry)
    };
    let can_place = validation.can_place;

    let draw_pos = geometry.draw_pos;
    let size = geometry.size;
    let texture = if companion_kind == Some(CompanionPlacementKind::BucketStorage) {
        game_assets.bucket_empty.clone()
    } else {
        match building_type {
            BuildingType::Wall => game_assets.wall_isolated.clone(),
            BuildingType::Door => game_assets.door_closed.clone(),
            BuildingType::Floor => game_assets.mud_floor.clone(),
            BuildingType::Tank => game_assets.tank_empty.clone(),
            BuildingType::MudMixer => game_assets.mud_mixer.clone(),
            BuildingType::RestArea => game_assets.rest_area.clone(),
            BuildingType::Bridge => game_assets.bridge.clone(),
            BuildingType::SandPile => game_assets.sand_pile.clone(),
            BuildingType::BonePile => game_assets.bone_pile.clone(),
            BuildingType::WheelbarrowParking => game_assets.wheelbarrow_parking.clone(),
        }
    };

    // 色（半透明 + 緑/赤判定）
    let color = if can_place {
        Color::srgba(0.5, 1.0, 0.5, 0.5)
    } else {
        Color::srgba(1.0, 0.2, 0.2, 0.5)
    };

    // ゴースト更新または生成
    if let Some((_, mut transform, mut sprite)) = q_ghost.iter_mut().next() {
        transform.translation = draw_pos.extend(hw_core::constants::Z_SELECTION);
        sprite.custom_size = Some(size);
        sprite.color = color;
        if sprite.image != texture {
            sprite.image = texture;
        }
    } else {
        for (entity, _, _) in q_ghost.iter() {
            commands.entity(entity).despawn();
        }

        commands.spawn((
            PlacementGhost,
            Sprite {
                image: texture,
                color,
                custom_size: Some(size),
                ..default()
            },
            Transform::from_translation(draw_pos.extend(hw_core::constants::Z_SELECTION)),
        ));
    }

    // companion 配置中は相方（親側）ゴーストを固定表示
    if let Some(companion) = companion_state.0.as_ref() {
        let partner_type = match companion.parent_kind {
            CompanionParentKind::Tank => BuildingType::Tank,
        };
        let partner_pos = building_spawn_pos(partner_type, companion.parent_anchor, RIVER_Y_MIN);
        let (partner_texture, partner_size) = match partner_type {
            BuildingType::Tank => (
                game_assets.tank_empty.clone(),
                building_size(BuildingType::Tank),
            ),
            _ => (game_assets.dirt.clone(), Vec2::splat(TILE_SIZE)),
        };
        let partner_color = Color::srgba(0.8, 0.9, 1.0, 0.35);

        if let Some((_, mut transform, mut sprite)) = q_partner_ghost.iter_mut().next() {
            transform.translation = partner_pos.extend(hw_core::constants::Z_SELECTION - 0.001);
            sprite.custom_size = Some(partner_size);
            sprite.color = partner_color;
            if sprite.image != partner_texture {
                sprite.image = partner_texture;
            }
        } else {
            for (entity, _, _) in q_partner_ghost.iter() {
                commands.entity(entity).despawn();
            }
            commands.spawn((
                PlacementPartnerGhost,
                Sprite {
                    image: partner_texture,
                    color: partner_color,
                    custom_size: Some(partner_size),
                    ..default()
                },
                Transform::from_translation(
                    partner_pos.extend(hw_core::constants::Z_SELECTION - 0.001),
                ),
            ));
        }
    } else {
        for (entity, _, _) in q_partner_ghost.iter() {
            commands.entity(entity).despawn();
        }
    }
}
