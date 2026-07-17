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
        assert!(
            col_has_river,
            "x={x} に River セルがない（横断が途切れている）"
        );
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

#[test]
fn final_sand_has_cells_not_adjacent_to_river() {
    // 2e 以降: dist >= 2 のセルが存在する = 川に 4 近傍隣接しない Sand が生まれている
    let masks = make_masks_with_sand();
    let has_non_adjacent = (0..MAP_HEIGHT).any(|y| {
        (0..MAP_WIDTH).any(|x| {
            if !masks.final_sand_mask.get((x, y)) {
                return false;
            }
            !CARDINAL_DIRS_4
                .iter()
                .any(|&(dx, dy)| masks.river_mask.get((x + dx, y + dy)))
        })
    });
    assert!(
        has_non_adjacent,
        "全 Sand が river に 4 近傍隣接している: 2e は dist>=2 の Sand を生成するはず"
    );
}

#[test]
fn final_sand_count_stays_bounded_for_representative_seed() {
    let masks = make_masks_with_sand();
    let sand = masks.final_sand_mask.count_set();
    let river = masks.river_mask.count_set();
    assert!(
        sand <= river * 3,
        "Sand area exploded for seed=42: sand={sand}, river={river}"
    );
}
