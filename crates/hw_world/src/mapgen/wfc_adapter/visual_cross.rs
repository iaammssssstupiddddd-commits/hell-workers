use super::*;

// ── 視覚十字境界修正 ──────────────────────────────────────────────────────────

fn zone_class_at(masks: &WorldMasks, x: i32, y: i32) -> u8 {
    if masks.grass_zone_mask.get((x, y)) {
        0
    } else if masks.dirt_zone_mask.get((x, y)) {
        2
    } else {
        1
    }
}

fn set_zone_class_at(masks: &mut WorldMasks, x: i32, y: i32, zone: u8) {
    masks.grass_zone_mask.set((x, y), zone == 0);
    masks.dirt_zone_mask.set((x, y), zone == 2);
}

/// ゾーンマスクの2×2対角パターン（ゾーン十字）を除去する。
/// terrain の enforce_no_visual_cross_2x2 では地形変更だけでは修正できない
/// ゾーン境界起因の十字を、ゾーンマスク自体を修正することで防ぐ。
/// 距離フィールドも再計算する。
pub(crate) fn fix_zone_mask_crosses(masks: &mut WorldMasks) {
    use crate::terrain_zones::compute_zone_distance_field;
    const MAX_PASSES: u32 = 64;
    let mut modified = false;
    for _ in 0..MAX_PASSES {
        let mut fixed = false;
        for y in 0..MAP_HEIGHT - 1 {
            for x in 0..MAP_WIDTH - 1 {
                let z = [
                    zone_class_at(masks, x, y),
                    zone_class_at(masks, x + 1, y),
                    zone_class_at(masks, x, y + 1),
                    zone_class_at(masks, x + 1, y + 1),
                ];
                if z[0] != z[1] && z[2] != z[3] && z[0] != z[2] && z[1] != z[3] {
                    // ゾーン十字: BR を BL と同じゾーンに合わせる
                    set_zone_class_at(masks, x + 1, y + 1, z[2]);
                    fixed = true;
                    modified = true;
                }
            }
        }
        if !fixed {
            break;
        }
    }
    if modified {
        masks.grass_zone_distance_field = compute_zone_distance_field(&masks.grass_zone_mask);
        masks.dirt_zone_distance_field = compute_zone_distance_field(&masks.dirt_zone_mask);
    }
}

fn visual_key_at(tiles: &[TerrainType], masks: &WorldMasks, x: i32, y: i32) -> u8 {
    tiles[(y * MAP_WIDTH + x) as usize].priority() * 3 + zone_class_at(masks, x, y)
}

fn is_visual_cross_2x2(tiles: &[TerrainType], masks: &WorldMasks, x: i32, y: i32) -> bool {
    let a = visual_key_at(tiles, masks, x, y);
    let b = visual_key_at(tiles, masks, x + 1, y);
    let c = visual_key_at(tiles, masks, x, y + 1);
    let d = visual_key_at(tiles, masks, x + 1, y + 1);
    a != b && c != d && a != c && b != d
}

#[cfg(test)]
pub(super) fn has_any_visual_cross_2x2(tiles: &[TerrainType], masks: &WorldMasks) -> bool {
    for y in 0..MAP_HEIGHT - 1 {
        for x in 0..MAP_WIDTH - 1 {
            if is_visual_cross_2x2(tiles, masks, x, y) {
                return true;
            }
        }
    }
    false
}

fn can_assign(tiles: &[TerrainType], masks: &WorldMasks, x: i32, y: i32, t: TerrainType) -> bool {
    // ハード制約: マスク固定セルは変更不可
    if masks.river_mask.get((x, y)) {
        return false;
    }
    if masks.final_sand_mask.get((x, y)) {
        return false;
    }
    if masks.rock_field_mask.get((x, y)) {
        return t == TerrainType::Dirt;
    }
    if t == TerrainType::River {
        return false;
    }
    const DIRS: [(i32, i32); 4] = [(1, 0), (-1, 0), (0, 1), (0, -1)];
    if t == TerrainType::Sand {
        // Sand は既存 Sand/River に隣接する場合のみ自然な拡張として許可
        return DIRS.iter().any(|&(dx, dy)| {
            let nx = x + dx;
            let ny = y + dy;
            if !(0..MAP_WIDTH).contains(&nx) || !(0..MAP_HEIGHT).contains(&ny) {
                return false;
            }
            let nb = tiles[(ny * MAP_WIDTH + nx) as usize];
            nb == TerrainType::Sand || nb == TerrainType::River
        });
    }
    // Grass/Dirt は River 隣接不可
    for (dx, dy) in DIRS {
        let nx = x + dx;
        let ny = y + dy;
        if !(0..MAP_WIDTH).contains(&nx) || !(0..MAP_HEIGHT).contains(&ny) {
            continue;
        }
        if tiles[(ny * MAP_WIDTH + nx) as usize] == TerrainType::River {
            return false;
        }
    }
    true
}

