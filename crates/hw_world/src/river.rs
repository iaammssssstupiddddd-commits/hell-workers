use crate::layout::{RIVER_X_MAX, RIVER_X_MIN, RIVER_Y_MAX, RIVER_Y_MIN};
use std::collections::HashSet;

/// 固定配置の川タイルを生成
pub fn generate_fixed_river_tiles() -> HashSet<(i32, i32)> {
    let mut river_tiles = HashSet::new();
    for y in RIVER_Y_MIN..=RIVER_Y_MAX {
        for x in RIVER_X_MIN..=RIVER_X_MAX {
            river_tiles.insert((x, y));
        }
    }
    river_tiles
}

/// 川の上下（南北）に砂を配置
pub fn generate_sand_tiles(
    river_tiles: &HashSet<(i32, i32)>,
    map_height: i32,
    sand_width: i32,
) -> HashSet<(i32, i32)> {
    let mut sand_tiles = HashSet::new();

    for &(rx, ry) in river_tiles {
        for dy in -sand_width..=sand_width {
            let y = ry + dy;
            if y >= 0 && y < map_height && !river_tiles.contains(&(rx, y)) {
                sand_tiles.insert((rx, y));
            }
        }
    }

    sand_tiles
}
