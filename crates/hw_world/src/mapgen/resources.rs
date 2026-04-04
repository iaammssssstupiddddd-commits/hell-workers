//! WFC 資源配置（MS-WFC-3）。
//!
//! - `generate_resource_layout()`: grass/dirt ゾーンを使い、木・岩を純粋関数で生成する。
//! - `generate_resource_layout_fallback()`: terrain fallback 地形向け縮退版。
//! - 配置候補不足で `None` を返した場合、`mapgen.rs` の `find_map` が次 attempt へ進む。

use hw_core::constants::{MAP_HEIGHT, MAP_WIDTH};
use hw_core::world::GridPos;
use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use rand::{Rng, SeedableRng};

use crate::mapgen::types::{GeneratedWorldLayout, WfcForestZone};
use crate::terrain::TerrainType;
use crate::world_masks::BitGrid;

// ── 定数 ─────────────────────────────────────────────────────────────────────

// 森林ゾーン
/// ゾーン数の下限（0 は許容しない）
pub const FOREST_ZONE_COUNT_MIN: u32 = 2;
/// ゾーン数の上限
pub const FOREST_ZONE_COUNT_MAX: u32 = 4;
/// ゾーン半径（チェビシェフ正方形の半辺）の下限
pub const FOREST_ZONE_RADIUS_MIN: u32 = 5;
/// ゾーン半径（チェビシェフ正方形の半辺）の上限
pub const FOREST_ZONE_RADIUS_MAX: u32 = 10;
/// ゾーン中心点同士の最低チェビシェフ距離（centers が近すぎるとゾーンが重複する）
pub const FOREST_ZONE_CENTER_SPACING: u32 = 12;
/// 1 ゾーンあたりの配置木数の下限
pub const TREES_PER_ZONE_MIN: usize = 6;
/// 1 ゾーンあたりの配置木数の上限
pub const TREES_PER_ZONE_MAX: usize = 16;
/// 木同士の最低チェビシェフ距離（密集しすぎ防止）
pub const TREE_MIN_SPACING: u32 = 2;

// ── 出力型 ────────────────────────────────────────────────────────────────────

/// `generate_resource_layout` の出力。`mapgen.rs` と `validate.rs` の間で共有する crate 内型。
/// `GeneratedWorldLayout` へのフラット展開は `mapgen.rs` の責務。
#[derive(Debug, Clone)]
pub(crate) struct ResourceLayout {
    pub initial_tree_positions: Vec<GridPos>,
    pub forest_regrowth_zones: Vec<WfcForestZone>,
    pub initial_rock_positions: Vec<GridPos>,
    /// bevy_app が岩採掘対象を参照するときの候補 (= initial_rock_positions と同値)
    pub rock_candidates: Vec<GridPos>,
}

// ── 公開 API ──────────────────────────────────────────────────────────────────

/// 木・岩・森林ゾーンを純粋関数として生成する。
/// `layout` は `lightweight_validate` 通過済みを前提とし、
/// `water_tiles` / `sand_tiles` が埋まっていることを仮定する。
/// 配置可能な候補セルが不足した場合は `None` を返し、attempt ごと捨てる。
pub(crate) fn generate_resource_layout(
    layout: &GeneratedWorldLayout,
    seed: u64,
) -> Option<ResourceLayout> {
    let mut rng = StdRng::seed_from_u64(seed);
    generate_resource_layout_inner(layout, &mut rng, FOREST_ZONE_COUNT_MIN)
}

