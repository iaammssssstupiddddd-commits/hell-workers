//! 地形境界曲線メッシュ生成
//!
//! WFC 生成タイルの境界をグリッドエッジから抽出し、ノイズ変位と Catmull-Rom スプラインで
//! 有機的な曲線境界メッシュを PostStartup 時にスポーンする。
//!
//! **純粋ビジュアル**: ゲームロジック・当たり判定・AI 経路に一切影響しない。
//!
//! 草・土の**亜種**だけが違う隣同士では曲線を出さない（格子状になるため）。**草ゾーン／中立／土ゾーン**（`terrain_zone_bias_byte`）
//! が隣接で変わる境は曲線で出す（両方草／両方土なら亜種が違っても可）。亜種の細かい色調は `terrain_id_map` とシェーダでも表現する。
//!
//! ノイズは `master_seed` と **ポリラインごとの幾何**（種別・端コーナー・弧長など）から
//! 決定論的に導出する。同じ地形でも境界線が複数あれば互いに別の波形になる。
//! **三叉路**（全境界グラフで次数 ≥ 3 のコーナー）では法線変位を 0 にし、種別の異なる帯が同一点で食い違わないようにする。

use std::collections::{HashMap, HashSet, VecDeque};

use bevy::image::{ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use hw_core::constants::{MAP_HEIGHT, MAP_WIDTH, TILE_SIZE};
use hw_visual::TerrainSurfaceMaterial;
use hw_world::{TerrainType, WorldMasks, grid_to_world};

use crate::plugins::startup::Terrain3dHandles;
use crate::world::map::spawn::GeneratedWorldLayoutResource;

// ── パラメータ定数 ──────────────────────────────────────────────────────────

/// ノイズの空間周波数（弧長ワールド単位に対する周波数）。
/// 約 3 タイル分（96 ワールド単位）で 1 周期。
const NOISE_FREQ: f32 = 1.0 / (TILE_SIZE * 3.0);

/// ノイズの最大変位量（ワールド単位）。
/// 隣セル中心 TILE_SIZE/2 = 16.0 未満に抑え、論理境界と視覚の乖離を防ぐ。
const NOISE_AMPLITUDE: f32 = TILE_SIZE * 0.375; // 12.0

/// Catmull-Rom スプライン 1 セグメントあたりのサンプル数。
const CATMULL_ROM_STEPS: u32 = 8;

/// 面取り（Chamfer）ベベル距離（ワールド単位）。
/// 川岸 1 タイル段差（32wu）の 35% を面取りし、Catmull-Rom のオーバーシュートを抑制する。
const CHAMFER_DISTANCE: f32 = TILE_SIZE * 0.35; // ≈ 11.2wu

/// 面取りを適用するコーナー角のコサイン閾値。
/// cos(60°) = 0.5: それより鋭い角（0°〜60°未満）のコーナーのみ面取りする。
/// 川岸の 90° ステップ（cos = 0）はこの閾値に確実に掛かる。
const CHAMFER_COS_THRESHOLD: f32 = 0.5;

/// terrain_region_map テクスチャの解像度（1 辺のピクセル数）。
/// MAP_WIDTH=100 に対して 10.24 px/tile（1024 にすると 5.12 の倍精細でジャギーが減る）。
const TERRAIN_REGION_RES: usize = 1024;

/// terrain_region_map のセンチネル値。River(255) と区別するため 254 を使う。
/// BFS flood fill のバリア壁として機能し、最終的に短 dilation で隣接値に置き換えられる。
const TERRAIN_REGION_SENTINEL: u8 = 254;

/// terrain_region_map の未割当値。BFS で塗りつぶされる前の初期値。
/// 11 値エンコーディング (0,1,2,85,86,87,170,171,172,255) および SENTINEL(254) と衝突しない。
const TERRAIN_REGION_UNASSIGNED: u8 = 253;

// ── 境界種別 ─────────────────────────────────────────────────────────────────

/// 隣接する 2 種類の TerrainType ペア（無向）を表す列挙型。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BoundaryKind {
    GrassDirt,
    GrassSand,
    GrassRiver,
    DirtSand,
    DirtRiver,
    SandRiver,
    /// 同じ `TerrainType` の草どうしで、草／中立／土ゾーン（`terrain_zone_bias_byte`）が隣接で変わる境。
    GrassZoneTone,
    /// 同じ `TerrainType` の土どうしで、ゾーンバイアスが隣接で変わる境。
    DirtZoneTone,
    /// 同じ Sand どうしで shore/inland variant が隣接で変わる境。
    SandZoneTone,
}

