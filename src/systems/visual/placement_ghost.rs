use crate::constants::TILE_SIZE;
use crate::game_state::{
    BuildContext, CompanionParentKind, CompanionPlacementKind, CompanionPlacementState, PlayMode,
};
use crate::interface::camera::MainCamera;
use crate::systems::jobs::BuildingType;
use crate::world::map::WorldMap;
use bevy::prelude::*;

const TANK_NEARBY_BUCKET_STORAGE_TILES: i32 = 3;

#[derive(Component)]
pub struct PlacementGhost;

#[derive(Component)]
pub struct PlacementPartnerGhost;

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
    world_map: Res<WorldMap>,
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
    let building_type = if companion_kind == Some(CompanionPlacementKind::SandPile) {
        BuildingType::SandPile
    } else {
        building_type_opt.unwrap_or(BuildingType::Floor)
    };

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

    // 座標計算（ここ重要：配置ロジックと一致させる）
    let grid_pos = WorldMap::world_to_grid(world_pos);

    // 占有グリッドの計算
    let occupied_grids = if companion_kind == Some(CompanionPlacementKind::BucketStorage) {
        vec![grid_pos, (grid_pos.0 + 1, grid_pos.1)]
    } else {
        match building_type {
            BuildingType::Tank | BuildingType::MudMixer | BuildingType::WheelbarrowParking => {
                vec![
                    grid_pos,
                    (grid_pos.0 + 1, grid_pos.1),
                    (grid_pos.0, grid_pos.1 + 1),
                    (grid_pos.0 + 1, grid_pos.1 + 1),
                ]
            }
            _ => vec![grid_pos],
        }
    };

    // 配置可能かチェック
    let can_place_on_grid = occupied_grids.iter().all(|&g| {
        !world_map.buildings.contains_key(&g)
            && !world_map.stockpiles.contains_key(&g)
            && world_map.is_walkable(g.0, g.1)
    });
    let in_companion_range = companion_state
        .0
        .as_ref()
        .map(|state| world_pos.distance(state.center) <= state.radius)
        .unwrap_or(true);
    let can_place_near_parent = companion_state.0.as_ref().is_none_or(|state| {
        if state.kind != CompanionPlacementKind::BucketStorage {
            return true;
        }
        let parent_occupied_grids =
            occupied_grids_for_parent(state.parent_kind, state.parent_anchor);
        occupied_grids.iter().all(|&storage_grid| {
            parent_occupied_grids.iter().any(|&parent_grid| {
                grid_is_nearby(parent_grid, storage_grid, TANK_NEARBY_BUCKET_STORAGE_TILES)
            })
        })
    });
    let can_place = can_place_on_grid && in_companion_range && can_place_near_parent;

    // 描画位置の計算
    // 2x2の場合はグリッドの交差点（4セルの中心）になるように補正
    let draw_pos = if companion_kind == Some(CompanionPlacementKind::BucketStorage) {
        let base_pos = WorldMap::grid_to_world(grid_pos.0, grid_pos.1);
        base_pos + Vec2::new(TILE_SIZE * 0.5, 0.0)
    } else {
        match building_type {
            BuildingType::Tank | BuildingType::MudMixer | BuildingType::WheelbarrowParking => {
                let base_pos = WorldMap::grid_to_world(grid_pos.0, grid_pos.1);
                base_pos + Vec2::new(TILE_SIZE * 0.5, TILE_SIZE * 0.5)
            }
            _ => WorldMap::snap_to_grid_center(world_pos),
        }
    };

    // 画像とサイズ
    let (texture, size) = if companion_kind == Some(CompanionPlacementKind::BucketStorage) {
        (
            game_assets.bucket_empty.clone(),
            Vec2::new(TILE_SIZE * 2.0, TILE_SIZE),
        )
    } else {
        match building_type {
            BuildingType::Wall => (game_assets.wall_isolated.clone(), Vec2::splat(TILE_SIZE)),
            BuildingType::Floor => (game_assets.dirt.clone(), Vec2::splat(TILE_SIZE)),
            BuildingType::Tank => (game_assets.tank_empty.clone(), Vec2::splat(TILE_SIZE * 2.0)),
            BuildingType::MudMixer => (game_assets.mud_mixer.clone(), Vec2::splat(TILE_SIZE * 2.0)),
            BuildingType::SandPile => (game_assets.sand_pile.clone(), Vec2::splat(TILE_SIZE)),
            BuildingType::BonePile => (game_assets.bone_pile.clone(), Vec2::splat(TILE_SIZE)),
            BuildingType::WheelbarrowParking => (
                game_assets.wheelbarrow_parking.clone(),
                Vec2::splat(TILE_SIZE * 2.0),
            ),
        }
    };

    // 色（半透明 + 緑/赤判定）
    let color = if can_place {
        Color::srgba(0.5, 1.0, 0.5, 0.5) // 配置可能: 緑っぽく
    } else {
        Color::srgba(1.0, 0.2, 0.2, 0.5) // 配置不可: 赤っぽく
    };

    // ゴースト更新または生成
    if let Some((_, mut transform, mut sprite)) = q_ghost.iter_mut().next() {
        transform.translation = draw_pos.extend(crate::constants::Z_SELECTION);
        sprite.custom_size = Some(size);
        sprite.color = color;
        if sprite.image != texture {
            sprite.image = texture;
        }
    } else {
        // 既存のゴーストが複数ある場合はバグなので全て消す
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
            Transform::from_translation(draw_pos.extend(crate::constants::Z_SELECTION)),
        ));
    }

    // companion 配置中は相方（親側）ゴーストを固定表示
    if let Some(companion) = companion_state.0.as_ref() {
        let partner_type = match companion.parent_kind {
            CompanionParentKind::Tank => BuildingType::Tank,
            CompanionParentKind::MudMixer => BuildingType::MudMixer,
        };
        let partner_base =
            WorldMap::grid_to_world(companion.parent_anchor.0, companion.parent_anchor.1);
        let partner_pos = match partner_type {
            BuildingType::Tank | BuildingType::MudMixer => {
                partner_base + Vec2::new(TILE_SIZE * 0.5, TILE_SIZE * 0.5)
            }
            _ => partner_base,
        };
        let (partner_texture, partner_size) = match partner_type {
            BuildingType::Tank => (game_assets.tank_empty.clone(), Vec2::splat(TILE_SIZE * 2.0)),
            BuildingType::MudMixer => (game_assets.mud_mixer.clone(), Vec2::splat(TILE_SIZE * 2.0)),
            _ => (game_assets.dirt.clone(), Vec2::splat(TILE_SIZE)),
        };
        let partner_color = Color::srgba(0.8, 0.9, 1.0, 0.35);

        if let Some((_, mut transform, mut sprite)) = q_partner_ghost.iter_mut().next() {
            transform.translation = partner_pos.extend(crate::constants::Z_SELECTION - 0.001);
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
                    partner_pos.extend(crate::constants::Z_SELECTION - 0.001),
                ),
            ));
        }
    } else {
        for (entity, _, _) in q_partner_ghost.iter() {
            commands.entity(entity).despawn();
        }
    }
}

fn grid_is_nearby(base: (i32, i32), target: (i32, i32), tiles: i32) -> bool {
    (target.0 - base.0).abs() <= tiles && (target.1 - base.1).abs() <= tiles
}

fn occupied_grids_for_parent(
    parent_kind: CompanionParentKind,
    anchor: (i32, i32),
) -> [(i32, i32); 4] {
    match parent_kind {
        CompanionParentKind::Tank | CompanionParentKind::MudMixer => [
            anchor,
            (anchor.0 + 1, anchor.1),
            (anchor.0, anchor.1 + 1),
            (anchor.0 + 1, anchor.1 + 1),
        ],
    }
}
