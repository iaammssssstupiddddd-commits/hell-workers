use crate::layout::{RIVER_X_MAX, RIVER_X_MIN, RIVER_Y_MAX, RIVER_Y_MIN};
use crate::world_masks::BitGrid;
use hw_core::constants::{MAP_HEIGHT, MAP_WIDTH};
use hw_core::world::GridPos;
use rand::Rng;
use rand::SeedableRng;
use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use std::collections::HashSet;
use std::collections::VecDeque;

// ── 川生成定数 ────────────────────────────────────────────────────────────────
/// 川の開始 y 範囲（anchor protection_band 下端 y=62 より下）
pub const RIVER_START_Y_MIN: i32 = 65;
pub const RIVER_START_Y_MAX: i32 = 82;
/// 川の y がマップ端に貼り付かないよう clamp する範囲
pub const RIVER_Y_CLAMP_MIN: i32 = 63;
pub const RIVER_Y_CLAMP_MAX: i32 = MAP_HEIGHT - 6; // = 94
/// セグメントごとの幅（タイル数、両端含む）
pub const RIVER_MIN_WIDTH: i32 = 2;
pub const RIVER_MAX_WIDTH: i32 = 4;
/// 全体タイル数の目安（検証テスト用; seed によって変動可）
pub const RIVER_TOTAL_TILES_TARGET_MIN: usize = 200;
pub const RIVER_TOTAL_TILES_TARGET_MAX: usize = 500;

// ── 砂マスク定数 ──────────────────────────────────────────────────────────────
/// 砂マスク生成用 8 近傍
const EIGHT_DIRS: [(i32, i32); 8] = [
    (0, -1), (1, 0), (0, 1), (-1, 0),
    (1, -1), (1, 1), (-1, 1), (-1, -1),
];
/// 砂 carve flood-fill 用 4 近傍（wfc_adapter 依存を避けて独立定義）
const CARDINAL_DIRS_4: [(i32, i32); 4] = [(0, -1), (1, 0), (0, 1), (-1, 0)];
/// carve 起点数の下限
pub const SAND_CARVE_SEED_COUNT_MIN: u32 = 2;
/// carve 起点数の上限
pub const SAND_CARVE_SEED_COUNT_MAX: u32 = 5;
/// candidate 全体に対する最大 carve 割合（%）
pub const SAND_CARVE_MAX_RATIO_PERCENT: usize = 35;
/// 1 carve region の最小面積（セル数）
pub const SAND_CARVE_REGION_SIZE_MIN: u32 = 6;
/// 1 carve region の最大面積（セル数）
pub const SAND_CARVE_REGION_SIZE_MAX: u32 = 24;
/// river RNG と区別するための seed XOR マスク
const SAND_SEED_SALT: u64 = 0xA5A5_A5A5_A5A5_A5A5;

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

/// 砂を配置
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

/// seed から deterministic な左端→右端横断川を生成する。
///
/// # 引数
/// - `seed`: 乱数シード（同一 seed で同一結果）
/// - `anchor_mask`: Site ∪ Yard の占有セル（`WorldMasks::from_anchor` 済み）
/// - `river_protection_band`: アンカー外周 PROTECTION_BAND_RIVER_WIDTH の禁止帯
///
/// # 戻り値
/// `(river_mask, river_centerline)`
pub fn generate_river_mask(
    seed: u64,
    anchor_mask: &BitGrid,
    river_protection_band: &BitGrid,
) -> (BitGrid, Vec<GridPos>) {
    let mut rng = StdRng::seed_from_u64(seed);
    let mut river_mask = BitGrid::map_sized();
    let mut centerline: Vec<GridPos> = Vec::with_capacity(MAP_WIDTH as usize);

    let start_y = rng.gen_range(RIVER_START_Y_MIN..=RIVER_START_Y_MAX);
    let mut current_y = start_y;

    // 蛇行バイアス: -1 が 2/7, 0 が 3/7, +1 が 2/7（期待値 0、標準偏差 ≈ 0.93）
    let steps: &[i32] = &[-1, -1, 0, 0, 0, 1, 1];

    for x in 0..MAP_WIDTH {
        let step = *steps.choose(&mut rng).unwrap();
        let mut next_y = (current_y + step).clamp(RIVER_Y_CLAMP_MIN, RIVER_Y_CLAMP_MAX);

        // next_y が禁止セルなら直進（current_y を維持）
        if river_protection_band.get((x, next_y)) || anchor_mask.get((x, next_y)) {
            next_y = current_y;
        }

        current_y = next_y;
        centerline.push((x, current_y));

        let width = rng.gen_range(RIVER_MIN_WIDTH..=RIVER_MAX_WIDTH);
        let top = current_y - width / 2;
        let bottom = top + width - 1;

        for ry in top..=bottom {
            if !(0..MAP_HEIGHT).contains(&ry) {
                continue;
            }
            let pos = (x, ry);
            if !anchor_mask.get(pos) && !river_protection_band.get(pos) {
                river_mask.set(pos, true);
            }
        }
    }

    (river_mask, centerline)
}

