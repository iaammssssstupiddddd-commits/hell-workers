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
    masks.fill_terrain_zones_from_seed(master_seed);

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
                terrain_tiles: fallback_terrain(&masks, master_seed),
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
                // inland_sand_mask 上も Sand は合法（MS-WFC-2.5 以降）
                let in_legal_mask = layout.masks.final_sand_mask.get((x, y))
                    || layout.masks.inland_sand_mask.get((x, y));
                // final_sand_mask 上のセルは必ず Sand
                if layout.masks.final_sand_mask.get((x, y)) {
                    assert!(
                        is_sand,
                        "final_sand_mask=true but terrain is not Sand at ({x},{y})"
                    );
                }
                // Sand セルは必ずどちらかのマスク内
                if is_sand {
                    assert!(
                        in_legal_mask,
                        "Sand at ({x},{y}) is outside both final_sand_mask and inland_sand_mask"
                    );
                }
            }
        }
    }
}

#[cfg(test)]
mod tile_dist_sim {
    use super::*;
    use hw_core::constants::{MAP_HEIGHT, MAP_WIDTH};
    use crate::terrain::TerrainType;
    use crate::terrain_zones::compute_anchor_distance_field;

    #[test]
    fn print_tile_distribution() {
        let seeds = [0u64, 42, 99, 12345];
        println!("\n=== Tile Distribution per Zone Category ===");
        println!("{:>8}  {:>10} {:>10} {:>10} {:>10} {:>10}",
            "seed", "area", "grass%", "dirt%", "sand%", "river%");

        for category in &["dirt_zone", "grass_zone", "neutral", "all_non_river"] {
            let mut g_sum = 0usize; let mut d_sum = 0usize;
            let mut s_sum = 0usize; let mut total_sum = 0usize;
            for &seed in &seeds {
                let layout = generate_world_layout(seed);
                let masks = &layout.masks;
                for y in 0..MAP_HEIGHT { for x in 0..MAP_WIDTH {
                    let p = (x, y);
                    if masks.river_mask.get(p) { continue; }
                    let in_cat = match *category {
                        "dirt_zone"      => masks.dirt_zone_mask.get(p),
                        "grass_zone"     => masks.grass_zone_mask.get(p),
                        "neutral"        => !masks.dirt_zone_mask.get(p) && !masks.grass_zone_mask.get(p),
                        _                => true,
                    };
                    if !in_cat { continue; }
                    let idx = (y * MAP_WIDTH + x) as usize;
                    match layout.terrain_tiles[idx] {
                        TerrainType::Grass => g_sum += 1,
                        TerrainType::Dirt  => d_sum += 1,
                        TerrainType::Sand  => s_sum += 1,
                        _ => {}
                    }
                    total_sum += 1;
                }}
            }
            if total_sum == 0 { continue; }
            println!("{:>12}  {:>10} {:>9}% {:>9}% {:>9}%",
                category,
                total_sum / seeds.len(),
                g_sum * 100 / total_sum,
                d_sum * 100 / total_sum,
                s_sum * 100 / total_sum);
        }
    }

    /// ゾーン / C グラデーション / 完全ニュートラルの割合を表示する
    #[test]
    fn print_neutral_breakdown() {
        let seeds = [0u64, 42, 99, 12345];
        use crate::terrain_zones::ZONE_GRADIENT_WIDTH;

        let mut zone_sum = 0usize;
        let mut gradient_sum = 0usize;
        let mut pure_neutral_sum = 0usize;
        let mut total_sum = 0usize;

        for &seed in &seeds {
            let layout = generate_world_layout(seed);
            let masks = &layout.masks;
            for y in 0..MAP_HEIGHT { for x in 0..MAP_WIDTH {
                let p = (x, y);
                if masks.river_mask.get(p) || masks.anchor_mask.get(p) { continue; }
                total_sum += 1;
                if masks.dirt_zone_mask.get(p) || masks.grass_zone_mask.get(p) {
                    zone_sum += 1;
                } else {
                    let idx = (y * MAP_WIDTH + x) as usize;
                    let dd = masks.dirt_zone_distance_field[idx];
                    let gd = masks.grass_zone_distance_field[idx];
                    if dd <= ZONE_GRADIENT_WIDTH || gd <= ZONE_GRADIENT_WIDTH {
                        gradient_sum += 1;
                    } else {
                        pure_neutral_sum += 1;
                    }
                }
            }}
        }
        let n = seeds.len();
        println!("\n=== Zone / Gradient / Pure-Neutral Breakdown (avg over {} seeds) ===", n);
        println!("  zone          : {:>5} cells ({:.1}%)", zone_sum/n, zone_sum*100/total_sum);
        println!("  C gradient    : {:>5} cells ({:.1}%)", gradient_sum/n, gradient_sum*100/total_sum);
        println!("  pure neutral  : {:>5} cells ({:.1}%)", pure_neutral_sum/n, pure_neutral_sum*100/total_sum);
        println!("  total non-river/anchor: {:>5}", total_sum/n);
    }

    /// 距離帯ごとにゾーンカバレッジを表示する（中立帯の構造的空白を可視化）
    #[test]
    fn print_zone_coverage_by_distance() {
        let seeds = [0u64, 42, 99, 12345];
        println!("\n=== Zone Coverage by Distance Band (avg over {} seeds) ===", seeds.len());
        println!("{:>10}  {:>8} {:>10} {:>11} {:>9}",
            "dist_range", "cells", "dirt_zone%", "grass_zone%", "neutral%");

        let bands: &[(u32, u32)] = &[
            (0, 4), (5, 8), (9, 12), (13, 15), (16, 19), (20, 24), (25, 30), (31, 50), (51, 99),
        ];
        // 距離場はシード非依存（anchor_maskが同一）なので1度だけ計算
        let dist_field = {
            let layout = generate_world_layout(seeds[0]);
            compute_anchor_distance_field(&layout.masks.anchor_mask)
        };
        for &(d_min, d_max) in bands {
            let mut total_sum = 0usize;
            let mut dirt_sum = 0usize;
            let mut grass_sum = 0usize;
            for &seed in &seeds {
                let layout = generate_world_layout(seed);
                let masks = &layout.masks;
                for y in 0..MAP_HEIGHT { for x in 0..MAP_WIDTH {
                    let p = (x, y);
                    if masks.anchor_mask.get(p) || masks.river_mask.get(p) { continue; }
                    let d = dist_field[(y * MAP_WIDTH + x) as usize];
                    if d < d_min || d > d_max { continue; }
                    total_sum += 1;
                    if masks.dirt_zone_mask.get(p) { dirt_sum += 1; }
                    if masks.grass_zone_mask.get(p) { grass_sum += 1; }
                }}
            }
            if total_sum == 0 { continue; }
            let neutral_sum = total_sum - dirt_sum - grass_sum;
            println!("{:>5}..{:<4}  {:>8} {:>9}% {:>10}% {:>8}%",
                d_min, d_max,
                total_sum / seeds.len(),
                dirt_sum * 100 / total_sum,
                grass_sum * 100 / total_sum,
                neutral_sum * 100 / total_sum);
        }
    }
}
