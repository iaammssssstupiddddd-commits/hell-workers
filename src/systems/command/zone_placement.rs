use crate::constants::*;
use crate::game_state::{PlayMode, TaskContext};
use crate::interface::camera::MainCamera;
use crate::interface::ui::UiInputState;
use crate::systems::command::{TaskArea, TaskMode};
use crate::systems::logistics::{Stockpile, ZoneType};
use crate::world::map::WorldMap;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

pub fn zone_placement_system(
    buttons: Res<ButtonInput<MouseButton>>,
    q_window: Query<&Window, With<PrimaryWindow>>,
    q_camera: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    ui_input_state: Res<UiInputState>,
    mut task_context: ResMut<TaskContext>,
    mut next_play_mode: ResMut<NextState<PlayMode>>,
    mut world_map: ResMut<WorldMap>,
    mut commands: Commands,
) {
    if ui_input_state.pointer_over_ui {
        return;
    }

    let TaskMode::ZonePlacement(zone_type, start_pos_opt) = task_context.0 else {
        return;
    };

    let Some(world_pos) = world_cursor_pos(&q_window, &q_camera) else {
        return;
    };
    let snapped_pos = WorldMap::snap_to_grid_edge(world_pos);

    // 開始
    if buttons.just_pressed(MouseButton::Left) {
        task_context.0 = TaskMode::ZonePlacement(zone_type, Some(snapped_pos));
        return;
    }

    // 確定
    if buttons.just_released(MouseButton::Left) {
        if let Some(start_pos) = start_pos_opt {
            let area = TaskArea::from_points(start_pos, snapped_pos);
            apply_zone_placement(&mut commands, &mut world_map, zone_type, &area);

            // Shift押下で継続、そうでなければ解除
            // FIXME: keyboard リソースが必要だが、一旦シンプルに解除
            task_context.0 = TaskMode::ZonePlacement(zone_type, None);
        }
        return;
    }

    // キャンセル (右クリック)
    if buttons.just_pressed(MouseButton::Right) {
        task_context.0 = TaskMode::None;
        next_play_mode.set(PlayMode::Normal);
    }
}

fn world_cursor_pos(
    q_window: &Query<&Window, With<PrimaryWindow>>,
    q_camera: &Query<(&Camera, &GlobalTransform), With<MainCamera>>,
) -> Option<Vec2> {
    let Ok((camera, camera_transform)) = q_camera.single() else {
        return None;
    };
    let Ok(window) = q_window.single() else {
        return None;
    };
    let cursor_pos: Vec2 = window.cursor_position()?;
    camera
        .viewport_to_world_2d(camera_transform, cursor_pos)
        .ok()
}

fn apply_zone_placement(
    commands: &mut Commands,
    world_map: &mut WorldMap,
    zone_type: ZoneType,
    area: &TaskArea,
) {
    let min_grid = WorldMap::world_to_grid(area.min + Vec2::splat(0.1));
    let max_grid = WorldMap::world_to_grid(area.max - Vec2::splat(0.1));

    for gy in min_grid.1..=max_grid.1 {
        for gx in min_grid.0..=max_grid.0 {
            let grid = (gx, gy);

            // 既に存在するか、建築物がある場合はスキップ
            if world_map.stockpiles.contains_key(&grid) || world_map.buildings.contains_key(&grid) {
                continue;
            }
            // 通行不能な場所もスキップ
            if !world_map.is_walkable(gx, gy) {
                continue;
            }

            let pos = WorldMap::grid_to_world(gx, gy);
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
                            Name::new("Stockpile"),
                        ))
                        .id();
                    world_map.stockpiles.insert(grid, entity);
                }
            }
        }
    }
}

pub fn zone_removal_system(
    buttons: Res<ButtonInput<MouseButton>>,
    q_window: Query<&Window, With<PrimaryWindow>>,
    q_camera: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    ui_input_state: Res<UiInputState>,
    mut task_context: ResMut<TaskContext>,
    mut next_play_mode: ResMut<NextState<PlayMode>>,
    mut world_map: ResMut<WorldMap>,
    mut commands: Commands,
    mut q_sprites: Query<&mut Sprite>,
) {
    if ui_input_state.pointer_over_ui {
        return;
    }

    let TaskMode::ZoneRemoval(zone_type, start_pos_opt) = task_context.0 else {
        return;
    };

    let Some(world_pos) = world_cursor_pos(&q_window, &q_camera) else {
        return;
    };
    let snapped_pos = WorldMap::snap_to_grid_edge(world_pos);

    // 開始
    if buttons.just_pressed(MouseButton::Left) {
        task_context.0 = TaskMode::ZoneRemoval(zone_type, Some(snapped_pos));
        return;
    }

    // プレビュー更新 (ドラッグ中のみ)
    if let Some(start_pos) = start_pos_opt {
        let area = TaskArea::from_points(start_pos, snapped_pos);
        update_removal_preview(&world_map, &area, &mut q_sprites);
    }

    // 確定
    if buttons.just_released(MouseButton::Left) {
        if let Some(start_pos) = start_pos_opt {
            let area = TaskArea::from_points(start_pos, snapped_pos);
            apply_zone_removal(&mut commands, &mut world_map, &area);

            // Shift押下で継続、そうでなければ解除
            task_context.0 = TaskMode::ZoneRemoval(zone_type, None);
        }
        // プレビュー解除 (全体を元の色に戻す簡易実装)
        // FIXME: パフォーマンス最適化の余地あり
        reset_stockpile_colors(&world_map, &mut q_sprites);
        return;
    }

    // キャンセル (右クリック)
    if buttons.just_pressed(MouseButton::Right) {
        if start_pos_opt.is_some() {
            task_context.0 = TaskMode::ZoneRemoval(zone_type, None);
            reset_stockpile_colors(&world_map, &mut q_sprites);
        } else {
            task_context.0 = TaskMode::None;
            next_play_mode.set(PlayMode::Normal);
        }
    }
}

