//! スポーン用の近傍歩行可能マス探索

use crate::world::map::WorldMap;

/// 指定グリッドを中心に、半径 `max_radius` の正方形範囲を走査し、
/// 最初に見つかった歩行可能なグリッドを返す。見つからなければ `center` を返す。
pub fn find_nearby_walkable_grid(
    center: (i32, i32),
    world_map: &WorldMap,
    max_radius: i32,
) -> (i32, i32) {
    for dx in -max_radius..=max_radius {
        for dy in -max_radius..=max_radius {
            let test = (center.0 + dx, center.1 + dy);
            if world_map.is_walkable(test.0, test.1) {
                return test;
            }
        }
    }
    center
}
