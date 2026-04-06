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

use std::collections::{HashMap, HashSet};

use bevy::asset::RenderAssetUsages;
use bevy::camera::visibility::RenderLayers;
use bevy::mesh::{Indices, Mesh, PrimitiveTopology};
use bevy::prelude::*;
use hw_core::constants::{LAYER_3D, MAP_HEIGHT, MAP_WIDTH, TILE_SIZE, Y_MAP_BOUNDARY_BASE};
use hw_visual::{BoundarySurfaceMaterialExt, BoundarySurfaceUniform, make_boundary_surface_material};
use hw_world::{TerrainType, WorldMasks, grid_to_world};

use crate::assets::GameAssets;
use crate::world::map::spawn::GeneratedWorldLayoutResource;
use crate::world::map::terrain_metadata::TerrainFeatureMap;

// ── パラメータ定数 ──────────────────────────────────────────────────────────

/// ノイズの空間周波数（弧長ワールド単位に対する周波数）。
/// 約 3 タイル分（96 ワールド単位）で 1 周期。
const NOISE_FREQ: f32 = 1.0 / (TILE_SIZE * 3.0);

/// ノイズの最大変位量（ワールド単位）。
/// 隣セル中心 TILE_SIZE/2 = 16.0 未満に抑え、論理境界と視覚の乖離を防ぐ。
const NOISE_AMPLITUDE: f32 = TILE_SIZE * 0.375; // 12.0

/// Catmull-Rom スプライン 1 セグメントあたりのサンプル数。
const CATMULL_ROM_STEPS: u32 = 8;

/// リボン幅（ワールド単位）。変位曲線を中心に ±NOISE_AMPLITUDE × 2 の半幅を持つ。
/// 完全不透明ゾーン: 中心 ± NOISE_AMPLITUDE（内側 50%）、フェードゾーン: 残り各 25%。
/// これにより、最大変位 (±NOISE_AMPLITUDE = ±12wu) 時でも u=0.25 位置がグリッドエッジと
/// 一致し、完全不透明ゾーン端でギャップなく境界をカバーできる。
const STRIP_WIDTH: f32 = NOISE_AMPLITUDE * 4.0; // 48wu

/// 開チェーン端のラウンドキャップ（半円）の円周分割数。
const ROUND_CAP_SEGMENTS: u32 = 10;

/// 面取り（Chamfer）ベベル距離（ワールド単位）。
/// 川岸 1 タイル段差（32wu）の 35% を面取りし、Catmull-Rom のオーバーシュートを抑制する。
const CHAMFER_DISTANCE: f32 = TILE_SIZE * 0.35; // ≈ 11.2wu

/// 面取りを適用するコーナー角のコサイン閾値。
/// cos(60°) = 0.5: それより鋭い角（0°〜60°未満）のコーナーのみ面取りする。
/// 川岸の 90° ステップ（cos = 0）はこの閾値に確実に掛かる。
const CHAMFER_COS_THRESHOLD: f32 = 0.5;

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
    /// a→b 方向に対する左側（CCW 法線側）の地形種別。
    pub left_terrain: TerrainType,
    /// a→b 方向に対する右側の地形種別。
    pub right_terrain: TerrainType,
}

/// 連続した境界ポリライン。開チェーンと閉ループの両方を表現する。
#[derive(Debug, Clone)]
pub struct BoundaryPolyline {
    pub points: Vec<Vec2>,
    /// 累積弧長テーブル（points と同じ長さ、先頭は 0.0）。
    pub arc_lengths: Vec<f32>,
    pub is_closed: bool,
    pub kind: BoundaryKind,
    /// ポリライン進行方向左側（メッシュで u=0 になる側）の地形種別。
    pub left_terrain: TerrainType,
    /// ポリライン進行方向右側（メッシュで u=1 になる側）の地形種別。
    pub right_terrain: TerrainType,
}

/// 境界リボンメッシュ所有エンティティを示すマーカーコンポーネント。
#[derive(Component)]
pub struct BoundaryMeshMarker;

