use noise::{NoiseFn, Perlin};
use std::collections::HashSet;

/// パーリンノイズで蛇行する川のタイル座標を生成
pub fn generate_river_tiles(
    map_width: i32,
    map_height: i32,
    river_width: i32,
) -> HashSet<(i32, i32)> {
    let perlin = Perlin::new(42); // 固定シード
    let mut river_tiles = HashSet::new();
    
    // 川は上から下に流れる（y=0 から y=map_height-1）
    for y in 0..map_height {
        // パーリンノイズで x 座標を蛇行させる
        // 周波数を下げて緩やかな蛇行にする
        let noise_value = perlin.get([y as f64 * 0.03, 0.0]);
        let center_x = (map_width / 2) as f64 + noise_value * (map_width as f64 * 0.2);
        
        // 川幅を考慮してタイルを追加
        for dx in -(river_width/2)..=(river_width/2) {
            let x = (center_x + dx as f64).round() as i32;
            if x >= 0 && x < map_width {
                river_tiles.insert((x, y));
            }
        }
    }
    
    river_tiles
}

/// 川の両岸に砂を配置
pub fn generate_sand_tiles(
    river_tiles: &HashSet<(i32, i32)>,
    map_width: i32,
    sand_width: i32,
) -> HashSet<(i32, i32)> {
    let mut sand_tiles = HashSet::new();
    
    for &(rx, ry) in river_tiles {
        // 川の左右 sand_width マスに砂を配置
        for dx in -sand_width..=sand_width {
            let x = rx + dx;
            if x >= 0 && x < map_width && !river_tiles.contains(&(x, ry)) {
                sand_tiles.insert((x, ry));
            }
        }
    }
    
    sand_tiles
}
