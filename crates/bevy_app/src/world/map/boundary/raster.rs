use std::collections::VecDeque;

use bevy::prelude::*;
use hw_core::constants::{MAP_HEIGHT, MAP_WIDTH, TILE_SIZE};
use hw_world::{TerrainType, WorldMasks};

use super::extract::{terrain_sand_variant_byte, terrain_zone_bias_byte};
use super::params::{
    BOUNDARY_PROXIMITY_DILATION_PX, TERRAIN_REGION_RES, TERRAIN_REGION_SENTINEL,
    TERRAIN_REGION_UNASSIGNED,
};

/// TerrainType + WorldMasks + タイル座標 → terrain_region_map 用バイト（11 値エンコーディング）。
///
/// Grass:  grass_zone=0, neutral=1, dirt_zone=2
/// Dirt:   grass_zone=85, neutral=86, dirt_zone=87
/// Sand:   regular=170, shore=171, inland=172
/// River:  255
fn terrain_region_byte(t: TerrainType, masks: &WorldMasks, pos: (i32, i32)) -> u8 {
    match t {
        TerrainType::Grass => match terrain_zone_bias_byte(masks, pos) {
            0 => 0,
            255 => 2,
            _ => 1,
        },
        TerrainType::Dirt => match terrain_zone_bias_byte(masks, pos) {
            0 => 85,
            255 => 87,
            _ => 86,
        },
        TerrainType::Sand => terrain_sand_variant_byte(masks, pos),
        TerrainType::River => 255,
    }
}
/// ワールド 2D 座標 → terrain_region_map テクスチャピクセル座標。
///
/// 3D スポーンが `Vec3(wx, 0, -wy)` なので `world_position.z = -world_2d.y`。
/// WGSL `world_to_boundary_uv` の `uv.y = (world_2d.y + half_h) / world_h` と
/// 同一の向き（Y反転なし）で書き込む。
/// grid_y=0 (マップ下端, world_2d.y ≈ -half_h) → py ≈ 0 (テクスチャ上端)。
#[inline]
fn world_to_region_pixel(p: Vec2) -> (f32, f32) {
    let world_w = MAP_WIDTH as f32 * TILE_SIZE;
    let world_h = MAP_HEIGHT as f32 * TILE_SIZE;
    let half_w = world_w / 2.0;
    let half_h = world_h / 2.0;
    let px = (p.x + half_w) / world_w * TERRAIN_REGION_RES as f32;
    let py = (p.y + half_h) / world_h * TERRAIN_REGION_RES as f32;
    (px, py)
}

