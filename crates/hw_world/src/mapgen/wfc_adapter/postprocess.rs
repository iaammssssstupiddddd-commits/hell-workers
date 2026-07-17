use super::*;

/// WFC が全試行で収束しなかった場合の安全マップ（MS-WFC-2d 版）。
/// hard constraint（River マスク・anchor 禁止）は維持しつつ、
/// final_sand_mask 上は Sand、残りは Grass で埋める。
/// MS-WFC-2.5 以降はゾーンバイアスと inland_sand も適用する。
pub(crate) fn fallback_terrain(masks: &WorldMasks, master_seed: u64) -> Vec<TerrainType> {
    let mut tiles = vec![TerrainType::Grass; (MAP_WIDTH * MAP_HEIGHT) as usize];
    for y in 0..MAP_HEIGHT {
        for x in 0..MAP_WIDTH {
            let idx = (y * MAP_WIDTH + x) as usize;
            if masks.river_mask.get((x, y)) {
                tiles[idx] = TerrainType::River;
            } else if masks.final_sand_mask.get((x, y)) {
                tiles[idx] = TerrainType::Sand;
            }
        }
    }
    // attempt 非依存の専用 seed で zone / inland_sand を適用
    let mut rng = StdRng::seed_from_u64(fallback_post_seed(master_seed));
    apply_zone_post_process(&mut tiles, masks, &mut rng);
    tiles
}

/// `fallback_terrain` 専用の post-process seed。attempt 非依存。
fn fallback_post_seed(master_seed: u64) -> u64 {
    master_seed ^ 0xfb7c_3a91_d5e2_4608
}

/// Step 4（ゾーンバイアス）、Step 4.5（rock field dirt 強制）、
/// Step 5（inland sand）を共通化したヘルパ。
/// `post_process_tiles` と `fallback_terrain` 両方から呼ぶ。
fn apply_zone_post_process(tiles: &mut [TerrainType], masks: &WorldMasks, rng: &mut StdRng) {
    // Step 4: zone bias（B: 確率的フリップ・強制率を範囲でランダム化）
    // + C: ゾーン端部グラデーションバイアス
    // + 完全中立リージョンバイアス（8×8 タイル単位でリージョンを Grass/Dirt 寄りに振り分け）
    let region_seed: u64 = Rng::r#gen::<u64>(rng);
    for y in 0..MAP_HEIGHT {
        for x in 0..MAP_WIDTH {
            let idx = (y * MAP_WIDTH + x) as usize;
            if masks.river_mask.get((x, y))
                || tiles[idx] == TerrainType::River
                || tiles[idx] == TerrainType::Sand
            {
                continue;
            }
            if masks.grass_zone_mask.get((x, y)) {
                // B: Grass ゾーン → 強制率を [ENFORCE_MIN, ENFORCE_MAX] からランダムに選ぶ
                let threshold = rng.gen_range(ZONE_GRASS_ENFORCE_MIN..=ZONE_GRASS_ENFORCE_MAX);
                if tiles[idx] == TerrainType::Dirt && rng.gen_range(0..100) < threshold {
                    tiles[idx] = TerrainType::Grass;
                }
            } else if masks.dirt_zone_mask.get((x, y)) {
                // B: Dirt ゾーン → 強制率を [ENFORCE_MIN, ENFORCE_MAX] からランダムに選ぶ
                let threshold = rng.gen_range(ZONE_DIRT_ENFORCE_MIN..=ZONE_DIRT_ENFORCE_MAX);
                if tiles[idx] == TerrainType::Grass && rng.gen_range(0..100) < threshold {
                    tiles[idx] = TerrainType::Dirt;
                }
            } else {
                // C: ゾーン端部 ZONE_GRADIENT_WIDTH マス以内の中立セルにグラデーションバイアス
                // 両ゾーンが範囲内の場合は近い方を優先
                let dirt_dist = masks.dirt_zone_distance_field[idx];
                let grass_dist = masks.grass_zone_distance_field[idx];
                let dirt_near = dirt_dist <= ZONE_GRADIENT_WIDTH;
                let grass_near = grass_dist <= ZONE_GRADIENT_WIDTH;
                if dirt_near
                    && (!grass_near || dirt_dist <= grass_dist)
                    && tiles[idx] == TerrainType::Grass
                    && rng.gen_range(0..100) < ZONE_GRADIENT_DIRT_BIAS_PERCENT
                {
                    tiles[idx] = TerrainType::Dirt;
                } else if grass_near
                    && tiles[idx] == TerrainType::Dirt
                    && rng.gen_range(0..100) < ZONE_GRADIENT_GRASS_BIAS_PERCENT
                {
                    tiles[idx] = TerrainType::Grass;
                } else if !dirt_near && !grass_near {
                    // 完全中立: 8×8 リージョン単位で Grass/Dirt 寄りに振り分け
                    let rx = (x / NEUTRAL_REGION_SIZE) as u64;
                    let ry = (y / NEUTRAL_REGION_SIZE) as u64;
                    let h = rx
                        .wrapping_mul(2_654_435_761u64)
                        .wrapping_add(ry.wrapping_mul(1_234_567_891u64))
                        .wrapping_add(region_seed);
                    if h & 1 == 0 {
                        if tiles[idx] == TerrainType::Dirt
                            && rng.gen_range(0..100) < NEUTRAL_REGION_BIAS_PERCENT
                        {
                            tiles[idx] = TerrainType::Grass;
                        }
                    } else if tiles[idx] == TerrainType::Grass
                        && rng.gen_range(0..100) < NEUTRAL_REGION_BIAS_PERCENT
                    {
                        tiles[idx] = TerrainType::Dirt;
                    }
                }
            }
        }
    }
    // Step 4.5: rock fields（zone bias 後に Dirt を強制）
    for y in 0..MAP_HEIGHT {
        for x in 0..MAP_WIDTH {
            if masks.rock_field_mask.get((x, y)) {
                tiles[(y * MAP_WIDTH + x) as usize] = TerrainType::Dirt;
            }
        }
    }

    // Step 5: inland sand（zone bias 後の状態を参照）
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
    for y in 0..MAP_HEIGHT {
        for x in 0..MAP_WIDTH {
            let idx = (y * MAP_WIDTH + x) as usize;
            if !masks.inland_sand_mask.get((x, y)) {
                continue;
            }
            if tiles[idx] == TerrainType::River || tiles[idx] == TerrainType::Sand {
                continue;
            }
            let all_grass = OCTILE_DIRS.iter().all(|&(dx, dy)| {
                let nx = x + dx;
                let ny = y + dy;
                if !(0..MAP_WIDTH).contains(&nx) || !(0..MAP_HEIGHT).contains(&ny) {
                    return false;
                }
                tiles[(ny * MAP_WIDTH + nx) as usize] == TerrainType::Grass
            });
            if all_grass {
                tiles[idx] = TerrainType::Sand;
            }
        }
    }
}