// ── 砂マスク生成 ──────────────────────────────────────────────────────────────

/// seed から deterministic に砂マスク 3 点セットを生成して返す。
///
/// # 戻り値
/// `(sand_candidate_mask, sand_carve_mask, final_sand_mask)`
///
/// `final_sand_mask` が空になる場合（shoreline 候補ゼロ）は carve を全捨てして
/// candidate 全面を final とするフォールバックを行う。candidate 自体がゼロの
/// 場合は全マスク空のまま返す（caller は lightweight_validate で検知する）。
pub fn generate_sand_masks(
    seed: u64,
    river_mask: &BitGrid,
    anchor_mask: &BitGrid,
    river_protection_band: &BitGrid,
) -> (BitGrid, BitGrid, BitGrid) {
    let mut rng = StdRng::seed_from_u64(seed ^ SAND_SEED_SALT);

    let candidate = build_sand_candidate_mask(river_mask, anchor_mask, river_protection_band);
    let mut carve = build_sand_carve_mask(&candidate, &mut rng);

    // final = candidate & !carve
    let mut final_mask = candidate.clone();
    for y in 0..MAP_HEIGHT {
        for x in 0..MAP_WIDTH {
            if carve.get((x, y)) {
                final_mask.set((x, y), false);
            }
        }
    }

    // フォールバック: final が空なら carve を全捨てして candidate 全面を採用
    if final_mask.count_set() == 0 {
        carve = BitGrid::map_sized();
        final_mask = candidate.clone();
    }

    (candidate, carve, final_mask)
}

fn build_sand_candidate_mask(
    river_mask: &BitGrid,
    anchor_mask: &BitGrid,
    river_protection_band: &BitGrid,
) -> BitGrid {
    let mut candidate = BitGrid::map_sized();
    for y in 0..MAP_HEIGHT {
        for x in 0..MAP_WIDTH {
            if !river_mask.get((x, y)) {
                continue;
            }
            for (dx, dy) in EIGHT_DIRS {
                let nx = x + dx;
                let ny = y + dy;
                if !(0..MAP_WIDTH).contains(&nx) || !(0..MAP_HEIGHT).contains(&ny) {
                    continue;
                }
                let p = (nx, ny);
                if !river_mask.get(p) && !anchor_mask.get(p) && !river_protection_band.get(p) {
                    candidate.set(p, true);
                }
            }
        }
    }
    candidate
}

fn build_sand_carve_mask(candidate: &BitGrid, rng: &mut StdRng) -> BitGrid {
    let candidate_positions: Vec<GridPos> = (0..MAP_HEIGHT)
        .flat_map(|y| {
            (0..MAP_WIDTH).filter_map(move |x| candidate.get((x, y)).then_some((x, y)))
        })
        .collect();

    let mut carve = BitGrid::map_sized();
    if candidate_positions.is_empty() {
        return carve;
    }

    let max_carve_cells = candidate_positions.len() * SAND_CARVE_MAX_RATIO_PERCENT / 100;
    let num_seeds = rng.gen_range(SAND_CARVE_SEED_COUNT_MIN..=SAND_CARVE_SEED_COUNT_MAX);
    let k = (num_seeds as usize).min(candidate_positions.len());

    let origins: Vec<GridPos> = candidate_positions
        .choose_multiple(rng, k)
        .copied()
        .collect();

    let mut total_carved = 0usize;
    for origin in origins {
        if total_carved >= max_carve_cells {
            break;
        }
        let region_size =
            rng.gen_range(SAND_CARVE_REGION_SIZE_MIN..=SAND_CARVE_REGION_SIZE_MAX) as usize;
        let limit = region_size.min(max_carve_cells - total_carved);
        total_carved += flood_fill_carve_region(candidate, &mut carve, origin, limit);
    }
    carve
}