/// Bresenham ラインを sentinel で 2px 幅に描画する。
fn rasterize_segment_barrier(buf: &mut [u8], res: usize, p0: (f32, f32), p1: (f32, f32)) {
    let (mut x0, mut y0) = (p0.0 as i32, p0.1 as i32);
    let (x1, y1) = (p1.0 as i32, p1.1 as i32);
    let dx = (x1 - x0).abs();
    let dy = (y1 - y0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx - dy;

    let set = |buf: &mut [u8], x: i32, y: i32| {
        if x >= 0 && y >= 0 && (x as usize) < res && (y as usize) < res {
            buf[y as usize * res + x as usize] = TERRAIN_REGION_SENTINEL;
        }
    };

    loop {
        set(buf, x0, y0);
        // 2px 幅: 横方向が支配的なら上下、縦方向が支配的なら左右に 1px 追加
        if dx >= dy {
            set(buf, x0, y0 + 1);
            set(buf, x0, y0 - 1);
        } else {
            set(buf, x0 + 1, y0);
            set(buf, x0 - 1, y0);
        }
        if x0 == x1 && y0 == y1 {
            break;
        }
        let e2 = 2 * err;
        if e2 > -dy {
            err -= dy;
            x0 += sx;
        }
        if e2 < dx {
            err += dx;
            y0 += sy;
        }
    }
}

/// sentinel blob（半径 r px の円）を描画してギャップを閉鎖する。
fn rasterize_blob(buf: &mut [u8], res: usize, center: (f32, f32), radius: i32) {
    let cx = center.0 as i32;
    let cy = center.1 as i32;
    for dy in -radius..=radius {
        for dx in -radius..=radius {
            if dx * dx + dy * dy <= radius * radius {
                let x = cx + dx;
                let y = cy + dy;
                if x >= 0 && y >= 0 && (x as usize) < res && (y as usize) < res {
                    buf[y as usize * res + x as usize] = TERRAIN_REGION_SENTINEL;
                }
            }
        }
    }
}

/// タイルマップ・WorldMasks・ポリライン点列群から terrain_region_map バッファ（RES×RES, u8）を生成。
///
/// アルゴリズム:
/// 1. バッファを UNASSIGNED(253) で初期化
/// 2. 全ポリライン点列を sentinel=254 で「バリア壁」として描画
/// 3. 非 junction 開端点に radius=3px の sentinel blob を描画（ギャップ閉鎖）
/// 4. タイル中心ピクセルをシード（タイル種別バイト）として書き込む
///    （sentinel と衝突する場合はスキップ — テクスチャ端や blob 境界上のレアケース）
/// 5. 多点源 BFS で UNASSIGNED ピクセルを塗りつぶす（sentinel でブロック）
/// 6. 残存 sentinel を最短 dilation (ダブルバッファ、最大 5 パス) で上書き
pub(crate) fn rasterize_terrain_regions(
    terrain_tiles: &[TerrainType],
    masks: &WorldMasks,
    sampled_polylines: &[Vec<Vec2>],
    endpoint_blobs: &[Vec2],
) -> Vec<u8> {
    let res = TERRAIN_REGION_RES;

    // Step 1: 全ピクセルを UNASSIGNED で初期化
    let mut buf = vec![TERRAIN_REGION_UNASSIGNED; res * res];

    // Step 2: ポリライン点列を sentinel で壁として描画
    for polyline in sampled_polylines {
        for pair in polyline.windows(2) {
            let p0 = world_to_region_pixel(pair[0]);
            let p1 = world_to_region_pixel(pair[1]);
            rasterize_segment_barrier(&mut buf, res, p0, p1);
        }
    }

    // Step 2.5: 非 junction 開端点に sentinel blob を描画（最大ギャップ ≈ 4px を封鎖）
    for &ep in endpoint_blobs {
        let center = world_to_region_pixel(ep);
        rasterize_blob(&mut buf, res, center, 3);
    }

    // Step 3: タイル中心をシード（BFS の多点源）として書き込む
    let w = MAP_WIDTH as usize;
    let h = MAP_HEIGHT as usize;
    let mut queue: VecDeque<(usize, usize)> = VecDeque::new();
    for ty in 0..h {
        for tx in 0..w {
            let id_byte =
                terrain_region_byte(terrain_tiles[ty * w + tx], masks, (tx as i32, ty as i32));
            let world_p = hw_world::grid_to_world(tx as i32, ty as i32);
            let (fpx, fpy) = world_to_region_pixel(world_p);
            let px = (fpx as usize).min(res - 1);
            let py = (fpy as usize).min(res - 1);
            // sentinel と衝突したら近傍 1px 範囲で非 sentinel を探す
            if buf[py * res + px] == TERRAIN_REGION_SENTINEL {
                let mut found = false;
                'outer: for dy in -1i32..=1 {
                    for dx in -1i32..=1 {
                        let nx = px as i32 + dx;
                        let ny = py as i32 + dy;
                        if nx >= 0 && ny >= 0 && (nx as usize) < res && (ny as usize) < res {
                            let idx = ny as usize * res + nx as usize;
                            if buf[idx] == TERRAIN_REGION_UNASSIGNED {
                                buf[idx] = id_byte;
                                queue.push_back((nx as usize, ny as usize));
                                found = true;
                                break 'outer;
                            }
                        }
                    }
                }
                if !found {
                    // sentinel blob の内部に完全に埋まっている場合 — スキップ
                    // (blob 境界上タイルのみ起こり得るレアケース; 近傍 BFS で後続補填される)
                }
            } else {
                buf[py * res + px] = id_byte;
                queue.push_back((px, py));
            }
        }
    }

    // Step 4: 多点源 BFS で UNASSIGNED ピクセルを塗りつぶす（sentinel でブロック）
    let neighbors: [(i32, i32); 4] = [(-1, 0), (1, 0), (0, -1), (0, 1)];
    while let Some((px, py)) = queue.pop_front() {
        let cur_val = buf[py * res + px];
        for (dx, dy) in neighbors {
            let nx = px as i32 + dx;
            let ny = py as i32 + dy;
            if nx < 0 || ny < 0 || nx as usize >= res || ny as usize >= res {
                continue;
            }
            let nidx = ny as usize * res + nx as usize;
            if buf[nidx] == TERRAIN_REGION_UNASSIGNED {
                buf[nidx] = cur_val;
                queue.push_back((nx as usize, ny as usize));
            }
        }
    }

    // Step 4.5: BFS 後に残存する UNASSIGNED ピクセルを SENTINEL に昇格させる。
    // sentinel 壁の交差部（blob 内部など）に閉じ込められた孤立 UNASSIGNED ピクセルが
    // BFS で到達できなかった場合にのみ発生する。これらは壁の一部として Step 5 が処理する。
    for v in buf.iter_mut() {
        if *v == TERRAIN_REGION_UNASSIGNED {
            *v = TERRAIN_REGION_SENTINEL;
        }
    }

    // Step 5: 残存 sentinel をダブルバッファ dilation で上書き
    // blob 半径=3px の場合、最大 3 パスで収束する。余裕を持って 8 パスとする。
    let mut tmp = buf.clone();
    for _ in 0..8 {
        let mut changed = false;
        for py in 0..res {
            for px in 0..res {
                if buf[py * res + px] != TERRAIN_REGION_SENTINEL {
                    continue;
                }
                for (dx, dy) in neighbors {
                    let qx = px as i32 + dx;
                    let qy = py as i32 + dy;
                    if qx < 0 || qy < 0 || qx as usize >= res || qy as usize >= res {
                        continue;
                    }
                    let neighbor = buf[qy as usize * res + qx as usize];
                    if neighbor != TERRAIN_REGION_SENTINEL {
                        tmp[py * res + px] = neighbor;
                        changed = true;
                        break;
                    }
                }
            }
        }
        buf.copy_from_slice(&tmp);
        if !changed {
            break;
        }
    }

    debug_assert!(
        !buf.contains(&TERRAIN_REGION_SENTINEL),
        "terrain_region_map: sentinel pixels remain after dilation"
    );

    buf
}

