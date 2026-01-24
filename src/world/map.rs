use crate::assets::GameAssets;
use crate::constants::*;
use bevy::prelude::*;
use std::collections::HashMap;

/// 川の基本幅
pub const RIVER_WIDTH: i32 = 5;
/// 砂浜の幅
pub const SAND_WIDTH: i32 = 2;

/// 固定位置の木の座標リスト（森林エリア: マップ左上付近）
pub const TREE_POSITIONS: &[(i32, i32)] = &[
    (12, 85), (15, 88), (18, 82), (20, 90), (22, 87),
    (25, 83), (14, 78), (17, 75), (28, 85), (30, 80),
    (13, 72), (16, 70), (19, 73), (24, 76), (27, 78),
    (32, 88), (35, 85), (38, 82), (40, 75), (42, 68),
];

/// 固定位置の岩の座標リスト（岩石エリア: マップ右上付近）
pub const ROCK_POSITIONS: &[(i32, i32)] = &[
    (72, 85), (75, 88), (78, 82), (80, 90), (82, 87),
    (85, 83), (74, 78), (77, 75), (88, 85), (90, 80),
    (65, 85), (68, 82), (70, 78), (85, 72), (92, 75),
];

/// 初期配置の木材アイテムの座標リスト
pub const INITIAL_WOOD_POSITIONS: &[(i32, i32)] = &[
    (45, 45), (46, 47), (48, 44), (52, 48), (55, 45)
];

#[derive(Component)]
pub struct Tile;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerrainType {
    Grass,
    Dirt,
    Stone,
    River,
    Sand,
}

impl TerrainType {
    pub fn is_walkable(&self) -> bool {
        match self {
            TerrainType::Grass | TerrainType::Dirt | TerrainType::Sand => true,
            TerrainType::Stone | TerrainType::River => false,
        }
    }
}

#[derive(Resource, Default)]
pub struct WorldMap {
    pub tiles: HashMap<(i32, i32), TerrainType>,
    pub buildings: HashMap<(i32, i32), Entity>,
    pub stockpiles: HashMap<(i32, i32), Entity>,
}

impl WorldMap {
    pub fn is_walkable(&self, x: i32, y: i32) -> bool {
        if x < 0 || x >= MAP_WIDTH || y < 0 || y >= MAP_HEIGHT {
            return false;
        }
        self.tiles.get(&(x, y)).map_or(false, |t| t.is_walkable())
    }

    pub fn world_to_grid(pos: Vec2) -> (i32, i32) {
        // (MAP_WIDTH - 1) / 2.0 = 24.5 を中心(0,0)とする計算
        let x = (pos.x / TILE_SIZE + (MAP_WIDTH as f32 - 1.0) / 2.0 + 0.5).floor() as i32;
        let y = (pos.y / TILE_SIZE + (MAP_HEIGHT as f32 - 1.0) / 2.0 + 0.5).floor() as i32;
        (x, y)
    }

    pub fn grid_to_world(x: i32, y: i32) -> Vec2 {
        Vec2::new(
            (x as f32 - (MAP_WIDTH as f32 - 1.0) / 2.0) * TILE_SIZE,
            (y as f32 - (MAP_HEIGHT as f32 - 1.0) / 2.0) * TILE_SIZE,
        )
    }
}

pub fn spawn_map(
    mut commands: Commands,
    game_assets: Res<GameAssets>,
    mut world_map: ResMut<WorldMap>,
) {
    use crate::world::river::{generate_river_tiles, generate_sand_tiles};

    // 川と砂のタイルを事前計算
    let river_tiles = generate_river_tiles(MAP_WIDTH, MAP_HEIGHT, RIVER_WIDTH);
    let sand_tiles = generate_sand_tiles(&river_tiles, MAP_WIDTH, SAND_WIDTH);

    for y in 0..MAP_HEIGHT {
        for x in 0..MAP_WIDTH {
            let (terrain, texture) = if river_tiles.contains(&(x, y)) {
                (TerrainType::River, game_assets.river.clone())
            } else if sand_tiles.contains(&(x, y)) {
                (TerrainType::Sand, game_assets.sand.clone())
            } else if x > 70 && y > 70 && (x * y) % 13 == 0 {
                // 岩石エリアに石タイルを点在させる
                (TerrainType::Stone, game_assets.stone.clone())
            } else if (x + y) % 30 == 0 {
                // 稀に土を混ぜる
                (TerrainType::Dirt, game_assets.dirt.clone())
            } else {
                (TerrainType::Grass, game_assets.grass.clone())
            };

            world_map.tiles.insert((x, y), terrain);

            let pos = WorldMap::grid_to_world(x, y);
            commands.spawn((
                Tile,
                Sprite {
                    image: texture,
                    custom_size: Some(Vec2::splat(TILE_SIZE)),
                    ..default()
                },
                Transform::from_xyz(pos.x, pos.y, Z_MAP),
            ));
        }
    }

    info!(
        "BEVY_STARTUP: Map spawned ({}x{} tiles, river generated)",
        MAP_WIDTH, MAP_HEIGHT
    );
}
