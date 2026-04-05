//! 地形境界曲線メッシュ生成
//!
//! WFC 生成タイルの境界をグリッドエッジから抽出し、ノイズ変位と Catmull-Rom スプラインで
//! 有機的な曲線境界メッシュを PostStartup 時にスポーンする。
//!
//! **純粋ビジュアル**: ゲームロジック・当たり判定・AI 経路に一切影響しない。

use std::collections::HashMap;

use bevy::asset::RenderAssetUsages;
use bevy::camera::visibility::RenderLayers;
use bevy::mesh::{Indices, Mesh, PrimitiveTopology};
use bevy::prelude::*;
use hw_core::constants::{LAYER_3D, MAP_HEIGHT, MAP_WIDTH, TILE_SIZE, Y_MAP_BOUNDARY_BASE};
use hw_world::{TerrainType, grid_to_world};

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

/// クワッドストリップの幅（ワールド単位）。
const STRIP_WIDTH: f32 = TILE_SIZE * 0.2; // 6.4

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
}

impl BoundaryKind {
    /// 2 つの TerrainType から BoundaryKind を決定する（順序非依存）。
    /// 同種の場合は None を返す。
    pub fn from_pair(a: TerrainType, b: TerrainType) -> Option<Self> {
        if a == b {
            return None;
        }
        let (hi, lo) = if a.priority() > b.priority() {
            (a, b)
        } else {
            (b, a)
        };
        match (hi, lo) {
            (TerrainType::Grass, TerrainType::Dirt) => Some(Self::GrassDirt),
            (TerrainType::Grass, TerrainType::Sand) => Some(Self::GrassSand),
            (TerrainType::Grass, TerrainType::River) => Some(Self::GrassRiver),
            (TerrainType::Dirt, TerrainType::Sand) => Some(Self::DirtSand),
            (TerrainType::Dirt, TerrainType::River) => Some(Self::DirtRiver),
            (TerrainType::Sand, TerrainType::River) => Some(Self::SandRiver),
            _ => None,
        }
    }

    /// この境界種別のインデックス（per-kind seed 生成用）。
    pub fn index(self) -> u32 {
        self as u32
    }

    /// この境界種別に対応する表示色。
    pub fn color(self) -> Color {
        match self {
            Self::GrassDirt => Color::srgb(0.35, 0.22, 0.08),
            Self::GrassSand => Color::srgb(0.85, 0.78, 0.45),
            Self::GrassRiver => Color::srgb(0.15, 0.45, 0.70),
            Self::DirtSand => Color::srgb(0.70, 0.60, 0.35),
            Self::DirtRiver => Color::srgb(0.20, 0.35, 0.55),
            Self::SandRiver => Color::srgb(0.45, 0.65, 0.75),
        }
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

// ── M1: エッジ抽出と連結 ──────────────────────────────────────────────────────

/// terrain_tiles（row-major: y*MAP_WIDTH+x）から全境界エッジを抽出する。
pub fn extract_boundary_edges(terrain_tiles: &[TerrainType]) -> Vec<BoundaryEdge> {
    let w = MAP_WIDTH as usize;
    let h = MAP_HEIGHT as usize;
    let half = TILE_SIZE / 2.0;
    let mut edges = Vec::new();

    // 水平エッジ: セル (x, y) と (x, y+1) の境界
    for y in 0..h - 1 {
        for x in 0..w {
            let t0 = terrain_tiles[y * w + x];
            let t1 = terrain_tiles[(y + 1) * w + x];
            if let Some(kind) = BoundaryKind::from_pair(t0, t1) {
                let center = grid_to_world(x as i32, y as i32);
                edges.push(BoundaryEdge {
                    a: Vec2::new(center.x - half, center.y + half),
                    b: Vec2::new(center.x + half, center.y + half),
                    kind,
                });
            }
        }
    }

    // 垂直エッジ: セル (x, y) と (x+1, y) の境界
    for y in 0..h {
        for x in 0..w - 1 {
            let t0 = terrain_tiles[y * w + x];
            let t1 = terrain_tiles[y * w + x + 1];
            if let Some(kind) = BoundaryKind::from_pair(t0, t1) {
                let center = grid_to_world(x as i32, y as i32);
                edges.push(BoundaryEdge {
                    a: Vec2::new(center.x + half, center.y - half),
                    b: Vec2::new(center.x + half, center.y + half),
                    kind,
                });
            }
        }
    }

    edges
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
            let points = follow_chain(
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
            let points = follow_chain(
                start_key,
                start_idx,
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
                    is_closed: true,
                    kind,
                });
            }
        }
    }

    result
}