fn is_zone_locked(masks: &WorldMasks, x: i32, y: i32) -> bool {
    masks.river_mask.get((x, y))
        || masks.final_sand_mask.get((x, y))
        || masks.rock_field_mask.get((x, y))
}

/// 1セルまたは2セル変更で視覚十字を修正する。
/// 修正したセルを `changed` に追加して返す（修正できた場合 true）。
fn try_fix_visual_cross_2x2(
    tiles: &mut [TerrainType],
    masks: &mut WorldMasks,
    x: i32,
    y: i32,
    protected: &std::collections::HashSet<(i32, i32)>,
    changed: &mut Vec<(i32, i32)>,
) -> bool {
    let candidates = [(x + 1, y + 1), (x, y + 1), (x + 1, y), (x, y)];
    const TRY_TERRAINS: [TerrainType; 3] =
        [TerrainType::Grass, TerrainType::Dirt, TerrainType::Sand];

    // Phase 1: 地形のみ変更（1セル）
    for (cx, cy) in candidates {
        if protected.contains(&(cx, cy)) {
            continue;
        }
        let orig = tiles[(cy * MAP_WIDTH + cx) as usize];
        for &t in &TRY_TERRAINS {
            if t == orig {
                continue;
            }
            if can_assign(tiles, masks, cx, cy, t) {
                tiles[(cy * MAP_WIDTH + cx) as usize] = t;
                if t == TerrainType::Sand {
                    masks.final_sand_mask.set((cx, cy), true);
                }
                if !is_visual_cross_2x2(tiles, masks, x, y) {
                    changed.push((cx, cy));
                    return true;
                }
                tiles[(cy * MAP_WIDTH + cx) as usize] = orig;
                if t == TerrainType::Sand {
                    masks.final_sand_mask.set((cx, cy), false);
                }
            }
        }
    }

    // Phase 2: ゾーンクラス変更（地形変更も組み合わせる、1セル）
    for (cx, cy) in candidates {
        if protected.contains(&(cx, cy)) || is_zone_locked(masks, cx, cy) {
            continue;
        }
        let orig_terrain = tiles[(cy * MAP_WIDTH + cx) as usize];
        let orig_zone = zone_class_at(masks, cx, cy);
        for new_zone in [0u8, 1, 2] {
            if new_zone == orig_zone {
                continue;
            }
            set_zone_class_at(masks, cx, cy, new_zone);
            // ゾーン変更のみ
            if !is_visual_cross_2x2(tiles, masks, x, y) {
                changed.push((cx, cy));
                return true;
            }
            // ゾーン変更 + 地形変更の組み合わせ
            for &t in &TRY_TERRAINS {
                if t == orig_terrain {
                    continue;
                }
                if can_assign(tiles, masks, cx, cy, t) {
                    tiles[(cy * MAP_WIDTH + cx) as usize] = t;
                    if t == TerrainType::Sand {
                        masks.final_sand_mask.set((cx, cy), true);
                    }
                    if !is_visual_cross_2x2(tiles, masks, x, y) {
                        changed.push((cx, cy));
                        return true;
                    }
                    tiles[(cy * MAP_WIDTH + cx) as usize] = orig_terrain;
                    if t == TerrainType::Sand {
                        masks.final_sand_mask.set((cx, cy), false);
                    }
                }
            }
            set_zone_class_at(masks, cx, cy, orig_zone);
        }
    }

    // Phase 3: 2セル同時変更（Phase 1/2 で修正できない場合）
    for i in 0..candidates.len() {
        for j in (i + 1)..candidates.len() {
            let (cx1, cy1) = candidates[i];
            let (cx2, cy2) = candidates[j];
            if protected.contains(&(cx1, cy1)) || protected.contains(&(cx2, cy2)) {
                continue;
            }
            let orig1 = tiles[(cy1 * MAP_WIDTH + cx1) as usize];
            let orig2 = tiles[(cy2 * MAP_WIDTH + cx2) as usize];
            for &t1 in &TRY_TERRAINS {
                if !can_assign(tiles, masks, cx1, cy1, t1) {
                    continue;
                }
                tiles[(cy1 * MAP_WIDTH + cx1) as usize] = t1;
                if t1 == TerrainType::Sand {
                    masks.final_sand_mask.set((cx1, cy1), true);
                }
                for &t2 in &TRY_TERRAINS {
                    if !can_assign(tiles, masks, cx2, cy2, t2) {
                        continue;
                    }
                    tiles[(cy2 * MAP_WIDTH + cx2) as usize] = t2;
                    if t2 == TerrainType::Sand {
                        masks.final_sand_mask.set((cx2, cy2), true);
                    }
                    if !is_visual_cross_2x2(tiles, masks, x, y) {
                        changed.push((cx1, cy1));
                        changed.push((cx2, cy2));
                        return true;
                    }
                    tiles[(cy2 * MAP_WIDTH + cx2) as usize] = orig2;
                    if t2 == TerrainType::Sand {
                        masks.final_sand_mask.set((cx2, cy2), false);
                    }
                }
                tiles[(cy1 * MAP_WIDTH + cx1) as usize] = orig1;
                if t1 == TerrainType::Sand {
                    masks.final_sand_mask.set((cx1, cy1), false);
                }
            }
        }
    }
    false
}

