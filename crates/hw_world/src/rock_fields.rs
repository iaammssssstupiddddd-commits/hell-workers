//! 岩場マスク生成。
//!
//! 川・砂浜の後段で seed から deterministic に「岩場」を決め、
//! WFC 後の post-process と資源配置の両方で共有する。

use std::collections::VecDeque;

use hw_core::constants::{MAP_HEIGHT, MAP_WIDTH};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

use crate::world_masks::BitGrid;

/// 岩場はマップ東側に限定する。
pub const ROCK_FIELD_X_MIN: i32 = 72;
/// 上側クラスターの Y 範囲
pub const ROCK_FIELD_TOP_Y_MIN: i32 = 14;
/// 上側クラスターの Y 範囲
pub const ROCK_FIELD_TOP_Y_MAX: i32 = 34;
/// 下側クラスターの Y 範囲
pub const ROCK_FIELD_BOTTOM_Y_MIN: i32 = 74;
/// 下側クラスターの Y 範囲
pub const ROCK_FIELD_BOTTOM_Y_MAX: i32 = 94;
/// 1 クラスターの面積下限
pub const ROCK_FIELD_CLUSTER_AREA_MIN: usize = 24;
/// 1 クラスターの面積上限
pub const ROCK_FIELD_CLUSTER_AREA_MAX: usize = 32;
/// 代表 seed 群で期待する最小岩場セル数
pub const ROCK_FIELD_TOTAL_AREA_MIN: usize = 48;

const CARDINAL_DIRS: [(i32, i32); 4] = [(0, 1), (0, -1), (1, 0), (-1, 0)];

/// 川・砂・内陸砂・アンカー帯を避けた岩場マスクを生成する。
pub fn generate_rock_field_mask(
    seed: u64,
    anchor_mask: &BitGrid,
    rock_protection_band: &BitGrid,
    river_mask: &BitGrid,
    final_sand_mask: &BitGrid,
    inland_sand_mask: &BitGrid,
) -> BitGrid {
    let mut rng = StdRng::seed_from_u64(seed ^ 0x5f8b_c43d_77a1_92e1);
    let allowed = build_allowed_mask(
        anchor_mask,
        rock_protection_band,
        river_mask,
        final_sand_mask,
        inland_sand_mask,
    );
    let mut result = BitGrid::map_sized();

    for (y_min, y_max) in [
        (ROCK_FIELD_TOP_Y_MIN, ROCK_FIELD_TOP_Y_MAX),
        (ROCK_FIELD_BOTTOM_Y_MIN, ROCK_FIELD_BOTTOM_Y_MAX),
    ] {
        let mut candidates = collect_candidates_in_band(&allowed, &result, y_min, y_max);
        if candidates.is_empty() {
            continue;
        }
        let area_target = rng.gen_range(ROCK_FIELD_CLUSTER_AREA_MIN..=ROCK_FIELD_CLUSTER_AREA_MAX);
        let origin = candidates.swap_remove(rng.gen_range(0..candidates.len()));
        grow_patch(&mut rng, &allowed, &mut result, origin, area_target);
    }

    result
}

fn build_allowed_mask(
    anchor_mask: &BitGrid,
    rock_protection_band: &BitGrid,
    river_mask: &BitGrid,
    final_sand_mask: &BitGrid,
    inland_sand_mask: &BitGrid,
) -> BitGrid {
    let mut allowed = BitGrid::map_sized();
    for y in 0..MAP_HEIGHT {
        for x in ROCK_FIELD_X_MIN..MAP_WIDTH {
            let p = (x, y);
            if !anchor_mask.get(p)
                && !rock_protection_band.get(p)
                && !river_mask.get(p)
                && !final_sand_mask.get(p)
                && !inland_sand_mask.get(p)
            {
                allowed.set(p, true);
            }
        }
    }
    allowed
}

