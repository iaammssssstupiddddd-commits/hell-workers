use crate::assets::GameAssets;
use crate::constants::*;
use bevy::prelude::*;
use std::collections::HashMap;

/// 砂浜の幅
pub const SAND_WIDTH: i32 = 2;

// ============================================================
// 川タイルの範囲（東西に横断）
// ============================================================
/// 川タイルのY範囲（y=65-69に配置）
pub const RIVER_Y_MIN: i32 = 65;
pub const RIVER_Y_MAX: i32 = 69;
/// 川タイルのX範囲（マップ全幅を横断）
pub const RIVER_X_MIN: i32 = 0;
pub const RIVER_X_MAX: i32 = 99;

// ============================================================
// 森林エリア（マップ西側）- 散らばり配置
// ============================================================
/// 全木の座標 - 散らばり配置
/// - 小森林A: 16本
/// - 小森林B: 16本
/// - 大森林: 60本（川の向こう）
/// - まばらな木: 20本
pub const TREE_POSITIONS: &[(i32, i32)] = &[
    // 小森林A - 散らばり (16本)
    (18, 48), (22, 53), (26, 47), (20, 56), (25, 50), (29, 55), (17, 52), (24, 58),
    (21, 45), (27, 51), (19, 54), (23, 46), (28, 54), (16, 49), (30, 48), (22, 57),
    // 小森林B - 散らばり (16本)
    (17, 24), (21, 29), (25, 23), (19, 32), (24, 26), (28, 31), (16, 28), (23, 34),
    (20, 21), (26, 27), (18, 30), (22, 22), (27, 30), (15, 25), (29, 24), (21, 33),
    // 大森林（川の向こう）- 散らばり (60本)
    (12, 78), (18, 83), (24, 77), (16, 88), (22, 82), (28, 87), (14, 81), (20, 94),
    (10, 85), (26, 80), (32, 85), (15, 90), (21, 76), (27, 91), (13, 86), (19, 79),
    (25, 93), (31, 78), (11, 82), (17, 95), (23, 84), (29, 89), (35, 80), (38, 86),
    (40, 82), (36, 92), (33, 88), (30, 76), (37, 79), (34, 94), (39, 88), (41, 84),
    (14, 93), (20, 87), (26, 77), (32, 91), (18, 81), (24, 95), (30, 83), (36, 77),
    (12, 89), (22, 86), (28, 78), (34, 84), (16, 76), (38, 90), (40, 78), (42, 88),
    (13, 84), (19, 92), (25, 79), (31, 93), (37, 83), (15, 77), (21, 89), (27, 82),
    (33, 77), (39, 93), (35, 87), (41, 81),
    // まばらな木（マップ各所）(20本)
    (45, 55), (55, 45), (60, 30), (40, 25), (50, 15), (35, 10), (65, 55), (70, 45),
    (8, 50), (5, 40), (92, 50), (95, 40), (50, 5), (45, 8), (55, 12), (38, 58),
    (42, 42), (58, 38), (12, 35), (88, 35),
];

