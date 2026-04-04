pub mod types;
pub mod wfc_adapter;

use crate::river::{generate_fixed_river_tiles, generate_sand_tiles};
use crate::terrain::TerrainType;

/// レガシー固定地形生成（`GeneratedWorldLayout::stub` および visual_test で使用）。
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

/// WFC 地形生成のエントリポイント（MS-WFC-2b）。
///
/// `WorldMasks`（anchor + river_mask）を構築し、WFC ソルバーで地形グリッドを生成する。
/// 収束失敗時は `MAX_WFC_RETRIES` まで deterministic retry し、それでも失敗した場合のみ
/// fallback（River マスクを維持した Grass マップ）を返す。
pub fn generate_world_layout(master_seed: u64) -> types::GeneratedWorldLayout {
    use crate::anchor::AnchorLayout;
    use crate::world_masks::WorldMasks;
    use types::ResourceSpawnCandidates;
    use wfc_adapter::{derive_sub_seed, fallback_terrain, run_wfc, MAX_WFC_RETRIES};

    let anchors = AnchorLayout::fixed();
    let mut masks = WorldMasks::from_anchor(&anchors);
    masks.fill_river_from_seed(master_seed);

    let (terrain_tiles, attempt, used_fallback) = (0..=MAX_WFC_RETRIES)
        .find_map(|attempt| {
            let sub_seed = derive_sub_seed(master_seed, attempt);
            run_wfc(&masks, sub_seed, attempt)
                .ok()
                .map(|tiles| (tiles, attempt, false))
        })
        .unwrap_or_else(|| {
            // debug でも release でもフォールバックは続行する（`debug_assert!(false)` は使わない）
            eprintln!("WFC: fallback terrain used for master_seed={master_seed}");
            (fallback_terrain(&masks), MAX_WFC_RETRIES + 1, true)
        });

    types::GeneratedWorldLayout {
        terrain_tiles,
        anchors,
        masks,
        resource_spawn_candidates: ResourceSpawnCandidates::default(),
        initial_tree_positions: Vec::new(),
        forest_regrowth_zones: Vec::new(),
        initial_rock_positions: Vec::new(),
        master_seed,
        generation_attempt: attempt,
        used_fallback,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hw_core::constants::{MAP_HEIGHT, MAP_WIDTH};

    const TEST_SEED_A: u64 = 42;
    const TEST_SEED_B: u64 = 12_345_678;

    #[test]
    fn generated_world_layout_river_mask_matches_terrain_tiles() {
        let layout = generate_world_layout(TEST_SEED_A);

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

    #[test]
    fn test_wfc_determinism() {
        let layout1 = generate_world_layout(TEST_SEED_A);
        let layout2 = generate_world_layout(TEST_SEED_A);
        assert_eq!(layout1.terrain_tiles, layout2.terrain_tiles);
    }

    #[test]
    fn test_wfc_different_seeds_differ() {
        let layout_a = generate_world_layout(TEST_SEED_A);
        let layout_b = generate_world_layout(TEST_SEED_B);
        assert_ne!(layout_a.terrain_tiles, layout_b.terrain_tiles);
    }

    #[test]
    fn test_site_yard_no_river_sand() {
        let layout = generate_world_layout(TEST_SEED_A);
        for y in 0..MAP_HEIGHT {
            for x in 0..MAP_WIDTH {
                if layout.masks.anchor_mask.get((x, y)) {
                    let tile = layout.terrain_tiles[(y * MAP_WIDTH + x) as usize];
                    assert!(
                        !matches!(tile, TerrainType::River | TerrainType::Sand),
                        "anchor cell ({x},{y}) has forbidden terrain {tile:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn test_river_stays_in_mask() {
        let layout = generate_world_layout(TEST_SEED_A);
        for y in 0..MAP_HEIGHT {
            for x in 0..MAP_WIDTH {
                let is_river_tile =
                    layout.terrain_tiles[(y * MAP_WIDTH + x) as usize] == TerrainType::River;
                let in_river_mask = layout.masks.river_mask.get((x, y));
                assert_eq!(
                    is_river_tile, in_river_mask,
                    "river mask mismatch at ({x},{y}): tile={is_river_tile}, mask={in_river_mask}"
                );
            }
        }
    }

    #[test]
    fn test_sand_is_only_river_adjacent_in_ms2b() {
        use wfc_adapter::CARDINAL_DIRS;

        let layout = generate_world_layout(TEST_SEED_A);
        for y in 0..MAP_HEIGHT {
            for x in 0..MAP_WIDTH {
                let idx = (y * MAP_WIDTH + x) as usize;
                if layout.terrain_tiles[idx] != TerrainType::Sand {
                    continue;
                }
                assert!(
                    !layout.masks.anchor_mask.get((x, y)),
                    "sand must not appear in anchor at ({x},{y})"
                );
                let is_river_adjacent = CARDINAL_DIRS
                    .iter()
                    .any(|(dx, dy)| layout.masks.river_mask.get((x + dx, y + dy)));
                assert!(
                    is_river_adjacent,
                    "sand at ({x},{y}) is not cardinally adjacent to river"
                );
            }
        }
    }
}