fn collect_candidates_in_band(
    allowed: &BitGrid,
    existing: &BitGrid,
    y_min: i32,
    y_max: i32,
) -> Vec<(i32, i32)> {
    (y_min..=y_max.min(MAP_HEIGHT - 1))
        .flat_map(|y| (ROCK_FIELD_X_MIN..MAP_WIDTH).map(move |x| (x, y)))
        .filter(|&p| allowed.get(p) && !existing.get(p))
        .collect()
}

fn grow_patch(
    rng: &mut StdRng,
    allowed: &BitGrid,
    result: &mut BitGrid,
    origin: (i32, i32),
    area_target: usize,
) {
    if !allowed.get(origin) || result.get(origin) {
        return;
    }

    let mut patch = BitGrid::map_sized();
    let mut frontier: VecDeque<(i32, i32)> = VecDeque::new();
    patch.set(origin, true);
    frontier.push_back(origin);
    let mut count = 1usize;

    while count < area_target {
        let Some(pos) = frontier.pop_front() else {
            break;
        };
        let mut dirs = CARDINAL_DIRS;
        for i in 0..dirs.len() {
            let j = rng.gen_range(i..dirs.len());
            dirs.swap(i, j);
        }
        for (dx, dy) in dirs {
            if count >= area_target {
                break;
            }
            let next = (pos.0 + dx, pos.1 + dy);
            if allowed.get(next) && !patch.get(next) && !result.get(next) {
                patch.set(next, true);
                frontier.push_back(next);
                count += 1;
            }
        }
    }

    for y in 0..MAP_HEIGHT {
        for x in ROCK_FIELD_X_MIN..MAP_WIDTH {
            let p = (x, y);
            if patch.get(p) {
                result.set(p, true);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::anchor::AnchorLayout;
    use crate::test_seeds::SEED_SUITE_ROCK_REGRESSION;
    use crate::world_masks::WorldMasks;

    fn make_masks(seed: u64) -> WorldMasks {
        let anchors = AnchorLayout::fixed();
        let mut masks = WorldMasks::from_anchor(&anchors);
        masks.fill_river_from_seed(seed);
        masks.fill_sand_from_river_seed(seed);
        masks.fill_terrain_zones_from_seed(seed);
        masks.fill_rock_fields_from_seed(seed);
        masks
    }

    #[test]
    fn rock_field_mask_is_deterministic() {
        let m1 = make_masks(42);
        let m2 = make_masks(42);
        assert_eq!(
            m1.rock_field_mask.count_set(),
            m2.rock_field_mask.count_set()
        );
        for y in 0..MAP_HEIGHT {
            for x in 0..MAP_WIDTH {
                assert_eq!(
                    m1.rock_field_mask.get((x, y)),
                    m2.rock_field_mask.get((x, y)),
                    "rock_field_mask mismatch at ({x},{y})"
                );
            }
        }
    }

    #[test]
    fn rock_field_mask_avoids_blocked_masks() {
        let masks = make_masks(42);
        for y in 0..MAP_HEIGHT {
            for x in 0..MAP_WIDTH {
                let p = (x, y);
                if !masks.rock_field_mask.get(p) {
                    continue;
                }
                assert!(
                    !masks.anchor_mask.get(p),
                    "rock field intersects anchor at {p:?}"
                );
                assert!(
                    !masks.rock_protection_band.get(p),
                    "rock field intersects rock protection band at {p:?}"
                );
                assert!(
                    !masks.river_mask.get(p),
                    "rock field intersects river at {p:?}"
                );
                assert!(
                    !masks.final_sand_mask.get(p),
                    "rock field intersects final sand at {p:?}"
                );
                assert!(
                    !masks.inland_sand_mask.get(p),
                    "rock field intersects inland sand at {p:?}"
                );
            }
        }
    }

    #[test]
    fn rock_field_mask_has_stable_min_area_on_representative_seeds() {
        for seed in SEED_SUITE_ROCK_REGRESSION.iter().copied() {
            let masks = make_masks(seed);
            assert!(
                masks.rock_field_mask.count_set() >= ROCK_FIELD_TOTAL_AREA_MIN,
                "seed={seed}: rock field area too small: {}",
                masks.rock_field_mask.count_set()
            );
        }
    }
}
