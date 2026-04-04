use crate::layout::{RIVER_X_MAX, RIVER_X_MIN, RIVER_Y_MAX, RIVER_Y_MIN};
use crate::world_masks::BitGrid;
use hw_core::constants::{MAP_HEIGHT, MAP_WIDTH};
use hw_core::world::GridPos;
use rand::SeedableRng;
use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use std::collections::HashSet;

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
    use rand::Rng;

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
}
