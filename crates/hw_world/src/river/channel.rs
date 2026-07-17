use super::*;

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
/// center_y・width 配列に適用する 1D 移動平均のパス数。
/// 値を増やすほど川の蛇行が滑らかになる。
const RIVER_SMOOTH_PASSES: usize = 3;
/// 全体タイル数の目安（検証テスト用; seed によって変動可）
pub const RIVER_TOTAL_TILES_TARGET_MIN: usize = 200;
pub const RIVER_TOTAL_TILES_TARGET_MAX: usize = 500;

/// seed から deterministic な左端→右端横断川を生成する。
///
/// # 引数
/// - `seed`: 乱数シード（同一 seed で同一結果）
/// - `anchor_mask`: Site ∪ Yard の占有セル（`WorldMasks::from_anchor` 済み）
/// - `river_protection_band`: アンカー外周 PROTECTION_BAND_RIVER_WIDTH の禁止帯
///
/// # 戻り値
/// `(river_mask, river_centerline)`
///
/// # アルゴリズム
/// 1. RNG で各列の `center_y` と `width` を生配列として生成。
/// 2. `smooth_1d_f32` で `RIVER_SMOOTH_PASSES` 回の移動平均を適用し、列ごとの急変を抑制。
/// 3. 平滑化後の値を `round() as i32` で整数に変換し、`river_mask` と `centerline` を構築。
///    タイル書き込み時は既存の保護帯フィルタを維持する。
pub fn generate_river_mask(
    seed: u64,
    anchor_mask: &BitGrid,
    river_protection_band: &BitGrid,
) -> (BitGrid, Vec<GridPos>) {
    let mut rng = StdRng::seed_from_u64(seed);
    let map_width = MAP_WIDTH as usize;

    let start_y = rng.gen_range(RIVER_START_Y_MIN..=RIVER_START_Y_MAX);
    let mut current_y = start_y;

    // 蛇行バイアス: -1 が 2/7, 0 が 3/7, +1 が 2/7（期待値 0、標準偏差 ≈ 0.93）
    let steps: &[i32] = &[-1, -1, 0, 0, 0, 1, 1];

    // Phase 1: RNG で生配列を生成（保護帯チェックは逐次維持）
    let mut raw_center_y: Vec<f32> = Vec::with_capacity(map_width);
    let mut raw_width: Vec<i32> = Vec::with_capacity(map_width);

    for x in 0..MAP_WIDTH {
        let step = *steps.choose(&mut rng).unwrap();
        let mut next_y = (current_y + step).clamp(RIVER_Y_CLAMP_MIN, RIVER_Y_CLAMP_MAX);

        // next_y が禁止セルなら直進（current_y を維持）
        if river_protection_band.get((x, next_y)) || anchor_mask.get((x, next_y)) {
            next_y = current_y;
        }

        current_y = next_y;
        raw_center_y.push(current_y as f32);

        let width = rng.gen_range(RIVER_MIN_WIDTH..=RIVER_MAX_WIDTH);
        raw_width.push(width);
    }

    // Phase 2: center_y に移動平均スムージングを適用（f32 で処理し精度損失を防ぐ）。
    // width はランダム性を維持し川岸の有機的な変化を保つ。
    let smoothed_center_y = smooth_1d_f32(&raw_center_y, RIVER_SMOOTH_PASSES);

    // Phase 3: スムージング後の配列から river_mask と centerline を構築
    let mut river_mask = BitGrid::map_sized();
    let mut centerline: Vec<GridPos> = Vec::with_capacity(map_width);

    for (x_usize, (&cy_f, &width)) in smoothed_center_y.iter().zip(raw_width.iter()).enumerate() {
        let x = x_usize as i32;
        let center_y = cy_f.round() as i32;

        centerline.push((x, center_y));

        let top = center_y - width / 2;
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

/// 1D 移動平均スムージングを `passes` 回適用する。
///
/// 端点はミラー補外（境界値を 2 回使用）で処理する。
fn smooth_1d_f32(arr: &[f32], passes: usize) -> Vec<f32> {
    let n = arr.len();
    if n < 3 {
        return arr.to_vec();
    }
    let mut result = arr.to_vec();
    for _ in 0..passes {
        let prev = result.clone();
        // 左端: prev[0] をミラー
        result[0] = (prev[0] + prev[0] + prev[1]) / 3.0;
        // 中間
        for i in 1..n - 1 {
            result[i] = (prev[i - 1] + prev[i] + prev[i + 1]) / 3.0;
        }
        // 右端: prev[n-1] をミラー
        result[n - 1] = (prev[n - 2] + prev[n - 1] + prev[n - 1]) / 3.0;
    }
    result
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