// ============================================================
// 岩エリア（マップ東側）- 完全な塊
// ============================================================
/// 全岩の座標 - 隙間なし塊配置
/// - 小岩A: 5x5=25個
/// - 小岩B: 5x5=25個
/// - 大岩場: 10x10=100個（川の向こう）
pub const ROCK_POSITIONS: &[(i32, i32)] = &[
    // 小岩A - 5x5の塊 (25個) 中心(77, 52)
    (75, 50), (76, 50), (77, 50), (78, 50), (79, 50),
    (75, 51), (76, 51), (77, 51), (78, 51), (79, 51),
    (75, 52), (76, 52), (77, 52), (78, 52), (79, 52),
    (75, 53), (76, 53), (77, 53), (78, 53), (79, 53),
    (75, 54), (76, 54), (77, 54), (78, 54), (79, 54),
    // 小岩B - 5x5の塊 (25個) 中心(75, 27)
    (73, 25), (74, 25), (75, 25), (76, 25), (77, 25),
    (73, 26), (74, 26), (75, 26), (76, 26), (77, 26),
    (73, 27), (74, 27), (75, 27), (76, 27), (77, 27),
    (73, 28), (74, 28), (75, 28), (76, 28), (77, 28),
    (73, 29), (74, 29), (75, 29), (76, 29), (77, 29),
    // 大岩場（川の向こう）- 10x10の塊 (100個) 左下(78, 80)
    (78, 80), (79, 80), (80, 80), (81, 80), (82, 80), (83, 80), (84, 80), (85, 80), (86, 80), (87, 80),
    (78, 81), (79, 81), (80, 81), (81, 81), (82, 81), (83, 81), (84, 81), (85, 81), (86, 81), (87, 81),
    (78, 82), (79, 82), (80, 82), (81, 82), (82, 82), (83, 82), (84, 82), (85, 82), (86, 82), (87, 82),
    (78, 83), (79, 83), (80, 83), (81, 83), (82, 83), (83, 83), (84, 83), (85, 83), (86, 83), (87, 83),
    (78, 84), (79, 84), (80, 84), (81, 84), (82, 84), (83, 84), (84, 84), (85, 84), (86, 84), (87, 84),
    (78, 85), (79, 85), (80, 85), (81, 85), (82, 85), (83, 85), (84, 85), (85, 85), (86, 85), (87, 85),
    (78, 86), (79, 86), (80, 86), (81, 86), (82, 86), (83, 86), (84, 86), (85, 86), (86, 86), (87, 86),
    (78, 87), (79, 87), (80, 87), (81, 87), (82, 87), (83, 87), (84, 87), (85, 87), (86, 87), (87, 87),
    (78, 88), (79, 88), (80, 88), (81, 88), (82, 88), (83, 88), (84, 88), (85, 88), (86, 88), (87, 88),
    (78, 89), (79, 89), (80, 89), (81, 89), (82, 89), (83, 89), (84, 89), (85, 89), (86, 89), (87, 89),
];

/// 初期配置の木材アイテムの座標リスト（開始地点周辺）
pub const INITIAL_WOOD_POSITIONS: &[(i32, i32)] = &[
    (48, 48), (52, 52), (47, 51), (53, 49), (50, 46)
];

#[derive(Component)]
pub struct Tile;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerrainType {
    Grass,
    Dirt,
    River,
    Sand,
}

impl TerrainType {
    pub fn is_walkable(&self) -> bool {
        match self {
            TerrainType::Grass | TerrainType::Dirt | TerrainType::Sand => true,
            TerrainType::River => false,
        }
    }
}

#[derive(Resource)]
pub struct WorldMap {
    pub tiles: Vec<TerrainType>,
    pub tile_entities: Vec<Option<Entity>>,
    pub buildings: HashMap<(i32, i32), Entity>,
    pub stockpiles: HashMap<(i32, i32), Entity>,
    /// 障害物（Rock, Treeなど）の座標
    pub obstacles: Vec<bool>,
}

impl Default for WorldMap {
    fn default() -> Self {
        let size = (MAP_WIDTH * MAP_HEIGHT) as usize;
        Self {
            tiles: vec![TerrainType::Grass; size],
            tile_entities: vec![None; size],
            buildings: HashMap::new(),
            stockpiles: HashMap::new(),
            obstacles: vec![false; size],
        }
    }
}

impl WorldMap {
    #[inline(always)]
    pub fn pos_to_idx(&self, x: i32, y: i32) -> Option<usize> {
        if x < 0 || x >= MAP_WIDTH || y < 0 || y >= MAP_HEIGHT {
            return None;
        }
        Some((y * MAP_WIDTH + x) as usize)
    }

    #[inline(always)]
    pub fn idx_to_pos(idx: usize) -> (i32, i32) {
        let x = idx as i32 % MAP_WIDTH;
        let y = idx as i32 / MAP_WIDTH;
        (x, y)
    }

    pub fn is_walkable(&self, x: i32, y: i32) -> bool {
        let idx = match self.pos_to_idx(x, y) {
            Some(i) => i,
            None => return false,
        };
        // 障害物があれば通行不可
        if self.obstacles[idx] {
            return false;
        }
        self.tiles[idx].is_walkable()
    }
    
    /// 障害物を追加
    pub fn add_obstacle(&mut self, x: i32, y: i32) {
        if let Some(idx) = self.pos_to_idx(x, y) {
            self.obstacles[idx] = true;
        }
    }
    
    /// 障害物を削除
    pub fn remove_obstacle(&mut self, x: i32, y: i32) {
        if let Some(idx) = self.pos_to_idx(x, y) {
            self.obstacles[idx] = false;
        }
    }

