pub mod types;

use crate::river::{generate_fixed_river_tiles, generate_sand_tiles};
use crate::terrain::TerrainType;

pub fn generate_base_terrain_tiles(
    map_width: i32,
    map_height: i32,
    sand_width: i32,
) -> Vec<TerrainType> {
    let river_tiles = generate_fixed_river_tiles();
    let sand_tiles = generate_sand_tiles(&river_tiles, map_height, sand_width);
    let mut tiles = vec![TerrainType::Grass; (map_width * map_height) as usize];

    for y in 0..map_height {
        for x in 0..map_width {
            let terrain = if river_tiles.contains(&(x, y)) {
                TerrainType::River
            } else if sand_tiles.contains(&(x, y)) {
                TerrainType::Sand
            } else if (x + y) % 30 == 0 {
                TerrainType::Dirt
            } else {
                TerrainType::Grass
            };
            tiles[(y * map_width + x) as usize] = terrain;
        }
    }

    tiles
}

/// WFC 地形生成のエントリポイント。
///
/// MS-WFC-2b 実装まではスタブを返す（固定地形 + 正しい anchor/mask のみ設定）。
pub fn generate_world_layout(master_seed: u64) -> types::GeneratedWorldLayout {
    types::GeneratedWorldLayout::stub(master_seed)
}