fn count_visual_crosses(tiles: &[TerrainType], masks: &WorldMasks) -> u32 {
    (0..MAP_HEIGHT - 1)
        .flat_map(|y| (0..MAP_WIDTH - 1).map(move |x| (x, y)))
        .filter(|&(x, y)| is_visual_cross_2x2(tiles, masks, x, y))
        .count() as u32
}

pub(super) fn enforce_no_visual_cross_2x2(tiles: &mut [TerrainType], masks: &mut WorldMasks) {
    use std::collections::HashSet;
    const MAX_PASSES: u32 = 128;

    // Stage 1: 保護セット付き multi-pass（振動防止）
    let mut protected: HashSet<(i32, i32)> = HashSet::new();
    for _ in 0..MAX_PASSES {
        let mut fixed_any = false;
        let mut changed = Vec::new();
        for y in 0..MAP_HEIGHT - 1 {
            for x in 0..MAP_WIDTH - 1 {
                if is_visual_cross_2x2(tiles, masks, x, y) {
                    changed.clear();
                    if try_fix_visual_cross_2x2(tiles, masks, x, y, &protected, &mut changed) {
                        protected.extend(changed.iter().copied());
                        fixed_any = true;
                    }
                }
            }
        }
        if !fixed_any {
            break;
        }
    }

    // Stage 2: 残存クロスがある場合、保護セットをリセットして再試行
    // （Stage 1 で別クロス修正に使われたセルが保護されているケースに対処）
    if count_visual_crosses(tiles, masks) == 0 {
        return;
    }
    let mut prev_count = count_visual_crosses(tiles, masks);
    for _ in 0..MAX_PASSES {
        let empty_protected: HashSet<(i32, i32)> = HashSet::new();
        let mut fixed_any = false;
        let mut changed = Vec::new();
        for y in 0..MAP_HEIGHT - 1 {
            for x in 0..MAP_WIDTH - 1 {
                if is_visual_cross_2x2(tiles, masks, x, y) {
                    changed.clear();
                    if try_fix_visual_cross_2x2(tiles, masks, x, y, &empty_protected, &mut changed)
                    {
                        fixed_any = true;
                    }
                }
            }
        }
        if !fixed_any {
            break;
        }
        let new_count = count_visual_crosses(tiles, masks);
        if new_count >= prev_count {
            // 進捗なし（振動）: 停止
            break;
        }
        prev_count = new_count;
        if new_count == 0 {
            break;
        }
    }
}