/// candidate 内を 4 近傍 BFS で最大 `limit` セル carve する。
/// 戻り値: 実際に carve したセル数（高々 `limit`）。
fn flood_fill_carve_region(
    candidate: &BitGrid,
    carve: &mut BitGrid,
    origin: GridPos,
    limit: usize,
) -> usize {
    if !candidate.get(origin) || carve.get(origin) {
        return 0;
    }
    let mut queue: VecDeque<GridPos> = VecDeque::new();
    queue.push_back(origin);
    carve.set(origin, true);
    let mut count = 1;

    while let Some(pos) = queue.pop_front() {
        if count >= limit {
            break;
        }
        for (dx, dy) in CARDINAL_DIRS_4 {
            if count >= limit {
                break;
            }
            let nx = pos.0 + dx;
            let ny = pos.1 + dy;
            let p = (nx, ny);
            if candidate.get(p) && !carve.get(p) {
                carve.set(p, true);
                count += 1;
                queue.push_back(p);
            }
        }
    }
    count
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::anchor::AnchorLayout;
    use crate::world_masks::WorldMasks;

    fn make_masks() -> WorldMasks {
        let anchor = AnchorLayout::fixed();
        let mut masks = WorldMasks::from_anchor(&anchor);
        masks.fill_river_from_seed(42);
        masks
    }

    #[test]
    fn river_mask_crosses_map_left_to_right() {
        let masks = make_masks();
        for x in 0..MAP_WIDTH {
            let col_has_river = (0..MAP_HEIGHT).any(|y| masks.river_mask.get((x, y)));
            assert!(col_has_river, "x={x} に River セルがない（横断が途切れている）");
        }
    }

    #[test]
    fn river_mask_does_not_enter_anchor() {
        let masks = make_masks();
        for y in 0..MAP_HEIGHT {
            for x in 0..MAP_WIDTH {
                let pos = (x, y);
                assert!(
                    !(masks.river_mask.get(pos) && masks.anchor_mask.get(pos)),
                    "pos {pos:?} が river かつ anchor に属している"
                );
            }
        }
    }

    #[test]
    fn river_mask_does_not_enter_protection_band() {
        let masks = make_masks();
        for y in 0..MAP_HEIGHT {
            for x in 0..MAP_WIDTH {
                let pos = (x, y);
                assert!(
                    !(masks.river_mask.get(pos) && masks.river_protection_band.get(pos)),
                    "pos {pos:?} が river かつ protection_band に属している"
                );
            }
        }
    }

    #[test]
    fn river_total_tile_count_in_range() {
        let masks = make_masks();
        let count = masks.river_mask.count_set();
        assert!(
            (RIVER_TOTAL_TILES_TARGET_MIN..=RIVER_TOTAL_TILES_TARGET_MAX).contains(&count),
            "river tile count {count} が想定範囲外 ({RIVER_TOTAL_TILES_TARGET_MIN}..={RIVER_TOTAL_TILES_TARGET_MAX})"
        );
    }

    #[test]
    fn river_generation_is_deterministic() {
        let masks_a = make_masks();
        let masks_b = make_masks();
        assert_eq!(
            masks_a.river_centerline, masks_b.river_centerline,
            "同一 seed で centerline が異なる"
        );
    }

    fn make_masks_with_sand() -> WorldMasks {
        let anchor = AnchorLayout::fixed();
        let mut masks = WorldMasks::from_anchor(&anchor);
        masks.fill_river_from_seed(42);
        masks.fill_sand_from_river_seed(42);
        masks
    }

    #[test]
    fn sand_mask_is_deterministic_for_same_seed() {
        let masks_a = make_masks_with_sand();
        let masks_b = make_masks_with_sand();
        for y in 0..MAP_HEIGHT {
            for x in 0..MAP_WIDTH {
                let pos = (x, y);
                assert_eq!(
                    masks_a.final_sand_mask.get(pos),
                    masks_b.final_sand_mask.get(pos),
                    "final_sand_mask differs at {pos:?}"
                );
            }
        }
    }

    #[test]
    fn sand_mask_does_not_overlap_anchor_or_protection_band() {
        let masks = make_masks_with_sand();
        for y in 0..MAP_HEIGHT {
            for x in 0..MAP_WIDTH {
                let pos = (x, y);
                if masks.final_sand_mask.get(pos) {
                    assert!(
                        !masks.anchor_mask.get(pos),
                        "final_sand_mask overlaps anchor_mask at {pos:?}"
                    );
                    assert!(
                        !masks.river_protection_band.get(pos),
                        "final_sand_mask overlaps river_protection_band at {pos:?}"
                    );
                    assert!(
                        !masks.river_mask.get(pos),
                        "final_sand_mask overlaps river_mask at {pos:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn final_sand_mask_is_non_empty() {
        let masks = make_masks_with_sand();
        assert!(
            masks.final_sand_mask.count_set() > 0,
            "final_sand_mask が空（seed=42）— shoreline 候補が無い seed の可能性。§5.1 参照"
        );
    }
}
