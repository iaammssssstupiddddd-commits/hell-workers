//! 地形バイアスゾーンマスク生成（MS-WFC-2.5）。
//!
//! - `generate_terrain_zone_masks()`: grass_zone_mask / dirt_zone_mask / inland_sand_mask を生成する公開 API
//! - アンカー距離場（D）→ flood fill（B）→ 内陸砂パッチ の順で処理する
//! - 生成したマスクは `WorldMasks::fill_terrain_zones_from_seed()` 経由で `WorldMasks` に格納する

use std::collections::VecDeque;

use hw_core::constants::{MAP_HEIGHT, MAP_WIDTH};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

use crate::world_masks::BitGrid;

// ── 定数 ─────────────────────────────────────────────────────────────────────

/// Dirt ゾーン起点のアンカー距離下限（保護帯 PROTECTION_BAND_RIVER_WIDTH=3 の外）
pub const ZONE_DIRT_DIST_MIN: u32 = 5;
/// Dirt ゾーン起点のアンカー距離上限
pub const ZONE_DIRT_DIST_MAX: u32 = 16;
/// Grass ゾーン起点のアンカー距離下限
pub const ZONE_GRASS_DIST_MIN: u32 = 18;

/// Dirt ゾーン起点数の下限
pub const ZONE_DIRT_SEED_COUNT_MIN: u32 = 2;
/// Dirt ゾーン起点数の上限
pub const ZONE_DIRT_SEED_COUNT_MAX: u32 = 6;
/// Grass ゾーン起点数の下限
pub const ZONE_GRASS_SEED_COUNT_MIN: u32 = 2;
/// Grass ゾーン起点数の上限
pub const ZONE_GRASS_SEED_COUNT_MAX: u32 = 5;

/// 1 Dirt パッチの面積上限（セル数）
pub const ZONE_DIRT_REGION_AREA_MAX: usize = 500;
/// 1 Grass パッチの面積上限（セル数）
pub const ZONE_GRASS_REGION_AREA_MAX: usize = 700;

// ── B: ゾーン強制率（範囲）────────────────────────────────────────────────────
/// Grass ゾーン内 Dirt → Grass 変換確率の下限（%）
pub const ZONE_GRASS_ENFORCE_MIN: u32 = 72;
/// Grass ゾーン内 Dirt → Grass 変換確率の上限（%）
pub const ZONE_GRASS_ENFORCE_MAX: u32 = 98;
/// Dirt ゾーン内 Grass → Dirt 変換確率の下限（%）
pub const ZONE_DIRT_ENFORCE_MIN: u32 = 72;
/// Dirt ゾーン内 Grass → Dirt 変換確率の上限（%）
pub const ZONE_DIRT_ENFORCE_MAX: u32 = 98;

// ── C: ゾーン端部グラデーション定数 ──────────────────────────────────────────
/// ゾーン境界から外側何マス以内の中立セルに C グラデーションバイアスをかけるか
pub const ZONE_GRADIENT_WIDTH: u32 = 3;
/// Dirt ゾーン端部グラデーション内の Grass→Dirt 変換確率（%）
pub const ZONE_GRADIENT_DIRT_BIAS_PERCENT: u32 = 30;
/// Grass ゾーン端部グラデーション内の Dirt→Grass 変換確率（%）
pub const ZONE_GRADIENT_GRASS_BIAS_PERCENT: u32 = 40;

// ── D: ゾーン間離隔定数 ───────────────────────────────────────────────────────
/// Dirt ゾーンと Grass ゾーンの間に設ける最低離隔（マス）
pub const ZONE_MIN_SEPARATION: u32 = 3;

// ── 内陸砂定数 ────────────────────────────────────────────────────────────────

/// 生成する内陸砂パッチ数の下限
pub const INLAND_SAND_PATCH_COUNT_MIN: u32 = 3;
/// 生成する内陸砂パッチ数の上限
pub const INLAND_SAND_PATCH_COUNT_MAX: u32 = 6;
/// 1 パッチの面積上限（セル数）
pub const INLAND_SAND_PATCH_AREA_MAX: usize = 5;

// ── 公開 API ──────────────────────────────────────────────────────────────────

