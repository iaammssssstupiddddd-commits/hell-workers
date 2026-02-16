//! 地形境界オーバーレイの生成
//!
//! 高優先度の地形が隣接する低優先度タイル上にエッジ/コーナーとしてはみ出す。
//! 優先度: Grass(3) > Dirt(2) > Sand(1) > River(0)

use crate::assets::GameAssets;
use crate::constants::*;
use bevy::prelude::*;
use std::f32::consts::PI;

use super::{TerrainType, WorldMap};

/// 境界オーバーレイであることを示すマーカー
#[derive(Component)]
pub struct TerrainBorder;

/// 4方向の隣接オフセット (dx, dy) と回転角度
const EDGE_DIRS: [(i32, i32, f32); 4] = [
    (0, 1, 0.0),       // 北: 0°
    (1, 0, -PI / 2.0), // 東: -90° (270°)
    (0, -1, PI),       // 南: 180°
    (-1, 0, PI / 2.0), // 西: 90°
];

/// 4角の隣接オフセット (dx, dy) と回転角度、
/// および隣接する2辺のインデックス (EDGE_DIRS のインデックス)
const CORNER_DIRS: [(i32, i32, f32, usize, usize); 4] = [
    (1, 1, 0.0, 0, 1),        // 北東: 0°, 北辺(0)と東辺(1)
    (1, -1, -PI / 2.0, 2, 1), // 南東: -90°, 南辺(2)と東辺(1)
    (-1, -1, PI, 2, 3),       // 南西: 180°, 南辺(2)と西辺(3)
    (-1, 1, PI / 2.0, 0, 3),  // 北西: 90°, 北辺(0)と西辺(3)
];

pub fn spawn_terrain_borders(
    mut commands: Commands,
    game_assets: Res<GameAssets>,
    world_map: Res<WorldMap>,
) {
    for y in 0..MAP_HEIGHT {
        for x in 0..MAP_WIDTH {
            let idx = match world_map.pos_to_idx(x, y) {
                Some(i) => i,
                None => continue,
            };
            let current = world_map.tiles[idx];
            let current_priority = current.priority();
            let pos = WorldMap::grid_to_world(x, y);

            // 辺ごとに高優先度の隣接を記録
            let mut edge_neighbor_priority = [0u8; 4];

            // 辺の処理
            for (dir_idx, &(dx, dy, angle)) in EDGE_DIRS.iter().enumerate() {
                let nx = x + dx;
                let ny = y + dy;
                let neighbor = match world_map.pos_to_idx(nx, ny) {
                    Some(ni) => world_map.tiles[ni],
                    None => continue,
                };
                let neighbor_priority = neighbor.priority();
                edge_neighbor_priority[dir_idx] = neighbor_priority;

                if neighbor_priority > current_priority {
                    let (edge_tex, _, _) = border_textures(&neighbor, &game_assets);
                    if let Some(texture) = edge_tex {
                        commands.spawn((
                            TerrainBorder,
                            Sprite {
                                image: texture,
                                custom_size: Some(Vec2::splat(TILE_SIZE)),
                                ..default()
                            },
                            Transform::from_xyz(pos.x, pos.y, neighbor.z_layer())
                                .with_rotation(Quat::from_rotation_z(angle)),
                        ));
                    }
                }
            }

            // 角の処理（隣接する2辺が既にカバーしていない場合のみ）
            for &(dx, dy, angle, edge_a, edge_b) in &CORNER_DIRS {
                let nx = x + dx;
                let ny = y + dy;
                let neighbor = match world_map.pos_to_idx(nx, ny) {
                    Some(ni) => world_map.tiles[ni],
                    None => continue,
                };
                let neighbor_priority = neighbor.priority();

                // 外角: 斜め隣接が高優先度で、隣接2辺は同優先度以下
                if neighbor_priority > current_priority
                    && edge_neighbor_priority[edge_a] <= current_priority
                    && edge_neighbor_priority[edge_b] <= current_priority
                {
                    let (_, corner_tex, _) = border_textures(&neighbor, &game_assets);
                    if let Some(texture) = corner_tex {
                        commands.spawn((
                            TerrainBorder,
                            Sprite {
                                image: texture,
                                custom_size: Some(Vec2::splat(TILE_SIZE)),
                                ..default()
                            },
                            Transform::from_xyz(pos.x, pos.y, neighbor.z_layer())
                                .with_rotation(Quat::from_rotation_z(angle)),
                        ));
                    }
                }

                // 内角: 隣接2辺が両方とも高優先度
                if edge_neighbor_priority[edge_a] > current_priority
                    && edge_neighbor_priority[edge_b] > current_priority
                {
                    // 2辺のうち高い方の地形テクスチャを使用
                    let dominant_priority =
                        edge_neighbor_priority[edge_a].max(edge_neighbor_priority[edge_b]);
                    let dominant_terrain = if edge_neighbor_priority[edge_a] == dominant_priority {
                        let na =
                            world_map.pos_to_idx(x + EDGE_DIRS[edge_a].0, y + EDGE_DIRS[edge_a].1);
                        na.map(|i| world_map.tiles[i])
                    } else {
                        let nb =
                            world_map.pos_to_idx(x + EDGE_DIRS[edge_b].0, y + EDGE_DIRS[edge_b].1);
                        nb.map(|i| world_map.tiles[i])
                    };
                    if let Some(terrain) = dominant_terrain {
                        let (_, _, inner_tex) = border_textures(&terrain, &game_assets);
                        if let Some(texture) = inner_tex {
                            commands.spawn((
                                TerrainBorder,
                                Sprite {
                                    image: texture,
                                    custom_size: Some(Vec2::splat(TILE_SIZE)),
                                    ..default()
                                },
                                Transform::from_xyz(pos.x, pos.y, terrain.z_layer())
                                    .with_rotation(Quat::from_rotation_z(angle)),
                            ));
                        }
                    }
                }
            }
        }
    }

    info!("BEVY_STARTUP: Terrain border overlays spawned");
}

/// 地形タイプに対応する (edge, corner, inner_corner) テクスチャを返す。
/// River は最低優先度なのでオーバーレイ不要で None を返す。
fn border_textures(
    terrain: &TerrainType,
    assets: &GameAssets,
) -> (
    Option<Handle<Image>>,
    Option<Handle<Image>>,
    Option<Handle<Image>>,
) {
    match terrain {
        TerrainType::Grass => (
            Some(assets.grass_edge.clone()),
            Some(assets.grass_corner.clone()),
            Some(assets.grass_inner_corner.clone()),
        ),
        TerrainType::Dirt => (
            Some(assets.dirt_edge.clone()),
            Some(assets.dirt_corner.clone()),
            Some(assets.dirt_inner_corner.clone()),
        ),
        TerrainType::Sand => (
            Some(assets.sand_edge.clone()),
            Some(assets.sand_corner.clone()),
            Some(assets.sand_inner_corner.clone()),
        ),
        TerrainType::River => (None, None, None),
    }
}