impl BoundaryKind {
    /// 2 つの TerrainType から BoundaryKind を決定する（順序非依存）。
    /// 完全に同一タイルなら None。
    pub fn from_pair(a: TerrainType, b: TerrainType) -> Option<Self> {
        if a == b {
            return None;
        }
        // priority() でソートして無向ペアを一意に決定（River=0 < Sand=1 < Dirt=2 < Grass=3）
        let (lo, hi) = if a.priority() < b.priority() {
            (a, b)
        } else {
            (b, a)
        };
        match (lo, hi) {
            (TerrainType::River, TerrainType::Sand) => Some(Self::SandRiver),
            (TerrainType::River, TerrainType::Dirt) => Some(Self::DirtRiver),
            (TerrainType::River, TerrainType::Grass) => Some(Self::GrassRiver),
            (TerrainType::Sand, TerrainType::Dirt) => Some(Self::DirtSand),
            (TerrainType::Sand, TerrainType::Grass) => Some(Self::GrassSand),
            (TerrainType::Dirt, TerrainType::Grass) => Some(Self::GrassDirt),
            _ => None,
        }
    }

    /// この境界種別のインデックス（per-kind seed 生成用）。
    pub fn index(self) -> u32 {
        self as u32
    }

}

// ── データ型 ──────────────────────────────────────────────────────────────────

/// 隣接する 2 セル間の境界エッジ（ワールド座標）。
#[derive(Debug, Clone)]
pub struct BoundaryEdge {
    pub a: Vec2,
    pub b: Vec2,
    pub kind: BoundaryKind,
}

/// 連続した境界ポリライン。開チェーンと閉ループの両方を表現する。
#[derive(Debug, Clone)]
pub struct BoundaryPolyline {
    pub points: Vec<Vec2>,
    /// 累積弧長テーブル（points と同じ長さ、先頭は 0.0）。
    pub arc_lengths: Vec<f32>,
    pub is_closed: bool,
    pub kind: BoundaryKind,
}

/// 境界リボンが影響するグリッドセルのインデックス。
///
/// PostStartup で build し、将来の TerrainChangedEvent 対応の基盤として使用する。
#[derive(Resource, Default)]
pub struct BoundarySliceSpatialIndex;

// ── M1: エッジ抽出と連結 ──────────────────────────────────────────────────────

#[inline]
fn zone_tone_boundary_kind(terrain: TerrainType, bias_a: u8, bias_b: u8) -> Option<BoundaryKind> {
    if bias_a == bias_b {
        return None;
    }
    match terrain {
        TerrainType::Grass => Some(BoundaryKind::GrassZoneTone),
        TerrainType::Dirt => Some(BoundaryKind::DirtZoneTone),
        _ => None,
    }
}

/// 粗い種別が両方草または両方土のときだけゾーン境界（亜種は一致不要。ゾーン境で亜種が違うことが多い）。
#[inline]
fn maybe_zone_tone_edge(t0: TerrainType, t1: TerrainType, bias_a: u8, bias_b: u8) -> Option<BoundaryKind> {
    let both_grass = matches!((t0, t1), (TerrainType::Grass, TerrainType::Grass));
    let both_dirt = matches!((t0, t1), (TerrainType::Dirt, TerrainType::Dirt));
    if !both_grass && !both_dirt {
        return None;
    }
    zone_tone_boundary_kind(t0, bias_a, bias_b)
}

/// グリッド座標のゾーンバイアスバイトを返す（grass zone=0, neutral=128, dirt zone=255）。
#[inline]
fn terrain_zone_bias_byte(masks: &WorldMasks, pos: (i32, i32)) -> u8 {
    if masks.grass_zone_mask.get(pos) {
        0
    } else if masks.dirt_zone_mask.get(pos) {
        255
    } else {
        128
    }
}