/// grass_zone_mask / dirt_zone_mask / inland_sand_mask を一括生成して返す。
///
/// 戻り値: `(grass_zone_mask, dirt_zone_mask, inland_sand_mask)`
///
/// 各マスクは互いに排他であり、river_mask / anchor_mask / river_protection_band /
/// final_sand_mask と交差しないことが保証される。
pub fn generate_terrain_zone_masks(
    seed: u64,
    anchor_mask: &BitGrid,
    river_mask: &BitGrid,
    river_protection_band: &BitGrid,
    final_sand_mask: &BitGrid,
) -> (BitGrid, BitGrid, BitGrid) {
    let mut rng = StdRng::seed_from_u64(seed);

    // 許可セルマスク（禁止領域を除いた全セル）
    let mut allowed = BitGrid::map_sized();
    for y in 0..MAP_HEIGHT {
        for x in 0..MAP_WIDTH {
            let p = (x, y);
            if !anchor_mask.get(p)
                && !river_mask.get(p)
                && !river_protection_band.get(p)
                && !final_sand_mask.get(p)
            {
                allowed.set(p, true);
            }
        }
    }

    // D: アンカーからの距離場
    let dist_field = compute_anchor_distance_field(anchor_mask);

    // B1: Dirt ゾーン
    let dirt_seeds = pick_zone_seeds(
        &mut rng,
        &dist_field,
        &allowed,
        ZONE_DIRT_DIST_MIN,
        ZONE_DIRT_DIST_MAX,
        ZONE_DIRT_SEED_COUNT_MIN,
        ZONE_DIRT_SEED_COUNT_MAX,
    );
    let dirt_zone_mask = flood_fill_zone_patches(&dirt_seeds, &allowed, ZONE_DIRT_REGION_AREA_MAX);

    // B2: Grass ゾーン（Dirt ゾーンから ZONE_MIN_SEPARATION マス以内を除外）
    let allowed_for_grass = {
        let mut a = allowed.clone();
        let dirt_buffer = expand_mask(&dirt_zone_mask, ZONE_MIN_SEPARATION);
        for y in 0..MAP_HEIGHT {
            for x in 0..MAP_WIDTH {
                if dirt_buffer.get((x, y)) {
                    a.set((x, y), false);
                }
            }
        }
        a
    };
    let grass_seeds = pick_zone_seeds(
        &mut rng,
        &dist_field,
        &allowed_for_grass,
        ZONE_GRASS_DIST_MIN,
        u32::MAX, // 上限なし
        ZONE_GRASS_SEED_COUNT_MIN,
        ZONE_GRASS_SEED_COUNT_MAX,
    );
    let grass_zone_mask =
        flood_fill_zone_patches(&grass_seeds, &allowed_for_grass, ZONE_GRASS_REGION_AREA_MAX);

    debug_assert!(
        !(0..MAP_HEIGHT)
            .flat_map(|y| (0..MAP_WIDTH).map(move |x| (x, y)))
            .any(|p| grass_zone_mask.get(p) && dirt_zone_mask.get(p)),
        "grass_zone と dirt_zone が重複しています"
    );

    // 内陸砂マスク（grass_zone 内の小パッチ）
    let inland_sand_mask = generate_inland_sand_mask(
        &mut rng,
        &grass_zone_mask,
        anchor_mask,
        river_mask,
        river_protection_band,
        final_sand_mask,
    );

    (grass_zone_mask, dirt_zone_mask, inland_sand_mask)
}

// ── 内部関数 ──────────────────────────────────────────────────────────────────

/// mask の true セルを多起点 BFS の起点（距離 0）として、全セルへの最短距離を返す。
fn distance_field_from_mask(mask: &BitGrid) -> Vec<u32> {
    let w = MAP_WIDTH;
    let h = MAP_HEIGHT;
    let mut dist = vec![u32::MAX; (w * h) as usize];
    let mut queue: VecDeque<(i32, i32)> = VecDeque::new();
    const DIRS: [(i32, i32); 4] = [(0, 1), (0, -1), (1, 0), (-1, 0)];

    for y in 0..h {
        for x in 0..w {
            if mask.get((x, y)) {
                dist[(y * w + x) as usize] = 0;
                queue.push_back((x, y));
            }
        }
    }
    while let Some((cx, cy)) = queue.pop_front() {
        let d = dist[(cy * w + cx) as usize];
        for (dx, dy) in DIRS {
            let nx = cx + dx;
            let ny = cy + dy;
            if nx < 0 || nx >= w || ny < 0 || ny >= h {
                continue;
            }
            let idx = (ny * w + nx) as usize;
            if dist[idx] == u32::MAX {
                dist[idx] = d + 1;
                queue.push_back((nx, ny));
            }
        }
    }
    dist
}