/// 境界リボンが影響するグリッドセルのインデックス。
///
/// PostStartup で build し、将来の TerrainChangedEvent 対応の基盤として使用する。
/// M4 で `cells: HashMap<(i32, i32), Vec<BoundaryKind>>` フィールドを追加予定。
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
                    left_terrain: t1,   // +Y 側 = upper cell
                    right_terrain: t0,  // -Y 側 = lower cell
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
                    left_terrain: t1,   // 同一種別（ゾーントーン境界）
                    right_terrain: t0,
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
                    left_terrain: t0,   // -X 側 = left cell
                    right_terrain: t1,  // +X 側 = right cell
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
                    left_terrain: t0,   // 同一種別（ゾーントーン境界）
                    right_terrain: t1,
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
            let (points, first_forward) = follow_chain(
                start_key,
                first,
                &kind_edges,
                &corner_keys,
                &adj,
                &mut visited,
            );
            if points.len() >= 2 {
                let arc_lengths = parameterize_arc_length(&points);
                let (left_terrain, right_terrain) = terrain_from_edge_polarity(&kind_edges[first], first_forward);
                result.push(BoundaryPolyline {
                    points,
                    arc_lengths,
                    is_closed: false,
                    kind,
                    left_terrain,
                    right_terrain,
                });
            }
        }

        // 残る未訪問エッジ → 閉ループ
        for start_idx in 0..n {
            if visited[start_idx] {
                continue;
            }
            let start_key = corner_keys[start_idx][0];
            let (mut points, first_forward) = follow_chain(
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
                let (left_terrain, right_terrain) = terrain_from_edge_polarity(&kind_edges[start_idx], first_forward);
                result.push(BoundaryPolyline {
                    points,
                    arc_lengths,
                    is_closed: true,
                    kind,
                    left_terrain,
                    right_terrain,
                });
            }
        }
    }

    result
}

/// `TerrainType` を shader に渡す粗い ID (0=Grass, 1=Dirt, 2=Sand, 3=River) に変換する。
#[inline]
fn terrain_coarse_id(t: TerrainType) -> u8 {
    match t {
        TerrainType::Grass => 0,
        TerrainType::Dirt => 1,
        TerrainType::Sand => 2,
        TerrainType::River => 3,
    }
}