/// terrain fallback 用の縮退版。下限を緩め、`None` を返さないことを優先する。
/// まず通常の下限で試み、失敗した場合は下限 1 で再試行する。
pub(crate) fn generate_resource_layout_fallback(
    layout: &GeneratedWorldLayout,
    seed: u64,
) -> Option<ResourceLayout> {
    let mut rng = StdRng::seed_from_u64(seed ^ 0xfb7c_3a91_d5e2_4608);
    if let Some(res) = generate_resource_layout_inner(layout, &mut rng, FOREST_ZONE_COUNT_MIN) {
        return Some(res);
    }
    // 下限を 1 に緩めて再試行（fallback terrain で Grass が少ない場合への対処）
    let mut rng = StdRng::seed_from_u64(seed ^ 0x0c4a_7b2f_9e81_d63e);
    generate_resource_layout_inner(layout, &mut rng, 1)
}

// ── 内部実装 ──────────────────────────────────────────────────────────────────

fn generate_resource_layout_inner(
    layout: &GeneratedWorldLayout,
    rng: &mut StdRng,
    zone_count_min: u32,
) -> Option<ResourceLayout> {
    let tree_exclusion = build_tree_exclusion(layout);
    let forest_regrowth_zones =
        generate_forest_zones(rng, layout, &tree_exclusion, zone_count_min)?;
    let initial_tree_positions = place_trees(rng, &forest_regrowth_zones, layout, &tree_exclusion)?;

    debug_assert!(
        initial_tree_positions
            .iter()
            .all(|&p| { forest_regrowth_zones.iter().any(|z| z.contains(p)) }),
        "initial_tree_positions に forest_regrowth_zones 外の木が含まれている"
    );

    let initial_rock_positions = place_rocks(layout)?;
    let rock_candidates = initial_rock_positions.clone();

    Some(ResourceLayout {
        initial_tree_positions,
        forest_regrowth_zones,
        initial_rock_positions,
        rock_candidates,
    })
}

// ── 森林ゾーン生成 ────────────────────────────────────────────────────────────

fn generate_forest_zones(
    rng: &mut StdRng,
    layout: &GeneratedWorldLayout,
    exclusion: &BitGrid,
    zone_count_min: u32,
) -> Option<Vec<WfcForestZone>> {
    // grass_zone_mask 内の非除外 Grass セルをゾーン中心候補とする
    let mut candidates: Vec<GridPos> = (0..MAP_HEIGHT)
        .flat_map(|y| {
            (0..MAP_WIDTH).filter_map(move |x| {
                let p = (x, y);
                let idx = (y * MAP_WIDTH + x) as usize;
                (layout.terrain_tiles[idx] == TerrainType::Grass
                    && layout.masks.grass_zone_mask.get(p)
                    && !exclusion.get(p))
                .then_some(p)
            })
        })
        .collect();

    if candidates.is_empty() {
        return None;
    }

    let zone_count = rng.gen_range(FOREST_ZONE_COUNT_MIN..=FOREST_ZONE_COUNT_MAX) as usize;
    candidates.shuffle(rng);

    let mut centers: Vec<GridPos> = Vec::new();
    for &p in &candidates {
        if centers
            .iter()
            .all(|&c| chebyshev(c, p) >= FOREST_ZONE_CENTER_SPACING)
        {
            centers.push(p);
            if centers.len() == zone_count {
                break;
            }
        }
    }

    if (centers.len() as u32) < zone_count_min {
        return None;
    }

    Some(
        centers
            .into_iter()
            .map(|center| WfcForestZone {
                center,
                radius: rng.gen_range(FOREST_ZONE_RADIUS_MIN..=FOREST_ZONE_RADIUS_MAX),
            })
            .collect(),
    )
}

// ── 木の配置 ──────────────────────────────────────────────────────────────────