/// アンカーセルを多起点 BFS の起点（距離 0）として、全セルへの最短距離を返す。
///
/// アンカーセルは 0、アンカー隣接セルは 1、以降 +1。
pub(crate) fn compute_anchor_distance_field(anchor_mask: &BitGrid) -> Vec<u32> {
    distance_field_from_mask(anchor_mask)
}

/// ゾーンマスクの true セルを多起点 BFS の起点として、全セルへの最短距離を返す。
///
/// ゾーンセル自体は 0。ゾーンが空の場合は全セル u32::MAX。
/// C グラデーション（ゾーン端部 N マス以内の中立セルへのバイアス）に使用する。
pub(crate) fn compute_zone_distance_field(zone_mask: &BitGrid) -> Vec<u32> {
    distance_field_from_mask(zone_mask)
}

/// mask の true セルから 4 近傍 BFS で radius マス以内を全て true にした BitGrid を返す。
/// 元の mask 自体も結果に含まれる。
fn expand_mask(mask: &BitGrid, radius: u32) -> BitGrid {
    let mut result = mask.clone();
    if radius == 0 {
        return result;
    }
    let w = MAP_WIDTH;
    let h = MAP_HEIGHT;
    let mut dist = vec![u32::MAX; (w * h) as usize];
    let mut queue: VecDeque<(i32, i32)> = VecDeque::new();
    const DIRS: [(i32, i32); 4] = [(0, 1), (0, -1), (1, 0), (-1, 0)];

    for y in 0..h {
        for x in 0..w {
            if mask.get((x, y)) {
                dist[(y * w + x) as usize] = 0;
                queue.push_back((x, y));
            }
        }
    }
    while let Some((cx, cy)) = queue.pop_front() {
        let d = dist[(cy * w + cx) as usize];
        if d >= radius {
            continue;
        }
        for (dx, dy) in DIRS {
            let nx = cx + dx;
            let ny = cy + dy;
            if nx < 0 || nx >= w || ny < 0 || ny >= h {
                continue;
            }
            let idx = (ny * w + nx) as usize;
            if dist[idx] == u32::MAX {
                dist[idx] = d + 1;
                result.set((nx, ny), true);
                queue.push_back((nx, ny));
            }
        }
    }
    result
}

/// 距離・許可マスク条件を満たすセルから RNG でゾーン起点を選択する。
///
/// `dist_max` に `u32::MAX` を渡すと上限なし（Grass ゾーン用）。
fn pick_zone_seeds(
    rng: &mut StdRng,
    dist_field: &[u32],
    allowed_mask: &BitGrid,
    dist_min: u32,
    dist_max: u32,
    count_min: u32,
    count_max: u32,
) -> Vec<(i32, i32)> {
    let mut candidates: Vec<(i32, i32)> = (0..MAP_HEIGHT)
        .flat_map(|y| (0..MAP_WIDTH).map(move |x| (x, y)))
        .filter(|&(x, y)| {
            let d = dist_field[(y * MAP_WIDTH + x) as usize];
            d >= dist_min && d <= dist_max && allowed_mask.get((x, y))
        })
        .collect();

    if candidates.is_empty() {
        return Vec::new();
    }
    let count = (rng.gen_range(count_min..=count_max) as usize).min(candidates.len());
    // partial Fisher-Yates: 先頭 count 個だけシャッフルして返す
    for i in 0..count {
        let j = rng.gen_range(i..candidates.len());
        candidates.swap(i, j);
    }
    candidates.truncate(count);
    candidates
}

/// seeds から順に 4 近傍 flood fill で BitGrid を生成する（結果は同一 result に累積）。
///
/// allowed_mask 外への展開は行わない。
/// area_max は 1 起点ごとの上限（seed 間で共有しない）。
fn flood_fill_zone_patches(
    seeds: &[(i32, i32)],
    allowed_mask: &BitGrid,
    area_max: usize,
) -> BitGrid {
    let mut result = BitGrid::map_sized();
    const DIRS: [(i32, i32); 4] = [(0, 1), (0, -1), (1, 0), (-1, 0)];

    for &origin in seeds {
        if !allowed_mask.get(origin) || result.get(origin) {
            continue;
        }
        let mut queue: VecDeque<(i32, i32)> = VecDeque::new();
        queue.push_back(origin);
        result.set(origin, true);
        let mut count = 1usize;

        'outer: while let Some(pos) = queue.pop_front() {
            for (dx, dy) in DIRS {
                if count >= area_max {
                    break 'outer;
                }
                let np = (pos.0 + dx, pos.1 + dy);
                if allowed_mask.get(np) && !result.get(np) {
                    result.set(np, true);
                    count += 1;
                    queue.push_back(np);
                }
            }
        }
    }
    result
}

