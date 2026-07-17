use super::*;
use crate::anchor::AnchorLayout;
use crate::test_seeds::{SEED_SUITE_TERRAIN_ZONE_CANDIDATES, TERRAIN_ZONE_DETERMINISM_SEED};
use crate::world_masks::WorldMasks;

fn make_masks(seed: u64) -> WorldMasks {
    let anchors = AnchorLayout::fixed();
    let mut masks = WorldMasks::from_anchor(&anchors);
    masks.fill_river_from_seed(seed);
    masks.fill_sand_from_river_seed(seed);
    masks.fill_terrain_zones_from_seed(seed);
    masks
}

#[test]
fn test_zone_masks_deterministic() {
    let m1 = make_masks(TERRAIN_ZONE_DETERMINISM_SEED);
    let m2 = make_masks(TERRAIN_ZONE_DETERMINISM_SEED);
    assert_eq!(
        m1.grass_zone_mask.count_set(),
        m2.grass_zone_mask.count_set()
    );
    assert_eq!(m1.dirt_zone_mask.count_set(), m2.dirt_zone_mask.count_set());
    assert_eq!(
        m1.inland_sand_mask.count_set(),
        m2.inland_sand_mask.count_set()
    );
}

#[test]
fn test_zone_masks_no_overlap() {
    let masks = make_masks(42);
    for y in 0..MAP_HEIGHT {
        for x in 0..MAP_WIDTH {
            let p = (x, y);
            assert!(
                !(masks.grass_zone_mask.get(p) && masks.dirt_zone_mask.get(p)),
                "grass_zone と dirt_zone が ({x},{y}) で重複"
            );
        }
    }
}

#[test]
fn test_zone_masks_no_intersection_with_blocked_cells() {
    let masks = make_masks(99);
    for y in 0..MAP_HEIGHT {
        for x in 0..MAP_WIDTH {
            let p = (x, y);
            let blocked = masks.anchor_mask.get(p)
                || masks.river_mask.get(p)
                || masks.river_protection_band.get(p)
                || masks.final_sand_mask.get(p);
            if blocked {
                assert!(
                    !masks.grass_zone_mask.get(p),
                    "grass_zone が禁止セル ({x},{y}) と交差"
                );
                assert!(
                    !masks.dirt_zone_mask.get(p),
                    "dirt_zone が禁止セル ({x},{y}) と交差"
                );
            }
        }
    }
}

#[test]
fn test_inland_sand_mask_no_intersection_with_river_anchor_sand() {
    let masks = make_masks(7);
    for y in 0..MAP_HEIGHT {
        for x in 0..MAP_WIDTH {
            let p = (x, y);
            if masks.inland_sand_mask.get(p) {
                assert!(
                    !masks.final_sand_mask.get(p),
                    "inland_sand が final_sand と交差 ({x},{y})"
                );
                assert!(
                    !masks.river_mask.get(p),
                    "inland_sand が river と交差 ({x},{y})"
                );
                assert!(
                    !masks.anchor_mask.get(p),
                    "inland_sand が anchor と交差 ({x},{y})"
                );
            }
        }
    }
}

/// アンカー距離 ZONE_DIRT_DIST_MIN..=ZONE_DIRT_DIST_MAX に Dirt ゾーンが
/// 少なくとも 1 セル存在するか（複数候補 seed のいずれかで成立すれば OK）。
#[test]
fn test_dirt_zone_exists_near_anchor() {
    let anchors = AnchorLayout::fixed();
    let dirt_near_anchor = SEED_SUITE_TERRAIN_ZONE_CANDIDATES
        .iter()
        .copied()
        .any(|seed| {
            let mut masks = WorldMasks::from_anchor(&anchors);
            masks.fill_river_from_seed(seed);
            masks.fill_sand_from_river_seed(seed);
            masks.fill_terrain_zones_from_seed(seed);
            let dist_field = compute_anchor_distance_field(&masks.anchor_mask);
            (0..MAP_HEIGHT)
                .flat_map(|y| (0..MAP_WIDTH).map(move |x| (x, y)))
                .any(|p| {
                    let d = dist_field[(p.1 * MAP_WIDTH + p.0) as usize];
                    masks.dirt_zone_mask.get(p)
                        && (ZONE_DIRT_DIST_MIN..=ZONE_DIRT_DIST_MAX).contains(&d)
                })
        });
    assert!(
        dirt_near_anchor,
        "いずれの候補 seed でも Dirt ゾーンがアンカー近傍（dist {}..={}）に現れなかった。\
             候補リストを走査して更新すること",
        ZONE_DIRT_DIST_MIN, ZONE_DIRT_DIST_MAX
    );
}

#[test]
fn test_expand_mask_respects_chamfer_radius_upper_bound() {
    let mut mask = BitGrid::map_sized();
    let origin = (50, 50);
    mask.set(origin, true);

    let expanded = expand_mask(&mask, 9);

    assert!(expanded.get(origin), "origin should remain included");
    assert!(
        expanded.get((51, 50)),
        "orthogonal neighbor with cost 3 should be included"
    );
    assert!(
        expanded.get((51, 51)),
        "diagonal neighbor with cost 4 should be included"
    );
    assert!(
        expanded.get((52, 50)),
        "two-step orthogonal cell with cost 6 should be included"
    );
    assert!(
        expanded.get((53, 50)),
        "cell with cost 9 should be included"
    );
    assert!(
        !expanded.get((53, 51)),
        "cell with minimum chamfer cost 10 must not be included for radius 9"
    );
}