/// エッジの `left_terrain`/`right_terrain` をポリライン走査方向に合わせて返す。
///
/// `first_forward = true`（a→b 方向で辿った）なら edge の left/right をそのまま使う。
/// `first_forward = false`（b→a 方向で辿った）なら左右を反転する。
#[inline]
fn terrain_from_edge_polarity(edge: &BoundaryEdge, first_forward: bool) -> (TerrainType, TerrainType) {
    if first_forward {
        (edge.left_terrain, edge.right_terrain)
    } else {
        (edge.right_terrain, edge.left_terrain)
    }
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

// ── M3: クワッドストリップメッシュと Bevy スポーン ────────────────────────────

fn push_vertex_xz(positions: &mut Vec<[f32; 3]>, normals: &mut Vec<[f32; 3]>, p: Vec2, y_offset: f32) {
    positions.push([p.x, y_offset, -p.y]);
    normals.push([0.0, 1.0, 0.0]);
}

/// [`append_round_cap`] への入力（Clippy `too_many_arguments` 回避用）。
struct RoundCapInput {
    center: Vec2,
    /// 単位ベクトル（`left = center + n_width * half_width` と帯本体で一致させること）。
    n_width: Vec2,
    /// 単位ベクトル。チェーンの外側へ膨らむ向き（始点では `-seg_dir`、終点では `+seg_dir`）。
    bulge_dir: Vec2,
    half_width: f32,
    y_offset: f32,
}

/// 開いたポリラインの **端** を半円で覆う（ストロークの round cap に相当）。
///
/// 弧は `theta in [0, π]` で `center + half_width * (cos(theta)*n_width + sin(theta)*bulge_dir)`。
/// UV: `theta=0`（n_width 側 = left = u=0.0）→ `theta=π`（-n_width 側 = right = u=1.0）。
fn append_round_cap(
    positions: &mut Vec<[f32; 3]>,
    normals: &mut Vec<[f32; 3]>,
    uvs: &mut Vec<[f32; 2]>,
    indices: &mut Vec<u32>,
    cap: RoundCapInput,
) {
    let RoundCapInput {
        center,
        n_width,
        bulge_dir,
        half_width,
        y_offset,
    } = cap;
    let seg = ROUND_CAP_SEGMENTS;
    let center_idx = positions.len() as u32;
    push_vertex_xz(positions, normals, center, y_offset);
    uvs.push([0.5, 0.0]);

    let arc_start = positions.len() as u32;
    for i in 0..=seg {
        let s = i as f32 / seg as f32;
        let theta = std::f32::consts::PI * s;
        let p = center + half_width * (theta.cos() * n_width + theta.sin() * bulge_dir);
        push_vertex_xz(positions, normals, p, y_offset);
        uvs.push([s, 0.0]); // s=0 → u=0.0 (left), s=1 → u=1.0 (right)
    }

    for i in 0..seg {
        indices.extend_from_slice(&[center_idx, arc_start + i, arc_start + i + 1]);
    }
}

/// 密な点列（Vec2）からミター補正クワッドストリップ Mesh を生成する。
///
/// Vec2(wx, wy) → Vec3(wx, y_offset, -wy) で 3D 化（タイルスポーンと同一ルール）。
///
/// **ミター補正**: 各点で隣接セグメントの法線を合成し、角度に依存した幅補正（miter scale）を
/// 適用して頂点位置を決める。隣接セグメント間で頂点を **共有** するため、カーブ部での
/// クワッド外縁ギャップ（短冊アーティファクト）が発生しない。
///
/// ミタースケールの上限を `MAX_MITER_SCALE` でクランプし、鋭角コーナーでの頂点突出を防ぐ。
///
/// **開チェーン**（`is_closed == false`）では `add_start_cap` / `add_end_cap` に従い
/// 端部にラウンドキャップを付ける。ジャンクション点（三叉路）では false を渡してキャップを省略する。
pub fn build_quad_strip_mesh(
    points: &[Vec2],
    width: f32,
    y_offset: f32,
    is_closed: bool,
    add_start_cap: bool,
    add_end_cap: bool,
) -> Mesh {
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD);
    let raw_n = points.len();
    if raw_n < 2 {
        return mesh;
    }

    // `sample_catmull_rom` は閉曲線で先頭点を末尾に複製する。重複を除く。
    let points: &[Vec2] = if is_closed
        && raw_n >= 2
        && points[0].distance_squared(points[raw_n - 1]) < 1e-12
    {
        &points[..raw_n - 1]
    } else {
        points
    };
    let n = points.len();
    if n < 2 {
        return mesh;
    }

    let hw = width * 0.5;
    /// 鋭角コーナーでのミター突出上限（これ以上伸長しない）。
    const MAX_MITER_SCALE: f32 = 4.0;
    let num_segs = if is_closed { n } else { n - 1 };

    // ── セグメント法線の事前計算 ──────────────────────────────────────────────
    let mut seg_normals: Vec<Vec2> = Vec::with_capacity(num_segs);
    for s in 0..num_segs {
        let i = s;
        let j = if is_closed { (s + 1) % n } else { s + 1 };
        let d = points[j] - points[i];
        let len = d.length();
        seg_normals.push(if len > 1e-8 {
            Vec2::new(-d.y, d.x) / len
        } else {
            Vec2::Y
        });
    }

    // ── 各点のミター補正頂点を計算 ──────────────────────────────────────────
    let mut lefts: Vec<Vec2> = Vec::with_capacity(n);
    let mut rights: Vec<Vec2> = Vec::with_capacity(n);

    for i in 0..n {
        let (l, r) = miter_pair(
            points[i],
            if is_closed {
                seg_normals[(i + num_segs - 1) % num_segs]
            } else if i == 0 {
                seg_normals[0]
            } else {
                seg_normals[i - 1]
            },
            if is_closed {
                seg_normals[i % num_segs]
            } else if i == n - 1 {
                seg_normals[num_segs - 1]
            } else {
                seg_normals[i]
            },
            hw,
            MAX_MITER_SCALE,
        );
        lefts.push(l);
        rights.push(r);
    }

    // ── 頂点・UV・インデックスの構築 ────────────────────────────────────────
    let cap_count = if is_closed { 0 } else { add_start_cap as usize + add_end_cap as usize };
    let cap_verts = cap_count * (ROUND_CAP_SEGMENTS as usize + 2);
    let mut positions: Vec<[f32; 3]> = Vec::with_capacity(n * 2 + cap_verts);
    let mut normals_buf: Vec<[f32; 3]> = Vec::with_capacity(n * 2 + cap_verts);
    let mut uvs: Vec<[f32; 2]> = Vec::with_capacity(n * 2 + cap_verts);
    let cap_idx = cap_count * ROUND_CAP_SEGMENTS as usize * 3;
    let mut indices: Vec<u32> = Vec::with_capacity(num_segs * 6 + cap_idx);

    for i in 0..n {
        push_vertex_xz(&mut positions, &mut normals_buf, lefts[i], y_offset);
        uvs.push([0.0, 0.0]);
        push_vertex_xz(&mut positions, &mut normals_buf, rights[i], y_offset);
        uvs.push([1.0, 0.0]);
    }

    for s in 0..num_segs {
        let a = (s * 2) as u32;
        let b = if is_closed {
            ((s + 1) % n * 2) as u32
        } else {
            ((s + 1) * 2) as u32
        };
        // CCW 巻き順（Bevy FrontFace::Ccw）
        indices.extend_from_slice(&[a, a + 1, b, a + 1, b + 1, b]);
    }

    // ── ラウンドキャップ（開チェーンのみ、ジャンクション点はスキップ）──────────
    if !is_closed && n >= 2 {
        if add_start_cap {
            let n0 = seg_normals[0];
            let t0 = Vec2::new(n0.y, -n0.x);
            append_round_cap(
                &mut positions,
                &mut normals_buf,
                &mut uvs,
                &mut indices,
                RoundCapInput {
                    center: points[0],
                    n_width: n0,
                    bulge_dir: -t0,
                    half_width: hw,
                    y_offset,
                },
            );
        }

        if add_end_cap {
            let nl = seg_normals[num_segs - 1];
            let tl = Vec2::new(nl.y, -nl.x);
            append_round_cap(
                &mut positions,
                &mut normals_buf,
                &mut uvs,
                &mut indices,
                RoundCapInput {
                    center: points[n - 1],
                    n_width: nl,
                    bulge_dir: tl,
                    half_width: hw,
                    y_offset,
                },
            );
        }
    }

    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals_buf);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

