pub mod types;
pub mod wfc_adapter;

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

/// MS-WFC-2a 用の一時スタブ地形。
///
/// WFC ソルバーはまだ使わず、legacy の Grass/Dirt パターンを維持しつつ、
/// `masks.river_mask` から River を、そこから導出した帯から Sand を配置する。
/// これにより `GeneratedWorldLayout.terrain_tiles` と `GeneratedWorldLayout.masks`
/// が同じ世界を表す状態を保つ。
fn generate_stub_terrain_tiles_from_masks(
    masks: &crate::world_masks::WorldMasks,
    map_width: i32,
    map_height: i32,
    sand_width: i32,
) -> Vec<TerrainType> {
    use std::collections::HashSet;

    let mut river_tiles = HashSet::new();
    for y in 0..map_height {
        for x in 0..map_width {
            if masks.river_mask.get((x, y)) {
                river_tiles.insert((x, y));
            }
        }
    }

    let sand_tiles = generate_sand_tiles(&river_tiles, map_height, sand_width);
    let mut tiles = vec![TerrainType::Grass; (map_width * map_height) as usize];

    for y in 0..map_height {
        for x in 0..map_width {
            let pos = (x, y);
            let terrain = if masks.river_mask.get(pos) {
                TerrainType::River
            } else if sand_tiles.contains(&pos) && !masks.anchor_mask.get(pos) {
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
/// MS-WFC-2a: anchor/protection-band/river_mask を含む正しい masks を生成して返す。
/// terrain_tiles は MS-WFC-2b まで WFC 未使用の簡易スタブだが、
/// river/sand 配置は `masks` と整合するように保つ。
pub fn generate_world_layout(master_seed: u64) -> types::GeneratedWorldLayout {
    use crate::anchor::AnchorLayout;
    use crate::layout::SAND_WIDTH;
    use crate::world_masks::WorldMasks;
    use hw_core::constants::{MAP_HEIGHT, MAP_WIDTH};
    use types::ResourceSpawnCandidates;

    let anchors = AnchorLayout::fixed();
    let mut masks = WorldMasks::from_anchor(&anchors);
    masks.fill_river_from_seed(master_seed);

    types::GeneratedWorldLayout {
        terrain_tiles: generate_stub_terrain_tiles_from_masks(&masks, MAP_WIDTH, MAP_HEIGHT, SAND_WIDTH),
        anchors,
        masks,
        resource_spawn_candidates: ResourceSpawnCandidates::default(),
        initial_tree_positions: Vec::new(),
        forest_regrowth_zones: Vec::new(),
        initial_rock_positions: Vec::new(),
        master_seed,
        generation_attempt: 0,
        used_fallback: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hw_core::constants::{MAP_HEIGHT, MAP_WIDTH};

    #[test]
    fn generated_world_layout_river_mask_matches_terrain_tiles() {
        let layout = generate_world_layout(42);

        for y in 0..MAP_HEIGHT {
            for x in 0..MAP_WIDTH {
                let idx = (y * MAP_WIDTH + x) as usize;
                let terrain_is_river = layout.terrain_tiles[idx] == TerrainType::River;
                let mask_is_river = layout.masks.river_mask.get((x, y));
                assert_eq!(
                    terrain_is_river, mask_is_river,
                    "terrain/mask river mismatch at ({x}, {y})"
                );
            }
        }
    }
}