fn apply_zone_removal(commands: &mut Commands, world_map: &mut WorldMap, area: &TaskArea) {
    let (to_remove, fragments) = identify_removal_targets(world_map, area);

    // 削除実行
    for grid in to_remove.iter().chain(fragments.iter()) {
        if let Some(entity) = world_map.stockpiles.remove(grid) {
            commands.entity(entity).despawn();
        }
    }
}

/// 削除対象と、それによって発生する孤立フラグメントを特定する
fn identify_removal_targets(
    world_map: &WorldMap,
    area: &TaskArea,
) -> (Vec<(i32, i32)>, Vec<(i32, i32)>) {
    let min_grid = WorldMap::world_to_grid(area.min + Vec2::splat(0.1));
    let max_grid = WorldMap::world_to_grid(area.max - Vec2::splat(0.1));

    let mut direct_removal = Vec::new();
    let mut remaining_candidates = std::collections::HashSet::new();

    // 1. 直接削除対象と、残存候補の洗い出し
    // 全てのストックパイルを確認するのは効率が悪いので、
    // 本来は「影響を受ける連結成分」だけを探索すべきだが、
    // ここでは簡易的に全ストックパイルを対象とする (数千個レベルなら問題ないはず)
    for (&grid, _) in &world_map.stockpiles {
        if grid.0 >= min_grid.0
            && grid.0 <= max_grid.0
            && grid.1 >= min_grid.1
            && grid.1 <= max_grid.1
        {
            direct_removal.push(grid);
        } else {
            remaining_candidates.insert(grid);
        }
    }

    if direct_removal.is_empty() {
        return (Vec::new(), Vec::new());
    }

    // 2. 残存候補の連結成分分析 (Flood Fill)
    let mut visited = std::collections::HashSet::new();
    let mut clusters: Vec<Vec<(i32, i32)>> = Vec::new();

    for &start_node in &remaining_candidates {
        if visited.contains(&start_node) {
            continue;
        }

        let mut cluster = Vec::new();
        let mut queue = std::collections::VecDeque::new();
        queue.push_back(start_node);
        visited.insert(start_node);

        while let Some(current) = queue.pop_front() {
            cluster.push(current);

            // 4方向隣接
            let neighbors = [
                (current.0 + 1, current.1),
                (current.0 - 1, current.1),
                (current.0, current.1 + 1),
                (current.0, current.1 - 1),
            ];

            for neighbor in neighbors {
                if remaining_candidates.contains(&neighbor) && visited.insert(neighbor) {
                    queue.push_back(neighbor);
                }
            }
        }
        clusters.push(cluster);
    }

    // 3. 最大クラスタ以外をフラグメントとして削除対象に追加
    if clusters.is_empty() {
        return (direct_removal, Vec::new());
    }

    // 最大サイズのクラスタを見つける
    // 同点の場合はどれか一つが残ればよいが、ちらつき防止のために
    // 座標（クラスタ内の最小座標）をタイブレーカーとして使用する
    let max_cluster_index = clusters
        .iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| {
            match a.len().cmp(&b.len()) {
                std::cmp::Ordering::Equal => {
                    // サイズが同じ場合、座標で比較して決定論的にする
                    let min_a = a.iter().min().unwrap();
                    let min_b = b.iter().min().unwrap();
                    min_a.cmp(min_b)
                }
                other => other,
            }
        })
        .map(|(i, _)| i)
        .unwrap();

    let mut fragment_removal = Vec::new();
    for (i, cluster) in clusters.iter().enumerate() {
        if i != max_cluster_index {
            fragment_removal.extend(cluster);
        }
    }

    (direct_removal, fragment_removal)
}

fn update_removal_preview(
    world_map: &WorldMap,
    area: &TaskArea,
    q_sprites: &mut Query<&mut Sprite>,
) {
    let (direct, fragments) = identify_removal_targets(world_map, area);

    // 全てリセット
    // (非効率だが確実)
    for (&_grid, &entity) in &world_map.stockpiles {
        if let Ok(mut sprite) = q_sprites.get_mut(entity) {
            sprite.color = Color::srgba(1.0, 1.0, 0.0, 0.2); // Default
        }
    }

    // 直接削除: 赤
    for grid in direct {
        if let Some(&entity) = world_map.stockpiles.get(&grid) {
            if let Ok(mut sprite) = q_sprites.get_mut(entity) {
                sprite.color = Color::srgba(1.0, 0.2, 0.2, 0.4);
            }
        }
    }

    // フラグメント削除: オレンジ
    for grid in fragments {
        if let Some(&entity) = world_map.stockpiles.get(&grid) {
            if let Ok(mut sprite) = q_sprites.get_mut(entity) {
                sprite.color = Color::srgba(1.0, 0.6, 0.0, 0.4);
            }
        }
    }
}

fn reset_stockpile_colors(world_map: &WorldMap, q_sprites: &mut Query<&mut Sprite>) {
    for (_, &entity) in &world_map.stockpiles {
        if let Ok(mut sprite) = q_sprites.get_mut(entity) {
            sprite.color = Color::srgba(1.0, 1.0, 0.0, 0.2);
        }
    }
}
