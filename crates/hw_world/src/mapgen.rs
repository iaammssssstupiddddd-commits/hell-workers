pub mod types;
pub mod validate;
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

/// WFC 地形生成のエントリポイント（MS-WFC-2c）。
///
/// `WorldMasks`（anchor + river_mask）を構築し、WFC ソルバーで地形グリッドを生成する。
/// 収束失敗時は `MAX_WFC_RETRIES` まで deterministic retry し、retry 内で
/// `validate::lightweight_validate()` を通過したレイアウトのみ採用する。
/// 全試行で通過できない場合のみ fallback（River マスクを維持した Grass マップ）を返す。
pub fn generate_world_layout(master_seed: u64) -> types::GeneratedWorldLayout {
    use crate::anchor::AnchorLayout;
    use crate::world_masks::WorldMasks;
    use types::ResourceSpawnCandidates;
    use wfc_adapter::{derive_sub_seed, fallback_terrain, run_wfc, MAX_WFC_RETRIES};

    let anchors = AnchorLayout::fixed();
    let mut masks = WorldMasks::from_anchor(&anchors);
    masks.fill_river_from_seed(master_seed);
    masks.fill_sand_from_river_seed(master_seed);

    let layout = (0..=MAX_WFC_RETRIES)
        .find_map(|attempt| {
            let sub_seed = derive_sub_seed(master_seed, attempt);
            let terrain_tiles = run_wfc(&masks, sub_seed, attempt).ok()?;
            let candidate = types::GeneratedWorldLayout {
                terrain_tiles,
                anchors: anchors.clone(),
                masks: masks.clone(),
                resource_spawn_candidates: ResourceSpawnCandidates::default(),
                initial_tree_positions: Vec::new(),
                forest_regrowth_zones: Vec::new(),
                initial_rock_positions: Vec::new(),
                master_seed,
                generation_attempt: attempt,
                used_fallback: false,
            };
            match validate::lightweight_validate(&candidate) {
                Ok(resource_spawn_candidates) => Some(types::GeneratedWorldLayout {
                    resource_spawn_candidates,
                    ..candidate
                }),
                Err(err) => {
                    eprintln!("[WFC validate] attempt={attempt} seed={sub_seed}: {err}");
                    None
                }
            }
        })
        .unwrap_or_else(|| {
            // debug でも release でもフォールバックは続行する（`debug_assert!(false)` は使わない）
            eprintln!("WFC: fallback terrain used for master_seed={master_seed}");
            types::GeneratedWorldLayout {
                terrain_tiles: fallback_terrain(&masks),
                anchors,
                masks,
                resource_spawn_candidates: ResourceSpawnCandidates::default(),
                initial_tree_positions: Vec::new(),
                forest_regrowth_zones: Vec::new(),
                initial_rock_positions: Vec::new(),
                master_seed,
                generation_attempt: MAX_WFC_RETRIES + 1,
                used_fallback: true,
            }
        });

    #[cfg(any(test, debug_assertions))]
    {
        let warnings = validate::debug_validate(&layout);
        for w in &warnings {
            eprintln!("[WFC debug] {:?}: {}", w.kind, w.message);
        }
    }

    layout
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
    fn test_sand_matches_final_sand_mask() {
        let layout = generate_world_layout(TEST_SEED_A);
        for y in 0..MAP_HEIGHT {
            for x in 0..MAP_WIDTH {
                let idx = (y * MAP_WIDTH + x) as usize;
                let is_sand = layout.terrain_tiles[idx] == TerrainType::Sand;
                let in_mask = layout.masks.final_sand_mask.get((x, y));
                assert_eq!(
                    is_sand, in_mask,
                    "Sand/mask mismatch at ({x},{y}): terrain={is_sand}, mask={in_mask}"
                );
            }
        }
    }
}