/// grass_zone_mask 内に小さな砂地パッチを生成する。
///
/// パッチの flood fill は 4 近傍。採用判定（8 近傍 Grass チェック）のみ斜め含む。
/// パッチ全体の 8 近傍が grass_zone_mask に収まらない場合はそのパッチを棄却する。
fn generate_inland_sand_mask(
    rng: &mut StdRng,
    grass_zone_mask: &BitGrid,
    anchor_mask: &BitGrid,
    river_mask: &BitGrid,
    river_protection_band: &BitGrid,
    final_sand_mask: &BitGrid,
) -> BitGrid {
    // 候補セル: grass_zone かつ全禁止マスクを通過
    let mut candidate = BitGrid::map_sized();
    for y in 0..MAP_HEIGHT {
        for x in 0..MAP_WIDTH {
            let p = (x, y);
            if grass_zone_mask.get(p)
                && !anchor_mask.get(p)
                && !river_mask.get(p)
                && !river_protection_band.get(p)
                && !final_sand_mask.get(p)
            {
                candidate.set(p, true);
            }
        }
    }

    let mut cand_list: Vec<(i32, i32)> = (0..MAP_HEIGHT)
        .flat_map(|y| (0..MAP_WIDTH).map(move |x| (x, y)))
        .filter(|&p| candidate.get(p))
        .collect();
    if cand_list.is_empty() {
        return BitGrid::map_sized();
    }
    let patch_count = (rng.gen_range(INLAND_SAND_PATCH_COUNT_MIN..=INLAND_SAND_PATCH_COUNT_MAX)
        as usize)
        .min(cand_list.len());
    // partial Fisher-Yates で起点を選択
    for i in 0..patch_count {
        let j = rng.gen_range(i..cand_list.len());
        cand_list.swap(i, j);
    }

    let mut result = BitGrid::map_sized();
    const DIRS: [(i32, i32); 4] = [(0, 1), (0, -1), (1, 0), (-1, 0)];
    const OCTILE_DIRS: [(i32, i32); 8] = [
        (0, 1),
        (0, -1),
        (1, 0),
        (-1, 0),
        (1, 1),
        (1, -1),
        (-1, 1),
        (-1, -1),
    ];

    for &origin in &cand_list[..patch_count] {
        if !candidate.get(origin) || result.get(origin) {
            continue;
        }
        // 4 近傍 flood fill でパッチ収集
        let mut patch: Vec<(i32, i32)> = Vec::new();
        let mut visited = BitGrid::map_sized();
        let mut queue: VecDeque<(i32, i32)> = VecDeque::new();
        queue.push_back(origin);
        visited.set(origin, true);
        patch.push(origin);

        'fill: while let Some(pos) = queue.pop_front() {
            for (dx, dy) in DIRS {
                if patch.len() >= INLAND_SAND_PATCH_AREA_MAX {
                    break 'fill;
                }
                let np = (pos.0 + dx, pos.1 + dy);
                if candidate.get(np) && !visited.get(np) && !result.get(np) {
                    visited.set(np, true);
                    patch.push(np);
                    queue.push_back(np);
                }
            }
        }

        // パッチ全体の 8 近傍が grass_zone_mask に収まるか検証
        // 境界外は Grass とみなさない（マップ端のパッチを棄却）
        let all_neighbors_in_grass = patch.iter().all(|&(px, py)| {
            OCTILE_DIRS.iter().all(|&(dx, dy)| {
                let np = (px + dx, py + dy);
                if np.0 < 0 || np.0 >= MAP_WIDTH || np.1 < 0 || np.1 >= MAP_HEIGHT {
                    return false;
                }
                grass_zone_mask.get(np)
            })
        });

        if all_neighbors_in_grass {
            for p in patch {
                result.set(p, true);
            }
        }
    }
    result
}

// ── テスト ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
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
                            && d >= ZONE_DIRT_DIST_MIN
                            && d <= ZONE_DIRT_DIST_MAX
                    })
            });
        assert!(
            dirt_near_anchor,
            "いずれの候補 seed でも Dirt ゾーンがアンカー近傍（dist {}..={}）に現れなかった。\
             候補リストを走査して更新すること",
            ZONE_DIRT_DIST_MIN, ZONE_DIRT_DIST_MAX
        );
    }
}
