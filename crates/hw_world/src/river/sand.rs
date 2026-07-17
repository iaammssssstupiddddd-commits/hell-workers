use super::*;

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
pub(super) const CARDINAL_DIRS_4: [(i32, i32); 4] = [(0, -1), (1, 0), (0, 1), (-1, 0)];
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
