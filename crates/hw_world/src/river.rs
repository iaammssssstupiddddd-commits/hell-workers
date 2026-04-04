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
/// 距離場初期化用 8 近傍（dist==1 は diagonal shoreline を含む）
const EIGHT_DIRS: [(i32, i32); 8] = [
    (0, -1),
    (1, 0),
    (0, 1),
    (-1, 0),
    (1, -1),
    (1, 1),
    (-1, 1),
    (-1, -1),
];
/// carve / growth flood-fill 用 4 近傍（wfc_adapter 依存を避けて独立定義）
const CARDINAL_DIRS_4: [(i32, i32); 4] = [(0, -1), (1, 0), (0, 1), (-1, 0)];
/// 距離場でラベルを付与する最大距離（超過セルは u32::MAX）
pub const SAND_SHORE_MAX_DISTANCE: u32 = 4;
/// base_candidate_mask に含める最小距離
pub const SAND_BASE_DIST_MIN: u32 = 1;
/// base_candidate_mask に含める最大距離
pub const SAND_BASE_DIST_MAX: u32 = 2;
/// growth 起点数の下限
pub const SAND_GROWTH_SEED_COUNT_MIN: u32 = 3;
/// growth 起点数の上限
pub const SAND_GROWTH_SEED_COUNT_MAX: u32 = 8;
/// growth flood が侵入してよい距離場の上限
pub const SAND_GROWTH_DIST_MAX: u32 = 4;
/// growth BFS の起点からの最大ステップ数（距離場制約との二重制限）
pub const SAND_GROWTH_STEP_LIMIT: usize = 6;
/// 1 growth region の最大面積（セル数）
pub const SAND_GROWTH_REGION_AREA_MAX: usize = 30;
/// carve 起点数の下限
pub const SAND_CARVE_SEED_COUNT_MIN: u32 = 3;
/// carve 起点数の上限
pub const SAND_CARVE_SEED_COUNT_MAX: u32 = 7;
/// candidate 全体に対する最大 carve 割合（%）
pub const SAND_CARVE_MAX_RATIO_PERCENT: usize = 35;
/// 1 carve region の最小面積（セル数）
pub const SAND_CARVE_REGION_SIZE_MIN: u32 = 6;
/// 1 carve region の最大面積（セル数）
pub const SAND_CARVE_REGION_SIZE_MAX: u32 = 32;
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

/// アンカー・保護帯なしでプレビュー川を生成し、川タイルの **最小 y** を返す。
///
/// `grid_to_world` では y が大きいほど Bevy の +Y（画面上の上）なので、
/// **最小 y が川の南端（画面下側の端）**に相当する。
pub fn preview_river_min_y(seed: u64) -> i32 {
    let empty_anchor = BitGrid::map_sized();
    let empty_band = BitGrid::map_sized();
    let (river_mask, _) = generate_river_mask(seed, &empty_anchor, &empty_band);
    let mut min_y = i32::MAX;
    let mut any = false;
    for y in 0..MAP_HEIGHT {
        for x in 0..MAP_WIDTH {
            if river_mask.get((x, y)) {
                any = true;
                min_y = min_y.min(y);
            }
        }
    }
    if !any {
        return RIVER_Y_CLAMP_MIN;
    }
    min_y
}

// ── 砂マスク生成 ──────────────────────────────────────────────────────────────