    pub fn world_to_grid(pos: Vec2) -> (i32, i32) {
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

    /// ワールド座標が通行可能かチェック
    pub fn is_walkable_world(&self, pos: Vec2) -> bool {
        let grid = Self::world_to_grid(pos);
        self.is_walkable(grid.0, grid.1)
    }

    /// 指定位置の近くにある通行可能なグリッドを探す
    pub fn get_nearest_walkable_grid(&self, pos: Vec2) -> Option<(i32, i32)> {
        let grid = Self::world_to_grid(pos);
        if self.is_walkable(grid.0, grid.1) {
            return Some(grid);
        }

        // 周辺3マスまで探索
        for r in 1..=3 {
            for dx in -r..=r {
                for dy in -r..=r {
                    let test = (grid.0 + dx, grid.1 + dy);
                    if self.is_walkable(test.0, test.1) {
                        return Some(test);
                    }
                }
            }
        }
        None
    }

    /// 2点間に障害物がないか（Line-of-Sight）を判定
    pub fn has_line_of_sight(&self, p1: (i32, i32), p2: (i32, i32)) -> bool {
        let (x1, y1) = p1;
        let (x2, y2) = p2;

        let dx = (x2 - x1).abs();
        let dy = (y2 - y1).abs();
        let mut x = x1;
        let mut y = y1;
        let n = 1 + dx + dy;
        let x_inc = if x2 > x1 { 1 } else { -1 };
        let y_inc = if y2 > y1 { 1 } else { -1 };
        let mut error = dx - dy;
        let dx_twice = dx * 2;
        let dy_twice = dy * 2;

        for _ in 0..n {
            if !self.is_walkable(x, y) {
                return false;
            }
            if x == x2 && y == y2 {
                break;
            }

            if error > 0 {
                x += x_inc;
                error -= dy_twice;
            } else if error < 0 {
                y += y_inc;
                error += dx_twice;
            } else {
                // error == 0 の場合（ど真ん中を通る場合）
                // 角抜けを確実に防ぐため、隣接する両方のマスもチェックする
                if !self.is_walkable(x + x_inc, y) || !self.is_walkable(x, y + y_inc) {
                    return false;
                }
                x += x_inc;
                y += y_inc;
                error += dx_twice - dy_twice;
            }
        }
        true
    }
}

/// 固定配置の川タイルを生成
pub fn generate_fixed_river_tiles() -> std::collections::HashSet<(i32, i32)> {
    let mut river_tiles = std::collections::HashSet::new();
    for y in RIVER_Y_MIN..=RIVER_Y_MAX {
        for x in RIVER_X_MIN..=RIVER_X_MAX {
            river_tiles.insert((x, y));
        }
    }
    river_tiles
}

pub fn spawn_map(
    mut commands: Commands,
    game_assets: Res<GameAssets>,
    mut world_map: ResMut<WorldMap>,
) {
    use crate::world::river::generate_sand_tiles;

    let river_tiles = generate_fixed_river_tiles();
    let sand_tiles = generate_sand_tiles(&river_tiles, MAP_HEIGHT, SAND_WIDTH);

    for y in 0..MAP_HEIGHT {
        for x in 0..MAP_WIDTH {
            let (terrain, texture) = if river_tiles.contains(&(x, y)) {
                (TerrainType::River, game_assets.river.clone())
            } else if sand_tiles.contains(&(x, y)) {
                (TerrainType::Sand, game_assets.sand.clone())
            } else if (x + y) % 30 == 0 {
                (TerrainType::Dirt, game_assets.dirt.clone())
            } else {
                (TerrainType::Grass, game_assets.grass.clone())
            };

            let idx = world_map.pos_to_idx(x, y).unwrap();
            world_map.tiles[idx] = terrain;

            let pos = WorldMap::grid_to_world(x, y);
            let entity = commands.spawn((
                Tile,
                Sprite {
                    image: texture,
                    custom_size: Some(Vec2::splat(TILE_SIZE)),
                    ..default()
                },
                Transform::from_xyz(pos.x, pos.y, Z_MAP),
            )).id();
            
            world_map.tile_entities[idx] = Some(entity);
        }
    }

    info!(
        "BEVY_STARTUP: Map spawned ({}x{} tiles, fixed river layout)",
        MAP_WIDTH, MAP_HEIGHT
    );
}
