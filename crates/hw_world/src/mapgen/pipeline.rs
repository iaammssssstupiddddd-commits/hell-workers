//! WFC 地形生成パイプライン（`generate_world_layout`）。
//!
//! オーケストレーション本体・retry ループ・fallback 分岐を担う。
//! ソルバー詳細は [`super::wfc_adapter`]、バリデータは [`super::validate`]、
//! 資源配置は [`super::resources`] に委譲する。

use crate::anchor::AnchorLayout;
use crate::world_masks::WorldMasks;

use super::resources;
use super::types::GeneratedWorldLayout;
use super::validate;
use super::wfc_adapter::{MAX_WFC_RETRIES, derive_sub_seed, fallback_terrain, run_wfc};

/// WFC 地形生成のエントリポイント（MS-WFC-2c）。
///
/// `AnchorLayout::aligned_to_worldgen_seed(master_seed)` で Site/Yard を川の縦位置に合わせ、
/// `WorldMasks`（anchor + river_mask）を構築し、WFC ソルバーで地形グリッドを生成する。
/// 収束失敗時は `MAX_WFC_RETRIES` まで deterministic retry し、retry 内で
/// `validate::lightweight_validate()`・資源配置・`validate_post_resource` を通過したレイアウトのみ採用する。
/// 全試行で通過できない場合のみ fallback（River マスクを維持した Grass マップ）を返す。
pub fn generate_world_layout(master_seed: u64) -> GeneratedWorldLayout {
    let anchors = AnchorLayout::aligned_to_worldgen_seed(master_seed);
    let mut masks = WorldMasks::from_anchor(&anchors);
    masks.fill_river_from_seed(master_seed);
    masks.fill_sand_from_river_seed(master_seed);
    masks.fill_terrain_zones_from_seed(master_seed);
    masks.fill_rock_fields_from_seed(master_seed);

    let layout = (0..=MAX_WFC_RETRIES)
        .find_map(|attempt| {
            let sub_seed = derive_sub_seed(master_seed, attempt);
            let terrain_tiles = run_wfc(&masks, sub_seed, attempt).ok()?;

            // ─ Step 3: 地形フェーズ検証 ─
            let candidate =
                GeneratedWorldLayout::initial(terrain_tiles, anchors.clone(), masks.clone(), master_seed, attempt, false);
            let validated_candidates = validate::lightweight_validate(&candidate).ok()?;
            let candidate = GeneratedWorldLayout {
                resource_spawn_candidates: validated_candidates,
                ..candidate
            };

            // ─ Step 4: 資源配置 ─
            let res = resources::generate_resource_layout(&candidate, sub_seed)?;

            // ─ Step 5: 資源配置後の導線再確認 ─
            validate::validate_post_resource(&candidate, &res).ok()?;

            // ─ 採用 ─
            let water = candidate.resource_spawn_candidates.water_tiles.clone();
            let sand = candidate.resource_spawn_candidates.sand_tiles.clone();
            Some(candidate.with_resources(res, water, sand))
        })
        .unwrap_or_else(|| {
            eprintln!("WFC: fallback terrain used for master_seed={master_seed}");
            let fallback_candidate = GeneratedWorldLayout::initial(
                fallback_terrain(&masks, master_seed),
                anchors,
                masks,
                master_seed,
                MAX_WFC_RETRIES + 1,
                true,
            );
            let validated = validate::lightweight_validate(&fallback_candidate)
                .expect("fallback terrain must satisfy lightweight_validate");
            let fallback_candidate = GeneratedWorldLayout {
                resource_spawn_candidates: validated,
                ..fallback_candidate
            };
            let res =
                resources::generate_resource_layout_fallback(&fallback_candidate, master_seed)
                    .expect("fallback resource generation must not return empty world");
            validate::validate_post_resource(&fallback_candidate, &res)
                .expect("fallback resource layout must preserve required paths");
            let water = fallback_candidate.resource_spawn_candidates.water_tiles.clone();
            let sand = fallback_candidate.resource_spawn_candidates.sand_tiles.clone();
            fallback_candidate.with_resources(res, water, sand)
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
    use super::generate_world_layout;
    use crate::terrain::TerrainType;
    use crate::test_seeds::{GOLDEN_SEED_PRIMARY, GOLDEN_SEED_SECONDARY};
    use hw_core::constants::{MAP_HEIGHT, MAP_WIDTH};

    #[test]
    fn generated_world_layout_river_mask_matches_terrain_tiles() {
        let layout = generate_world_layout(GOLDEN_SEED_PRIMARY);

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
        let layout1 = generate_world_layout(GOLDEN_SEED_PRIMARY);
        let layout2 = generate_world_layout(GOLDEN_SEED_PRIMARY);
        assert_eq!(layout1.terrain_tiles, layout2.terrain_tiles);
    }

    #[test]
    fn test_wfc_different_seeds_differ() {
        let layout_a = generate_world_layout(GOLDEN_SEED_PRIMARY);
        let layout_b = generate_world_layout(GOLDEN_SEED_SECONDARY);
        assert_ne!(layout_a.terrain_tiles, layout_b.terrain_tiles);
    }

    #[test]
    fn test_site_yard_no_river_sand() {
        let layout = generate_world_layout(GOLDEN_SEED_PRIMARY);
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
        let layout = generate_world_layout(GOLDEN_SEED_PRIMARY);
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
        let layout = generate_world_layout(GOLDEN_SEED_PRIMARY);
        for y in 0..MAP_HEIGHT {
            for x in 0..MAP_WIDTH {
                let idx = (y * MAP_WIDTH + x) as usize;
                let is_sand = layout.terrain_tiles[idx] == TerrainType::Sand;
                let in_legal_mask = layout.masks.final_sand_mask.get((x, y))
                    || layout.masks.inland_sand_mask.get((x, y));
                if layout.masks.final_sand_mask.get((x, y)) {
                    assert!(
                        is_sand,
                        "final_sand_mask=true but terrain is not Sand at ({x},{y})"
                    );
                }
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
    use super::generate_world_layout;
    use crate::terrain::TerrainType;
    use crate::terrain_zones::{ZONE_GRADIENT_WIDTH, compute_anchor_distance_field};
    use crate::test_seeds::SEED_SUITE_DIAG_PRINT;
    use hw_core::constants::{MAP_HEIGHT, MAP_WIDTH};

    #[test]
    #[ignore = "diagnostic only – run with: cargo test -p hw_world -- --ignored"]
    fn print_tile_distribution() {
        println!("\n=== Tile Distribution per Zone Category ===");
        println!(
            "{:>8}  {:>10} {:>10} {:>10} {:>10} {:>10}",
            "seed", "area", "grass%", "dirt%", "sand%", "river%"
        );

        for category in &["dirt_zone", "grass_zone", "neutral", "all_non_river"] {
            let mut g_sum = 0usize;
            let mut d_sum = 0usize;
            let mut s_sum = 0usize;
            let mut total_sum = 0usize;
            for &seed in SEED_SUITE_DIAG_PRINT {
                let layout = generate_world_layout(seed);
                let masks = &layout.masks;
                for y in 0..MAP_HEIGHT {
                    for x in 0..MAP_WIDTH {
                        let p = (x, y);
                        if masks.river_mask.get(p) {
                            continue;
                        }
                        let in_cat = match *category {
                            "dirt_zone" => masks.dirt_zone_mask.get(p),
                            "grass_zone" => masks.grass_zone_mask.get(p),
                            "neutral" => {
                                !masks.dirt_zone_mask.get(p) && !masks.grass_zone_mask.get(p)
                            }
                            _ => true,
                        };
                        if !in_cat {
                            continue;
                        }
                        let idx = (y * MAP_WIDTH + x) as usize;
                        match layout.terrain_tiles[idx] {
                            TerrainType::Grass => g_sum += 1,
                            TerrainType::Dirt => d_sum += 1,
                            TerrainType::Sand => s_sum += 1,
                            _ => {}
                        }
                        total_sum += 1;
                    }
                }
            }
            if total_sum == 0 {
                continue;
            }
            println!(
                "{:>12}  {:>10} {:>9}% {:>9}% {:>9}%",
                category,
                total_sum / SEED_SUITE_DIAG_PRINT.len(),
                g_sum * 100 / total_sum,
                d_sum * 100 / total_sum,
                s_sum * 100 / total_sum
            );
        }
    }

    #[test]
    #[ignore = "diagnostic only – run with: cargo test -p hw_world -- --ignored"]
    fn print_neutral_breakdown() {
        let n = SEED_SUITE_DIAG_PRINT.len();
        let mut zone_sum = 0usize;
        let mut gradient_sum = 0usize;
        let mut pure_neutral_sum = 0usize;
        let mut total_sum = 0usize;

        for &seed in SEED_SUITE_DIAG_PRINT {
            let layout = generate_world_layout(seed);
            let masks = &layout.masks;
            for y in 0..MAP_HEIGHT {
                for x in 0..MAP_WIDTH {
                    let p = (x, y);
                    if masks.river_mask.get(p) || masks.anchor_mask.get(p) {
                        continue;
                    }
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
                }
            }
        }
        println!(
            "\n=== Zone / Gradient / Pure-Neutral Breakdown (avg over {} seeds) ===",
            n
        );
        println!(
            "  zone          : {:>5} cells ({:.1}%)",
            zone_sum / n,
            zone_sum * 100 / total_sum
        );
        println!(
            "  C gradient    : {:>5} cells ({:.1}%)",
            gradient_sum / n,
            gradient_sum * 100 / total_sum
        );
        println!(
            "  pure neutral  : {:>5} cells ({:.1}%)",
            pure_neutral_sum / n,
            pure_neutral_sum * 100 / total_sum
        );
        println!("  total non-river/anchor: {:>5}", total_sum / n);
    }

    #[test]
    #[ignore = "diagnostic only – run with: cargo test -p hw_world -- --ignored"]
    fn print_zone_coverage_by_distance() {
        println!(
            "\n=== Zone Coverage by Distance Band (avg over {} seeds) ===",
            SEED_SUITE_DIAG_PRINT.len()
        );
        println!(
            "{:>10}  {:>8} {:>10} {:>11} {:>9}",
            "dist_range", "cells", "dirt_zone%", "grass_zone%", "neutral%"
        );

        let bands: &[(u32, u32)] = &[
            (0, 4),
            (5, 8),
            (9, 12),
            (13, 15),
            (16, 19),
            (20, 24),
            (25, 30),
            (31, 50),
            (51, 99),
        ];
        // 距離場はシード非依存（anchor_maskが同一）なので1度だけ計算
        let dist_field = {
            let layout = generate_world_layout(SEED_SUITE_DIAG_PRINT[0]);
            compute_anchor_distance_field(&layout.masks.anchor_mask)
        };
        for &(d_min, d_max) in bands {
            let mut total_sum = 0usize;
            let mut dirt_sum = 0usize;
            let mut grass_sum = 0usize;
            for &seed in SEED_SUITE_DIAG_PRINT {
                let layout = generate_world_layout(seed);
                let masks = &layout.masks;
                for y in 0..MAP_HEIGHT {
                    for x in 0..MAP_WIDTH {
                        let p = (x, y);
                        if masks.anchor_mask.get(p) || masks.river_mask.get(p) {
                            continue;
                        }
                        let d = dist_field[(y * MAP_WIDTH + x) as usize];
                        if d < d_min || d > d_max {
                            continue;
                        }
                        total_sum += 1;
                        if masks.dirt_zone_mask.get(p) {
                            dirt_sum += 1;
                        }
                        if masks.grass_zone_mask.get(p) {
                            grass_sum += 1;
                        }
                    }
                }
            }
            if total_sum == 0 {
                continue;
            }
            let neutral_sum = total_sum - dirt_sum - grass_sum;
            println!(
                "{:>5}..{:<4}  {:>8} {:>9}% {:>10}% {:>8}%",
                d_min,
                d_max,
                total_sum / SEED_SUITE_DIAG_PRINT.len(),
                dirt_sum * 100 / total_sum,
                grass_sum * 100 / total_sum,
                neutral_sum * 100 / total_sum
            );
        }
    }
}