/// seed から deterministic に砂マスク 3 点セットを生成して返す。
///
/// # 戻り値
/// `(sand_candidate_mask, sand_carve_mask, final_sand_mask)`
///
/// 生成フロー:
/// 1. `river_mask` から距離場を計算（dist==1 は 8 近傍 shoreline shell、dist>=2 は 4 近傍展開）
/// 2. dist 1..=2 を `base_candidate_mask` とする
/// 3. dist==1 の frontier から bounded growth を加算して `growth_mask` を作る
/// 4. `candidate = base | growth`
/// 5. candidate に non-sand carve を適用して `final_sand_mask` を得る
///
/// `final_sand_mask` が空になる場合は carve を全捨てして candidate 全面を final とする
/// フォールバックを行う。candidate 自体がゼロの場合は全マスク空で返す。
pub fn generate_sand_masks(
    seed: u64,
    river_mask: &BitGrid,
    anchor_mask: &BitGrid,
    river_protection_band: &BitGrid,
) -> (BitGrid, BitGrid, BitGrid) {
    let mut rng = StdRng::seed_from_u64(seed ^ SAND_SEED_SALT);

    // 1. 距離場（dist==1 は 8 近傍 shoreline、dist>=2 は 4 近傍外側展開）
    let dist_field = compute_river_distance_field(river_mask, anchor_mask, river_protection_band);

    // 2. base shoreline (dist 1..=2)
    let base = build_base_shoreline_mask(&dist_field);

    // 3. additive growth（dist==1 frontier から dist<=SAND_GROWTH_DIST_MAX へ）
    let growth = build_sand_growth_mask(&dist_field, &base, &mut rng);

    // 4. candidate = base | growth
    let mut candidate = base;
    for y in 0..MAP_HEIGHT {
        for x in 0..MAP_WIDTH {
            if growth.get((x, y)) {
                candidate.set((x, y), true);
            }
        }
    }

    // 5. carve
    let mut carve = build_sand_carve_mask(&candidate, &mut rng);

    // 6. final = candidate & !carve
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

/// 4 近傍 BFS で許可セルの river 距離場を計算する。
///
/// 戻り値: `Vec<u32>` indexed by `y * MAP_WIDTH + x`。
/// - 許可セルかつ `dist <= SAND_SHORE_MAX_DISTANCE` のセル: 距離値 1..=SAND_SHORE_MAX_DISTANCE
/// - river/anchor/protection_band 上または未到達セル: `u32::MAX`
///
/// dist==1 は「8 近傍のいずれかが river_mask」な許可セル（diagonal shoreline 許容を維持）。
/// dist>=2 は dist==1 セルからの 4 近傍展開により付与される。
fn compute_river_distance_field(
    river_mask: &BitGrid,
    anchor_mask: &BitGrid,
    river_protection_band: &BitGrid,
) -> Vec<u32> {
    let size = (MAP_WIDTH * MAP_HEIGHT) as usize;
    let mut dist = vec![u32::MAX; size];
    let mut queue: VecDeque<GridPos> = VecDeque::new();

    for y in 0..MAP_HEIGHT {
        for x in 0..MAP_WIDTH {
            let p = (x, y);
            if river_mask.get(p) || anchor_mask.get(p) || river_protection_band.get(p) {
                continue;
            }
            // 8 近傍のいずれかが river なら距離 1 で初期化（2d の diagonal 契約を維持）
            let adjacent_to_river = EIGHT_DIRS.iter().any(|&(dx, dy)| {
                let nx = x + dx;
                let ny = y + dy;
                (0..MAP_WIDTH).contains(&nx)
                    && (0..MAP_HEIGHT).contains(&ny)
                    && river_mask.get((nx, ny))
            });
            if adjacent_to_river {
                let idx = (y * MAP_WIDTH + x) as usize;
                dist[idx] = 1;
                queue.push_back(p);
            }
        }
    }

    while let Some(pos) = queue.pop_front() {
        let d = dist[(pos.1 * MAP_WIDTH + pos.0) as usize];
        if d >= SAND_SHORE_MAX_DISTANCE {
            continue;
        }
        for &(dx, dy) in &CARDINAL_DIRS_4 {
            let nx = pos.0 + dx;
            let ny = pos.1 + dy;
            if !(0..MAP_WIDTH).contains(&nx) || !(0..MAP_HEIGHT).contains(&ny) {
                continue;
            }
            let np = (nx, ny);
            if river_mask.get(np) || anchor_mask.get(np) || river_protection_band.get(np) {
                continue;
            }
            let nidx = (ny * MAP_WIDTH + nx) as usize;
            if dist[nidx] == u32::MAX {
                dist[nidx] = d + 1;
                queue.push_back(np);
            }
        }
    }

    dist
}

fn build_base_shoreline_mask(dist_field: &[u32]) -> BitGrid {
    let mut base = BitGrid::map_sized();
    for y in 0..MAP_HEIGHT {
        for x in 0..MAP_WIDTH {
            let d = dist_field[(y * MAP_WIDTH + x) as usize];
            if (SAND_BASE_DIST_MIN..=SAND_BASE_DIST_MAX).contains(&d) {
                base.set((x, y), true);
            }
        }
    }
    base
}

fn build_sand_growth_mask(
    dist_field: &[u32],
    base_candidate: &BitGrid,
    rng: &mut StdRng,
) -> BitGrid {
    // frontier: dist==1 の base セル（8 近傍 river に直接隣接する岸の芯）
    let frontier: Vec<GridPos> = (0..MAP_HEIGHT)
        .flat_map(|y| {
            (0..MAP_WIDTH).filter_map(move |x| {
                let idx = (y * MAP_WIDTH + x) as usize;
                (base_candidate.get((x, y)) && dist_field[idx] == 1).then_some((x, y))
            })
        })
        .collect();

    let mut growth = BitGrid::map_sized();
    if frontier.is_empty() {
        return growth;
    }

    let num_origins = rng.gen_range(SAND_GROWTH_SEED_COUNT_MIN..=SAND_GROWTH_SEED_COUNT_MAX);
    let k = (num_origins as usize).min(frontier.len());
    let origins: Vec<GridPos> = frontier.choose_multiple(rng, k).copied().collect();

    for origin in origins {
        flood_fill_growth_region(dist_field, &mut growth, origin, SAND_GROWTH_REGION_AREA_MAX);
    }

    growth
}

/// dist_field が SAND_GROWTH_DIST_MAX 以下の許可セルに対して 4 近傍 BFS で最大 `area_max`
/// セル、かつ起点から最大 `SAND_GROWTH_STEP_LIMIT` ステップまで growth する。
/// 戻り値: 実際に growth したセル数。
fn flood_fill_growth_region(
    dist_field: &[u32],
    growth: &mut BitGrid,
    origin: GridPos,
    area_max: usize,
) -> usize {
    let origin_d = dist_field[(origin.1 * MAP_WIDTH + origin.0) as usize];
    if origin_d == u32::MAX || origin_d > SAND_GROWTH_DIST_MAX || growth.get(origin) {
        return 0;
    }

    let mut queue: VecDeque<(GridPos, usize)> = VecDeque::new();
    queue.push_back((origin, 0));
    growth.set(origin, true);
    let mut count = 1usize;

    while let Some((pos, steps)) = queue.pop_front() {
        if count >= area_max {
            break;
        }
        if steps >= SAND_GROWTH_STEP_LIMIT {
            continue;
        }
        for &(dx, dy) in &CARDINAL_DIRS_4 {
            if count >= area_max {
                break;
            }
            let nx = pos.0 + dx;
            let ny = pos.1 + dy;
            if !(0..MAP_WIDTH).contains(&nx) || !(0..MAP_HEIGHT).contains(&ny) {
                continue;
            }
            let np = (nx, ny);
            let nd = dist_field[(ny * MAP_WIDTH + nx) as usize];
            if nd != u32::MAX && nd <= SAND_GROWTH_DIST_MAX && !growth.get(np) {
                growth.set(np, true);
                count += 1;
                queue.push_back((np, steps + 1));
            }
        }
    }
    count
}

fn build_sand_carve_mask(candidate: &BitGrid, rng: &mut StdRng) -> BitGrid {
    let candidate_positions: Vec<GridPos> = (0..MAP_HEIGHT)
        .flat_map(|y| (0..MAP_WIDTH).filter_map(move |x| candidate.get((x, y)).then_some((x, y))))
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
}