/// Sand タイルの shore/inland variant バイトを返す（shore=171, inland=172, regular=170）。
#[inline]
fn terrain_sand_variant_byte(masks: &WorldMasks, pos: (i32, i32)) -> u8 {
    let is_final = masks.final_sand_mask.get(pos);
    let is_inland = masks.inland_sand_mask.get(pos);
    if is_final && !is_inland {
        171 // shore
    } else if is_inland {
        172 // inland
    } else {
        170 // regular
    }
}

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
fn rasterize_terrain_regions(
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
            let id_byte = terrain_region_byte(terrain_tiles[ty * w + tx], masks, (tx as i32, ty as i32));
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

/// terrain_tiles（row-major: y*MAP_WIDTH+x）と `WorldMasks` から全境界エッジを抽出する。
///
/// - **粗いカテゴリ**が変わる境（`BoundaryKind::from_pair`）
/// - **草↔草／土↔土**（亜種は問わない）で `terrain_zone_bias_byte`（草ゾーン／中立／土ゾーン）が隣接で変わる境
pub fn extract_boundary_edges(terrain_tiles: &[TerrainType], masks: &WorldMasks) -> Vec<BoundaryEdge> {
    let w = MAP_WIDTH as usize;
    let h = MAP_HEIGHT as usize;
    let half = TILE_SIZE / 2.0;
    let mut edges = Vec::new();

    // 水平エッジ: セル (x, y) と (x, y+1) の境界
    for y in 0..h - 1 {
        for x in 0..w {
            let t0 = terrain_tiles[y * w + x];
            let t1 = terrain_tiles[(y + 1) * w + x];
            let gx = x as i32;
            let gy = y as i32;
            if let Some(kind) = BoundaryKind::from_pair(t0, t1) {
                let center = grid_to_world(gx, gy);
                edges.push(BoundaryEdge {
                    a: Vec2::new(center.x - half, center.y + half),
                    b: Vec2::new(center.x + half, center.y + half),
                    kind,
                });
            } else if let Some(kind) = maybe_zone_tone_edge(
                t0,
                t1,
                terrain_zone_bias_byte(masks, (gx, gy)),
                terrain_zone_bias_byte(masks, (gx, gy + 1)),
            ) {
                let center = grid_to_world(gx, gy);
                edges.push(BoundaryEdge {
                    a: Vec2::new(center.x - half, center.y + half),
                    b: Vec2::new(center.x + half, center.y + half),
                    kind,
                });
            } else if t0 == TerrainType::Sand
                && t1 == TerrainType::Sand
                && terrain_sand_variant_byte(masks, (gx, gy))
                    != terrain_sand_variant_byte(masks, (gx, gy + 1))
            {
                let center = grid_to_world(gx, gy);
                edges.push(BoundaryEdge {
                    a: Vec2::new(center.x - half, center.y + half),
                    b: Vec2::new(center.x + half, center.y + half),
                    kind: BoundaryKind::SandZoneTone,
                });
            }
        }
    }

    // 垂直エッジ: セル (x, y) と (x+1, y) の境界
    for y in 0..h {
        for x in 0..w - 1 {
            let t0 = terrain_tiles[y * w + x];
            let t1 = terrain_tiles[y * w + x + 1];
            let gx = x as i32;
            let gy = y as i32;
            if let Some(kind) = BoundaryKind::from_pair(t0, t1) {
                let center = grid_to_world(gx, gy);
                edges.push(BoundaryEdge {
                    a: Vec2::new(center.x + half, center.y - half),
                    b: Vec2::new(center.x + half, center.y + half),
                    kind,
                });
            } else if let Some(kind) = maybe_zone_tone_edge(
                t0,
                t1,
                terrain_zone_bias_byte(masks, (gx, gy)),
                terrain_zone_bias_byte(masks, (gx + 1, gy)),
            ) {
                let center = grid_to_world(gx, gy);
                edges.push(BoundaryEdge {
                    a: Vec2::new(center.x + half, center.y - half),
                    b: Vec2::new(center.x + half, center.y + half),
                    kind,
                });
            } else if t0 == TerrainType::Sand
                && t1 == TerrainType::Sand
                && terrain_sand_variant_byte(masks, (gx, gy))
                    != terrain_sand_variant_byte(masks, (gx + 1, gy))
            {
                let center = grid_to_world(gx, gy);
                edges.push(BoundaryEdge {
                    a: Vec2::new(center.x + half, center.y - half),
                    b: Vec2::new(center.x + half, center.y + half),
                    kind: BoundaryKind::SandZoneTone,
                });
            }
        }
    }

    edges
}

/// 抽出済み境界エッジ全体で、**3 本以上**の辺が接するグリッドコーナー（多地形の三叉路など）。
///
/// 各ポリラインが別シードの法線ノイズを受けると、同一点が幾何的にずれて継ぎ目が空く。
/// これらのコーナーでは変位を 0 にし、全種別で同一座標に固定する。
fn boundary_junction_corner_keys(edges: &[BoundaryEdge]) -> HashSet<(i32, i32)> {
    let mut deg: HashMap<(i32, i32), u32> = HashMap::new();
    for e in edges {
        *deg.entry(world_to_corner_key(e.a)).or_insert(0) += 1;
        *deg.entry(world_to_corner_key(e.b)).or_insert(0) += 1;
    }
    deg.into_iter()
        .filter(|&(_, c)| c >= 3)
        .map(|(k, _)| k)
        .collect()
}

/// ワールド座標 Vec2 をグリッドコーナーインデックス (i32, i32) に変換する。
///
/// すべての境界エッジ端点は TILE_SIZE の倍数のグリッドコーナーに位置するため、
/// round() で一意な整数キーが得られる（浮動小数点等値比較を回避）。
fn world_to_corner_key(p: Vec2) -> (i32, i32) {
    let cx = (p.x / TILE_SIZE + MAP_WIDTH as f32 / 2.0).round() as i32;
    let cy = (p.y / TILE_SIZE + MAP_HEIGHT as f32 / 2.0).round() as i32;
    (cx, cy)
}

/// BoundaryEdge のリストを連続ポリライン群（開チェーンと閉ループ）に変換する。
pub fn chain_edges_to_polylines(edges: Vec<BoundaryEdge>) -> Vec<BoundaryPolyline> {
    // 種別ごとにエッジをグループ化
    let mut by_kind: HashMap<BoundaryKind, Vec<BoundaryEdge>> = HashMap::new();
    for e in edges {
        by_kind.entry(e.kind).or_default().push(e);
    }

    let mut result = Vec::new();
    for (kind, kind_edges) in by_kind {
        let n = kind_edges.len();
        let corner_keys: Vec<[(i32, i32); 2]> = kind_edges
            .iter()
            .map(|e| [world_to_corner_key(e.a), world_to_corner_key(e.b)])
            .collect();

        // コーナー → [エッジインデックス] の隣接マップ
        let mut adj: HashMap<(i32, i32), Vec<usize>> = HashMap::new();
        for (i, keys) in corner_keys.iter().enumerate() {
            adj.entry(keys[0]).or_default().push(i);
            adj.entry(keys[1]).or_default().push(i);
        }

        let mut visited = vec![false; n];

        // degree-1 コーナー（開チェーンの端点）から処理
        let chain_starts: Vec<(i32, i32)> = adj
            .iter()
            .filter(|(_, es)| es.len() == 1)
            .map(|(k, _)| *k)
            .collect();

        for start_key in chain_starts {
            let first = match adj[&start_key].iter().find(|&&i| !visited[i]) {
                Some(&i) => i,
                None => continue,
            };
            let (points, _first_forward) = follow_chain(
                start_key,
                first,
                &kind_edges,
                &corner_keys,
                &adj,
                &mut visited,
            );
            if points.len() >= 2 {
                let arc_lengths = parameterize_arc_length(&points);
                result.push(BoundaryPolyline {
                    points,
                    arc_lengths,
                    is_closed: false,
                    kind,
                });
            }
        }

        // 残る未訪問エッジ → 閉ループ
        for start_idx in 0..n {
            if visited[start_idx] {
                continue;
            }
            let start_key = corner_keys[start_idx][0];
            let (mut points, _first_forward) = follow_chain(
                start_key,
                start_idx,
                &kind_edges,
                &corner_keys,
                &adj,
                &mut visited,
            );
            trim_closed_polyline_duplicate_end(&mut points);
            // 閉じた単純ループは少なくとも 3 頂点（重複除去後）。
            if points.len() >= 3 {
                let arc_lengths = parameterize_arc_length(&points);
                result.push(BoundaryPolyline {
                    points,
                    arc_lengths,
                    is_closed: true,
                    kind,
                });
            }
        }
    }

    result
}

/// 閉ループ走査では始点コーナーが **先頭と末尾の両方** に入る（`follow_chain` が一周して戻るため）。
/// `p[0] == p[n-1]` のままだと、メッシュ側のセグメント `p[n-1] → p[0]` が長さ 0 になり
/// 閉じるクワッドが落ち、継ぎ目だけ鋭角に見える。末尾の重複を除いて真の環状点列にする。
fn trim_closed_polyline_duplicate_end(points: &mut Vec<Vec2>) {
    if points.len() < 2 {
        return;
    }
    let last = points.len() - 1;
    if points[0].distance_squared(points[last]) < 1e-10 {
        points.pop();
    }
}

/// 指定コーナーから始まる連続チェーンを辿り、点列と「最初のエッジを順方向（a→b）で辿ったか」を返す。
fn follow_chain(
    start_key: (i32, i32),
    first_edge_idx: usize,
    edges: &[BoundaryEdge],
    corner_keys: &[[(i32, i32); 2]],
    adj: &HashMap<(i32, i32), Vec<usize>>,
    visited: &mut [bool],
) -> (Vec<Vec2>, bool) {
    let mut points = Vec::new();
    let mut cur_key = start_key;
    let mut cur_edge_idx = first_edge_idx;
    let mut first_forward = true;

    loop {
        visited[cur_edge_idx] = true;
        let [ka, kb] = corner_keys[cur_edge_idx];
        let edge = &edges[cur_edge_idx];

        if points.is_empty() {
            if ka == cur_key {
                first_forward = true;
                points.push(edge.a);
                points.push(edge.b);
                cur_key = kb;
            } else {
                first_forward = false;
                points.push(edge.b);
                points.push(edge.a);
                cur_key = ka;
            }
        } else if ka == cur_key {
            points.push(edge.b);
            cur_key = kb;
        } else {
            points.push(edge.a);
            cur_key = ka;
        }

        match adj
            .get(&cur_key)
            .and_then(|es| es.iter().find(|&&i| !visited[i]))
            .copied()
        {
            Some(next_idx) => cur_edge_idx = next_idx,
            None => break,
        }
    }

    (points, first_forward)
}

/// 点列の累積弧長テーブルを構築する（先頭は 0.0、points と同じ長さ）。
pub fn parameterize_arc_length(points: &[Vec2]) -> Vec<f32> {
    let mut arc = vec![0.0f32; points.len()];
    for i in 1..points.len() {
        arc[i] = arc[i - 1] + points[i].distance(points[i - 1]);
    }
    arc
}

// ── M2: ノイズ変位と Catmull-Rom ─────────────────────────────────────────────

/// ハッシュベースの 1D 値ノイズ（[-1.0, 1.0]）。
fn value_noise_1d(t: f32, seed: u32) -> f32 {
    let i = t.floor() as i32;
    let f = t.fract();
    let f = f * f * (3.0 - 2.0 * f); // smoothstep
    let v0 = hash_to_f32(i, seed);
    let v1 = hash_to_f32(i + 1, seed);
    v0 + (v1 - v0) * f
}

fn hash_to_f32(i: i32, seed: u32) -> f32 {
    let h = (i as u32).wrapping_mul(2_654_435_761).wrapping_add(seed);
    let h = h ^ (h >> 16);
    let h = h.wrapping_mul(0x45d9f3b);
    let h = h ^ (h >> 16);
    (h as f32 / u32::MAX as f32) * 2.0 - 1.0
}

#[inline]
fn mix64(z: u64) -> u64 {
    let mut x = z.wrapping_add(0x9e37_79b9_7f4a_7c15);
    x = (x ^ (x >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    x ^ (x >> 31)
}

#[inline]
fn u64_to_unit_f32(x: u64) -> f32 {
    (((x >> 8) & 0xFF_FFFF) as f32) * (1.0 / 16_777_216.0)
}

/// ポリラインごとのノイズパラメータ（`master_seed` と幾何から決定論的に導出）。
///
/// 種別だけ XOR していた頃と違い、**同じ `BoundaryKind` の複数線**でも別シード・別位相になる。
#[derive(Clone, Copy, Debug)]
pub struct PolylineNoiseParams {
    /// `value_noise_1d` のシード。
    seed: u32,
    /// 弧長座標への位相オフセット（同じ全長でも波形をずらす）。
    arc_phase: f32,
    /// 基準周波数に掛る倍率。
    freq_scale: f32,
}

pub fn boundary_polyline_noise_params(master_seed: u64, polyline: &BoundaryPolyline) -> PolylineNoiseParams {
    let mut h = mix64(master_seed);
    h ^= mix64(polyline.kind.index() as u64);
    h ^= mix64(polyline.points.len() as u64);
    h ^= mix64(if polyline.is_closed {
        0xC001_D00D_C0DE_u64
    } else {
        0x5EED_FACE_u64
    });

    if let Some(p0) = polyline.points.first() {
        let k = world_to_corner_key(*p0);
        h ^= mix64((k.0 as u64).wrapping_shl(32) ^ (k.1 as u32 as u64));
    }
    if polyline.points.len() > 1 && let Some(pl) = polyline.points.last() {
        let k = world_to_corner_key(*pl);
        h ^= mix64((k.0 as u64).wrapping_shl(16) ^ (k.1 as u32 as u64).wrapping_shl(48));
    }
    if let Some(pm) = polyline.points.get(polyline.points.len() / 2) {
        let k = world_to_corner_key(*pm);
        h ^= mix64(k.0 as u64 ^ (k.1 as u64).wrapping_shl(32));
    }
    if let Some(total) = polyline.arc_lengths.last() {
        let q = (*total * 1000.0) as u64;
        h ^= mix64(q);
    }

    let h1 = mix64(h);
    let h2 = mix64(h.wrapping_add(0x9e37_79b9_7f4a_7c15));
    let h3 = mix64(h.wrapping_add(0xc6bc_2796_92b5_c323));

    PolylineNoiseParams {
        seed: (h1 ^ (h1 >> 32)) as u32,
        arc_phase: u64_to_unit_f32(h2) * 800.0,
        freq_scale: 0.82 + u64_to_unit_f32(h3) * 0.36,
    }
}

/// ポリラインの各制御点を法線方向にノイズ変位した点列を返す。
///
/// `junctions` に含まれるコーナー（全境界グラフで次数 ≥ 3）は変位 0 とし、三叉路で帯が割れないようにする。
pub fn displace_polyline(
    polyline: &BoundaryPolyline,
    noise: &PolylineNoiseParams,
    base_freq: f32,
    amplitude: f32,
    junctions: &HashSet<(i32, i32)>,
) -> Vec<Vec2> {
    let freq = base_freq * noise.freq_scale;
    let points = &polyline.points;
    let arcs = &polyline.arc_lengths;
    let n = points.len();
    let mut result = Vec::with_capacity(n);

    for i in 0..n {
        let key = world_to_corner_key(points[i]);
        let tangent = compute_tangent(points, i, polyline.is_closed);
        let normal = Vec2::new(-tangent.y, tangent.x);
        let displacement = if junctions.contains(&key) {
            0.0
        } else {
            let t = arcs[i] * freq + noise.arc_phase;
            value_noise_1d(t, noise.seed) * amplitude
        };
        result.push(points[i] + normal * displacement);
    }

    result
}

/// ノイズ変位済み点列の鋭角コーナーを面取り（Chamfer）し、
/// Catmull-Rom スプラインのオーバーシュートを抑制した新しい点列を返す。
///
/// 各コーナーを 2 つのベベル点で置換する：
/// - `bevel1 = p - t * d_in`  （コーナー手前）
/// - `bevel2 = p + t * d_out` （コーナー直後）
///
/// 以下の頂点は変更しない：
/// - 開ポリラインの端点
/// - `junctions` に含まれるコーナー（三叉路点: 変位 0 で元座標にある）
/// - 内角が `cos_threshold` 以上の緩やかな曲がり（面取り不要）
fn chamfer_polyline_points(
    points: &[Vec2],
    is_closed: bool,
    junctions: &HashSet<(i32, i32)>,
    t: f32,
    cos_threshold: f32,
) -> Vec<Vec2> {
    let n = points.len();
    if n < 3 {
        return points.to_vec();
    }

    let mut result = Vec::with_capacity(n + n / 3);

    for i in 0..n {
        let p = points[i];

        // 開ポリラインの端点は変更しない
        if !is_closed && (i == 0 || i == n - 1) {
            result.push(p);
            continue;
        }

        // ジャンクション頂点は変更しない（displace_polyline で変位=0 なので元の grid 座標にある）
        if junctions.contains(&world_to_corner_key(p)) {
            result.push(p);
            continue;
        }

        let prev_i = if i == 0 { n - 1 } else { i - 1 };
        let next_i = if i == n - 1 { 0 } else { i + 1 };

        let d_in = (p - points[prev_i]).normalize_or_zero();
        let d_out = (points[next_i] - p).normalize_or_zero();

        // 内角コサインが cos_threshold より小さい（より鋭い）コーナーのみ面取り
        if d_in.dot(d_out) < cos_threshold {
            result.push(p - t * d_in); // コーナー手前
            result.push(p + t * d_out); // コーナー直後
        } else {
            result.push(p);
        }
    }

    result
}

/// 点列の i 番目における接線方向を返す（中央差分、端点は前後向き差分）。
fn compute_tangent(points: &[Vec2], i: usize, is_closed: bool) -> Vec2 {
    let n = points.len();
    if n < 2 {
        return Vec2::X;
    }
    if i == 0 {
        if is_closed {
            (points[1] - points[n - 1]).normalize_or_zero()
        } else {
            (points[1] - points[0]).normalize_or_zero()
        }
    } else if i == n - 1 {
        if is_closed {
            // 中央差分の wrap: 次は points[0]（閉曲線の先頭へ戻る）
            (points[0] - points[n - 2]).normalize_or_zero()
        } else {
            (points[n - 1] - points[n - 2]).normalize_or_zero()
        }
    } else {
        (points[i + 1] - points[i - 1]).normalize_or_zero()
    }
}

/// Catmull-Rom スプライン補間で密な点列を生成する。
///
/// - 開チェーン: 両端に外挿ゴースト点を付加して全セグメントを補間する
/// - 閉ループ: 末尾/先頭の点を折り返してゴーストとし、接続を滑らかにする
pub fn sample_catmull_rom(
    points: &[Vec2],
    is_closed: bool,
    steps_per_segment: u32,
) -> Vec<Vec2> {
    let n = points.len();
    if n < 2 || steps_per_segment == 0 {
        return points.to_vec();
    }

    // ゴースト点を含む拡張制御点列を構築（長さ = n + 3）
    let extended: Vec<Vec2> = if is_closed {
        // 先頭に p_{n-1}、末尾に p0, p1 を追加
        let mut v = Vec::with_capacity(n + 3);
        v.push(points[n - 1]);
        v.extend_from_slice(points);
        v.push(points[0]);
        v.push(points[1]);
        v
    } else {
        // 先頭に外挿ゴースト、末尾に外挿ゴーストを追加
        let ghost_start = 2.0 * points[0] - points[1];
        let ghost_end = 2.0 * points[n - 1] - points[n - 2];
        let mut v = Vec::with_capacity(n + 2);
        v.push(ghost_start);
        v.extend_from_slice(points);
        v.push(ghost_end);
        v
    };

    let num_segments = if is_closed { n } else { n - 1 };
    let mut result = Vec::with_capacity(num_segments * steps_per_segment as usize + 1);

    for seg in 0..num_segments {
        let (p0, p1, p2, p3) = (
            extended[seg],
            extended[seg + 1],
            extended[seg + 2],
            extended[seg + 3],
        );
        for step in 0..steps_per_segment {
            let t = step as f32 / steps_per_segment as f32;
            result.push(catmull_rom_point(p0, p1, p2, p3, t));
        }
    }

    // 開チェーン: 末尾制御点を追加
    // 閉ループ: 先頭サンプル点を複製して完全に閉じる
    if is_closed {
        if let Some(&first) = result.first() {
            result.push(first);
        }
    } else {
        result.push(*points.last().expect("points is non-empty"));
    }

    result
}

fn catmull_rom_point(p0: Vec2, p1: Vec2, p2: Vec2, p3: Vec2, t: f32) -> Vec2 {
    let t2 = t * t;
    let t3 = t2 * t;
    0.5 * ((2.0 * p1)
        + (-p0 + p2) * t
        + (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * t2
        + (-p0 + 3.0 * p1 - 3.0 * p2 + p3) * t3)
}

// ── メッシュスポーン ────────────────────────────────────────────────────────────

/// PostStartup で呼ばれる境界ラスタライズシステム。
///
/// GeneratedWorldLayoutResource から地形タイルを読み取り、
/// CPU で terrain_region_map テクスチャをベイクして TerrainSurfaceMaterial に設定する。
pub fn spawn_boundary_meshes(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    layout: Res<GeneratedWorldLayoutResource>,
    terrain_handles: Res<Terrain3dHandles>,
    mut terrain_surface_materials: ResMut<Assets<TerrainSurfaceMaterial>>,
) {
    let terrain_tiles = &layout.layout.terrain_tiles;
    let master_seed = layout.master_seed;

    let edges = extract_boundary_edges(terrain_tiles, &layout.layout.masks);
    let junctions = boundary_junction_corner_keys(&edges);
    let polylines = chain_edges_to_polylines(edges);
    let count = polylines.len();

    let mut sampled_polylines: Vec<Vec<Vec2>> = Vec::new();
    let mut endpoint_blobs: Vec<Vec2> = Vec::new();

    for polyline in polylines {
        let noise = boundary_polyline_noise_params(master_seed, &polyline);
        let displaced = displace_polyline(
            &polyline,
            &noise,
            NOISE_FREQ,
            NOISE_AMPLITUDE,
            &junctions,
        );
        let chamfered = chamfer_polyline_points(
            &displaced,
            polyline.is_closed,
            &junctions,
            CHAMFER_DISTANCE,
            CHAMFER_COS_THRESHOLD,
        );
        let sampled = sample_catmull_rom(&chamfered, polyline.is_closed, CATMULL_ROM_STEPS);
        if sampled.len() < 2 {
            continue;
        }

        // 非 junction 開端点を endpoint_blobs に追加（ギャップ閉鎖用）
        if !polyline.is_closed {
            if !polyline.points.is_empty()
                && !junctions.contains(&world_to_corner_key(polyline.points[0]))
            {
                endpoint_blobs.push(sampled[0]);
            }
            if polyline.points.len() > 1
                && !junctions.contains(&world_to_corner_key(*polyline.points.last().unwrap()))
            {
                endpoint_blobs.push(*sampled.last().unwrap());
            }
        }

        sampled_polylines.push(sampled);
    }

    let buf = rasterize_terrain_regions(
        terrain_tiles,
        &layout.layout.masks,
        &sampled_polylines,
        &endpoint_blobs,
    );

    let mut image = Image::new(
        Extent3d {
            width: TERRAIN_REGION_RES as u32,
            height: TERRAIN_REGION_RES as u32,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        buf,
        TextureFormat::R8Unorm,
        default(),
    );
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::ClampToEdge,
        address_mode_v: ImageAddressMode::ClampToEdge,
        mag_filter: ImageFilterMode::Nearest,
        min_filter: ImageFilterMode::Nearest,
        ..default()
    });

    let handle = images.add(image);

    if let Some(mat) = terrain_surface_materials.get_mut(&terrain_handles.surface) {
        mat.extension.boundary_mask = Some(handle);
    }

    commands.insert_resource(BoundarySliceSpatialIndex);

    info!("BEVY_STARTUP: Rasterized terrain_region_map from {} boundary polylines", count);
}

#[cfg(test)]
mod tests {
    use super::BoundaryKind;
    use hw_world::TerrainType;

    #[test]
    fn from_pair_grass_dirt_is_grass_dirt() {
        let g = TerrainType::Grass;
        let d = TerrainType::Dirt;
        assert_eq!(BoundaryKind::from_pair(g, d), Some(BoundaryKind::GrassDirt));
        assert_eq!(BoundaryKind::from_pair(d, g), Some(BoundaryKind::GrassDirt));
    }

    #[test]
    fn from_pair_grass_variants_no_edge() {
        // 同一の Grass どうしは境界なし
        let a = TerrainType::Grass;
        let b = TerrainType::Grass;
        assert_eq!(BoundaryKind::from_pair(a, b), None);
    }

    #[test]
    fn from_pair_identical_grass_none() {
        let a = TerrainType::Grass;
        assert_eq!(BoundaryKind::from_pair(a, a), None);
    }

    #[test]
    fn zone_tone_same_bias_none() {
        let g = TerrainType::Grass;
        assert_eq!(super::zone_tone_boundary_kind(g, 0, 0), None);
    }

    #[test]
    fn zone_tone_grass_zone_vs_neutral() {
        let g = TerrainType::Grass;
        assert_eq!(
            super::zone_tone_boundary_kind(g, 0, 128),
            Some(BoundaryKind::GrassZoneTone)
        );
    }

    #[test]
    fn zone_tone_dirt_zone_vs_neutral() {
        let d = TerrainType::Dirt;
        assert_eq!(
            super::zone_tone_boundary_kind(d, 255, 128),
            Some(BoundaryKind::DirtZoneTone)
        );
    }

    #[test]
    fn maybe_zone_tone_grass_different_variants_still_zone_edge() {
        // flat enum では Grass どうし同士のゾーン境界をテスト
        let a = TerrainType::Grass;
        let b = TerrainType::Grass;
        assert_eq!(
            super::maybe_zone_tone_edge(a, b, 0, 128),
            Some(BoundaryKind::GrassZoneTone)
        );
    }
}