fn place_trees(
    rng: &mut StdRng,
    zones: &[WfcForestZone],
    layout: &GeneratedWorldLayout,
    exclusion: &BitGrid,
) -> Option<Vec<GridPos>> {
    // 非除外 Grass セルを grass_zone_mask 内/外で事前収集（各ゾーンで再利用）
    let mut primary_pool: Vec<GridPos> = Vec::new();
    let mut secondary_pool: Vec<GridPos> = Vec::new();
    for y in 0..MAP_HEIGHT {
        for x in 0..MAP_WIDTH {
            let p = (x, y);
            let idx = (y * MAP_WIDTH + x) as usize;
            if layout.terrain_tiles[idx] != TerrainType::Grass || exclusion.get(p) {
                continue;
            }
            if layout.masks.grass_zone_mask.get(p) {
                primary_pool.push(p);
            } else {
                secondary_pool.push(p);
            }
        }
    }

    let mut all_trees: Vec<GridPos> = Vec::new();
    for zone in zones {
        let target = rng.gen_range(TREES_PER_ZONE_MIN..=TREES_PER_ZONE_MAX);
        let mut zone_trees: Vec<GridPos> = Vec::new();

        // 1 次候補: ゾーン内の grass_zone_mask 内 Grass
        let mut zone_primary: Vec<GridPos> = primary_pool
            .iter()
            .copied()
            .filter(|&p| zone.contains(p))
            .collect();
        zone_primary.shuffle(rng);
        for p in zone_primary {
            if all_trees
                .iter()
                .chain(zone_trees.iter())
                .all(|&t| chebyshev(t, p) >= TREE_MIN_SPACING)
            {
                zone_trees.push(p);
                if zone_trees.len() >= target {
                    break;
                }
            }
        }

        // 2 次候補: 最低本数に届いていない場合、ゾーン内の grass_zone_mask 外 Grass で補充
        if zone_trees.len() < TREES_PER_ZONE_MIN {
            let mut zone_secondary: Vec<GridPos> = secondary_pool
                .iter()
                .copied()
                .filter(|&p| zone.contains(p))
                .collect();
            zone_secondary.shuffle(rng);
            for p in zone_secondary {
                if all_trees
                    .iter()
                    .chain(zone_trees.iter())
                    .all(|&t| chebyshev(t, p) >= TREE_MIN_SPACING)
                {
                    zone_trees.push(p);
                    if zone_trees.len() >= TREES_PER_ZONE_MIN {
                        break;
                    }
                }
            }
        }

        if zone_trees.len() < TREES_PER_ZONE_MIN {
            return None;
        }
        all_trees.extend(zone_trees);
    }

    Some(all_trees)
}

// ── 岩の配置 ──────────────────────────────────────────────────────────────────

fn place_rocks(layout: &GeneratedWorldLayout) -> Option<Vec<GridPos>> {
    let mut rocks: Vec<GridPos> = Vec::new();
    for y in 0..MAP_HEIGHT {
        for x in 0..MAP_WIDTH {
            let p = (x, y);
            let idx = (y * MAP_WIDTH + x) as usize;
            if layout.masks.rock_field_mask.get(p) && layout.terrain_tiles[idx] == TerrainType::Dirt
            {
                rocks.push(p);
            }
        }
    }

    (!rocks.is_empty()).then_some(rocks)
}

// ── exclusion マスク構築 ──────────────────────────────────────────────────────

/// anchor_mask | tree_dense_protection_band | river_mask | final_sand_mask | inland_sand_mask
fn build_tree_exclusion(layout: &GeneratedWorldLayout) -> BitGrid {
    let mut ex = layout.masks.anchor_mask.clone();
    ex |= &layout.masks.tree_dense_protection_band;
    ex |= &layout.masks.river_mask;
    ex |= &layout.masks.final_sand_mask;
    ex |= &layout.masks.inland_sand_mask;
    ex
}

// ── ユーティリティ ────────────────────────────────────────────────────────────

fn chebyshev(a: GridPos, b: GridPos) -> u32 {
    let dx = (a.0 - b.0).unsigned_abs();
    let dy = (a.1 - b.1).unsigned_abs();
    dx.max(dy)
}