/// 2 セグメントの法線からミター補正した左右頂点オフセットを計算する。
///
/// 両端点（開チェーンの end points）では prev/next が同じ法線を渡す。
/// スケールを `max_scale` でクランプし鋭角コーナーでの頂点突出を防ぐ。
fn miter_pair(
    center: Vec2,
    n_prev: Vec2,
    n_next: Vec2,
    hw: f32,
    max_scale: f32,
) -> (Vec2, Vec2) {
    let m = (n_prev + n_next).normalize_or_zero();
    if m.length_squared() < 1e-8 {
        // 対向セグメント（真逆）→ prev 法線をそのまま使う
        return (center + n_prev * hw, center - n_prev * hw);
    }
    let scale = (1.0 / m.dot(n_prev).max(1.0 / max_scale)).min(max_scale);
    (center + m * hw * scale, center - m * hw * scale)
}

/// PostStartup で呼ばれる境界曲線メッシュスポーンシステム。
///
/// GeneratedWorldLayoutResource から地形タイルを読み取り、
/// 全境界種別のポリラインを生成・スポーンする。
/// 各リボンに `BoundarySurfaceMaterial` を割り当て、world-space UV で地形テクスチャをブレンドする。
pub fn spawn_boundary_meshes(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut boundary_materials: ResMut<Assets<hw_visual::BoundarySurfaceMaterial>>,
    layout: Res<GeneratedWorldLayoutResource>,
    game_assets: Res<GameAssets>,
    feature_map: Res<TerrainFeatureMap>,
) {
    let terrain_tiles = &layout.layout.terrain_tiles;
    let master_seed = layout.master_seed;

    let edges = extract_boundary_edges(terrain_tiles, &layout.layout.masks);
    let junctions = boundary_junction_corner_keys(&edges);
    let polylines = chain_edges_to_polylines(edges);
    let count = polylines.len();

    // (BoundaryKind, left_id, right_id) → マテリアルハンドルのキャッシュ。
    // ポリラインごとに left/right が異なる場合があるため、kind 単位ではなく
    // テクスチャ構成まで含めたキーでキャッシュする。
    let mut kind_material_cache: HashMap<
        (BoundaryKind, u8, u8),
        Handle<hw_visual::BoundarySurfaceMaterial>,
    > = HashMap::new();

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

        // ジャンクション点（三叉路以上）ではラウンドキャップが複数リボン間で干渉して
        // 円形のアーティファクトを生む。ジャンクション端点にはキャップを付けない。
        let add_start_cap = !polyline.is_closed
            && !polyline.points.is_empty()
            && !junctions.contains(&world_to_corner_key(polyline.points[0]));
        let add_end_cap = !polyline.is_closed
            && polyline.points.len() > 1
            && !junctions.contains(&world_to_corner_key(*polyline.points.last().unwrap()));

        let mesh = build_quad_strip_mesh(
            &sampled,
            STRIP_WIDTH,
            Y_MAP_BOUNDARY_BASE,
            polyline.is_closed,
            add_start_cap,
            add_end_cap,
        );
        let mesh_handle = meshes.add(mesh);

        let left_id = terrain_coarse_id(polyline.left_terrain);
        let right_id = terrain_coarse_id(polyline.right_terrain);
        let cache_key = (polyline.kind, left_id, right_id);
        let material_handle = kind_material_cache.entry(cache_key).or_insert_with(|| {
            boundary_materials.add(make_boundary_surface_material(
                BoundarySurfaceMaterialExt {
                    uniforms: BoundarySurfaceUniform {
                        left_terrain_id: left_id as f32,
                        right_terrain_id: right_id as f32,
                        uv_scale: 1.0 / TILE_SIZE,
                        blend_softness: 0.15,
                    },
                    grass_albedo: Some(game_assets.grass.clone()),
                    dirt_albedo: Some(game_assets.dirt.clone()),
                    sand_albedo: Some(game_assets.sand.clone()),
                    river_albedo: Some(game_assets.river.clone()),
                    terrain_macro_noise: Some(game_assets.terrain_macro_noise.clone()),
                    grass_macro_overlay: Some(game_assets.grass_macro_overlay.clone()),
                    dirt_macro_overlay: Some(game_assets.dirt_macro_overlay.clone()),
                    sand_macro_overlay: Some(game_assets.sand_macro_overlay.clone()),
                    terrain_feature_map: Some(feature_map.image.clone()),
                    terrain_feature_lut: Some(game_assets.terrain_feature_lut.clone()),
                    shoreline_detail: Some(game_assets.shoreline_detail.clone()),
                },
            ))
        });

        // ── メインリボン（ノイズ変位済み Catmull-Rom 曲線） ───────────────────
        commands.spawn((
            Mesh3d(mesh_handle),
            MeshMaterial3d(material_handle.clone()),
            Transform::IDENTITY,
            RenderLayers::from_layers(&[LAYER_3D]),
            BoundaryMeshMarker,
        ));
    }

    commands.insert_resource(BoundarySliceSpatialIndex);

    info!("BEVY_STARTUP: Spawned {} boundary polylines", count);
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
