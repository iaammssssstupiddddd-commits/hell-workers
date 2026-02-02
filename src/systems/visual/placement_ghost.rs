use crate::game_state::{BuildContext, PlayMode};
use crate::interface::camera::MainCamera;
use crate::systems::jobs::BuildingType;
use crate::world::map::WorldMap;
use crate::constants::TILE_SIZE;
use bevy::prelude::*;

#[derive(Component)]
pub struct PlacementGhost;

pub fn placement_ghost_system(
    mut commands: Commands,
    play_mode: Res<State<PlayMode>>,
    build_context: Res<BuildContext>,
    mut q_ghost: Query<
        (Entity, &mut Transform, &mut Sprite),
        With<PlacementGhost>,
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
        return;
    }

    let Some(building_type) = build_context.0 else {
        for (entity, _, _) in q_ghost.iter() {
            commands.entity(entity).despawn();
        }
        return;
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
    let occupied_grids = match building_type {
        BuildingType::Tank | BuildingType::MudMixer => {
            vec![
                grid_pos,
                (grid_pos.0 + 1, grid_pos.1),
                (grid_pos.0, grid_pos.1 + 1),
                (grid_pos.0 + 1, grid_pos.1 + 1),
            ]
        }
        _ => vec![grid_pos],
    };

    // 配置可能かチェック
    let can_place = occupied_grids.iter().all(|&g| {
        !world_map.buildings.contains_key(&g)
            && !world_map.stockpiles.contains_key(&g)
            && world_map.is_walkable(g.0, g.1)
    });

    // 描画位置の計算
    // 2x2の場合はグリッドの交差点（4セルの中心）になるように補正
    let draw_pos = match building_type {
        BuildingType::Tank | BuildingType::MudMixer => {
            let base_pos = WorldMap::grid_to_world(grid_pos.0, grid_pos.1);
            base_pos + Vec2::new(TILE_SIZE * 0.5, TILE_SIZE * 0.5)
        }
        _ => WorldMap::snap_to_grid_center(world_pos)
    };

    // 画像とサイズ
    let (texture, size) = match building_type {
        BuildingType::Wall => (game_assets.wall_isolated.clone(), Vec2::splat(TILE_SIZE)),
        BuildingType::Floor => (game_assets.dirt.clone(), Vec2::splat(TILE_SIZE)),
        BuildingType::Tank => (game_assets.tank_empty.clone(), Vec2::splat(TILE_SIZE * 2.0)),
        BuildingType::MudMixer => (game_assets.mud_mixer.clone(), Vec2::splat(TILE_SIZE * 2.0)),
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
}