/// 指定コーナーから始まる連続チェーンを辿り、Vec<Vec2> の点列を返す。
fn follow_chain(
    start_key: (i32, i32),
    first_edge_idx: usize,
    edges: &[BoundaryEdge],
    corner_keys: &[[(i32, i32); 2]],
    adj: &HashMap<(i32, i32), Vec<usize>>,
    visited: &mut [bool],
) -> Vec<Vec2> {
    let mut points = Vec::new();
    let mut cur_key = start_key;
    let mut cur_edge_idx = first_edge_idx;

    loop {
        visited[cur_edge_idx] = true;
        let [ka, kb] = corner_keys[cur_edge_idx];
        let edge = &edges[cur_edge_idx];

        if points.is_empty() {
            if ka == cur_key {
                points.push(edge.a);
                points.push(edge.b);
                cur_key = kb;
            } else {
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

    points
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

/// ポリラインの各制御点を法線方向にノイズ変位した点列を返す。
pub fn displace_polyline(
    polyline: &BoundaryPolyline,
    seed: u32,
    freq: f32,
    amplitude: f32,
) -> Vec<Vec2> {
    let points = &polyline.points;
    let arcs = &polyline.arc_lengths;
    let n = points.len();
    let mut result = Vec::with_capacity(n);

    for i in 0..n {
        let tangent = compute_tangent(points, i, polyline.is_closed);
        let normal = Vec2::new(-tangent.y, tangent.x);
        let displacement = value_noise_1d(arcs[i] * freq, seed) * amplitude;
        result.push(points[i] + normal * displacement);
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
            (points[1] - points[n - 2]).normalize_or_zero()
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

/// 密な点列（Vec2）からクワッドストリップ Mesh を生成する。
///
/// Vec2(wx, wy) → Vec3(wx, y_offset, -wy) で 3D 化（タイルスポーンと同一ルール）。
pub fn build_quad_strip_mesh(points: &[Vec2], width: f32, y_offset: f32) -> Mesh {
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD);
    let n = points.len();
    if n < 2 {
        return mesh;
    }

    let hw = width * 0.5;
    let mut positions: Vec<[f32; 3]> = Vec::with_capacity(n * 2);
    let mut normals: Vec<[f32; 3]> = Vec::with_capacity(n * 2);
    let mut indices: Vec<u32> = Vec::with_capacity((n - 1) * 6);

    for i in 0..n {
        let tangent = compute_tangent(points, i, false);
        let normal_2d = Vec2::new(-tangent.y, tangent.x);
        let p = points[i];
        let left = p + normal_2d * hw;
        let right = p - normal_2d * hw;

        // Vec2(wx, wy) → Vec3(wx, y_offset, -wy)
        positions.push([left.x, y_offset, -left.y]);
        positions.push([right.x, y_offset, -right.y]);
        normals.push([0.0, 1.0, 0.0]);
        normals.push([0.0, 1.0, 0.0]);
    }

    for i in 0..n - 1 {
        let l0 = (2 * i) as u32;
        let r0 = (2 * i + 1) as u32;
        let l1 = (2 * (i + 1)) as u32;
        let r1 = (2 * (i + 1) + 1) as u32;
        // Vulkan Y-down NDC で CCW（Bevy FrontFace::Ccw）になる巻き順。
        // cross(r0-l0, l1-l0) の Z 成分が負 → 表面。
        indices.extend_from_slice(&[l0, r0, l1, r0, r1, l1]);
    }

    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

/// PostStartup で呼ばれる境界曲線メッシュスポーンシステム。
///
/// GeneratedWorldLayoutResource から地形タイルを読み取り、
/// 全境界種別のポリラインを生成・スポーンする。
pub fn spawn_boundary_meshes(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    layout: Res<GeneratedWorldLayoutResource>,
) {
    let terrain_tiles = &layout.layout.terrain_tiles;
    let master_seed = layout.master_seed;

    let edges = extract_boundary_edges(terrain_tiles);
    let polylines = chain_edges_to_polylines(edges);
    let count = polylines.len();

    for polyline in polylines {
        let seed = (master_seed
            ^ (polyline.kind.index() as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15))
            as u32;
        let displaced = displace_polyline(&polyline, seed, NOISE_FREQ, NOISE_AMPLITUDE);
        let sampled = sample_catmull_rom(&displaced, polyline.is_closed, CATMULL_ROM_STEPS);
        if sampled.len() < 2 {
            continue;
        }

        let mesh = build_quad_strip_mesh(&sampled, STRIP_WIDTH, Y_MAP_BOUNDARY_BASE);
        let mesh_handle = meshes.add(mesh);
        let material_handle = materials.add(StandardMaterial {
            base_color: polyline.kind.color(),
            unlit: true,
            ..default()
        });

        commands.spawn((
            Mesh3d(mesh_handle),
            MeshMaterial3d(material_handle),
            Transform::IDENTITY,
            RenderLayers::from_layers(&[LAYER_3D]),
        ));
    }

    info!("BEVY_STARTUP: Spawned {} boundary polylines", count);
}
