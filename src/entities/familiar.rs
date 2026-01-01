use bevy::prelude::*;
use crate::constants::*;
use crate::assets::GameAssets;
use crate::world::map::WorldMap;
use crate::entities::damned_soul::{Destination, Path};

/// 使い魔のコンポーネント
#[derive(Component)]
pub struct Familiar {
    pub familiar_type: FamiliarType,
    pub command_radius: f32,      // 指示を出せる範囲
    pub efficiency: f32,          // 人間を動かす効率 (0.0-1.0)
}

impl Familiar {
    pub fn new(familiar_type: FamiliarType) -> Self {
        let (command_radius, efficiency) = match familiar_type {
            FamiliarType::Imp => (TILE_SIZE * 5.0, 0.5),
            FamiliarType::Taskmaster => (TILE_SIZE * 8.0, 0.3),
            FamiliarType::Whisperer => (TILE_SIZE * 3.0, 0.8),
        };
        Self {
            familiar_type,
            command_radius,
            efficiency,
        }
    }
}

/// 使い魔の種類（パラメーター調整用に拡張可能）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FamiliarType {
    #[default]
    Imp,            // インプ - 汎用型、バランス
    Taskmaster,     // 監督官 - 広範囲、低効率
    Whisperer,      // 囁き手 - 狭範囲、高効率
}

/// 使い魔への指示
#[derive(Debug, Clone)]
pub enum FamiliarCommand {
    Idle,                            // 待機
    GatherResources,                 // リソース収集を命じる
    Construct(Entity),               // 建築命令
    Patrol,                          // パトロール
}

impl Default for FamiliarCommand {
    fn default() -> Self {
        Self::Idle
    }
}

/// 現在のアクティブな指示
#[derive(Component, Default)]
pub struct ActiveCommand {
    pub command: FamiliarCommand,
    pub assigned_souls: Vec<Entity>,  // 割り当てられた人間
}

/// 使い魔をスポーンする
pub fn spawn_familiar(
    mut commands: Commands,
    game_assets: Res<GameAssets>,
    world_map: Res<WorldMap>,
) {
    // マップ中央付近に使い魔を配置
    let spawn_pos = Vec2::new(0.0, 0.0);
    let spawn_grid = WorldMap::world_to_grid(spawn_pos);
    
    // 歩ける場所を探す
    let mut actual_grid = spawn_grid;
    'search: for dx in -3..=3 {
        for dy in -3..=3 {
            let test = (spawn_grid.0 + dx, spawn_grid.1 + dy);
            if world_map.is_walkable(test.0, test.1) {
                actual_grid = test;
                break 'search;
            }
        }
    }
    let actual_pos = WorldMap::grid_to_world(actual_grid.0, actual_grid.1);

    commands.spawn((
        Familiar::new(FamiliarType::Imp),
        ActiveCommand::default(),
        Destination(actual_pos),  // 移動先
        Path::default(),          // 経路
        Sprite {
            image: game_assets.colonist.clone(),  // TODO: 専用テクスチャ
            custom_size: Some(Vec2::splat(TILE_SIZE * 0.9)),
            color: Color::srgb(1.0, 0.3, 0.3),  // 赤みがかった色で区別
            ..default()
        },
        Transform::from_xyz(actual_pos.x, actual_pos.y, 1.5),  // 人間より少し上に表示
    ));

    info!("SPAWN: Familiar (Imp) at {:?}", actual_pos);
}

/// 使い魔の範囲表示用コンポーネント
#[derive(Component)]
pub struct FamiliarRangeIndicator(pub Entity);  // 親の使い魔Entity

/// 使い魔が選択されている時に範囲を表示するシステム
pub fn update_familiar_range_indicator(
    q_familiars: Query<(Entity, &Transform, &Familiar)>,
    selected: Res<crate::interface::selection::SelectedEntity>,
    mut q_indicators: Query<(Entity, &FamiliarRangeIndicator, &mut Transform, &mut Visibility), Without<Familiar>>,
    mut commands: Commands,
) {
    // 選択されている使い魔を確認
    let selected_familiar = selected.0.and_then(|e| q_familiars.get(e).ok());

    if let Some((fam_entity, fam_transform, familiar)) = selected_familiar {
        // インジケーターがあれば更新、なければ作成
        let mut found = false;
        for (_, indicator, mut transform, mut visibility) in q_indicators.iter_mut() {
            if indicator.0 == fam_entity {
                transform.translation = fam_transform.translation.truncate().extend(0.3);
                *visibility = Visibility::Visible;
                found = true;
            } else {
                *visibility = Visibility::Hidden;
            }
        }

        if !found {
            commands.spawn((
                FamiliarRangeIndicator(fam_entity),
                Sprite {
                    color: Color::srgba(1.0, 0.5, 0.0, 0.15),
                    custom_size: Some(Vec2::splat(familiar.command_radius * 2.0)),
                    ..default()
                },
                Transform::from_translation(fam_transform.translation.truncate().extend(0.3)),
            ));
        }
    } else {
        // 使い魔が選択されていなければ全て非表示
        for (_, _, _, mut visibility) in q_indicators.iter_mut() {
            *visibility = Visibility::Hidden;
        }
    }
}

/// 使い魔の移動システム
pub fn familiar_movement(
    time: Res<Time>,
    mut query: Query<(&mut Transform, &mut Path), With<Familiar>>,
) {
    for (mut transform, mut path) in query.iter_mut() {
        if path.current_index < path.waypoints.len() {
            let target = path.waypoints[path.current_index];
            let current_pos = transform.translation.truncate();
            let to_target = target - current_pos;
            let distance = to_target.length();

            if distance > 1.0 {
                let speed = 100.0;  // 使い魔は速く動く
                let move_dist = (speed * time.delta_secs()).min(distance);
                let direction = to_target.normalize();
                let velocity = direction * move_dist;
                transform.translation += velocity.extend(0.0);
            } else {
                path.current_index += 1;
            }
        }
    }
}