/// WFC 後のポスト処理（MS-WFC-2d 版）。
///
/// wfc ライブラリの制約として、weighted パターンの `forbid_pattern` は
/// priority queue を stale にするため `WorldConstraints::forbid()` では適用できない。
/// 代わりにここで `final_sand_mask` を主軸として以下の制約を強制する:
///
/// 処理順:
/// 1. river_mask セルは常に River のまま（WFC で固定済み）
/// 2. final_sand_mask セルは強制 Sand（WFC 結果に関わらず上書き）
/// 3. final_sand_mask 外で terrain == Sand の stray Sand を Grass/Dirt に置換
/// 4. ゾーンバイアス + inland sand（apply_zone_post_process に委譲）
pub(super) fn post_process_tiles(
    tiles: &mut [TerrainType],
    masks: &mut WorldMasks,
    rng: &mut StdRng,
) {
    let total = WEIGHT_GRASS + WEIGHT_DIRT;
    for y in 0..MAP_HEIGHT {
        for x in 0..MAP_WIDTH {
            let idx = (y * MAP_WIDTH + x) as usize;
            if masks.river_mask.get((x, y)) {
                // River は WFC で固定済み。変更しない。
                continue;
            }
            if masks.final_sand_mask.get((x, y)) {
                // マスク上のセルは必ず Sand に揃える
                tiles[idx] = TerrainType::Sand;
            } else if tiles[idx] == TerrainType::Sand {
                // マスク外の stray Sand を Grass/Dirt に落とす
                let r = rng.gen_range(0..total);
                tiles[idx] = if r < WEIGHT_GRASS {
                    TerrainType::Grass
                } else {
                    TerrainType::Dirt
                };
            }
        }
    }
    apply_zone_post_process(tiles, masks, rng);
    enforce_no_visual_cross_2x2(tiles, masks);
}