pub(crate) fn bake_boundary_proximity_mask(buf: &[u8], res: usize) -> Vec<u8> {
    let mut edge = vec![0u8; res * res];
    for y in 0..res {
        for x in 0..res {
            let idx = y * res + x;
            let center = buf[idx];
            let mut is_boundary = false;
            let y0 = y.saturating_sub(1);
            let y1 = (y + 1).min(res - 1);
            let x0 = x.saturating_sub(1);
            let x1 = (x + 1).min(res - 1);
            'outer: for ny in y0..=y1 {
                for nx in x0..=x1 {
                    if buf[ny * res + nx] != center {
                        is_boundary = true;
                        break 'outer;
                    }
                }
            }
            if is_boundary {
                edge[idx] = 255;
            }
        }
    }

    let mut dilated = vec![0u8; res * res];
    for y in 0..res {
        for x in 0..res {
            let idx = y * res + x;
            if edge[idx] == 0 {
                continue;
            }
            let y0 = y.saturating_sub(BOUNDARY_PROXIMITY_DILATION_PX as usize);
            let y1 = (y + BOUNDARY_PROXIMITY_DILATION_PX as usize).min(res - 1);
            let x0 = x.saturating_sub(BOUNDARY_PROXIMITY_DILATION_PX as usize);
            let x1 = (x + BOUNDARY_PROXIMITY_DILATION_PX as usize).min(res - 1);
            for ny in y0..=y1 {
                for nx in x0..=x1 {
                    dilated[ny * res + nx] = 255;
                }
            }
        }
    }
    dilated
}

pub(crate) fn downsample_boundary_proximity_mask(src: &[u8], src_res: usize, dst_res: usize) -> Vec<u8> {
    let scale = src_res / dst_res;
    let mut out = vec![0u8; dst_res * dst_res];
    for y in 0..dst_res {
        for x in 0..dst_res {
            let mut max_v = 0u8;
            for oy in 0..scale {
                for ox in 0..scale {
                    let sx = x * scale + ox;
                    let sy = y * scale + oy;
                    max_v = max_v.max(src[sy * src_res + sx]);
                }
            }
            out[y * dst_res + x] = max_v;
        }
    }
    out
}