// ── テスト ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::anchor::AnchorLayout;
    use crate::mapgen::generate_world_layout;
    use crate::mapgen::types::ResourceSpawnCandidates;
    use crate::mapgen::validate::validate_post_resource;
    use crate::mapgen::wfc_adapter::fallback_terrain;
    use crate::world_masks::WorldMasks;

    const TEST_SEED_A: u64 = 42;
    const TEST_SEED_B: u64 = 12_345_678;

    fn make_fallback_layout(seed: u64) -> GeneratedWorldLayout {
        let anchors = AnchorLayout::aligned_to_worldgen_seed(seed);
        let mut masks = WorldMasks::from_anchor(&anchors);
        masks.fill_river_from_seed(seed);
        masks.fill_sand_from_river_seed(seed);
        masks.fill_terrain_zones_from_seed(seed);
        masks.fill_rock_fields_from_seed(seed);

        let candidate = GeneratedWorldLayout {
            terrain_tiles: fallback_terrain(&masks, seed),
            anchors,
            masks,
            resource_spawn_candidates: ResourceSpawnCandidates::default(),
            initial_tree_positions: Vec::new(),
            forest_regrowth_zones: Vec::new(),
            initial_rock_positions: Vec::new(),
            master_seed: seed,
            generation_attempt: 65,
            used_fallback: true,
        };

        let validated =
            crate::mapgen::validate::lightweight_validate(&candidate).expect("fallback terrain");

        GeneratedWorldLayout {
            resource_spawn_candidates: validated,
            ..candidate
        }
    }

    #[test]
    fn trees_not_in_exclusion_zone() {
        let layout = generate_world_layout(TEST_SEED_A);
        assert!(
            !layout.used_fallback,
            "seed={TEST_SEED_A}: fallback が使われた"
        );
        for &pos in &layout.initial_tree_positions {
            assert!(
                !layout.masks.anchor_mask.get(pos),
                "tree at {pos:?} is inside anchor_mask"
            );
            assert!(
                !layout.masks.tree_dense_protection_band.get(pos),
                "tree at {pos:?} is inside tree_dense_protection_band"
            );
            assert!(
                !layout.masks.river_mask.get(pos),
                "tree at {pos:?} is inside river_mask"
            );
            assert!(
                !layout.masks.final_sand_mask.get(pos),
                "tree at {pos:?} is inside final_sand_mask"
            );
            assert!(
                !layout.masks.inland_sand_mask.get(pos),
                "tree at {pos:?} is inside inland_sand_mask"
            );
        }
    }

    #[test]
    fn trees_are_inside_some_forest_zone() {
        let layout = generate_world_layout(TEST_SEED_A);
        assert!(!layout.used_fallback);
        for &pos in &layout.initial_tree_positions {
            assert!(
                layout.forest_regrowth_zones.iter().any(|z| z.contains(pos)),
                "tree at {pos:?} is outside all forest_regrowth_zones"
            );
        }
    }

    #[test]
    fn rocks_not_in_exclusion_zone() {
        let layout = generate_world_layout(TEST_SEED_A);
        assert!(!layout.used_fallback);
        for &pos in &layout.initial_rock_positions {
            assert!(
                !layout.masks.anchor_mask.get(pos),
                "rock at {pos:?} is inside anchor_mask"
            );
            assert!(
                !layout.masks.rock_protection_band.get(pos),
                "rock at {pos:?} is inside rock_protection_band"
            );
            assert!(
                !layout.masks.river_mask.get(pos),
                "rock at {pos:?} is inside river_mask"
            );
            assert!(
                !layout.masks.final_sand_mask.get(pos),
                "rock at {pos:?} is inside final_sand_mask"
            );
            assert!(
                !layout.masks.inland_sand_mask.get(pos),
                "rock at {pos:?} is inside inland_sand_mask"
            );
        }
    }

    #[test]
    fn rocks_match_rock_field_mask() {
        let layout = generate_world_layout(TEST_SEED_A);
        assert!(!layout.used_fallback);
        for &pos in &layout.initial_rock_positions {
            assert!(
                layout.masks.rock_field_mask.get(pos),
                "rock at {pos:?} is outside rock_field_mask"
            );
        }
        assert_eq!(
            layout.initial_rock_positions.len(),
            layout.masks.rock_field_mask.count_set(),
            "all rock_field_mask cells should materialize as rocks"
        );
    }

    #[test]
    fn rock_field_mask_is_dirt_in_final_layout() {
        let layout = generate_world_layout(TEST_SEED_A);
        for y in 0..MAP_HEIGHT {
            for x in 0..MAP_WIDTH {
                if layout.masks.rock_field_mask.get((x, y)) {
                    assert_eq!(
                        layout.terrain_tiles[(y * MAP_WIDTH + x) as usize],
                        TerrainType::Dirt,
                        "rock_field_mask cell ({x},{y}) is not Dirt"
                    );
                }
            }
        }
    }

    #[test]
    fn resource_layout_keeps_required_paths_open() {
        for seed in [TEST_SEED_A, TEST_SEED_B] {
            let layout = generate_world_layout(seed);
            assert!(!layout.used_fallback, "seed={seed}: fallback が使われた");
            assert!(
                !layout.initial_tree_positions.is_empty(),
                "seed={seed}: initial_tree_positions が空"
            );
            assert!(
                !layout.forest_regrowth_zones.is_empty(),
                "seed={seed}: forest_regrowth_zones が空"
            );
            assert!(
                !layout.initial_rock_positions.is_empty(),
                "seed={seed}: initial_rock_positions が空"
            );
            let res = ResourceLayout {
                initial_tree_positions: layout.initial_tree_positions.clone(),
                forest_regrowth_zones: layout.forest_regrowth_zones.clone(),
                initial_rock_positions: layout.initial_rock_positions.clone(),
                rock_candidates: layout.resource_spawn_candidates.rock_candidates.clone(),
            };
            assert!(
                validate_post_resource(&layout, &res).is_ok(),
                "seed={seed}: validate_post_resource failed"
            );
        }
    }

    #[test]
    fn rock_candidates_equals_initial_rock_positions() {
        let layout = generate_world_layout(TEST_SEED_A);
        assert!(!layout.used_fallback);
        let mut expected = layout.initial_rock_positions.clone();
        let mut actual = layout.resource_spawn_candidates.rock_candidates.clone();
        expected.sort();
        actual.sort();
        assert_eq!(expected, actual);
    }

    #[test]
    fn resource_layout_is_deterministic() {
        let l1 = generate_world_layout(TEST_SEED_A);
        let l2 = generate_world_layout(TEST_SEED_A);
        assert_eq!(l1.initial_tree_positions, l2.initial_tree_positions);
        assert_eq!(l1.initial_rock_positions, l2.initial_rock_positions);
        assert_eq!(
            l1.forest_regrowth_zones
                .iter()
                .map(|z| (z.center, z.radius))
                .collect::<Vec<_>>(),
            l2.forest_regrowth_zones
                .iter()
                .map(|z| (z.center, z.radius))
                .collect::<Vec<_>>(),
        );
    }

    #[test]
    fn fallback_resource_layout_is_non_empty_and_paths_open() {
        for seed in [0u64, TEST_SEED_A, 99, TEST_SEED_B] {
            let layout = make_fallback_layout(seed);
            let res = generate_resource_layout_fallback(&layout, seed)
                .expect("fallback resource generation must succeed for representative seeds");

            assert!(
                !res.initial_tree_positions.is_empty(),
                "seed={seed}: fallback trees are empty"
            );
            assert!(
                !res.forest_regrowth_zones.is_empty(),
                "seed={seed}: fallback forest zones are empty"
            );
            assert!(
                !res.initial_rock_positions.is_empty(),
                "seed={seed}: fallback rocks are empty"
            );
            assert!(
                validate_post_resource(&layout, &res).is_ok(),
                "seed={seed}: fallback resource layout broke required paths"
            );
        }
    }
}
